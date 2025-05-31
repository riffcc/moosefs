use thiserror::Error;
use mooseng_common::types::Status;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Master server error: {0}")]
    MasterError(String),
    
    #[error("Connection error: {0}")]
    ConnectionError(#[from] tonic::transport::Error),
    
    #[error("gRPC error: {0}")]
    GrpcError(#[from] tonic::Status),
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("MooseFS status error: {0}")]
    StatusError(Status),
    
    #[error("File not found: inode {0}")]
    FileNotFound(u64),
    
    #[error("Permission denied")]
    PermissionDenied,
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    
    #[error("Operation not supported: {0}")]
    NotSupported(String),
}

impl From<Status> for ClientError {
    fn from(status: Status) -> Self {
        Self::StatusError(status)
    }
}

impl ClientError {
    pub fn to_errno(&self) -> libc::c_int {
        match self {
            ClientError::FileNotFound(_) => libc::ENOENT,
            ClientError::PermissionDenied => libc::EACCES,
            ClientError::InvalidArgument(_) => libc::EINVAL,
            ClientError::NotSupported(_) => libc::ENOSYS,
            ClientError::StatusError(status) => match status {
                Status::Ok => 0,
                Status::EPerm => libc::EPERM,
                Status::ENotDir => libc::ENOTDIR,
                Status::ENoEnt => libc::ENOENT,
                Status::EAcces => libc::EACCES,
                Status::EExist => libc::EEXIST,
                Status::EInval => libc::EINVAL,
                Status::ENotEmpty => libc::ENOTEMPTY,
                Status::EDQuot => libc::EDQUOT,
                Status::EIO => libc::EIO,
                Status::ENoSpc => libc::ENOSPC,
                Status::EBusy => libc::EBUSY,
                _ => libc::EIO,
            },
            _ => libc::EIO,
        }
    }
}

pub type ClientResult<T> = Result<T, ClientError>;