//! Unified MooseNG Benchmark Runner
//!
//! This is the main CLI for running all MooseNG benchmarks in a unified manner.
//! It supports individual benchmarks, benchmark suites, live monitoring, and result storage.

use clap::{Parser, Subcommand, ValueEnum};
use mooseng_benchmarks::{
    BenchmarkConfig, Benchmark, BenchmarkResult,
    benchmarks::BenchmarkSuite,
    config,
    report::{ReportFormat, generate_report},
    analysis::BenchmarkAnalyzer,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{info, warn, error, debug};
use tracing_subscriber;
use serde_json;
use tokio;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "mooseng-bench")]
#[command(about = "Unified MooseNG Benchmark Suite")]
#[command(version = "1.0.0")]
#[command(long_about = "A comprehensive benchmarking tool for MooseNG distributed file system. 
Supports individual benchmarks, full suites, real-time monitoring, and historical analysis.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,
    
    /// Output directory for results
    #[arg(short, long, default_value = "./benchmark_results")]
    output: String,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Quiet mode (minimal output)
    #[arg(short, long)]
    quiet: bool,
    
    /// Number of parallel benchmark workers
    #[arg(short, long, default_value = "1")]
    workers: usize,
    
    /// Export format for results
    #[arg(long, value_enum, default_value = "json")]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all available benchmarks
    All {
        /// Quick mode (fewer iterations, smaller test set)
        #[arg(short, long)]
        quick: bool,
        
        /// Skip expensive benchmarks (large files, high concurrency)
        #[arg(long)]
        skip_expensive: bool,
        
        /// Continue on benchmark failures
        #[arg(long)]
        continue_on_error: bool,
    },
    
    /// Run specific benchmark categories
    Category {
        /// Category name (files, metadata, network, multiregion, comparison)
        categories: Vec<String>,
        
        /// Custom configuration for this run
        #[arg(short, long)]
        config_override: Option<String>,
    },
    
    /// Run individual benchmarks by name
    Benchmark {
        /// Benchmark names to run
        names: Vec<String>,
        
        /// Number of iterations
        #[arg(short, long)]
        iterations: Option<usize>,
        
        /// File sizes to test (comma separated, in bytes)
        #[arg(long)]
        file_sizes: Option<String>,
        
        /// Concurrency levels to test (comma separated)
        #[arg(long)]
        concurrency: Option<String>,
    },
    
    /// Run benchmarks with live monitoring
    Monitor {
        /// Refresh interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
        
        /// Duration to run in minutes (0 = infinite)
        #[arg(short, long, default_value = "0")]
        duration: u64,
        
        /// Enable performance alerts
        #[arg(long)]
        alerts: bool,
    },
    
    /// List available benchmarks and categories
    List {
        /// Show detailed descriptions
        #[arg(short, long)]
        detailed: bool,
        
        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,
    },
    
    /// Analyze existing benchmark results
    Analyze {
        /// Path to results file or directory
        path: String,
        
        /// Generate comparison report
        #[arg(short, long)]
        compare: bool,
        
        /// Detect performance regressions
        #[arg(long)]
        regression: bool,
        
        /// Baseline results for comparison
        #[arg(long)]
        baseline: Option<String>,
    },
    
    /// Start the benchmark dashboard server
    Dashboard {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Host to bind to
        #[arg(long, default_value = "localhost")]
        host: String,
        
        /// Enable real-time streaming
        #[arg(long)]
        realtime: bool,
        
        /// Database connection string
        #[arg(long)]
        database: Option<String>,
    },
    
    /// Generate configuration templates
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Generate default configuration file
    Generate {
        /// Output path for configuration
        #[arg(short, long, default_value = "benchmark_config.toml")]
        output: String,
        
        /// Include advanced options
        #[arg(long)]
        advanced: bool,
    },
    
    /// Validate configuration file
    Validate {
        /// Configuration file to validate
        path: String,
    },
    
    /// Show current configuration
    Show {
        /// Show resolved configuration with defaults
        #[arg(long)]
        resolved: bool,
    },
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Json,
    Csv,
    Html,
    Markdown,
    Prometheus,
}

