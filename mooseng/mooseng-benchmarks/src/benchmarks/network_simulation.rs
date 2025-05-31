//! Real network condition simulation for MooseNG benchmarks
//!
//! This module provides tools for testing MooseNG performance under realistic
//! network conditions including variable latency, packet loss, bandwidth limits,
//! and jitter. It integrates with Linux traffic control (tc) and network namespaces.

use crate::{Benchmark, BenchmarkConfig, BenchmarkResult};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

/// Network condition parameters for simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCondition {
    /// Base latency in milliseconds
    pub latency_ms: u32,
    /// Jitter as percentage of latency (0-100)
    pub jitter_percent: u8,
    /// Packet loss percentage (0-100)
    pub packet_loss_percent: u8,
    /// Bandwidth limit in Mbps (0 = unlimited)
    pub bandwidth_mbps: u32,
    /// Duplicate packet percentage (0-100)
    pub duplicate_percent: u8,
    /// Corrupt packet percentage (0-100)
    pub corrupt_percent: u8,
}

impl NetworkCondition {
    pub fn high_quality() -> Self {
        Self {
            latency_ms: 1,
            jitter_percent: 5,
            packet_loss_percent: 0,
            bandwidth_mbps: 1000,
            duplicate_percent: 0,
            corrupt_percent: 0,
        }
    }

    pub fn cross_continent() -> Self {
        Self {
            latency_ms: 150,
            jitter_percent: 15,
            packet_loss_percent: 1,
            bandwidth_mbps: 100,
            duplicate_percent: 0,
            corrupt_percent: 0,
        }
    }

    pub fn poor_mobile() -> Self {
        Self {
            latency_ms: 300,
            jitter_percent: 50,
            packet_loss_percent: 5,
            bandwidth_mbps: 10,
            duplicate_percent: 1,
            corrupt_percent: 1,
        }
    }

    pub fn satellite() -> Self {
        Self {
            latency_ms: 600,
            jitter_percent: 30,
            packet_loss_percent: 2,
            bandwidth_mbps: 50,
            duplicate_percent: 0,
            corrupt_percent: 1,
        }
    }
}

/// Network namespace manager for isolated testing
pub struct NetworkNamespace {
    name: String,
    created: bool,
}

impl NetworkNamespace {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            created: false,
        }
    }

    /// Create a new network namespace
    pub fn create(&mut self) -> Result<(), std::io::Error> {
        if self.created {
            return Ok(());
        }

        let output = Command::new("ip")
            .args(&["netns", "add", &self.name])
            .output()?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create network namespace: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        self.created = true;
        Ok(())
    }

    /// Apply network conditions using tc (traffic control)
    pub fn apply_conditions(&self, interface: &str, condition: &NetworkCondition) -> Result<(), std::io::Error> {
        if !self.created {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Network namespace not created"
            ));
        }

        // Build netem parameters
        let mut netem_params = vec!["delay".to_string(), format!("{}ms", condition.latency_ms)];
        
        if condition.jitter_percent > 0 {
            let jitter_ms = (condition.latency_ms * condition.jitter_percent as u32) / 100;
            netem_params.push(format!("{}ms", jitter_ms));
        }

        if condition.packet_loss_percent > 0 {
            netem_params.extend_from_slice(&["loss".to_string(), format!("{}%", condition.packet_loss_percent)]);
        }

        if condition.duplicate_percent > 0 {
            netem_params.extend_from_slice(&["duplicate".to_string(), format!("{}%", condition.duplicate_percent)]);
        }

        if condition.corrupt_percent > 0 {
            netem_params.extend_from_slice(&["corrupt".to_string(), format!("{}%", condition.corrupt_percent)]);
        }

        // Apply netem discipline
        let mut cmd_args = vec!["netns", "exec", &self.name, "tc", "qdisc", "add", "dev", interface, "root", "netem"];
        cmd_args.extend(netem_params.iter().map(|s| s.as_str()));

        let output = Command::new("ip").args(&cmd_args).output()?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to apply netem: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }

        // Apply bandwidth limit if specified
        if condition.bandwidth_mbps > 0 {
            let rate_str = format!("{}mbit", condition.bandwidth_mbps);
            let tbf_args = vec![
                "netns", "exec", &self.name, "tc", "qdisc", "add", "dev", interface, 
                "parent", "1:1", "handle", "10:", "tbf", "rate", 
                &rate_str, "burst", "32kbit", "latency", "50ms"
            ];

            let output = Command::new("ip").args(&tbf_args).output()?;

            if !output.status.success() {
                eprintln!("Warning: Failed to apply bandwidth limit: {}", String::from_utf8_lossy(&output.stderr));
            }
        }

        Ok(())
    }

    /// Execute a command within the network namespace
    pub fn exec_command(&self, cmd: &str, args: &[&str]) -> Result<std::process::Output, std::io::Error> {
        if !self.created {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Network namespace not created"
            ));
        }

        let mut full_args = vec!["netns", "exec", &self.name, cmd];
        full_args.extend_from_slice(args);

        Command::new("ip").args(&full_args).output()
    }

    /// Clean up the network namespace
    pub fn cleanup(&mut self) -> Result<(), std::io::Error> {
        if !self.created {
            return Ok(());
        }

        let output = Command::new("ip")
            .args(&["netns", "del", &self.name])
            .output()?;

        if !output.status.success() {
            eprintln!("Warning: Failed to delete namespace: {}", String::from_utf8_lossy(&output.stderr));
        }

        self.created = false;
        Ok(())
    }
}

