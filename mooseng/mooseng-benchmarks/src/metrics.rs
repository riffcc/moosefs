//! Advanced metrics collection and analysis for MooseNG benchmarks
//!
//! This module provides comprehensive metrics collection, real-time analysis,
//! and visualization capabilities for benchmarking results.

use crate::{BenchmarkResult, BenchmarkConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

/// System performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_usage: usize,
    pub disk_usage: f64,
    pub network_rx: usize,
    pub network_tx: usize,
    pub timestamp: u64,
}

/// Network performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub latency_ms: f64,
    pub bandwidth_mbps: f64,
    pub packet_loss: f64,
    pub jitter_ms: f64,
    pub connections_active: usize,
    pub timestamp: u64,
}

/// Summary of metrics for a given time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_operations: usize,
    pub success_rate: f64,
    pub average_latency: Duration,
    pub throughput: f64,
    pub error_count: usize,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
}

/// Detailed performance metrics for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedMetrics {
    /// Operation name
    pub operation: String,
    /// Timestamp when the operation started
    pub timestamp: u64,
    /// Duration of the operation
    pub duration: Duration,
    /// Data size involved (bytes)
    pub data_size: Option<usize>,
    /// CPU usage during operation (percentage)
    pub cpu_usage: Option<f64>,
    /// Memory usage during operation (bytes)
    pub memory_usage: Option<usize>,
    /// Network I/O during operation (bytes sent/received)
    pub network_io: Option<(usize, usize)>,
    /// Disk I/O during operation (bytes read/written)
    pub disk_io: Option<(usize, usize)>,
    /// Error information if operation failed
    pub error: Option<String>,
    /// Custom metadata for the operation
    pub metadata: HashMap<String, String>,
}

/// Aggregated statistics for a collection of metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedStats {
    /// Total number of operations
    pub count: usize,
    /// Mean duration
    pub mean_duration: Duration,
    /// Median duration
    pub median_duration: Duration,
    /// 95th percentile duration
    pub p95_duration: Duration,
    /// 99th percentile duration
    pub p99_duration: Duration,
    /// Standard deviation
    pub std_dev: Duration,
    /// Minimum duration
    pub min_duration: Duration,
    /// Maximum duration
    pub max_duration: Duration,
    /// Total throughput (operations per second)
    pub ops_per_second: f64,
    /// Total data throughput (MB/s)
    pub data_throughput_mbps: Option<f64>,
    /// Error rate (percentage)
    pub error_rate: f64,
    /// Success rate (percentage)
    pub success_rate: f64,
}

/// Real-time metrics collector
pub struct MetricsCollector {
    /// Storage for collected metrics
    metrics: Vec<DetailedMetrics>,
    /// Configuration for collection
    config: MetricsConfig,
    /// Start time of collection
    start_time: SystemTime,
}

/// Configuration for metrics collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Whether to collect system metrics (CPU, memory)
    pub collect_system_metrics: bool,
    /// Whether to collect network metrics
    pub collect_network_metrics: bool,
    /// Whether to collect disk I/O metrics
    pub collect_disk_metrics: bool,
    /// Sampling interval for system metrics (milliseconds)
    pub sampling_interval_ms: u64,
    /// Maximum number of metrics to store in memory
    pub max_metrics_in_memory: usize,
    /// Whether to write metrics to disk immediately
    pub write_to_disk: bool,
    /// Output directory for metrics files
    pub output_dir: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collect_system_metrics: true,
            collect_network_metrics: true,
            collect_disk_metrics: true,
            sampling_interval_ms: 100,
            max_metrics_in_memory: 10000,
            write_to_disk: true,
            output_dir: "/tmp/mooseng-metrics".to_string(),
        }
    }
}

impl MetricsCollector {
    pub fn new(config: MetricsConfig) -> Self {
        // Create output directory if it doesn't exist
        if config.write_to_disk {
            if let Err(e) = std::fs::create_dir_all(&config.output_dir) {
                warn!("Failed to create metrics output directory: {}", e);
            }
        }
        
        Self {
            metrics: Vec::new(),
            config,
            start_time: SystemTime::now(),
        }
    }
    
