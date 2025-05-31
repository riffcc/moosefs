//! Simple but comprehensive benchmark runner for MooseNG
//! 
//! This tool runs realistic performance tests that demonstrate MooseNG's capabilities

use clap::{Parser, Subcommand};
use mooseng_benchmarks::{
    BenchmarkConfig, Benchmark, BenchmarkResult,
    benchmarks::{
        network_file_operations::NetworkFileBenchmark,
        file_operations::{SmallFileBenchmark, LargeFileBenchmark, RandomAccessBenchmark},
        metadata_operations::{CreateDeleteBenchmark, ListingBenchmark, AttributesBenchmark},
        integration::{IntegrationBenchmark, NetworkPerformanceBenchmark},
    },
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{info, warn};
use tracing_subscriber;
use serde_json;

#[derive(Parser)]
#[command(name = "simple_benchmark")]
#[command(about = "Fast and comprehensive MooseNG benchmarks")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Number of iterations for each benchmark
    #[arg(short, long, default_value = "50")]
    iterations: usize,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Output directory for results
    #[arg(short, long, default_value = "./results")]
    output: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all benchmarks
    All {
        /// Quick mode (fewer iterations, smaller file sizes)
        #[arg(short, long)]
        quick: bool,
    },
    /// Run network benchmarks
    Network {
        /// Test real servers if available
        #[arg(short, long)]
        real: bool,
    },
    /// Run file operation benchmarks
    Files {
        /// Maximum file size to test (in MB)
        #[arg(short, long, default_value = "100")]
        max_size: usize,
    },
    /// Run integration benchmarks
    Integration {
        /// Number of concurrent clients to simulate
        #[arg(short, long, default_value = "50")]
        clients: usize,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("simple_benchmark={},mooseng_benchmarks={}", level, level))
        .init();

    // Create output directory
    fs::create_dir_all(&cli.output)?;
    
    let start_time = Instant::now();
    
    match cli.command {
        Commands::All { quick } => {
            info!("🚀 Running comprehensive MooseNG benchmarks");
            run_all_benchmarks(&cli.output, cli.iterations, quick)?;
        },
        Commands::Network { real } => {
            info!("🌐 Running network performance benchmarks");
            run_network_benchmarks(&cli.output, cli.iterations, real)?;
        },
        Commands::Files { max_size } => {
            info!("📁 Running file operation benchmarks");
            run_file_benchmarks(&cli.output, cli.iterations, max_size)?;
        },
        Commands::Integration { clients } => {
            info!("🔗 Running integration benchmarks");
            run_integration_benchmarks(&cli.output, cli.iterations, clients)?;
        },
    }
    
    let total_time = start_time.elapsed();
    info!("✅ All benchmarks completed in {:.2}s", total_time.as_secs_f64());
    info!("📊 Results saved to: {}", cli.output);
    
    Ok(())
}

fn run_all_benchmarks(output_dir: &str, iterations: usize, quick: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = if quick {
        BenchmarkConfig {
            warmup_iterations: 5,
            measurement_iterations: iterations.min(20),
            file_sizes: vec![1024, 65536, 1048576], // 1KB, 64KB, 1MB
            concurrency_levels: vec![1, 10, 25],
            regions: vec!["local".to_string()],
            detailed_report: false,
        }
    } else {
        BenchmarkConfig {
            warmup_iterations: 10,
            measurement_iterations: iterations,
            file_sizes: vec![1024, 4096, 65536, 1048576, 10485760], // 1KB to 10MB
            concurrency_levels: vec![1, 10, 50, 100],
            regions: vec!["us-east".to_string(), "eu-west".to_string(), "ap-south".to_string()],
            detailed_report: true,
        }
    };

    let mut all_results = HashMap::new();
    
    // File operations
    info!("📁 Running file operation benchmarks...");
    run_benchmark_category("small_files", SmallFileBenchmark::new(), &config, &mut all_results);
    run_benchmark_category("large_files", LargeFileBenchmark::new(), &config, &mut all_results);
    run_benchmark_category("random_access", RandomAccessBenchmark::new(), &config, &mut all_results);
    
    // Metadata operations
    info!("🗂️  Running metadata benchmarks...");
    run_benchmark_category("create_delete", CreateDeleteBenchmark::new(), &config, &mut all_results);
    run_benchmark_category("directory_listing", ListingBenchmark::new(), &config, &mut all_results);
    run_benchmark_category("file_attributes", AttributesBenchmark::new(), &config, &mut all_results);
    
    // Network operations
    info!("🌐 Running network benchmarks...");
    run_benchmark_category("network_files", NetworkFileBenchmark::new(), &config, &mut all_results);
    
    // Integration tests
    info!("🔗 Running integration benchmarks...");
    run_benchmark_category("integration", IntegrationBenchmark::new(), &config, &mut all_results);
    run_benchmark_category("network_performance", NetworkPerformanceBenchmark::new(), &config, &mut all_results);
    
    // Save results
    save_results(&all_results, output_dir)?;
    print_performance_summary(&all_results);
    
    Ok(())
}

