use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// MooseNG version information
pub const MOOSENG_VERSION: &str = "0.1.0";
pub const MOOSENG_SIGNATURE: &str = "MNG";

/// File system constants (compatible with MooseFS)
pub const CHUNK_SIZE: u64 = 0x04000000; // 64MB
pub const CHUNK_MASK: u64 = 0x03FFFFFF;
pub const CHUNK_BITS: u32 = 26;
pub const BLOCK_SIZE: u64 = 0x10000; // 64KB
pub const BLOCK_MASK: u64 = 0x0FFFF;
pub const BLOCK_BITS: u32 = 16;
pub const BLOCKS_IN_CHUNK: u32 = 0x400;
pub const MFS_ROOT_ID: u64 = 1;
pub const MFS_NAME_MAX: usize = 255;
pub const MFS_PATH_MAX: usize = 1024;
pub const MAX_FILE_SIZE: u64 = CHUNK_SIZE << 31;

/// Erasure coding constants
pub const MAX_EC_PARTS: usize = 8;
pub const EC_4_PLUS_N_DATA: usize = 4;
pub const EC_8_PLUS_N_DATA: usize = 8;

/// Region and replication constants
pub const MAX_REGIONS: usize = 16;
pub const DEFAULT_LEADER_LEASE_MS: u64 = 2000;
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 500;

/// Unique identifier for inodes
pub type InodeId = u64;

/// Unique identifier for chunks
pub type ChunkId = u64;

/// Unique identifier for chunk servers
pub type ChunkServerId = u16;

/// Unique identifier for sessions
pub type SessionId = u64;

/// Version number for chunks
pub type ChunkVersion = u32;

/// Unique identifier for regions
pub type RegionId = u8;

/// Unique identifier for storage classes
pub type StorageClassId = u8;

/// Timestamps in microseconds since UNIX epoch
pub type Timestamp = u64;

/// Get current timestamp in microseconds
pub fn now_micros() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

/// Status codes compatible with MooseFS protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Status {
    Ok = 0,
    EPerm = 1,
    ENotDir = 2,
    ENoEnt = 3,
    EAcces = 4,
    EExist = 5,
    EInval = 6,
    ENotEmpty = 7,
    EDQuot = 8,
    EIO = 9,
    ENoSpc = 10,
    EChunkLost = 11,
    EOutOfMem = 12,
    EIndexTooBig = 13,
    ELocked = 14,
    ENoChunk = 15,
    EBusy = 16,
    ENoChunkServers = 17,
    ENoSpace = 18,
    EChunkTooBig = 19,
    // ... additional status codes
}

impl Status {
    pub fn is_ok(&self) -> bool {
        matches!(self, Status::Ok)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::EPerm => "Operation not permitted",
            Status::ENotDir => "Not a directory",
            Status::ENoEnt => "No such file or directory",
            Status::EAcces => "Permission denied",
            Status::EExist => "File exists",
            Status::EInval => "Invalid argument",
            Status::ENotEmpty => "Directory not empty",
            Status::EDQuot => "Quota exceeded",
            Status::EIO => "I/O error",
            Status::ENoSpc => "No space left on device",
            Status::EChunkLost => "Chunk lost",
            Status::EOutOfMem => "Out of memory",
            Status::EIndexTooBig => "Index too big",
            Status::ELocked => "Locked",
            Status::ENoChunk => "No such chunk",
            Status::EBusy => "Device or resource busy",
            Status::ENoChunkServers => "No chunk servers",
            Status::ENoSpace => "No space",
            Status::EChunkTooBig => "Chunk too big",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// File types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum FileType {
    File = 1,
    Directory = 2,
    Symlink = 3,
    Fifo = 4,
    BlockDev = 5,
    CharDev = 6,
    Socket = 7,
    Trash = 8,
    Sustained = 9,
}

/// Storage class for files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageClass {
    pub id: u8,
    pub admin_only: bool,
    pub min_replicas: u8,
    pub ec_data_parts: u8,
    pub ec_parity_parts: u8,
    pub region_spread: u8,
}

