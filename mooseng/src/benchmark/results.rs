//! Benchmark Results Collection and Analysis
//!
//! This module provides functionality for collecting, analyzing, and comparing
//! benchmark results with statistical analysis and trend detection.

use super::{UnifiedBenchmarkResult, BenchmarkCategory, PerformanceGrade, calculate_performance_grade};
use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Comprehensive analysis of benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkAnalysis {
    /// Summary statistics
    pub summary: ResultSummary,
    /// Detailed breakdown by operation
    pub operation_breakdown: HashMap<String, OperationStats>,
    /// Category-based analysis
    pub category_analysis: HashMap<BenchmarkCategory, CategoryAnalysis>,
    /// Performance trends
    pub trends: Vec<PerformanceTrend>,
    /// Outlier detection results
    pub outliers: Vec<OutlierResult>,
    /// Recommendations for improvement
    pub recommendations: Vec<String>,
}

/// Summary statistics for a set of results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultSummary {
    /// Total number of benchmark results
    pub total_results: usize,
    /// Total number of samples across all results
    pub total_samples: usize,
    /// Overall average latency
    pub average_latency: Duration,
    /// Median latency
    pub median_latency: Duration,
    /// 95th percentile latency
    pub p95_latency: Duration,
    /// 99th percentile latency
    pub p99_latency: Duration,
    /// Average throughput (if available)
    pub average_throughput: Option<f64>,
    /// Peak throughput observed
    pub peak_throughput: Option<f64>,
    /// Performance grade
    pub performance_grade: PerformanceGrade,
    /// Confidence interval for average latency
    pub latency_confidence_interval: (Duration, Duration),
}

/// Statistics for a specific operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    /// Operation name
    pub operation: String,
    /// Number of measurements
    pub count: usize,
    /// Average latency
    pub avg_latency: Duration,
    /// Standard deviation
    pub std_dev: Duration,
    /// Minimum latency
    pub min_latency: Duration,
    /// Maximum latency
    pub max_latency: Duration,
    /// Coefficient of variation (std_dev / mean)
    pub coefficient_of_variation: f64,
    /// Average throughput
    pub avg_throughput: Option<f64>,
    /// Samples per measurement
    pub avg_samples: f64,
}

/// Analysis for a benchmark category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryAnalysis {
    /// Category name
    pub category: BenchmarkCategory,
    /// Number of different operations in this category
    pub operation_count: usize,
    /// Total measurements
    pub total_measurements: usize,
    /// Category average latency
    pub avg_latency: Duration,
    /// Best performing operation
    pub best_operation: Option<String>,
    /// Worst performing operation
    pub worst_operation: Option<String>,
    /// Consistency score (lower variation = higher score)
    pub consistency_score: f64,
    /// Category-specific grade
    pub category_grade: PerformanceGrade,
}

/// Performance trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrend {
    /// Operation or category being analyzed
    pub subject: String,
    /// Trend direction
    pub direction: TrendDirection,
    /// Magnitude of change (percentage)
    pub magnitude: f64,
    /// Confidence in the trend analysis
    pub confidence: f64,
    /// Time period analyzed
    pub time_span: Duration,
    /// Data points used in analysis
    pub data_points: usize,
}

/// Direction of performance trend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Degrading,
    Stable,
    Volatile,
    Insufficient,
}

impl std::fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrendDirection::Improving => write!(f, "Improving"),
            TrendDirection::Degrading => write!(f, "Degrading"),
            TrendDirection::Stable => write!(f, "Stable"),
            TrendDirection::Volatile => write!(f, "Volatile"),
            TrendDirection::Insufficient => write!(f, "Insufficient Data"),
        }
    }
}

/// Outlier detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierResult {
    /// Result ID that is an outlier
    pub result_id: String,
    /// Operation name
    pub operation: String,
    /// Benchmark name
    pub benchmark_name: String,
    /// How many standard deviations from the mean
    pub z_score: f64,
    /// The outlier value
    pub value: Duration,
    /// Expected range
    pub expected_range: (Duration, Duration),
    /// Possible explanation
    pub explanation: String,
}

/// Comparison between two sets of results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    /// Baseline analysis
    pub baseline: BenchmarkAnalysis,
    /// Current analysis
    pub current: BenchmarkAnalysis,
    /// Operation-level comparisons
    pub operation_comparisons: HashMap<String, OperationComparison>,
    /// Overall performance change
    pub overall_change: PerformanceChange,
    /// Significant regressions detected
    pub regressions: Vec<RegressionAlert>,
    /// Significant improvements detected
    pub improvements: Vec<ImprovementAlert>,
}