#[derive(Clone)]
struct BenchmarkProgress {
    name: String,
    completed: usize,
    total: usize,
    current_result: Option<BenchmarkResult>,
    elapsed: Duration,
}

struct LiveMonitor {
    tx: mpsc::UnboundedSender<BenchmarkProgress>,
    interval: Duration,
    alerts_enabled: bool,
}

impl LiveMonitor {
    fn new(interval: Duration, alerts_enabled: bool) -> (Self, mpsc::UnboundedReceiver<BenchmarkProgress>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx, interval, alerts_enabled }, rx)
    }
    
    async fn send_progress(&self, progress: BenchmarkProgress) {
        let _ = self.tx.send(progress);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.quiet {
        "warn"
    } else if cli.verbose {
        "debug"
    } else {
        "info"
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(format!("mooseng_bench={},mooseng_benchmarks={}", log_level, log_level))
        .with_target(false)
        .init();
    
    // Create output directory
    fs::create_dir_all(&cli.output)?;
    
    // Load configuration
    let config = load_configuration(cli.config.as_deref()).await?;
    
    let start_time = Instant::now();
    
    let result = match cli.command {
        Commands::All { quick, skip_expensive, continue_on_error } => {
            run_all_benchmarks(&cli, &config, quick, skip_expensive, continue_on_error).await
        },
        Commands::Category { categories, config_override } => {
            run_category_benchmarks(&cli, &config, categories, config_override).await
        },
        Commands::Benchmark { names, iterations, file_sizes, concurrency } => {
            run_specific_benchmarks(&cli, &config, names, iterations, file_sizes, concurrency).await
        },
        Commands::Monitor { interval, duration, alerts } => {
            run_live_monitoring(&cli, &config, interval, duration, alerts).await
        },
        Commands::List { detailed, category } => {
            list_benchmarks(detailed, category).await
        },
        Commands::Analyze { path, compare, regression, baseline } => {
            analyze_results(path, compare, regression, baseline).await
        },
        Commands::Dashboard { port, host, realtime, database } => {
            start_dashboard_server(port, host, realtime, database).await
        },
        Commands::Config { action } => {
            handle_config_command(action).await
        },
    };
    
    let total_time = start_time.elapsed();
    
    match result {
        Ok(_) => {
            if !cli.quiet {
                info!("✅ Completed in {:.2}s", total_time.as_secs_f64());
            }
        },
        Err(e) => {
            error!("❌ Failed: {}", e);
            std::process::exit(1);
        }
    }
    
    Ok(())
}

async fn load_configuration(config_path: Option<&str>) -> Result<BenchmarkConfig, Box<dyn std::error::Error>> {
    if let Some(path) = config_path {
        debug!("Loading configuration from {}", path);
        config::load_from_file(path).await
    } else {
        debug!("Using default configuration");
        Ok(BenchmarkConfig::default())
    }
}

async fn run_all_benchmarks(
    cli: &Cli,
    config: &BenchmarkConfig,
    quick: bool,
    skip_expensive: bool,
    continue_on_error: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🚀 Running comprehensive MooseNG benchmark suite");
    
    let mut benchmark_config = config.clone();
    
    if quick {
        benchmark_config.warmup_iterations = 5;
        benchmark_config.measurement_iterations = benchmark_config.measurement_iterations.min(20);
        benchmark_config.file_sizes = vec![1024, 65536, 1048576]; // 1KB, 64KB, 1MB
        benchmark_config.concurrency_levels = vec![1, 10];
        benchmark_config.detailed_report = false;
    }
    
    if skip_expensive {
        // Remove large files and high concurrency
        benchmark_config.file_sizes.retain(|&size| size <= 10 * 1024 * 1024); // Max 10MB
        benchmark_config.concurrency_levels.retain(|&level| level <= 50);
    }
    
    let suite = BenchmarkSuite::new();
    let mut all_results = HashMap::new();
    let mut failed_benchmarks = Vec::new();
    
    // Run benchmarks with progress tracking
    if cli.workers > 1 {
        run_parallel_benchmarks(&suite, &benchmark_config, cli.workers, &mut all_results, &mut failed_benchmarks, continue_on_error).await?;
    } else {
        run_sequential_benchmarks(&suite, &benchmark_config, &mut all_results, &mut failed_benchmarks, continue_on_error).await?;
    }
    
    // Save and analyze results
    save_results(&all_results, &cli.output, &cli.format).await?;
    print_comprehensive_summary(&all_results, &failed_benchmarks);
    
    if !failed_benchmarks.is_empty() && !continue_on_error {
        return Err(format!("❌ {} benchmarks failed", failed_benchmarks.len()).into());
    }
    
    Ok(())
}

async fn run_sequential_benchmarks(
    suite: &BenchmarkSuite,
    config: &BenchmarkConfig,
    all_results: &mut HashMap<String, Vec<BenchmarkResult>>,
    failed_benchmarks: &mut Vec<String>,
    continue_on_error: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let benchmark_results = suite.run_all(config);
    
    for (name, results) in benchmark_results {
        if results.is_empty() && !continue_on_error {
            let error = format!("Benchmark '{}' produced no results", name);
            error!("{}", error);
            failed_benchmarks.push(name.clone());
            if !continue_on_error {
                return Err(error.into());
            }
        } else {
            info!("✅ {} completed ({} results)", name, results.len());
            all_results.insert(name, results);
        }
    }
    
    Ok(())
}

async fn run_parallel_benchmarks(
    suite: &BenchmarkSuite,
    config: &BenchmarkConfig,
    workers: usize,
    all_results: &mut HashMap<String, Vec<BenchmarkResult>>,
    failed_benchmarks: &mut Vec<String>,
    continue_on_error: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Running benchmarks with {} parallel workers", workers);
    
    // For now, run sequentially but with async support
    // TODO: Implement true parallel execution with proper resource isolation
    run_sequential_benchmarks(suite, config, all_results, failed_benchmarks, continue_on_error).await
}

async fn run_category_benchmarks(
    cli: &Cli,
    config: &BenchmarkConfig,
    categories: Vec<String>,
    _config_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎯 Running benchmark categories: {}", categories.join(", "));
    
    let suite = BenchmarkSuite::new();
    let mut results = HashMap::new();
    
    for category in &categories {
        info!("Running {} benchmarks...", category);
        
        // Map category names to specific benchmark runs
        let category_results = match category.as_str() {
            "files" | "file-operations" => {
                suite.run_selected(&["small_files", "large_files", "random_access"], config)
            },
            "metadata" => {
                suite.run_selected(&["create_delete", "directory_listing", "file_attributes"], config)
            },
            "network" => {
                suite.run_selected(&["network_files", "network_latency"], config)
            },
            "multiregion" => {
                suite.run_selected(&["cross_region_replication", "consistency", "multiregion_performance"], config)
            },
            "comparison" => {
                suite.run_selected(&["moosefs_comparison"], config)
            },
            _ => {
                warn!("Unknown category: {}", category);
                continue;
            }
        };
        
        for (name, benchmark_results) in category_results {
            results.insert(format!("{}_{}", category, name), benchmark_results);
        }
    }
    
    save_results(&results, &cli.output, &cli.format).await?;
    print_comprehensive_summary(&results, &Vec::new());
    
    Ok(())
}

async fn run_specific_benchmarks(
    cli: &Cli,
    config: &BenchmarkConfig,
    names: Vec<String>,
    iterations: Option<usize>,
    file_sizes: Option<String>,
    concurrency: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎯 Running specific benchmarks: {}", names.join(", "));
    
    let mut benchmark_config = config.clone();
    
    // Apply overrides
    if let Some(iter) = iterations {
        benchmark_config.measurement_iterations = iter;
    }
    
    if let Some(sizes_str) = file_sizes {
        let sizes: Result<Vec<usize>, _> = sizes_str
            .split(',')
            .map(|s| s.trim().parse::<usize>())
            .collect();
        match sizes {
            Ok(sizes) => benchmark_config.file_sizes = sizes,
            Err(e) => return Err(format!("Invalid file sizes: {}", e).into()),
        }
    }
    
    if let Some(conc_str) = concurrency {
        let levels: Result<Vec<usize>, _> = conc_str
            .split(',')
            .map(|s| s.trim().parse::<usize>())
            .collect();
        match levels {
            Ok(levels) => benchmark_config.concurrency_levels = levels,
            Err(e) => return Err(format!("Invalid concurrency levels: {}", e).into()),
        }
    }
    
    let suite = BenchmarkSuite::new();
    let results = suite.run_selected(&names.iter().map(|s| s.as_str()).collect::<Vec<_>>(), &benchmark_config);
    
    let results_map: HashMap<String, Vec<BenchmarkResult>> = results.into_iter().collect();
    
    save_results(&results_map, &cli.output, &cli.format).await?;
    print_comprehensive_summary(&results_map, &Vec::new());
    
    Ok(())
}

async fn run_live_monitoring(
    _cli: &Cli,
    _config: &BenchmarkConfig,
    interval: u64,
    duration: u64,
    alerts: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Starting live benchmark monitoring (interval: {}s)", interval);
    
    let (monitor, mut rx) = LiveMonitor::new(
        Duration::from_secs(interval),
        alerts,
    );
    
    let end_time = if duration > 0 {
        Some(Instant::now() + Duration::from_secs(duration * 60))
    } else {
        None
    };
    
    // Start monitoring display
    tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            // Clear screen and display current status
            print!("\x1B[2J\x1B[1;1H");
            println!("🔄 MooseNG Live Benchmark Monitor");
            println!("{}", "=".repeat(60));
            println!("Benchmark: {}", progress.name);
            println!("Progress: {}/{} ({:.1}%)", 
                     progress.completed, 
                     progress.total,
                     (progress.completed as f64 / progress.total.max(1) as f64) * 100.0);
            println!("Elapsed: {:.2}s", progress.elapsed.as_secs_f64());
            
            if let Some(result) = &progress.current_result {
                println!("Current: {} - {:.3}ms", 
                         result.operation, 
                         result.mean_time.as_secs_f64() * 1000.0);
                if let Some(throughput) = result.throughput {
                    println!("Throughput: {:.2} MB/s", throughput);
                }
            }
            
            println!("\nPress Ctrl+C to stop monitoring");
        }
    });
    
    // Continuous benchmark execution
    loop {
        if let Some(end) = end_time {
            if Instant::now() >= end {
                info!("⏰ Monitoring duration completed");
                break;
            }
        }
        
        // Run a lightweight benchmark cycle
        let suite = BenchmarkSuite::new();
        let quick_config = BenchmarkConfig {
            warmup_iterations: 2,
            measurement_iterations: 10,
            file_sizes: vec![4096, 65536],
            concurrency_levels: vec![1, 10],
            regions: vec!["local".to_string()],
            detailed_report: false,
        };
        
        let results = suite.run_selected(&["small_files"], &quick_config);
        
        for (name, benchmark_results) in results {
            for (i, result) in benchmark_results.iter().enumerate() {
                let progress = BenchmarkProgress {
                    name: name.clone(),
                    completed: i + 1,
                    total: benchmark_results.len(),
                    current_result: Some(result.clone()),
                    elapsed: Duration::from_secs(interval * (i as u64 + 1)),
                };
                
                monitor.send_progress(progress).await;
            }
        }
        
        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
    
    Ok(())
}

