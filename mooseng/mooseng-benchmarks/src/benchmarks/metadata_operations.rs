//! Metadata operation benchmarks for MooseNG

use crate::{Benchmark, BenchmarkConfig, BenchmarkResult};
use criterion::black_box;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Benchmark for file/directory creation and deletion
pub struct CreateDeleteBenchmark {
    runtime: Arc<Runtime>,
}

impl CreateDeleteBenchmark {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Runtime::new().unwrap()),
        }
    }
    
    async fn benchmark_create_files(&self, count: usize) -> Duration {
        let start = Instant::now();
        
        let tasks = (0..count).map(|i| {
            async move {
                // TODO: Replace with actual MooseNG client create file operation
                tokio::time::sleep(Duration::from_micros(50)).await;
                black_box(format!("file_{}", i));
                Ok::<_, std::io::Error>(())
            }
        });
        
        let _: Vec<_> = stream::iter(tasks)
            .buffer_unordered(50)
            .collect()
            .await;
        
        start.elapsed()
    }
    
    async fn benchmark_delete_files(&self, count: usize) -> Duration {
        let start = Instant::now();
        
        let tasks = (0..count).map(|i| {
            async move {
                // TODO: Replace with actual MooseNG client delete file operation
                tokio::time::sleep(Duration::from_micros(50)).await;
                black_box(format!("file_{}", i));
                Ok::<_, std::io::Error>(())
            }
        });
        
        let _: Vec<_> = stream::iter(tasks)
            .buffer_unordered(50)
            .collect()
            .await;
        
        start.elapsed()
    }
    
    async fn benchmark_create_directories(&self, count: usize) -> Duration {
        let start = Instant::now();
        
        for i in 0..count {
            // TODO: Replace with actual MooseNG client create directory operation
            tokio::time::sleep(Duration::from_micros(75)).await;
            black_box(format!("dir_{}", i));
        }
        
        start.elapsed()
    }
}

impl Benchmark for CreateDeleteBenchmark {
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        let file_counts = vec![100, 1000, 10000];
        
        for &count in &file_counts {
            // File creation benchmark
            let create_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_create_files(count))
                })
                .collect();
            
            let create_result = calculate_metadata_stats(&create_times, &format!("create_{}_files", count), count);
            results.push(create_result);
            
            // File deletion benchmark
            let delete_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_delete_files(count))
                })
                .collect();
            
            let delete_result = calculate_metadata_stats(&delete_times, &format!("delete_{}_files", count), count);
            results.push(delete_result);
        }
        
        // Directory creation benchmark
        let dir_times: Vec<Duration> = (0..config.measurement_iterations)
            .map(|_| {
                self.runtime.block_on(self.benchmark_create_directories(100))
            })
            .collect();
        
        let dir_result = calculate_metadata_stats(&dir_times, "create_100_directories", 100);
        results.push(dir_result);
        
        results
    }
    
    fn name(&self) -> &str {
        "create_delete_metadata"
    }
    
    fn description(&self) -> &str {
        "Benchmarks for file and directory creation/deletion operations"
    }
}

/// Benchmark for directory listing operations
pub struct ListingBenchmark {
    runtime: Arc<Runtime>,
}

impl ListingBenchmark {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Runtime::new().unwrap()),
        }
    }
    
    async fn benchmark_list_directory(&self, file_count: usize) -> Duration {
        let start = Instant::now();
        
        // TODO: Replace with actual MooseNG client list directory operation
        tokio::time::sleep(Duration::from_micros(100 + file_count as u64 / 10)).await;
        let entries: Vec<String> = (0..file_count).map(|i| format!("file_{}", i)).collect();
        black_box(&entries);
        
        start.elapsed()
    }
    
    async fn benchmark_recursive_list(&self, depth: usize, files_per_dir: usize) -> Duration {
        let start = Instant::now();
        
        // Simulate recursive directory traversal
        let total_ops = depth * files_per_dir;
        tokio::time::sleep(Duration::from_micros(200 * total_ops as u64)).await;
        black_box(total_ops);
        
        start.elapsed()
    }
}