/// Comparison for a specific operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationComparison {
    /// Operation name
    pub operation: String,
    /// Latency change (percentage)
    pub latency_change: f64,
    /// Throughput change (percentage)
    pub throughput_change: Option<f64>,
    /// Statistical significance
    pub significance: StatisticalSignificance,
    /// Change classification
    pub change_type: ChangeType,
}

/// Overall performance change summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceChange {
    /// Overall latency change (percentage)
    pub latency_change: f64,
    /// Overall throughput change (percentage)
    pub throughput_change: Option<f64>,
    /// Performance grade change
    pub grade_change: GradeChange,
    /// Summary description
    pub summary: String,
}

/// Change in performance grade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeChange {
    /// Previous grade
    pub from: PerformanceGrade,
    /// Current grade
    pub to: PerformanceGrade,
    /// Direction of change
    pub direction: GradeDirection,
}

/// Direction of grade change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradeDirection {
    Improved,
    Degraded,
    Unchanged,
}

/// Statistical significance level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatisticalSignificance {
    High,      // p < 0.01
    Medium,    // p < 0.05
    Low,       // p < 0.10
    None,      // p >= 0.10
}

/// Type of performance change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    SignificantImprovement,
    MinorImprovement,
    NoChange,
    MinorRegression,
    SignificantRegression,
}

/// Performance regression alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAlert {
    /// Operation that regressed
    pub operation: String,
    /// Magnitude of regression (percentage)
    pub regression_percent: f64,
    /// Statistical significance
    pub significance: StatisticalSignificance,
    /// Severity level
    pub severity: AlertSeverity,
    /// Description
    pub description: String,
}

/// Performance improvement alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementAlert {
    /// Operation that improved
    pub operation: String,
    /// Magnitude of improvement (percentage)
    pub improvement_percent: f64,
    /// Statistical significance
    pub significance: StatisticalSignificance,
    /// Description
    pub description: String,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,  // > 50% regression
    High,      // > 25% regression
    Medium,    // > 10% regression
    Low,       // > 5% regression
}

/// Results analyzer for comprehensive analysis
pub struct ResultsAnalyzer;

impl ResultsAnalyzer {
    /// Create a new results analyzer
    pub fn new() -> Self {
        Self
    }
    
    /// Analyze a collection of benchmark results
    pub fn analyze(&self, results: &[UnifiedBenchmarkResult]) -> BenchmarkAnalysis {
        if results.is_empty() {
            return self.empty_analysis();
        }
        
        let summary = self.calculate_summary(results);
        let operation_breakdown = self.analyze_operations(results);
        let category_analysis = self.analyze_categories(results);
        let trends = self.analyze_trends(results);
        let outliers = self.detect_outliers(results);
        let recommendations = self.generate_recommendations(&summary, &operation_breakdown, &outliers);
        
        BenchmarkAnalysis {
            summary,
            operation_breakdown,
            category_analysis,
            trends,
            outliers,
            recommendations,
        }
    }
    
    /// Compare two sets of results
    pub fn compare(
        &self,
        baseline: &[UnifiedBenchmarkResult],
        current: &[UnifiedBenchmarkResult],
    ) -> BenchmarkComparison {
        let baseline_analysis = self.analyze(baseline);
        let current_analysis = self.analyze(current);
        
        let operation_comparisons = self.compare_operations(baseline, current);
        let overall_change = self.calculate_overall_change(&baseline_analysis, &current_analysis);
        let regressions = self.detect_regressions(&operation_comparisons);
        let improvements = self.detect_improvements(&operation_comparisons);
        
        BenchmarkComparison {
            baseline: baseline_analysis,
            current: current_analysis,
            operation_comparisons,
            overall_change,
            regressions,
            improvements,
        }
    }
    
