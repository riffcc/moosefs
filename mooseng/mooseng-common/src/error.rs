use thiserror::Error;
use crate::types::Status;

/// Main error type for MooseNG
#[derive(Error, Debug)]
pub enum MooseNGError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Raft error: {0}")]
    Raft(String),

    #[error("Region error: {0}")]
    Region(String),

    #[error("Erasure coding error: {0}")]
    ErasureCoding(String),

    #[error("Status error: {0}")]
    Status(Status),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Leader lease expired")]
    LeaseExpired,

    #[error("Not leader for region {0}")]
    NotLeader(u8),

    #[error("Region {0} is offline")]
    RegionOffline(u8),

    #[error("Chunk server {0} is offline")]
    ChunkServerOffline(u16),

    #[error("Insufficient replicas: need {needed}, have {available}")]
    InsufficientReplicas { needed: usize, available: usize },

    #[error("Erasure coding failed: {0}")]
    ErasureCodeFailed(String),

    #[error("Cross-region replication error: {0}")]
    CrossRegionReplication(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Component not found: {0}")]
    ComponentNotFound(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<Status> for MooseNGError {
    fn from(status: Status) -> Self {
        MooseNGError::Status(status)
    }
}

impl From<MooseNGError> for Status {
    fn from(error: MooseNGError) -> Self {
        match error {
            MooseNGError::Status(s) => s,
            MooseNGError::Io(_) => Status::EIO,
            MooseNGError::Network(_) => Status::EIO,
            MooseNGError::Protocol(_) => Status::EInval,
            MooseNGError::Storage(_) => Status::EIO,
            MooseNGError::Timeout => Status::EIO,
            MooseNGError::LeaseExpired => Status::EIO,
            MooseNGError::NotLeader(_) => Status::EIO,
            MooseNGError::RegionOffline(_) => Status::EIO,
            MooseNGError::ChunkServerOffline(_) => Status::ENoChunkServers,
            MooseNGError::InsufficientReplicas { .. } => Status::ENoChunkServers,
            MooseNGError::ErasureCodeFailed(_) => Status::EIO,
            MooseNGError::ResourceExhausted(_) => Status::ENoSpc,
            MooseNGError::AuthenticationFailed(_) => Status::EAcces,
            _ => Status::EIO,
        }
    }
}

pub type Result<T> = std::result::Result<T, MooseNGError>;