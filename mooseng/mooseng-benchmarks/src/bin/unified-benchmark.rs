//! Unified Benchmark Runner for MooseNG
//!
//! This tool provides a unified interface to run all MooseNG benchmarks,
//! collect results, and generate reports.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mooseng_benchmarks::{
    benchmarks::*,
    config::BenchmarkConfig,
    metrics::BenchmarkMetrics,
    report::{BenchmarkReport, ReportFormat},
    BenchmarkResult,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs;
use tracing::{debug, error, info, warn};

#[derive(Parser)]
#[command(name = "unified-benchmark")]
#[command(about = "Unified benchmark runner for MooseNG distributed file system")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Output directory for results
    #[arg(short, long, default_value = "./benchmark-results")]
    output: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all benchmarks
    All {
        /// Include long-running benchmarks
        #[arg(long)]
        include_long: bool,
        
        /// Include network-intensive benchmarks
        #[arg(long)]
        include_network: bool,
        
        /// Tags to filter benchmarks (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
    
    /// Run specific benchmark suite
    Suite {
        /// Name of the benchmark suite
        name: String,
        
        /// Custom configuration for this run
        #[arg(long)]
        custom_config: Option<PathBuf>,
    },
    
    /// List available benchmarks
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
        
        /// Filter by tags
        #[arg(long)]
        tags: Option<String>,
    },
    
    /// Compare benchmark results
    Compare {
        /// Baseline results file
        baseline: PathBuf,
        
        /// Current results file
        current: PathBuf,
        
        /// Output format (json, html, text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    
    /// Generate performance report
    Report {
        /// Input results directory or file
        input: PathBuf,
        
        /// Report format (html, json, csv)
        #[arg(short, long, default_value = "html")]
        format: String,
        
        /// Include historical trends
        #[arg(long)]
        include_trends: bool,
    },
    
    /// Validate benchmark configuration
    Validate {
        /// Configuration file to validate
        config_file: PathBuf,
    },
    
    /// Run continuous benchmarks (CI mode)
    Continuous {
        /// Number of iterations to run
        #[arg(short, long, default_value = "1")]
        iterations: usize,
        
        /// Interval between runs (in minutes)
        #[arg(short, long, default_value = "60")]
        interval: u64,
        
        /// Regression threshold (percentage)
        #[arg(long, default_value = "5.0")]
        regression_threshold: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuite {
    pub name: String,
    pub description: String,
    pub benchmarks: Vec<String>,
    pub tags: Vec<String>,
    pub estimated_duration: Duration,
    pub requirements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBenchmarkResult {
    pub suite_name: String,
    pub benchmark_name: String,
    pub result: BenchmarkResult,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSession {
    pub session_id: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub config: BenchmarkConfig,
    pub results: Vec<UnifiedBenchmarkResult>,
    pub summary: Option<BenchmarkSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_benchmarks: usize,
    pub successful_benchmarks: usize,
    pub failed_benchmarks: usize,
    pub total_duration: Duration,
    pub performance_score: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    
    // Create output directory
    fs::create_dir_all(&cli.output).await
        .context("Failed to create output directory")?;
    
    // Load configuration
    let config = load_config(cli.config.as_deref()).await?;
    
    match cli.command {
        Commands::All { include_long, include_network, tags } => {
            run_all_benchmarks(&cli.output, &config, include_long, include_network, tags).await
        }
        Commands::Suite { name, custom_config } => {
            let suite_config = if let Some(custom_path) = custom_config {
                load_config(Some(&custom_path)).await?
            } else {
                config
            };
            run_benchmark_suite(&cli.output, &suite_config, &name).await
        }
        Commands::List { detailed, tags } => {
            list_benchmarks(detailed, tags).await
        }
        Commands::Compare { baseline, current, format } => {
            compare_results(baseline, current, &format).await
        }
        Commands::Report { input, format, include_trends } => {
            generate_report(input, &format, include_trends).await
        }
        Commands::Validate { config_file } => {
            validate_config(config_file).await
        }
        Commands::Continuous { iterations, interval, regression_threshold } => {
            run_continuous_benchmarks(&cli.output, &config, iterations, interval, regression_threshold).await
        }
    }
}

async fn load_config(config_path: Option<&std::path::Path>) -> Result<BenchmarkConfig> {
    if let Some(path) = config_path {
        let content = fs::read_to_string(path).await
            .context("Failed to read configuration file")?;
        
        if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::from_str(&content).context("Failed to parse TOML configuration")
        } else {
            serde_json::from_str(&content).context("Failed to parse JSON configuration")
        }
    } else {
        Ok(BenchmarkConfig::default())
    }
}

async fn run_all_benchmarks(
    output_dir: &std::path::Path,
    config: &BenchmarkConfig,
    include_long: bool,
    include_network: bool,
    tags: Option<String>,
) -> Result<()> {
    info!("Starting comprehensive benchmark suite");
    
    let session_id = generate_session_id();
    let start_time = chrono::Utc::now();
    
    let suites = get_available_suites(include_long, include_network, tags).await?;
    let mut session = BenchmarkSession {
        session_id: session_id.clone(),
        start_time,
        end_time: None,
        config: config.clone(),
        results: Vec::new(),
        summary: None,
    };
    
    info!("Found {} benchmark suites to run", suites.len());
    
    let session_start = Instant::now();
    let mut successful = 0;
    let mut failed = 0;
    
    for suite in &suites {
        info!("Running benchmark suite: {}", suite.name);
        
        match run_suite_benchmarks(suite, config).await {
            Ok(results) => {
                successful += results.len();
                for result in results {
                    session.results.push(UnifiedBenchmarkResult {
                        suite_name: suite.name.clone(),
                        benchmark_name: result.operation.clone(),
                        result,
                        metadata: HashMap::new(),
                    });
                }
                info!("✓ Completed suite: {}", suite.name);
            }
            Err(e) => {
                failed += 1;
                error!("✗ Failed suite {}: {}", suite.name, e);
            }
        }
    }
    
    let total_duration = session_start.elapsed();
    session.end_time = Some(chrono::Utc::now());
    session.summary = Some(BenchmarkSummary {
        total_benchmarks: successful + failed,
        successful_benchmarks: successful,
        failed_benchmarks: failed,
        total_duration,
        performance_score: calculate_performance_score(&session.results),
    });
    
    // Save session results
    let session_file = output_dir.join(format!("session_{}.json", session_id));
    let session_json = serde_json::to_string_pretty(&session)?;
    fs::write(&session_file, session_json).await?;
    
    // Generate summary report
    generate_session_summary(&session, output_dir).await?;
    
    info!("Benchmark session completed in {:.2}s", total_duration.as_secs_f64());
    info!("Results saved to: {}", session_file.display());
    
    Ok(())
}

async fn get_available_suites(
    include_long: bool,
    include_network: bool,
    tags: Option<String>,
) -> Result<Vec<BenchmarkSuite>> {
    let mut suites = vec![
        BenchmarkSuite {
            name: "file_operations".to_string(),
            description: "Basic file operations benchmarks".to_string(),
            benchmarks: vec!["small_files".to_string(), "large_files".to_string()],
            tags: vec!["core".to_string(), "file".to_string()],
            estimated_duration: Duration::from_secs(300),
            requirements: vec![],
        },
        BenchmarkSuite {
            name: "metadata_operations".to_string(),
            description: "Metadata operations and directory handling".to_string(),
            benchmarks: vec!["directory_operations".to_string(), "metadata_stress".to_string()],
            tags: vec!["core".to_string(), "metadata".to_string()],
            estimated_duration: Duration::from_secs(180),
            requirements: vec![],
        },
        BenchmarkSuite {
            name: "concurrent_operations".to_string(),
            description: "Concurrent access patterns".to_string(),
            benchmarks: vec!["concurrent_reads".to_string(), "concurrent_writes".to_string()],
            tags: vec!["core".to_string(), "concurrency".to_string()],
            estimated_duration: Duration::from_secs(420),
            requirements: vec![],
        },
    ];
    
    if include_network {
        suites.push(BenchmarkSuite {
            name: "network_operations".to_string(),
            description: "Network performance and multiregion tests".to_string(),
            benchmarks: vec!["cross_region".to_string(), "network_latency".to_string()],
            tags: vec!["network".to_string(), "multiregion".to_string()],
            estimated_duration: Duration::from_secs(600),
            requirements: vec!["network_access".to_string()],
        });
    }
    
    if include_long {
        suites.push(BenchmarkSuite {
            name: "stress_tests".to_string(),
            description: "Long-running stress and endurance tests".to_string(),
            benchmarks: vec!["endurance_test".to_string(), "memory_stress".to_string()],
            tags: vec!["stress".to_string(), "long".to_string()],
            estimated_duration: Duration::from_secs(3600),
            requirements: vec!["extended_runtime".to_string()],
        });
    }
    
    // Filter by tags if specified
    if let Some(tag_filter) = tags {
        let filter_tags: Vec<&str> = tag_filter.split(',').map(|s| s.trim()).collect();
        suites.retain(|suite| {
            filter_tags.iter().any(|tag| suite.tags.contains(&tag.to_string()))
        });
    }
    
    Ok(suites)
}

async fn run_benchmark_suite(
    output_dir: &std::path::Path,
    config: &BenchmarkConfig,
    suite_name: &str,
) -> Result<()> {
    info!("Running specific benchmark suite: {}", suite_name);
    
    let suites = get_available_suites(true, true, None).await?;
    let suite = suites.iter()
        .find(|s| s.name == suite_name)
        .ok_or_else(|| anyhow::anyhow!("Benchmark suite '{}' not found", suite_name))?;
    
    let results = run_suite_benchmarks(suite, config).await?;
    
    // Save results
    let output_file = output_dir.join(format!("suite_{}_{}.json", 
        suite_name, chrono::Utc::now().format("%Y%m%d_%H%M%S")));
    let json_output = serde_json::to_string_pretty(&results)?;
    fs::write(&output_file, json_output).await?;
    
    info!("Suite results saved to: {}", output_file.display());
    print_suite_summary(suite, &results);
    
    Ok(())
}

async fn run_suite_benchmarks(
    suite: &BenchmarkSuite,
    config: &BenchmarkConfig,
) -> Result<Vec<BenchmarkResult>> {
    let mut results = Vec::new();
    
    for benchmark_name in &suite.benchmarks {
        info!("Running benchmark: {}", benchmark_name);
        
        match run_individual_benchmark(benchmark_name, config).await {
            Ok(mut benchmark_results) => {
                results.append(&mut benchmark_results);
                info!("✓ Completed: {}", benchmark_name);
            }
            Err(e) => {
                warn!("✗ Failed: {} - {}", benchmark_name, e);
                // Create a failure result
                results.push(BenchmarkResult {
                    operation: benchmark_name.clone(),
                    mean_time: Duration::default(),
                    std_dev: Duration::default(),
                    min_time: Duration::default(),
                    max_time: Duration::default(),
                    throughput: None,
                    samples: 0,
                    timestamp: chrono::Utc::now(),
                    metadata: Some(serde_json::json!({
                        "error": e.to_string(),
                        "status": "failed"
                    })),
                    percentiles: None,
                });
            }
        }
    }
    
    Ok(results)
}

async fn run_individual_benchmark(
    benchmark_name: &str,
    config: &BenchmarkConfig,
) -> Result<Vec<BenchmarkResult>> {
    match benchmark_name {
        "small_files" => {
            let file_ops = file_operations::FileOperationsBenchmark::new();
            Ok(file_ops.run(config))
        }
        "large_files" => {
            let file_ops = file_operations::FileOperationsBenchmark::new();
            Ok(file_ops.run(config))
        }
        "directory_operations" => {
            let metadata_ops = metadata_operations::MetadataOperationsBenchmark::new();
            Ok(metadata_ops.run(config))
        }
        "metadata_stress" => {
            let metadata_ops = metadata_operations::MetadataOperationsBenchmark::new();
            Ok(metadata_ops.run(config))
        }
        "concurrent_reads" | "concurrent_writes" => {
            // Placeholder for concurrent benchmarks
            Ok(vec![create_placeholder_result(benchmark_name)])
        }
        "cross_region" | "network_latency" => {
            let network_ops = network_file_operations::NetworkFileOperationsBenchmark::new();
            Ok(network_ops.run(config))
        }
        "endurance_test" | "memory_stress" => {
            // Placeholder for stress tests
            Ok(vec![create_placeholder_result(benchmark_name)])
        }
        _ => {
            Err(anyhow::anyhow!("Unknown benchmark: {}", benchmark_name))
        }
    }
}

fn create_placeholder_result(operation: &str) -> BenchmarkResult {
    BenchmarkResult {
        operation: operation.to_string(),
        mean_time: Duration::from_millis(100),
        std_dev: Duration::from_millis(10),
        min_time: Duration::from_millis(80),
        max_time: Duration::from_millis(150),
        throughput: Some(100.0),
        samples: 50,
        timestamp: chrono::Utc::now(),
        metadata: Some(serde_json::json!({
            "status": "placeholder",
            "note": "This is a placeholder result for development"
        })),
        percentiles: None,
    }
}

async fn list_benchmarks(detailed: bool, tags: Option<String>) -> Result<()> {
    let suites = get_available_suites(true, true, tags).await?;
    
    println!("Available Benchmark Suites:");
    println!("{}", "=".repeat(50));
    
    for suite in &suites {
        println!("\n🏷️  {} ({})", suite.name, suite.tags.join(", "));
        
        if detailed {
            println!("   Description: {}", suite.description);
            println!("   Estimated Duration: {:.1} minutes", suite.estimated_duration.as_secs_f64() / 60.0);
            println!("   Benchmarks:");
            for benchmark in &suite.benchmarks {
                println!("     • {}", benchmark);
            }
            if !suite.requirements.is_empty() {
                println!("   Requirements: {}", suite.requirements.join(", "));
            }
        } else {
            println!("   {} benchmarks", suite.benchmarks.len());
        }
    }
    
    println!("\nTotal: {} benchmark suites", suites.len());
    Ok(())
}

async fn compare_results(baseline: PathBuf, current: PathBuf, format: &str) -> Result<()> {
    info!("Comparing benchmark results");
    
    let baseline_content = fs::read_to_string(&baseline).await
        .context("Failed to read baseline results")?;
    let current_content = fs::read_to_string(&current).await
        .context("Failed to read current results")?;
    
    let baseline_results: Vec<BenchmarkResult> = serde_json::from_str(&baseline_content)
        .context("Failed to parse baseline results")?;
    let current_results: Vec<BenchmarkResult> = serde_json::from_str(&current_content)
        .context("Failed to parse current results")?;
    
    let comparison = generate_comparison(&baseline_results, &current_results);
    
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&comparison)?);
        }
        "html" => {
            let html = generate_html_comparison(&comparison);
            println!("{}", html);
        }
        _ => {
            print_comparison_table(&comparison);
        }
    }
    
    Ok(())
}