impl Default for StorageClass {
    fn default() -> Self {
        Self {
            id: 1,
            admin_only: false,
            min_replicas: 2,
            ec_data_parts: 0,
            ec_parity_parts: 0,
            region_spread: 1,
        }
    }
}

/// File attributes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAttr {
    pub inode: InodeId,
    pub file_type: FileType,
    pub mode: u16,
    pub uid: u32,
    pub gid: u32,
    pub atime: Timestamp,
    pub mtime: Timestamp,
    pub ctime: Timestamp,
    pub nlink: u32,
    pub length: u64,
    pub storage_class: StorageClass,
}

/// Chunk location information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkLocation {
    pub chunk_server_id: ChunkServerId,
    pub ip: std::net::Ipv4Addr,
    pub port: u16,
    pub region_id: RegionId,
    pub rack_id: u16,
}

/// Chunk information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub chunk_id: ChunkId,
    pub version: ChunkVersion,
    pub locations: Vec<ChunkLocation>,
    pub ec_parts: Option<ErasureCodeInfo>,
}

/// Erasure coding information for a chunk
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureCodeInfo {
    pub data_parts: u8,
    pub parity_parts: u8,
    pub part_size: u32,
    pub stripe_id: u64,
    pub part_locations: Vec<(u8, ChunkLocation)>, // (part_index, location)
}

/// Region configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionConfig {
    pub id: RegionId,
    pub name: String,
    pub datacenter: String,
    pub geo_location: (f64, f64), // (latitude, longitude)
    pub priority: u8,
    pub bandwidth_mbps: u32,
    pub latency_ms: u32,
}

/// Hybrid logical clock for distributed ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridLogicalClock {
    pub physical_time: u64,
    pub logical_time: u32,
    pub node_id: u16,
}

impl HybridLogicalClock {
    pub fn new(node_id: u16) -> Self {
        Self {
            physical_time: now_micros(),
            logical_time: 0,
            node_id,
        }
    }

    pub fn update(&mut self, received: &HybridLogicalClock) {
        let now = now_micros();
        if received.physical_time > self.physical_time && received.physical_time > now {
            self.physical_time = received.physical_time;
            self.logical_time = received.logical_time + 1;
        } else if now > self.physical_time && now > received.physical_time {
            self.physical_time = now;
            self.logical_time = 0;
        } else {
            self.logical_time = std::cmp::max(self.logical_time, received.logical_time) + 1;
        }
    }

    pub fn tick(&mut self) {
        let now = now_micros();
        if now > self.physical_time {
            self.physical_time = now;
            self.logical_time = 0;
        } else {
            self.logical_time += 1;
        }
    }
}

impl PartialOrd for HybridLogicalClock {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HybridLogicalClock {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.physical_time
            .cmp(&other.physical_time)
            .then(self.logical_time.cmp(&other.logical_time))
            .then(self.node_id.cmp(&other.node_id))
    }
}

/// Raft leader lease information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderLease {
    pub leader_id: u16,
    pub term: u64,
    pub lease_start: Timestamp,
    pub lease_duration_ms: u64,
    pub region_id: RegionId,
}

impl LeaderLease {
    pub fn is_valid(&self) -> bool {
        let now = now_micros();
        let lease_end = self.lease_start + (self.lease_duration_ms * 1000);
        now < lease_end
    }

    pub fn remaining_ms(&self) -> u64 {
        let now = now_micros();
        let lease_end = self.lease_start + (self.lease_duration_ms * 1000);
        if now < lease_end {
            (lease_end - now) / 1000
        } else {
            0
        }
    }
}

/// File system node (inode) structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsNode {
    pub inode: InodeId,
    pub parent: Option<InodeId>,
    pub ctime: Timestamp,
    pub mtime: Timestamp,
    pub atime: Timestamp,
    pub uid: u32,
    pub gid: u32,
    pub mode: u16,
    pub flags: u8,
    pub winattr: u8,
    pub storage_class_id: u8,
    pub trash_retention: u32,
    pub node_type: FsNodeType,
}