    /// Calculate summary statistics
    fn calculate_summary(&self, results: &[UnifiedBenchmarkResult]) -> ResultSummary {
        let total_results = results.len();
        let total_samples = results.iter().map(|r| r.samples).sum();
        
        // Calculate latency statistics
        let mut latencies: Vec<f64> = results.iter()
            .map(|r| r.mean_time.as_secs_f64() * 1000.0)
            .collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let average_latency_ms = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let average_latency = Duration::from_secs_f64(average_latency_ms / 1000.0);
        
        let median_latency = Duration::from_secs_f64(
            self.percentile(&latencies, 50.0) / 1000.0
        );
        let p95_latency = Duration::from_secs_f64(
            self.percentile(&latencies, 95.0) / 1000.0
        );
        let p99_latency = Duration::from_secs_f64(
            self.percentile(&latencies, 99.0) / 1000.0
        );
        
        // Calculate confidence interval (95%)
        let std_dev = self.standard_deviation(&latencies);
        let margin_of_error = 1.96 * std_dev / (latencies.len() as f64).sqrt();
        let latency_confidence_interval = (
            Duration::from_secs_f64((average_latency_ms - margin_of_error) / 1000.0),
            Duration::from_secs_f64((average_latency_ms + margin_of_error) / 1000.0),
        );
        
        // Calculate throughput statistics
        let throughput_values: Vec<f64> = results.iter()
            .filter_map(|r| r.throughput)
            .collect();
        
        let average_throughput = if !throughput_values.is_empty() {
            Some(throughput_values.iter().sum::<f64>() / throughput_values.len() as f64)
        } else {
            None
        };
        
        let peak_throughput = throughput_values.iter()
            .copied()
            .fold(None, |acc, x| Some(acc.map_or(x, |a| a.max(x))));
        
        let performance_grade = calculate_performance_grade(average_latency_ms, average_throughput);
        
        ResultSummary {
            total_results,
            total_samples,
            average_latency,
            median_latency,
            p95_latency,
            p99_latency,
            average_throughput,
            peak_throughput,
            performance_grade,
            latency_confidence_interval,
        }
    }
    
    /// Analyze results by operation
    fn analyze_operations(&self, results: &[UnifiedBenchmarkResult]) -> HashMap<String, OperationStats> {
        let mut operation_groups: HashMap<String, Vec<&UnifiedBenchmarkResult>> = HashMap::new();
        
        for result in results {
            operation_groups
                .entry(result.operation.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }
        
        let mut operation_breakdown = HashMap::new();
        
        for (operation, op_results) in operation_groups {
            let count = op_results.len();
            
            let latencies: Vec<f64> = op_results.iter()
                .map(|r| r.mean_time.as_secs_f64())
                .collect();
            
            let avg_latency_secs = latencies.iter().sum::<f64>() / latencies.len() as f64;
            let avg_latency = Duration::from_secs_f64(avg_latency_secs);
            
            let std_dev_secs = self.standard_deviation(&latencies);
            let std_dev = Duration::from_secs_f64(std_dev_secs);
            
            let min_latency = Duration::from_secs_f64(
                latencies.iter().copied().fold(f64::INFINITY, f64::min)
            );
            let max_latency = Duration::from_secs_f64(
                latencies.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            );
            
            let coefficient_of_variation = if avg_latency_secs > 0.0 {
                std_dev_secs / avg_latency_secs
            } else {
                0.0
            };
            
            let throughput_values: Vec<f64> = op_results.iter()
                .filter_map(|r| r.throughput)
                .collect();
            
            let avg_throughput = if !throughput_values.is_empty() {
                Some(throughput_values.iter().sum::<f64>() / throughput_values.len() as f64)
            } else {
                None
            };
            
            let avg_samples = op_results.iter().map(|r| r.samples as f64).sum::<f64>() / count as f64;
            
            operation_breakdown.insert(operation.clone(), OperationStats {
                operation,
                count,
                avg_latency,
                std_dev,
                min_latency,
                max_latency,
                coefficient_of_variation,
                avg_throughput,
                avg_samples,
            });
        }
        
        operation_breakdown
    }
    
    /// Analyze results by category
    fn analyze_categories(&self, results: &[UnifiedBenchmarkResult]) -> HashMap<BenchmarkCategory, CategoryAnalysis> {
        let mut category_groups: HashMap<BenchmarkCategory, Vec<&UnifiedBenchmarkResult>> = HashMap::new();
        
        for result in results {
            category_groups
                .entry(result.category)
                .or_insert_with(Vec::new)
                .push(result);
        }
        
        let mut category_analysis = HashMap::new();
        
        for (category, cat_results) in category_groups {
            let operation_count = cat_results.iter()
                .map(|r| r.operation.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len();
            
            let total_measurements = cat_results.len();
            
            let avg_latency_ms = cat_results.iter()
                .map(|r| r.mean_time.as_secs_f64() * 1000.0)
                .sum::<f64>() / total_measurements as f64;
            let avg_latency = Duration::from_secs_f64(avg_latency_ms / 1000.0);
            
            // Find best and worst operations
            let mut operation_avgs: HashMap<String, f64> = HashMap::new();
            for result in &cat_results {
                let entry = operation_avgs.entry(result.operation.clone()).or_insert(0.0);
                *entry += result.mean_time.as_secs_f64();
            }
            
            for (op, total) in &mut operation_avgs {
                let count = cat_results.iter().filter(|r| r.operation == *op).count();
                *total /= count as f64;
            }
            
            let best_operation = operation_avgs.iter()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(op, _)| op.clone());
            
            let worst_operation = operation_avgs.iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(op, _)| op.clone());
            
            // Calculate consistency score (0-100, higher is better)
            let latencies: Vec<f64> = cat_results.iter()
                .map(|r| r.mean_time.as_secs_f64())
                .collect();
            let coefficient_of_variation = if avg_latency_ms > 0.0 {
                self.standard_deviation(&latencies) / (avg_latency_ms / 1000.0)
            } else {
                0.0
            };
            let consistency_score = (1.0 - coefficient_of_variation.min(1.0)) * 100.0;
            
            let category_grade = calculate_performance_grade(avg_latency_ms, None);
            
            category_analysis.insert(category, CategoryAnalysis {
                category,
                operation_count,
                total_measurements,
                avg_latency,
                best_operation,
                worst_operation,
                consistency_score,
                category_grade,
            });
        }
        
        category_analysis
    }
    