async fn list_benchmarks(detailed: bool, category: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Available MooseNG Benchmarks\n");
    
    // Define benchmark categories and their benchmarks
    let categories = vec![
        ("File Operations", vec![
            ("small_files", "Small file read/write performance"),
            ("large_files", "Large file sequential I/O"),
            ("random_access", "Random access patterns"),
        ]),
        ("Metadata Operations", vec![
            ("create_delete", "File/directory creation and deletion"),
            ("directory_listing", "Directory listing performance"),
            ("file_attributes", "Attribute operations"),
        ]),
        ("Network Operations", vec![
            ("network_files", "Network file operations"),
            ("network_latency", "Multi-region latency"),
        ]),
        ("Multi-Region", vec![
            ("cross_region_replication", "Cross-region data replication"),
            ("consistency", "Consistency guarantees"),
            ("multiregion_performance", "Multi-region performance"),
        ]),
        ("Comparison", vec![
            ("moosefs_comparison", "Comparison with MooseFS"),
        ]),
    ];
    
    for (cat_name, benchmarks) in &categories {
        if let Some(ref filter) = category {
            if !cat_name.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }
        
        println!("🗂️  {}", cat_name);
        println!("{}", "-".repeat(cat_name.len() + 4));
        
        for (name, description) in benchmarks {
            if detailed {
                println!("  {} - {}", name, description);
            } else {
                println!("  {}", name);
            }
        }
        println!();
    }
    
    if detailed {
        println!("💡 Usage examples:");
        println!("  mooseng-bench all --quick");
        println!("  mooseng-bench category files metadata");
        println!("  mooseng-bench benchmark small_files large_files --iterations 100");
        println!("  mooseng-bench monitor --interval 10 --duration 30");
    }
    
    Ok(())
}

