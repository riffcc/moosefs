//! Unified Benchmark Suite for MooseNG
//!
//! This module provides a unified interface for running and managing all MooseNG benchmarks.
//! It consolidates the functionality from mooseng-benchmarks into a cohesive system that
//! integrates with the main MooseNG codebase.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use anyhow::Result;

pub mod runner;
pub mod collector;
pub mod results;

/// Unified benchmark configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBenchmarkConfig {
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
    /// Enable live monitoring
    pub live_monitoring: bool,
    /// Database storage enabled
    pub store_results: bool,
}

impl Default for UnifiedBenchmarkConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 10,
            measurement_iterations: 100,
            file_sizes: vec![1024, 4096, 65536, 1048576, 10485760],
            concurrency_levels: vec![1, 10, 50, 100],
            regions: vec!["us-east".to_string(), "eu-west".to_string(), "ap-south".to_string()],
            detailed_report: true,
            live_monitoring: false,
            store_results: true,
        }
    }
}

/// Standardized benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBenchmarkResult {
    /// Unique identifier for this result
    pub id: String,
    /// Benchmark name
    pub benchmark_name: String,
    /// Operation being tested
    pub operation: String,
    /// Test suite category
    pub category: BenchmarkCategory,
    /// Mean execution time
    pub mean_time: Duration,
    /// Standard deviation
    pub std_dev: Duration,
    /// Minimum time observed
    pub min_time: Duration,
    /// Maximum time observed
    pub max_time: Duration,
    /// Throughput in operations per second
    pub throughput: Option<f64>,
    /// Number of samples collected
    pub samples: usize,
    /// Timestamp of the benchmark run
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Percentile measurements
    pub percentiles: Option<PercentileData>,
}

/// Percentile data for detailed analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercentileData {
    pub p50: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p999: Duration,
}

/// Benchmark categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BenchmarkCategory {
    FileOperations,
    MetadataOperations,
    NetworkOperations,
    MultiRegion,
    Comparison,
    Integration,
}

impl std::fmt::Display for BenchmarkCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchmarkCategory::FileOperations => write!(f, "file_operations"),
            BenchmarkCategory::MetadataOperations => write!(f, "metadata_operations"),
            BenchmarkCategory::NetworkOperations => write!(f, "network_operations"),
            BenchmarkCategory::MultiRegion => write!(f, "multiregion"),
            BenchmarkCategory::Comparison => write!(f, "comparison"),
            BenchmarkCategory::Integration => write!(f, "integration"),
        }
    }
}

/// Trait for unified benchmark execution
pub trait UnifiedBenchmark: Send + Sync {
    /// Get the benchmark name
    fn name(&self) -> &str;
    
    /// Get the benchmark description
    fn description(&self) -> &str;
    
    /// Get the benchmark category
    fn category(&self) -> BenchmarkCategory;
    
    /// Run the benchmark with the given configuration
    async fn run(&self, config: &UnifiedBenchmarkConfig) -> Result<Vec<UnifiedBenchmarkResult>>;
    
    /// Validate that the benchmark can run in the current environment
    async fn validate_environment(&self) -> Result<()> {
        Ok(())
    }
    
    /// Get estimated runtime in seconds
    fn estimated_runtime(&self, config: &UnifiedBenchmarkConfig) -> Duration {
        Duration::from_secs(
            (config.warmup_iterations + config.measurement_iterations) as u64 
            * config.file_sizes.len() as u64 
            * config.concurrency_levels.len() as u64
        )
    }
}

/// Progress update for live monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    /// Benchmark name
    pub benchmark_name: String,
    /// Current step
    pub current_step: String,
    /// Progress percentage (0.0 to 1.0)
    pub progress: f64,
    /// Estimated time remaining
    pub estimated_remaining: Option<Duration>,
    /// Current result (if available)
    pub current_result: Option<UnifiedBenchmarkResult>,
}

/// Live monitoring channel type
pub type ProgressSender = mpsc::UnboundedSender<BenchmarkProgress>;
pub type ProgressReceiver = mpsc::UnboundedReceiver<BenchmarkProgress>;

/// Session information for benchmark runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSession {
    /// Unique session identifier
    pub id: String,
    /// Session name
    pub name: String,
    /// Session description
    pub description: Option<String>,
    /// Configuration used
    pub config: UnifiedBenchmarkConfig,
    /// Start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// End time (if completed)
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Session status
    pub status: SessionStatus,
    /// Results collected
    pub results: Vec<UnifiedBenchmarkResult>,
}

