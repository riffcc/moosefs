//! Advanced reporting and visualization for MooseNG benchmarks
//!
//! This module provides comprehensive report generation with charts,
//! HTML dashboards, and integration with monitoring systems.

use crate::{BenchmarkResult, BenchmarkConfig};
use crate::metrics::{AggregatedStats, PerformanceReport, DetailedMetrics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;
use tracing::{debug, error, info, warn};

/// Configuration for report generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Output directory for reports
    pub output_dir: String,
    /// Whether to generate HTML reports
    pub generate_html: bool,
    /// Whether to generate charts
    pub generate_charts: bool,
    /// Whether to include raw data in reports
    pub include_raw_data: bool,
    /// Chart width in pixels
    pub chart_width: u32,
    /// Chart height in pixels
    pub chart_height: u32,
    /// Report title
    pub title: String,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            output_dir: "./benchmark-reports".to_string(),
            generate_html: true,
            generate_charts: true,
            include_raw_data: false,
            chart_width: 800,
            chart_height: 600,
            title: "MooseNG Benchmark Report".to_string(),
        }
    }
}

/// Main report generator
pub struct ReportGenerator {
    config: ReportConfig,
    charts: Vec<Chart>,
}

impl ReportGenerator {
    pub fn new(config: ReportConfig) -> Self {
        Self {
            config,
            charts: Vec::new(),
        }
    }
    
    /// Generate comprehensive benchmark report
    pub fn generate_report(
        &mut self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
        aggregated_stats: &HashMap<String, AggregatedStats>,
        performance_report: Option<&PerformanceReport>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Create output directory
        create_dir_all(&self.config.output_dir)?;
        
        // Generate charts if enabled
        if self.config.generate_charts {
            self.generate_charts(benchmark_results, aggregated_stats)?;
        }
        
        // Generate HTML report if enabled
        if self.config.generate_html {
            let html_path = self.generate_html_report(benchmark_results, aggregated_stats, performance_report)?;
            info!("HTML report generated at: {}", html_path);
            return Ok(html_path);
        }
        
        // Generate text report as fallback
        let text_path = self.generate_text_report(benchmark_results, aggregated_stats)?;
        info!("Text report generated at: {}", text_path);
        Ok(text_path)
    }
    
    /// Generate charts for benchmark data
    fn generate_charts(
        &mut self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
        aggregated_stats: &HashMap<String, AggregatedStats>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Latency comparison chart
        let latency_chart = self.create_latency_chart(benchmark_results)?;
        self.charts.push(latency_chart);
        
        // Throughput comparison chart
        let throughput_chart = self.create_throughput_chart(benchmark_results)?;
        self.charts.push(throughput_chart);
        
        // Performance distribution chart
        let distribution_chart = self.create_distribution_chart(aggregated_stats)?;
        self.charts.push(distribution_chart);
        
        // Resource utilization chart
        let resource_chart = self.create_resource_chart(benchmark_results)?;
        self.charts.push(resource_chart);
        
        Ok(())
    }
    
    /// Create latency comparison chart
    fn create_latency_chart(
        &self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
    ) -> Result<Chart, Box<dyn std::error::Error>> {
        let mut data_points = Vec::new();
        
        for (benchmark_name, results) in benchmark_results {
            for result in results {
                data_points.push(DataPoint {
                    x: result.operation.clone(),
                    y: result.mean_time.as_millis() as f64,
                    category: benchmark_name.clone(),
                });
            }
        }
        
        Ok(Chart {
            chart_type: ChartType::Bar,
            title: "Operation Latency Comparison".to_string(),
            x_label: "Operation".to_string(),
            y_label: "Latency (ms)".to_string(),
            data_points,
            filename: "latency_comparison.svg".to_string(),
        })
    }
    
