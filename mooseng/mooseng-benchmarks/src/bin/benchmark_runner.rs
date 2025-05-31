//! Command-line tool for running comprehensive MooseNG benchmarks
//!
//! This tool orchestrates the complete benchmarking pipeline including:
//! - Real network condition testing
//! - Multi-region infrastructure deployment
//! - Performance metrics collection and analysis
//! - Report generation and visualization

use clap::{Parser, Subcommand};
use mooseng_benchmarks::{
    BenchmarkConfig, Benchmark, BenchmarkResult,
    benchmarks::{
        network_file_operations::NetworkFileBenchmark,
        multiregion::{CrossRegionReplicationBenchmark, ConsistencyBenchmark},
        file_operations::{SmallFileBenchmark, LargeFileBenchmark},
        metadata_operations::{CreateDeleteBenchmark, ListingBenchmark},
        integration::{IntegrationBenchmark, NetworkPerformanceBenchmark},
        real_network::RealNetworkBenchmark,
        multiregion_infrastructure::MultiRegionInfrastructureBenchmark,
    },
    metrics::{MetricsCollector, MetricsConfig, PerformanceAnalyzer, ExportFormat, create_metric},
    report::{ReportGenerator, ReportConfig, DashboardGenerator},
    config::InfrastructureConfig,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{info, warn, error};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "benchmark_runner")]
#[command(about = "Comprehensive benchmarking tool for MooseNG")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,
    
    /// Output directory for results
    #[arg(short, long, default_value = "./benchmark-results")]
    output: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Run real network condition tests
    Network {
        /// Network interface to use for traffic control
        #[arg(short, long, default_value = "lo")]
        interface: String,
        
        /// Test scenarios to run (comma-separated)
        #[arg(short, long)]
        scenarios: Option<String>,
        
        /// Data sizes to test (comma-separated, in bytes)
        #[arg(short, long)]
        data_sizes: Option<String>,
    },
    
    /// Deploy and test multi-region infrastructure
    Infrastructure {
        /// Configuration file for infrastructure
        #[arg(short, long)]
        config_file: Option<String>,
        
        /// Skip actual deployment (dry run)
        #[arg(long)]
        dry_run: bool,
        
        /// Regions to deploy to (comma-separated)
        #[arg(short, long)]
        regions: Option<String>,
    },
    
    /// Run complete benchmark suite
    Suite {
        /// Benchmark types to include (comma-separated)
        #[arg(short, long)]
        benchmarks: Option<String>,
        
        /// Number of iterations for each benchmark
        #[arg(short, long, default_value = "100")]
        iterations: usize,
        
        /// Enable real network testing
        #[arg(long)]
        real_network: bool,
        
        /// Enable infrastructure testing
        #[arg(long)]
        infrastructure: bool,
    },
    
    /// Analyze existing benchmark results
    Analyze {
        /// Directory containing benchmark results
        #[arg(short, long)]
        results_dir: String,
        
        /// Baseline results file for comparison
        #[arg(short, long)]
        baseline: Option<String>,
        
        /// Export format (json, csv, prometheus, influxdb)
        #[arg(short, long, default_value = "json")]
        format: String,
    },
    
    /// Generate reports from benchmark data
    Report {
        /// Input data file or directory
        #[arg(short, long)]
        input: String,
        
        /// Report format (html, text, dashboard)
        #[arg(short, long, default_value = "html")]
        format: String,
        
        /// Include charts in report
        #[arg(long)]
        charts: bool,
        
        /// Report title
        #[arg(short, long, default_value = "MooseNG Benchmark Report")]
        title: String,
    },
    
    /// Generate monitoring dashboards
    Dashboard {
        /// Dashboard type (grafana, prometheus)
        #[arg(short, long, default_value = "grafana")]
        dashboard_type: String,
        
        /// Output file for dashboard configuration
        #[arg(short, long)]
        output_file: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("benchmark_runner={},mooseng_benchmarks={}", log_level, log_level))
        .init();
    
    info!("Starting MooseNG benchmark runner");
    
    // Create output directory
    fs::create_dir_all(&cli.output)?;
    
    match cli.command {
        Commands::Network { interface, scenarios, data_sizes } => {
            run_network_benchmarks(&interface, scenarios, data_sizes, &cli.output)?;
        }
        
        Commands::Infrastructure { config_file, dry_run, regions } => {
            run_infrastructure_benchmarks(config_file, dry_run, regions, &cli.output)?;
        }
        
        Commands::Suite { benchmarks, iterations, real_network, infrastructure } => {
            run_benchmark_suite(benchmarks, iterations, real_network, infrastructure, &cli.output)?;
        }
        
        Commands::Analyze { results_dir, baseline, format } => {
            analyze_results(&results_dir, baseline, &format, &cli.output)?;
        }
        
        Commands::Report { input, format, charts, title } => {
            generate_report(&input, &format, charts, &title, &cli.output)?;
        }
        
        Commands::Dashboard { dashboard_type, output_file } => {
            generate_dashboard(&dashboard_type, &output_file)?;
        }
    }
    
    info!("Benchmark runner completed successfully");
    Ok(())
}

