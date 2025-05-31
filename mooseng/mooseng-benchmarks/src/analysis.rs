//! Advanced performance analysis and visualization tools for MooseNG benchmarks
//!
//! This module provides statistical analysis, trend detection, and advanced
//! visualization capabilities for benchmark results.

use crate::{BenchmarkResult, BenchmarkConfig};
use crate::metrics::{MetricsSummary, PerformanceReport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Statistical analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalAnalysis {
    /// Operation being analyzed
    pub operation: String,
    /// Number of samples
    pub sample_count: usize,
    /// Mean performance
    pub mean: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Coefficient of variation (std_dev / mean)
    pub coefficient_of_variation: f64,
    /// 95% confidence interval
    pub confidence_interval_95: (f64, f64),
    /// Percentile distribution
    pub percentiles: PercentileDistribution,
    /// Outlier detection results
    pub outliers: OutlierAnalysis,
    /// Trend analysis
    pub trend: TrendAnalysis,
}

/// Percentile distribution analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercentileDistribution {
    pub p10: f64,
    pub p25: f64,
    pub p50: f64,  // Median
    pub p75: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub p999: f64,
}

/// Outlier detection results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierAnalysis {
    /// Number of outliers detected
    pub outlier_count: usize,
    /// Percentage of samples that are outliers
    pub outlier_percentage: f64,
    /// Outlier detection method used
    pub detection_method: String,
    /// Threshold values used for detection
    pub thresholds: (f64, f64),
}

/// Trend analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// Linear regression slope
    pub slope: f64,
    /// R-squared correlation coefficient
    pub r_squared: f64,
    /// Trend direction
    pub trend_direction: TrendDirection,
    /// Confidence in trend detection
    pub confidence: f64,
}

/// Trend direction enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Degrading,
    Stable,
    Volatile,
}

/// Performance regression analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAnalysis {
    /// Baseline vs current comparison
    pub baseline_comparison: BaselineComparison,
    /// Performance regression severity
    pub regression_severity: RegressionSeverity,
    /// Affected operations
    pub affected_operations: Vec<String>,
    /// Root cause analysis suggestions
    pub suggested_investigations: Vec<String>,
}

/// Baseline comparison results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    /// Operations that improved
    pub improvements: Vec<PerformanceChange>,
    /// Operations that regressed
    pub regressions: Vec<PerformanceChange>,
    /// Operations that remained stable
    pub stable: Vec<String>,
    /// Overall performance score
    pub overall_score: f64,
}

/// Individual performance change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceChange {
    /// Operation name
    pub operation: String,
    /// Percentage change (positive = improvement, negative = regression)
    pub percentage_change: f64,
    /// Absolute time change
    pub absolute_change: Duration,
    /// Statistical significance
    pub significance: f64,
}

/// Regression severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegressionSeverity {
    None,
    Minor,     // < 5% regression
    Moderate,  // 5-15% regression
    Major,     // 15-30% regression
    Critical,  // > 30% regression
}

/// Advanced performance analyzer
pub struct AdvancedAnalyzer {
    /// Historical results for trend analysis
    historical_results: Vec<Vec<BenchmarkResult>>,
    /// Statistical configuration
    config: AnalysisConfig,
}

/// Configuration for statistical analysis
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Confidence level for intervals (e.g., 0.95 for 95%)
    pub confidence_level: f64,
    /// Outlier detection sensitivity (1.5 = standard, 3.0 = conservative)
    pub outlier_sensitivity: f64,
    /// Minimum sample size for trend analysis
    pub min_trend_samples: usize,
    /// Regression threshold for significance (e.g., 0.05 for 5%)
    pub regression_threshold: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            confidence_level: 0.95,
            outlier_sensitivity: 1.5,
            min_trend_samples: 10,
            regression_threshold: 0.05,
        }
    }
}

impl AdvancedAnalyzer {
    pub fn new() -> Self {
        Self::with_config(AnalysisConfig::default())
    }

    pub fn with_config(config: AnalysisConfig) -> Self {
        Self {
            historical_results: Vec::new(),
            config,
        }
    }

    /// Add historical results for trend analysis
    pub fn add_historical_results(&mut self, results: Vec<BenchmarkResult>) {
        self.historical_results.push(results);
    }

    /// Perform comprehensive statistical analysis
    pub fn analyze_results(&self, results: &[BenchmarkResult]) -> Vec<StatisticalAnalysis> {
        let mut analyses = Vec::new();

        // Group results by operation
        let mut by_operation: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();
        for result in results {
            by_operation.entry(result.operation.clone()).or_insert_with(Vec::new).push(result);
        }

        for (operation, operation_results) in by_operation {
            let analysis = self.analyze_operation(&operation, &operation_results);
            analyses.push(analysis);
        }

        analyses
    }

