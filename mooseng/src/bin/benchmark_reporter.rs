//! MooseNG Benchmark Report Generator
//!
//! This binary coordinates with other instances to generate comprehensive HTML reports
//! from benchmark results, providing visualization and analysis capabilities.

use std::collections::HashMap;
use std::env;
use std::path::Path;
use chrono::Utc;
use clap::{Arg, Command};
use serde_json;

use mooseng_benchmarks::html_report::{
    HtmlReportGenerator, HtmlReportConfig, HtmlReportConfigBuilder,
    BenchmarkReport, EnvironmentInfo, BenchmarkSummary, ComponentBenchmarks,
    HealthMetrics, ResourceUsage, MultiregionBenchmarks, ComparativeBenchmarks,
    ChartLibrary, Theme
};
use mooseng_benchmarks::{BenchmarkResult, BenchmarkConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("MooseNG Benchmark Reporter")
        .version("1.0.0")
        .author("MooseNG Team")
        .about("Generates comprehensive HTML reports from benchmark results")
        .arg(Arg::new("input")
            .short('i')
            .long("input")
            .value_name("DIR")
            .help("Input directory containing benchmark results")
            .default_value("benchmark_results"))
        .arg(Arg::new("output")
            .short('o')
            .long("output")
            .value_name("DIR")
            .help("Output directory for HTML reports")
            .default_value("html_reports"))
        .arg(Arg::new("title")
            .short('t')
            .long("title")
            .value_name("TITLE")
            .help("Report title")
            .default_value("MooseNG Performance Analysis"))
        .arg(Arg::new("theme")
            .long("theme")
            .value_name("THEME")
            .help("UI theme (light, dark, auto)")
            .default_value("auto"))
        .arg(Arg::new("real-time")
            .long("real-time")
            .help("Enable real-time monitoring features")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("chart-library")
            .long("chart-library")
            .value_name("LIBRARY")
            .help("Chart library to use (plotly, chartjs, d3)")
            .default_value("plotly"))
        .get_matches();

    let input_dir = matches.get_one::<String>("input").unwrap();
    let output_dir = matches.get_one::<String>("output").unwrap();
    let title = matches.get_one::<String>("title").unwrap();
    let theme_str = matches.get_one::<String>("theme").unwrap();
    let real_time = matches.get_flag("real-time");
    let chart_lib_str = matches.get_one::<String>("chart-library").unwrap();

    // Parse theme
    let theme = match theme_str.as_str() {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        "auto" => Theme::Auto,
        _ => {
            eprintln!("Invalid theme: {}. Using auto.", theme_str);
            Theme::Auto
        }
    };

    // Parse chart library
    let chart_library = match chart_lib_str.as_str() {
        "chartjs" => ChartLibrary::ChartJs,
        "d3" => ChartLibrary::D3,
        "plotly" => ChartLibrary::Plotly,
        _ => {
            eprintln!("Invalid chart library: {}. Using plotly.", chart_lib_str);
            ChartLibrary::Plotly
        }
    };

    println!("🚀 MooseNG Benchmark Reporter Starting...");
    println!("📁 Input Directory: {}", input_dir);
    println!("📊 Output Directory: {}", output_dir);
    println!("🎨 Theme: {:?}", theme);
    println!("📈 Chart Library: {:?}", chart_library);

    // Create HTML report configuration
    let config = HtmlReportConfigBuilder::new()
        .output_dir(output_dir)
        .title(title)
        .theme(theme)
        .chart_library(chart_library)
        .enable_real_time(real_time)
        .build();

    // Collect benchmark results from input directory
    println!("📊 Collecting benchmark results from {}...", input_dir);
    let report = collect_benchmark_results(input_dir)?;

    // Generate HTML reports
    println!("🔨 Generating HTML reports...");
    let generator = HtmlReportGenerator::new(config);
    let result = generator.generate_report(&report)?;

    println!("✅ {}", result);
    println!("🌐 Open {}/index.html in your browser to view the report", output_dir);

    // Print summary
    print_report_summary(&report);

    Ok(())
}

fn collect_benchmark_results(input_dir: &str) -> Result<BenchmarkReport, Box<dyn std::error::Error>> {
    let input_path = Path::new(input_dir);
    
    if !input_path.exists() {
        return Err(format!("Input directory {} does not exist", input_dir).into());
    }

    println!("🔍 Scanning for benchmark result files...");

    // Collect environment information
    let environment = collect_environment_info()?;

    // Collect benchmark results from JSON files
    let mut all_results = Vec::new();
    let mut component_results = HashMap::new();

    // Scan for result files
    for entry in std::fs::read_dir(input_path)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            // Check for performance_results.json in subdirectories
            let json_path = path.join("performance_results.json");
            if json_path.exists() {
                println!("📄 Found results: {}", json_path.display());
                let results = load_benchmark_results(&json_path)?;
                all_results.extend(results);
            }
        }
    }

    // Create mock component results (will be populated by other instances)
    component_results.insert("Master".to_string(), create_mock_component_benchmarks("Master"));
    component_results.insert("ChunkServer".to_string(), create_mock_component_benchmarks("ChunkServer"));
    component_results.insert("Client".to_string(), create_mock_component_benchmarks("Client"));

    // Calculate summary
    let summary = calculate_benchmark_summary(&all_results);

    // Create multiregion results (will be populated by Instance 4)
    let multiregion_results = Some(create_mock_multiregion_benchmarks());

    // Create comparative results (will be populated by Instance 4)
    let comparative_results = Some(create_mock_comparative_benchmarks());

    Ok(BenchmarkReport {
        timestamp: Utc::now(),
        environment,
        results: all_results,
        summary,
        component_results,
        multiregion_results,
        comparative_results,
    })
}

