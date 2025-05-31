//! Multi-region performance benchmarks for MooseNG

use crate::{Benchmark, BenchmarkConfig, BenchmarkResult};
use criterion::black_box;
use futures::stream::{self, StreamExt};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Benchmark for cross-region replication performance
pub struct CrossRegionReplicationBenchmark {
    runtime: Arc<Runtime>,
}

impl CrossRegionReplicationBenchmark {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Runtime::new().unwrap()),
        }
    }
    
    async fn benchmark_sync_replication(&self, data_size: usize, region_count: usize) -> Duration {
        let data = vec![0u8; data_size];
        let start = Instant::now();
        
        // Simulate writing to primary region
        tokio::time::sleep(Duration::from_micros(100)).await;
        black_box(&data);
        
        // Simulate synchronous replication to other regions
        let replication_tasks = (1..region_count).map(|region| {
            let data = data.clone();
            async move {
                // Simulate network latency based on region
                let latency = match region {
                    1 => Duration::from_millis(20),  // Same continent
                    2 => Duration::from_millis(80),  // Cross-continent
                    _ => Duration::from_millis(150), // Global
                };
                tokio::time::sleep(latency).await;
                black_box(&data);
                Ok::<_, std::io::Error>(())
            }
        });
        
        let _: Vec<_> = stream::iter(replication_tasks)
            .buffer_unordered(region_count - 1)
            .collect()
            .await;
        
        start.elapsed()
    }
    
    async fn benchmark_async_replication(&self, data_size: usize, region_count: usize) -> Duration {
        let data = vec![0u8; data_size];
        let start = Instant::now();
        
        // Simulate writing to primary region
        tokio::time::sleep(Duration::from_micros(100)).await;
        black_box(&data);
        
        // Simulate async replication (fire and forget)
        for region in 1..region_count {
            let data = data.clone();
            tokio::spawn(async move {
                // Simulate async replication with varying delays
                let delay = Duration::from_millis(10 + region as u64 * 5);
                tokio::time::sleep(delay).await;
                black_box(&data);
            });
        }
        
        // Return time for primary write only (async doesn't block)
        start.elapsed()
    }
    
    async fn benchmark_quorum_write(&self, data_size: usize, region_count: usize, quorum_size: usize) -> Duration {
        let data = vec![0u8; data_size];
        let start = Instant::now();
        
        // Simulate writing to multiple regions with quorum
        let write_tasks = (0..region_count).map(|region| {
            let data = data.clone();
            async move {
                // Simulate varying network latencies
                let latency = match region {
                    0 => Duration::from_micros(100),    // Local
                    1 => Duration::from_millis(20),     // Same continent
                    2 => Duration::from_millis(80),     // Cross-continent
                    _ => Duration::from_millis(150),    // Global
                };
                tokio::time::sleep(latency).await;
                black_box(&data);
                Ok::<_, std::io::Error>(region)
            }
        });
        
        // Wait for quorum of successful writes
        let mut successful = 0;
        let mut stream = stream::iter(write_tasks).buffer_unordered(region_count);
        
        while let Some(result) = stream.next().await {
            if result.is_ok() {
                successful += 1;
                if successful >= quorum_size {
                    break;
                }
            }
        }
        
        start.elapsed()
    }
}

impl Benchmark for CrossRegionReplicationBenchmark {
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        let data_sizes = vec![1024, 65536, 1048576]; // 1KB, 64KB, 1MB
        let region_counts = vec![3, 5]; // 3 and 5 region deployments
        
        for &size in &data_sizes {
            for &regions in &region_counts {
                // Synchronous replication benchmark
                let sync_times: Vec<Duration> = (0..config.measurement_iterations)
                    .map(|_| {
                        self.runtime.block_on(self.benchmark_sync_replication(size, regions))
                    })
                    .collect();
                
                let sync_result = calculate_replication_stats(
                    &sync_times,
                    &format!("sync_replication_{}KB_{}_regions", size / 1024, regions),
                    size,
                );
                results.push(sync_result);
                
                // Asynchronous replication benchmark
                let async_times: Vec<Duration> = (0..config.measurement_iterations)
                    .map(|_| {
                        self.runtime.block_on(self.benchmark_async_replication(size, regions))
                    })
                    .collect();
                
                let async_result = calculate_replication_stats(
                    &async_times,
                    &format!("async_replication_{}KB_{}_regions", size / 1024, regions),
                    size,
                );
                results.push(async_result);
                
                // Quorum write benchmark (majority quorum)
                let quorum_size = (regions / 2) + 1;
                let quorum_times: Vec<Duration> = (0..config.measurement_iterations)
                    .map(|_| {
                        self.runtime.block_on(self.benchmark_quorum_write(size, regions, quorum_size))
                    })
                    .collect();
                
                let quorum_result = calculate_replication_stats(
                    &quorum_times,
                    &format!("quorum_write_{}KB_{}_regions", size / 1024, regions),
                    size,
                );
                results.push(quorum_result);
            }
        }
        
        results
    }
    
    fn name(&self) -> &str {
        "cross_region_replication"
    }
    
    fn description(&self) -> &str {
        "Benchmarks for cross-region replication including sync, async, and quorum writes"
    }
}

/// Benchmark for consistency models in multi-region deployments
pub struct ConsistencyBenchmark {
    runtime: Arc<Runtime>,
}