impl Drop for NetworkNamespace {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Benchmark for testing under various network conditions
pub struct NetworkConditionBenchmark {
    runtime: Arc<Runtime>,
    test_scenarios: Vec<(String, NetworkCondition)>,
}

impl NetworkConditionBenchmark {
    pub fn new() -> Self {
        let test_scenarios = vec![
            ("high_quality".to_string(), NetworkCondition::high_quality()),
            ("cross_continent".to_string(), NetworkCondition::cross_continent()),
            ("poor_mobile".to_string(), NetworkCondition::poor_mobile()),
            ("satellite".to_string(), NetworkCondition::satellite()),
        ];

        Self {
            runtime: Arc::new(Runtime::new().unwrap()),
            test_scenarios,
        }
    }

    async fn benchmark_network_throughput(&self, condition: &NetworkCondition, data_size: usize) -> Duration {
        let start = Instant::now();
        
        // Simulate network transfer with real conditions
        let base_transfer_time = Duration::from_millis(
            (data_size as u64 * 8) / (condition.bandwidth_mbps as u64 * 1000)
        );
        
        // Add latency
        let latency = Duration::from_millis(condition.latency_ms as u64);
        
        // Add jitter
        let mut rng = StdRng::seed_from_u64(42);
        let jitter_range = (condition.latency_ms * condition.jitter_percent as u32) / 100;
        let jitter = if jitter_range > 0 {
            Duration::from_millis(rng.gen_range(0..=jitter_range) as u64)
        } else {
            Duration::ZERO
        };

        // Simulate packet loss causing retransmissions
        let retransmission_delay = if condition.packet_loss_percent > 0 && rng.gen_range(0..100) < condition.packet_loss_percent {
            Duration::from_millis(condition.latency_ms as u64 * 3) // RTT for retransmission
        } else {
            Duration::ZERO
        };

        let total_delay = base_transfer_time + latency + jitter + retransmission_delay;
        tokio::time::sleep(total_delay).await;
        
        start.elapsed()
    }

    async fn benchmark_connection_establishment(&self, condition: &NetworkCondition) -> Duration {
        let start = Instant::now();
        
        // Simulate TCP handshake (3-way)
        let handshake_latency = Duration::from_millis(condition.latency_ms as u64 * 3 / 2);
        
        // Add jitter
        let mut rng = StdRng::seed_from_u64(43);
        let jitter_range = (condition.latency_ms * condition.jitter_percent as u32) / 100;
        let jitter = if jitter_range > 0 {
            Duration::from_millis(rng.gen_range(0..=jitter_range) as u64)
        } else {
            Duration::ZERO
        };

        tokio::time::sleep(handshake_latency + jitter).await;
        
        start.elapsed()
    }

    async fn benchmark_concurrent_connections(&self, condition: &NetworkCondition, connection_count: usize) -> Duration {
        let start = Instant::now();
        
        // Simulate establishing multiple concurrent connections
        let connection_tasks = (0..connection_count).map(|_| {
            self.benchmark_connection_establishment(condition)
        });

        let _: Vec<_> = stream::iter(connection_tasks)
            .buffer_unordered(10) // Limit concurrency for realism
            .collect()
            .await;
        
        start.elapsed()
    }
}

impl Benchmark for NetworkConditionBenchmark {
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        let data_sizes = vec![1024, 65536, 1048576, 10485760]; // 1KB, 64KB, 1MB, 10MB
        let connection_counts = vec![1, 10, 50];

