//! Core Types for MooseFS + MooseNG Integration
//! 
//! This module defines the fundamental types used throughout the integration layer.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Chunk identifier type
pub type ChunkId = u64;

/// Storage class identifier type  
pub type StorageClassId = u32;

/// Node identifier type
pub type NodeId = u32;

/// Session identifier type
pub type SessionId = u64;

/// File descriptor type
pub type FileDescriptor = u32;

/// Version type for metadata versioning
pub type Version = u64;

/// Timestamp type (microseconds since Unix epoch)
pub type Timestamp = u64;

/// Size type for file and chunk sizes
pub type Size = u64;

/// Offset type for file operations
pub type Offset = u64;

/// I/O operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOResult<T> {
    /// Operation result
    pub result: Result<T, IOError>,
    /// Operation duration in milliseconds
    pub duration_ms: u64,
    /// Bytes transferred
    pub bytes_transferred: u64,
    /// Timestamp of operation
    pub timestamp: SystemTime,
}

/// I/O error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IOError {
    /// File not found
    FileNotFound,
    /// Permission denied
    PermissionDenied,
    /// I/O error
    IOError(String),
    /// Network error
    NetworkError(String),
    /// Timeout error
    Timeout,
    /// Storage full
    StorageFull,
    /// Corrupted data
    DataCorruption,
    /// Service unavailable
    ServiceUnavailable,
}

/// File metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File path
    pub path: String,
    /// File size in bytes
    pub size: Size,
    /// Creation timestamp
    pub created_at: SystemTime,
    /// Last modification timestamp
    pub modified_at: SystemTime,
    /// Last access timestamp
    pub accessed_at: SystemTime,
    /// File permissions
    pub permissions: FilePermissions,
    /// Owner information
    pub owner: OwnerInfo,
    /// File type
    pub file_type: FileType,
    /// Extended attributes
    pub extended_attributes: std::collections::HashMap<String, String>,
}

/// File permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePermissions {
    /// Owner permissions
    pub owner: PermissionSet,
    /// Group permissions
    pub group: PermissionSet,
    /// Other permissions
    pub other: PermissionSet,
}

/// Permission set (read, write, execute)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSet {
    /// Read permission
    pub read: bool,
    /// Write permission
    pub write: bool,
    /// Execute permission
    pub execute: bool,
}

/// Owner information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerInfo {
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Username (optional)
    pub username: Option<String>,
    /// Group name (optional)
    pub groupname: Option<String>,
}

/// File type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    /// Regular file
    Regular,
    /// Directory
    Directory,
    /// Symbolic link
    SymbolicLink,
    /// Character device
    CharacterDevice,
    /// Block device
    BlockDevice,
    /// FIFO (named pipe)
    Fifo,
    /// Socket
    Socket,
}

/// Chunk metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Chunk ID
    pub chunk_id: ChunkId,
    /// Chunk version
    pub version: Version,
    /// Chunk size in bytes
    pub size: Size,
    /// Checksum for integrity verification
    pub checksum: String,
    /// Storage locations (node IDs)
    pub locations: Vec<NodeId>,
    /// Creation timestamp
    pub created_at: SystemTime,
    /// Last verification timestamp
    pub last_verified: SystemTime,
    /// Replication level
    pub replication_level: u8,
    /// Storage class
    pub storage_class: StorageClassId,
}

/// Node metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Node ID
    pub node_id: NodeId,
    /// Node address
    pub address: String,
    /// Node port
    pub port: u16,
    /// Node type
    pub node_type: NodeType,
    /// Node status
    pub status: NodeStatus,
    /// Available capacity
    pub available_capacity: Size,
    /// Total capacity
    pub total_capacity: Size,
    /// Last heartbeat
    pub last_heartbeat: SystemTime,
    /// Node version
    pub version: String,
}

/// Node type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    /// Master server
    Master,
    /// Chunk server
    ChunkServer,
    /// Metalogger
    MetaLogger,
    /// Client
    Client,
    /// CGI server
    CGIServer,
}

/// Node status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is online and healthy
    Online,
    /// Node is offline
    Offline,
    /// Node is in maintenance mode
    Maintenance,
    /// Node is degraded (some issues but still functional)
    Degraded,
    /// Node status is unknown
    Unknown,
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session ID
    pub session_id: SessionId,
    /// Client address
    pub client_address: String,
    /// Session start time
    pub started_at: SystemTime,
    /// Last activity timestamp
    pub last_activity: SystemTime,
    /// Session type
    pub session_type: SessionType,
    /// Active operations count
    pub active_operations: u32,
    /// Total bytes transferred
    pub bytes_transferred: Size,
}

/// Session type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionType {
    /// FUSE mount session
    FUSE,
    /// Direct API session
    API,
    /// Administrative session
    Admin,
    /// Replication session
    Replication,
}

/// Operation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    /// Total operations count
    pub total_operations: u64,
    /// Successful operations count
    pub successful_operations: u64,
    /// Failed operations count
    pub failed_operations: u64,
    /// Average operation duration (milliseconds)
    pub avg_duration_ms: f64,
    /// Total bytes processed
    pub total_bytes: Size,
    /// Operations per second
    pub ops_per_second: f64,
    /// Last reset timestamp
    pub last_reset: SystemTime,
}