fn run_network_benchmarks(output_dir: &str, iterations: usize, test_real: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = BenchmarkConfig {
        warmup_iterations: 10,
        measurement_iterations: iterations,
        file_sizes: vec![4096, 65536, 1048576, 10485760], // Network-relevant sizes
        concurrency_levels: vec![1, 10, 50, 100, 200],
        regions: vec!["us-east".to_string(), "eu-west".to_string(), "ap-south".to_string()],
        detailed_report: true,
    };

    let mut results = HashMap::new();
    
    if test_real {
        info!("Testing with real network endpoints...");
    } else {
        info!("Testing with simulated network conditions...");
    }
    
    run_benchmark_category("network_files", NetworkFileBenchmark::new(), &config, &mut results);
    run_benchmark_category("network_performance", NetworkPerformanceBenchmark::new(), &config, &mut results);
    
    save_results(&results, output_dir)?;
    print_performance_summary(&results);
    
    Ok(())
}

fn run_file_benchmarks(output_dir: &str, iterations: usize, max_size_mb: usize) -> Result<(), Box<dyn std::error::Error>> {
    let max_size = max_size_mb * 1024 * 1024;
    let mut file_sizes = vec![1024, 4096, 65536, 1048576]; // 1KB to 1MB
    
    // Add larger sizes up to max_size
    let mut size = 10 * 1024 * 1024; // Start at 10MB
    while size <= max_size {
        file_sizes.push(size);
        size *= 10; // 10MB, 100MB, 1GB, etc.
    }
    
    let config = BenchmarkConfig {
        warmup_iterations: 10,
        measurement_iterations: iterations,
        file_sizes,
        concurrency_levels: vec![1, 10, 50],
        regions: vec!["local".to_string()],
        detailed_report: true,
    };

    let mut results = HashMap::new();
    
    info!("Testing file sizes up to {} MB", max_size_mb);
    
    run_benchmark_category("small_files", SmallFileBenchmark::new(), &config, &mut results);
    run_benchmark_category("large_files", LargeFileBenchmark::new(), &config, &mut results);
    run_benchmark_category("random_access", RandomAccessBenchmark::new(), &config, &mut results);
    
    save_results(&results, output_dir)?;
    print_performance_summary(&results);
    
    Ok(())
}

fn run_integration_benchmarks(output_dir: &str, iterations: usize, max_clients: usize) -> Result<(), Box<dyn std::error::Error>> {
    let config = BenchmarkConfig {
        warmup_iterations: 5,
        measurement_iterations: iterations,
        file_sizes: vec![4096, 65536, 1048576], // Typical sizes for integration tests
        concurrency_levels: vec![1, 10, 25, 50, max_clients.min(200)],
        regions: vec!["us-east".to_string(), "eu-west".to_string(), "ap-south".to_string()],
        detailed_report: true,
    };

    let mut results = HashMap::new();
    
    info!("Testing with up to {} concurrent clients", max_clients);
    
    run_benchmark_category("integration", IntegrationBenchmark::new(), &config, &mut results);
    run_benchmark_category("network_performance", NetworkPerformanceBenchmark::new(), &config, &mut results);
    
    save_results(&results, output_dir)?;
    print_performance_summary(&results);
    
    Ok(())
}