/// Session execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Created,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Created => write!(f, "created"),
            SessionStatus::Running => write!(f, "running"),
            SessionStatus::Completed => write!(f, "completed"),
            SessionStatus::Failed => write!(f, "failed"),
            SessionStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Statistics and analysis for benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStatistics {
    /// Total number of results
    pub total_results: usize,
    /// Average latency across all operations
    pub average_latency: Duration,
    /// Average throughput
    pub average_throughput: Option<f64>,
    /// Peak throughput observed
    pub peak_throughput: Option<f64>,
    /// Results by category
    pub category_stats: HashMap<BenchmarkCategory, CategoryStats>,
    /// Performance grade
    pub performance_grade: PerformanceGrade,
}

/// Statistics for a specific category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub count: usize,
    pub avg_latency: Duration,
    pub avg_throughput: Option<f64>,
    pub fastest_operation: Option<String>,
    pub slowest_operation: Option<String>,
}

/// Performance grading system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceGrade {
    Excellent,  // A+
    VeryGood,   // A
    Good,       // B+
    Fair,       // B
    BelowAverage, // C
    NeedsImprovement, // D
}

impl std::fmt::Display for PerformanceGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PerformanceGrade::Excellent => write!(f, "A+ (Excellent)"),
            PerformanceGrade::VeryGood => write!(f, "A (Very Good)"),
            PerformanceGrade::Good => write!(f, "B+ (Good)"),
            PerformanceGrade::Fair => write!(f, "B (Fair)"),
            PerformanceGrade::BelowAverage => write!(f, "C (Below Average)"),
            PerformanceGrade::NeedsImprovement => write!(f, "D (Needs Improvement)"),
        }
    }
}

/// Calculate performance grade based on metrics
pub fn calculate_performance_grade(avg_latency_ms: f64, avg_throughput_mbps: Option<f64>) -> PerformanceGrade {
    let mut score = 100.0;
    
    // Latency scoring (lower is better)
    if avg_latency_ms > 100.0 {
        score -= 30.0;
    } else if avg_latency_ms > 50.0 {
        score -= 20.0;
    } else if avg_latency_ms > 20.0 {
        score -= 10.0;
    } else if avg_latency_ms < 5.0 {
        score += 10.0;
    }
    
    // Throughput scoring (higher is better)
    if let Some(throughput) = avg_throughput_mbps {
        if throughput > 1000.0 {
            score += 15.0;
        } else if throughput > 500.0 {
            score += 10.0;
        } else if throughput > 100.0 {
            score += 5.0;
        } else if throughput < 10.0 {
            score -= 25.0;
        } else if throughput < 50.0 {
            score -= 15.0;
        }
    }
    
    match score as i32 {
        90..=150 => PerformanceGrade::Excellent,
        80..=89 => PerformanceGrade::VeryGood,
        70..=79 => PerformanceGrade::Good,
        60..=69 => PerformanceGrade::Fair,
        50..=59 => PerformanceGrade::BelowAverage,
        _ => PerformanceGrade::NeedsImprovement,
    }
}

