import struct
import socket
import time

from .constants import *

class Master:
    """Represents a MooseFS master server"""
    def __init__(self, host, port):
        self.host = host
        self.port = port
        self.version = (0, 0, 0)
        self.features = []
        self.conn = None
        self._connect()
    
    def _connect(self):
        """Connect to master server"""
        try:
            self.conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.conn.connect((self.host, self.port))
            return True
        except socket.error:
            self.conn = None
            return False
    
    def set_version(self, version):
        """Set the master server version"""
        self.version = version
    
    def version_at_least(self, major, minor, micro):
        """Check if master version is at least the specified version"""
        return (self.version[0] > major or 
               (self.version[0] == major and self.version[1] > minor) or
               (self.version[0] == major and self.version[1] == minor and self.version[2] >= micro))
    
    def version_less_than(self, major, minor, micro):
        """Check if master version is less than the specified version"""
        return (self.version[0] < major or 
               (self.version[0] == major and self.version[1] < minor) or
               (self.version[0] == major and self.version[1] == minor and self.version[2] < micro))
               
    def version_unknown(self):
        """Check if master version is unknown"""
        return self.version == (0, 0, 0)
        
    def has_feature(self, feature):
        """Check if master server has a specific feature"""
        return feature in self.features
    
    def command(self, command, response, data=None):
        """Send command to master and receive response"""
        if not self.conn and not self._connect():
            raise ConnectionError(f"Can't connect to master: {self.host}:{self.port}")
        
        try:
            # Prepare the command packet
            if data:
                packet = struct.pack(">LL", command, len(data)) + data
            else:
                packet = struct.pack(">LL", command, 0)
            
            # Send the command
            self.conn.sendall(packet)
            
            # Read the response
            header = b""
            while len(header) < 8:
                data = self.conn.recv(8 - len(header))
                if not data:
                    raise ConnectionError("Connection closed by master")
                header += data
            
            cmd, length = struct.unpack(">LL", header)
            
            if cmd != response:
                raise RuntimeError(f"Invalid response: expected {response}, got {cmd}")
            
            if length == 0:
                return b"", 0
            
            # Read the response data
            res_data = b""
            while len(res_data) < length:
                data = self.conn.recv(length - len(res_data))
                if not data:
                    raise ConnectionError("Connection closed by master")
                res_data += data
            
            return res_data, length
        except socket.error:
            # Close and reset connection
            if self.conn:
                try:
                    self.conn.close()
                except:
                    pass
                self.conn = None
            raise

class MFSConn:
    """Connection to a MooseFS server"""
    def __init__(self, host, port):
        self.host = host
        self.port = port
        self.conn = None
    
    def _connect(self):
        """Connect to server"""
        try:
            self.conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.conn.connect((self.host, self.port))
            return True
        except socket.error:
            self.conn = None
            return False
        
    def command(self, command, response, data=None):
        """Send command to server and receive response"""
        if not self.conn and not self._connect():
            raise ConnectionError(f"Can't connect to server: {self.host}:{self.port}")
        
        try:
            # Prepare the command packet
            if data:
                packet = struct.pack(">LL", command, len(data)) + data
            else:
                packet = struct.pack(">LL", command, 0)
            
            # Send the command
            self.conn.sendall(packet)
            
            # Read the response
            header = b""
            while len(header) < 8:
                chunk = self.conn.recv(8 - len(header))
                if not chunk:
                    raise ConnectionError("Connection closed by server")
                header += chunk
            
            cmd, length = struct.unpack(">LL", header)
            
            if cmd != response:
                raise RuntimeError(f"Invalid response: expected {response}, got {cmd}")
            
            if length == 0:
                return b"", 0
            
            # Read the response data
            res_data = b""
            while len(res_data) < length:
                chunk = self.conn.recv(length - len(res_data))
                if not chunk:
                    raise ConnectionError("Connection closed by server")
                res_data += chunk
            
            return res_data, length
        except socket.error:
            # Close and reset connection
            if self.conn:
                try:
                    self.conn.close()
                except:
                    pass
                self.conn = None
            raise

class MFSMultiConn:
    """Multiple connections to MooseFS servers"""
    def __init__(self):
        self.connections = {}
        
    def register(self, host, port):
        """Register a new connection to a server"""
        key = f"{host}:{port}"
        if key not in self.connections:
            self.connections[key] = MFSConn(host, port)
            
    def command(self, command, response, data=None):
        """Send command to all servers and collect responses"""
        results = {}
        for key, conn in self.connections.items():
            try:
                results[key] = conn.command(command, response, data)
            except Exception as e:
                results[key] = (str(e), 0)
        return results 