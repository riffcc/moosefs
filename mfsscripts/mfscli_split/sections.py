import sys
import time
import struct
from datetime import datetime
from .constants import *
from .utils import resolve, myunicode
from .charts import charts_convert_data, get_charts_multi_data

def get_valid_sections():
    """Return list of valid section names"""
    return [
        "info", "memory", "cs", "hdd", "disks", "dir", "file", "fs", "errors", "exports",
        "mounts", "chunks", "quota", "configs", "stats", "chunkservers", "repl", "matocu", 
        "matocs", "operations", "matrix", "missing", "paths"
    ]

def is_valid_section(name):
    """Check if a section name is valid"""
    return name in get_valid_sections()

def display_section(name, masterconn, params=None):
    """Display a specific section"""
    if name == "info":
        return display_info_section(masterconn, params)
    elif name == "memory":
        return display_memory_section(masterconn, params)
    elif name == "cs" or name == "chunkservers":
        return display_chunkservers_section(masterconn, params)
    elif name == "hdd" or name == "disks":
        return display_disks_section(masterconn, params)
    elif name == "chunks":
        return display_chunks_section(masterconn, params)
    elif name == "operations":
        return display_operations_section(masterconn, params)
    elif name == "missing":
        return display_missing_chunks_section(masterconn, params)
    # Add other sections as needed
    else:
        print(f"Section '{name}' not implemented")
        return False

def display_info_section(masterconn, params=None):
    """Display general information about the MooseFS installation"""
    data, length = masterconn.command(CLTOMA_INFO, MATOCL_INFO)
    if length >= 12:
        v1, v2, v3, state, metaversion, ip1, ip2, ip3, ip4 = struct.unpack(">HBBBIBBBB", data[:12])
        ip = "%u.%u.%u.%u" % (ip1, ip2, ip3, ip4)
        version = (v1, v2, v3)
        print(f"MooseFS Master - version: {v1}.{v2}.{v3}, IP: {ip}")
        
        if state == STATE_LEADER:
            print("State: Leader")
        elif state == STATE_ELECT:
            print("State: Election")
        elif state == STATE_FOLLOWERS:
            print("State: Follower")
        else:
            print(f"State: Unknown ({state})")
        
        print(f"Metadata version: {metaversion}")
        
        pos = 12
        if length >= pos + 4:
            totalspace = struct.unpack(">L", data[pos:pos+4])[0]
            pos += 4
            print(f"Total space: {format_bytes(totalspace)}")
        
        if length >= pos + 4:
            availspace = struct.unpack(">L", data[pos:pos+4])[0]
            pos += 4
            print(f"Available space: {format_bytes(availspace)}")
            if totalspace > 0:
                print(f"Used space: {format_bytes(totalspace - availspace)} ({(totalspace - availspace) * 100.0 / totalspace:.2f}%)")
        
        if length >= pos + 4:
            chunks = struct.unpack(">L", data[pos:pos+4])[0]
            pos += 4
            print(f"Chunks: {chunks}")
        
        # Additional information can be added based on the master version
        return True
    else:
        print("Can't get information from master")
        return False

def display_memory_section(masterconn, params=None):
    """Display memory usage information"""
    data, length = masterconn.command(CLTOMA_FSTEST_INFO, MATOCL_FSTEST_INFO)
    if length >= 8:
        loopstart, loopend = struct.unpack(">LL", data[:8])
        pos = 8
        
        print("Memory Usage Information:")
        print(f"Loop start time: {format_time(loopstart)}")
        print(f"Loop end time: {format_time(loopend)}")
        
        # Parse memory usage data based on format
        while pos + 8 <= length:
            entryname_len = struct.unpack(">L", data[pos:pos+4])[0]
            pos += 4
            if pos + entryname_len <= length:
                entryname = data[pos:pos+entryname_len].decode('utf-8', 'replace')
                pos += entryname_len
                if pos + 4 <= length:
                    memory = struct.unpack(">L", data[pos:pos+4])[0]
                    pos += 4
                    print(f"{entryname}: {format_bytes(memory)}")
        
        return True
    else:
        print("Can't get memory information from master")
        return False

