//! Comprehensive test framework for MooseNG benchmarking
//! 
//! This module provides utilities for setting up test environments,
//! managing test data, and coordinating benchmark execution.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::fs;
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug};

/// Test environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEnvironment {
    /// Base directory for test data
    pub test_data_dir: PathBuf,
    /// Directory for test results
    pub results_dir: PathBuf,
    /// Directory for logs
    pub logs_dir: PathBuf,
    /// Temporary directory for test artifacts
    pub temp_dir: PathBuf,
    /// Available test regions
    pub regions: Vec<TestRegion>,
    /// Network simulation settings
    pub network_config: NetworkConfig,
    /// Resource limits for tests
    pub resource_limits: ResourceLimits,
}

/// Test region configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRegion {
    pub name: String,
    pub endpoint: String,
    pub latency_ms: u64,
    pub bandwidth_mbps: u64,
    pub enabled: bool,
}

/// Network simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable latency simulation
    pub simulate_latency: bool,
    /// Enable packet loss simulation  
    pub simulate_packet_loss: bool,
    /// Enable bandwidth limiting
    pub simulate_bandwidth_limits: bool,
    /// Base latency between regions (ms)
    pub base_latency_ms: u64,
    /// Packet loss percentage (0.0 - 1.0)
    pub packet_loss_rate: f64,
    /// Bandwidth limit in Mbps
    pub bandwidth_limit_mbps: u64,
}

/// Resource limits for test execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    /// Maximum test duration
    pub max_duration: Duration,
    /// Maximum file size for tests
    pub max_file_size_mb: u64,
}

/// Test data generator for creating benchmark data
pub struct TestDataGenerator {
    base_dir: PathBuf,
    config: TestDataConfig,
}

/// Configuration for test data generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDataConfig {
    /// Sizes of small files to generate
    pub small_file_sizes: Vec<usize>,
    /// Sizes of large files to generate
    pub large_file_sizes: Vec<usize>,
    /// Number of metadata test files
    pub metadata_file_count: usize,
    /// Directory depth for metadata tests
    pub directory_depth: usize,
    /// File patterns to generate
    pub file_patterns: Vec<FilePattern>,
}

/// File pattern for generating test data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePattern {
    pub name: String,
    pub size_bytes: usize,
    pub count: usize,
    pub content_type: ContentType,
}

/// Type of content for test files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    Random,
    Zeros,
    Ones,
    Pattern(Vec<u8>),
    Text(String),
}

/// Test executor for running benchmarks
pub struct TestExecutor {
    environment: TestEnvironment,
    metrics_collector: MetricsCollector,
}

/// Metrics collection during tests
pub struct MetricsCollector {
    start_time: Option<Instant>,
    measurements: HashMap<String, Vec<Duration>>,
    system_metrics: Vec<SystemMetrics>,
}

/// System metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: std::time::SystemTime,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_io_bytes_per_sec: u64,
    pub network_io_bytes_per_sec: u64,
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self {
            test_data_dir: PathBuf::from("test_data"),
            results_dir: PathBuf::from("test_results"),
            logs_dir: PathBuf::from("logs"),
            temp_dir: PathBuf::from("tmp"),
            regions: vec![
                TestRegion {
                    name: "local".to_string(),
                    endpoint: "127.0.0.1:8080".to_string(),
                    latency_ms: 1,
                    bandwidth_mbps: 1000,
                    enabled: true,
                },
                TestRegion {
                    name: "remote".to_string(),
                    endpoint: "127.0.0.1:8081".to_string(),
                    latency_ms: 50,
                    bandwidth_mbps: 100,
                    enabled: false,
                },
            ],
            network_config: NetworkConfig::default(),
            resource_limits: ResourceLimits::default(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            simulate_latency: false,
            simulate_packet_loss: false,
            simulate_bandwidth_limits: false,
            base_latency_ms: 10,
            packet_loss_rate: 0.0,
            bandwidth_limit_mbps: 1000,
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 1024,
            max_cpu_percent: 80.0,
            max_duration: Duration::from_secs(300), // 5 minutes
            max_file_size_mb: 100,
        }
    }
}

