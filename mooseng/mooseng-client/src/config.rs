use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Master server address
    pub master_addr: SocketAddr,
    
    /// Mount point path
    pub mount_point: PathBuf,
    
    /// Client session timeout
    pub session_timeout: Duration,
    
    /// Connection timeout
    pub connect_timeout: Duration,
    
    /// Cache configuration
    pub cache: CacheConfig,
    
    /// File system options
    pub fs_options: FsOptions,
    
    /// Debug level (0-3)
    pub debug_level: u8,
    
    /// Enable read-only mode
    pub read_only: bool,
    
    /// Allow other users to access the mount
    pub allow_other: bool,
    
    /// Allow root to access the mount
    pub allow_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Metadata cache size (number of entries)
    pub metadata_cache_size: usize,
    
    /// Metadata cache TTL
    pub metadata_cache_ttl: Duration,
    
    /// Directory cache size (number of entries)
    pub dir_cache_size: usize,
    
    /// Directory cache TTL
    pub dir_cache_ttl: Duration,
    
    /// Data cache size in bytes (0 to disable)
    pub data_cache_size: u64,
    
    /// Data cache block size
    pub data_cache_block_size: usize,
    
    /// Data cache TTL
    pub data_cache_ttl: Duration,
    
    /// Enable negative cache for non-existent files
    pub enable_negative_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsOptions {
    /// Maximum number of open files
    pub max_open_files: usize,
    
    /// Read ahead size in bytes
    pub read_ahead_size: usize,
    
    /// Write back cache enabled
    pub write_back_cache: bool,
    
    /// Asynchronous read operations
    pub async_read: bool,
    
    /// Default file mode for new files
    pub default_file_mode: u16,
    
    /// Default directory mode for new directories
    pub default_dir_mode: u16,
    
    /// Enable extended attributes
    pub enable_xattr: bool,
    
    /// Enable POSIX ACLs
    pub enable_acl: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            master_addr: "127.0.0.1:9421".parse().unwrap(),
            mount_point: PathBuf::from("/mnt/moosefs"),
            session_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(10),
            cache: CacheConfig::default(),
            fs_options: FsOptions::default(),
            debug_level: 0,
            read_only: false,
            allow_other: false,
            allow_root: false,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            metadata_cache_size: 10000,
            metadata_cache_ttl: Duration::from_secs(10),
            dir_cache_size: 1000,
            dir_cache_ttl: Duration::from_secs(5),
            data_cache_size: 256 * 1024 * 1024, // 256MB
            data_cache_block_size: 64 * 1024,   // 64KB
            data_cache_ttl: Duration::from_secs(30),
            enable_negative_cache: true,
        }
    }
}

impl Default for FsOptions {
    fn default() -> Self {
        Self {
            max_open_files: 1024,
            read_ahead_size: 128 * 1024, // 128KB
            write_back_cache: true,
            async_read: true,
            default_file_mode: 0o644,
            default_dir_mode: 0o755,
            enable_xattr: true,
            enable_acl: false,
        }
    }
}

impl ClientConfig {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> crate::ClientResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    pub fn to_file<P: AsRef<std::path::Path>>(&self, path: P) -> crate::ClientResult<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}