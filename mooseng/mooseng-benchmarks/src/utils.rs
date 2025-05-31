//! Utility functions for MooseNG benchmarks

use std::time::Duration;
use crate::{BenchmarkResult, Percentiles};

/// Format duration for human-readable output
pub fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    
    if nanos < 1_000 {
        format!("{}ns", nanos)
    } else if nanos < 1_000_000 {
        format!("{:.2}μs", nanos as f64 / 1_000.0)
    } else if nanos < 1_000_000_000 {
        format!("{:.2}ms", nanos as f64 / 1_000_000.0)
    } else {
        format!("{:.2}s", nanos as f64 / 1_000_000_000.0)
    }
}

/// Format bytes for human-readable output
pub fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{}B", bytes)
    } else {
        format!("{:.2}{}", size, UNITS[unit_index])
    }
}

/// Calculate standard deviation from a set of values
pub fn calculate_std_dev(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter()
        .map(|v| (v - mean).powi(2))
        .sum::<f64>() / values.len() as f64;
    
    variance.sqrt()
}

/// Calculate percentile from sorted values
pub fn calculate_percentile(sorted_values: &[f64], percentile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    
    let index = (percentile / 100.0 * (sorted_values.len() - 1) as f64).round() as usize;
    sorted_values[index.min(sorted_values.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_nanos(500)), "500ns");
        assert_eq!(format_duration(Duration::from_micros(1500)), "1.50ms");
        assert_eq!(format_duration(Duration::from_millis(2500)), "2.50s");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1536), "1.50KB");
        assert_eq!(format_bytes(2097152), "2.00MB");
    }

    #[test]
    fn test_calculate_std_dev() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let std_dev = calculate_std_dev(&values);
        assert!((std_dev - 1.58).abs() < 0.1);
    }

    #[test]
    fn test_calculate_percentile() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(calculate_percentile(&values, 50.0), 5.0);
        assert_eq!(calculate_percentile(&values, 90.0), 9.0);
    }
}

/// Helper function to create a BenchmarkResult from timing data
pub fn create_benchmark_result(
    times: &[Duration],
    operation: impl Into<String>,
    data_size: usize,
) -> BenchmarkResult {
    let times_ns: Vec<f64> = times.iter().map(|d| d.as_nanos() as f64).collect();
    let mean = times_ns.iter().sum::<f64>() / times_ns.len().max(1) as f64;
    let variance = times_ns.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / times_ns.len().max(1) as f64;
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
    
    // Calculate percentiles
    let mut sorted_times = times_ns.clone();
    sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let percentiles = if !sorted_times.is_empty() {
        let p50_idx = ((sorted_times.len() - 1) as f64 * 0.50) as usize;
        let p90_idx = ((sorted_times.len() - 1) as f64 * 0.90) as usize;
        let p95_idx = ((sorted_times.len() - 1) as f64 * 0.95) as usize;
        let p99_idx = ((sorted_times.len() - 1) as f64 * 0.99) as usize;
        let p999_idx = ((sorted_times.len() - 1) as f64 * 0.999) as usize;
        
        Some(Percentiles {
            p50: Duration::from_nanos(sorted_times.get(p50_idx).cloned().unwrap_or(mean) as u64),
            p90: Duration::from_nanos(sorted_times.get(p90_idx).cloned().unwrap_or(mean) as u64),
            p95: Duration::from_nanos(sorted_times.get(p95_idx).cloned().unwrap_or(mean) as u64),
            p99: Duration::from_nanos(sorted_times.get(p99_idx).cloned().unwrap_or(mean) as u64),
            p999: Duration::from_nanos(sorted_times.get(p999_idx).cloned().unwrap_or(mean) as u64),
        })
    } else {
        None
    };
    
    BenchmarkResult {
        operation: operation.into(),
        mean_time: mean_duration,
        std_dev: Duration::from_nanos(std_dev as u64),
        min_time: min,
        max_time: max,
        throughput,
        samples: times.len(),
        timestamp: chrono::Utc::now(),
        metadata: None,
        percentiles,
    }
}