//! Comprehensive benchmark runner for MooseNG
//!
//! This binary orchestrates the complete benchmarking suite including:
//! - Real network condition testing
//! - Multi-region infrastructure deployment and testing
//! - Performance metrics collection and analysis
//! - Report generation and visualization

use clap::{Arg, Command};
use mooseng_benchmarks::{
    Benchmark, BenchmarkConfig, BenchmarkResult,
    benchmarks::{
        network_simulation::{NetworkConditionBenchmark, RealNetworkBenchmark},
        infrastructure::{InfrastructureBenchmark, InfrastructureConfig},
        mod::BenchmarkSuite,
    },
    config::AdvancedBenchmarkConfig,
    metrics::{MetricsCollector, MetricsConfig, PerformanceAnalyzer, create_metric},
    report::{ReportGenerator, ReportConfig},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tracing::{info, warn, error, debug};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let matches = Command::new("MooseNG Comprehensive Benchmark")
        .version("1.0.0")
        .author("MooseNG Team")
        .about("Comprehensive benchmarking suite for MooseNG distributed file system")
        .arg(Arg::new("config")
            .short('c')
            .long("config")
            .value_name("FILE")
            .help("Configuration file path")
            .required(false))
        .arg(Arg::new("output")
            .short('o')
            .long("output")
            .value_name("DIR")
            .help("Output directory for results")
            .required(false))
        .arg(Arg::new("suite")
            .short('s')
            .long("suite")
            .value_name("SUITE")
            .help("Benchmark suite to run: network, infrastructure, full")
            .default_value("full"))
        .arg(Arg::new("iterations")
            .short('i')
            .long("iterations")
            .value_name("NUM")
            .help("Number of benchmark iterations")
            .default_value("10"))
        .arg(Arg::new("parallel")
            .short('p')
            .long("parallel")
            .help("Run benchmarks in parallel where possible")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("baseline")
            .short('b')
            .long("baseline")
            .value_name("FILE")
            .help("Baseline results file for comparison")
            .required(false))
        .arg(Arg::new("real-network")
            .long("real-network")
            .help("Enable real network interface testing (requires root)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("infrastructure")
            .long("infrastructure")
            .help("Enable infrastructure deployment testing")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("export-formats")
            .long("export-formats")
            .value_name("FORMATS")
            .help("Export formats: json,csv,prometheus,influxdb (comma-separated)")
            .default_value("json,csv"))
        .get_matches();

    let config_file = matches.get_one::<String>("config");
    let output_dir = matches.get_one::<String>("output").map(|s| s.as_str()).unwrap_or("./benchmark_results");
    let suite = matches.get_one::<String>("suite").unwrap();
    let iterations: usize = matches.get_one::<String>("iterations").unwrap().parse()?;
    let parallel = matches.get_flag("parallel");
    let baseline_file = matches.get_one::<String>("baseline");
    let real_network = matches.get_flag("real-network");
    let infrastructure = matches.get_flag("infrastructure");
    let export_formats: Vec<&str> = matches.get_one::<String>("export-formats").unwrap().split(',').collect();

    info!("Starting MooseNG Comprehensive Benchmark Suite");
    info!("Suite: {}, Iterations: {}, Output: {}", suite, iterations, output_dir);

    // Load configuration
    let config = if let Some(config_path) = config_file {
        AdvancedBenchmarkConfig::from_file(config_path)?
    } else {
        AdvancedBenchmarkConfig::default()
    };

    // Validate configuration
    config.validate().map_err(|e| format!("Configuration validation failed: {}", e))?;

    // Create output directory
    std::fs::create_dir_all(output_dir)?;

    // Initialize metrics collector
    let metrics_config = MetricsConfig {
        collect_system_metrics: true,
        collect_network_metrics: true,
        collect_disk_metrics: true,
        sampling_interval_ms: 100,
        max_metrics_in_memory: 50000,
        write_to_disk: true,
        output_dir: format!("{}/metrics", output_dir),
    };
    let mut metrics_collector = MetricsCollector::new(metrics_config);

    // Initialize performance analyzer
    let mut performance_analyzer = PerformanceAnalyzer::new();

    // Load baseline if provided
    if let Some(baseline_path) = baseline_file {
        let baseline_data = std::fs::read_to_string(baseline_path)?;
        let baseline_results: Vec<BenchmarkResult> = serde_json::from_str(&baseline_data)?;
        performance_analyzer.set_baseline(baseline_results);
        info!("Loaded baseline from: {}", baseline_path);
    }

    // Start metrics collection
    metrics_collector.start_collection();
    let benchmark_start = Instant::now();

    // Run benchmarks based on suite selection
    let all_results = match suite.as_str() {
        "network" => run_network_benchmarks(&config, real_network, parallel).await?,
        "infrastructure" => run_infrastructure_benchmarks(&config, infrastructure).await?,
        "full" => run_full_benchmark_suite(&config, real_network, infrastructure, parallel).await?,
        _ => return Err(format!("Unknown benchmark suite: {}", suite).into()),
    };

    let benchmark_duration = benchmark_start.elapsed();
    info!("Benchmark suite completed in {:?}", benchmark_duration);

    // Stop metrics collection
    let metrics_summary = metrics_collector.stop_collection();
    
    // Add results to performance analyzer
    let flattened_results: Vec<BenchmarkResult> = all_results.values().flatten().cloned().collect();
    performance_analyzer.add_results(flattened_results.clone());

    // Generate performance comparison report if baseline was provided
    let performance_report = if baseline_file.is_some() {
        Some(performance_analyzer.generate_comparison_report())
    } else {
        None
    };

    // Export metrics in requested formats
    export_metrics(&metrics_collector, &export_formats, output_dir)?;

    // Generate comprehensive report
    let report_config = ReportConfig {
        output_dir: output_dir.to_string(),
        generate_html: true,
        generate_charts: true,
        include_raw_data: true,
        chart_width: 1200,
        chart_height: 800,
        title: "MooseNG Comprehensive Benchmark Report".to_string(),
    };

    let mut report_generator = ReportGenerator::new(report_config);
    let aggregated_stats = calculate_aggregated_stats(&all_results);
    let benchmark_results_vec: Vec<(String, Vec<BenchmarkResult>)> = all_results.into_iter().collect();
    
    let report_path = report_generator.generate_report(
        &benchmark_results_vec,
        &aggregated_stats,
        performance_report.as_ref(),
    )?;

    // Print summary
    print_comprehensive_summary(&flattened_results, &metrics_summary, benchmark_duration);

    // Save results to JSON for future use as baseline
    let results_json = serde_json::to_string_pretty(&flattened_results)?;
    std::fs::write(format!("{}/benchmark_results.json", output_dir), results_json)?;

    info!("Benchmark completed successfully!");
    info!("Results saved to: {}", output_dir);
    info!("Report generated: {}", report_path);

    Ok(())
}

/// Run network benchmarks
async fn run_network_benchmarks(
    config: &AdvancedBenchmarkConfig,
    real_network: bool,
    parallel: bool,
) -> Result<HashMap<String, Vec<BenchmarkResult>>, Box<dyn std::error::Error>> {
    let mut results = HashMap::new();
    
    info!("Running network benchmarks...");

    // Network condition benchmarks
    let network_benchmark = NetworkConditionBenchmark::new();
    let network_results = if parallel {
        run_benchmark_async(&network_benchmark, &config.base).await
    } else {
        network_benchmark.run(&config.base)
    };
    results.insert("network_conditions".to_string(), network_results);

    // Real network benchmarks (if enabled)
    if real_network {
        info!("Running real network benchmarks...");
        let real_network_benchmark = RealNetworkBenchmark::new();
        let real_results = if parallel {
            run_benchmark_async(&real_network_benchmark, &config.base).await
        } else {
            real_network_benchmark.run(&config.base)
        };
        results.insert("real_network".to_string(), real_results);
    }

    Ok(results)
}

/// Run infrastructure benchmarks
async fn run_infrastructure_benchmarks(
    config: &AdvancedBenchmarkConfig,
    enable_infrastructure: bool,
) -> Result<HashMap<String, Vec<BenchmarkResult>>, Box<dyn std::error::Error>> {
    let mut results = HashMap::new();

    if enable_infrastructure {
        info!("Running infrastructure benchmarks...");
        
        let infra_config = InfrastructureConfig::default();
        let infra_benchmark = InfrastructureBenchmark::new(infra_config);
        let infra_results = infra_benchmark.run(&config.base);
        results.insert("infrastructure".to_string(), infra_results);
    } else {
        info!("Infrastructure benchmarks disabled (use --infrastructure to enable)");
    }

    Ok(results)
}

/// Run the full benchmark suite
async fn run_full_benchmark_suite(
    config: &AdvancedBenchmarkConfig,
    real_network: bool,
    infrastructure: bool,
    parallel: bool,
) -> Result<HashMap<String, Vec<BenchmarkResult>>, Box<dyn std::error::Error>> {
    let mut results = HashMap::new();

    info!("Running full benchmark suite...");

    // Network benchmarks
    let network_results = run_network_benchmarks(config, real_network, parallel).await?;
    results.extend(network_results);

    // Infrastructure benchmarks
    let infra_results = run_infrastructure_benchmarks(config, infrastructure).await?;
    results.extend(infra_results);

    // Standard benchmark suite
    let benchmark_suite = BenchmarkSuite::new();
    let suite_results = benchmark_suite.run_all(&config.base);
    
    for (name, result_vec) in suite_results {
        results.insert(name, result_vec);
    }

    Ok(results)
}

/// Run a benchmark asynchronously for better performance
async fn run_benchmark_async<B: Benchmark + Send + Sync>(
    benchmark: &B,
    config: &BenchmarkConfig,
) -> Vec<BenchmarkResult> {
    // For now, just run synchronously since our Benchmark trait isn't async
    // In a real implementation, you might want to make Benchmark async
    benchmark.run(config)
}

/// Export metrics in various formats
fn export_metrics(
    metrics_collector: &MetricsCollector,
    formats: &[&str],
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use mooseng_benchmarks::metrics::ExportFormat;

    for format in formats {
        let export_format = match *format {
            "json" => ExportFormat::Json,
            "csv" => ExportFormat::Csv,
            "prometheus" => ExportFormat::Prometheus,
            "influxdb" => ExportFormat::InfluxDB,
            _ => {
                warn!("Unknown export format: {}", format);
                continue;
            }
        };

        let exported_data = metrics_collector.export_metrics(export_format)?;
        let filename = format!("{}/metrics.{}", output_dir, format);
        std::fs::write(&filename, exported_data)?;
        info!("Exported metrics to: {}", filename);
    }

    Ok(())
}

/// Calculate aggregated statistics from results
fn calculate_aggregated_stats(
    results: &HashMap<String, Vec<BenchmarkResult>>,
) -> HashMap<String, mooseng_benchmarks::metrics::AggregatedStats> {
    use mooseng_benchmarks::metrics::AggregatedStats;
    
    let mut aggregated = HashMap::new();

    for (category, result_vec) in results {
        for result in result_vec {
            // Create aggregated stats for each operation
            let stats = AggregatedStats {
                count: result.samples,
                mean_duration: result.mean_time,
                median_duration: result.mean_time, // Simplified - would need raw data for true median
                p95_duration: result.mean_time + result.std_dev,
                p99_duration: result.max_time,
                std_dev: result.std_dev,
                min_duration: result.min_time,
                max_duration: result.max_time,
                ops_per_second: result.throughput.unwrap_or(0.0),
                data_throughput_mbps: result.throughput,
                error_rate: 0.0, // Would need error tracking
                success_rate: 100.0,
            };

            aggregated.insert(format!("{}_{}", category, result.operation), stats);
        }
    }

    aggregated
}

/// Print comprehensive summary of benchmark results
fn print_comprehensive_summary(
    results: &[BenchmarkResult],
    metrics_summary: &mooseng_benchmarks::metrics::MetricsSummary,
    total_duration: Duration,
) {
    println!("\n{}", "=".repeat(80));
    println!("             MOOSENG COMPREHENSIVE BENCHMARK SUMMARY");
    println!("{}", "=".repeat(80));
    
    println!("Total Duration: {:?}", total_duration);
    println!("Total Operations: {}", results.len());
    println!("Peak CPU Usage: {:.1}%", metrics_summary.peak_cpu_usage);
    println!("Peak Memory Usage: {:.1} MB", metrics_summary.peak_memory_usage as f64 / 1024.0 / 1024.0);
    println!("Average Load: {:.2}", metrics_summary.average_load);
    
    println!("\n{}", "-".repeat(80));
    println!("PERFORMANCE HIGHLIGHTS");
    println!("{}", "-".repeat(80));
    
    // Find fastest and slowest operations
    if let Some(fastest) = results.iter().min_by_key(|r| r.mean_time) {
        println!("Fastest Operation: {} ({:.2}ms)", fastest.operation, fastest.mean_time.as_millis());
    }
    
    if let Some(slowest) = results.iter().max_by_key(|r| r.mean_time) {
        println!("Slowest Operation: {} ({:.2}ms)", slowest.operation, slowest.mean_time.as_millis());
    }
    
    // Find highest throughput
    if let Some(highest_throughput) = results.iter().max_by(|a, b| 
        a.throughput.partial_cmp(&b.throughput).unwrap_or(std::cmp::Ordering::Equal)) {
        if let Some(throughput) = highest_throughput.throughput {
            println!("Highest Throughput: {} ({:.2} ops/sec)", highest_throughput.operation, throughput);
        }
    }
    
    println!("\n{}", "-".repeat(80));
    println!("DETAILED RESULTS BY CATEGORY");
    println!("{}", "-".repeat(80));
    
    // Group results by category
    let mut by_category: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();
    for result in results {
        let category = if result.operation.contains("network") {
            "Network"
        } else if result.operation.contains("infrastructure") {
            "Infrastructure"
        } else if result.operation.contains("file") {
            "File Operations"
        } else if result.operation.contains("metadata") {
            "Metadata"
        } else {
            "Other"
        };
        
        by_category.entry(category.to_string()).or_insert_with(Vec::new).push(result);
    }
    
    for (category, category_results) in by_category {
        println!("\n{} ({} operations):", category, category_results.len());
        
        let avg_time: u128 = category_results.iter()
            .map(|r| r.mean_time.as_millis())
            .sum::<u128>() / category_results.len().max(1) as u128;
            
        let avg_throughput: f64 = category_results.iter()
            .filter_map(|r| r.throughput)
            .sum::<f64>() / category_results.len().max(1) as f64;
            
        println!("  Average Latency: {}ms", avg_time);
        if avg_throughput > 0.0 {
            println!("  Average Throughput: {:.2} ops/sec", avg_throughput);
        }
        
        // Show top 3 results in each category
        let mut sorted_results = category_results.clone();
        sorted_results.sort_by_key(|r| r.mean_time);
        
        for (i, result) in sorted_results.iter().take(3).enumerate() {
            println!("  {}. {} - {:.2}ms", i + 1, result.operation, result.mean_time.as_millis());
        }
    }
    
    println!("\n{}", "=".repeat(80));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_aggregated_stats() {
        let mut results = HashMap::new();
        results.insert("test".to_string(), vec![
            BenchmarkResult {
                operation: "test_op".to_string(),
                mean_time: Duration::from_millis(100),
                std_dev: Duration::from_millis(10),
                min_time: Duration::from_millis(90),
                max_time: Duration::from_millis(110),
                throughput: Some(10.0),
                samples: 5,
            }
        ]);

        let stats = calculate_aggregated_stats(&results);
        assert!(stats.contains_key("test_test_op"));
        assert_eq!(stats["test_test_op"].count, 5);
    }
}