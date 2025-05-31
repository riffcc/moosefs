//! HTML Report Generation for MooseNG Benchmarks
//!
//! This module provides comprehensive HTML report generation with interactive visualizations,
//! performance dashboards, and real-time data presentation.

use crate::{BenchmarkResult, BenchmarkConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use chrono::{DateTime, Utc};

/// HTML report configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlReportConfig {
    /// Output directory for HTML reports
    pub output_dir: String,
    /// Report title
    pub title: String,
    /// Include interactive charts
    pub interactive_charts: bool,
    /// Include real-time updates
    pub real_time_updates: bool,
    /// Chart library to use (plotly, chartjs, d3)
    pub chart_library: ChartLibrary,
    /// Theme (light, dark, auto)
    pub theme: Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChartLibrary {
    Plotly,
    ChartJs,
    D3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    Auto,
}

impl Default for HtmlReportConfig {
    fn default() -> Self {
        Self {
            output_dir: "benchmark_reports".to_string(),
            title: "MooseNG Performance Benchmarks".to_string(),
            interactive_charts: true,
            real_time_updates: false,
            chart_library: ChartLibrary::Plotly,
            theme: Theme::Auto,
        }
    }
}

/// Comprehensive benchmark report data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub timestamp: DateTime<Utc>,
    pub environment: EnvironmentInfo,
    pub results: Vec<BenchmarkResult>,
    pub summary: BenchmarkSummary,
    pub component_results: HashMap<String, ComponentBenchmarks>,
    pub multiregion_results: Option<MultiregionBenchmarks>,
    pub comparative_results: Option<ComparativeBenchmarks>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    pub os: String,
    pub arch: String,
    pub cores: u32,
    pub memory_gb: u32,
    pub rust_version: String,
    pub mooseng_version: String,
    pub build_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub total_duration: std::time::Duration,
    pub overall_grade: String,
    pub performance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentBenchmarks {
    pub component_name: String,
    pub operations: Vec<BenchmarkResult>,
    pub health_metrics: HealthMetrics,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub disk_io_mbps: f64,
    pub network_io_mbps: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub peak_memory_mb: f64,
    pub avg_cpu_percent: f64,
    pub disk_writes_mb: f64,
    pub disk_reads_mb: f64,
    pub network_bytes: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiregionBenchmarks {
    pub regions: Vec<String>,
    pub cross_region_latency: HashMap<String, HashMap<String, f64>>,
    pub replication_lag: HashMap<String, f64>,
    pub failover_times: Vec<f64>,
    pub consistency_metrics: ConsistencyMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyMetrics {
    pub eventual_consistency_time: f64,
    pub strong_consistency_overhead: f64,
    pub conflict_resolution_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparativeBenchmarks {
    pub baseline_system: String,
    pub performance_ratio: f64,
    pub feature_comparison: HashMap<String, ComparisonResult>,
    pub resource_efficiency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub mooseng_value: f64,
    pub baseline_value: f64,
    pub improvement_percent: f64,
}

/// HTML report generator
pub struct HtmlReportGenerator {
    config: HtmlReportConfig,
}

impl HtmlReportGenerator {
    pub fn new(config: HtmlReportConfig) -> Self {
        Self { config }
    }

    /// Generate complete HTML report
    pub fn generate_report(&self, report: &BenchmarkReport) -> Result<String, Box<dyn std::error::Error>> {
        // Create output directory
        fs::create_dir_all(&self.config.output_dir)?;

        // Generate main dashboard
        let dashboard_html = self.generate_dashboard(report)?;
        let dashboard_path = Path::new(&self.config.output_dir).join("index.html");
        fs::write(dashboard_path, dashboard_html)?;

        // Generate component-specific reports
        for (component, benchmarks) in &report.component_results {
            let component_html = self.generate_component_report(component, benchmarks)?;
            let component_path = Path::new(&self.config.output_dir)
                .join(format!("{}_report.html", component.to_lowercase()));
            fs::write(component_path, component_html)?;
        }

        // Generate multiregion report if available
        if let Some(multiregion) = &report.multiregion_results {
            let multiregion_html = self.generate_multiregion_report(multiregion)?;
            let multiregion_path = Path::new(&self.config.output_dir).join("multiregion_report.html");
            fs::write(multiregion_path, multiregion_html)?;
        }

        // Generate comparative report if available
        if let Some(comparative) = &report.comparative_results {
            let comparative_html = self.generate_comparative_report(comparative)?;
            let comparative_path = Path::new(&self.config.output_dir).join("comparative_report.html");
            fs::write(comparative_path, comparative_html)?;
        }

        // Copy static assets
        self.copy_static_assets()?;

        Ok(format!("Reports generated in {}", self.config.output_dir))
    }

    fn generate_dashboard(&self, report: &BenchmarkReport) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::new();
        
        // HTML head with dependencies
        html.push_str(&self.generate_html_head("Dashboard")?);
        
        html.push_str(r#"
<body>
    <div class="container-fluid">
        <nav class="navbar navbar-expand-lg navbar-dark bg-primary">
            <div class="container-fluid">
                <a class="navbar-brand" href="#">
                    <i class="fas fa-chart-line me-2"></i>MooseNG Benchmarks
                </a>
                <div class="navbar-nav ms-auto">
                    <div class="nav-item dropdown">
                        <a class="nav-link dropdown-toggle" href="#" role="button" data-bs-toggle="dropdown">
                            <i class="fas fa-cog"></i> Reports
                        </a>
                        <ul class="dropdown-menu">
                            <li><a class="dropdown-item" href="index.html">Dashboard</a></li>"#);

        // Add component links
        for component in report.component_results.keys() {
            html.push_str(&format!(
                r#"                            <li><a class="dropdown-item" href="{}_report.html">{} Report</a></li>
"#,
                component.to_lowercase(),
                component
            ));
        }

        if report.multiregion_results.is_some() {
            html.push_str(r#"                            <li><a class="dropdown-item" href="multiregion_report.html">Multiregion Report</a></li>
"#);
        }

        if report.comparative_results.is_some() {
            html.push_str(r#"                            <li><a class="dropdown-item" href="comparative_report.html">Comparative Report</a></li>
"#);
        }

        html.push_str(r#"
                        </ul>
                    </div>
                </div>
            </div>
        </nav>

        <div class="row mt-4">
            <div class="col-12">
                <h1 class="display-4 text-center mb-4">
                    <i class="fas fa-tachometer-alt me-3"></i>Performance Dashboard
                </h1>
            </div>
        </div>
"#);

        // Summary cards
        html.push_str(&self.generate_summary_cards(&report.summary)?);

        // Environment info
        html.push_str(&self.generate_environment_section(&report.environment)?);

        // Main charts
        html.push_str(&self.generate_main_charts(&report.results)?);

        // Component overview
        html.push_str(&self.generate_component_overview(&report.component_results)?);

        // Real-time updates section
        if self.config.real_time_updates {
            html.push_str(&self.generate_realtime_section()?);
        }

        html.push_str(r#"
    </div>

    <script src="assets/js/dashboard.js"></script>
</body>
</html>
"#);

        Ok(html)
    }

    fn generate_html_head(&self, title: &str) -> Result<String, Box<dyn std::error::Error>> {
        let theme_class = match self.config.theme {
            Theme::Dark => "theme-dark",
            Theme::Light => "theme-light",
            Theme::Auto => "theme-auto",
        };

        Ok(format!(r#"<!DOCTYPE html>
<html lang="en" class="{}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{} - {}</title>
    
    <!-- Bootstrap CSS -->
    <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css" rel="stylesheet">
    <!-- Font Awesome -->
    <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.0.0/css/all.min.css" rel="stylesheet">
    <!-- Chart.js -->
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <!-- Plotly.js -->
    <script src="https://cdn.plot.ly/plotly-latest.min.js"></script>
    <!-- Custom CSS -->
    <link href="assets/css/style.css" rel="stylesheet">
    
    <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js"></script>
</head>
"#, theme_class, title, self.config.title))
    }

    fn generate_summary_cards(&self, summary: &BenchmarkSummary) -> Result<String, Box<dyn std::error::Error>> {
        let success_rate = if summary.total_tests > 0 {
            (summary.passed_tests as f64 / summary.total_tests as f64) * 100.0
        } else {
            0.0
        };

        Ok(format!(r#"
        <div class="row mb-4">
            <div class="col-md-3">
                <div class="card bg-primary text-white h-100">
                    <div class="card-body text-center">
                        <i class="fas fa-chart-line fa-3x mb-3"></i>
                        <h4>Performance Score</h4>
                        <h2>{:.1}</h2>
                        <p class="mb-0">Overall Grade: {}</p>
                    </div>
                </div>
            </div>
            <div class="col-md-3">
                <div class="card bg-success text-white h-100">
                    <div class="card-body text-center">
                        <i class="fas fa-check-circle fa-3x mb-3"></i>
                        <h4>Success Rate</h4>
                        <h2>{:.1}%</h2>
                        <p class="mb-0">{} / {} tests passed</p>
                    </div>
                </div>
            </div>
            <div class="col-md-3">
                <div class="card bg-info text-white h-100">
                    <div class="card-body text-center">
                        <i class="fas fa-clock fa-3x mb-3"></i>
                        <h4>Total Duration</h4>
                        <h2>{:.1}s</h2>
                        <p class="mb-0">Benchmark runtime</p>
                    </div>
                </div>
            </div>
            <div class="col-md-3">
                <div class="card bg-warning text-white h-100">
                    <div class="card-body text-center">
                        <i class="fas fa-list fa-3x mb-3"></i>
                        <h4>Total Tests</h4>
                        <h2>{}</h2>
                        <p class="mb-0">Benchmark operations</p>
                    </div>
                </div>
            </div>
        </div>
"#, 
            summary.performance_score,
            summary.overall_grade,
            success_rate,
            summary.passed_tests,
            summary.total_tests,
            summary.total_duration.as_secs_f64(),
            summary.total_tests
        ))
    }

    fn generate_environment_section(&self, env: &EnvironmentInfo) -> Result<String, Box<dyn std::error::Error>> {
        Ok(format!(r#"
        <div class="row mb-4">
            <div class="col-12">
                <div class="card">
                    <div class="card-header">
                        <h5><i class="fas fa-server me-2"></i>Environment Information</h5>
                    </div>
                    <div class="card-body">
                        <div class="row">
                            <div class="col-md-4">
                                <strong>Operating System:</strong> {}<br>
                                <strong>Architecture:</strong> {}<br>
                                <strong>CPU Cores:</strong> {}
                            </div>
                            <div class="col-md-4">
                                <strong>Memory:</strong> {} GB<br>
                                <strong>Rust Version:</strong> {}<br>
                                <strong>Build Profile:</strong> {}
                            </div>
                            <div class="col-md-4">
                                <strong>MooseNG Version:</strong> {}<br>
                                <strong>Test Timestamp:</strong> <span id="test-timestamp"></span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
"#, 
            env.os, 
            env.arch, 
            env.cores,
            env.memory_gb,
            env.rust_version,
            env.build_profile,
            env.mooseng_version
        ))
    }

    fn generate_main_charts(&self, results: &[BenchmarkResult]) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::new();

        html.push_str(r#"
        <div class="row mb-4">
            <div class="col-lg-6">
                <div class="card h-100">
                    <div class="card-header">
                        <h5><i class="fas fa-chart-bar me-2"></i>Performance Overview</h5>
                    </div>
                    <div class="card-body">
                        <canvas id="performanceChart"></canvas>
                    </div>
                </div>
            </div>
            <div class="col-lg-6">
                <div class="card h-100">
                    <div class="card-header">
                        <h5><i class="fas fa-chart-line me-2"></i>Throughput Analysis</h5>
                    </div>
                    <div class="card-body">
                        <canvas id="throughputChart"></canvas>
                    </div>
                </div>
            </div>
        </div>
"#);

        // Generate chart data
        let chart_data = self.generate_chart_data(results)?;
        html.push_str(&format!(r#"
    <script>
        // Chart data
        const benchmarkData = {};
        
        // Initialize charts
        document.addEventListener('DOMContentLoaded', function() {{
            initializeCharts(benchmarkData);
        }});
    </script>
"#, chart_data));

        Ok(html)
    }

    fn generate_component_overview(&self, components: &HashMap<String, ComponentBenchmarks>) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::new();

        html.push_str(r#"
        <div class="row mb-4">
            <div class="col-12">
                <div class="card">
                    <div class="card-header">
                        <h5><i class="fas fa-cubes me-2"></i>Component Performance Overview</h5>
                    </div>
                    <div class="card-body">
                        <div class="row">
"#);

        for (name, component) in components {
            let avg_latency = component.operations.iter()
                .map(|op| op.mean_time.as_millis() as f64)
                .sum::<f64>() / component.operations.len() as f64;

            html.push_str(&format!(r#"
                            <div class="col-md-4 mb-3">
                                <div class="card border-left-primary">
                                    <div class="card-body">
                                        <h6 class="card-title">{}</h6>
                                        <p class="card-text">
                                            <small class="text-muted">Avg Latency: {:.2}ms</small><br>
                                            <small class="text-muted">CPU: {:.1}%</small><br>
                                            <small class="text-muted">Memory: {:.1}MB</small>
                                        </p>
                                        <a href="{}_report.html" class="btn btn-sm btn-outline-primary">
                                            View Details <i class="fas fa-arrow-right"></i>
                                        </a>
                                    </div>
                                </div>
                            </div>
"#, name, avg_latency, component.health_metrics.cpu_usage_percent, 
            component.health_metrics.memory_usage_mb, name.to_lowercase()));
        }

        html.push_str(r#"
                        </div>
                    </div>
                </div>
            </div>
        </div>
"#);

        Ok(html)
    }

    fn generate_realtime_section(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(r#"
        <div class="row mb-4">
            <div class="col-12">
                <div class="card">
                    <div class="card-header d-flex justify-content-between align-items-center">
                        <h5><i class="fas fa-broadcast-tower me-2"></i>Real-Time Monitoring</h5>
                        <div>
                            <span class="badge bg-success" id="connection-status">Connected</span>
                            <button class="btn btn-sm btn-outline-secondary" onclick="toggleRealTime()">
                                <i class="fas fa-pause" id="realtime-icon"></i>
                            </button>
                        </div>
                    </div>
                    <div class="card-body">
                        <div class="row">
                            <div class="col-md-6">
                                <canvas id="realtimeLatencyChart"></canvas>
                            </div>
                            <div class="col-md-6">
                                <canvas id="realtimeThroughputChart"></canvas>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
"#.to_string())
    }

    fn generate_chart_data(&self, results: &[BenchmarkResult]) -> Result<String, Box<dyn std::error::Error>> {
        let mut data = serde_json::Map::new();
        
        let operations: Vec<String> = results.iter().map(|r| r.operation.clone()).collect();
        let latencies: Vec<f64> = results.iter().map(|r| r.mean_time.as_millis() as f64).collect();
        let throughputs: Vec<f64> = results.iter()
            .map(|r| r.throughput.unwrap_or(0.0))
            .collect();

        data.insert("operations".to_string(), serde_json::to_value(operations)?);
        data.insert("latencies".to_string(), serde_json::to_value(latencies)?);
        data.insert("throughputs".to_string(), serde_json::to_value(throughputs)?);

        Ok(serde_json::to_string(&data)?)
    }

    fn generate_component_report(&self, component: &str, benchmarks: &ComponentBenchmarks) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::new();
        
        html.push_str(&self.generate_html_head(&format!("{} Report", component))?);
        
        // Component-specific report content will be generated by Instance 3
        html.push_str(&format!(r#"
<body>
    <div class="container-fluid">
        <nav class="navbar navbar-expand-lg navbar-dark bg-primary">
            <div class="container-fluid">
                <a class="navbar-brand" href="index.html">
                    <i class="fas fa-arrow-left me-2"></i>Back to Dashboard
                </a>
                <span class="navbar-text">
                    {} Component Report
                </span>
            </div>
        </nav>

        <div class="container mt-4">
            <h1>{} Performance Analysis</h1>
            <!-- Component-specific content will be added by Instance 3 -->
            <div id="component-content">
                <p>Component-specific visualizations and metrics will be implemented by Instance 3.</p>
            </div>
        </div>
    </div>
</body>
</html>
"#, component, component));

        Ok(html)
    }

    fn generate_multiregion_report(&self, multiregion: &MultiregionBenchmarks) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::new();
        
        html.push_str(&self.generate_html_head("Multiregion Report")?);
        
        html.push_str(r#"
<body>
    <div class="container-fluid">
        <nav class="navbar navbar-expand-lg navbar-dark bg-primary">
            <div class="container-fluid">
                <a class="navbar-brand" href="index.html">
                    <i class="fas fa-arrow-left me-2"></i>Back to Dashboard
                </a>
                <span class="navbar-text">
                    Multiregion Performance Report
                </span>
            </div>
        </nav>

        <div class="container mt-4">
            <h1>Multiregion Performance Analysis</h1>
            <!-- Multiregion-specific content will be added by Instance 4 -->
            <div id="multiregion-content">
                <p>Multiregion analysis and visualizations will be implemented by Instance 4.</p>
            </div>
        </div>
    </div>
</body>
</html>
"#);

        Ok(html)
    }

    fn generate_comparative_report(&self, comparative: &ComparativeBenchmarks) -> Result<String, Box<dyn std::error::Error>> {
        let mut html = String::new();
        
        html.push_str(&self.generate_html_head("Comparative Report")?);
        
        html.push_str(r#"
<body>
    <div class="container-fluid">
        <nav class="navbar navbar-expand-lg navbar-dark bg-primary">
            <div class="container-fluid">
                <a class="navbar-brand" href="index.html">
                    <i class="fas fa-arrow-left me-2"></i>Back to Dashboard
                </a>
                <span class="navbar-text">
                    Comparative Performance Report
                </span>
            </div>
        </nav>

        <div class="container mt-4">
            <h1>Comparative Performance Analysis</h1>
            <!-- Comparative analysis content will be added by Instance 4 -->
            <div id="comparative-content">
                <p>Comparative analysis and benchmarks will be implemented by Instance 4.</p>
            </div>
        </div>
    </div>
</body>
</html>
"#);

        Ok(html)
    }

    fn copy_static_assets(&self) -> Result<(), Box<dyn std::error::Error>> {
        let assets_dir = Path::new(&self.config.output_dir).join("assets");
        fs::create_dir_all(&assets_dir)?;
        
        // Create CSS directory
        let css_dir = assets_dir.join("css");
        fs::create_dir_all(&css_dir)?;
        
        // Create JS directory
        let js_dir = assets_dir.join("js");
        fs::create_dir_all(&js_dir)?;
        
        // Generate CSS file
        let css_content = self.generate_css()?;
        fs::write(css_dir.join("style.css"), css_content)?;
        
        // Generate JS file
        let js_content = self.generate_js()?;
        fs::write(js_dir.join("dashboard.js"), js_content)?;
        
        Ok(())
    }

    fn generate_css(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(r#"
/* MooseNG Benchmark Dashboard Styles */
:root {
    --primary-color: #0d6efd;
    --secondary-color: #6c757d;
    --success-color: #198754;
    --info-color: #0dcaf0;
    --warning-color: #ffc107;
    --danger-color: #dc3545;
    --light-color: #f8f9fa;
    --dark-color: #212529;
}

.theme-dark {
    --bs-body-bg: #1a1a1a;
    --bs-body-color: #ffffff;
    --bs-card-bg: #2d3748;
    --bs-border-color: #4a5568;
}

.theme-light {
    --bs-body-bg: #ffffff;
    --bs-body-color: #212529;
    --bs-card-bg: #ffffff;
    --bs-border-color: #dee2e6;
}

body {
    background-color: var(--bs-body-bg);
    color: var(--bs-body-color);
    font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
}

.card {
    background-color: var(--bs-card-bg);
    border-color: var(--bs-border-color);
    transition: transform 0.2s ease-in-out;
}

.card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 8px rgba(0,0,0,0.1);
}

.border-left-primary {
    border-left: 4px solid var(--primary-color) !important;
}

.navbar-brand {
    font-weight: bold;
    font-size: 1.5rem;
}

.metric-card {
    text-align: center;
    padding: 1.5rem;
}

.metric-card i {
    font-size: 2rem;
    margin-bottom: 1rem;
    opacity: 0.8;
}

.chart-container {
    position: relative;
    height: 400px;
    width: 100%;
}

.status-indicator {
    display: inline-block;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    margin-right: 8px;
}

.status-indicator.online {
    background-color: var(--success-color);
}

.status-indicator.offline {
    background-color: var(--danger-color);
}

.status-indicator.warning {
    background-color: var(--warning-color);
}

.performance-badge {
    font-size: 0.9rem;
    padding: 0.5rem 1rem;
    border-radius: 20px;
}

.grade-a {
    background-color: var(--success-color);
    color: white;
}

.grade-b {
    background-color: var(--info-color);
    color: white;
}

.grade-c {
    background-color: var(--warning-color);
    color: black;
}

.grade-d {
    background-color: var(--danger-color);
    color: white;
}

@media (max-width: 768px) {
    .metric-card {
        padding: 1rem;
        margin-bottom: 1rem;
    }
    
    .chart-container {
        height: 300px;
    }
}

/* Animation for loading states */
.loading-spinner {
    border: 3px solid #f3f3f3;
    border-top: 3px solid var(--primary-color);
    border-radius: 50%;
    width: 30px;
    height: 30px;
    animation: spin 1s linear infinite;
}

@keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}

/* Real-time update indicators */
.pulse {
    animation: pulse 2s infinite;
}

@keyframes pulse {
    0% {
        box-shadow: 0 0 0 0 rgba(13, 110, 253, 0.7);
    }
    70% {
        box-shadow: 0 0 0 10px rgba(13, 110, 253, 0);
    }
    100% {
        box-shadow: 0 0 0 0 rgba(13, 110, 253, 0);
    }
}
"#.to_string())
    }

    fn generate_js(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(r#"
// MooseNG Benchmark Dashboard JavaScript

let realtimeEnabled = false;
let realtimeInterval = null;
let charts = {};

// Initialize all charts
function initializeCharts(data) {
    // Performance overview chart
    const perfCtx = document.getElementById('performanceChart');
    if (perfCtx) {
        charts.performance = new Chart(perfCtx, {
            type: 'bar',
            data: {
                labels: data.operations || [],
                datasets: [{
                    label: 'Latency (ms)',
                    data: data.latencies || [],
                    backgroundColor: 'rgba(13, 110, 253, 0.7)',
                    borderColor: 'rgba(13, 110, 253, 1)',
                    borderWidth: 1
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Latency (ms)'
                        }
                    }
                },
                plugins: {
                    legend: {
                        display: true,
                        position: 'top'
                    },
                    tooltip: {
                        callbacks: {
                            label: function(context) {
                                return `${context.dataset.label}: ${context.parsed.y.toFixed(2)}ms`;
                            }
                        }
                    }
                }
            }
        });
    }

    // Throughput analysis chart
    const throughputCtx = document.getElementById('throughputChart');
    if (throughputCtx) {
        charts.throughput = new Chart(throughputCtx, {
            type: 'line',
            data: {
                labels: data.operations || [],
                datasets: [{
                    label: 'Throughput (ops/sec)',
                    data: data.throughputs || [],
                    backgroundColor: 'rgba(25, 135, 84, 0.1)',
                    borderColor: 'rgba(25, 135, 84, 1)',
                    borderWidth: 2,
                    fill: true,
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Operations per Second'
                        }
                    }
                },
                plugins: {
                    legend: {
                        display: true,
                        position: 'top'
                    }
                }
            }
        });
    }

    // Initialize real-time charts if enabled
    if (document.getElementById('realtimeLatencyChart')) {
        initializeRealtimeCharts();
    }

    // Update timestamp
    updateTimestamp();
}

// Initialize real-time monitoring charts
function initializeRealtimeCharts() {
    const realtimeLatencyCtx = document.getElementById('realtimeLatencyChart');
    const realtimeThroughputCtx = document.getElementById('realtimeThroughputChart');

    if (realtimeLatencyCtx) {
        charts.realtimeLatency = new Chart(realtimeLatencyCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Latency (ms)',
                    data: [],
                    backgroundColor: 'rgba(255, 193, 7, 0.1)',
                    borderColor: 'rgba(255, 193, 7, 1)',
                    borderWidth: 2,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    x: {
                        type: 'time',
                        time: {
                            displayFormats: {
                                second: 'HH:mm:ss'
                            }
                        }
                    },
                    y: {
                        beginAtZero: true
                    }
                },
                plugins: {
                    legend: {
                        display: false
                    }
                }
            }
        });
    }

    if (realtimeThroughputCtx) {
        charts.realtimeThroughput = new Chart(realtimeThroughputCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Throughput (ops/sec)',
                    data: [],
                    backgroundColor: 'rgba(220, 53, 69, 0.1)',
                    borderColor: 'rgba(220, 53, 69, 1)',
                    borderWidth: 2,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    x: {
                        type: 'time',
                        time: {
                            displayFormats: {
                                second: 'HH:mm:ss'
                            }
                        }
                    },
                    y: {
                        beginAtZero: true
                    }
                },
                plugins: {
                    legend: {
                        display: false
                    }
                }
            }
        });
    }
}

// Toggle real-time monitoring
function toggleRealTime() {
    realtimeEnabled = !realtimeEnabled;
    const icon = document.getElementById('realtime-icon');
    const status = document.getElementById('connection-status');

    if (realtimeEnabled) {
        icon.className = 'fas fa-pause';
        status.textContent = 'Live';
        status.className = 'badge bg-success';
        startRealtimeUpdates();
    } else {
        icon.className = 'fas fa-play';
        status.textContent = 'Paused';
        status.className = 'badge bg-warning';
        stopRealtimeUpdates();
    }
}

// Start real-time updates
function startRealtimeUpdates() {
    if (realtimeInterval) return;

    realtimeInterval = setInterval(async () => {
        try {
            // Simulate real-time data - in production, this would fetch from an API
            const latency = Math.random() * 100 + 10;
            const throughput = Math.random() * 1000 + 500;
            const now = new Date();

            updateRealtimeChart(charts.realtimeLatency, now, latency);
            updateRealtimeChart(charts.realtimeThroughput, now, throughput);
        } catch (error) {
            console.error('Real-time update failed:', error);
            // Update connection status to show error
            const status = document.getElementById('connection-status');
            status.textContent = 'Error';
            status.className = 'badge bg-danger';
        }
    }, 1000); // Update every second
}

// Stop real-time updates
function stopRealtimeUpdates() {
    if (realtimeInterval) {
        clearInterval(realtimeInterval);
        realtimeInterval = null;
    }
}

// Update real-time chart with new data point
function updateRealtimeChart(chart, timestamp, value) {
    if (!chart) return;

    chart.data.labels.push(timestamp);
    chart.data.datasets[0].data.push(value);

    // Keep only last 60 data points
    if (chart.data.labels.length > 60) {
        chart.data.labels.shift();
        chart.data.datasets[0].data.shift();
    }

    chart.update('none');
}

// Update timestamp display
function updateTimestamp() {
    const timestampElement = document.getElementById('test-timestamp');
    if (timestampElement) {
        timestampElement.textContent = new Date().toLocaleString();
    }
}

// Theme switching
function switchTheme(theme) {
    document.documentElement.className = `theme-${theme}`;
    localStorage.setItem('theme', theme);
    
    // Update all charts for new theme
    Object.values(charts).forEach(chart => {
        if (chart) chart.update();
    });
}

// Load saved theme
document.addEventListener('DOMContentLoaded', function() {
    const savedTheme = localStorage.getItem('theme') || 'auto';
    if (savedTheme !== 'auto') {
        switchTheme(savedTheme);
    }
});

// Export functionality
function exportReport(format) {
    switch (format) {
        case 'pdf':
            window.print();
            break;
        case 'csv':
            exportToCSV();
            break;
        case 'json':
            exportToJSON();
            break;
        default:
            console.error('Unknown export format:', format);
    }
}

function exportToCSV() {
    // Implementation for CSV export
    console.log('CSV export not yet implemented');
}

function exportToJSON() {
    // Implementation for JSON export
    console.log('JSON export not yet implemented');
}

// Error handling
window.addEventListener('error', function(event) {
    console.error('Dashboard error:', event.error);
    // You could send this to a logging service
});

// Performance monitoring
if ('performance' in window) {
    window.addEventListener('load', function() {
        setTimeout(function() {
            const perfData = performance.getEntriesByType('navigation')[0];
            console.log('Page load time:', perfData.loadEventEnd - perfData.loadEventStart, 'ms');
        }, 0);
    });
}
"#.to_string())
    }
}

// Default instance creation function
impl Default for HtmlReportGenerator {
    fn default() -> Self {
        Self::new(HtmlReportConfig::default())
    }
}

/// Builder pattern for HTML report configuration
pub struct HtmlReportConfigBuilder {
    config: HtmlReportConfig,
}

impl HtmlReportConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: HtmlReportConfig::default(),
        }
    }

    pub fn output_dir<S: Into<String>>(mut self, dir: S) -> Self {
        self.config.output_dir = dir.into();
        self
    }

    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.config.title = title.into();
        self
    }

    pub fn chart_library(mut self, library: ChartLibrary) -> Self {
        self.config.chart_library = library;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.config.theme = theme;
        self
    }

    pub fn enable_real_time(mut self, enabled: bool) -> Self {
        self.config.real_time_updates = enabled;
        self
    }

    pub fn build(self) -> HtmlReportConfig {
        self.config
    }
}

impl Default for HtmlReportConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}