    /// Analyze a specific operation
    fn analyze_operation(&self, operation: &str, results: &[&BenchmarkResult]) -> StatisticalAnalysis {
        // Convert results to timing values
        let values: Vec<f64> = results.iter()
            .map(|r| r.mean_time.as_nanos() as f64)
            .collect();

        // Calculate basic statistics
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        let coefficient_of_variation = if mean > 0.0 { std_dev / mean } else { 0.0 };

        // Calculate confidence interval
        let confidence_interval_95 = self.calculate_confidence_interval(&values, self.config.confidence_level);

        // Calculate percentiles
        let percentiles = self.calculate_percentiles(&values);

        // Detect outliers
        let outliers = self.detect_outliers(&values);

        // Perform trend analysis
        let trend = self.analyze_trend(operation);

        StatisticalAnalysis {
            operation: operation.to_string(),
            sample_count: values.len(),
            mean,
            std_dev,
            coefficient_of_variation,
            confidence_interval_95,
            percentiles,
            outliers,
            trend,
        }
    }

    /// Calculate confidence interval for mean
    fn calculate_confidence_interval(&self, values: &[f64], confidence_level: f64) -> (f64, f64) {
        if values.len() < 2 {
            let mean = values.first().cloned().unwrap_or(0.0);
            return (mean, mean);
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let std_dev = {
            let variance = values.iter()
                .map(|v| (v - mean).powi(2))
                .sum::<f64>() / (values.len() - 1) as f64;
            variance.sqrt()
        };

        let standard_error = std_dev / (values.len() as f64).sqrt();
        
        // Use t-distribution critical value (approximated)
        let t_critical = self.get_t_critical(values.len() - 1, confidence_level);
        let margin_of_error = t_critical * standard_error;

        (mean - margin_of_error, mean + margin_of_error)
    }

    /// Get t-distribution critical value (simplified approximation)
    fn get_t_critical(&self, df: usize, confidence_level: f64) -> f64 {
        // Simplified approximation - in practice, would use proper t-table
        let alpha = 1.0 - confidence_level;
        match confidence_level {
            x if x >= 0.99 => 2.576,
            x if x >= 0.95 => 1.96,
            x if x >= 0.90 => 1.645,
            _ => 1.96,
        }
    }

    /// Calculate percentile distribution
    fn calculate_percentiles(&self, values: &[f64]) -> PercentileDistribution {
        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let percentile = |p: f64| -> f64 {
            if sorted_values.is_empty() {
                return 0.0;
            }
            let index = (p * (sorted_values.len() - 1) as f64).round() as usize;
            sorted_values[index.min(sorted_values.len() - 1)]
        };

        PercentileDistribution {
            p10: percentile(0.10),
            p25: percentile(0.25),
            p50: percentile(0.50),
            p75: percentile(0.75),
            p90: percentile(0.90),
            p95: percentile(0.95),
            p99: percentile(0.99),
            p999: percentile(0.999),
        }
    }

    /// Detect outliers using IQR method
    fn detect_outliers(&self, values: &[f64]) -> OutlierAnalysis {
        if values.len() < 4 {
            return OutlierAnalysis {
                outlier_count: 0,
                outlier_percentage: 0.0,
                detection_method: "iqr".to_string(),
                thresholds: (0.0, 0.0),
            };
        }

        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let q1_index = sorted_values.len() / 4;
        let q3_index = 3 * sorted_values.len() / 4;
        let q1 = sorted_values[q1_index];
        let q3 = sorted_values[q3_index];
        let iqr = q3 - q1;

        let lower_threshold = q1 - self.config.outlier_sensitivity * iqr;
        let upper_threshold = q3 + self.config.outlier_sensitivity * iqr;

        let outlier_count = values.iter()
            .filter(|&&v| v < lower_threshold || v > upper_threshold)
            .count();

        OutlierAnalysis {
            outlier_count,
            outlier_percentage: (outlier_count as f64 / values.len() as f64) * 100.0,
            detection_method: "iqr".to_string(),
            thresholds: (lower_threshold, upper_threshold),
        }
    }

    /// Analyze performance trends across historical data
    fn analyze_trend(&self, operation: &str) -> TrendAnalysis {
        if self.historical_results.len() < self.config.min_trend_samples {
            return TrendAnalysis {
                slope: 0.0,
                r_squared: 0.0,
                trend_direction: TrendDirection::Stable,
                confidence: 0.0,
            };
        }

        // Extract time series data for this operation
        let mut time_series = Vec::new();
        for (i, results) in self.historical_results.iter().enumerate() {
            if let Some(result) = results.iter().find(|r| r.operation == operation) {
                time_series.push((i as f64, result.mean_time.as_nanos() as f64));
            }
        }

        if time_series.len() < 3 {
            return TrendAnalysis {
                slope: 0.0,
                r_squared: 0.0,
                trend_direction: TrendDirection::Stable,
                confidence: 0.0,
            };
        }

        // Perform linear regression
        let (slope, r_squared) = self.linear_regression(&time_series);

        // Determine trend direction
        let trend_direction = match slope {
            s if s > 1000000.0 => TrendDirection::Degrading,   // Getting slower
            s if s < -1000000.0 => TrendDirection::Improving, // Getting faster
            _ if r_squared < 0.5 => TrendDirection::Volatile,  // High variance
            _ => TrendDirection::Stable,
        };

        TrendAnalysis {
            slope,
            r_squared,
            trend_direction,
            confidence: r_squared,
        }
    }

    /// Perform linear regression on time series data
    fn linear_regression(&self, data: &[(f64, f64)]) -> (f64, f64) {
        let n = data.len() as f64;
        if n < 2.0 {
            return (0.0, 0.0);
        }

        let sum_x: f64 = data.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = data.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = data.iter().map(|(x, y)| x * y).sum();
        let sum_x2: f64 = data.iter().map(|(x, _)| x * x).sum();
        let sum_y2: f64 = data.iter().map(|(_, y)| y * y).sum();

        let mean_x = sum_x / n;
        let mean_y = sum_y / n;

        let numerator = sum_xy - n * mean_x * mean_y;
        let denominator = sum_x2 - n * mean_x * mean_x;

        let slope = if denominator != 0.0 {
            numerator / denominator
        } else {
            0.0
        };

        // Calculate R-squared
        let ss_tot = sum_y2 - n * mean_y * mean_y;
        let ss_res = data.iter()
            .map(|(x, y)| {
                let predicted = mean_y + slope * (x - mean_x);
                (y - predicted).powi(2)
            })
            .sum::<f64>();

        let r_squared = if ss_tot != 0.0 {
            1.0 - (ss_res / ss_tot)
        } else {
            0.0
        };

        (slope, r_squared.max(0.0))
    }

    /// Perform regression analysis comparing against baseline
    pub fn analyze_regression(
        &self,
        current_results: &[BenchmarkResult],
        baseline_results: &[BenchmarkResult],
    ) -> RegressionAnalysis {
        let mut improvements = Vec::new();
        let mut regressions = Vec::new();
        let mut stable = Vec::new();

        // Group results by operation
        let current_by_op: HashMap<&str, &BenchmarkResult> = current_results.iter()
            .map(|r| (r.operation.as_str(), r))
            .collect();

        let baseline_by_op: HashMap<&str, &BenchmarkResult> = baseline_results.iter()
            .map(|r| (r.operation.as_str(), r))
            .collect();

        for (operation, current_result) in &current_by_op {
            if let Some(baseline_result) = baseline_by_op.get(operation) {
                let current_time = current_result.mean_time.as_nanos() as f64;
                let baseline_time = baseline_result.mean_time.as_nanos() as f64;

                if baseline_time > 0.0 {
                    let percentage_change = ((baseline_time - current_time) / baseline_time) * 100.0;
                    let absolute_change = current_result.mean_time - baseline_result.mean_time;

                    // Calculate statistical significance (simplified)
                    let significance = self.calculate_significance(current_result, baseline_result);

                    let change = PerformanceChange {
                        operation: operation.to_string(),
                        percentage_change,
                        absolute_change,
                        significance,
                    };

                    if percentage_change.abs() < self.config.regression_threshold * 100.0 {
                        stable.push(operation.to_string());
                    } else if percentage_change > 0.0 {
                        improvements.push(change);
                    } else {
                        regressions.push(change);
                    }
                }
            }
        }

        // Calculate overall performance score
        let total_operations = improvements.len() + regressions.len() + stable.len();
        let overall_score = if total_operations > 0 {
            let improvement_score = improvements.len() as f64 * 1.0;
            let stable_score = stable.len() as f64 * 0.5;
            let regression_penalty = regressions.iter()
                .map(|r| r.percentage_change.abs() / 100.0)
                .sum::<f64>();

            ((improvement_score + stable_score - regression_penalty) / total_operations as f64).max(0.0)
        } else {
            0.5
        };

        // Determine regression severity
        let max_regression = regressions.iter()
            .map(|r| r.percentage_change.abs())
            .fold(0.0, f64::max);

        let regression_severity = match max_regression {
            x if x > 30.0 => RegressionSeverity::Critical,
            x if x > 15.0 => RegressionSeverity::Major,
            x if x > 5.0 => RegressionSeverity::Moderate,
            x if x > 0.0 => RegressionSeverity::Minor,
            _ => RegressionSeverity::None,
        };

        // Generate investigation suggestions
        let suggested_investigations = self.generate_investigation_suggestions(&regressions);

        RegressionAnalysis {
            baseline_comparison: BaselineComparison {
                improvements,
                regressions: regressions.clone(),
                stable,
                overall_score,
            },
            regression_severity,
            affected_operations: regressions.iter().map(|r| r.operation.clone()).collect(),
            suggested_investigations,
        }
    }

    /// Calculate statistical significance of difference (simplified)
    fn calculate_significance(&self, current: &BenchmarkResult, baseline: &BenchmarkResult) -> f64 {
        // Simplified t-test approximation
        let current_mean = current.mean_time.as_nanos() as f64;
        let baseline_mean = baseline.mean_time.as_nanos() as f64;
        let current_std = current.std_dev.as_nanos() as f64;
        let baseline_std = baseline.std_dev.as_nanos() as f64;

        let pooled_std = ((current_std * current_std + baseline_std * baseline_std) / 2.0).sqrt();
        
        if pooled_std > 0.0 {
            let t_stat = (current_mean - baseline_mean).abs() / 
                         (pooled_std * (2.0 / current.samples.max(1) as f64).sqrt());
            
            // Convert to rough p-value (simplified)
            if t_stat > 2.0 {
                0.05  // Significant
            } else if t_stat > 1.0 {
                0.1   // Marginally significant
            } else {
                0.5   // Not significant
            }
        } else {
            0.5
        }
    }

    /// Generate investigation suggestions based on regressions
    fn generate_investigation_suggestions(&self, regressions: &[PerformanceChange]) -> Vec<String> {
        let mut suggestions = Vec::new();

        if regressions.is_empty() {
            return suggestions;
        }

        // Categorize regressions
        let network_regressions = regressions.iter()
            .filter(|r| r.operation.contains("network") || r.operation.contains("latency"))
            .count();

        let disk_regressions = regressions.iter()
            .filter(|r| r.operation.contains("file") || r.operation.contains("disk"))
            .count();

        let cpu_regressions = regressions.iter()
            .filter(|r| r.operation.contains("cpu") || r.operation.contains("compute"))
            .count();

        // Generate specific suggestions
        if network_regressions > 0 {
            suggestions.push("Investigate network configuration and latency changes".to_string());
            suggestions.push("Check for network congestion or routing changes".to_string());
        }

        if disk_regressions > 0 {
            suggestions.push("Analyze disk I/O patterns and storage performance".to_string());
            suggestions.push("Check disk space and fragmentation levels".to_string());
        }

        if cpu_regressions > 0 {
            suggestions.push("Profile CPU usage and identify bottlenecks".to_string());
            suggestions.push("Check for resource contention or background processes".to_string());
        }

        // General suggestions
        if regressions.len() > regressions.len() / 2 {
            suggestions.push("Consider system-wide performance issues".to_string());
            suggestions.push("Review recent system changes and updates".to_string());
        }

        suggestions
    }
}

impl Default for AdvancedAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_calculation() {
        let analyzer = AdvancedAnalyzer::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let percentiles = analyzer.calculate_percentiles(&values);
        
