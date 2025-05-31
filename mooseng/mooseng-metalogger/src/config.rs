use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use config::{Config, ConfigError, File};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaloggerConfig {
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// Replication configuration
    pub replication: ReplicationConfig,
    
    /// Snapshot configuration
    pub snapshot: SnapshotConfig,
    
    /// Performance tuning
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Address to listen on
    pub listen_address: String,
    
    /// Port to listen on
    pub listen_port: u16,
    
    /// Master server address
    pub master_address: String,
    
    /// Master server port
    pub master_port: u16,
    
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    
    /// Keepalive interval in seconds
    pub keepalive_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Data directory for storing WAL and snapshots
    pub data_dir: String,
    
    /// WAL directory (relative to data_dir)
    pub wal_dir: String,
    
    /// Snapshot directory (relative to data_dir)
    pub snapshot_dir: String,
    
    /// Maximum WAL size before rotation (in MB)
    pub max_wal_size: u64,
    
    /// Number of WAL files to keep
    pub wal_retention_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConfig {
    /// Buffer size for replication channel
    pub buffer_size: usize,
    
    /// Batch size for processing changes
    pub batch_size: usize,
    
    /// Flush interval in milliseconds
    pub flush_interval_ms: u64,
    
    /// Enable compression for replication
    pub enable_compression: bool,
    
    /// Retry attempts for failed replication
    pub retry_attempts: u32,
    
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    /// Snapshot interval in seconds
    pub interval: u64,
    
    /// Number of snapshots to retain
    pub retention_count: u32,
    
    /// Enable snapshot compression
    pub enable_compression: bool,
    
    /// Snapshot parallelism
    pub parallel_workers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads
    pub worker_threads: usize,
    
    /// Maximum blocking threads
    pub max_blocking_threads: usize,
    
    /// I/O buffer size
    pub io_buffer_size: usize,
    
    /// Enable direct I/O
    pub enable_direct_io: bool,
}

impl MetaloggerConfig {
    pub fn load(path: &str) -> Result<Self> {
        let config = Config::builder()
            .add_source(File::with_name(path))
            .add_source(config::Environment::with_prefix("MOOSENG_METALOGGER"))
            .build()?;

        config.try_deserialize().map_err(|e| e.into())
    }

    pub fn default() -> Self {
        Self {
            network: NetworkConfig {
                listen_address: "0.0.0.0".to_string(),
                listen_port: 9422,
                master_address: "localhost".to_string(),
                master_port: 9420,
                connection_timeout: 30,
                keepalive_interval: 10,
            },
            storage: StorageConfig {
                data_dir: "/var/lib/mooseng/metalogger".to_string(),
                wal_dir: "wal".to_string(),
                snapshot_dir: "snapshots".to_string(),
                max_wal_size: 1024, // 1GB
                wal_retention_count: 10,
            },
            replication: ReplicationConfig {
                buffer_size: 10000,
                batch_size: 100,
                flush_interval_ms: 100,
                enable_compression: true,
                retry_attempts: 3,
                retry_delay_ms: 1000,
            },
            snapshot: SnapshotConfig {
                interval: 3600, // 1 hour
                retention_count: 24,
                enable_compression: true,
                parallel_workers: 4,
            },
            performance: PerformanceConfig {
                worker_threads: 4,
                max_blocking_threads: 512,
                io_buffer_size: 65536,
                enable_direct_io: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = MetaloggerConfig::default();
        assert_eq!(config.network.listen_port, 9422);
        assert_eq!(config.storage.data_dir, "/var/lib/mooseng/metalogger");
        assert_eq!(config.snapshot.interval, 3600);
    }

    #[test]
    fn test_load_config() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, r#"
[network]
listen_address = "127.0.0.1"
listen_port = 9999

[storage]
data_dir = "/tmp/test"

[replication]
buffer_size = 5000

[snapshot]
interval = 1800

[performance]
worker_threads = 8
"#)?;

        let config = MetaloggerConfig::load(file.path().to_str().unwrap())?;
        assert_eq!(config.network.listen_address, "127.0.0.1");
        assert_eq!(config.network.listen_port, 9999);
        assert_eq!(config.storage.data_dir, "/tmp/test");
        assert_eq!(config.replication.buffer_size, 5000);
        assert_eq!(config.snapshot.interval, 1800);
        assert_eq!(config.performance.worker_threads, 8);

        Ok(())
    }
}