impl ConsistencyBenchmark {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Runtime::new().unwrap()),
        }
    }
    
    async fn benchmark_eventual_consistency(&self, operations: usize) -> Duration {
        let mut rng = StdRng::seed_from_u64(42);
        let start = Instant::now();
        
        // Simulate eventually consistent operations
        for i in 0..operations {
            let operation_type = rng.gen_range(0..3);
            match operation_type {
                0 => {
                    // Read operation - might see stale data
                    tokio::time::sleep(Duration::from_micros(50)).await;
                    black_box(format!("read_{}", i));
                }
                1 => {
                    // Write operation - propagates asynchronously
                    tokio::time::sleep(Duration::from_micros(100)).await;
                    black_box(format!("write_{}", i));
                }
                _ => {
                    // Update operation - conflicts resolved with CRDT
                    tokio::time::sleep(Duration::from_micros(150)).await;
                    black_box(format!("update_{}", i));
                }
            }
        }
        
        start.elapsed()
    }
    
    async fn benchmark_strong_consistency(&self, operations: usize) -> Duration {
        let start = Instant::now();
        
        // Simulate strongly consistent operations
        for i in 0..operations {
            // All operations go through consensus
            tokio::time::sleep(Duration::from_millis(5)).await;
            black_box(format!("consensus_op_{}", i));
        }
        
        start.elapsed()
    }
    
    async fn benchmark_bounded_staleness(&self, operations: usize, staleness_ms: u64) -> Duration {
        let mut rng = StdRng::seed_from_u64(42);
        let start = Instant::now();
        
        // Simulate operations with bounded staleness
        for i in 0..operations {
            let is_read = rng.gen_bool(0.8); // 80% reads
            
            if is_read {
                // Read might be stale but within bounds
                tokio::time::sleep(Duration::from_micros(30)).await;
                black_box(format!("bounded_read_{}", i));
            } else {
                // Write needs to propagate within staleness bound
                tokio::time::sleep(Duration::from_micros(100 + staleness_ms * 10)).await;
                black_box(format!("bounded_write_{}", i));
            }
        }
        
        start.elapsed()
    }
}

impl Benchmark for ConsistencyBenchmark {
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        let operation_counts = vec![100, 1000];
        
        for &ops in &operation_counts {
            // Eventual consistency benchmark
            let eventual_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_eventual_consistency(ops))
                })
                .collect();
            
            let eventual_result = calculate_consistency_stats(
                &eventual_times,
                &format!("eventual_consistency_{}_ops", ops),
                ops,
            );
            results.push(eventual_result);
            
            // Strong consistency benchmark
            let strong_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_strong_consistency(ops))
                })
                .collect();
            
            let strong_result = calculate_consistency_stats(
                &strong_times,
                &format!("strong_consistency_{}_ops", ops),
                ops,
            );
            results.push(strong_result);
            
            // Bounded staleness benchmark
            let bounded_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_bounded_staleness(ops, 100))
                })
                .collect();
            
            let bounded_result = calculate_consistency_stats(
                &bounded_times,
                &format!("bounded_staleness_{}_ops", ops),
                ops,
            );
            results.push(bounded_result);
        }
        
        results
    }
    
    fn name(&self) -> &str {
        "consistency_models"
    }
    
    fn description(&self) -> &str {
        "Benchmarks for different consistency models in multi-region deployments"
    }
}

/// Calculate statistics for replication benchmarks
fn calculate_replication_stats(times: &[Duration], operation: &str, data_size: usize) -> BenchmarkResult {
    let times_ns: Vec<f64> = times.iter().map(|d| d.as_nanos() as f64).collect();
    let mean = times_ns.iter().sum::<f64>() / times_ns.len() as f64;
    let variance = times_ns.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / times_ns.len() as f64;
    let std_dev = variance.sqrt();
    
    let min = times.iter().min().cloned().unwrap_or_default();
    let max = times.iter().max().cloned().unwrap_or_default();
    let mean_duration = Duration::from_nanos(mean as u64);
    
    // Calculate throughput in MB/s
    let throughput = if data_size > 0 && mean > 0.0 {
        Some((data_size as f64 / (1024.0 * 1024.0)) / (mean / 1_000_000_000.0))
    } else {
        None
    };
    
    BenchmarkResult {
        operation: operation.to_string(),
        mean_time: mean_duration,
        std_dev: Duration::from_nanos(std_dev as u64),
        min_time: min,
        max_time: max,
        throughput,
        samples: times.len(),
    }
}

/// Calculate statistics for consistency benchmarks
fn calculate_consistency_stats(times: &[Duration], operation: &str, operation_count: usize) -> BenchmarkResult {
    let times_ns: Vec<f64> = times.iter().map(|d| d.as_nanos() as f64).collect();
    let mean = times_ns.iter().sum::<f64>() / times_ns.len() as f64;
    let variance = times_ns.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / times_ns.len() as f64;
    let std_dev = variance.sqrt();
    
    let min = times.iter().min().cloned().unwrap_or_default();
    let max = times.iter().max().cloned().unwrap_or_default();
    let mean_duration = Duration::from_nanos(mean as u64);
    
    // Calculate operations per second
    let ops_per_sec = if mean > 0.0 {
        Some((operation_count as f64) / (mean / 1_000_000_000.0))
    } else {
        None
    };
    
    BenchmarkResult {
        operation: operation.to_string(),
        mean_time: mean_duration,
        std_dev: Duration::from_nanos(std_dev as u64),
        min_time: min,
        max_time: max,
        throughput: ops_per_sec,
        samples: times.len(),
    }
}