//! MooseNG - Next Generation Distributed File System
//!
//! This is the main crate that coordinates all MooseNG components and provides
//! utilities for benchmarking, reporting, and system management.

pub use mooseng_benchmarks as benchmarks;

/// Re-export common types and utilities
pub mod prelude {
    pub use crate::benchmarks::*;
}