use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the ChunkServer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkServerConfig {
    /// Base directory for chunk storage
    pub data_dir: PathBuf,
    
    /// Server bind address
    pub bind_address: String,
    
    /// Server port
    pub port: u16,
    
    /// Master server address
    pub master_address: String,
    
    /// Master server port
    pub master_port: u16,
    
    /// Maximum cache size in bytes
    pub cache_size_bytes: u64,
    
    /// Number of worker threads for I/O operations
    pub worker_threads: usize,
    
    /// Enable memory-mapped file access
    pub enable_mmap: bool,
    
    /// Maximum number of memory-mapped files
    pub max_mmap_files: usize,
    
    /// Enable data integrity checks
    pub enable_checksums: bool,
    
    /// Chunk verification interval in seconds
    pub verification_interval_secs: u64,
    
    /// Maximum concurrent chunk operations
    pub max_concurrent_ops: usize,
    
    /// Heartbeat interval to master in seconds
    pub heartbeat_interval_secs: u64,
    
    /// Chunk server unique identifier
    pub server_id: u16,
    
    /// Region ID for this chunk server
    pub region_id: u8,
    
    /// Rack ID for this chunk server
    pub rack_id: u16,
    
    /// Labels for chunk server categorization
    pub labels: Vec<String>,
    
    /// Enable detailed metrics collection
    pub enable_metrics: bool,
    
    /// Metrics export port
    pub metrics_port: Option<u16>,
}

impl Default for ChunkServerConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("/var/lib/mooseng/chunks"),
            bind_address: "0.0.0.0".to_string(),
            port: 9422,
            master_address: "127.0.0.1".to_string(),
            master_port: 9421,
            cache_size_bytes: 512 * 1024 * 1024, // 512MB
            worker_threads: num_cpus::get().max(4),
            enable_mmap: true,
            max_mmap_files: 1024,
            enable_checksums: true,
            verification_interval_secs: 300, // 5 minutes
            max_concurrent_ops: 100,
            heartbeat_interval_secs: 30,
            server_id: 1,
            region_id: 0,
            rack_id: 1,
            labels: Vec::new(),
            enable_metrics: true,
            metrics_port: Some(9423),
        }
    }
}

impl ChunkServerConfig {
    /// Load configuration from specific file
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let default_config = Self::default();
        
        let mut builder = config::Config::builder()
            .set_default("data_dir", default_config.data_dir.to_string_lossy().to_string())?
            .set_default("bind_address", default_config.bind_address)?
            .set_default("port", default_config.port as i64)?
            .set_default("master_address", default_config.master_address)?
            .set_default("master_port", default_config.master_port as i64)?
            .set_default("cache_size_bytes", default_config.cache_size_bytes as i64)?
            .set_default("worker_threads", default_config.worker_threads as i64)?
            .set_default("enable_mmap", default_config.enable_mmap)?
            .set_default("max_mmap_files", default_config.max_mmap_files as i64)?
            .set_default("enable_checksums", default_config.enable_checksums)?
            .set_default("verification_interval_secs", default_config.verification_interval_secs as i64)?
            .set_default("max_concurrent_ops", default_config.max_concurrent_ops as i64)?
            .set_default("heartbeat_interval_secs", default_config.heartbeat_interval_secs as i64)?
            .set_default("server_id", default_config.server_id as i64)?
            .set_default("region_id", default_config.region_id as i64)?
            .set_default("rack_id", default_config.rack_id as i64)?
            .set_default("enable_metrics", default_config.enable_metrics)?;
            
        if let Some(port) = default_config.metrics_port {
            builder = builder.set_default("metrics_port", port as i64)?;
        }
        
        // Load from specific file
        builder = builder.add_source(config::File::with_name(path.as_ref().to_string_lossy().as_ref()));
        
        // Override with environment variables
        builder = builder.add_source(config::Environment::with_prefix("MOOSENG_CHUNKSERVER"));
        
