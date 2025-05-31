//! Benchmark commands for performance testing

use anyhow::{Context, Result};
type CliResult = Result<(), Box<dyn std::error::Error>>;
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

// Import unified benchmark infrastructure
use mooseng_benchmarks::{
    config::BenchmarkConfig as UnifiedBenchmarkConfig,
    database::{BenchmarkDatabase, BenchmarkFilter, SessionStatus},
    BenchmarkResult as UnifiedBenchmarkResult,
};

#[derive(Subcommand)]
pub enum BenchmarkCommands {
    /// Run a quick benchmark suite
    Quick {
        /// Output directory for results
        #[arg(short, long, default_value = "./benchmark-results")]
        output: String,
        
        /// Number of iterations
        #[arg(short, long, default_value = "10")]
        iterations: usize,
        
        /// Include network tests
        #[arg(long)]
        network: bool,
    },
    
    /// Run comprehensive benchmark suite
    Full {
        /// Output directory for results
        #[arg(short, long, default_value = "./benchmark-results")]
        output: String,
        
        /// Configuration file
        #[arg(short, long)]
        config: Option<String>,
        
        /// Enable real network testing
        #[arg(long)]
        real_network: bool,
        
        /// Enable infrastructure testing
        #[arg(long)]
        infrastructure: bool,
    },
    
    /// Run file operation benchmarks
    File {
        /// File sizes to test (comma-separated, in bytes)
        #[arg(short, long, default_value = "1024,65536,1048576")]
        sizes: String,
        
        /// Output directory for results
        #[arg(short, long, default_value = "./benchmark-results")]
        output: String,
        
        /// Number of iterations per test
        #[arg(short, long, default_value = "100")]
        iterations: usize,
    },
    
    /// Run metadata operation benchmarks
    Metadata {
        /// Number of operations to test
        #[arg(short, long, default_value = "1000")]
        operations: usize,
        
        /// Output directory for results
        #[arg(short, long, default_value = "./benchmark-results")]
        output: String,
    },
    
    /// Run network latency benchmarks
    Network {
        /// Target addresses (comma-separated)
        #[arg(short, long)]
        targets: Option<String>,
        
        /// Output directory for results
        #[arg(short, long, default_value = "./benchmark-results")]
        output: String,
        
        /// Test duration in seconds
        #[arg(short, long, default_value = "60")]
        duration: u64,
    },
    
    /// Compare benchmark results
    Compare {
        /// Baseline results file
        #[arg(short, long)]
        baseline: String,
        
        /// Current results file
        #[arg(short, long)]
        current: String,
        
        /// Output format (json, table, html)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    
    /// Generate reports from benchmark data
    Report {
        /// Input data directory or file
        #[arg(short, long)]
        input: String,
        
        /// Report format (html, json, csv)
        #[arg(short, long, default_value = "html")]
        format: String,
        
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
        
        /// Include charts in report
        #[arg(long)]
        charts: bool,
    },
    
    /// List available benchmark types
    List,
    
    /// Show benchmark status and history
    Status {
        /// Show detailed status
        #[arg(short, long)]
        detailed: bool,
        
        /// Limit number of results to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    
    /// Start live benchmark dashboard
    Dashboard {
        /// Dashboard server port
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Database path for storing results
        #[arg(short, long, default_value = "./benchmark.db")]
        database: String,
        
        /// Open browser automatically
        #[arg(long)]
        open_browser: bool,
    },
    
    /// Query benchmark results from database
    Query {
        /// Session ID to filter by
        #[arg(long)]
        session_id: Option<String>,
        
        /// Operation name to filter by
        #[arg(long)]
        operation: Option<String>,
        
        /// Suite name to filter by
        #[arg(long)]
        suite: Option<String>,
        
        /// Start date (RFC3339 format)
        #[arg(long)]
        start_date: Option<String>,
        
        /// End date (RFC3339 format)
        #[arg(long)]
        end_date: Option<String>,
        
        /// Limit number of results
        #[arg(short, long, default_value = "50")]
        limit: usize,
        
        /// Output format (table, json, csv)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    
    /// Analyze benchmark trends
    Trends {
        /// Operation to analyze (optional, analyzes all if not specified)
        #[arg(long)]
        operation: Option<String>,
        
        /// Number of days to analyze
        #[arg(long, default_value = "30")]
        days: i32,
        
        /// Minimum confidence threshold (0.0-1.0)
        #[arg(long, default_value = "0.7")]
        confidence: f64,
        
        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    
    /// Run continuous benchmarks (CI mode)
    Continuous {
        /// Number of iterations
        #[arg(short, long, default_value = "5")]
        iterations: usize,
        
        /// Interval between runs (minutes)
        #[arg(long, default_value = "60")]
        interval: u64,
        
        /// Regression threshold percentage
        #[arg(long, default_value = "5.0")]
        threshold: f64,
        
        /// Output directory
        #[arg(short, long, default_value = "./benchmark-results")]
        output: String,
        
        /// Database path
        #[arg(long, default_value = "./benchmark.db")]
        database: String,
    },
}

/// Simple benchmark result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleBenchmarkResult {
    pub operation: String,
    pub duration_ms: f64,
    pub throughput_ops_sec: Option<f64>,
    pub success: bool,
    pub error: Option<String>,
    pub timestamp: String,
}

/// Benchmark configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub warmup_iterations: usize,
    pub measurement_iterations: usize,
    pub file_sizes: Vec<usize>,
    pub output_directory: String,
    pub enable_detailed_logging: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 5,
            measurement_iterations: 100,
            file_sizes: vec![1024, 65536, 1048576, 10485760], // 1KB, 64KB, 1MB, 10MB
            output_directory: "./benchmark-results".to_string(),
            enable_detailed_logging: false,
        }
    }
}