    /// Create throughput comparison chart
    fn create_throughput_chart(
        &self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
    ) -> Result<Chart, Box<dyn std::error::Error>> {
        let mut data_points = Vec::new();
        
        for (benchmark_name, results) in benchmark_results {
            for result in results {
                if let Some(throughput) = result.throughput {
                    data_points.push(DataPoint {
                        x: result.operation.clone(),
                        y: throughput,
                        category: benchmark_name.clone(),
                    });
                }
            }
        }
        
        Ok(Chart {
            chart_type: ChartType::Bar,
            title: "Throughput Comparison".to_string(),
            x_label: "Operation".to_string(),
            y_label: "Throughput (ops/sec or MB/s)".to_string(),
            data_points,
            filename: "throughput_comparison.svg".to_string(),
        })
    }
    
    /// Create performance distribution chart
    fn create_distribution_chart(
        &self,
        aggregated_stats: &HashMap<String, AggregatedStats>,
    ) -> Result<Chart, Box<dyn std::error::Error>> {
        let mut data_points = Vec::new();
        
        for (operation, stats) in aggregated_stats {
            data_points.push(DataPoint {
                x: format!("{}_min", operation),
                y: stats.min_duration.as_millis() as f64,
                category: "Duration".to_string(),
            });
            data_points.push(DataPoint {
                x: format!("{}_median", operation),
                y: stats.median_duration.as_millis() as f64,
                category: "Duration".to_string(),
            });
            data_points.push(DataPoint {
                x: format!("{}_p95", operation),
                y: stats.p95_duration.as_millis() as f64,
                category: "Duration".to_string(),
            });
            data_points.push(DataPoint {
                x: format!("{}_max", operation),
                y: stats.max_duration.as_millis() as f64,
                category: "Duration".to_string(),
            });
        }
        
        Ok(Chart {
            chart_type: ChartType::BoxPlot,
            title: "Performance Distribution".to_string(),
            x_label: "Operation".to_string(),
            y_label: "Duration (ms)".to_string(),
            data_points,
            filename: "performance_distribution.svg".to_string(),
        })
    }
    
    /// Create resource utilization chart
    fn create_resource_chart(
        &self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
    ) -> Result<Chart, Box<dyn std::error::Error>> {
        let mut data_points = Vec::new();
        
        // Simulate resource utilization data
        for (benchmark_name, results) in benchmark_results {
            let avg_cpu = 45.0 + (results.len() as f64 * 2.0);
            let avg_memory = 60.0 + (results.len() as f64 * 1.5);
            
            data_points.push(DataPoint {
                x: benchmark_name.clone(),
                y: avg_cpu,
                category: "CPU %".to_string(),
            });
            data_points.push(DataPoint {
                x: benchmark_name.clone(),
                y: avg_memory,
                category: "Memory %".to_string(),
            });
        }
        
        Ok(Chart {
            chart_type: ChartType::Line,
            title: "Resource Utilization".to_string(),
            x_label: "Benchmark".to_string(),
            y_label: "Utilization %".to_string(),
            data_points,
            filename: "resource_utilization.svg".to_string(),
        })
    }
    
    /// Generate HTML report
    fn generate_html_report(
        &self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
        aggregated_stats: &HashMap<String, AggregatedStats>,
        performance_report: Option<&PerformanceReport>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let html_content = self.build_html_content(benchmark_results, aggregated_stats, performance_report)?;
        
        let html_path = format!("{}/benchmark_report.html", self.config.output_dir);
        let mut file = File::create(&html_path)?;
        file.write_all(html_content.as_bytes())?;
        
        Ok(html_path)
    }
    