async fn analyze_results(
    path: String,
    compare: bool,
    regression: bool,
    baseline: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Analyzing benchmark results from: {}", path);
    
    let analyzer = BenchmarkAnalyzer::new();
    
    // Load results
    let results = if Path::new(&path).is_file() {
        analyzer.load_results_from_file(&path).await?
    } else {
        analyzer.load_results_from_directory(&path).await?
    };
    
    // Basic analysis
    let analysis = analyzer.analyze(&results);
    println!("{}", analysis.summary);
    
    if compare || regression {
        if let Some(baseline_path) = baseline {
            let baseline_results = analyzer.load_results_from_file(&baseline_path).await?;
            let comparison = analyzer.compare(&baseline_results, &results);
            println!("\n📈 Comparison Results:");
            println!("{}", comparison.summary);
            
            if regression {
                let regressions = analyzer.detect_regressions(&comparison);
                if !regressions.is_empty() {
                    println!("\n⚠️  Performance Regressions Detected:");
                    for regression in regressions {
                        println!("  {}", regression);
                    }
                } else {
                    println!("\n✅ No performance regressions detected");
                }
            }
        } else {
            warn!("Comparison/regression analysis requires --baseline argument");
        }
    }
    
    Ok(())
}

async fn start_dashboard_server(
    port: u16,
    host: String,
    realtime: bool,
    database: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🌐 Starting benchmark dashboard server on {}:{}", host, port);
    
    if realtime {
        info!("📡 Real-time streaming enabled");
    }
    
    if let Some(db) = database {
        info!("💾 Using database: {}", db);
    }
    
    // TODO: Implement actual dashboard server
    // For now, just show placeholder
    println!("Dashboard would be available at: http://{}:{}", host, port);
    println!("Features:");
    println!("  - Real-time benchmark monitoring");
    println!("  - Historical result visualization");
    println!("  - Performance trend analysis");
    println!("  - Regression detection");
    
    // Keep server running
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn handle_config_command(action: ConfigCommands) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigCommands::Generate { output, advanced } => {
            info!("📄 Generating configuration file: {}", output);
            let config = if advanced {
                config::generate_advanced_config()
            } else {
                config::generate_default_config()
            };
            
            config::save_to_file(&config, &output).await?;
            println!("✅ Configuration saved to: {}", output);
        },
        ConfigCommands::Validate { path } => {
            info!("✅ Validating configuration: {}", path);
            match config::validate_file(&path).await {
                Ok(_) => println!("✅ Configuration is valid"),
                Err(e) => return Err(format!("❌ Configuration validation failed: {}", e).into()),
            }
        },
        ConfigCommands::Show { resolved } => {
            let config = if resolved {
                config::load_with_defaults().await?
            } else {
                BenchmarkConfig::default()
            };
            
            println!("📋 Current Configuration:");
            println!("{}", toml::to_string_pretty(&config)?);
        },
    }
    
    Ok(())
}