/// Type-specific data for filesystem nodes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsNodeType {
    File {
        length: u64,
        chunk_ids: Vec<ChunkId>,
        session_id: Option<SessionId>,
    },
    Directory {
        children: Vec<FsEdge>,
        stats: DirStats,
        quota: Option<Quota>,
    },
    Symlink {
        target: String,
    },
    Device {
        rdev: u32,
    },
}

/// Directory edge (parent-child relationship)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsEdge {
    pub edge_id: u64,
    pub parent_inode: InodeId,
    pub child_inode: InodeId,
    pub name: String,
}

/// Directory statistics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DirStats {
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_size: u64,
    pub total_chunks: u64,
}

/// Quota information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quota {
    pub soft_inodes: Option<u64>,
    pub hard_inodes: Option<u64>,
    pub soft_size: Option<u64>,
    pub hard_size: Option<u64>,
    pub soft_chunks: Option<u64>,
    pub hard_chunks: Option<u64>,
    pub current_inodes: u64,
    pub current_size: u64,
    pub current_chunks: u64,
}

/// Extended attributes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendedAttr {
    pub name: String,
    pub value: Vec<u8>,
}

/// Access Control List entry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclEntry {
    pub qualifier: u32,
    pub permissions: u16,
    pub entry_type: AclType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AclType {
    UserObj = 0,
    User = 1,
    GroupObj = 2,
    Group = 3,
    Mask = 4,
    Other = 5,
}

/// Chunk metadata stored by master
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_id: ChunkId,
    pub version: ChunkVersion,
    pub locked_to: Option<SessionId>,
    pub archive_flag: bool,
    pub storage_class_id: u8,
    pub locations: Vec<ChunkLocation>,
    pub ec_info: Option<ErasureCodeInfo>,
    pub last_modified: Timestamp,
}

/// Session information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub client_ip: std::net::IpAddr,
    pub mount_point: String,
    pub version: u32,
    pub open_files: Vec<InodeId>,
    pub locked_chunks: Vec<ChunkId>,
    pub last_activity: Timestamp,
    pub metadata_cache_size: u64,
}

/// Storage class definition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageClassDef {
    pub id: u8,
    pub name: String,
    pub admin_only: bool,
    pub create_labels: Vec<String>,
    pub keep_labels: Vec<String>,
    pub archive_labels: Vec<String>,
    pub trash_labels: Vec<String>,
    pub create_replication: ReplicationPolicy,
    pub keep_replication: ReplicationPolicy,
    pub archive_replication: ReplicationPolicy,
    pub trash_replication: ReplicationPolicy,
}

/// Replication policy for storage classes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicationPolicy {
    Copies { count: u8 },
    ErasureCoding { data: u8, parity: u8 },
    XRegion { min_regions: u8, min_copies: u8 },
}

/// System configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemConfig {
    pub metadata_checksum_interval_ms: u64,
    pub chunk_test_interval_ms: u64,
    pub session_timeout_ms: u64,
    pub write_lease_timeout_ms: u64,
    pub max_file_size: u64,
    pub trash_retention_days: u32,
    pub snapshot_retention_days: u32,
    pub regions: Vec<RegionConfig>,
    pub storage_classes: Vec<StorageClassDef>,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            metadata_checksum_interval_ms: 3600000, // 1 hour
            chunk_test_interval_ms: 300000,         // 5 minutes
            session_timeout_ms: 60000,              // 1 minute
            write_lease_timeout_ms: 30000,          // 30 seconds
            max_file_size: MAX_FILE_SIZE,
            trash_retention_days: 7,
            snapshot_retention_days: 30,
            regions: vec![],
            storage_classes: vec![
                StorageClassDef {
                    id: 1,
                    name: "default".to_string(),
                    admin_only: false,
                    create_labels: vec![],
                    keep_labels: vec![],
                    archive_labels: vec![],
                    trash_labels: vec![],
                    create_replication: ReplicationPolicy::Copies { count: 2 },
                    keep_replication: ReplicationPolicy::Copies { count: 2 },
                    archive_replication: ReplicationPolicy::Copies { count: 1 },
                    trash_replication: ReplicationPolicy::Copies { count: 1 },
                },
            ],
        }
    }
}