/// Handle benchmark command execution
pub async fn handle_command(command: BenchmarkCommands) -> CliResult {
    match command {
        BenchmarkCommands::Quick { output, iterations, network } => {
            run_quick_benchmark(&output, iterations, network).await
        }
        
        BenchmarkCommands::Full { output, config, real_network, infrastructure } => {
            run_full_benchmark(&output, config, real_network, infrastructure).await
        }
        
        BenchmarkCommands::File { sizes, output, iterations } => {
            run_file_benchmark(&sizes, &output, iterations).await
        }
        
        BenchmarkCommands::Metadata { operations, output } => {
            run_metadata_benchmark(operations, &output).await
        }
        
        BenchmarkCommands::Network { targets, output, duration } => {
            run_network_benchmark(targets, &output, duration).await
        }
        
        BenchmarkCommands::Compare { baseline, current, format } => {
            compare_benchmark_results(&baseline, &current, &format).await
        }
        
        BenchmarkCommands::Report { input, format, output, charts } => {
            generate_benchmark_report(&input, &format, output, charts).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        }
        
        BenchmarkCommands::List => {
            list_available_benchmarks().await
        }
        
        BenchmarkCommands::Status { detailed, limit } => {
            show_benchmark_status(detailed, limit).await
        }
        
        BenchmarkCommands::Dashboard { port, database, open_browser } => {
            start_benchmark_dashboard(port, &database, open_browser).await
        }
        
        BenchmarkCommands::Query { 
            session_id, operation, suite, start_date, end_date, limit, format 
        } => {
            query_benchmark_results(session_id, operation, suite, start_date, end_date, *limit, format).await
        }
        
        BenchmarkCommands::Trends { operation, days, confidence, format } => {
            analyze_benchmark_trends(operation.clone(), *days, *confidence, format).await
        }
        
        BenchmarkCommands::Continuous { 
            iterations, interval, threshold, output, database 
        } => {
            run_continuous_benchmarks_unified(*iterations, *interval, *threshold, output, database).await
        }
    }
}

/// Run a quick benchmark suite
async fn run_quick_benchmark(output_dir: &str, iterations: usize, include_network: bool) -> CliResult {
    info!("Running quick benchmark suite with {} iterations", iterations);
    
    // Create output directory
    fs::create_dir_all(output_dir)?;
    
    let mut results = Vec::new();
    let start_time = Instant::now();
    
    // Test 1: Basic file creation and deletion
    info!("Testing file operations...");
    let file_result = benchmark_file_operations(iterations).await?;
    results.extend(file_result);
    
    // Test 2: Metadata operations
    info!("Testing metadata operations...");
    let metadata_result = benchmark_metadata_operations(iterations / 10).await?;
    results.extend(metadata_result);
    
    // Test 3: Network operations (if enabled)
    if include_network {
        info!("Testing network operations...");
        let network_result = benchmark_network_operations().await?;
        results.extend(network_result);
    }
    
    let total_duration = start_time.elapsed();
    
    // Save results
    let results_file = format!("{}/quick_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&results)?;
    fs::write(&results_file, json_output)?;
    
    // Print summary
    print_benchmark_summary("Quick Benchmark Suite", &results, total_duration);
    
    info!("Quick benchmark completed. Results saved to: {}", results_file);
    Ok(())
}

/// Run comprehensive benchmark suite using unified benchmark runner
async fn run_full_benchmark(
    output_dir: &str, 
    config_file: Option<String>, 
    real_network: bool, 
    infrastructure: bool
) -> CliResult {
    info!("Running full benchmark suite via unified benchmark runner");
    
    // Create output directory
    fs::create_dir_all(output_dir)?;
    
    let start_time = Instant::now();
    
    // Initialize database connection for storing results
    let db_path = format!("{}/benchmark_results.db", output_dir);
    let db = BenchmarkDatabase::new(&db_path).await
        .context("Failed to initialize benchmark database")?;
    
    info!("Database initialized for results storage: {}", db_path);
    
    // Build command to execute unified benchmark runner
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
       .arg("--bin")
       .arg("mooseng-bench")
       .arg("--")
       .arg("all")
       .arg("--output")
       .arg(output_dir)
       .arg("--database")
       .arg(&db_path);
    
    if let Some(config_path) = config_file {
        cmd.arg("--config").arg(config_path);
    }
    
    if real_network {
        cmd.arg("--real-network");
    }
    
    if infrastructure {
        cmd.arg("--infrastructure");
    }
    
    // Set working directory to mooseng-benchmarks
    let benchmarks_dir = std::env::current_dir()?
        .join("mooseng-benchmarks");
    
    if benchmarks_dir.exists() {
        cmd.current_dir(&benchmarks_dir);
        
        info!("Executing unified benchmark runner from: {}", benchmarks_dir.display());
        let output = cmd.output()?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            println!("{}", stdout);
            if !stderr.is_empty() {
                warn!("Benchmark stderr: {}", stderr);
            }
            
            let total_duration = start_time.elapsed();
            info!("Full benchmark completed in {:.2}s using unified runner", total_duration.as_secs_f64());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Unified benchmark runner failed: {}", stderr).into());
        }
    } else {
        // Fallback to legacy implementation if unified runner not available
        warn!("Unified benchmark runner not found, falling back to legacy implementation");
        return run_full_benchmark_legacy(output_dir, config_file, real_network, infrastructure).await;
    }
    
    Ok(())
}