/// Calculate comprehensive statistics from benchmark results
pub fn calculate_statistics(results: &[UnifiedBenchmarkResult]) -> BenchmarkStatistics {
    if results.is_empty() {
        return BenchmarkStatistics {
            total_results: 0,
            average_latency: Duration::from_millis(0),
            average_throughput: None,
            peak_throughput: None,
            category_stats: HashMap::new(),
            performance_grade: PerformanceGrade::NeedsImprovement,
        };
    }
    
    let total_results = results.len();
    
    // Calculate overall averages
    let total_latency_ms: f64 = results.iter()
        .map(|r| r.mean_time.as_secs_f64() * 1000.0)
        .sum();
    let average_latency = Duration::from_secs_f64(total_latency_ms / total_results as f64 / 1000.0);
    
    let throughput_values: Vec<f64> = results.iter()
        .filter_map(|r| r.throughput)
        .collect();
    
    let average_throughput = if !throughput_values.is_empty() {
        Some(throughput_values.iter().sum::<f64>() / throughput_values.len() as f64)
    } else {
        None
    };
    
    let peak_throughput = throughput_values.iter().copied().fold(None, |acc, x| {
        Some(acc.map_or(x, |a| a.max(x)))
    });
    
    // Calculate category statistics
    let mut category_stats = HashMap::new();
    
    for category in [
        BenchmarkCategory::FileOperations,
        BenchmarkCategory::MetadataOperations,
        BenchmarkCategory::NetworkOperations,
        BenchmarkCategory::MultiRegion,
        BenchmarkCategory::Comparison,
        BenchmarkCategory::Integration,
    ] {
        let category_results: Vec<_> = results.iter()
            .filter(|r| r.category == category)
            .collect();
        
        if !category_results.is_empty() {
            let count = category_results.len();
            let avg_latency_ms = category_results.iter()
                .map(|r| r.mean_time.as_secs_f64() * 1000.0)
                .sum::<f64>() / count as f64;
            let avg_latency = Duration::from_secs_f64(avg_latency_ms / 1000.0);
            
            let category_throughputs: Vec<f64> = category_results.iter()
                .filter_map(|r| r.throughput)
                .collect();
            let avg_throughput = if !category_throughputs.is_empty() {
                Some(category_throughputs.iter().sum::<f64>() / category_throughputs.len() as f64)
            } else {
                None
            };
            
            let fastest_operation = category_results.iter()
                .min_by_key(|r| r.mean_time)
                .map(|r| r.operation.clone());
            
            let slowest_operation = category_results.iter()
                .max_by_key(|r| r.mean_time)
                .map(|r| r.operation.clone());
            
            category_stats.insert(category, CategoryStats {
                count,
                avg_latency,
                avg_throughput,
                fastest_operation,
                slowest_operation,
            });
        }
    }
    
    let performance_grade = calculate_performance_grade(
        average_latency.as_secs_f64() * 1000.0,
        average_throughput,
    );
    
    BenchmarkStatistics {
        total_results,
        average_latency,
        average_throughput,
        peak_throughput,
        category_stats,
        performance_grade,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    
    #[test]
    fn test_default_config() {
        let config = UnifiedBenchmarkConfig::default();
        assert!(config.warmup_iterations > 0);
        assert!(config.measurement_iterations > 0);
        assert!(!config.file_sizes.is_empty());
        assert!(!config.concurrency_levels.is_empty());
    }
    
    #[test]
    fn test_performance_grade_calculation() {
        // Test excellent performance
        let grade = calculate_performance_grade(5.0, Some(1500.0));
        assert_eq!(grade, PerformanceGrade::Excellent);
        
        // Test poor performance
        let grade = calculate_performance_grade(150.0, Some(5.0));
        assert_eq!(grade, PerformanceGrade::NeedsImprovement);
        
        // Test average performance
        let grade = calculate_performance_grade(30.0, Some(200.0));
        assert_eq!(grade, PerformanceGrade::Good);
    }
    
    #[test]
    fn test_statistics_calculation() {
        let results = vec![
            UnifiedBenchmarkResult {
                id: Uuid::new_v4().to_string(),
                benchmark_name: "test1".to_string(),
                operation: "read".to_string(),
                category: BenchmarkCategory::FileOperations,
                mean_time: Duration::from_millis(10),
                std_dev: Duration::from_millis(1),
                min_time: Duration::from_millis(8),
                max_time: Duration::from_millis(12),
                throughput: Some(100.0),
                samples: 50,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                percentiles: None,
            },
            UnifiedBenchmarkResult {
                id: Uuid::new_v4().to_string(),
                benchmark_name: "test2".to_string(),
                operation: "write".to_string(),
                category: BenchmarkCategory::FileOperations,
                mean_time: Duration::from_millis(20),
                std_dev: Duration::from_millis(2),
                min_time: Duration::from_millis(18),
                max_time: Duration::from_millis(22),
                throughput: Some(200.0),
                samples: 50,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                percentiles: None,
            },
        ];
        
        let stats = calculate_statistics(&results);
        assert_eq!(stats.total_results, 2);
        assert_eq!(stats.average_latency, Duration::from_millis(15));
        assert_eq!(stats.average_throughput, Some(150.0));
        assert_eq!(stats.peak_throughput, Some(200.0));
        
        let file_ops_stats = stats.category_stats.get(&BenchmarkCategory::FileOperations).unwrap();
        assert_eq!(file_ops_stats.count, 2);
        assert_eq!(file_ops_stats.fastest_operation, Some("read".to_string()));
        assert_eq!(file_ops_stats.slowest_operation, Some("write".to_string()));
    }
}