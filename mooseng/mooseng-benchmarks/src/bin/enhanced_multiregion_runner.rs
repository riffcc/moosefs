//! Enhanced Multi-Region Benchmark Runner
//! 
//! A specialized CLI tool for running comprehensive multi-region benchmarks
//! with async runtime optimization and network resilience testing.

use clap::{Arg, Command};
use mooseng_benchmarks::{
    BenchmarkConfig,
    benchmarks::enhanced_multiregion::{EnhancedMultiRegionBenchmark, MultiRegionBenchmarkResult},
    Benchmark,
};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let matches = Command::new("Enhanced Multi-Region Benchmark")
        .version("1.0.0")
        .author("MooseNG Team")
        .about("Advanced benchmarking for multi-region MooseNG deployments")
        .arg(
            Arg::new("iterations")
                .short('i')
                .long("iterations")
                .value_name("NUMBER")
                .help("Number of benchmark iterations")
                .default_value("100")
        )
        .arg(
            Arg::new("warmup")
                .short('w')
                .long("warmup")
                .value_name("NUMBER")
                .help("Number of warmup iterations")
                .default_value("10")
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output file for detailed results")
                .default_value("enhanced_multiregion_results.json")
        )
        .arg(
            Arg::new("test-type")
                .short('t')
                .long("test-type")
                .value_name("TYPE")
                .help("Type of test to run")
                .value_parser(["all", "scheduling", "streaming", "resilience"])
                .default_value("all")
        )
        .arg(
            Arg::new("regions")
                .short('r')
                .long("regions")
                .value_name("LIST")
                .help("Comma-separated list of regions to test")
                .default_value("us-east-1,eu-west-1,ap-south-1")
        )
        .arg(
            Arg::new("detailed")
                .short('d')
                .long("detailed")
                .help("Enable detailed performance analysis")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("export-format")
                .long("export-format")
                .value_name("FORMAT")
                .help("Export format for results")
                .value_parser(["json", "csv", "markdown"])
                .default_value("json")
        )
        .get_matches();

    // Parse command line arguments
    let iterations: usize = matches.get_one::<String>("iterations")
        .unwrap()
        .parse()
        .expect("Invalid iterations value");

    let warmup: usize = matches.get_one::<String>("warmup")
        .unwrap()
        .parse()
        .expect("Invalid warmup value");

    let output_file = matches.get_one::<String>("output").unwrap();
    let test_type = matches.get_one::<String>("test-type").unwrap();
    let regions_str = matches.get_one::<String>("regions").unwrap();
    let detailed = matches.get_flag("detailed");
    let export_format = matches.get_one::<String>("export-format").unwrap();

    let regions: Vec<String> = regions_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    info!("Starting Enhanced Multi-Region Benchmark");
    info!("Configuration:");
    info!("  Iterations: {}", iterations);
    info!("  Warmup: {}", warmup);
    info!("  Test Type: {}", test_type);
    info!("  Regions: {:?}", regions);
    info!("  Detailed Analysis: {}", detailed);
    info!("  Output: {}", output_file);

    // Create benchmark configuration
    let config = BenchmarkConfig {
        warmup_iterations: warmup,
        measurement_iterations: iterations,
        file_sizes: vec![1024, 65536, 1048576, 10485760], // 1KB to 10MB
        concurrency_levels: vec![1, 10, 50, 100],
        regions: regions.clone(),
        detailed_report: detailed,
    };

    // Initialize and run benchmarks
    let mut benchmark = EnhancedMultiRegionBenchmark::new();
    
    info!("Initializing multi-region scheduler...");
    if let Err(e) = benchmark.initialize_scheduler().await {
        error!("Failed to initialize scheduler: {:?}", e);
        return Err(e);
    }

    let mut all_results = Vec::new();

    match test_type.as_str() {
        "all" => {
            info!("Running all benchmark types...");
            all_results.extend(run_all_benchmarks(&benchmark, &config).await?);
        }
        "scheduling" => {
            info!("Running task scheduling benchmarks...");
            let result = run_task_scheduling_benchmark(&benchmark, &config).await?;
            all_results.push(("task_scheduling".to_string(), result));
        }
        "streaming" => {
            info!("Running stream processing benchmarks...");
            let result = run_stream_processing_benchmark(&benchmark, &config).await?;
            all_results.push(("stream_processing".to_string(), result));
        }
        "resilience" => {
            info!("Running network resilience benchmarks...");
            let results = run_network_resilience_benchmark(&benchmark, &config).await?;
            for (name, result) in results {
                all_results.push((format!("resilience_{}", name), result));
            }
        }
        _ => {
            error!("Unknown test type: {}", test_type);
            return Err(anyhow::anyhow!("Invalid test type"));
        }
    }

    // Export results
    match export_format.as_str() {
        "json" => export_json_results(&all_results, output_file)?,
        "csv" => export_csv_results(&all_results, output_file)?,
        "markdown" => export_markdown_results(&all_results, output_file)?,
        _ => {
            error!("Unknown export format: {}", export_format);
            return Err(anyhow::anyhow!("Invalid export format"));
        }
    }

    // Print summary
    print_summary(&all_results);

    info!("Benchmark completed successfully!");
    info!("Results exported to: {}", output_file);

    Ok(())
}