impl Benchmark for ListingBenchmark {
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        let dir_sizes = vec![10, 100, 1000, 10000];
        
        for &size in &dir_sizes {
            // Simple listing benchmark
            let list_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_list_directory(size))
                })
                .collect();
            
            let list_result = calculate_metadata_stats(&list_times, &format!("list_{}_files", size), size);
            results.push(list_result);
        }
        
        // Recursive listing benchmark
        let recursive_times: Vec<Duration> = (0..config.measurement_iterations)
            .map(|_| {
                self.runtime.block_on(self.benchmark_recursive_list(5, 100))
            })
            .collect();
        
        let recursive_result = calculate_metadata_stats(&recursive_times, "recursive_list_5_levels", 500);
        results.push(recursive_result);
        
        results
    }
    
    fn name(&self) -> &str {
        "directory_listing"
    }
    
    fn description(&self) -> &str {
        "Benchmarks for directory listing operations including recursive traversal"
    }
}

/// Benchmark for file attribute operations
pub struct AttributesBenchmark {
    runtime: Arc<Runtime>,
}

impl AttributesBenchmark {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Runtime::new().unwrap()),
        }
    }
    
    async fn benchmark_stat_operations(&self, count: usize) -> Duration {
        let start = Instant::now();
        
        let tasks = (0..count).map(|i| {
            async move {
                // TODO: Replace with actual MooseNG client stat operation
                tokio::time::sleep(Duration::from_micros(30)).await;
                black_box(format!("file_{}", i));
                Ok::<_, std::io::Error>(())
            }
        });
        
        let _: Vec<_> = stream::iter(tasks)
            .buffer_unordered(100)
            .collect()
            .await;
        
        start.elapsed()
    }
    
    async fn benchmark_chmod_operations(&self, count: usize) -> Duration {
        let start = Instant::now();
        
        for i in 0..count {
            // TODO: Replace with actual MooseNG client chmod operation
            tokio::time::sleep(Duration::from_micros(40)).await;
            black_box((format!("file_{}", i), 0o644));
        }
        
        start.elapsed()
    }
    
    async fn benchmark_xattr_operations(&self, count: usize) -> Duration {
        let start = Instant::now();
        
        for i in 0..count {
            // TODO: Replace with actual MooseNG client xattr operation
            tokio::time::sleep(Duration::from_micros(60)).await;
            black_box((format!("file_{}", i), "user.test", "value"));
        }
        
        start.elapsed()
    }
}

impl Benchmark for AttributesBenchmark {
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        let operation_counts = vec![100, 1000];
        
        for &count in &operation_counts {
            // Stat operations benchmark
            let stat_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_stat_operations(count))
                })
                .collect();
            
            let stat_result = calculate_metadata_stats(&stat_times, &format!("stat_{}_files", count), count);
            results.push(stat_result);
            
            // Chmod operations benchmark
            let chmod_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_chmod_operations(count))
                })
                .collect();
            
            let chmod_result = calculate_metadata_stats(&chmod_times, &format!("chmod_{}_files", count), count);
            results.push(chmod_result);
            
            // Extended attributes benchmark
            let xattr_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_xattr_operations(count))
                })
                .collect();
            
            let xattr_result = calculate_metadata_stats(&xattr_times, &format!("xattr_{}_operations", count), count);
            results.push(xattr_result);
        }
        
        results
    }
    
    fn name(&self) -> &str {
        "file_attributes"
    }
    
    fn description(&self) -> &str {
        "Benchmarks for file attribute operations including stat, chmod, and extended attributes"
    }
}

/// Calculate statistics for metadata operations
fn calculate_metadata_stats(times: &[Duration], operation: &str, operation_count: usize) -> BenchmarkResult {
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