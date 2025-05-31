//! MooseNG FUSE Client Library
//! 
//! This crate implements a FUSE filesystem client for MooseNG, providing
//! a POSIX-compatible filesystem interface to the distributed MooseNG storage system.

pub mod filesystem;
pub mod master_client;
pub mod cache;
pub mod error;
pub mod config;

pub use crate::master_client::MasterClient;
pub use crate::config::ClientConfig;
pub use crate::error::{ClientError, ClientResult};
pub use crate::filesystem::MooseFuse;