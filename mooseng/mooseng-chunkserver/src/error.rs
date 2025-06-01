use thiserror::Error;
use mooseng_common::types::Status;

/// ChunkServer specific errors
#[derive(Error, Debug)]
pub enum ChunkServerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Chunk not found: {chunk_id}")]
    ChunkNotFound { chunk_id: u64 },
    
    #[error("Chunk already exists: {chunk_id}")]
    ChunkAlreadyExists { chunk_id: u64 },
    
    #[error("Invalid chunk size: expected {expected}, got {actual}")]
    InvalidChunkSize { expected: u64, actual: u64 },
    
    #[error("Checksum mismatch for chunk {chunk_id}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        chunk_id: u64,
        expected: String,
        actual: String,
    },
    
    #[error("Memory mapping error: {0}")]
    MemoryMapping(String),
    
    #[error("Cache error: {0}")]
    Cache(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Insufficient shards for chunk {chunk_id}: {available} available, {required} required")]
    InsufficientShards {
        chunk_id: u64,
        available: usize,
        required: usize,
    },
}

impl From<anyhow::Error> for ChunkServerError {
    fn from(err: anyhow::Error) -> Self {
        ChunkServerError::Internal(err.to_string())
    }
}

impl ChunkServerError {
    /// Convert to MooseFS status code
    pub fn to_status(&self) -> Status {
        match self {
            ChunkServerError::ChunkNotFound { .. } => Status::ENoChunk,
            ChunkServerError::ChunkAlreadyExists { .. } => Status::EExist,
            ChunkServerError::Io(_) => Status::EIO,
            ChunkServerError::InvalidChunkSize { .. } => Status::EInval,
            ChunkServerError::ChecksumMismatch { .. } => Status::EIO,
            ChunkServerError::ResourceExhausted(_) => Status::ENoSpc,
            ChunkServerError::InvalidOperation(_) => Status::EInval,
            _ => Status::EIO,
        }
    }
}

/// Result type for ChunkServer operations
pub type Result<T> = std::result::Result<T, ChunkServerError>;