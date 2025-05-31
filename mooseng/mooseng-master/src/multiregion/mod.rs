// Multiregion support module for MooseNG
// Implements cross-region replication, consistency management, and distributed consensus

pub mod raft_multiregion;
pub mod hybrid_clock;
// pub mod region_manager;
pub mod replication;
pub mod crdt;
pub mod consistency;
pub mod consistency_manager;
pub mod placement;
pub mod failover;
pub mod failure_handler;
pub mod inter_region_comm;
pub mod communication_optimizer;
pub mod monitoring;

pub use raft_multiregion::{MultiRegionRaft, RegionStatus, PeerRegionStatus};
pub use hybrid_clock::{HybridLogicalClock, HLCTimestamp};
pub use consistency::ConsistencyLevel;
pub use replication::{CrossRegionReplicator, ReplicationOperation, ReplicationAck, RegionReplicationStatus};
pub use placement::{
    RegionPlacementManager, PlacementPolicy, PlacementDecision, PlacementRules,
    AccessPattern, DataCriticality, RegionInfo, RegionCapacity, RegionLoad,
    ComplianceInfo, RegionStatistics
};
pub use crdt::{
    CRDT, GCounter, PNCounter, GSet, TwoPSet, LWWRegister, ORSet, LWWMap,
    FileMetadataCRDT, CRDTManager, CRDTStatistics, NodeId
};
pub use consistency_manager::{
    ConsistencyManager, ConsistencyManagerConfig, ReadRequest, WriteRequest,
    OperationResponse, SessionState, ConsistencyViolation, NetworkPartition,
    AdaptiveStrategy, ConsistencyMetrics, OperationType, ViolationType, PartitionType
};
pub use failover::{
    FailureManager, FailureType, FailureEvent, FailureSeverity, FailoverStrategy,
    CircuitBreaker, CircuitBreakerState, CircuitBreakerConfig, PartitionType as FailoverPartitionType
};
pub use failure_handler::{
    FailureHandler, FailureInstance, RecoveryStrategy, FailureState,
    ImpactAssessment, RecoveryProgress, OutageSeverity, CorruptionType
};
pub use inter_region_comm::{
    InterRegionCommManager, InterRegionMessage, MessageType, MessagePriority,
    NetworkProtocol, CompressionType, EncryptionType, InterRegionMetrics
};
pub use communication_optimizer::{
    CommunicationOptimizer, CommunicationConfig, BatchConfig, ConnectionPoolConfig,
    PriorityConfig, NetworkOptimization, NetworkStatistics
};
pub use monitoring::{
    MultiregionMonitor, MultiregionSystemMetrics, TraceEvent, TraceFilter,
    PerformanceProfile, SystemHealthStatus, AlertRule, DebugSession, HealthLevel
};

use std::time::Duration;

/// Configuration for multiregion support
#[derive(Debug, Clone)]
pub struct MultiregionConfig {
    /// This region's unique identifier
    pub region_id: u32,
    
    /// This region's name (e.g., "us-east-1")
    pub region_name: String,
    
    /// List of peer regions for replication
    pub peer_regions: Vec<RegionPeer>,
    
    /// Maximum allowed replication lag
    pub max_replication_lag: Duration,
    
    /// RPO (Recovery Point Objective) in seconds
    pub rpo_seconds: u32,
    
    /// RTO (Recovery Time Objective) in seconds
    pub rto_seconds: u32,
    
    /// Enable region-aware erasure coding
    pub region_aware_ec: bool,
    
    /// Default consistency level for operations
    pub default_consistency: ConsistencyLevel,
    
    /// Enable leader leases for fast reads
    pub enable_leader_leases: bool,
    
    /// Leader lease duration
    pub leader_lease_duration: Duration,
}

/// Information about a peer region
#[derive(Debug, Clone)]
pub struct RegionPeer {
    pub region_id: u32,
    pub region_name: String,
    pub endpoints: Vec<String>,
    pub priority: u8,  // For failover ordering
    pub latency_ms: u32,  // Expected latency to this region
}

impl Default for MultiregionConfig {
    fn default() -> Self {
        Self {
            region_id: 1,
            region_name: "primary".to_string(),
            peer_regions: Vec::new(),
            max_replication_lag: Duration::from_secs(30),
            rpo_seconds: 60,
            rto_seconds: 300,
            region_aware_ec: true,
            default_consistency: ConsistencyLevel::BoundedStaleness(Duration::from_secs(5)),
            enable_leader_leases: true,
            leader_lease_duration: Duration::from_secs(10),
        }
    }
}