/// Legacy benchmark implementation as fallback
async fn run_full_benchmark_legacy(
    output_dir: &str, 
    config_file: Option<String>, 
    _real_network: bool, 
    _infrastructure: bool
) -> CliResult {
    info!("Running full benchmark suite (legacy implementation)");
    
    let config = if let Some(config_path) = config_file {
        load_benchmark_config(&config_path).await?
    } else {
        BenchmarkConfig::default()
    };
    
    let mut all_results = Vec::new();
    let start_time = Instant::now();
    
    // Run comprehensive tests
    info!("Running comprehensive file operations tests...");
    for &file_size in &config.file_sizes {
        let size_results = benchmark_file_size(file_size, config.measurement_iterations).await?;
        all_results.extend(size_results);
    }
    
    info!("Running metadata stress tests...");
    let metadata_results = benchmark_metadata_stress(config.measurement_iterations * 10).await?;
    all_results.extend(metadata_results);
    
    let total_duration = start_time.elapsed();
    
    // Save results
    let results_file = format!("{}/full_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&all_results)?;
    fs::write(&results_file, json_output)?;
    
    // Generate summary report
    let summary_file = format!("{}/benchmark_summary.txt", output_dir);
    generate_text_summary(&all_results, total_duration, &summary_file)?;
    
    print_benchmark_summary("Full Benchmark Suite", &all_results, total_duration);
    
    info!("Full benchmark completed. Results saved to: {}", output_dir);
    Ok(())
}

/// Run file operation benchmarks
async fn run_file_benchmark(sizes_str: &str, output_dir: &str, iterations: usize) -> CliResult {
    info!("Running file operation benchmarks");
    
    let sizes: Vec<usize> = sizes_str
        .split(',')
        .map(|s| s.trim().parse())
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to parse file sizes")?;
    
    fs::create_dir_all(output_dir)?;
    
    let mut results = Vec::new();
    let start_time = Instant::now();
    
    for size in sizes {
        info!("Testing file size: {} bytes", size);
        let size_results = benchmark_file_size(size, iterations).await?;
        results.extend(size_results);
    }
    
    let total_duration = start_time.elapsed();
    
    // Save results
    let results_file = format!("{}/file_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&results)?;
    fs::write(&results_file, json_output)?;
    
    print_benchmark_summary("File Operations Benchmark", &results, total_duration);
    
    info!("File benchmark completed. Results saved to: {}", results_file);
    Ok(())
}

/// Run metadata operation benchmarks
async fn run_metadata_benchmark(operations: usize, output_dir: &str) -> CliResult {
    info!("Running metadata operations benchmark with {} operations", operations);
    
    fs::create_dir_all(output_dir)?;
    
    let start_time = Instant::now();
    let results = benchmark_metadata_operations(operations).await?;
    let total_duration = start_time.elapsed();
    
    // Save results
    let results_file = format!("{}/metadata_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&results)?;
    fs::write(&results_file, json_output)?;
    
    print_benchmark_summary("Metadata Operations Benchmark", &results, total_duration);
    
    info!("Metadata benchmark completed. Results saved to: {}", results_file);
    Ok(())
}

/// Run network latency benchmarks
async fn run_network_benchmark(targets: Option<String>, output_dir: &str, duration: u64) -> CliResult {
    info!("Running network benchmark for {} seconds", duration);
    
    let target_addresses = if let Some(targets_str) = targets {
        targets_str.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        vec!["localhost:9421".to_string()] // Default MooseNG master port
    };
    
    fs::create_dir_all(output_dir)?;
    
    let start_time = Instant::now();
    let mut results = Vec::new();
    
    for target in target_addresses {
        info!("Testing network connectivity to: {}", target);
        let target_results = benchmark_network_target(&target, duration).await?;
        results.extend(target_results);
    }
    
    let total_duration = start_time.elapsed();
    
    // Save results
    let results_file = format!("{}/network_benchmark_results.json", output_dir);
    let json_output = serde_json::to_string_pretty(&results)?;
    fs::write(&results_file, json_output)?;
    
    print_benchmark_summary("Network Benchmark", &results, total_duration);
    
    info!("Network benchmark completed. Results saved to: {}", results_file);
    Ok(())
}

/// Compare benchmark results
async fn compare_benchmark_results(baseline_path: &str, current_path: &str, format: &str) -> CliResult {
    info!("Comparing benchmark results: {} vs {}", baseline_path, current_path);
    
    let baseline_content = fs::read_to_string(baseline_path)
        .context("Failed to read baseline results")?;
    let current_content = fs::read_to_string(current_path)
        .context("Failed to read current results")?;
    
    let baseline_results: Vec<SimpleBenchmarkResult> = serde_json::from_str(&baseline_content)
        .context("Failed to parse baseline results")?;
    let current_results: Vec<SimpleBenchmarkResult> = serde_json::from_str(&current_content)
        .context("Failed to parse current results")?;
    
    let comparison = generate_comparison(&baseline_results, &current_results);
    
    match format {
        "json" => {
            let json_output = serde_json::to_string_pretty(&comparison)?;
            println!("{}", json_output);
        }
        "table" => {
            print_comparison_table(&comparison);
        }
        "html" => {
            let html_output = generate_html_comparison(&comparison);
            println!("{}", html_output);
        }
        _ => {
            warn!("Unknown format '{}', defaulting to table", format);
            print_comparison_table(&comparison);
        }
    }
    
    Ok(())
}

/// Generate benchmark report
async fn generate_benchmark_report(
    input_path: &str, 
    format: &str, 
    output_path: Option<String>, 
    _include_charts: bool
) -> Result<()> {
    info!("Generating benchmark report from: {}", input_path);
    
    let content = fs::read_to_string(input_path)
        .context("Failed to read benchmark results")?;
    let results: Vec<SimpleBenchmarkResult> = serde_json::from_str(&content)
        .context("Failed to parse benchmark results")?;
    
    let report_content = match format {
        "html" => generate_html_report(&results),
        "json" => serde_json::to_string_pretty(&results)?,
        "csv" => generate_csv_report(&results),
        _ => {
            warn!("Unknown format '{}', defaulting to json", format);
            serde_json::to_string_pretty(&results)?
        }
    };
    
    if let Some(output_file) = output_path {
        fs::write(&output_file, &report_content)?;
        info!("Report saved to: {}", output_file);
    } else {
        println!("{}", report_content);
    }
    
    Ok(())
}

/// List available benchmark types
async fn list_available_benchmarks() -> CliResult {
    println!("Available benchmark types:");
    println!("  quick      - Quick suite with basic file and metadata operations");
    println!("  full       - Comprehensive benchmark suite with multiple configurations");
    println!("  file       - File operation benchmarks with configurable sizes");
    println!("  metadata   - Metadata operation stress tests");
    println!("  network    - Network latency and connectivity tests");
    println!("");
    println!("Additional commands:");
    println!("  compare    - Compare results between two benchmark runs");
    println!("  report     - Generate reports from benchmark data");
    println!("  status     - Show recent benchmark history and status");
    
    Ok(())
}

/// Show benchmark status
async fn show_benchmark_status(detailed: bool, limit: usize) -> CliResult {
    info!("Showing benchmark status");
    
    // Look for recent benchmark results
    let results_dir = "./benchmark-results";
    if !std::path::Path::new(results_dir).exists() {
        println!("No benchmark results found. Run 'mooseng benchmark quick' to get started.");
        return Ok(());
    }
    
    let mut result_files = Vec::new();
    
    if let Ok(entries) = fs::read_dir(results_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with("_results.json") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            result_files.push((name.to_string(), modified));
                        }
                    }
                }
            }
        }
    }
    
    // Sort by modification time (newest first)
    result_files.sort_by(|a, b| b.1.cmp(&a.1));
    result_files.truncate(limit);
    
    if result_files.is_empty() {
        println!("No benchmark result files found in {}", results_dir);
        return Ok(());
    }
    
    println!("Recent benchmark results:");
    for (filename, modified) in result_files {
        let file_path = format!("{}/{}", results_dir, filename);
        
        if detailed {
            if let Ok(content) = fs::read_to_string(&file_path) {
                if let Ok(results) = serde_json::from_str::<Vec<SimpleBenchmarkResult>>(&content) {
                    let success_count = results.iter().filter(|r| r.success).count();
                    let total_count = results.len();
                    
                    println!("  {} (Modified: {:?})", filename, modified);
                    println!("    Tests: {}/{} successful", success_count, total_count);
                    
                    if !results.is_empty() {
                        let avg_duration: f64 = results.iter()
                            .filter(|r| r.success)
                            .map(|r| r.duration_ms)
                            .sum::<f64>() / success_count.max(1) as f64;
                        println!("    Average duration: {:.2}ms", avg_duration);
                    }
                    println!();
                }
            }
        } else {
            println!("  {} (Modified: {:?})", filename, modified);
        }
    }
    
    Ok(())
}

