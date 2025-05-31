//! MooseNG Benchmarking Framework
//!
//! This crate provides comprehensive benchmarking tools for the MooseNG distributed file system.
//! It includes benchmarks for:
//! - File operations (small and large files)
//! - Metadata operations
//! - Multi-region scenarios
//! - Comparison with other distributed file systems

pub mod benchmarks;
pub mod config;
pub mod metrics;
pub mod report;
pub mod utils;
pub mod analysis;
pub mod test_framework;
pub mod database;
// pub mod html_report; // Temporarily disabled due to template syntax issues

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Benchmark result for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub operation: String,
    pub mean_time: Duration,
    pub std_dev: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
    pub throughput: Option<f64>,
    pub samples: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: Option<serde_json::Value>,
    pub percentiles: Option<Percentiles>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Percentiles {
    pub p50: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p999: Duration,
}

/// Configuration for running benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Number of warmup iterations
    pub warmup_iterations: usize,
    /// Number of measurement iterations
    pub measurement_iterations: usize,
    /// File sizes to test (in bytes)
    pub file_sizes: Vec<usize>,
    /// Number of concurrent operations
    pub concurrency_levels: Vec<usize>,
    /// Target regions for multi-region tests
    pub regions: Vec<String>,
    /// Enable detailed reporting
    pub detailed_report: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 10,
            measurement_iterations: 100,
            file_sizes: vec![
                1024,           // 1KB
                4096,           // 4KB
                65536,          // 64KB
                1048576,        // 1MB
                10485760,       // 10MB
                104857600,      // 100MB
            ],
            concurrency_levels: vec![1, 10, 50, 100],
            regions: vec!["us-east".to_string(), "eu-west".to_string(), "ap-south".to_string()],
            detailed_report: true,
        }
    }
}

/// Trait for implementing custom benchmarks
pub trait Benchmark {
    /// Run the benchmark and return results
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult>;
    
    /// Get the name of the benchmark
    fn name(&self) -> &str;
    
    /// Get a description of what this benchmark measures
    fn description(&self) -> &str;
}