    /// Analyze performance trends over time
    fn analyze_trends(&self, results: &[UnifiedBenchmarkResult]) -> Vec<PerformanceTrend> {
        // Group results by operation and sort by timestamp
        let mut operation_groups: HashMap<String, Vec<&UnifiedBenchmarkResult>> = HashMap::new();
        
        for result in results {
            operation_groups
                .entry(result.operation.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }
        
        let mut trends = Vec::new();
        
        for (operation, mut op_results) in operation_groups {
            if op_results.len() < 3 {
                trends.push(PerformanceTrend {
                    subject: operation,
                    direction: TrendDirection::Insufficient,
                    magnitude: 0.0,
                    confidence: 0.0,
                    time_span: Duration::from_secs(0),
                    data_points: op_results.len(),
                });
                continue;
            }
            
            // Sort by timestamp
            op_results.sort_by_key(|r| r.timestamp);
            
            let time_span = (op_results.last().unwrap().timestamp - op_results.first().unwrap().timestamp)
                .to_std().unwrap_or(Duration::from_secs(0));
            
            // Simple linear regression for trend analysis
            let (direction, magnitude, confidence) = self.calculate_trend(&op_results);
            
            trends.push(PerformanceTrend {
                subject: operation,
                direction,
                magnitude,
                confidence,
                time_span,
                data_points: op_results.len(),
            });
        }
        
        trends
    }
    
    /// Detect statistical outliers
    fn detect_outliers(&self, results: &[UnifiedBenchmarkResult]) -> Vec<OutlierResult> {
        let mut outliers = Vec::new();
        
        // Group by operation for outlier detection
        let mut operation_groups: HashMap<String, Vec<&UnifiedBenchmarkResult>> = HashMap::new();
        
        for result in results {
            operation_groups
                .entry(result.operation.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }
        
        for (operation, op_results) in operation_groups {
            if op_results.len() < 3 {
                continue; // Need at least 3 data points for meaningful outlier detection
            }
            
            let latencies: Vec<f64> = op_results.iter()
                .map(|r| r.mean_time.as_secs_f64())
                .collect();
            
            let mean = latencies.iter().sum::<f64>() / latencies.len() as f64;
            let std_dev = self.standard_deviation(&latencies);
            
            if std_dev == 0.0 {
                continue; // No variation, no outliers
            }
            
            for result in &op_results {
                let value_secs = result.mean_time.as_secs_f64();
                let z_score = (value_secs - mean) / std_dev;
                
                // Consider results beyond 2 standard deviations as outliers
                if z_score.abs() > 2.0 {
                    let expected_range = (
                        Duration::from_secs_f64(mean - 2.0 * std_dev),
                        Duration::from_secs_f64(mean + 2.0 * std_dev),
                    );
                    
                    let explanation = if z_score > 0.0 {
                        "Performance significantly slower than expected".to_string()
                    } else {
                        "Performance significantly faster than expected".to_string()
                    };
                    
                    outliers.push(OutlierResult {
                        result_id: result.id.clone(),
                        operation: operation.clone(),
                        benchmark_name: result.benchmark_name.clone(),
                        z_score,
                        value: result.mean_time,
                        expected_range,
                        explanation,
                    });
                }
            }
        }
        
        outliers
    }
    
    /// Generate performance recommendations
    fn generate_recommendations(
        &self,
        summary: &ResultSummary,
        operations: &HashMap<String, OperationStats>,
        outliers: &[OutlierResult],
    ) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Latency-based recommendations
        let avg_latency_ms = summary.average_latency.as_secs_f64() * 1000.0;
        if avg_latency_ms > 100.0 {
            recommendations.push("Consider optimizing for latency - average response time is above 100ms".to_string());
        }
        
        // Throughput-based recommendations
        if let Some(throughput) = summary.average_throughput {
            if throughput < 50.0 {
                recommendations.push("Low throughput detected - consider scaling or optimization".to_string());
            }
        }
        
        // Variability recommendations
        for (operation, stats) in operations {
            if stats.coefficient_of_variation > 0.5 {
                recommendations.push(format!(
                    "High variability in {} operation (CV: {:.2}) - investigate inconsistent performance",
                    operation, stats.coefficient_of_variation
                ));
            }
        }
        
        // Outlier recommendations
        if outliers.len() > operations.len() * 2 {
            recommendations.push("High number of outliers detected - check for environmental factors affecting performance".to_string());
        }
        
        // Grade-based recommendations
        match summary.performance_grade {
            PerformanceGrade::NeedsImprovement => {
                recommendations.push("Overall performance needs significant improvement - consider system optimization".to_string());
            }
            PerformanceGrade::BelowAverage => {
                recommendations.push("Performance is below average - review configuration and resource allocation".to_string());
            }
            PerformanceGrade::Fair => {
                recommendations.push("Performance is acceptable but has room for improvement".to_string());
            }
            _ => {}
        }
        
        if recommendations.is_empty() {
            recommendations.push("Performance looks good - continue monitoring for regressions".to_string());
        }
        
        recommendations
    }
    
    /// Calculate percentile value from sorted data
    fn percentile(&self, sorted_data: &[f64], percentile: f64) -> f64 {
        if sorted_data.is_empty() {
            return 0.0;
        }
        
        let index = (percentile / 100.0) * (sorted_data.len() - 1) as f64;
        let lower = index.floor() as usize;
        let upper = index.ceil() as usize;
        
        if lower == upper {
            sorted_data[lower]
        } else {
            let weight = index - lower as f64;
            sorted_data[lower] * (1.0 - weight) + sorted_data[upper] * weight
        }
    }
    
    /// Calculate standard deviation
    fn standard_deviation(&self, data: &[f64]) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }
        
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        let variance = data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / (data.len() - 1) as f64;
        