/// Run network condition benchmarks
fn run_network_benchmarks(
    interface: &str,
    scenarios: Option<String>,
    data_sizes: Option<String>,
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running network condition benchmarks on interface: {}", interface);
    
    let benchmark = RealNetworkBenchmark::new(interface);
    let mut config = BenchmarkConfig::default();
    
    // Parse data sizes if provided
    if let Some(sizes_str) = data_sizes {
        let sizes: Result<Vec<usize>, _> = sizes_str
            .split(',')
            .map(|s| s.trim().parse())
            .collect();
        
        match sizes {
            Ok(parsed_sizes) => config.file_sizes = parsed_sizes,
            Err(e) => {
                warn!("Failed to parse data sizes: {}. Using defaults.", e);
            }
        }
    }
    
    let results = benchmark.run(&config);
    
    // Save results
    let results_file = format!("{}/network_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&results)?;
    fs::write(&results_file, json_output)?;
    
    info!("Network benchmark results saved to: {}", results_file);
    
    // Generate quick summary
    print_benchmark_summary("Network Benchmarks", &results);
    
    Ok(())
}

/// Run infrastructure deployment benchmarks
fn run_infrastructure_benchmarks(
    config_file: Option<String>,
    dry_run: bool,
    regions: Option<String>,
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running infrastructure deployment benchmarks");
    
    let mut infra_config = if let Some(config_path) = config_file {
        if Path::new(&config_path).exists() {
            let config_str = fs::read_to_string(&config_path)?;
            serde_json::from_str(&config_str)?
        } else {
            warn!("Config file not found: {}. Using defaults.", config_path);
            InfrastructureConfig::default()
        }
    } else {
        InfrastructureConfig::default()
    };
    
    // Override regions if provided
    if let Some(regions_str) = regions {
        let region_list: Vec<String> = regions_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        
        info!("Using custom regions: {:?}", region_list);
        // Would need to update the infrastructure config with custom regions
    }
    
    if dry_run {
        info!("Dry run mode - skipping actual deployment");
        println!("Infrastructure configuration:");
        println!("{}", serde_json::to_string_pretty(&infra_config)?);
        return Ok(());
    }
    
    let benchmark = MultiRegionInfrastructureBenchmark::new(infra_config);
    let config = BenchmarkConfig::default();
    
    let results = benchmark.run(&config);
    
    // Save results
    let results_file = format!("{}/infrastructure_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&results)?;
    fs::write(&results_file, json_output)?;
    
    info!("Infrastructure benchmark results saved to: {}", results_file);
    print_benchmark_summary("Infrastructure Benchmarks", &results);
    
    Ok(())
}

/// Run complete benchmark suite
fn run_benchmark_suite(
    benchmarks: Option<String>,
    iterations: usize,
    enable_real_network: bool,
    enable_infrastructure: bool,
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running comprehensive benchmark suite");
    
    let mut config = BenchmarkConfig::default();
    config.measurement_iterations = iterations;
    
    let mut all_results = Vec::new();
    let mut metrics_collector = MetricsCollector::new(MetricsConfig::default());
    
    // Core benchmarks
    let core_benchmarks: Vec<(String, Box<dyn Benchmark>)> = vec![
        ("small_files".to_string(), Box::new(SmallFileBenchmark::new())),
        ("large_files".to_string(), Box::new(LargeFileBenchmark::new())),
        ("create_delete".to_string(), Box::new(CreateDeleteBenchmark::new())),
        ("listing".to_string(), Box::new(ListingBenchmark::new())),
        ("cross_region_replication".to_string(), Box::new(CrossRegionReplicationBenchmark::new())),
        ("consistency".to_string(), Box::new(ConsistencyBenchmark::new())),
    ];
    
    // Filter benchmarks if specified
    let benchmarks_to_run: Vec<_> = if let Some(bench_str) = benchmarks {
        let requested: Vec<&str> = bench_str.split(',').map(|s| s.trim()).collect();
        core_benchmarks
            .into_iter()
            .filter(|(name, _)| requested.contains(&name.as_str()))
            .collect()
    } else {
        core_benchmarks
    };
    
    // Run core benchmarks
    for (name, benchmark) in benchmarks_to_run {
        info!("Running benchmark: {}", name);
        let results = benchmark.run(&config);
        all_results.push((name.clone(), results));
        
        // Record metrics
        for result in &all_results.last().unwrap().1 {
            // Create metric for result
            println!("  {} - Mean: {:?}", result.operation, result.mean_time);
            let metric = create_metric(&result.operation, result.mean_time);
            metrics_collector.record_metric(metric);
        }
    }
    
    // Run real network tests if enabled
    if enable_real_network {
        info!("Running real network tests");
        let network_benchmark = RealNetworkBenchmark::new("lo");
        let network_results = network_benchmark.run(&config);
        all_results.push(("real_network".to_string(), network_results));
    }
    
    // Run infrastructure tests if enabled
    if enable_infrastructure {
        info!("Running infrastructure tests");
        let infra_config = InfrastructureConfig::default();
        let infra_benchmark = MultiRegionInfrastructureBenchmark::new(infra_config);
        let infra_results = infra_benchmark.run(&config);
        all_results.push(("infrastructure".to_string(), infra_results));
    }
    
    // Calculate aggregated statistics
    let aggregated_stats = metrics_collector.calculate_stats();
    
    // Save all results
    let results_file = format!("{}/complete_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&all_results)?;
    fs::write(&results_file, json_output)?;
    
    // Save aggregated statistics
    let stats_file = format!("{}/aggregated_statistics.json", output_dir);
    let stats_output = serde_json::to_string_pretty(&aggregated_stats)?;
    fs::write(&stats_file, stats_output)?;
    
    // Generate HTML report
    let report_config = ReportConfig {
        output_dir: output_dir.to_string(),
        generate_html: true,
        generate_charts: true,
        title: "Complete MooseNG Benchmark Suite".to_string(),
        ..Default::default()
    };
    
    let mut report_generator = ReportGenerator::new(report_config);
    let report_path = report_generator.generate_report(&all_results, &aggregated_stats, None)?;
    
    info!("Complete benchmark results saved to: {}", results_file);
    info!("Aggregated statistics saved to: {}", stats_file);
    info!("HTML report generated at: {}", report_path);
    
    // Print summary
    for (name, results) in &all_results {
        print_benchmark_summary(name, results);
    }
    
    Ok(())
}

/// Analyze existing benchmark results
fn analyze_results(
    results_dir: &str,
    baseline: Option<String>,
    format: &str,
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Analyzing benchmark results from: {}", results_dir);
    
    // Load current results
    let results_file = format!("{}/complete_benchmark_results.json", results_dir);
    if !Path::new(&results_file).exists() {
        return Err(format!("Results file not found: {}", results_file).into());
    }
    
    let results_content = fs::read_to_string(&results_file)?;
    let current_results: Vec<(String, Vec<BenchmarkResult>)> = serde_json::from_str(&results_content)?;
    
    let mut analyzer = PerformanceAnalyzer::new();
    
    // Add current results
    for (_, results) in &current_results {
        analyzer.add_results(results.clone());
    }
    
    // Load baseline if provided
    if let Some(baseline_path) = baseline {
        if Path::new(&baseline_path).exists() {
            let baseline_content = fs::read_to_string(&baseline_path)?;
            let baseline_results: Vec<BenchmarkResult> = serde_json::from_str(&baseline_content)?;
            analyzer.set_baseline(baseline_results);
        } else {
            warn!("Baseline file not found: {}", baseline_path);
        }
    }
    
    // Generate performance report
    let performance_report = analyzer.generate_comparison_report();
    
    // Export in requested format
    let export_format = match format {
        "csv" => ExportFormat::Csv,
        "prometheus" => ExportFormat::Prometheus,
        "influxdb" => ExportFormat::InfluxDB,
        _ => ExportFormat::Json,
    };
    
    let metrics_config = MetricsConfig {
        output_dir: output_dir.to_string(),
        ..Default::default()
    };
    let metrics_collector = MetricsCollector::new(metrics_config);
    let exported_data = metrics_collector.export_metrics(export_format)?;
    
    let export_file = format!("{}/analyzed_results.{}", output_dir, format);
    fs::write(&export_file, exported_data)?;
    
    // Save performance report
    let report_file = format!("{}/performance_analysis.json", output_dir);
    let report_json = serde_json::to_string_pretty(&performance_report)?;
    fs::write(&report_file, report_json)?;
    
    info!("Analysis results exported to: {}", export_file);
    info!("Performance report saved to: {}", report_file);
    
    // Print summary
    println!("\nPerformance Analysis Summary:");
    println!("Improvements: {}", performance_report.improvements.len());
    println!("Regressions: {}", performance_report.regressions.len());
    println!("Recommendations: {}", performance_report.recommendations.len());
    
    Ok(())
}

/// Generate reports from benchmark data
fn generate_report(
    input: &str,
    format: &str,
    charts: bool,
    title: &str,
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Generating {} report from: {}", format, input);
    
    // Load benchmark data
    let data_content = fs::read_to_string(input)?;
    let benchmark_results: Vec<(String, Vec<BenchmarkResult>)> = serde_json::from_str(&data_content)?;
    
    let report_config = ReportConfig {
        output_dir: output_dir.to_string(),
        generate_html: format == "html",
        generate_charts: charts,
        title: title.to_string(),
        ..Default::default()
    };
    
    let mut report_generator = ReportGenerator::new(report_config);
    let aggregated_stats = HashMap::new(); // Would need to calculate these
    
    let report_path = report_generator.generate_report(&benchmark_results, &aggregated_stats, None)?;
    
    info!("Report generated at: {}", report_path);
    Ok(())
}

/// Generate monitoring dashboards
fn generate_dashboard(
    dashboard_type: &str,
    output_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Generating {} dashboard", dashboard_type);
    
    let report_config = ReportConfig::default();
    let dashboard_generator = DashboardGenerator::new(report_config);
    
    let dashboard_config = match dashboard_type {
        "grafana" => dashboard_generator.generate_grafana_dashboard()?,
        "prometheus" => dashboard_generator.generate_prometheus_alerts()?,
        _ => return Err(format!("Unsupported dashboard type: {}", dashboard_type).into()),
    };
    
    fs::write(output_file, dashboard_config)?;
    info!("Dashboard configuration saved to: {}", output_file);
    
    Ok(())
}

/// Print a summary of benchmark results
fn print_benchmark_summary(name: &str, results: &[BenchmarkResult]) {
    println!("\n{}", "=".repeat(50));
    println!("{}", name);
    println!("{}", "=".repeat(50));
    
    for result in results {
        println!(
            "{:<30} {:>8.2}ms  {:>8} samples",
            result.operation,
            result.mean_time.as_millis(),
            result.samples
        );
        
        if let Some(throughput) = result.throughput {
            println!("{:<30} {:>8.2} ops/sec", "", throughput);
        }
    }
    
    println!();
}