async fn run_all_benchmarks(
    benchmark: &EnhancedMultiRegionBenchmark,
    config: &BenchmarkConfig,
) -> anyhow::Result<Vec<(String, MultiRegionBenchmarkResult)>> {
    let mut results = Vec::new();

    // Task scheduling
    let scheduling_result = run_task_scheduling_benchmark(benchmark, config).await?;
    results.push(("task_scheduling".to_string(), scheduling_result));

    // Stream processing
    let streaming_result = run_stream_processing_benchmark(benchmark, config).await?;
    results.push(("stream_processing".to_string(), streaming_result));

    // Network resilience
    let resilience_results = run_network_resilience_benchmark(benchmark, config).await?;
    for (name, result) in resilience_results {
        results.push((format!("resilience_{}", name), result));
    }

    Ok(results)
}

async fn run_task_scheduling_benchmark(
    benchmark: &EnhancedMultiRegionBenchmark,
    config: &BenchmarkConfig,
) -> anyhow::Result<MultiRegionBenchmarkResult> {
    info!("Starting task scheduling benchmark...");
    let result = benchmark.benchmark_task_scheduling(config).await;
    info!("Task scheduling benchmark completed");
    Ok(result)
}

async fn run_stream_processing_benchmark(
    benchmark: &EnhancedMultiRegionBenchmark,
    config: &BenchmarkConfig,
) -> anyhow::Result<MultiRegionBenchmarkResult> {
    info!("Starting adaptive stream processing benchmark...");
    let result = benchmark.benchmark_adaptive_stream_processing(config).await;
    info!("Stream processing benchmark completed");
    Ok(result)
}

async fn run_network_resilience_benchmark(
    benchmark: &EnhancedMultiRegionBenchmark,
    config: &BenchmarkConfig,
) -> anyhow::Result<Vec<(String, MultiRegionBenchmarkResult)>> {
    info!("Starting network resilience benchmarks...");
    let results = benchmark.benchmark_network_resilience(config).await;
    info!("Network resilience benchmarks completed");
    
    let named_results = results.into_iter()
        .enumerate()
        .map(|(i, result)| {
            let condition_name = match i {
                0 => "normal",
                1 => "high_latency",
                2 => "low_bandwidth",
                3 => "packet_loss",
                4 => "region_partition",
                _ => "unknown",
            };
            (condition_name.to_string(), result)
        })
        .collect();
    
    Ok(named_results)
}

fn export_json_results(
    results: &[(String, MultiRegionBenchmarkResult)],
    output_file: &str,
) -> anyhow::Result<()> {
    let json_data = serde_json::to_string_pretty(results)?;
    let mut file = File::create(output_file)?;
    file.write_all(json_data.as_bytes())?;
    Ok(())
}

fn export_csv_results(
    results: &[(String, MultiRegionBenchmarkResult)],
    output_file: &str,
) -> anyhow::Result<()> {
    let mut file = File::create(output_file)?;
    
    // Write CSV header
    writeln!(
        file,
        "test_name,operation,mean_time_ms,throughput,samples,cpu_usage,memory_usage,error_rate"
    )?;
    
    // Write data rows
    for (test_name, result) in results {
        let mean_time_ms = result.base.mean_time.as_millis();
        let throughput = result.base.throughput.unwrap_or(0.0);
        let samples = result.base.samples;
        
        // Calculate average resource usage across regions
        let (avg_cpu, avg_memory, avg_error_rate) = result.regional_metrics.values()
            .fold((0.0, 0.0, 0.0), |(cpu, mem, err), metrics| {
                (
                    cpu + metrics.resource_utilization.cpu_usage,
                    mem + metrics.resource_utilization.memory_usage,
                    err + metrics.error_rate,
                )
            });
        
        let region_count = result.regional_metrics.len() as f64;
        let avg_cpu = avg_cpu / region_count;
        let avg_memory = avg_memory / region_count;
        let avg_error_rate = avg_error_rate / region_count;
        
        writeln!(
            file,
            "{},{},{},{},{},{:.3},{:.3},{:.3}",
            test_name,
            result.base.operation,
            mean_time_ms,
            throughput,
            samples,
            avg_cpu,
            avg_memory,
            avg_error_rate
        )?;
    }
    
    Ok(())
}