        variance.sqrt()
    }
    
    /// Calculate trend direction and magnitude
    fn calculate_trend(&self, results: &[&UnifiedBenchmarkResult]) -> (TrendDirection, f64, f64) {
        if results.len() < 3 {
            return (TrendDirection::Insufficient, 0.0, 0.0);
        }
        
        // Simple linear regression on latency over time
        let n = results.len() as f64;
        let x_values: Vec<f64> = (0..results.len()).map(|i| i as f64).collect();
        let y_values: Vec<f64> = results.iter()
            .map(|r| r.mean_time.as_secs_f64() * 1000.0)
            .collect();
        
        let x_mean = x_values.iter().sum::<f64>() / n;
        let y_mean = y_values.iter().sum::<f64>() / n;
        
        let numerator: f64 = x_values.iter().zip(&y_values)
            .map(|(x, y)| (x - x_mean) * (y - y_mean))
            .sum();
        
        let denominator: f64 = x_values.iter()
            .map(|x| (x - x_mean).powi(2))
            .sum();
        
        if denominator == 0.0 {
            return (TrendDirection::Stable, 0.0, 0.0);
        }
        
        let slope = numerator / denominator;
        let magnitude = (slope.abs() / y_mean) * 100.0; // Percentage change
        
        // Calculate correlation coefficient for confidence
        let y_std_dev = self.standard_deviation(&y_values);
        let x_std_dev = self.standard_deviation(&x_values);
        
        let correlation = if y_std_dev > 0.0 && x_std_dev > 0.0 {
            numerator / ((n - 1.0) * x_std_dev * y_std_dev)
        } else {
            0.0
        };
        
        let confidence = correlation.abs();
        
        let direction = if magnitude < 5.0 {
            TrendDirection::Stable
        } else if confidence < 0.3 {
            TrendDirection::Volatile
        } else if slope > 0.0 {
            TrendDirection::Degrading
        } else {
            TrendDirection::Improving
        };
        
        (direction, magnitude, confidence)
    }
    
    /// Compare operations between two result sets
    fn compare_operations(
        &self,
        baseline: &[UnifiedBenchmarkResult],
        current: &[UnifiedBenchmarkResult],
    ) -> HashMap<String, OperationComparison> {
        let baseline_ops = self.analyze_operations(baseline);
        let current_ops = self.analyze_operations(current);
        
        let mut comparisons = HashMap::new();
        
        for (operation, current_stats) in &current_ops {
            if let Some(baseline_stats) = baseline_ops.get(operation) {
                let latency_change = self.calculate_percentage_change(
                    baseline_stats.avg_latency.as_secs_f64() * 1000.0,
                    current_stats.avg_latency.as_secs_f64() * 1000.0,
                );
                
                let throughput_change = match (baseline_stats.avg_throughput, current_stats.avg_throughput) {
                    (Some(baseline_tp), Some(current_tp)) => {
                        Some(self.calculate_percentage_change(baseline_tp, current_tp))
                    }
                    _ => None,
                };
                
                let significance = self.calculate_significance(baseline_stats, current_stats);
                let change_type = self.classify_change(latency_change, significance);
                
                comparisons.insert(operation.clone(), OperationComparison {
                    operation: operation.clone(),
                    latency_change,
                    throughput_change,
                    significance,
                    change_type,
                });
            }
        }
        
        comparisons
    }
    
    /// Calculate percentage change between two values
    fn calculate_percentage_change(&self, baseline: f64, current: f64) -> f64 {
        if baseline == 0.0 {
            return if current == 0.0 { 0.0 } else { 100.0 };
        }
        ((current - baseline) / baseline) * 100.0
    }
    
    /// Calculate statistical significance of difference
    fn calculate_significance(&self, baseline: &OperationStats, current: &OperationStats) -> StatisticalSignificance {
        // Simplified significance calculation based on effect size and sample size
        let effect_size = (current.avg_latency.as_secs_f64() - baseline.avg_latency.as_secs_f64()).abs()
            / baseline.std_dev.as_secs_f64().max(0.001);
        
        let min_sample_size = baseline.count.min(current.count);
        
        if effect_size > 0.8 && min_sample_size >= 30 {
            StatisticalSignificance::High
        } else if effect_size > 0.5 && min_sample_size >= 20 {
            StatisticalSignificance::Medium
        } else if effect_size > 0.2 && min_sample_size >= 10 {
            StatisticalSignificance::Low
        } else {
            StatisticalSignificance::None
        }
    }
    
    /// Classify the type of change
    fn classify_change(&self, latency_change: f64, significance: StatisticalSignificance) -> ChangeType {
        match significance {
            StatisticalSignificance::None => ChangeType::NoChange,
            _ => {
                if latency_change.abs() < 5.0 {
                    ChangeType::NoChange
                } else if latency_change < -25.0 {
                    ChangeType::SignificantImprovement
                } else if latency_change < -5.0 {
                    ChangeType::MinorImprovement
                } else if latency_change > 25.0 {
                    ChangeType::SignificantRegression
                } else {
                    ChangeType::MinorRegression
                }
            }
        }
    }
    
    /// Calculate overall performance change
    fn calculate_overall_change(
        &self,
        baseline: &BenchmarkAnalysis,
        current: &BenchmarkAnalysis,
    ) -> PerformanceChange {
        let latency_change = self.calculate_percentage_change(
            baseline.summary.average_latency.as_secs_f64() * 1000.0,
            current.summary.average_latency.as_secs_f64() * 1000.0,
        );
        
        let throughput_change = match (baseline.summary.average_throughput, current.summary.average_throughput) {
            (Some(baseline_tp), Some(current_tp)) => {
                Some(self.calculate_percentage_change(baseline_tp, current_tp))
            }
            _ => None,
        };
        
        let grade_change = GradeChange {
            from: baseline.summary.performance_grade,
            to: current.summary.performance_grade,
            direction: self.compare_grades(baseline.summary.performance_grade, current.summary.performance_grade),
        };
        
        let summary = self.generate_change_summary(latency_change, throughput_change, &grade_change);
        
        PerformanceChange {
            latency_change,
            throughput_change,
            grade_change,
            summary,
        }
    }
    
    /// Compare performance grades
    fn compare_grades(&self, from: PerformanceGrade, to: PerformanceGrade) -> GradeDirection {
        let from_score = match from {
            PerformanceGrade::Excellent => 6,
            PerformanceGrade::VeryGood => 5,
            PerformanceGrade::Good => 4,
            PerformanceGrade::Fair => 3,
            PerformanceGrade::BelowAverage => 2,
            PerformanceGrade::NeedsImprovement => 1,
        };
        
        let to_score = match to {
            PerformanceGrade::Excellent => 6,
            PerformanceGrade::VeryGood => 5,
            PerformanceGrade::Good => 4,
            PerformanceGrade::Fair => 3,
            PerformanceGrade::BelowAverage => 2,
            PerformanceGrade::NeedsImprovement => 1,
        };
        
        match to_score.cmp(&from_score) {
            std::cmp::Ordering::Greater => GradeDirection::Improved,
            std::cmp::Ordering::Less => GradeDirection::Degraded,
            std::cmp::Ordering::Equal => GradeDirection::Unchanged,
        }
    }
    
    /// Generate change summary text
    fn generate_change_summary(
        &self,
        latency_change: f64,
        throughput_change: Option<f64>,
        grade_change: &GradeChange,
    ) -> String {
        let mut parts = Vec::new();
        
        if latency_change.abs() > 5.0 {
            if latency_change > 0.0 {
                parts.push(format!("Latency increased by {:.1}%", latency_change));
            } else {
                parts.push(format!("Latency improved by {:.1}%", -latency_change));
            }
        }
        
        if let Some(tp_change) = throughput_change {
            if tp_change.abs() > 5.0 {
                if tp_change > 0.0 {
                    parts.push(format!("Throughput increased by {:.1}%", tp_change));
                } else {
                    parts.push(format!("Throughput decreased by {:.1}%", -tp_change));
                }
            }
        }
        
        match grade_change.direction {
            GradeDirection::Improved => {
                parts.push(format!("Performance grade improved from {} to {}", grade_change.from, grade_change.to));
            }
            GradeDirection::Degraded => {
                parts.push(format!("Performance grade degraded from {} to {}", grade_change.from, grade_change.to));
            }
            GradeDirection::Unchanged => {
                parts.push(format!("Performance grade remained {}", grade_change.to));
            }
        }
        
        if parts.is_empty() {
            "No significant performance changes detected".to_string()
        } else {
            parts.join("; ")
        }
    }
    
    /// Detect regressions from operation comparisons
    fn detect_regressions(&self, comparisons: &HashMap<String, OperationComparison>) -> Vec<RegressionAlert> {
        let mut regressions = Vec::new();
        
        for (_, comparison) in comparisons {
            if matches!(comparison.change_type, ChangeType::MinorRegression | ChangeType::SignificantRegression) {
                let severity = if comparison.latency_change > 50.0 {
                    AlertSeverity::Critical
                } else if comparison.latency_change > 25.0 {
                    AlertSeverity::High
                } else if comparison.latency_change > 10.0 {
                    AlertSeverity::Medium
                } else {
                    AlertSeverity::Low
                };
                
                let description = format!(
                    "{} operation shows {:.1}% latency increase",
                    comparison.operation, comparison.latency_change
                );
                
                regressions.push(RegressionAlert {
                    operation: comparison.operation.clone(),
                    regression_percent: comparison.latency_change,
                    significance: comparison.significance,
                    severity,
                    description,
                });
            }
        }
        
        regressions
    }
    
    /// Detect improvements from operation comparisons
    fn detect_improvements(&self, comparisons: &HashMap<String, OperationComparison>) -> Vec<ImprovementAlert> {
        let mut improvements = Vec::new();
        
        for (_, comparison) in comparisons {
            if matches!(comparison.change_type, ChangeType::MinorImprovement | ChangeType::SignificantImprovement) {
                let description = format!(
                    "{} operation shows {:.1}% latency improvement",
                    comparison.operation, -comparison.latency_change
                );
                
                improvements.push(ImprovementAlert {
                    operation: comparison.operation.clone(),
                    improvement_percent: -comparison.latency_change,
                    significance: comparison.significance,
                    description,
                });
            }
        }
        
        improvements
    }
    
    /// Create empty analysis for when no results are available
    fn empty_analysis(&self) -> BenchmarkAnalysis {
        BenchmarkAnalysis {
            summary: ResultSummary {
                total_results: 0,
                total_samples: 0,
                average_latency: Duration::from_secs(0),
                median_latency: Duration::from_secs(0),
                p95_latency: Duration::from_secs(0),
                p99_latency: Duration::from_secs(0),
                average_throughput: None,
                peak_throughput: None,
                performance_grade: PerformanceGrade::NeedsImprovement,
                latency_confidence_interval: (Duration::from_secs(0), Duration::from_secs(0)),
            },
            operation_breakdown: HashMap::new(),
            category_analysis: HashMap::new(),
            trends: Vec::new(),
            outliers: Vec::new(),
            recommendations: vec!["No benchmark results available for analysis".to_string()],
        }
    }
}