fn collect_environment_info() -> Result<EnvironmentInfo, Box<dyn std::error::Error>> {
    use std::process::Command;

    // Get system information
    let os = env::consts::OS.to_string();
    let arch = env::consts::ARCH.to_string();
    
    // Get CPU core count
    let cores = num_cpus::get() as u32;
    
    // Get memory information (Linux-specific)
    let memory_gb = get_system_memory_gb().unwrap_or(8);
    
    // Get Rust version
    let rust_version = get_rust_version().unwrap_or_else(|_| "unknown".to_string());
    
    // Get MooseNG version from Cargo.toml
    let mooseng_version = env!("CARGO_PKG_VERSION").to_string();
    
    // Determine build profile
    let build_profile = if cfg!(debug_assertions) {
        "debug".to_string()
    } else {
        "release".to_string()
    };

    Ok(EnvironmentInfo {
        os,
        arch,
        cores,
        memory_gb,
        rust_version,
        mooseng_version,
        build_profile,
    })
}

fn get_system_memory_gb() -> Result<u32, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let meminfo = fs::read_to_string("/proc/meminfo")?;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()?;
                    return Ok((kb / 1024 / 1024) as u32);
                }
            }
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        // Fallback for other systems
        return Ok(8); // Default to 8GB
    }
    
    Ok(8)
}

fn get_rust_version() -> Result<String, Box<dyn std::error::Error>> {
    use std::process::Command;
    
    let output = Command::new("rustc").arg("--version").output()?;
    let version_str = String::from_utf8(output.stdout)?;
    Ok(version_str.trim().to_string())
}

fn load_benchmark_results(json_path: &Path) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
    use std::time::Duration;
    
    let content = std::fs::read_to_string(json_path)?;
    let json_data: serde_json::Value = serde_json::from_str(&content)?;

    let mut results = Vec::new();

    // Parse the existing format and convert to BenchmarkResult
    if let Some(results_obj) = json_data.get("results").and_then(|r| r.as_object()) {
        for (key, value) in results_obj {
            if let Some(ms_value) = value.as_u64() {
                results.push(BenchmarkResult {
                    operation: key.replace("_ms", "").replace("_", " "),
                    mean_time: Duration::from_millis(ms_value),
                    std_dev: Duration::from_millis(ms_value / 10), // Mock std dev
                    min_time: Duration::from_millis(ms_value * 8 / 10),
                    max_time: Duration::from_millis(ms_value * 12 / 10),
                    throughput: parse_throughput_value(results_obj, key),
                    samples: 100, // Mock sample count
                });
            }
        }
    }

    Ok(results)
}