/// Configuration for integration components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentConfig {
    /// Component name
    pub name: String,
    /// Component version
    pub version: String,
    /// Configuration parameters
    pub parameters: std::collections::HashMap<String, ConfigValue>,
    /// Last updated timestamp
    pub last_updated: SystemTime,
}

/// Configuration value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigValue {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Array of values
    Array(Vec<ConfigValue>),
    /// Object (nested configuration)
    Object(std::collections::HashMap<String, ConfigValue>),
}

/// Health check result specific to integration components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealthResult {
    /// Component name
    pub component: String,
    /// Health status
    pub status: HealthStatus,
    /// Status message
    pub message: String,
    /// Metrics collected during health check
    pub metrics: std::collections::HashMap<String, f64>,
    /// Timestamp of health check
    pub timestamp: SystemTime,
    /// Health check duration
    pub duration_ms: u64,
    /// Recommendations for improving health
    pub recommendations: Vec<String>,
}

/// Health status levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Component is healthy
    Healthy,
    /// Component has warnings but is functional
    Warning,
    /// Component is critical (may fail soon)
    Critical,
    /// Component is degraded (reduced functionality)
    Degraded,
    /// Component status is unknown
    Unknown,
}

/// Storage tier information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierInfo {
    /// Tier identifier
    pub tier_id: String,
    /// Tier type
    pub tier_type: TierType,
    /// Total capacity
    pub total_capacity: Size,
    /// Used capacity
    pub used_capacity: Size,
    /// Available capacity
    pub available_capacity: Size,
    /// Performance characteristics
    pub performance: TierPerformance,
    /// Cost information
    pub cost_info: TierCostInfo,
}

/// Storage tier types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TierType {
    /// Hot tier (high-performance SSD)
    Hot,
    /// Warm tier (standard SSD or high-performance HDD)
    Warm,
    /// Cold tier (object storage)
    Cold,
    /// Archive tier (long-term storage)
    Archive,
}

/// Tier performance characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierPerformance {
    /// Average read latency (milliseconds)
    pub avg_read_latency_ms: f64,
    /// Average write latency (milliseconds)
    pub avg_write_latency_ms: f64,
    /// IOPS capability
    pub max_iops: u32,
    /// Bandwidth (MB/s)
    pub max_bandwidth_mbps: u32,
}

/// Tier cost information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierCostInfo {
    /// Cost per GB per month
    pub cost_per_gb_month: f64,
    /// Cost per operation
    pub cost_per_operation: f64,
    /// Data transfer cost per GB
    pub transfer_cost_per_gb: f64,
}

/// Migration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRequest {
    /// Request ID
    pub request_id: String,
    /// Source chunk ID
    pub chunk_id: ChunkId,
    /// Source tier
    pub source_tier: TierType,
    /// Target tier
    pub target_tier: TierType,
    /// Migration priority
    pub priority: MigrationPriority,
    /// Reason for migration
    pub reason: String,
    /// Requested timestamp
    pub requested_at: SystemTime,
}

/// Migration priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationPriority {
    /// Low priority (background migration)
    Low,
    /// Normal priority
    Normal,
    /// High priority
    High,
    /// Urgent (immediate migration required)
    Urgent,
}

/// Migration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    /// Request ID
    pub request_id: String,
    /// Current status
    pub status: MigrationState,
    /// Progress percentage (0-100)
    pub progress_percent: f64,
    /// Bytes transferred
    pub bytes_transferred: Size,
    /// Total bytes to transfer
    pub total_bytes: Size,
    /// Started timestamp
    pub started_at: Option<SystemTime>,
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    /// Error message (if failed)
    pub error_message: Option<String>,
}

/// Migration states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationState {
    /// Migration is queued
    Queued,
    /// Migration is in progress
    InProgress,
    /// Migration completed successfully
    Completed,
    /// Migration failed
    Failed,
    /// Migration was cancelled
    Cancelled,
}

/// Utility functions for timestamp conversion
pub fn now_micros() -> Timestamp {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as Timestamp
}

/// Convert microseconds to SystemTime
pub fn micros_to_system_time(micros: Timestamp) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_micros(micros)
}

/// Convert SystemTime to microseconds
pub fn system_time_to_micros(time: SystemTime) -> Timestamp {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as Timestamp
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timestamp_conversion() {
        let now = SystemTime::now();
        let micros = system_time_to_micros(now);
        let converted_back = micros_to_system_time(micros);
        
        // Allow for small differences due to precision
        let diff = now.duration_since(converted_back)
            .or_else(|_| converted_back.duration_since(now))
            .unwrap();
        
        assert!(diff.as_millis() < 1);
    }
    
    #[test]
    fn test_health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Warning);
    }
    
    #[test]
    fn test_tier_type_ordering() {
        assert!(TierType::Hot != TierType::Cold);
        assert_eq!(TierType::Hot, TierType::Hot);
    }
    
    #[test]
    fn test_io_result_creation() {
        let result: IOResult<String> = IOResult {
            result: Ok("success".to_string()),
            duration_ms: 100,
            bytes_transferred: 1024,
            timestamp: SystemTime::now(),
        };
        
        assert!(result.result.is_ok());
        assert_eq!(result.duration_ms, 100);
        assert_eq!(result.bytes_transferred, 1024);
    }
}