async fn save_results(
    results: &HashMap<String, Vec<BenchmarkResult>>,
    output_dir: &str,
    format: &OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    
    match format {
        OutputFormat::Json => {
            let path = Path::new(output_dir).join(format!("results_{}.json", timestamp));
            let json = serde_json::to_string_pretty(results)?;
            fs::write(&path, json)?;
            info!("💾 Results saved to: {}", path.display());
        },
        OutputFormat::Csv => {
            let path = Path::new(output_dir).join(format!("results_{}.csv", timestamp));
            generate_report(results, ReportFormat::Csv, &path).await?;
            info!("💾 Results saved to: {}", path.display());
        },
        OutputFormat::Html => {
            let path = Path::new(output_dir).join(format!("results_{}.html", timestamp));
            generate_report(results, ReportFormat::Html, &path).await?;
            info!("💾 Results saved to: {}", path.display());
        },
        OutputFormat::Markdown => {
            let path = Path::new(output_dir).join(format!("results_{}.md", timestamp));
            generate_report(results, ReportFormat::Markdown, &path).await?;
            info!("💾 Results saved to: {}", path.display());
        },
        OutputFormat::Prometheus => {
            let path = Path::new(output_dir).join(format!("results_{}.prom", timestamp));
            generate_report(results, ReportFormat::Prometheus, &path).await?;
            info!("💾 Results saved to: {}", path.display());
        },
    }
    
    Ok(())
}