impl Default for TestDataConfig {
    fn default() -> Self {
        Self {
            small_file_sizes: vec![1024, 4096, 65536], // 1KB, 4KB, 64KB
            large_file_sizes: vec![1048576, 10485760], // 1MB, 10MB
            metadata_file_count: 100,
            directory_depth: 3,
            file_patterns: vec![
                FilePattern {
                    name: "sequential".to_string(),
                    size_bytes: 4096,
                    count: 50,
                    content_type: ContentType::Random,
                },
                FilePattern {
                    name: "sparse".to_string(),
                    size_bytes: 1048576,
                    count: 10,
                    content_type: ContentType::Zeros,
                },
            ],
        }
    }
}

impl TestDataGenerator {
    pub fn new(base_dir: PathBuf, config: TestDataConfig) -> Self {
        Self { base_dir, config }
    }
    
    /// Generate all test data according to configuration
    pub async fn generate_all(&self) -> Result<()> {
        info!("Generating test data in: {:?}", self.base_dir);
        
        // Create base directories
        fs::create_dir_all(&self.base_dir).await?;
        fs::create_dir_all(self.base_dir.join("small_files")).await?;
        fs::create_dir_all(self.base_dir.join("large_files")).await?;
        fs::create_dir_all(self.base_dir.join("metadata")).await?;
        
        // Generate small files
        for &size in &self.config.small_file_sizes {
            self.generate_file("small_files", &format!("test_{}.dat", size), size, ContentType::Random).await?;
        }
        
        // Generate large files
        for &size in &self.config.large_file_sizes {
            self.generate_file("large_files", &format!("test_{}.dat", size), size, ContentType::Random).await?;
        }
        
        // Generate metadata test files
        self.generate_metadata_files().await?;
        
        // Generate pattern-based files
        for pattern in &self.config.file_patterns {
            self.generate_pattern_files(pattern).await?;
        }
        
        info!("Test data generation completed");
        Ok(())
    }
    
    async fn generate_file(&self, subdir: &str, filename: &str, size: usize, content_type: ContentType) -> Result<()> {
        let file_path = self.base_dir.join(subdir).join(filename);
        
        let content = match content_type {
            ContentType::Random => self.generate_random_content(size),
            ContentType::Zeros => vec![0u8; size],
            ContentType::Ones => vec![1u8; size],
            ContentType::Pattern(pattern) => {
                let mut content = Vec::with_capacity(size);
                for _ in 0..size {
                    content.extend_from_slice(&pattern);
                    if content.len() >= size {
                        content.truncate(size);
                        break;
                    }
                }
                content
            },
            ContentType::Text(text) => {
                let mut content = Vec::with_capacity(size);
                let text_bytes = text.as_bytes();
                while content.len() < size {
                    content.extend_from_slice(text_bytes);
                }
                content.truncate(size);
                content
            },
        };
        
        fs::write(file_path, content).await?;
        debug!("Generated file: {}/{} ({} bytes)", subdir, filename, size);
        Ok(())
    }
    
    async fn generate_metadata_files(&self) -> Result<()> {
        let metadata_dir = self.base_dir.join("metadata");
        
        // Create directory structure
        for depth in 1..=self.config.directory_depth {
            for i in 1..=5 {
                let dir_path = metadata_dir.join(format!("level{}", depth)).join(format!("dir{}", i));
                fs::create_dir_all(&dir_path).await?;
                
                // Create files in each directory
                for j in 1..=(self.config.metadata_file_count / (self.config.directory_depth * 5)) {
                    let file_path = dir_path.join(format!("file_{}.txt", j));
                    let content = format!("Test file {} in directory level {} dir {}", j, depth, i);
                    fs::write(file_path, content).await?;
                }
            }
        }
        
        info!("Generated {} metadata test files", self.config.metadata_file_count);
        Ok(())
    }
    
    async fn generate_pattern_files(&self, pattern: &FilePattern) -> Result<()> {
        let pattern_dir = self.base_dir.join("patterns").join(&pattern.name);
        fs::create_dir_all(&pattern_dir).await?;
        
        for i in 1..=pattern.count {
            let filename = format!("{}_{}.dat", pattern.name, i);
            self.generate_file("patterns", &format!("{}/{}", pattern.name, filename), 
                             pattern.size_bytes, pattern.content_type.clone()).await?;
        }
        
        info!("Generated {} files for pattern: {}", pattern.count, pattern.name);
        Ok(())
    }
    
