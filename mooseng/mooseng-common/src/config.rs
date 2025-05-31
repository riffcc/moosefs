use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use crate::types::{RegionId, ChunkServerId};

/// Master server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConfig {
    pub bind_address: IpAddr,
    pub client_port: u16,
    pub cs_port: u16,
    pub metalogger_port: u16,
    pub admin_port: u16,
    pub data_dir: PathBuf,
    pub metadata_file: PathBuf,
    pub changelog_dir: PathBuf,
    
    // Node identity
    pub node_id: String,
    
    // Raft configuration
    pub raft_port: u16,
    pub raft_heartbeat_ms: u64,
    pub raft_election_timeout_ms: u64,
    pub raft_snapshot_interval: u64,
    
    // Region configuration
    pub region_id: RegionId,
    pub region_name: String,
    pub leader_lease_ms: u64,
    pub leader_lease_duration_ms: u64,
    pub cross_region_ports: Vec<u16>,
    
    // High availability
    pub ha_enabled: bool,
    pub ha_peers: Vec<String>,
    pub ha_auto_failover: bool,
    
    // Performance tuning
    pub metadata_cache_size: usize,
    pub max_clients: usize,
    pub max_chunk_servers: usize,
    pub worker_threads: usize,
    
    // Session and timing
    pub session_timeout_ms: u64,
    pub metadata_checksum_interval_ms: u64,
    pub chunk_test_interval_ms: u64,
}

impl Default for MasterConfig {
    fn default() -> Self {
        Self {
            bind_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            client_port: 9421,
            cs_port: 9422,
            metalogger_port: 9423,
            admin_port: 9424,
            data_dir: PathBuf::from("/var/lib/mooseng/master"),
            metadata_file: PathBuf::from("/var/lib/mooseng/master/metadata.mng"),
            changelog_dir: PathBuf::from("/var/lib/mooseng/master/changelog"),
            node_id: String::from("master-0"),
            raft_port: 9425,
            raft_heartbeat_ms: 150,
            raft_election_timeout_ms: 1000,
            raft_snapshot_interval: 10000,
            region_id: 0,
            region_name: String::from("default"),
            leader_lease_ms: 2000,
            leader_lease_duration_ms: 2000,
            cross_region_ports: vec![9426, 9427, 9428],
            ha_enabled: true,
            ha_peers: vec![],
            ha_auto_failover: true,
            metadata_cache_size: 128 * 1024 * 1024, // 128MB
            max_clients: 10_000,
            max_chunk_servers: 1_000,
            worker_threads: 0, // 0 means use number of CPU cores
            session_timeout_ms: 3600000, // 1 hour
            metadata_checksum_interval_ms: 300000, // 5 minutes
            chunk_test_interval_ms: 60000, // 1 minute
        }
    }
}

/// Chunk server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkServerConfig {
    pub bind_address: IpAddr,
    pub master_address: String,
    pub master_port: u16,
    pub cs_port: u16,
    pub cs_id: Option<ChunkServerId>,
    pub data_dirs: Vec<PathBuf>,
    
    // Region and rack awareness
    pub region_id: RegionId,
    pub rack_id: u16,
    pub datacenter: String,
    
    // Storage configuration
    pub hdd_test_freq: u64,
    pub hdd_min_free_space: u64,
    pub hdd_leave_free_space: u64,
    
    // Erasure coding
    pub ec_enabled: bool,
    pub ec_cache_size: usize,
    pub ec_worker_threads: usize,
    
    // Performance tuning
    pub io_threads: usize,
    pub network_threads: usize,
    pub replication_threads: usize,
    pub max_read_behind_mb: usize,
    pub max_write_behind_mb: usize,
    
    // Connection settings
    pub tcp_timeout: u64,
    pub max_connections: usize,
}