fn run_benchmark_category<B: Benchmark>(
    name: &str, 
    benchmark: B, 
    config: &BenchmarkConfig,
    all_results: &mut HashMap<String, Vec<BenchmarkResult>>
) {
    info!("Running {} benchmark...", name);
    let start = Instant::now();
    
    let results = benchmark.run(config);
    let duration = start.elapsed();
    
    info!("✅ {} completed in {:.2}s ({} tests)", name, duration.as_secs_f64(), results.len());
    
    // Print quick summary
    if !results.is_empty() {
        let avg_latency: f64 = results.iter()
            .map(|r| r.mean_time.as_secs_f64() * 1000.0)
            .sum::<f64>() / results.len() as f64;
        
        let avg_throughput: f64 = results.iter()
            .filter_map(|r| r.throughput)
            .sum::<f64>() / results.iter().filter(|r| r.throughput.is_some()).count().max(1) as f64;
        
        info!("   Avg latency: {:.2}ms, Avg throughput: {:.2} MB/s", avg_latency, avg_throughput);
    }
    
    all_results.insert(name.to_string(), results);
}

fn save_results(results: &HashMap<String, Vec<BenchmarkResult>>, output_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    
    // Save JSON results
    let json_path = Path::new(output_dir).join(format!("benchmark_results_{}.json", timestamp));
    let json_data = serde_json::to_string_pretty(results)?;
    fs::write(&json_path, json_data)?;
    
    // Save CSV summary
    let csv_path = Path::new(output_dir).join(format!("benchmark_summary_{}.csv", timestamp));
    save_csv_summary(results, &csv_path)?;
    
    // Save markdown report
    let md_path = Path::new(output_dir).join(format!("benchmark_report_{}.md", timestamp));
    save_markdown_report(results, &md_path)?;
    
    info!("📄 Results saved:");
    info!("   JSON: {}", json_path.display());
    info!("   CSV:  {}", csv_path.display());
    info!("   MD:   {}", md_path.display());
    
    Ok(())
}

fn save_csv_summary(results: &HashMap<String, Vec<BenchmarkResult>>, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut csv_content = String::new();
    csv_content.push_str("category,operation,mean_time_ms,std_dev_ms,min_time_ms,max_time_ms,throughput_mbps,samples\n");
    
    for (category, benchmark_results) in results {
        for result in benchmark_results {
            csv_content.push_str(&format!(
                "{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{}\n",
                category,
                result.operation,
                result.mean_time.as_secs_f64() * 1000.0,
                result.std_dev.as_secs_f64() * 1000.0,
                result.min_time.as_secs_f64() * 1000.0,
                result.max_time.as_secs_f64() * 1000.0,
                result.throughput.unwrap_or(0.0),
                result.samples
            ));
        }
    }
    
    fs::write(path, csv_content)?;
    Ok(())
}

