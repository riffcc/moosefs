//! Comprehensive benchmark runner for MooseNG
//! 
//! This binary coordinates all benchmark types and generates
//! detailed performance reports for the MooseNG system.

use clap::Parser;
use mooseng_benchmarks::{
    benchmarks::BenchmarkSuite,
    config::BenchmarkConfig,
    report::ReportGenerator,
    BenchmarkResult,
};
use std::path::PathBuf;
use std::time::Instant;
use tracing::{info, warn, error};

#[derive(Parser)]
#[command(name = "comprehensive_bench")]
#[command(about = "Comprehensive MooseNG benchmark runner")]
pub struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "test_configs/benchmark_config.toml")]
    config: PathBuf,
    
    /// Output file for results
    #[arg(short, long, default_value = "test_results/comprehensive_results.json")]
    output: PathBuf,
    
    /// Master server address
    #[arg(long, default_value = "127.0.0.1:9420")]
    master_address: String,
    
    /// Run specific benchmark categories
    #[arg(long, value_delimiter = ',')]
    categories: Option<Vec<String>>,
    
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
    
    /// Number of parallel benchmark threads
    #[arg(long, default_value = "1")]
    threads: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Setup logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("comprehensive_bench={},mooseng_benchmarks={}", log_level, log_level))
        .init();
    
    info!("Starting MooseNG comprehensive benchmark suite");
    info!("Configuration: {:?}", args.config);
    info!("Output: {:?}", args.output);
    info!("Master address: {}", args.master_address);
    
    // Load configuration
    let config = if args.config.exists() {
        match std::fs::read_to_string(&args.config) {
            Ok(content) => {
                match toml::from_str::<BenchmarkConfig>(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        warn!("Failed to parse config file: {}. Using defaults.", e);
                        BenchmarkConfig::default()
                    }
                }
            },
            Err(e) => {
                warn!("Failed to read config file: {}. Using defaults.", e);
                BenchmarkConfig::default()
            }
        }
    } else {
        info!("Config file not found, using defaults");
        BenchmarkConfig::default()
    };
    
    // Initialize benchmark suite
    let suite = BenchmarkSuite::new();
    
    // Run benchmarks
    let start_time = Instant::now();
    
    let results = if let Some(categories) = args.categories {
        info!("Running selected categories: {:?}", categories);
        let category_refs: Vec<&str> = categories.iter().map(|s| s.as_str()).collect();
        suite.run_selected(&category_refs, &config)
    } else {
        info!("Running all available benchmarks");
        suite.run_all(&config)
    };
    
    let total_duration = start_time.elapsed();
    
    // Process results
    let mut total_benchmarks = 0;
    let mut successful_benchmarks = 0;
    
    for (benchmark_name, benchmark_results) in &results {
        total_benchmarks += benchmark_results.len();
        successful_benchmarks += benchmark_results.iter()
            .filter(|r| r.samples > 0)
            .count();
            
        info!("Completed {}: {} results", benchmark_name, benchmark_results.len());
    }
    
    info!("Benchmark suite completed in {:?}", total_duration);
    info!("Total benchmarks: {}, Successful: {}", total_benchmarks, successful_benchmarks);
    
    // Save results
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let output_data = serde_json::json!({
        "metadata": {
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "total_duration_secs": total_duration.as_secs_f64(),
            "master_address": args.master_address,
            "total_benchmarks": total_benchmarks,
            "successful_benchmarks": successful_benchmarks,
            "config": config
        },
        "results": results
    });
    
    std::fs::write(&args.output, serde_json::to_string_pretty(&output_data)?)?;
    info!("Results saved to {:?}", args.output);
    
    // Generate HTML report
    let html_output = args.output.with_extension("html");
    match ReportGenerator::generate_html_report(&results, &html_output) {
        Ok(_) => info!("HTML report generated: {:?}", html_output),
        Err(e) => error!("Failed to generate HTML report: {}", e),
    }
    
    if successful_benchmarks < total_benchmarks {
        warn!("Some benchmarks failed. Check logs for details.");
        std::process::exit(1);
    }
    
    info!("Comprehensive benchmark suite completed successfully!");
    Ok(())
}