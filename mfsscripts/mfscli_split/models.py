class ChunkServer:
    """Represents a MooseFS chunk server"""
    def __init__(self, host=None, port=None):
        self.host = host
        self.port = port
        self.version = (0, 0, 0)
        self.total_space = 0
        self.used_space = 0
        self.blocks = 0
        self.tdisks = 0
        self.state = 0
        self.flags = 0
        self.load_factor = 0
        self.label = ""
        self.hdds = []
        self.last_state_change = 0

    @property
    def id(self):
        """Get unique identifier for this chunk server"""
        if self.host and self.port:
            return f"{self.host}:{self.port}"
        return "unknown"
    
    @property
    def free_space(self):
        """Get free space on this chunk server"""
        return self.total_space - self.used_space

    @property
    def usage_percent(self):
        """Get used space percentage"""
        if self.total_space > 0:
            return (self.used_space * 100.0) / self.total_space
        return 0.0

class HDD:
    """Represents a hard drive in a MooseFS chunk server"""
    def __init__(self, path=None):
        self.path = path
        self.flags = 0
        self.error_count = 0
        self.read_bytes = 0
        self.write_bytes = 0
        self.used_space = 0
        self.total_space = 0
        self.chunks = 0
        self.last_error = 0
        self.last_error_timestamp = 0
        self.status = 0

    @property
    def id(self):
        """Get unique identifier for this HDD"""
        return self.path or "unknown"
    
    @property
    def free_space(self):
        """Get free space on this HDD"""
        return self.total_space - self.used_space
    
    @property
    def usage_percent(self):
        """Get used space percentage"""
        if self.total_space > 0:
            return (self.used_space * 100.0) / self.total_space
        return 0.0 