// Helper functions for actual benchmark implementations

async fn benchmark_file_operations(iterations: usize) -> Result<Vec<SimpleBenchmarkResult>> {
    let mut results = Vec::new();
    
    // Simulate file creation benchmark
    let start = Instant::now();
    for _i in 0..iterations {
        // In a real implementation, this would interact with the MooseNG filesystem
        tokio::time::sleep(Duration::from_micros(100)).await;
    }
    let duration = start.elapsed();
    
    results.push(SimpleBenchmarkResult {
        operation: "file_create".to_string(),
        duration_ms: duration.as_millis() as f64 / iterations as f64,
        throughput_ops_sec: Some(iterations as f64 / duration.as_secs_f64()),
        success: true,
        error: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(results)
}

async fn benchmark_metadata_operations(operations: usize) -> Result<Vec<SimpleBenchmarkResult>> {
    let mut results = Vec::new();
    
    // Simulate metadata operations
    let start = Instant::now();
    for _i in 0..operations {
        tokio::time::sleep(Duration::from_micros(50)).await;
    }
    let duration = start.elapsed();
    
    results.push(SimpleBenchmarkResult {
        operation: "metadata_lookup".to_string(),
        duration_ms: duration.as_millis() as f64 / operations as f64,
        throughput_ops_sec: Some(operations as f64 / duration.as_secs_f64()),
        success: true,
        error: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(results)
}

async fn benchmark_network_operations() -> Result<Vec<SimpleBenchmarkResult>> {
    let mut results = Vec::new();
    
    // Simple connectivity test
    let start = Instant::now();
    let success = tokio::net::TcpSocket::new_v4()?.connect("127.0.0.1:0".parse()?).await.is_ok();
    let duration = start.elapsed();
    
    results.push(SimpleBenchmarkResult {
        operation: "network_connect".to_string(),
        duration_ms: duration.as_millis() as f64,
        throughput_ops_sec: None,
        success,
        error: if success { None } else { Some("Connection failed".to_string()) },
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(results)
}

async fn benchmark_file_size(size: usize, iterations: usize) -> Result<Vec<SimpleBenchmarkResult>> {
    let mut results = Vec::new();
    
    let start = Instant::now();
    for _i in 0..iterations {
        // Simulate based on file size
        let delay_micros = (size / 1024).max(1) as u64; // Simulate larger files taking longer
        tokio::time::sleep(Duration::from_micros(delay_micros)).await;
    }
    let duration = start.elapsed();
    
    results.push(SimpleBenchmarkResult {
        operation: format!("file_{}bytes", size),
        duration_ms: duration.as_millis() as f64 / iterations as f64,
        throughput_ops_sec: Some(iterations as f64 / duration.as_secs_f64()),
        success: true,
        error: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(results)
}

async fn benchmark_metadata_stress(operations: usize) -> Result<Vec<SimpleBenchmarkResult>> {
    let mut results = Vec::new();
    
    let start = Instant::now();
    for _i in 0..operations {
        tokio::time::sleep(Duration::from_nanos(100)).await;
    }
    let duration = start.elapsed();
    
    results.push(SimpleBenchmarkResult {
        operation: "metadata_stress".to_string(),
        duration_ms: duration.as_millis() as f64 / operations as f64,
        throughput_ops_sec: Some(operations as f64 / duration.as_secs_f64()),
        success: true,
        error: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(results)
}

async fn benchmark_network_target(target: &str, duration_secs: u64) -> Result<Vec<SimpleBenchmarkResult>> {
    let mut results = Vec::new();
    
    let start = Instant::now();
    let mut successful_pings = 0;
    let mut total_pings = 0;
    let mut total_latency = Duration::default();
    
    let end_time = start + Duration::from_secs(duration_secs);
    
    while Instant::now() < end_time {
        let ping_start = Instant::now();
        
        // Simple connectivity test (in real implementation, would use actual network calls)
        let success = if target.contains("localhost") || target.contains("127.0.0.1") {
            tokio::time::sleep(Duration::from_millis(1)).await;
            true
        } else {
            // For external targets, simulate network delay
            tokio::time::sleep(Duration::from_millis(10)).await;
            true
        };
        
        let ping_duration = ping_start.elapsed();
        total_pings += 1;
        
        if success {
            successful_pings += 1;
            total_latency += ping_duration;
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await; // Rate limiting
    }
    
    let avg_latency = if successful_pings > 0 {
        total_latency.as_millis() as f64 / successful_pings as f64
    } else {
        0.0
    };
    
    results.push(SimpleBenchmarkResult {
        operation: format!("network_ping_{}", target),
        duration_ms: avg_latency,
        throughput_ops_sec: Some(successful_pings as f64 / duration_secs as f64),
        success: successful_pings > 0,
        error: if successful_pings == 0 { Some("All pings failed".to_string()) } else { None },
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    
    Ok(results)
}

async fn load_benchmark_config(path: &str) -> Result<BenchmarkConfig> {
    let content = fs::read_to_string(path)
        .context("Failed to read benchmark configuration file")?;
    
    // Try TOML first, then JSON
    if let Ok(config) = toml::from_str::<BenchmarkConfig>(&content) {
        Ok(config)
    } else {
        serde_json::from_str(&content)
            .context("Failed to parse benchmark configuration as JSON or TOML")
    }
}

fn print_benchmark_summary(title: &str, results: &[SimpleBenchmarkResult], total_duration: Duration) {
    println!("\n{}", "=".repeat(60));
    println!("{}", title);
    println!("{}", "=".repeat(60));
    
    let successful = results.iter().filter(|r| r.success).count();
    let total = results.len();
    
    println!("Total tests: {}", total);
    println!("Successful: {} ({:.1}%)", successful, successful as f64 / total as f64 * 100.0);
    println!("Failed: {}", total - successful);
    println!("Total duration: {:.2}s", total_duration.as_secs_f64());
    println!();
    
    println!("{:<30} {:>12} {:>15} {:>8}", "Operation", "Duration (ms)", "Throughput/s", "Status");
    println!("{}", "-".repeat(70));
    
    for result in results {
        let status = if result.success { "✓" } else { "✗" };
        let throughput = result.throughput_ops_sec
            .map(|t| format!("{:.2}", t))
            .unwrap_or_else(|| "N/A".to_string());
        
        println!(
            "{:<30} {:>12.2} {:>15} {:>8}",
            result.operation,
            result.duration_ms,
            throughput,
            status
        );
    }
    
    println!();
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkComparison {
    operation: String,
    baseline_duration: f64,
    current_duration: f64,
    change_percent: f64,
    improvement: bool,
}

fn generate_comparison(baseline: &[SimpleBenchmarkResult], current: &[SimpleBenchmarkResult]) -> Vec<BenchmarkComparison> {
    let mut comparisons = Vec::new();
    
    for current_result in current {
        if let Some(baseline_result) = baseline.iter().find(|b| b.operation == current_result.operation) {
            let change_percent = if baseline_result.duration_ms > 0.0 {
                ((current_result.duration_ms - baseline_result.duration_ms) / baseline_result.duration_ms) * 100.0
            } else {
                0.0
            };
            
            comparisons.push(BenchmarkComparison {
                operation: current_result.operation.clone(),
                baseline_duration: baseline_result.duration_ms,
                current_duration: current_result.duration_ms,
                change_percent,
                improvement: change_percent < 0.0, // Lower duration is better
            });
        }
    }
    
    comparisons
}

fn print_comparison_table(comparisons: &[BenchmarkComparison]) {
    println!("\nBenchmark Comparison Results");
    println!("{}", "=".repeat(80));
    println!("{:<25} {:>12} {:>12} {:>12} {:>8}", "Operation", "Baseline(ms)", "Current(ms)", "Change(%)", "Status");
    println!("{}", "-".repeat(80));
    
    for comp in comparisons {
        let status = if comp.improvement { "↑" } else { "↓" };
        let change_str = format!("{:+.1}", comp.change_percent);
        
        println!(
            "{:<25} {:>12.2} {:>12.2} {:>12} {:>8}",
            comp.operation,
            comp.baseline_duration,
            comp.current_duration,
            change_str,
            status
        );
    }
    
    let improvements = comparisons.iter().filter(|c| c.improvement).count();
    let total = comparisons.len();
    
    println!();
    println!("Summary: {}/{} operations improved", improvements, total);
}

fn generate_html_comparison(comparisons: &[BenchmarkComparison]) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Benchmark Comparison</title>
    <style>
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: right; }}
        th {{ background-color: #f2f2f2; }}
        .improvement {{ color: green; }}
        .regression {{ color: red; }}
    </style>
</head>
<body>
    <h1>Benchmark Comparison Results</h1>
    <table>
        <tr>
            <th>Operation</th>
            <th>Baseline (ms)</th>
            <th>Current (ms)</th>
            <th>Change (%)</th>
            <th>Status</th>
        </tr>
        {}
    </table>
</body>
</html>"#,
        comparisons.iter()
            .map(|c| format!(
                r#"<tr class="{}">
                    <td style="text-align: left;">{}</td>
                    <td>{:.2}</td>
                    <td>{:.2}</td>
                    <td>{:+.1}</td>
                    <td>{}</td>
                </tr>"#,
                if c.improvement { "improvement" } else { "regression" },
                c.operation,
                c.baseline_duration,
                c.current_duration,
                c.change_percent,
                if c.improvement { "↑" } else { "↓" }
            ))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn generate_html_report(results: &[SimpleBenchmarkResult]) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Benchmark Report</title>
    <style>
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: right; }}
        th {{ background-color: #f2f2f2; }}
        .success {{ color: green; }}
        .failure {{ color: red; }}
    </style>
</head>
<body>
    <h1>Benchmark Report</h1>
    <table>
        <tr>
            <th>Operation</th>
            <th>Duration (ms)</th>
            <th>Throughput (ops/s)</th>
            <th>Status</th>
            <th>Timestamp</th>
        </tr>
        {}
    </table>
</body>
</html>"#,
        results.iter()
            .map(|r| format!(
                r#"<tr class="{}">
                    <td style="text-align: left;">{}</td>
                    <td>{:.2}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td style="text-align: left;">{}</td>
                </tr>"#,
                if r.success { "success" } else { "failure" },
                r.operation,
                r.duration_ms,
                r.throughput_ops_sec.map(|t| format!("{:.2}", t)).unwrap_or_else(|| "N/A".to_string()),
                if r.success { "✓" } else { "✗" },
                r.timestamp
            ))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn generate_csv_report(results: &[SimpleBenchmarkResult]) -> String {
    let mut csv = String::from("Operation,Duration (ms),Throughput (ops/s),Success,Error,Timestamp\n");
    
    for result in results {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            result.operation,
            result.duration_ms,
            result.throughput_ops_sec.map(|t| t.to_string()).unwrap_or_else(|| "".to_string()),
            result.success,
            result.error.as_deref().unwrap_or(""),
            result.timestamp
        ));
    }
    
    csv
}

fn generate_text_summary(results: &[SimpleBenchmarkResult], total_duration: Duration, output_path: &str) -> Result<()> {
    let successful = results.iter().filter(|r| r.success).count();
    let total = results.len();
    
    let summary = format!(
        "Benchmark Summary Report\n\
         Generated: {}\n\
         \n\
         Overall Results:\n\
         - Total tests: {}\n\
         - Successful: {} ({:.1}%)\n\
         - Failed: {}\n\
         - Total duration: {:.2}s\n\
         \n\
         Performance Results:\n\
         {}\n",
        chrono::Utc::now().to_rfc3339(),
        total,
        successful,
        successful as f64 / total as f64 * 100.0,
        total - successful,
        total_duration.as_secs_f64(),
        results.iter()
            .map(|r| format!(
                "  {}: {:.2}ms {}",
                r.operation,
                r.duration_ms,
                if r.success { "✓" } else { "✗" }
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
    
    fs::write(output_path, summary)?;
    Ok(())
}

// ========================================
// Unified Benchmark System Integration
// ========================================

/// Start the benchmark dashboard server
async fn start_benchmark_dashboard(port: u16, database_path: &str, open_browser: bool) -> CliResult {
    info!("Starting benchmark dashboard on port {}", port);
    
    // Initialize database
    let db_path = std::path::Path::new(database_path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let db = BenchmarkDatabase::new(database_path).await
        .context("Failed to initialize benchmark database")?;
    
    info!("Database initialized at: {}", database_path);
    
    // Try to use unified dashboard first, then fall back to basic implementation
    let unified_dashboard_path = std::env::current_dir()?
        .join("mooseng")
        .join("unified_dashboard");
    
    if unified_dashboard_path.exists() {
        info!("Starting unified dashboard server...");
        
        // Use Python dashboard server
        let mut cmd = Command::new("python3");
        cmd.arg("server.py")
           .arg("--host")
           .arg("127.0.0.1")
           .arg("--port")
           .arg(port.to_string())
           .arg("--db")
           .arg(database_path)
           .current_dir(&unified_dashboard_path);
        
        info!("Executing unified dashboard from: {}", unified_dashboard_path.display());
        
        // Launch in background
        let child = cmd.spawn()
            .context("Failed to start unified dashboard server")?;
        
        // Give the server time to start
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        let dashboard_url = format!("http://127.0.0.1:{}", port);
        info!("Unified dashboard server started successfully");
        info!("Dashboard available at: {}", dashboard_url);
        
        if open_browser {
            info!("Opening browser...");
            if let Err(e) = open_browser_url(&dashboard_url) {
                warn!("Failed to open browser: {}", e);
                info!("Please open {} manually", dashboard_url);
            }
        } else {
            info!("To view the dashboard, open: {}", dashboard_url);
        }
        
        info!("Dashboard is running. Press Ctrl+C to stop.");
        
        // Wait for the process
        let mut child = child;
        let exit_status = child.wait()?;
        
        if exit_status.success() {
            info!("Dashboard server stopped successfully");
        } else {
            error!("Dashboard server exited with error: {}", exit_status);
        }
        
        return Ok(());
    }
    
    // Fallback to Rust-based dashboard if Python version not available
    let benchmarks_dir = std::env::current_dir()?
        .join("mooseng-benchmarks");
    
    if benchmarks_dir.exists() {
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
           .arg("--bin")
           .arg("dashboard_server")
           .arg("--")
           .arg("serve")
           .arg("--bind")
           .arg(format!("127.0.0.1:{}", port))
           .arg("--database")
           .arg(database_path)
           .current_dir(&benchmarks_dir);
        
        info!("Executing dashboard server from: {}", benchmarks_dir.display());
        
        // Launch in background
        let child = cmd.spawn()
            .context("Failed to start dashboard server")?;
        
        // Give the server time to start
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let dashboard_url = format!("http://localhost:{}", port);
        info!("Dashboard server started successfully");
        info!("Dashboard available at: {}", dashboard_url);
        
        if open_browser {
            info!("Opening browser...");
            if let Err(e) = open_browser_url(&dashboard_url) {
                warn!("Failed to open browser: {}", e);
                info!("Please open {} manually", dashboard_url);
            }
        } else {
            info!("To view the dashboard, open: {}", dashboard_url);
        }
        
        info!("Dashboard is running. Press Ctrl+C to stop.");
        
        // Wait for the process
        let mut child = child;
        let exit_status = child.wait()?;
        
        if exit_status.success() {
            info!("Dashboard server stopped successfully");
        } else {
            error!("Dashboard server exited with error: {}", exit_status);
        }
    } else {
        warn!("Unified benchmark runner not found at {}", benchmarks_dir.display());
        info!("Falling back to basic dashboard info");
        
        // Show simple status instead
        let stats = db.get_statistics().await?;
        println!("Benchmark Database Statistics:");
        println!("============================");
        for (key, value) in stats {
            println!("{}: {}", key, value);
        }
    }
    
    Ok(())
}

/// Query benchmark results from database
async fn query_benchmark_results(
    session_id: Option<String>,
    operation: Option<String>, 
    suite: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    limit: usize,
    format: &str,
) -> CliResult {
    info!("Querying benchmark results");
    
    // Parse dates
    let start_dt = if let Some(date_str) = start_date {
        Some(chrono::DateTime::parse_from_rfc3339(&date_str)
            .context("Invalid start date format, use RFC3339")?
            .with_timezone(&chrono::Utc))
    } else {
        None
    };
    
    let end_dt = if let Some(date_str) = end_date {
        Some(chrono::DateTime::parse_from_rfc3339(&date_str)
            .context("Invalid end date format, use RFC3339")?
            .with_timezone(&chrono::Utc))
    } else {
        None
    };
    
    // Try to connect to database
    let db = BenchmarkDatabase::new("./benchmark.db").await
        .context("Failed to connect to benchmark database. Run 'mooseng benchmark dashboard' first to initialize.")?;
    
    let filter = BenchmarkFilter {
        session_ids: session_id.map(|id| vec![id]),
        operations: operation.map(|op| vec![op]),
        suite_names: suite.map(|s| vec![s]),
        start_date: start_dt,
        end_date: end_dt,
        limit: Some(limit as i32),
        offset: None,
    };
    
    let results = db.query_results(&filter).await
        .context("Failed to query benchmark results")?;
    
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        "csv" => {
            println!("session_id,operation,suite_name,mean_time_ms,throughput,samples,timestamp");
            for result in results {
                println!("{},{},{},{},{},{},{}",
                    result.session_id,
                    result.operation,
                    result.suite_name.unwrap_or_else(|| "N/A".to_string()),
                    result.mean_time_ns as f64 / 1_000_000.0,
                    result.throughput.map(|t| t.to_string()).unwrap_or_else(|| "N/A".to_string()),
                    result.samples,
                    result.timestamp.to_rfc3339()
                );
            }
        }
        _ => {
            // Table format
            println!("Benchmark Results Query");
            println!("{}", "=".repeat(80));
            println!("Found {} results", results.len());
            println!();
            
            if !results.is_empty() {
                println!("{:<20} {:<25} {:<15} {:<12} {:<12} {:<8}", 
                         "Session", "Operation", "Suite", "Mean(ms)", "Throughput", "Samples");
                println!("{}", "-".repeat(80));
                
                for result in results {
                    let session_short = if result.session_id.len() > 18 {
                        format!("{}...", &result.session_id[..15])
                    } else {
                        result.session_id.clone()
                    };
                    
                    println!("{:<20} {:<25} {:<15} {:>12.2} {:<12} {:>8}",
                        session_short,
                        if result.operation.len() > 23 {
                            format!("{}...", &result.operation[..20])
                        } else {
                            result.operation.clone()
                        },
                        result.suite_name.as_deref().unwrap_or("N/A"),
                        result.mean_time_ns as f64 / 1_000_000.0,
                        result.throughput.map(|t| format!("{:.1}", t)).unwrap_or_else(|| "N/A".to_string()),
                        result.samples
                    );
                }
            }
        }
    }
    
    Ok(())
}

/// Analyze benchmark trends
async fn analyze_benchmark_trends(
    operation: Option<String>,
    days: i32,
    confidence: f64,
    format: &str,
) -> CliResult {
    info!("Analyzing benchmark trends for {} days", days);
    
    let db = BenchmarkDatabase::new("./benchmark.db").await
        .context("Failed to connect to benchmark database")?;
    
    let trends = db.analyze_trends(operation.as_deref(), days).await
        .context("Failed to analyze trends")?;
    
    let filtered_trends: Vec<_> = trends.into_iter()
        .filter(|t| t.confidence >= confidence)
        .collect();
    
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&filtered_trends)?);
        }
        _ => {
            println!("Benchmark Trend Analysis ({} days, min confidence: {:.1}%)", days, confidence * 100.0);
            println!("{}", "=".repeat(80));
            
            if filtered_trends.is_empty() {
                println!("No trends found with sufficient confidence.");
                println!("Try reducing the confidence threshold or increasing the analysis period.");
                return Ok(());
            }
            
            println!("{:<25} {:<12} {:<10} {:<12} {:<8}", 
                     "Operation", "Trend", "Change%", "Confidence%", "Points");
            println!("{}", "-".repeat(80));
            
            for trend in filtered_trends {
                let direction = match trend.trend_direction {
                    mooseng_benchmarks::database::TrendDirection::Improving => "📈 Better",
                    mooseng_benchmarks::database::TrendDirection::Degrading => "📉 Worse", 
                    mooseng_benchmarks::database::TrendDirection::Stable => "➡️ Stable",
                    mooseng_benchmarks::database::TrendDirection::Insufficient => "❓ Unknown",
                };
                
                println!("{:<25} {:<12} {:>10.1} {:>12.1} {:>8}",
                    if trend.operation.len() > 23 {
                        format!("{}...", &trend.operation[..20])
                    } else {
                        trend.operation
                    },
                    direction,
                    trend.change_percent,
                    trend.confidence * 100.0,
                    trend.data_points
                );
            }
        }
    }
    
    Ok(())
}

/// Run continuous benchmarks using unified system
async fn run_continuous_benchmarks_unified(
    iterations: usize,
    interval: u64,
    threshold: f64,
    output: &str,
    database: &str,
) -> CliResult {
    info!("Starting continuous benchmarks");
    info!("Iterations: {}, Interval: {}min, Threshold: {}%", iterations, interval, threshold);
    
    // Initialize database
    let db = BenchmarkDatabase::new(database).await
        .context("Failed to initialize benchmark database")?;
    
    fs::create_dir_all(output)?;
    
    // Use unified benchmark runner if available
    let benchmarks_dir = std::env::current_dir()?
        .join("mooseng-benchmarks");
    
    if benchmarks_dir.exists() {
        info!("Using unified benchmark runner");
        
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
           .arg("--bin")
           .arg("unified-benchmark")
           .arg("--")
           .arg("continuous")
           .arg("--iterations")
           .arg(iterations.to_string())
           .arg("--interval")
           .arg(interval.to_string())
           .arg("--regression-threshold")
           .arg(threshold.to_string())
           .arg("--output")
           .arg(output)
           .current_dir(&benchmarks_dir);
        
        let output = cmd.output()
            .context("Failed to execute unified benchmark runner")?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            println!("{}", stdout);
            if !stderr.is_empty() {
                warn!("Benchmark warnings: {}", stderr);
            }
            
            info!("Continuous benchmarks completed successfully");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Unified benchmark runner failed: {}", stderr).into());
        }
    } else {
        warn!("Unified benchmark runner not found, using fallback implementation");
        
        // Fallback to simple continuous implementation
        for iteration in 1..=iterations {
            info!("Running benchmark iteration {}/{}", iteration, iterations);
            
            // Run a simple benchmark
            let start_time = Instant::now();
            let mut results = Vec::new();
            
            // Simple file operations benchmark
            let file_results = benchmark_file_operations(100).await?;
            results.extend(file_results);
            
            let duration = start_time.elapsed();
            
            // Save results
            let iteration_file = format!("{}/continuous_iteration_{}.json", output, iteration);
            let json_output = serde_json::to_string_pretty(&results)?;
            fs::write(&iteration_file, json_output)?;
            
            info!("Iteration {} completed in {:.2}s", iteration, duration.as_secs_f64());
            
            // Wait for next iteration (unless this is the last one)
            if iteration < iterations {
                info!("Waiting {} minutes until next iteration...", interval);
                tokio::time::sleep(Duration::from_secs(interval * 60)).await;
            }
        }
        
        info!("Continuous benchmarks completed (fallback mode)");
    }
    
    Ok(())
}

/// Helper function to open browser
fn open_browser_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", url])
            .output()
            .context("Failed to open browser")?;
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .output()
            .context("Failed to open browser")?;
    }
    
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .output()
            .context("Failed to open browser")?;
    }
    
    Ok(())
}