//! MooseNG FUSE Client Library
//! 
//! This crate implements a FUSE filesystem client for MooseNG, providing
//! a POSIX-compatible filesystem interface to the distributed MooseNG storage system.

pub mod filesystem;
pub mod master_client;
pub mod cache;
pub mod error;
pub mod config;
pub mod health_checker;
pub mod health_integration;
pub mod health_endpoints;
pub mod metrics;

pub use crate::master_client::MasterClient;
pub use crate::config::ClientConfig;
pub use crate::error::{ClientError, ClientResult};
pub use crate::filesystem::MooseFuse;
pub use crate::health_checker::{ClientHealthChecker, ClientMetrics};
pub use crate::health_integration::ClientHealthService;
pub use crate::health_endpoints::ClientHealthEndpoints;