fn save_markdown_report(results: &HashMap<String, Vec<BenchmarkResult>>, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut md_content = String::new();
    
    md_content.push_str("# MooseNG Performance Benchmark Report\n\n");
    md_content.push_str(&format!("**Generated:** {}\n\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    
    // Overall summary
    let total_tests: usize = results.values().map(|v| v.len()).sum();
    let avg_latency: f64 = results.values()
        .flat_map(|v| v.iter())
        .map(|r| r.mean_time.as_secs_f64() * 1000.0)
        .sum::<f64>() / total_tests.max(1) as f64;
    
    let throughput_results: Vec<f64> = results.values()
        .flat_map(|v| v.iter())
        .filter_map(|r| r.throughput)
        .collect();
    let avg_throughput = if !throughput_results.is_empty() {
        throughput_results.iter().sum::<f64>() / throughput_results.len() as f64
    } else {
        0.0
    };
    
    md_content.push_str("## Summary\n\n");
    md_content.push_str(&format!("- **Total Tests:** {}\n", total_tests));
    md_content.push_str(&format!("- **Average Latency:** {:.2} ms\n", avg_latency));
    md_content.push_str(&format!("- **Average Throughput:** {:.2} MB/s\n", avg_throughput));
    md_content.push_str("\n");
    
    // Detailed results by category
    for (category, benchmark_results) in results {
        md_content.push_str(&format!("## {}\n\n", category.replace('_', " ").to_uppercase()));
        md_content.push_str("| Operation | Mean Time (ms) | Throughput (MB/s) | Samples |\n");
        md_content.push_str("|-----------|----------------|-------------------|----------|\n");
        
        for result in benchmark_results {
            md_content.push_str(&format!(
                "| {} | {:.3} | {:.3} | {} |\n",
                result.operation,
                result.mean_time.as_secs_f64() * 1000.0,
                result.throughput.unwrap_or(0.0),
                result.samples
            ));
        }
        md_content.push_str("\n");
    }
    
    fs::write(path, md_content)?;
    Ok(())
}

fn print_performance_summary(results: &HashMap<String, Vec<BenchmarkResult>>) {
    println!("\n{}", "=".repeat(80));
    println!("🎯 MOOSENG PERFORMANCE SUMMARY");
    println!("{}", "=".repeat(80));
    
    let total_tests: usize = results.values().map(|v| v.len()).sum();
    let total_samples: usize = results.values()
        .flat_map(|v| v.iter())
        .map(|r| r.samples)
        .sum();
    
    println!("📊 Total Tests: {}", total_tests);
    println!("🔢 Total Samples: {}", total_samples);
    
    // Calculate performance metrics
    let all_results: Vec<&BenchmarkResult> = results.values()
        .flat_map(|v| v.iter())
        .collect();
    
    if !all_results.is_empty() {
        let avg_latency: f64 = all_results.iter()
            .map(|r| r.mean_time.as_secs_f64() * 1000.0)
            .sum::<f64>() / all_results.len() as f64;
        
        let throughput_results: Vec<f64> = all_results.iter()
            .filter_map(|r| r.throughput)
            .collect();
        
        println!("⏱️  Average Latency: {:.2} ms", avg_latency);
        
        if !throughput_results.is_empty() {
            let avg_throughput = throughput_results.iter().sum::<f64>() / throughput_results.len() as f64;
            let max_throughput = throughput_results.iter().fold(0.0, |a, &b| a.max(b));
            
            println!("🚀 Average Throughput: {:.2} MB/s", avg_throughput);
            println!("🎯 Peak Throughput: {:.2} MB/s", max_throughput);
        }
        
        // Performance grade
        let grade = calculate_performance_grade(avg_latency, throughput_results.get(0).copied().unwrap_or(0.0));
        println!("🏆 Performance Grade: {}", grade);
    }
    
    println!("\n{}", "=".repeat(80));
    
    // Top performers and bottlenecks
    let mut top_performers = Vec::new();
    let mut bottlenecks = Vec::new();
    
    for (category, benchmark_results) in results {
        for result in benchmark_results {
            if let Some(throughput) = result.throughput {
                if throughput > 100.0 { // > 100 MB/s
                    top_performers.push((category, &result.operation, throughput));
                }
            }
            
            if result.mean_time.as_millis() > 100 { // > 100ms
                bottlenecks.push((category, &result.operation, result.mean_time.as_secs_f64() * 1000.0));
            }
        }
    }
    
    if !top_performers.is_empty() {
        println!("🌟 TOP PERFORMERS:");
        top_performers.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        for (category, operation, throughput) in top_performers.iter().take(5) {
            println!("   • {}/{}: {:.1} MB/s", category, operation, throughput);
        }
        println!();
    }
    
    if !bottlenecks.is_empty() {
        println!("⚠️  PERFORMANCE BOTTLENECKS:");
        bottlenecks.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        for (category, operation, latency) in bottlenecks.iter().take(5) {
            println!("   • {}/{}: {:.1} ms", category, operation, latency);
        }
        println!();
    }
}

fn calculate_performance_grade(avg_latency_ms: f64, avg_throughput_mbps: f64) -> String {
    let mut score = 100.0;
    
    // Latency penalties
    if avg_latency_ms > 100.0 {
        score -= 30.0;
    } else if avg_latency_ms > 50.0 {
        score -= 20.0;
    } else if avg_latency_ms > 20.0 {
        score -= 10.0;
    }
    
    // Throughput bonuses/penalties
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