        assert_eq!(percentiles.p50, 5.0);
        assert_eq!(percentiles.p90, 9.0);
    }

    #[test]
    fn test_outlier_detection() {
        let analyzer = AdvancedAnalyzer::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0]; // 100.0 is an outlier
        let outliers = analyzer.detect_outliers(&values);
        
        assert!(outliers.outlier_count > 0);
        assert!(outliers.outlier_percentage > 0.0);
    }

    #[test]
    fn test_confidence_interval() {
        let analyzer = AdvancedAnalyzer::new();
        let values = vec![10.0, 12.0, 14.0, 16.0, 18.0];
        let (lower, upper) = analyzer.calculate_confidence_interval(&values, 0.95);
        
        assert!(lower < 14.0); // Mean is 14.0
        assert!(upper > 14.0);
        assert!(upper > lower);
    }

    #[test]
    fn test_linear_regression() {
        let analyzer = AdvancedAnalyzer::new();
        let data = vec![(1.0, 2.0), (2.0, 4.0), (3.0, 6.0)]; // Perfect linear relationship
        let (slope, r_squared) = analyzer.linear_regression(&data);
        
        assert!((slope - 2.0).abs() < 0.1); // Slope should be ~2
        assert!(r_squared > 0.9); // Should be near 1.0 for perfect fit
    }
}