#[derive(Debug, Serialize)]
struct BenchmarkComparison {
    operation: String,
    baseline_mean: f64,
    current_mean: f64,
    change_percent: f64,
    is_improvement: bool,
    significance: String,
}

fn generate_comparison(baseline: &[BenchmarkResult], current: &[BenchmarkResult]) -> Vec<BenchmarkComparison> {
    let mut comparisons = Vec::new();
    
    for current_result in current {
        if let Some(baseline_result) = baseline.iter().find(|b| b.operation == current_result.operation) {
            let baseline_ms = baseline_result.mean_time.as_millis() as f64;
            let current_ms = current_result.mean_time.as_millis() as f64;
            
            let change_percent = if baseline_ms > 0.0 {
                ((current_ms - baseline_ms) / baseline_ms) * 100.0
            } else {
                0.0
            };
            
            let significance = if change_percent.abs() > 10.0 {
                "significant".to_string()
            } else if change_percent.abs() > 5.0 {
                "moderate".to_string()
            } else {
                "minor".to_string()
            };
            
            comparisons.push(BenchmarkComparison {
                operation: current_result.operation.clone(),
                baseline_mean: baseline_ms,
                current_mean: current_ms,
                change_percent,
                is_improvement: change_percent < 0.0, // Lower time is better
                significance,
            });
        }
    }
    
    comparisons
}