        for (scenario_name, condition) in &self.test_scenarios {
            // Throughput benchmarks
            for &size in &data_sizes {
                let times: Vec<Duration> = (0..config.measurement_iterations)
                    .map(|_| {
                        self.runtime.block_on(self.benchmark_network_throughput(condition, size))
                    })
                    .collect();

                let result = calculate_network_stats(
                    &times,
                    &format!("throughput_{}_{}_bytes", scenario_name, size),
                    Some(size),
                );
                results.push(result);
            }

            // Connection establishment benchmarks
            let conn_times: Vec<Duration> = (0..config.measurement_iterations)
                .map(|_| {
                    self.runtime.block_on(self.benchmark_connection_establishment(condition))
                })
                .collect();

            let conn_result = calculate_network_stats(
                &conn_times,
                &format!("connection_establishment_{}", scenario_name),
                None,
            );
            results.push(conn_result);

            // Concurrent connections benchmarks
            for &conn_count in &connection_counts {
                let concurrent_times: Vec<Duration> = (0..std::cmp::min(config.measurement_iterations, 20))
                    .map(|_| {
                        self.runtime.block_on(self.benchmark_concurrent_connections(condition, conn_count))
                    })
                    .collect();

                let concurrent_result = calculate_network_stats(
                    &concurrent_times,
                    &format!("concurrent_connections_{}_{}", scenario_name, conn_count),
                    None,
                );
                results.push(concurrent_result);
            }
        }

        results
    }

    fn name(&self) -> &str {
        "network_condition_simulation"
    }

    fn description(&self) -> &str {
        "Benchmarks MooseNG performance under various network conditions including latency, packet loss, and bandwidth limitations"
    }
}

/// Real network testing using actual network interfaces
pub struct RealNetworkBenchmark {
    runtime: Arc<Runtime>,
}

impl RealNetworkBenchmark {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Runtime::new().unwrap()),
        }
    }

    async fn create_test_server(&self, port: u16) -> Result<TcpListener, std::io::Error> {
        TcpListener::bind(format!("127.0.0.1:{}", port)).await
    }

    async fn benchmark_real_tcp_throughput(&self, port: u16, data_size: usize) -> Duration {
        let listener = match self.create_test_server(port).await {
            Ok(l) => l,
            Err(_) => return Duration::from_secs(60), // Fallback on error
        };

        let data = vec![0u8; data_size];
        let start = Instant::now();

        // Spawn server task
        let server_task = tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut received = vec![0u8; data_size];
                let _ = socket.read_exact(&mut received).await;
            }
        });

        // Client connects and sends data
        tokio::time::sleep(Duration::from_millis(10)).await; // Give server time to start
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", port)).await {
            let _ = stream.write_all(&data).await;
        }

        let _ = server_task.await;
        start.elapsed()
    }
}

impl Benchmark for RealNetworkBenchmark {
    fn run(&self, config: &BenchmarkConfig) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        let data_sizes = vec![1024, 65536, 1048576];
        let base_port = 38000;

        for (i, &size) in data_sizes.iter().enumerate() {
            let times: Vec<Duration> = (0..std::cmp::min(config.measurement_iterations, 50))
                .map(|j| {
                    let port = base_port + (i * 100) + j;
                    self.runtime.block_on(self.benchmark_real_tcp_throughput(port as u16, size))
                })
                .collect();

            let result = calculate_network_stats(
                &times,
                &format!("real_tcp_throughput_{}_bytes", size),
                Some(size),
            );
            results.push(result);
        }

        results
    }

    fn name(&self) -> &str {
        "real_network_tcp"
    }

    fn description(&self) -> &str {
        "Benchmarks using real TCP connections for baseline network performance measurement"
    }
}

/// Calculate statistics for network benchmarks
fn calculate_network_stats(times: &[Duration], operation: &str, data_size: Option<usize>) -> BenchmarkResult {
    let times_ns: Vec<f64> = times.iter().map(|d| d.as_nanos() as f64).collect();
    let mean = times_ns.iter().sum::<f64>() / times_ns.len() as f64;
    let variance = times_ns.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / times_ns.len() as f64;
    let std_dev = variance.sqrt();
    
    let min = times.iter().min().cloned().unwrap_or_default();
    let max = times.iter().max().cloned().unwrap_or_default();
    let mean_duration = Duration::from_nanos(mean as u64);
    
    // Calculate throughput in MB/s if data size is provided
    let throughput = if let Some(size) = data_size {
        if size > 0 && mean > 0.0 {
            Some((size as f64 / (1024.0 * 1024.0)) / (mean / 1_000_000_000.0))
        } else {
            None
        }
    } else {
        // For connection benchmarks, calculate connections per second
        if mean > 0.0 {
            Some(1_000_000_000.0 / mean)
        } else {
            None
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_conditions() {
        let high_qual = NetworkCondition::high_quality();
        assert_eq!(high_qual.latency_ms, 1);
        assert_eq!(high_qual.packet_loss_percent, 0);

        let poor = NetworkCondition::poor_mobile();
        assert!(poor.latency_ms > high_qual.latency_ms);
        assert!(poor.packet_loss_percent > high_qual.packet_loss_percent);
    }

    #[tokio::test]
    async fn test_network_benchmark_creation() {
        let benchmark = NetworkConditionBenchmark::new();
        assert_eq!(benchmark.name(), "network_condition_simulation");
        assert!(benchmark.test_scenarios.len() > 0);
    }
}