import struct
import socket
import time
import traceback
from .constants import CLTOMA_MASS_RESOLVE_PATHS, MATOCL_MASS_RESOLVE_PATHS, UNRESOLVED

def myunicode(x):
    """Convert to unicode string"""
    return str(x)

def print_exception():
    """Print current exception traceback"""
    tracedata = traceback.format_exc()
    print("<pre>%s</pre>" % tracedata.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))

def gettzoff(t):
    """Get timezone offset"""
    return time.localtime(t).tm_gmtoff

def shiftts(t):
    """Shift timestamp by timezone offset"""
    return t - gettzoff(t)

def resolve(strip):
    """Resolve IP to hostname"""
    try:
        return (socket.gethostbyaddr(strip))[0]
    except Exception:
        return UNRESOLVED

def create_chunks(list_name, n):
    """Split a list into chunks of size n"""
    listobj = list(list_name)
    for i in range(0, len(listobj), n):
        yield listobj[i:i + n]

def resolve_inodes_paths(masterconn, inodes):
    """Resolve inode IDs to paths"""
    inodepaths = {}
    for chunk in create_chunks(inodes, 100):
        if len(chunk) > 0:
            # Prepare format string for the number of inodes in this chunk
            fmt = ">" + len(chunk) * "L"
            data, length = masterconn.command(CLTOMA_MASS_RESOLVE_PATHS, MATOCL_MASS_RESOLVE_PATHS, 
                                           struct.pack(fmt, *chunk))
            pos = 0
            while pos + 8 <= length:
                inode, psize = struct.unpack(">LL", data[pos:pos+8])
                pos += 8
                if psize == 0:
                    if inode not in inodepaths:
                        inodepaths[inode] = []
                    inodepaths[inode].append("./META")
                elif pos + psize <= length:
                    while psize >= 4:
                        pleng = struct.unpack(">L", data[pos:pos+4])[0]
                        pos += 4
                        psize -= 4
                        path = data[pos:pos+pleng]
                        pos += pleng
                        psize -= pleng
                        path = path.decode('utf-8', 'replace')
                        if inode not in inodepaths:
                            inodepaths[inode] = []
                        inodepaths[inode].append(path)
                    if psize != 0:
                        raise RuntimeError("MFS packet malformed")
            if pos != length:
                raise RuntimeError("MFS packet malformed")
    return inodepaths

def version_str_and_sort(version_tuple):
    """
    Convert version tuple to string and sortable representation
    Returns: (version_string, version_sort_key)
    """
    v1, v2, v3 = version_tuple
    return f"{v1}.{v2}.{v3}", (v1, v2, v3)

def cmp_ver(ver1, ver2):
    """
    Compare version strings
    Returns: 1 if ver1 > ver2, -1 if ver1 < ver2, 0 if equal
    """
    v1 = tuple(map(int, ver1.split('.')))
    v2 = tuple(map(int, ver2.split('.')))
    
    if v1 > v2:
        return 1
    elif v1 < v2:
        return -1
    else:
        return 0 