fn print_comparison_table(comparisons: &[BenchmarkComparison]) {
    println!("\nBenchmark Comparison Results");
    println!("{}", "=".repeat(80));
    println!("{:<25} {:>12} {:>12} {:>12} {:>10} {:>10}", 
             "Operation", "Baseline(ms)", "Current(ms)", "Change(%)", "Trend", "Significance");
    println!("{}", "-".repeat(80));
    
    for comp in comparisons {
        let trend = if comp.is_improvement { "↗️  Better" } else { "↘️  Worse" };
        println!("{:<25} {:>12.2} {:>12.2} {:>12.1} {:>10} {:>10}",
                 comp.operation,
                 comp.baseline_mean,
                 comp.current_mean,
                 comp.change_percent,
                 trend,
                 comp.significance);
    }
    
    let improvements = comparisons.iter().filter(|c| c.is_improvement).count();
    println!("\nSummary: {}/{} operations improved", improvements, comparisons.len());
}

fn generate_html_comparison(comparisons: &[BenchmarkComparison]) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>MooseNG Benchmark Comparison</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 12px; text-align: left; }}
        th {{ background-color: #f2f2f2; font-weight: bold; }}
        .improvement {{ color: #28a745; }}
        .regression {{ color: #dc3545; }}
        .significant {{ font-weight: bold; }}
        .summary {{ margin-top: 20px; padding: 15px; background-color: #f8f9fa; border-radius: 5px; }}
    </style>
</head>
<body>
    <h1>MooseNG Benchmark Comparison Report</h1>
    <p>Generated on: {}</p>
    <table>
        <thead>
            <tr>
                <th>Operation</th>
                <th>Baseline (ms)</th>
                <th>Current (ms)</th>
                <th>Change (%)</th>
                <th>Trend</th>
                <th>Significance</th>
            </tr>
        </thead>
        <tbody>
            {}
        </tbody>
    </table>
    <div class="summary">
        <h3>Summary</h3>
        <p><strong>{}</strong> out of <strong>{}</strong> operations showed improvement.</p>
    </div>
</body>
</html>"#,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        comparisons.iter()
            .map(|c| format!(
                r#"<tr class="{}">
                    <td>{}</td>
                    <td>{:.2}</td>
                    <td>{:.2}</td>
                    <td>{:+.1}</td>
                    <td>{}</td>
                    <td class="{}">{}</td>
                </tr>"#,
                if c.is_improvement { "improvement" } else { "regression" },
                c.operation,
                c.baseline_mean,
                c.current_mean,
                c.change_percent,
                if c.is_improvement { "↗️ Better" } else { "↘️ Worse" },
                c.significance,
                c.significance
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        comparisons.iter().filter(|c| c.is_improvement).count(),
        comparisons.len()
    )
}

async fn generate_report(input: PathBuf, format: &str, include_trends: bool) -> Result<()> {
    info!("Generating performance report from: {}", input.display());
    
    let content = fs::read_to_string(&input).await
        .context("Failed to read input file")?;
    
    let results: Vec<BenchmarkResult> = serde_json::from_str(&content)
        .context("Failed to parse benchmark results")?;
    
    match format {
        "html" => {
            let html = generate_html_report(&results, include_trends);
            println!("{}", html);
        }
        "csv" => {
            let csv = generate_csv_report(&results);
            println!("{}", csv);
        }
        _ => {
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        }
    }
    
    Ok(())
}

fn generate_html_report(results: &[BenchmarkResult], include_trends: bool) -> String {
    let successful = results.iter().filter(|r| r.samples > 0).count();
    let total = results.len();
    let success_rate = (successful as f64 / total as f64) * 100.0;
    
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>MooseNG Performance Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .header {{ background-color: #f8f9fa; padding: 20px; border-radius: 5px; margin-bottom: 20px; }}
        .summary {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; margin-bottom: 20px; }}
        .metric {{ background-color: #e9ecef; padding: 15px; border-radius: 5px; text-align: center; }}
        .metric-value {{ font-size: 2em; font-weight: bold; color: #495057; }}
        .metric-label {{ color: #6c757d; margin-top: 5px; }}
        table {{ border-collapse: collapse; width: 100%; margin-top: 20px; }}
        th, td {{ border: 1px solid #ddd; padding: 12px; text-align: left; }}
        th {{ background-color: #f2f2f2; }}
        .success {{ color: #28a745; }}
        .failure {{ color: #dc3545; }}
        {}
    </style>
</head>
<body>
    <div class="header">
        <h1>MooseNG Performance Benchmark Report</h1>
        <p>Generated on: {}</p>
    </div>
    
    <div class="summary">
        <div class="metric">
            <div class="metric-value">{}</div>
            <div class="metric-label">Total Benchmarks</div>
        </div>
        <div class="metric">
            <div class="metric-value">{:.1}%</div>
            <div class="metric-label">Success Rate</div>
        </div>
        <div class="metric">
            <div class="metric-value">{:.2}</div>
            <div class="metric-label">Avg Performance Score</div>
        </div>
    </div>
    
    <table>
        <thead>
            <tr>
                <th>Operation</th>
                <th>Mean Time (ms)</th>
                <th>Throughput (ops/s)</th>
                <th>Samples</th>
                <th>Status</th>
                <th>Timestamp</th>
            </tr>
        </thead>
        <tbody>
            {}
        </tbody>
    </table>
    
    {}
</body>
</html>"#,
        if include_trends { ".trends { margin-top: 30px; }" } else { "" },
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        total,
        success_rate,
        calculate_avg_performance_score(results),
        results.iter()
            .map(|r| format!(
                r#"<tr class="{}">
                    <td>{}</td>
                    <td>{:.2}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                if r.samples > 0 { "success" } else { "failure" },
                r.operation,
                r.mean_time.as_millis() as f64,
                r.throughput.map(|t| format!("{:.2}", t)).unwrap_or_else(|| "N/A".to_string()),
                r.samples,
                if r.samples > 0 { "✓" } else { "✗" },
                r.timestamp.format("%Y-%m-%d %H:%M:%S")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        if include_trends {
            "<div class=\"trends\"><h2>Performance Trends</h2><p>Trend analysis would be implemented here with historical data.</p></div>"
        } else {
            ""
        }
    )
}

fn generate_csv_report(results: &[BenchmarkResult]) -> String {
    let mut csv = String::from("Operation,Mean Time (ms),Std Dev (ms),Min Time (ms),Max Time (ms),Throughput (ops/s),Samples,Timestamp\n");
    
    for result in results {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            result.operation,
            result.mean_time.as_millis(),
            result.std_dev.as_millis(),
            result.min_time.as_millis(),
            result.max_time.as_millis(),
            result.throughput.map(|t| t.to_string()).unwrap_or_else(|| "".to_string()),
            result.samples,
            result.timestamp.format("%Y-%m-%d %H:%M:%S")
        ));
    }
    
    csv
}

async fn validate_config(config_file: PathBuf) -> Result<()> {
    info!("Validating configuration file: {}", config_file.display());
    
    let config = load_config(Some(&config_file)).await?;
    
    println!("✓ Configuration file is valid");
    println!("Configuration details:");
    println!("  Warmup iterations: {}", config.warmup_iterations);
    println!("  Measurement iterations: {}", config.measurement_iterations);
    println!("  File sizes: {} entries", config.file_sizes.len());
    println!("  Concurrency levels: {:?}", config.concurrency_levels);
    println!("  Regions: {:?}", config.regions);
    println!("  Detailed report: {}", config.detailed_report);
    
    Ok(())
}

async fn run_continuous_benchmarks(
    output_dir: &std::path::Path,
    config: &BenchmarkConfig,
    iterations: usize,
    interval: u64,
    regression_threshold: f64,
) -> Result<()> {
    info!("Starting continuous benchmark mode");
    info!("Iterations: {}, Interval: {} minutes, Regression threshold: {}%", 
          iterations, interval, regression_threshold);
    
    let mut previous_results: Option<Vec<BenchmarkResult>> = None;
    
    for iteration in 1..=iterations {
        info!("Running benchmark iteration {}/{}", iteration, iterations);
        
        // Run core benchmarks
        let suites = get_available_suites(false, true, Some("core".to_string())).await?;
        let mut current_results = Vec::new();
        
        for suite in &suites {
            match run_suite_benchmarks(suite, config).await {
                Ok(mut results) => {
                    current_results.append(&mut results);
                }
                Err(e) => {
                    error!("Failed to run suite {}: {}", suite.name, e);
                }
            }
        }
        
        // Save iteration results
        let iteration_file = output_dir.join(format!("continuous_iteration_{}.json", iteration));
        let json_output = serde_json::to_string_pretty(&current_results)?;
        fs::write(&iteration_file, json_output).await?;
        
        // Check for regressions
        if let Some(ref prev_results) = previous_results {
            let regressions = detect_regressions(prev_results, &current_results, regression_threshold);
            if !regressions.is_empty() {
                warn!("Detected {} performance regressions:", regressions.len());
                for regression in &regressions {
                    warn!("  {}: {:.1}% slower", regression.operation, regression.change_percent);
                }
            }
        }
        
        previous_results = Some(current_results);
        
        // Wait for next iteration (unless this is the last one)
        if iteration < iterations {
            info!("Waiting {} minutes until next iteration...", interval);
            tokio::time::sleep(Duration::from_secs(interval * 60)).await;
        }
    }
    
    info!("Continuous benchmark mode completed");
    Ok(())
}

fn detect_regressions(
    previous: &[BenchmarkResult],
    current: &[BenchmarkResult],
    threshold: f64,
) -> Vec<BenchmarkComparison> {
    let comparisons = generate_comparison(previous, current);
    comparisons.into_iter()
        .filter(|comp| !comp.is_improvement && comp.change_percent.abs() > threshold)
        .collect()
}

fn generate_session_id() -> String {
    format!("bench_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"))
}

fn calculate_performance_score(results: &[UnifiedBenchmarkResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    
    let scores: Vec<f64> = results.iter()
        .filter_map(|r| {
            r.result.throughput.map(|t| {
                // Normalize throughput to a 0-100 scale (simplified)
                (t / 1000.0).min(100.0)
            })
        })
        .collect();
    
    if scores.is_empty() {
        50.0 // Default neutral score
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    }
}

fn calculate_avg_performance_score(results: &[BenchmarkResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    
    let scores: Vec<f64> = results.iter()
        .filter_map(|r| {
            r.throughput.map(|t| (t / 1000.0).min(100.0))
        })
        .collect();
    
    if scores.is_empty() {
        50.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    }
}

async fn generate_session_summary(session: &BenchmarkSession, output_dir: &std::path::Path) -> Result<()> {
    let summary_file = output_dir.join(format!("summary_{}.txt", session.session_id));
    
    let summary_text = format!(
        "MooseNG Benchmark Session Summary\n\
         ================================\n\
         \n\
         Session ID: {}\n\
         Start Time: {}\n\
         End Time: {}\n\
         Duration: {:.2} seconds\n\
         \n\
         Results:\n\
         - Total Benchmarks: {}\n\
         - Successful: {}\n\
         - Failed: {}\n\
         - Performance Score: {:.2}/100\n\
         \n\
         Configuration:\n\
         - Warmup Iterations: {}\n\
         - Measurement Iterations: {}\n\
         - File Sizes: {} entries\n\
         - Concurrency Levels: {:?}\n\
         \n\
         Benchmark Results:\n\
         {}\n",
        session.session_id,
        session.start_time.format("%Y-%m-%d %H:%M:%S UTC"),
        session.end_time.map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "In Progress".to_string()),
        session.summary.as_ref().map(|s| s.total_duration.as_secs_f64()).unwrap_or(0.0),
        session.summary.as_ref().map(|s| s.total_benchmarks).unwrap_or(0),
        session.summary.as_ref().map(|s| s.successful_benchmarks).unwrap_or(0),
        session.summary.as_ref().map(|s| s.failed_benchmarks).unwrap_or(0),
        session.summary.as_ref().map(|s| s.performance_score).unwrap_or(0.0),
        session.config.warmup_iterations,
        session.config.measurement_iterations,
        session.config.file_sizes.len(),
        session.config.concurrency_levels,
        session.results.iter()
            .map(|r| format!("  {} ({}): {:.2}ms", 
                           r.benchmark_name, r.suite_name, 
                           r.result.mean_time.as_millis() as f64))
            .collect::<Vec<_>>()
            .join("\n")
    );
    
    fs::write(&summary_file, summary_text).await?;
    info!("Session summary saved to: {}", summary_file.display());
    
    Ok(())
}

fn print_suite_summary(suite: &BenchmarkSuite, results: &[BenchmarkResult]) {
    println!("\n{}", "=".repeat(60));
    println!("Benchmark Suite: {}", suite.name);
    println!("{}", "=".repeat(60));
    
    let successful = results.iter().filter(|r| r.samples > 0).count();
    let total = results.len();
    
    println!("Results: {}/{} successful", successful, total);
    println!();
    
    println!("{:<30} {:>12} {:>15} {:>8}", "Operation", "Mean (ms)", "Throughput/s", "Status");
    println!("{}", "-".repeat(70));
    
    for result in results {
        let status = if result.samples > 0 { "✓" } else { "✗" };
        let throughput = result.throughput
            .map(|t| format!("{:.2}", t))
            .unwrap_or_else(|| "N/A".to_string());
        
        println!(
            "{:<30} {:>12.2} {:>15} {:>8}",
            result.operation,
            result.mean_time.as_millis() as f64,
            throughput,
            status
        );
    }
    
    println!();
}