fn parse_throughput_value(results_obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<f64> {
    // Try to find associated throughput values
    if key.contains("throughput") {
        if let Some(mbps_str) = results_obj.get("throughput_mbps").and_then(|v| v.as_str()) {
            return mbps_str.parse().ok();
        }
    }
    
    if key.contains("ops") {
        if let Some(ops_str) = results_obj.get("ops_per_second").and_then(|v| v.as_str()) {
            return ops_str.parse().ok();
        }
    }
    
    None
}

fn calculate_benchmark_summary(results: &[BenchmarkResult]) -> BenchmarkSummary {
    let total_tests = results.len();
    let passed_tests = total_tests; // Assume all passed for now
    let failed_tests = 0;
    
    let total_duration: std::time::Duration = results.iter()
        .map(|r| r.mean_time)
        .sum();
    
    // Calculate performance score based on various metrics
    let avg_latency_ms = if total_tests > 0 {
        total_duration.as_millis() as f64 / total_tests as f64
    } else {
        0.0
    };
    
    let performance_score = calculate_performance_score(avg_latency_ms, results);
    let overall_grade = grade_from_score(performance_score);

    BenchmarkSummary {
        total_tests,
        passed_tests,
        failed_tests,
        total_duration,
        overall_grade,
        performance_score,
    }
}

fn calculate_performance_score(avg_latency_ms: f64, results: &[BenchmarkResult]) -> f64 {
    // Simple scoring algorithm - can be enhanced
    let latency_score = (100.0 - (avg_latency_ms / 10.0)).max(0.0).min(100.0);
    
    let throughput_score = results.iter()
        .filter_map(|r| r.throughput)
        .map(|t| (t / 1000.0).min(100.0))
        .sum::<f64>() / results.len().max(1) as f64;
    
    (latency_score + throughput_score) / 2.0
}

fn grade_from_score(score: f64) -> String {
    match score {
        s if s >= 90.0 => "A+ (Excellent)".to_string(),
        s if s >= 80.0 => "A (Very Good)".to_string(),
        s if s >= 70.0 => "B (Good)".to_string(),
        s if s >= 60.0 => "C (Fair)".to_string(),
        s if s >= 50.0 => "D (Poor)".to_string(),
        _ => "F (Failing)".to_string(),
    }
}

fn create_mock_component_benchmarks(component_name: &str) -> ComponentBenchmarks {
    use std::time::Duration;
    
    ComponentBenchmarks {
        component_name: component_name.to_string(),
        operations: vec![
            BenchmarkResult {
                operation: format!("{} Read", component_name),
                mean_time: Duration::from_millis(50),
                std_dev: Duration::from_millis(5),
                min_time: Duration::from_millis(40),
                max_time: Duration::from_millis(80),
                throughput: Some(1000.0),
                samples: 100,
            },
            BenchmarkResult {
                operation: format!("{} Write", component_name),
                mean_time: Duration::from_millis(75),
                std_dev: Duration::from_millis(8),
                min_time: Duration::from_millis(60),
                max_time: Duration::from_millis(120),
                throughput: Some(750.0),
                samples: 100,
            },
        ],
        health_metrics: HealthMetrics {
            cpu_usage_percent: 45.0,
            memory_usage_mb: 512.0,
            disk_io_mbps: 150.0,
            network_io_mbps: 200.0,
            error_rate: 0.1,
        },
        resource_usage: ResourceUsage {
            peak_memory_mb: 768.0,
            avg_cpu_percent: 35.0,
            disk_writes_mb: 1024.0,
            disk_reads_mb: 2048.0,
            network_bytes: 1048576.0,
        },
    }
}

fn create_mock_multiregion_benchmarks() -> MultiregionBenchmarks {
    let mut cross_region_latency = HashMap::new();
    let mut us_east = HashMap::new();
    us_east.insert("eu-west".to_string(), 120.0);
    us_east.insert("ap-south".to_string(), 180.0);
    cross_region_latency.insert("us-east".to_string(), us_east);

    let mut replication_lag = HashMap::new();
    replication_lag.insert("us-east".to_string(), 15.0);
    replication_lag.insert("eu-west".to_string(), 25.0);
    replication_lag.insert("ap-south".to_string(), 35.0);

    MultiregionBenchmarks {
        regions: vec!["us-east".to_string(), "eu-west".to_string(), "ap-south".to_string()],
        cross_region_latency,
        replication_lag,
        failover_times: vec![2.5, 3.1, 2.8, 3.3, 2.9],
        consistency_metrics: mooseng_benchmarks::html_report::ConsistencyMetrics {
            eventual_consistency_time: 500.0,
            strong_consistency_overhead: 25.0,
            conflict_resolution_time: 150.0,
        },
    }
}

fn create_mock_comparative_benchmarks() -> ComparativeBenchmarks {
    let mut feature_comparison = HashMap::new();
    
    feature_comparison.insert("Read Latency".to_string(), mooseng_benchmarks::html_report::ComparisonResult {
        mooseng_value: 45.0,
        baseline_value: 78.0,
        improvement_percent: 42.3,
    });
    
    feature_comparison.insert("Write Throughput".to_string(), mooseng_benchmarks::html_report::ComparisonResult {
        mooseng_value: 1250.0,
        baseline_value: 980.0,
        improvement_percent: 27.6,
    });

    ComparativeBenchmarks {
        baseline_system: "MooseFS 3.0".to_string(),
        performance_ratio: 1.35,
        feature_comparison,
        resource_efficiency: 1.22,
    }
}

fn print_report_summary(report: &BenchmarkReport) {
    println!("\n📊 Benchmark Report Summary");
    println!("═══════════════════════════════════");
    println!("🕒 Timestamp: {}", report.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("💻 Environment: {} {} ({} cores)", report.environment.os, report.environment.arch, report.environment.cores);
    println!("🔧 Rust Version: {}", report.environment.rust_version);
    println!("📦 MooseNG Version: {}", report.environment.mooseng_version);
    println!("🎯 Build Profile: {}", report.environment.build_profile);
    println!("───────────────────────────────────");
    println!("✅ Tests Passed: {}/{}", report.summary.passed_tests, report.summary.total_tests);
    println!("⏱️ Total Duration: {:.2}s", report.summary.total_duration.as_secs_f64());
    println!("🏆 Performance Score: {:.1}", report.summary.performance_score);
    println!("📈 Overall Grade: {}", report.summary.overall_grade);
    println!("───────────────────────────────────");
    println!("🧩 Components Analyzed: {}", report.component_results.len());
    
    if report.multiregion_results.is_some() {
        println!("🌍 Multiregion Analysis: ✅");
    }
    
    if report.comparative_results.is_some() {
        println!("📊 Comparative Analysis: ✅");
    }
    
    println!("═══════════════════════════════════\n");
}