    /// Record a new metric
    pub fn record_metric(&mut self, mut metric: DetailedMetrics) {
        // Add system metrics if enabled
        if self.config.collect_system_metrics {
            metric.cpu_usage = self.get_cpu_usage();
            metric.memory_usage = self.get_memory_usage();
        }
        
        // Add network metrics if enabled
        if self.config.collect_network_metrics {
            metric.network_io = self.get_network_io();
        }
        
        // Add disk metrics if enabled
        if self.config.collect_disk_metrics {
            metric.disk_io = self.get_disk_io();
        }
        
        // Write to disk immediately if configured
        if self.config.write_to_disk {
            if let Err(e) = self.write_metric_to_disk(&metric) {
                warn!("Failed to write metric to disk: {}", e);
            }
        }
        
        self.metrics.push(metric);
        
        // Prune old metrics if we exceed the limit
        if self.metrics.len() > self.config.max_metrics_in_memory {
            let excess = self.metrics.len() - self.config.max_metrics_in_memory;
            self.metrics.drain(0..excess);
        }
    }
    
    /// Get current CPU usage percentage
    fn get_cpu_usage(&self) -> Option<f64> {
        // This would use a system metrics library like sysinfo
        // For now, simulate random CPU usage
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Some(rng.gen_range(10.0..80.0))
    }
    
    /// Get current memory usage in bytes
    fn get_memory_usage(&self) -> Option<usize> {
        // This would use a system metrics library
        // For now, simulate memory usage
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Some(rng.gen_range(100_000_000..1_000_000_000)) // 100MB to 1GB
    }
    
    /// Get network I/O (bytes sent, bytes received)
    fn get_network_io(&self) -> Option<(usize, usize)> {
        // This would read from /proc/net/dev or use a system library
        // For now, simulate network I/O
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Some((
            rng.gen_range(1000..100_000),
            rng.gen_range(1000..100_000),
        ))
    }
    
    /// Get disk I/O (bytes read, bytes written)
    fn get_disk_io(&self) -> Option<(usize, usize)> {
        // This would read from /proc/diskstats or use a system library
        // For now, simulate disk I/O
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Some((
            rng.gen_range(1000..1_000_000),
            rng.gen_range(1000..1_000_000),
        ))
    }
    
    /// Write a metric to disk
    fn write_metric_to_disk(&self, metric: &DetailedMetrics) -> Result<(), std::io::Error> {
        let timestamp = metric.timestamp;
        let filename = format!("{}/metrics_{}.jsonl", self.config.output_dir, timestamp / 1000);
        
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(filename)?;
        
        let json_line = serde_json::to_string(metric)?;
        writeln!(file, "{}", json_line)?;
        
        Ok(())
    }
    
    /// Get all metrics
    pub fn get_metrics(&self) -> &[DetailedMetrics] {
        &self.metrics
    }
    
    /// Clear all metrics from memory
    pub fn clear_metrics(&mut self) {
        self.metrics.clear();
    }
    
    /// Export metrics to various formats
    pub fn export_metrics(&self, format: ExportFormat) -> Result<String, Box<dyn std::error::Error>> {
        match format {
            ExportFormat::Json => self.export_json(),
            ExportFormat::Csv => self.export_csv(),
            ExportFormat::Prometheus => self.export_prometheus(),
            ExportFormat::InfluxDB => self.export_influxdb(),
        }
    }
    
    fn export_json(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(serde_json::to_string_pretty(&self.metrics)?)
    }
    