        let config: ChunkServerConfig = builder.build()?.try_deserialize()?;
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }

    /// Load configuration from file or environment
    pub fn load() -> anyhow::Result<Self> {
        let default_config = Self::default();
        
        let mut builder = config::Config::builder()
            .set_default("data_dir", default_config.data_dir.to_string_lossy().to_string())?
            .set_default("bind_address", default_config.bind_address)?
            .set_default("port", default_config.port as i64)?
            .set_default("master_address", default_config.master_address)?
            .set_default("master_port", default_config.master_port as i64)?
            .set_default("cache_size_bytes", default_config.cache_size_bytes as i64)?
            .set_default("worker_threads", default_config.worker_threads as i64)?
            .set_default("enable_mmap", default_config.enable_mmap)?
            .set_default("max_mmap_files", default_config.max_mmap_files as i64)?
            .set_default("enable_checksums", default_config.enable_checksums)?
            .set_default("verification_interval_secs", default_config.verification_interval_secs as i64)?
            .set_default("max_concurrent_ops", default_config.max_concurrent_ops as i64)?
            .set_default("heartbeat_interval_secs", default_config.heartbeat_interval_secs as i64)?
            .set_default("server_id", default_config.server_id as i64)?
            .set_default("region_id", default_config.region_id as i64)?
            .set_default("rack_id", default_config.rack_id as i64)?
            .set_default("enable_metrics", default_config.enable_metrics)?;
            
        if let Some(port) = default_config.metrics_port {
            builder = builder.set_default("metrics_port", port as i64)?;
        }
        
        // Try to load from file
        if let Ok(config_file) = std::env::var("MOOSENG_CHUNKSERVER_CONFIG") {
            builder = builder.add_source(config::File::with_name(&config_file));
        } else {
            // Try standard locations
            let config_paths = [
                "/etc/mooseng/chunkserver.toml",
                "./chunkserver.toml",
                "./config/chunkserver.toml",
            ];
            
            for path in &config_paths {
                if std::path::Path::new(path).exists() {
                    builder = builder.add_source(config::File::with_name(path));
                    break;
                }
            }
        }
        
        // Override with environment variables
        builder = builder.add_source(config::Environment::with_prefix("MOOSENG_CHUNKSERVER"));
        
        let config: ChunkServerConfig = builder.build()?.try_deserialize()?;
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Validate configuration parameters
    fn validate(&self) -> anyhow::Result<()> {
        if self.cache_size_bytes == 0 {
            return Err(anyhow::anyhow!("Cache size must be greater than 0"));
        }
        
        if self.worker_threads == 0 {
            return Err(anyhow::anyhow!("Worker threads must be greater than 0"));
        }
        
        if self.max_concurrent_ops == 0 {
            return Err(anyhow::anyhow!("Max concurrent operations must be greater than 0"));
        }
        
        if self.heartbeat_interval_secs == 0 {
            return Err(anyhow::anyhow!("Heartbeat interval must be greater than 0"));
        }
        
        Ok(())
    }
    
    /// Get the sharded directory path for a chunk
    pub fn chunk_dir_path(&self, chunk_id: u64) -> PathBuf {
        // Create a 2-level directory structure for sharding
        let level1 = (chunk_id >> 16) & 0xFF;
        let level2 = (chunk_id >> 8) & 0xFF;
        
        self.data_dir
            .join(format!("{:02x}", level1))
            .join(format!("{:02x}", level2))
    }
    
    /// Get the full file path for a chunk
    pub fn chunk_file_path(&self, chunk_id: u64, version: u32) -> PathBuf {
        self.chunk_dir_path(chunk_id)
            .join(format!("chunk_{:016x}_v{:08x}.dat", chunk_id, version))
    }
    
    /// Get the metadata file path for a chunk
    pub fn chunk_metadata_path(&self, chunk_id: u64, version: u32) -> PathBuf {
        self.chunk_dir_path(chunk_id)
            .join(format!("chunk_{:016x}_v{:08x}.meta", chunk_id, version))
    }
}