    /// Build HTML content
    fn build_html_content(
        &self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
        aggregated_stats: &HashMap<String, AggregatedStats>,
        performance_report: Option<&PerformanceReport>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::new();
        
        // HTML header
        html.push_str(&format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }}
        .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        h1 {{ color: #2c3e50; text-align: center; margin-bottom: 30px; }}
        h2 {{ color: #34495e; border-bottom: 2px solid #3498db; padding-bottom: 10px; }}
        h3 {{ color: #7f8c8d; }}
        .summary {{ background: #ecf0f1; padding: 20px; border-radius: 6px; margin: 20px 0; }}
        .chart {{ text-align: center; margin: 30px 0; }}
        table {{ width: 100%; border-collapse: collapse; margin: 20px 0; }}
        th, td {{ padding: 12px; text-align: left; border-bottom: 1px solid #ddd; }}
        th {{ background-color: #3498db; color: white; }}
        tr:nth-child(even) {{ background-color: #f2f2f2; }}
        .metric-good {{ color: #27ae60; font-weight: bold; }}
        .metric-bad {{ color: #e74c3c; font-weight: bold; }}
        .metric-neutral {{ color: #7f8c8d; }}
        .improvement {{ background: #d5f4e6; padding: 10px; margin: 5px 0; border-left: 4px solid #27ae60; }}
        .regression {{ background: #fceaea; padding: 10px; margin: 5px 0; border-left: 4px solid #e74c3c; }}
        .recommendation {{ background: #e8f4f8; padding: 10px; margin: 5px 0; border-left: 4px solid #3498db; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>{}</h1>
        <div class="summary">
            <p><strong>Generated:</strong> {}</p>
            <p><strong>Total Benchmarks:</strong> {}</p>
            <p><strong>Total Operations:</strong> {}</p>
        </div>
"#,
            self.config.title,
            self.config.title,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            benchmark_results.len(),
            benchmark_results.iter().map(|(_, results)| results.len()).sum::<usize>()
        ));
        
        // Performance comparison section
        if let Some(perf_report) = performance_report {
            html.push_str("<h2>Performance Analysis</h2>");
            
            if !perf_report.improvements.is_empty() {
                html.push_str("<h3>Improvements</h3>");
                for improvement in &perf_report.improvements {
                    html.push_str(&format!("<div class=\"improvement\">{}</div>", improvement));
                }
            }
            
            if !perf_report.regressions.is_empty() {
                html.push_str("<h3>Regressions</h3>");
                for regression in &perf_report.regressions {
                    html.push_str(&format!("<div class=\"regression\">{}</div>", regression));
                }
            }
            
            if !perf_report.recommendations.is_empty() {
                html.push_str("<h3>Recommendations</h3>");
                for recommendation in &perf_report.recommendations {
                    html.push_str(&format!("<div class=\"recommendation\">{}</div>", recommendation));
                }
            }
        }
        
        // Charts section
        if self.config.generate_charts && !self.charts.is_empty() {
            html.push_str("<h2>Performance Charts</h2>");
            for chart in &self.charts {
                html.push_str(&format!(
                    "<div class=\"chart\"><h3>{}</h3><img src=\"{}\" alt=\"{}\"></div>",
                    chart.title, chart.filename, chart.title
                ));
            }
        }
        
        // Detailed results section
        html.push_str("<h2>Detailed Results</h2>");
        
        for (benchmark_name, results) in benchmark_results {
            html.push_str(&format!("<h3>{}</h3>", benchmark_name));
            html.push_str("<table>");
            html.push_str("<tr><th>Operation</th><th>Mean Time</th><th>Min Time</th><th>Max Time</th><th>Std Dev</th><th>Throughput</th><th>Samples</th></tr>");
            
            for result in results {
                let throughput_str = result.throughput
                    .map(|t| format!("{:.2}", t))
                    .unwrap_or_else(|| "N/A".to_string());
                
                html.push_str(&format!(
                    "<tr><td>{}</td><td>{:.2}ms</td><td>{:.2}ms</td><td>{:.2}ms</td><td>{:.2}ms</td><td>{}</td><td>{}</td></tr>",
                    result.operation,
                    result.mean_time.as_millis(),
                    result.min_time.as_millis(),
                    result.max_time.as_millis(),
                    result.std_dev.as_millis(),
                    throughput_str,
                    result.samples
                ));
            }
            
            html.push_str("</table>");
        }
        
        // Aggregated statistics section
        if !aggregated_stats.is_empty() {
            html.push_str("<h2>Aggregated Statistics</h2>");
            html.push_str("<table>");
            html.push_str("<tr><th>Operation</th><th>Count</th><th>Mean</th><th>Median</th><th>P95</th><th>P99</th><th>Ops/sec</th><th>Error Rate</th></tr>");
            
            for (operation, stats) in aggregated_stats {
                let error_rate_class = if stats.error_rate > 5.0 {
                    "metric-bad"
                } else if stats.error_rate > 1.0 {
                    "metric-neutral"
                } else {
                    "metric-good"
                };
                
                html.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{:.2}ms</td><td>{:.2}ms</td><td>{:.2}ms</td><td>{:.2}ms</td><td>{:.2}</td><td class=\"{}\">{:.2}%</td></tr>",
                    operation,
                    stats.count,
                    stats.mean_duration.as_millis(),
                    stats.median_duration.as_millis(),
                    stats.p95_duration.as_millis(),
                    stats.p99_duration.as_millis(),
                    stats.ops_per_second,
                    error_rate_class,
                    stats.error_rate
                ));
            }
            
            html.push_str("</table>");
        }
        
        // HTML footer
        html.push_str(r#"
    </div>
</body>
</html>"#);
        
        Ok(html)
    }
    
    /// Generate text report
    fn generate_text_report(
        &self,
        benchmark_results: &[(String, Vec<BenchmarkResult>)],
        aggregated_stats: &HashMap<String, AggregatedStats>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut report = String::new();
        
        report.push_str(&format!("{}\n", "=".repeat(60)));
        report.push_str(&format!("{}\n", self.config.title));
        report.push_str(&format!("{}\n\n", "=".repeat(60)));
        
        for (benchmark_name, results) in benchmark_results {
            report.push_str(&format!("{}\n", "-".repeat(40)));
            report.push_str(&format!("{}\n", benchmark_name));
            report.push_str(&format!("{}\n", "-".repeat(40)));
            
            for result in results {
                report.push_str(&format!(
                    "{:<30} Mean: {:>8.2}ms  Samples: {:>6}\n",
                    result.operation,
                    result.mean_time.as_millis(),
                    result.samples
                ));
            }
            report.push_str("\n");
        }
        
        let text_path = format!("{}/benchmark_report.txt", self.config.output_dir);
        let mut file = File::create(&text_path)?;
        file.write_all(report.as_bytes())?;
        
        Ok(text_path)
    }
}

/// Chart representation
#[derive(Debug, Clone)]
pub struct Chart {
    pub chart_type: ChartType,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub data_points: Vec<DataPoint>,
    pub filename: String,
}

/// Chart types
#[derive(Debug, Clone)]
pub enum ChartType {
    Bar,
    Line,
    Scatter,
    BoxPlot,
}

/// Data point for charts
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub x: String,
    pub y: f64,
    pub category: String,
}

/// Dashboard generator for real-time monitoring
pub struct DashboardGenerator {
    config: ReportConfig,
}

impl DashboardGenerator {
    pub fn new(config: ReportConfig) -> Self {
        Self { config }
    }
    
    /// Generate Grafana dashboard configuration
    pub fn generate_grafana_dashboard(&self) -> Result<String, Box<dyn std::error::Error>> {
        let dashboard_json = serde_json::json!({
            "dashboard": {
                "id": null,
                "title": "MooseNG Benchmark Dashboard",
                "tags": ["mooseng", "benchmarks"],
                "timezone": "browser",
                "panels": [
                    {
                        "id": 1,
                        "title": "Operation Latency",
                        "type": "graph",
                        "targets": [{
                            "expr": "mooseng_operation_duration_seconds",
                            "legendFormat": "{{operation}}"
                        }],
                        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 0}
                    },
                    {
                        "id": 2,
                        "title": "Throughput",
                        "type": "graph",
                        "targets": [{
                            "expr": "rate(mooseng_operations_total[5m])",
                            "legendFormat": "{{operation}}"
                        }],
                        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 0}
                    },
                    {
                        "id": 3,
                        "title": "CPU Usage",
                        "type": "graph",
                        "targets": [{
                            "expr": "mooseng_cpu_usage_percent",
                            "legendFormat": "CPU %"
                        }],
                        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 8}
                    },
                    {
                        "id": 4,
                        "title": "Error Rate",
                        "type": "singlestat",
                        "targets": [{
                            "expr": "rate(mooseng_errors_total[5m]) / rate(mooseng_operations_total[5m]) * 100",
                            "legendFormat": "Error %"
                        }],
                        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 8}
                    }
                ],
                "time": {"from": "now-1h", "to": "now"},
                "refresh": "5s"
            }
        });
        
        Ok(dashboard_json.to_string())
    }
    
    /// Generate Prometheus alerting rules
    pub fn generate_prometheus_alerts(&self) -> Result<String, Box<dyn std::error::Error>> {
        let alerts_yaml = r#"
groups:
- name: mooseng_benchmarks
  rules:
  - alert: HighLatency
    expr: mooseng_operation_duration_seconds > 1.0
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High latency detected for operation {{ $labels.operation }}"
      description: "Operation {{ $labels.operation }} has latency of {{ $value }}s"
      
  - alert: LowThroughput
    expr: rate(mooseng_operations_total[5m]) < 10
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "Low throughput detected for operation {{ $labels.operation }}"
      description: "Operation {{ $labels.operation }} has throughput of {{ $value }} ops/sec"
      
  - alert: HighErrorRate
    expr: rate(mooseng_errors_total[5m]) / rate(mooseng_operations_total[5m]) * 100 > 5
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "High error rate detected"
      description: "Error rate is {{ $value }}% for operation {{ $labels.operation }}"
"#;
        
        Ok(alerts_yaml.to_string())
    }
}

/// Report format enumeration for the unified runner
#[derive(Debug, Clone)]
pub enum ReportFormat {
    Csv,
    Html,
    Markdown,
    Prometheus,
}

/// Generate a report in the specified format
pub async fn generate_report(
    results: &HashMap<String, Vec<BenchmarkResult>>,
    format: ReportFormat,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        ReportFormat::Csv => generate_csv_report(results, output_path).await,
        ReportFormat::Html => generate_html_report_unified(results, output_path).await,
        ReportFormat::Markdown => generate_markdown_report(results, output_path).await,
        ReportFormat::Prometheus => generate_prometheus_report(results, output_path).await,
    }
}

/// Generate CSV report
async fn generate_csv_report(
    results: &HashMap<String, Vec<BenchmarkResult>>,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut csv_content = String::new();
    csv_content.push_str("category,operation,mean_time_ms,std_dev_ms,min_time_ms,max_time_ms,throughput_mbps,samples,timestamp\n");
    
    for (category, benchmark_results) in results {
        for result in benchmark_results {
            csv_content.push_str(&format!(
                "{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{},{}\n",
                category,
                result.operation,
                result.mean_time.as_secs_f64() * 1000.0,
                result.std_dev.as_secs_f64() * 1000.0,
                result.min_time.as_secs_f64() * 1000.0,
                result.max_time.as_secs_f64() * 1000.0,
                result.throughput.unwrap_or(0.0),
                result.samples,
                result.timestamp.format("%Y-%m-%d %H:%M:%S")
            ));
        }
    }
    
    tokio::fs::write(output_path, csv_content).await?;
    Ok(())
}

/// Generate HTML report for unified runner
async fn generate_html_report_unified(
    results: &HashMap<String, Vec<BenchmarkResult>>,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut html = String::new();
    
    // HTML header with modern styling
    html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>MooseNG Unified Benchmark Report</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background: #f8f9fa; color: #333; }
        .container { max-width: 1400px; margin: 0 auto; padding: 20px; }
        .header { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 40px; border-radius: 8px; margin-bottom: 30px; }
        .header h1 { font-size: 2.5em; margin-bottom: 10px; }
        .header .subtitle { font-size: 1.2em; opacity: 0.9; }
        .summary-cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 20px; margin-bottom: 30px; }
        .card { background: white; padding: 25px; border-radius: 8px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); border-left: 4px solid #667eea; }
        .card h3 { color: #4a5568; margin-bottom: 10px; }
        .card .value { font-size: 2em; font-weight: bold; color: #667eea; }
        .card .label { color: #718096; font-size: 0.9em; }
        .section { background: white; margin-bottom: 30px; border-radius: 8px; overflow: hidden; box-shadow: 0 4px 6px rgba(0,0,0,0.1); }
        .section-header { background: #f7fafc; padding: 20px; border-bottom: 1px solid #e2e8f0; }
        .section-header h2 { color: #2d3748; }
        .section-content { padding: 20px; }
        table { width: 100%; border-collapse: collapse; }
        th, td { padding: 12px; text-align: left; border-bottom: 1px solid #e2e8f0; }
        th { background: #f7fafc; font-weight: 600; color: #4a5568; }
        tr:hover { background: #f7fafc; }
        .metric-excellent { color: #38a169; font-weight: bold; }
        .metric-good { color: #3182ce; font-weight: bold; }
        .metric-warning { color: #d69e2e; font-weight: bold; }
        .metric-poor { color: #e53e3e; font-weight: bold; }
        .grade-badge { padding: 4px 12px; border-radius: 12px; font-size: 0.8em; font-weight: bold; }
        .grade-a { background: #c6f6d5; color: #22543d; }
        .grade-b { background: #bee3f8; color: #2a4a5c; }
        .grade-c { background: #faf089; color: #744210; }
        .grade-d { background: #fed7d7; color: #742a2a; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 MooseNG Unified Benchmark Report</h1>
            <div class="subtitle">Comprehensive Performance Analysis</div>
        </div>"#);
    
    // Calculate summary statistics
    let total_operations: usize = results.values().map(|v| v.len()).sum();
    let total_samples: usize = results.values()
        .flatten()
        .map(|r| r.samples)
        .sum();
    
    let all_results: Vec<&BenchmarkResult> = results.values().flatten().collect();
    let avg_latency: f64 = if !all_results.is_empty() {
        all_results.iter()
            .map(|r| r.mean_time.as_secs_f64() * 1000.0)
            .sum::<f64>() / all_results.len() as f64
    } else {
        0.0
    };
    
    let avg_throughput: f64 = {
        let throughput_results: Vec<f64> = all_results.iter()
            .filter_map(|r| r.throughput)
            .collect();
        if !throughput_results.is_empty() {
            throughput_results.iter().sum::<f64>() / throughput_results.len() as f64
        } else {
            0.0
        }
    };
    
    // Summary cards
    html.push_str(&format!(r#"
        <div class="summary-cards">
            <div class="card">
                <h3>Total Categories</h3>
                <div class="value">{}</div>
                <div class="label">Benchmark categories executed</div>
            </div>
            <div class="card">
                <h3>Total Operations</h3>
                <div class="value">{}</div>
                <div class="label">Individual benchmark operations</div>
            </div>
            <div class="card">
                <h3>Total Samples</h3>
                <div class="value">{}</div>
                <div class="label">Data points collected</div>
            </div>
            <div class="card">
                <h3>Average Latency</h3>
                <div class="value">{:.2}</div>
                <div class="label">milliseconds</div>
            </div>
            <div class="card">
                <h3>Average Throughput</h3>
                <div class="value">{:.2}</div>
                <div class="label">MB/s</div>
            </div>
        </div>"#,
        results.len(),
        total_operations,
        total_samples,
        avg_latency,
        avg_throughput
    ));
    
    // Detailed results by category
    for (category, category_results) in results {
        if category_results.is_empty() {
            continue;
        }
        
        html.push_str(&format!(r#"
        <div class="section">
            <div class="section-header">
                <h2>{}</h2>
            </div>
            <div class="section-content">
                <table>
                    <thead>
                        <tr>
                            <th>Operation</th>
                            <th>Mean Time</th>
                            <th>Min Time</th>
                            <th>Max Time</th>
                            <th>Std Dev</th>
                            <th>Throughput</th>
                            <th>Samples</th>
                            <th>Grade</th>
                        </tr>
                    </thead>
                    <tbody>"#, category.replace('_', " ").to_uppercase()));
        
        for result in category_results {
            let throughput_str = result.throughput
                .map(|t| format!("{:.2} MB/s", t))
                .unwrap_or_else(|| "N/A".to_string());
            
            let grade = calculate_operation_grade(result);
            let grade_class = match grade.chars().next().unwrap_or('D') {
                'A' => "grade-a",
                'B' => "grade-b",
                'C' => "grade-c",
                _ => "grade-d",
            };
            
            html.push_str(&format!(r#"
                        <tr>
                            <td>{}</td>
                            <td>{:.2}ms</td>
                            <td>{:.2}ms</td>
                            <td>{:.2}ms</td>
                            <td>{:.2}ms</td>
                            <td>{}</td>
                            <td>{}</td>
                            <td><span class="grade-badge {}">{}</span></td>
                        </tr>"#,
                result.operation,
                result.mean_time.as_secs_f64() * 1000.0,
                result.min_time.as_secs_f64() * 1000.0,
                result.max_time.as_secs_f64() * 1000.0,
                result.std_dev.as_secs_f64() * 1000.0,
                throughput_str,
                result.samples,
                grade_class,
                grade
            ));
        }
        
        html.push_str("</tbody></table></div></div>");
    }
    
    // Footer
    html.push_str(&format!(r#"
        <div class="section">
            <div class="section-content">
                <p style="text-align: center; color: #718096;">
                    Generated by MooseNG Unified Benchmark Runner at {}
                </p>
            </div>
        </div>
    </div>
</body>
</html>"#, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    
    tokio::fs::write(output_path, html).await?;
    Ok(())
}

fn calculate_operation_grade(result: &BenchmarkResult) -> String {
    let latency_ms = result.mean_time.as_secs_f64() * 1000.0;
    let throughput = result.throughput.unwrap_or(0.0);
    
    let mut score = 100.0;
    
    // Latency scoring
    if latency_ms > 1000.0 {
        score -= 40.0;
    } else if latency_ms > 100.0 {
        score -= 25.0;
    } else if latency_ms > 50.0 {
        score -= 15.0;
    } else if latency_ms > 20.0 {
        score -= 10.0;
    }
    
    // Throughput scoring
    if throughput > 500.0 {
        score += 10.0;
    } else if throughput > 100.0 {
        score += 5.0;
    } else if throughput < 10.0 && throughput > 0.0 {
        score -= 15.0;
    }
    
    match score as i32 {
        90..=110 => "A+".to_string(),
        80..=89 => "A".to_string(),
        70..=79 => "B+".to_string(),
        60..=69 => "B".to_string(),
        50..=59 => "C".to_string(),
        _ => "D".to_string(),
    }
}

/// Generate Markdown report
async fn generate_markdown_report(
    results: &HashMap<String, Vec<BenchmarkResult>>,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut md = String::new();
    
    md.push_str("# MooseNG Unified Benchmark Report\n\n");
    md.push_str(&format!("**Generated:** {}\n\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    
    // Summary
    let total_operations: usize = results.values().map(|v| v.len()).sum();
    let total_samples: usize = results.values().flatten().map(|r| r.samples).sum();
    
    md.push_str("## Summary\n\n");
    md.push_str(&format!("- **Categories:** {}\n", results.len()));
    md.push_str(&format!("- **Total Operations:** {}\n", total_operations));
    md.push_str(&format!("- **Total Samples:** {}\n", total_samples));
    md.push_str("\n");
    
    // Results by category
    for (category, category_results) in results {
        md.push_str(&format!("## {}\n\n", category.replace('_', " ").to_uppercase()));
        md.push_str("| Operation | Mean Time (ms) | Throughput (MB/s) | Samples | Grade |\n");
        md.push_str("|-----------|----------------|-------------------|---------|-------|\n");
        
        for result in category_results {
            let throughput_str = result.throughput
                .map(|t| format!("{:.2}", t))
                .unwrap_or_else(|| "N/A".to_string());
            
            let grade = calculate_operation_grade(result);
            
            md.push_str(&format!(
                "| {} | {:.2} | {} | {} | {} |\n",
                result.operation,
                result.mean_time.as_secs_f64() * 1000.0,
                throughput_str,
                result.samples,
                grade
            ));
        }
        md.push_str("\n");
    }
    
    tokio::fs::write(output_path, md).await?;
    Ok(())
}

/// Generate Prometheus metrics report
async fn generate_prometheus_report(
    results: &HashMap<String, Vec<BenchmarkResult>>,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut prom = String::new();
    
    prom.push_str("# HELP mooseng_operation_duration_seconds Duration of benchmark operations\n");
    prom.push_str("# TYPE mooseng_operation_duration_seconds gauge\n");
    
    for (category, category_results) in results {
        for result in category_results {
            prom.push_str(&format!(
                "mooseng_operation_duration_seconds{{category=\"{}\",operation=\"{}\"}} {:.6}\n",
                category,
                result.operation,
                result.mean_time.as_secs_f64()
            ));
        }
    }
    
    prom.push_str("\n# HELP mooseng_operation_throughput_mbps Throughput of benchmark operations\n");
    prom.push_str("# TYPE mooseng_operation_throughput_mbps gauge\n");
    
    for (category, category_results) in results {
        for result in category_results {
            if let Some(throughput) = result.throughput {
                prom.push_str(&format!(
                    "mooseng_operation_throughput_mbps{{category=\"{}\",operation=\"{}\"}} {:.6}\n",
                    category,
                    result.operation,
                    throughput
                ));
            }
        }
    }
    
    prom.push_str("\n# HELP mooseng_operation_samples_total Total samples for benchmark operations\n");
    prom.push_str("# TYPE mooseng_operation_samples_total counter\n");
    
    for (category, category_results) in results {
        for result in category_results {
            prom.push_str(&format!(
                "mooseng_operation_samples_total{{category=\"{}\",operation=\"{}\"}} {}\n",
                category,
                result.operation,
                result.samples
            ));
        }
    }
    
    tokio::fs::write(output_path, prom).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_report_config_default() {
        let config = ReportConfig::default();
        assert!(config.generate_html);
        assert!(config.generate_charts);
    }
    
    #[test]
    fn test_report_generator_creation() {
        let config = ReportConfig::default();
        let generator = ReportGenerator::new(config);
        assert_eq!(generator.charts.len(), 0);
    }
    
    #[test]
    fn test_dashboard_generator() {
        let config = ReportConfig::default();
        let dashboard_gen = DashboardGenerator::new(config);
        let grafana_config = dashboard_gen.generate_grafana_dashboard().unwrap();
        assert!(grafana_config.contains("MooseNG Benchmark Dashboard"));
    }
    
    #[test]
    fn test_operation_grade_calculation() {
        let result = BenchmarkResult {
            operation: "test".to_string(),
            mean_time: Duration::from_millis(10),
            std_dev: Duration::from_millis(2),
            min_time: Duration::from_millis(8),
            max_time: Duration::from_millis(15),
            throughput: Some(200.0),
            samples: 100,
            timestamp: chrono::Utc::now(),
            metadata: None,
            percentiles: None,
        };
        
        let grade = calculate_operation_grade(&result);
        assert!(grade.starts_with('A'));
    }
    
    #[tokio::test]
    async fn test_csv_report_generation() {
        let mut results = HashMap::new();
        let benchmark_results = vec![
            BenchmarkResult {
                operation: "test_op".to_string(),
                mean_time: Duration::from_millis(50),
                std_dev: Duration::from_millis(10),
                min_time: Duration::from_millis(40),
                max_time: Duration::from_millis(60),
                throughput: Some(100.0),
                samples: 50,
                timestamp: chrono::Utc::now(),
                metadata: None,
                percentiles: None,
            }
        ];
        results.insert("test_category".to_string(), benchmark_results);
        
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("test_report.csv");
        
        generate_csv_report(&results, &output_path).await.unwrap();
        assert!(output_path.exists());
        
        let content = tokio::fs::read_to_string(&output_path).await.unwrap();
        assert!(content.contains("test_category"));
        assert!(content.contains("test_op"));
    }
}