def display_chunkservers_section(masterconn, params=None):
    """Display information about chunk servers"""
    if not masterconn.version_at_least(1, 6, 0):
        print("Master too old - can't display chunkservers information")
        return False
    
    data, length = masterconn.command(CLTOMA_CSSERV_COMMAND, MATOCL_CSSERV_COMMAND, 
                                   struct.pack(">B", 0xFF))  # Special command to list servers
    
    if length == 0:
        print("No chunkservers connected")
        return True
    
    print("Chunkservers:")
    print("IP               | Port  | Label           | Status        | Space Used           | Total Space        | Chunks |")
    print("-----------------|-------|-----------------|---------------|----------------------|--------------------|---------")
    
    offset = 0
    while offset + 18 <= length:
        ip1, ip2, ip3, ip4, port, flags, tdisks, total_space = struct.unpack(">BBBBHHQL", 
                                                                      data[offset:offset+18])
        offset += 18
        
        ip = f"{ip1}.{ip2}.{ip3}.{ip4}"
        
        # Get additional fields based on version
        used_space = 0
        if masterconn.version_at_least(1, 6, 26) and offset + 8 <= length:
            used_space = struct.unpack(">Q", data[offset:offset+8])[0]
            offset += 8
            
        blocks = 0
        if masterconn.version_at_least(1, 7, 0) and offset + 4 <= length:
            blocks = struct.unpack(">L", data[offset:offset+4])[0]
            offset += 4
            
        label = ""
        if masterconn.version_at_least(2, 0, 0) and offset + 4 <= length:
            label_len = struct.unpack(">L", data[offset:offset+4])[0]
            offset += 4
            if offset + label_len <= length:
                label = data[offset:offset+label_len].decode('utf-8', 'replace')
                offset += label_len
        
        # Determine status
        status = "OK"
        if flags & CS_BEING_DELETED:
            status = "MARKED FOR REMOVAL"
        elif flags & CS_TEMPORARILY_NONOPERATIONAL:
            status = "NONOPERATIONAL"
        elif flags & CS_MAINTENANCE:
            status = "MAINTENANCE"
        
        # Format the output
        percent = (used_space * 100.0) / total_space if total_space > 0 else 0
        print(f"{ip:<16} | {port:<5} | {label:<15} | {status:<13} | {format_bytes(used_space):<20} | {format_bytes(total_space):<18} | {blocks:<7}")
    
    return True

def display_disks_section(masterconn, params=None):
    """Display information about disks in chunk servers"""
    # Implementation of disk section display
    return True

def display_chunks_section(masterconn, params=None):
    """Display chunk statistics"""
    # Implementation of chunks section display
    return True

def display_operations_section(masterconn, params=None):
    """Display operations statistics"""
    # Implementation of operations section display
    return True

def display_missing_chunks_section(masterconn, params=None):
    """Display missing chunks information"""
    data, length = masterconn.command(CLTOMA_MISSING_CHUNKS, MATOCL_MISSING_CHUNKS)
    if length >= 8:
        chunks, inodes = struct.unpack(">LL", data[:8])
        print(f"Missing chunks: {chunks}")
        print(f"Affected files: {inodes}")
        return True
    else:
        print("Can't get missing chunks info")
        return False

def format_bytes(size):
    """Format bytes to human-readable size"""
    if size >= 10 * 1024 * 1024 * 1024 * 1024:
        return f"{size/(1024*1024*1024*1024):.2f} TB"
    elif size >= 10 * 1024 * 1024 * 1024:
        return f"{size/(1024*1024*1024):.2f} GB"
    elif size >= 10 * 1024 * 1024:
        return f"{size/(1024*1024):.2f} MB"
    elif size >= 10 * 1024:
        return f"{size/1024:.2f} KB"
    else:
        return f"{size} bytes"

def format_time(timestamp):
    """Format timestamp to human-readable time"""
    return datetime.fromtimestamp(timestamp).strftime('%Y-%m-%d %H:%M:%S') 