    fn export_csv(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut csv_output = String::new();
        csv_output.push_str("timestamp,operation,duration_ms,data_size,cpu_usage,memory_usage,error\n");
        
        for metric in &self.metrics {
            csv_output.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                metric.timestamp,
                metric.operation,
                metric.duration.as_millis(),
                metric.data_size.unwrap_or(0),
                metric.cpu_usage.unwrap_or(0.0),
                metric.memory_usage.unwrap_or(0),
                metric.error.as_deref().unwrap_or("")
            ));
        }
        
        Ok(csv_output)
    }
    
    fn export_prometheus(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut prom_output = String::new();
        
        // Operation duration histogram
        prom_output.push_str("# HELP mooseng_operation_duration_seconds Duration of operations in seconds\n");
        prom_output.push_str("# TYPE mooseng_operation_duration_seconds histogram\n");
        
        for metric in &self.metrics {
            prom_output.push_str(&format!(
                "mooseng_operation_duration_seconds{{operation=\"{}\"}} {}\n",
                metric.operation,
                metric.duration.as_secs_f64()
            ));
        }
        
        // CPU usage gauge
        prom_output.push_str("# HELP mooseng_cpu_usage_percent CPU usage percentage\n");
        prom_output.push_str("# TYPE mooseng_cpu_usage_percent gauge\n");
        
        for metric in &self.metrics {
            if let Some(cpu) = metric.cpu_usage {
                prom_output.push_str(&format!(
                    "mooseng_cpu_usage_percent{{operation=\"{}\"}} {}\n",
                    metric.operation, cpu
                ));
            }
        }
        
        Ok(prom_output)
    }
    
    fn export_influxdb(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut influx_output = String::new();
        
        for metric in &self.metrics {
            // InfluxDB line protocol format
            influx_output.push_str(&format!(
                "mooseng_metrics,operation={} duration={},cpu_usage={},memory_usage={} {}\n",
                metric.operation,
                metric.duration.as_nanos(),
                metric.cpu_usage.unwrap_or(0.0),
                metric.memory_usage.unwrap_or(0),
                metric.timestamp * 1_000_000 // Convert to nanoseconds
            ));
        }
        
        Ok(influx_output)
    }
    
    /// Generate aggregated statistics
    pub fn calculate_stats(&self) -> HashMap<String, AggregatedStats> {
        let mut stats_by_operation: HashMap<String, Vec<&DetailedMetrics>> = HashMap::new();
        
        // Group metrics by operation
        for metric in &self.metrics {
            stats_by_operation
                .entry(metric.operation.clone())
                .or_insert_with(Vec::new)
                .push(metric);
        }
        
        // Calculate stats for each operation
        let mut result = HashMap::new();
        for (operation, metrics) in stats_by_operation {
            let stats = self.calculate_operation_stats(&metrics);
            result.insert(operation, stats);
        }
        
        result
    }
    
    fn calculate_operation_stats(&self, metrics: &[&DetailedMetrics]) -> AggregatedStats {
        if metrics.is_empty() {
            return AggregatedStats {
                count: 0,
                mean_duration: Duration::from_secs(0),
                median_duration: Duration::from_secs(0),
                p95_duration: Duration::from_secs(0),
                p99_duration: Duration::from_secs(0),
                std_dev: Duration::from_secs(0),
                min_duration: Duration::from_secs(0),
                max_duration: Duration::from_secs(0),
                ops_per_second: 0.0,
                data_throughput_mbps: None,
                error_rate: 0.0,
                success_rate: 100.0,
            };
        }
        
        let mut durations: Vec<Duration> = metrics.iter().map(|m| m.duration).collect();
        durations.sort();
        
        let count = metrics.len();
        let total_duration: Duration = durations.iter().sum();
        let mean_duration = total_duration / count as u32;
        
        let median_duration = durations[count / 2];
        let p95_duration = durations[(count as f64 * 0.95) as usize];
        let p99_duration = durations[(count as f64 * 0.99) as usize];
        
        let min_duration = durations.first().cloned().unwrap_or_default();
        let max_duration = durations.last().cloned().unwrap_or_default();
        
        // Calculate standard deviation
        let mean_ns = mean_duration.as_nanos() as f64;
        let variance = durations
            .iter()
            .map(|d| {
                let diff = d.as_nanos() as f64 - mean_ns;
                diff * diff
            })
            .sum::<f64>() / count as f64;
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);
        
        // Calculate throughput
        let total_time = self.start_time.elapsed().unwrap_or_default();
        let ops_per_second = if total_time.as_secs_f64() > 0.0 {
            count as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };
        
        // Calculate data throughput
        let total_data_size: usize = metrics
            .iter()
            .filter_map(|m| m.data_size)
            .sum();
        let data_throughput_mbps = if total_time.as_secs_f64() > 0.0 && total_data_size > 0 {
            let mb_per_second = (total_data_size as f64 / (1024.0 * 1024.0)) / total_time.as_secs_f64();
            Some(mb_per_second)
        } else {
            None
        };
        
        // Calculate error rates
        let error_count = metrics.iter().filter(|m| m.error.is_some()).count();
        let error_rate = (error_count as f64 / count as f64) * 100.0;
        let success_rate = 100.0 - error_rate;
        
        AggregatedStats {
            count,
            mean_duration,
            median_duration,
            p95_duration,
            p99_duration,
            std_dev,
            min_duration,
            max_duration,
            ops_per_second,
            data_throughput_mbps,
            error_rate,
            success_rate,
        }
    }
}

/// Export format for metrics
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Prometheus,
    InfluxDB,
}

/// Performance analyzer for benchmark results
pub struct PerformanceAnalyzer {
    baseline_results: Option<Vec<BenchmarkResult>>,
    current_results: Vec<BenchmarkResult>,
}

impl PerformanceAnalyzer {
    pub fn new() -> Self {
        Self {
            baseline_results: None,
            current_results: Vec::new(),
        }
    }
    
    /// Set baseline results for comparison
    pub fn set_baseline(&mut self, results: Vec<BenchmarkResult>) {
        self.baseline_results = Some(results);
    }
    
    /// Add current results
    pub fn add_results(&mut self, results: Vec<BenchmarkResult>) {
        self.current_results.extend(results);
    }
    