impl Default for ResultsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use std::collections::HashMap;
    
    fn create_test_result(
        operation: &str,
        category: BenchmarkCategory,
        mean_time_ms: u64,
        throughput: Option<f64>,
    ) -> UnifiedBenchmarkResult {
        UnifiedBenchmarkResult {
            id: Uuid::new_v4().to_string(),
            benchmark_name: "test_benchmark".to_string(),
            operation: operation.to_string(),
            category,
            mean_time: Duration::from_millis(mean_time_ms),
            std_dev: Duration::from_millis(mean_time_ms / 10),
            min_time: Duration::from_millis(mean_time_ms - mean_time_ms / 5),
            max_time: Duration::from_millis(mean_time_ms + mean_time_ms / 5),
            throughput,
            samples: 100,
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            percentiles: None,
        }
    }
    
    #[test]
    fn test_results_analysis() {
        let analyzer = ResultsAnalyzer::new();
        
        let results = vec![
            create_test_result("read", BenchmarkCategory::FileOperations, 10, Some(100.0)),
            create_test_result("write", BenchmarkCategory::FileOperations, 20, Some(50.0)),
            create_test_result("list", BenchmarkCategory::MetadataOperations, 5, None),
        ];
        
        let analysis = analyzer.analyze(&results);
        
        assert_eq!(analysis.summary.total_results, 3);
        assert_eq!(analysis.operation_breakdown.len(), 3);
        assert_eq!(analysis.category_analysis.len(), 2);
        assert!(!analysis.recommendations.is_empty());
    }
    
    #[test]
    fn test_outlier_detection() {
        let analyzer = ResultsAnalyzer::new();
        
        let mut results = vec![
            create_test_result("read", BenchmarkCategory::FileOperations, 10, Some(100.0)),
            create_test_result("read", BenchmarkCategory::FileOperations, 12, Some(90.0)),
            create_test_result("read", BenchmarkCategory::FileOperations, 11, Some(95.0)),
        ];
        
        // Add outlier
        results.push(create_test_result("read", BenchmarkCategory::FileOperations, 50, Some(20.0)));
        
        let analysis = analyzer.analyze(&results);
        
        assert!(!analysis.outliers.is_empty());
        assert_eq!(analysis.outliers[0].operation, "read");
    }
    
    #[test]
    fn test_performance_comparison() {
        let analyzer = ResultsAnalyzer::new();
        
        let baseline = vec![
            create_test_result("read", BenchmarkCategory::FileOperations, 10, Some(100.0)),
            create_test_result("write", BenchmarkCategory::FileOperations, 20, Some(50.0)),
        ];
        
        let current = vec![
            create_test_result("read", BenchmarkCategory::FileOperations, 15, Some(80.0)), // Regression
            create_test_result("write", BenchmarkCategory::FileOperations, 18, Some(55.0)), // Improvement
        ];
        
        let comparison = analyzer.compare(&baseline, &current);
        
        assert_eq!(comparison.operation_comparisons.len(), 2);
        
        let read_comparison = &comparison.operation_comparisons["read"];
        assert!(read_comparison.latency_change > 0.0); // Increased latency
        
        let write_comparison = &comparison.operation_comparisons["write"];
        assert!(write_comparison.latency_change < 0.0); // Decreased latency
    }
    
    #[test]
    fn test_trend_analysis() {
        let analyzer = ResultsAnalyzer::new();
        
        let mut results = Vec::new();
        let base_time = chrono::Utc::now();
        
        // Create trending data - performance degrading over time
        for i in 0..10 {
            let mut result = create_test_result("read", BenchmarkCategory::FileOperations, 10 + i * 2, Some(100.0));
            result.timestamp = base_time + chrono::Duration::minutes(i as i64);
            results.push(result);
        }
        
        let analysis = analyzer.analyze(&results);
        
        assert!(!analysis.trends.is_empty());
        let read_trend = analysis.trends.iter().find(|t| t.subject == "read").unwrap();
        assert_eq!(read_trend.direction, TrendDirection::Degrading);
    }
}