impl Default for ChunkServerConfig {
    fn default() -> Self {
        Self {
            bind_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            master_address: String::from("127.0.0.1"),
            master_port: 9422,
            cs_port: 9522,
            cs_id: None,
            data_dirs: vec![PathBuf::from("/var/lib/mooseng/chunkserver")],
            region_id: 0,
            rack_id: 0,
            datacenter: String::from("default"),
            hdd_test_freq: 3600,
            hdd_min_free_space: 256 * 1024 * 1024, // 256MB
            hdd_leave_free_space: 4 * 1024 * 1024 * 1024, // 4GB
            ec_enabled: true,
            ec_cache_size: 64,
            ec_worker_threads: 4,
            io_threads: 0, // 0 means auto
            network_threads: 0,
            replication_threads: 2,
            max_read_behind_mb: 128,
            max_write_behind_mb: 256,
            tcp_timeout: 60,
            max_connections: 1000,
        }
    }
}

/// Client mount configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub master_address: String,
    pub master_port: u16,
    pub mount_point: PathBuf,
    pub subfolder: String,
    
    // Cache configuration
    pub cache_size: usize,
    pub cache_per_inode: usize,
    pub read_cache_ahead: usize,
    pub write_cache_behind: usize,
    
    // Region preference
    pub preferred_region: Option<RegionId>,
    pub region_failover: bool,
    
    // Performance
    pub io_threads: usize,
    pub prefetch_xor_chunks: bool,
    pub no_std_mounts: bool,
    pub no_bsd_locks: bool,
    pub no_posix_locks: bool,
    
    // Timeouts
    pub io_timeout: u64,
    pub metadata_timeout: u64,
    pub sync_timeout: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            master_address: String::from("127.0.0.1"),
            master_port: 9421,
            mount_point: PathBuf::from("/mnt/mooseng"),
            subfolder: String::from("/"),
            cache_size: 512 * 1024 * 1024, // 512MB
            cache_per_inode: 16 * 1024 * 1024, // 16MB
            read_cache_ahead: 1024 * 1024, // 1MB
            write_cache_behind: 4 * 1024 * 1024, // 4MB
            preferred_region: None,
            region_failover: true,
            io_threads: 4,
            prefetch_xor_chunks: false,
            no_std_mounts: false,
            no_bsd_locks: false,
            no_posix_locks: false,
            io_timeout: 30,
            metadata_timeout: 5,
            sync_timeout: 60,
        }
    }
}

/// Metalogger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaloggerConfig {
    pub bind_address: IpAddr,
    pub master_address: String,
    pub master_port: u16,
    pub data_dir: PathBuf,
    pub backup_logs: u32,
    pub backup_period: u64,
    pub region_id: RegionId,
}

impl Default for MetaloggerConfig {
    fn default() -> Self {
        Self {
            bind_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            master_address: String::from("127.0.0.1"),
            master_port: 9423,
            data_dir: PathBuf::from("/var/lib/mooseng/metalogger"),
            backup_logs: 50,
            backup_period: 3600,
            region_id: 0,
        }
    }
}

/// Global system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub regions: Vec<RegionInfo>,
    pub default_storage_class: u8,
    pub trash_retention_days: u32,
    pub session_timeout: u64,
    pub chunk_replication_delay: u64,
    pub ec_min_chunk_size: u64,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo {
    pub id: RegionId,
    pub name: String,
    pub masters: Vec<String>,
    pub priority: u8,
    pub bandwidth_mbps: u32,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub auth_enabled: bool,
    pub tls_enabled: bool,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub ca_path: PathBuf,
    pub token_expiry_seconds: u64,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            regions: vec![RegionInfo {
                id: 0,
                name: String::from("default"),
                masters: vec![String::from("127.0.0.1:9421")],
                priority: 100,
                bandwidth_mbps: 10000,
                latency_ms: 1,
            }],
            default_storage_class: 1,
            trash_retention_days: 7,
            session_timeout: 3600,
            chunk_replication_delay: 300,
            ec_min_chunk_size: 64 * 1024 * 1024, // 64MB
            security: SecurityConfig {
                auth_enabled: false,
                tls_enabled: false,
                cert_path: PathBuf::from("/etc/mooseng/cert.pem"),
                key_path: PathBuf::from("/etc/mooseng/key.pem"),
                ca_path: PathBuf::from("/etc/mooseng/ca.pem"),
                token_expiry_seconds: 86400,
            },
        }
    }
}