    /// Generate performance comparison report
    pub fn generate_comparison_report(&self) -> PerformanceReport {
        let mut report = PerformanceReport {
            summary: "Performance Analysis Report".to_string(),
            improvements: Vec::new(),
            regressions: Vec::new(),
            recommendations: Vec::new(),
            detailed_comparisons: Vec::new(),
        };
        
        if let Some(baseline) = &self.baseline_results {
            for current in &self.current_results {
                if let Some(baseline_result) = baseline.iter().find(|b| b.operation == current.operation) {
                    let comparison = self.compare_results(baseline_result, current);
                    report.detailed_comparisons.push(comparison.clone());
                    
                    // Categorize as improvement or regression
                    if comparison.performance_change > 5.0 {
                        report.improvements.push(format!(
                            "{}: {:.1}% improvement in {}", 
                            current.operation, 
                            comparison.performance_change,
                            comparison.primary_metric
                        ));
                    } else if comparison.performance_change < -5.0 {
                        report.regressions.push(format!(
                            "{}: {:.1}% regression in {}", 
                            current.operation, 
                            -comparison.performance_change,
                            comparison.primary_metric
                        ));
                    }
                }
            }
            
            // Generate recommendations
            report.recommendations = self.generate_recommendations(&report);
        }
        
        report
    }
    
    fn compare_results(&self, baseline: &BenchmarkResult, current: &BenchmarkResult) -> ResultComparison {
        let baseline_time = baseline.mean_time.as_nanos() as f64;
        let current_time = current.mean_time.as_nanos() as f64;
        
        let time_change = if baseline_time > 0.0 {
            ((baseline_time - current_time) / baseline_time) * 100.0
        } else {
            0.0
        };
        
        let throughput_change = match (baseline.throughput, current.throughput) {
            (Some(b), Some(c)) => ((c - b) / b) * 100.0,
            _ => 0.0,
        };
        
        // Use throughput change if available, otherwise use time change
        let performance_change = if throughput_change != 0.0 {
            throughput_change
        } else {
            time_change
        };
        
        let primary_metric = if throughput_change != 0.0 {
            "throughput".to_string()
        } else {
            "latency".to_string()
        };
        
        ResultComparison {
            operation: current.operation.clone(),
            baseline_time: baseline.mean_time,
            current_time: current.mean_time,
            time_change,
            baseline_throughput: baseline.throughput,
            current_throughput: current.throughput,
            throughput_change,
            performance_change,
            primary_metric,
        }
    }
    
    fn generate_recommendations(&self, report: &PerformanceReport) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if report.regressions.len() > report.improvements.len() {
            recommendations.push("Consider profiling the regressed operations to identify bottlenecks".to_string());
        }
        
        if report.detailed_comparisons.iter().any(|c| c.throughput_change < -10.0) {
            recommendations.push("Investigate network or I/O performance issues".to_string());
        }
        
        if report.detailed_comparisons.iter().any(|c| c.time_change < -20.0) {
            recommendations.push("Check for resource contention or configuration changes".to_string());
        }
        
        recommendations
    }
}

/// Performance analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub summary: String,
    pub improvements: Vec<String>,
    pub regressions: Vec<String>,
    pub recommendations: Vec<String>,
    pub detailed_comparisons: Vec<ResultComparison>,
}

/// Comparison between baseline and current results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultComparison {
    pub operation: String,
    pub baseline_time: Duration,
    pub current_time: Duration,
    pub time_change: f64, // Percentage change
    pub baseline_throughput: Option<f64>,
    pub current_throughput: Option<f64>,
    pub throughput_change: f64, // Percentage change
    pub performance_change: f64, // Overall performance change
    pub primary_metric: String, // Which metric is primary for this comparison
}

/// Create a simple metric for recording
pub fn create_metric(operation: &str, duration: Duration) -> DetailedMetrics {
    DetailedMetrics {
        operation: operation.to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        duration,
        data_size: None,
        cpu_usage: None,
        memory_usage: None,
        network_io: None,
        disk_io: None,
        error: None,
        metadata: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_collector_creation() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);
        assert_eq!(collector.metrics.len(), 0);
    }
    
    #[test]
    fn test_create_metric() {
        let metric = create_metric("test_operation", Duration::from_millis(100));
        assert_eq!(metric.operation, "test_operation");
        assert_eq!(metric.duration, Duration::from_millis(100));
    }
    
    #[test]
    fn test_performance_analyzer() {
        let analyzer = PerformanceAnalyzer::new();
        assert!(analyzer.baseline_results.is_none());
        assert_eq!(analyzer.current_results.len(), 0);
    }
}