/// Result type for MooseNG operations
pub type Result<T> = std::result::Result<T, Status>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_logical_clock() {
        let mut clock1 = HybridLogicalClock::new(1);
        let mut clock2 = HybridLogicalClock::new(2);

        clock1.tick();
        assert!(clock1.logical_time == 0 || clock1.logical_time == 1);

        let old_clock1 = clock1;
        clock2.update(&clock1);
        assert!(clock2 > old_clock1);
    }

    #[test]
    fn test_leader_lease() {
        let lease = LeaderLease {
            leader_id: 1,
            term: 1,
            lease_start: now_micros(),
            lease_duration_ms: 2000,
            region_id: 0,
        };

        assert!(lease.is_valid());
        assert!(lease.remaining_ms() > 1900);
        assert!(lease.remaining_ms() <= 2000);
    }

    #[test]
    fn test_fs_node_serialization() {
        let node = FsNode {
            inode: 42,
            parent: Some(1),
            ctime: now_micros(),
            mtime: now_micros(),
            atime: now_micros(),
            uid: 1000,
            gid: 1000,
            mode: 0o644,
            flags: 0,
            winattr: 0,
            storage_class_id: 1,
            trash_retention: 7,
            node_type: FsNodeType::File {
                length: 1024,
                chunk_ids: vec![100, 101],
                session_id: None,
            },
        };

        let serialized = serde_json::to_string(&node).unwrap();
        let deserialized: FsNode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(node, deserialized);
    }

    #[test]
    fn test_storage_class_def() {
        let sclass = StorageClassDef {
            id: 2,
            name: "ec-4-2".to_string(),
            admin_only: false,
            create_labels: vec!["fast".to_string()],
            keep_labels: vec!["reliable".to_string()],
            archive_labels: vec!["cold".to_string()],
            trash_labels: vec!["any".to_string()],
            create_replication: ReplicationPolicy::ErasureCoding { data: 4, parity: 2 },
            keep_replication: ReplicationPolicy::ErasureCoding { data: 4, parity: 2 },
            archive_replication: ReplicationPolicy::Copies { count: 1 },
            trash_replication: ReplicationPolicy::Copies { count: 1 },
        };

        assert_eq!(sclass.name, "ec-4-2");
        match &sclass.create_replication {
            ReplicationPolicy::ErasureCoding { data, parity } => {
                assert_eq!(*data, 4);
                assert_eq!(*parity, 2);
            }
            _ => panic!("Expected erasure coding policy"),
        }
    }

    #[test]
    fn test_quota() {
        let quota = Quota {
            soft_inodes: Some(1000),
            hard_inodes: Some(2000),
            soft_size: Some(1024 * 1024 * 1024), // 1GB
            hard_size: Some(2 * 1024 * 1024 * 1024), // 2GB
            soft_chunks: None,
            hard_chunks: None,
            current_inodes: 100,
            current_size: 100 * 1024 * 1024, // 100MB
            current_chunks: 10,
        };

        assert!(quota.current_inodes < quota.soft_inodes.unwrap());
        assert!(quota.current_size < quota.soft_size.unwrap());
    }

    #[test]
    fn test_chunk_metadata() {
        let chunk = ChunkMetadata {
            chunk_id: 12345,
            version: 1,
            locked_to: Some(100),
            archive_flag: false,
            storage_class_id: 1,
            locations: vec![
                ChunkLocation {
                    chunk_server_id: 1,
                    ip: std::net::Ipv4Addr::new(10, 0, 0, 1),
                    port: 9422,
                    region_id: 0,
                    rack_id: 1,
                },
                ChunkLocation {
                    chunk_server_id: 2,
                    ip: std::net::Ipv4Addr::new(10, 0, 0, 2),
                    port: 9422,
                    region_id: 0,
                    rack_id: 2,
                },
            ],
            ec_info: None,
            last_modified: now_micros(),
        };

        assert_eq!(chunk.locations.len(), 2);
        assert!(chunk.locked_to.is_some());
    }
}