fn export_markdown_results(
    results: &[(String, MultiRegionBenchmarkResult)],
    output_file: &str,
) -> anyhow::Result<()> {
    let mut file = File::create(output_file)?;
    
    writeln!(file, "# Enhanced Multi-Region Benchmark Results")?;
    writeln!(file)?;
    writeln!(file, "Generated: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"))?;
    writeln!(file)?;
    
    for (test_name, result) in results {
        writeln!(file, "## {}", test_name)?;
        writeln!(file)?;
        
        // Base metrics
        writeln!(file, "### Base Performance")?;
        writeln!(file, "- **Operation**: {}", result.base.operation)?;
        writeln!(file, "- **Mean Time**: {:?}", result.base.mean_time)?;
        writeln!(file, "- **Throughput**: {:.2} ops/s", result.base.throughput.unwrap_or(0.0))?;
        writeln!(file, "- **Samples**: {}", result.base.samples)?;
        writeln!(file)?;
        
        // Regional metrics
        writeln!(file, "### Regional Performance")?;
        writeln!(file, "| Region | Completion Rate | Avg Time | CPU Usage | Memory Usage | Error Rate |")?;
        writeln!(file, "|--------|-----------------|----------|-----------|--------------|------------|")?;
        
        for (region, metrics) in &result.regional_metrics {
            writeln!(
                file,
                "| {} | {:.2}% | {:?} | {:.1}% | {:.1}% | {:.2}% |",
                region,
                metrics.task_completion_rate * 100.0,
                metrics.average_execution_time,
                metrics.resource_utilization.cpu_usage * 100.0,
                metrics.resource_utilization.memory_usage * 100.0,
                metrics.error_rate * 100.0
            )?;
        }
        writeln!(file)?;
        
        // Cross-region metrics
        writeln!(file, "### Cross-Region Metrics")?;
        writeln!(file, "- **Message Success Rate**: {:.2}%", result.cross_region_metrics.message_success_rate * 100.0)?;
        if let Some(failover_time) = result.cross_region_metrics.failover_time {
            writeln!(file, "- **Failover Time**: {:?}", failover_time)?;
        }
        writeln!(file, "- **Conflict Resolution Time**: {:?}", result.cross_region_metrics.conflict_resolution_time)?;
        writeln!(file)?;
        
        // Consistency metrics
        writeln!(file, "### Consistency Metrics")?;
        writeln!(file, "- **Read Violations**: {}", result.consistency_metrics.read_consistency_violations)?;
        writeln!(file, "- **Write Propagation**: {:?}", result.consistency_metrics.write_propagation_time)?;
        writeln!(file, "- **Conflict Resolution Success**: {:.2}%", result.consistency_metrics.conflict_resolution_success_rate * 100.0)?;
        writeln!(file)?;
        
        // Adaptive performance
        writeln!(file, "### Adaptive Performance")?;
        writeln!(file, "- **Load Balancing Efficiency**: {:.2}%", result.adaptive_performance.load_balancing_efficiency * 100.0)?;
        writeln!(file, "- **Auto-Scaling Response**: {:?}", result.adaptive_performance.auto_scaling_response_time)?;
        writeln!(file, "- **Resource Optimization Gain**: {:.2}%", result.adaptive_performance.resource_optimization_gain * 100.0)?;
        writeln!(file, "- **Adaptation Accuracy**: {:.2}%", result.adaptive_performance.adaptation_accuracy * 100.0)?;
        writeln!(file)?;
    }
    
    Ok(())
}

fn print_summary(results: &[(String, MultiRegionBenchmarkResult)]) {
    println!("\n=== ENHANCED MULTI-REGION BENCHMARK SUMMARY ===");
    println!();
    
    for (test_name, result) in results {
        println!("📊 {}", test_name.to_uppercase());
        println!("   Operation: {}", result.base.operation);
        println!("   Mean Time: {:?}", result.base.mean_time);
        
        if let Some(throughput) = result.base.throughput {
            println!("   Throughput: {:.2} ops/s", throughput);
        }
        
        // Show best and worst performing regions
        if !result.regional_metrics.is_empty() {
            let (best_region, best_metrics) = result.regional_metrics
                .iter()
                .max_by(|(_, a), (_, b)| a.task_completion_rate.partial_cmp(&b.task_completion_rate).unwrap())
                .unwrap();
            
            let (worst_region, worst_metrics) = result.regional_metrics
                .iter()
                .min_by(|(_, a), (_, b)| a.task_completion_rate.partial_cmp(&b.task_completion_rate).unwrap())
                .unwrap();
            
            println!("   Best Region: {} ({:.1}% completion)", best_region, best_metrics.task_completion_rate * 100.0);
            println!("   Worst Region: {} ({:.1}% completion)", worst_region, worst_metrics.task_completion_rate * 100.0);
        }
        
        println!("   Adaptive Efficiency: {:.1}%", result.adaptive_performance.load_balancing_efficiency * 100.0);
        println!();
    }
    
    // Overall summary
    let total_tests = results.len();
    let avg_throughput = results.iter()
        .filter_map(|(_, r)| r.base.throughput)
        .sum::<f64>() / total_tests as f64;
    
    println!("📈 OVERALL SUMMARY");
    println!("   Total Tests: {}", total_tests);
    println!("   Average Throughput: {:.2} ops/s", avg_throughput);
    
    // Performance grade
    let grade = if avg_throughput > 1000.0 {
        "A+ (Excellent)"
    } else if avg_throughput > 500.0 {
        "A (Very Good)"
    } else if avg_throughput > 100.0 {
        "B (Good)"
    } else if avg_throughput > 50.0 {
        "C (Fair)"
    } else {
        "D (Needs Improvement)"
    };
    
    println!("   Performance Grade: {}", grade);
    println!();
}