fn print_comprehensive_summary(results: &HashMap<String, Vec<BenchmarkResult>>, failed: &[String]) {
    println!("\n{}", "=".repeat(80));
    println!("🎯 BENCHMARK EXECUTION SUMMARY");
    println!("{}", "=".repeat(80));
    
    let total_tests: usize = results.values().map(|v| v.len()).sum();
    let total_samples: usize = results.values()
        .flat_map(|v| v.iter())
        .map(|r| r.samples)
        .sum();
    
    println!("📊 Execution Statistics:");
    println!("   Total Tests: {}", total_tests);
    println!("   Total Samples: {}", total_samples);
    println!("   Failed Benchmarks: {}", failed.len());
    
    if !failed.is_empty() {
        println!("   Failed: {}", failed.join(", "));
    }
    
    // Performance summary
    let all_results: Vec<&BenchmarkResult> = results.values()
        .flat_map(|v| v.iter())
        .collect();
    
    if !all_results.is_empty() {
        println!("\n🚀 Performance Summary:");
        
        let avg_latency: f64 = all_results.iter()
            .map(|r| r.mean_time.as_secs_f64() * 1000.0)
            .sum::<f64>() / all_results.len() as f64;
        
        let throughput_results: Vec<f64> = all_results.iter()
            .filter_map(|r| r.throughput)
            .collect();
        
        println!("   Average Latency: {:.2} ms", avg_latency);
        
        if !throughput_results.is_empty() {
            let avg_throughput = throughput_results.iter().sum::<f64>() / throughput_results.len() as f64;
            let max_throughput = throughput_results.iter().fold(0.0, |a, &b| a.max(b));
            
            println!("   Average Throughput: {:.2} MB/s", avg_throughput);
            println!("   Peak Throughput: {:.2} MB/s", max_throughput);
        }
        
        // Performance grade
        let grade = calculate_performance_grade(avg_latency, throughput_results.get(0).copied().unwrap_or(0.0));
        println!("   Overall Grade: {}", grade);
    }
    
    println!("\n{}", "=".repeat(80));
}

fn calculate_performance_grade(avg_latency_ms: f64, avg_throughput_mbps: f64) -> String {
    let mut score = 100.0;
    
    // Latency scoring
    if avg_latency_ms > 100.0 {
        score -= 30.0;
    } else if avg_latency_ms > 50.0 {
        score -= 20.0;
    } else if avg_latency_ms > 20.0 {
        score -= 10.0;
    }
    
    // Throughput scoring
    if avg_throughput_mbps > 1000.0 {
        score += 10.0;
    } else if avg_throughput_mbps > 500.0 {
        score += 5.0;
    } else if avg_throughput_mbps < 50.0 {
        score -= 20.0;
    } else if avg_throughput_mbps < 100.0 {
        score -= 10.0;
    }
    
    match score as i32 {
        90..=100 => "A+ (Excellent)".to_string(),
        80..=89 => "A (Very Good)".to_string(),
        70..=79 => "B+ (Good)".to_string(),
        60..=69 => "B (Fair)".to_string(),
        50..=59 => "C (Below Average)".to_string(),
        _ => "D (Needs Improvement)".to_string(),
    }
}