    fn generate_random_content(&self, size: usize) -> Vec<u8> {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut content = vec![0u8; size];
        rng.fill_bytes(&mut content);
        content
    }
}

impl TestExecutor {
    pub fn new(environment: TestEnvironment) -> Self {
        Self {
            environment,
            metrics_collector: MetricsCollector::new(),
        }
    }
    
    /// Setup the test environment
    pub async fn setup(&mut self) -> Result<()> {
        info!("Setting up test environment...");
        
        // Create necessary directories
        fs::create_dir_all(&self.environment.test_data_dir).await?;
        fs::create_dir_all(&self.environment.results_dir).await?;
        fs::create_dir_all(&self.environment.logs_dir).await?;
        fs::create_dir_all(&self.environment.temp_dir).await?;
        
        // Generate test data
        let data_generator = TestDataGenerator::new(
            self.environment.test_data_dir.clone(),
            TestDataConfig::default(),
        );
        data_generator.generate_all().await?;
        
        // Initialize metrics collection
        self.metrics_collector.start();
        
        info!("Test environment setup completed");
        Ok(())
    }
    
    /// Cleanup test environment
    pub async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up test environment...");
        
        // Stop metrics collection
        self.metrics_collector.stop();
        
        // Save final metrics
        let metrics_file = self.environment.results_dir.join("system_metrics.json");
        self.metrics_collector.save_metrics(&metrics_file).await?;
        
        info!("Test environment cleanup completed");
        Ok(())
    }
    
    /// Run a benchmark with metrics collection
    pub async fn run_benchmark<F, Fut>(&mut self, name: &str, benchmark_fn: F) -> Result<Duration>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        info!("Starting benchmark: {}", name);
        
        let start_time = Instant::now();
        
        // Run the benchmark
        benchmark_fn().await?;
        
        let duration = start_time.elapsed();
        
        // Record the measurement
        self.metrics_collector.record_measurement(name, duration);
        
        info!("Benchmark {} completed in {:?}", name, duration);
        Ok(duration)
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: None,
            measurements: HashMap::new(),
            system_metrics: Vec::new(),
        }
    }
    
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        info!("Metrics collection started");
    }
    
    pub fn stop(&mut self) {
        if let Some(start_time) = self.start_time.take() {
            let total_duration = start_time.elapsed();
            info!("Metrics collection stopped after {:?}", total_duration);
        }
    }
    
    pub fn record_measurement(&mut self, name: &str, duration: Duration) {
        self.measurements.entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(duration);
    }
    
    pub async fn save_metrics(&self, path: &Path) -> Result<()> {
        let summary = self.generate_summary();
        let json = serde_json::to_string_pretty(&summary)?;
        fs::write(path, json).await?;
        info!("Metrics saved to: {:?}", path);
        Ok(())
    }
    
    fn generate_summary(&self) -> serde_json::Value {
        use serde_json::json;
        
        let mut benchmark_summary = serde_json::Map::new();
        
        for (name, measurements) in &self.measurements {
            if !measurements.is_empty() {
                let total_measurements = measurements.len();
                let total_time: Duration = measurements.iter().sum();
                let avg_time = total_time / total_measurements as u32;
                let min_time = measurements.iter().min().copied().unwrap_or_default();
                let max_time = measurements.iter().max().copied().unwrap_or_default();
                
                benchmark_summary.insert(name.clone(), json!({
                    "total_measurements": total_measurements,
                    "average_duration_ms": avg_time.as_millis(),
                    "min_duration_ms": min_time.as_millis(),
                    "max_duration_ms": max_time.as_millis(),
                    "total_duration_ms": total_time.as_millis(),
                }));
            }
        }
        
        json!({
            "benchmark_results": benchmark_summary,
            "system_metrics_count": self.system_metrics.len(),
            "collection_start": self.start_time.map(|t| t.elapsed().as_secs()),
        })
    }
}

/// Convenience function to create a default test environment
pub fn create_default_test_environment() -> TestEnvironment {
    TestEnvironment::default()
}

/// Convenience function to run a simple benchmark
pub async fn run_simple_benchmark<F, Fut>(name: &str, benchmark_fn: F) -> Result<Duration>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let mut executor = TestExecutor::new(create_default_test_environment());
    executor.setup().await?;
    let result = executor.run_benchmark(name, benchmark_fn).await;
    executor.cleanup().await?;
    result
}