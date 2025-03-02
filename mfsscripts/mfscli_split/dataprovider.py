import struct
import time
from datetime import datetime

from .constants import *
from .models import ChunkServer, HDD
from .utils import resolve

class DataProvider:
    """Provides data from MooseFS master server"""
    def __init__(self, master_conn):
        self.master_conn = master_conn
        self.last_update = 0
        self.update_interval = 5  # seconds
        
    def get_master_info(self):
        """Get master server info"""
        data, length = self.master_conn.command(CLTOMA_INFO, MATOCL_INFO)
        if length >= 12:
            v1, v2, v3, state, metaversion, ip1, ip2, ip3, ip4 = struct.unpack(">HBBBIBBBB", data[:12])
            version = (v1, v2, v3)
            ip = f"{ip1}.{ip2}.{ip3}.{ip4}"
            
            result = {
                'version': version,
                'version_string': f"{v1}.{v2}.{v3}",
                'state': state,
                'meta_version': metaversion,
                'ip': ip,
                'is_leader': state == STATE_LEADER,
                'is_electing': state == STATE_ELECT
            }
            
            # Additional data based on version
            pos = 12
            if length >= pos + 4:
                result['total_space'] = struct.unpack(">L", data[pos:pos+4])[0]
                pos += 4
            if length >= pos + 4:
                result['available_space'] = struct.unpack(">L", data[pos:pos+4])[0]
                pos += 4
            if length >= pos + 4:
                result['chunk_count'] = struct.unpack(">L", data[pos:pos+4])[0]
                pos += 4
                
            return result
        return None
    
    def get_chunkservers(self):
        """Get list of chunk servers"""
        if not self.master_conn.version_at_least(1, 6, 0):
            return []
            
        data, length = self.master_conn.command(CLTOMA_CSSERV_COMMAND, MATOCL_CSSERV_COMMAND, 
                                             struct.pack(">B", 0xFF))  # Special command to list servers
                                             
        chunkservers = []
        offset = 0
        
        while offset + 18 <= length:
            ip1, ip2, ip3, ip4, port, flags, tdisks, total_space = struct.unpack(">BBBBHHQL", 
                                                                              data[offset:offset+18])
            offset += 18
            
            cs = ChunkServer()
            cs.host = f"{ip1}.{ip2}.{ip3}.{ip4}"
            cs.port = port
            cs.flags = flags
            cs.tdisks = tdisks
            cs.total_space = total_space
            
            # Get additional fields based on version
            if self.master_conn.version_at_least(1, 6, 26) and offset + 8 <= length:
                cs.used_space = struct.unpack(">Q", data[offset:offset+8])[0]
                offset += 8
                
            if self.master_conn.version_at_least(1, 7, 0) and offset + 4 <= length:
                cs.blocks = struct.unpack(">L", data[offset:offset+4])[0]
                offset += 4
                
            if self.master_conn.version_at_least(2, 0, 0) and offset + 4 <= length:
                label_len = struct.unpack(">L", data[offset:offset+4])[0]
                offset += 4
                if offset + label_len <= length:
                    cs.label = data[offset:offset+label_len].decode('utf-8', 'replace')
                    offset += label_len
                    
            # Add to list
            chunkservers.append(cs)
            
        return chunkservers
        
    def get_hdds(self, chunkserver):
        """Get list of HDDs for a specific chunk server"""
        if not self.master_conn.version_at_least(1, 6, 0):
            return []
            
        # Special command to get HDDs for a specific server
        ip1, ip2, ip3, ip4 = map(int, chunkserver.host.split('.'))
        data, length = self.master_conn.command(CLTOMA_CSSERV_COMMAND, MATOCL_CSSERV_COMMAND,
                                             struct.pack(">BBBBBH", 0xFE, ip1, ip2, ip3, ip4, chunkserver.port))
                                             
        hdds = []
        offset = 0
        
        while offset + 4 <= length:
            path_len = struct.unpack(">L", data[offset:offset+4])[0]
            offset += 4
            
            if offset + path_len <= length:
                path = data[offset:offset+path_len].decode('utf-8', 'replace')
                offset += path_len
                
                hdd = HDD(path)
                
                if offset + 4 <= length:
                    hdd.flags, hdd.error_count = struct.unpack(">BxH", data[offset:offset+4])
                    offset += 4
                    
                if offset + 16 <= length:
                    hdd.used_space, hdd.total_space = struct.unpack(">QQ", data[offset:offset+16])
                    offset += 16
                    
                if offset + 4 <= length:
                    hdd.chunks = struct.unpack(">L", data[offset:offset+4])[0]
                    offset += 4
                    
                if self.master_conn.version_at_least(1, 7, 0) and offset + 16 <= length:
                    hdd.read_bytes, hdd.write_bytes = struct.unpack(">QQ", data[offset:offset+16])
                    offset += 16
                    
                if self.master_conn.version_at_least(1, 7, 15) and offset + 8 <= length:
                    hdd.last_error, hdd.last_error_timestamp = struct.unpack(">LL", data[offset:offset+8])
                    offset += 8
                    
                hdds.append(hdd)
                
        return hdds 