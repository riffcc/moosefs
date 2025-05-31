//! Health check endpoints with Prometheus metrics integration
//! 
//! Provides HTTP endpoints for health monitoring and integrates with
//! Prometheus metrics collection for comprehensive monitoring.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

// HTTP server imports
use axum::{
    extract::{Query, Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;

use super::disk_monitor::{DiskSpaceMonitor as DiskMonitor, DiskMonitorConfig};
use super::load_monitor::{LoadMonitor, LoadMonitorConfig};
use super::consistency_monitor::{ConsistencyMonitor, ConsistencyMonitorConfig};

/// Health monitoring service
pub struct HealthService {
    pub disk_monitor: Arc<RwLock<DiskMonitor>>,
    pub load_monitor: Arc<RwLock<LoadMonitor>>,
    pub consistency_monitor: Arc<RwLock<ConsistencyMonitor>>,
}

/// Health server state for Axum handlers
#[derive(Clone)]
pub struct HealthServerState {
    pub disk_monitor: Arc<RwLock<DiskMonitor>>,
    pub system_monitor: Arc<RwLock<LoadMonitor>>,
    pub consistency_checker: Arc<RwLock<ConsistencyMonitor>>,
}

/// Overall health status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthLevel,
    pub timestamp: u64,
    pub checks: HashMap<String, HealthCheckDetail>,
    pub summary: HealthSummary,
    pub alerts: Vec<HealthAlert>,
}

/// Individual health check detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckDetail {
    pub name: String,
    pub status: HealthLevel,
    pub message: String,
    pub metrics: HashMap<String, f64>,
    pub last_check: u64,
    pub duration_ms: u64,
}

/// Health status levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthLevel {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "critical")]
    Critical,
    #[serde(rename = "unknown")]
    Unknown,
}

/// Health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    pub total_checks: u32,
    pub healthy_checks: u32,
    pub warning_checks: u32,
    pub critical_checks: u32,
    pub uptime_seconds: u64,
    pub last_full_check: u64,
}

/// Health alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub id: String,
    pub level: AlertLevel,
    pub component: String,
    pub message: String,
    pub timestamp: u64,
    pub acknowledged: bool,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "critical")]
    Critical,
}

/// Query parameters for health checks
#[derive(Debug, Deserialize)]
pub struct HealthQuery {
    /// Include detailed metrics
    detailed: Option<bool>,
    /// Filter by check type
    check_type: Option<String>,
    /// Timeout for checks in seconds
    timeout: Option<u64>,
}

/// Disk space query parameters
#[derive(Debug, Deserialize)]
pub struct DiskQuery {
    /// Path to check (optional)
    path: Option<String>,
    /// Include usage breakdown
    breakdown: Option<bool>,
}

/// System load query parameters
#[derive(Debug, Deserialize)]
pub struct LoadQuery {
    /// Time window in seconds
    window: Option<u64>,
    /// Include process details
    processes: Option<bool>,
}

/// Data consistency query parameters
#[derive(Debug, Deserialize)]
pub struct ConsistencyQuery {
    /// Chunk ID to check (optional)
    chunk_id: Option<String>,
    /// Region to focus on (optional)
    region: Option<String>,
    /// Deep check (slower but more thorough)
    deep: Option<bool>,
}

/// Start the health check HTTP server
pub async fn start_health_server(
    bind_addr: &str,
    state: HealthServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/health", get(overall_health))
        .route("/health/detailed", get(detailed_health))
        .route("/health/disk", get(disk_health))
        .route("/health/load", get(system_load))
        .route("/health/consistency", get(data_consistency))
        .route("/health/checks/:check_name", get(individual_check))
        .route("/health/alerts", get(get_alerts))
        .route("/health/alerts/:alert_id/ack", post(acknowledge_alert))
        .route("/metrics", get(prometheus_metrics))
        .with_state(state);

    info!("Starting health check server on {}", bind_addr);
    let listener = TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// GET /health - Overall health status
async fn overall_health(
    Query(params): Query<HealthQuery>,
    State(state): State<HealthServerState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let timeout = Duration::from_secs(params.timeout.unwrap_or(10));
    let detailed = params.detailed.unwrap_or(false);
    
    match perform_health_checks(&state, timeout, detailed).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!("Health check failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /health/detailed - Detailed health status with all metrics
async fn detailed_health(
    State(state): State<HealthServerState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let timeout = Duration::from_secs(30);
    
    match perform_health_checks(&state, timeout, true).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!("Detailed health check failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /health/disk - Disk space utilization check
async fn disk_health(
    Query(params): Query<DiskQuery>,
    State(state): State<HealthServerState>,
) -> Result<Json<HealthCheckDetail>, StatusCode> {
    let start_time = SystemTime::now();
    
    match state.disk_monitor.check_disk_space(params.path.as_deref()).await {
        Ok(metrics) => {
            let duration = start_time.elapsed().unwrap_or_default();
            let status = determine_disk_health_status(&metrics);
            
            let detail = HealthCheckDetail {
                name: "disk_space".to_string(),
                status,
                message: format_disk_health_message(&metrics),
                metrics,
                last_check: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                duration_ms: duration.as_millis() as u64,
            };
            
            Ok(Json(detail))
        }
        Err(e) => {
            error!("Disk health check failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /health/load - System load and resource utilization
async fn system_load(
    Query(params): Query<LoadQuery>,
    State(state): State<HealthServerState>,
) -> Result<Json<HealthCheckDetail>, StatusCode> {
    let start_time = SystemTime::now();
    let window = Duration::from_secs(params.window.unwrap_or(300)); // 5 minute default
    
    match state.system_monitor.check_system_load(window).await {
        Ok(metrics) => {
            let duration = start_time.elapsed().unwrap_or_default();
            let status = determine_load_health_status(&metrics);
            
            let detail = HealthCheckDetail {
                name: "system_load".to_string(),
                status,
                message: format_load_health_message(&metrics),
                metrics,
                last_check: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                duration_ms: duration.as_millis() as u64,
            };
            
            Ok(Json(detail))
        }
        Err(e) => {
            error!("System load check failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /health/consistency - Data consistency across replicas
async fn data_consistency(
    Query(params): Query<ConsistencyQuery>,
    State(state): State<HealthServerState>,
) -> Result<Json<HealthCheckDetail>, StatusCode> {
    let start_time = SystemTime::now();
    let deep_check = params.deep.unwrap_or(false);
    
    match state.consistency_checker.check_data_consistency(
        params.chunk_id.as_deref(),
        params.region.as_deref(),
        deep_check,
    ).await {
        Ok(metrics) => {
            let duration = start_time.elapsed().unwrap_or_default();
            let status = determine_consistency_health_status(&metrics);
            
            let detail = HealthCheckDetail {
                name: "data_consistency".to_string(),
                status,
                message: format_consistency_health_message(&metrics),
                metrics,
                last_check: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                duration_ms: duration.as_millis() as u64,
            };
            
            Ok(Json(detail))
        }
        Err(e) => {
            error!("Data consistency check failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /health/checks/:check_name - Individual health check
async fn individual_check(
    Path(check_name): Path<String>,
    State(state): State<HealthServerState>,
) -> Result<Json<HealthCheckDetail>, StatusCode> {
    match check_name.as_str() {
        "disk" => disk_health(Query(DiskQuery { path: None, breakdown: Some(true) }), State(state)).await,
        "load" => system_load(Query(LoadQuery { window: None, processes: Some(true) }), State(state)).await,
        "consistency" => data_consistency(Query(ConsistencyQuery { 
            chunk_id: None, 
            region: None, 
            deep: Some(false) 
        }), State(state)).await,
        _ => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /health/alerts - Get current alerts
async fn get_alerts(
    State(state): State<HealthServerState>,
) -> Result<Json<Vec<HealthAlert>>, StatusCode> {
    // In a real implementation, this would fetch from an alert store
    let alerts = vec![
        // Sample alerts - would be real alerts in production
    ];
    
    Ok(Json(alerts))
}

/// POST /health/alerts/:alert_id/ack - Acknowledge an alert
async fn acknowledge_alert(
    Path(alert_id): Path<String>,
    State(_state): State<HealthServerState>,
) -> Result<StatusCode, StatusCode> {
    // In a real implementation, this would update the alert store
    info!("Alert {} acknowledged", alert_id);
    Ok(StatusCode::OK)
}

/// GET /metrics - Prometheus metrics endpoint
async fn prometheus_metrics(
    State(state): State<HealthServerState>,
) -> Result<String, StatusCode> {
    // Export all health-related metrics in Prometheus format
    let mut metrics = String::new();
    
    // Add basic health status metrics
    metrics.push_str("# HELP mooseng_health_check_status Health check status (1=healthy, 0=unhealthy)\n");
    metrics.push_str("# TYPE mooseng_health_check_status gauge\n");
    
    // System load metrics
    if let Ok(load_metrics) = state.system_monitor.check_system_load(Duration::from_secs(300)).await {
        for (metric_name, value) in load_metrics {
            metrics.push_str(&format!(
                "mooseng_system_{} {}\n",
                metric_name.replace('.', "_"),
                value
            ));
        }
    }
    
    // Disk space metrics
    if let Ok(disk_metrics) = state.disk_monitor.check_disk_space(None).await {
        for (metric_name, value) in disk_metrics {
            metrics.push_str(&format!(
                "mooseng_disk_{} {}\n",
                metric_name.replace('.', "_"),
                value
            ));
        }
    }
    
    // Data consistency metrics
    if let Ok(consistency_metrics) = state.consistency_checker.check_data_consistency(None, None, false).await {
        for (metric_name, value) in consistency_metrics {
            metrics.push_str(&format!(
                "mooseng_consistency_{} {}\n",
                metric_name.replace('.', "_"),
                value
            ));
        }
    }
    
    Ok(metrics)
}

/// Perform comprehensive health checks
async fn perform_health_checks(
    state: &HealthServerState,
    timeout: Duration,
    detailed: bool,
) -> Result<HealthResponse, Box<dyn std::error::Error>> {
    let start_time = SystemTime::now();
    let mut checks = HashMap::new();
    let mut alerts = Vec::new();
    
    // Perform all health checks concurrently
    let (disk_result, load_result, consistency_result) = tokio::join!(
        tokio::time::timeout(timeout, state.disk_monitor.check_disk_space(None)),
        tokio::time::timeout(timeout, state.system_monitor.check_system_load(Duration::from_secs(300))),
        tokio::time::timeout(timeout, state.consistency_checker.check_data_consistency(None, None, false)),
    );
    
    // Process disk check results
    let disk_status = match disk_result {
        Ok(Ok(metrics)) => {
            let status = determine_disk_health_status(&metrics);
            let check = HealthCheckDetail {
                name: "disk_space".to_string(),
                status: status.clone(),
                message: format_disk_health_message(&metrics),
                metrics: if detailed { metrics } else { HashMap::new() },
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
            };
            checks.insert("disk_space".to_string(), check);
            status
        }
        Ok(Err(e)) => {
            warn!("Disk health check failed: {}", e);
            let check = HealthCheckDetail {
                name: "disk_space".to_string(),
                status: HealthLevel::Critical,
                message: format!("Disk check failed: {}", e),
                metrics: HashMap::new(),
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
            };
            checks.insert("disk_space".to_string(), check);
            HealthLevel::Critical
        }
        Err(_) => {
            let check = HealthCheckDetail {
                name: "disk_space".to_string(),
                status: HealthLevel::Critical,
                message: "Disk check timed out".to_string(),
                metrics: HashMap::new(),
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: timeout.as_millis() as u64,
            };
            checks.insert("disk_space".to_string(), check);
            HealthLevel::Critical
        }
    };
    
    // Process load check results
    let load_status = match load_result {
        Ok(Ok(metrics)) => {
            let status = determine_load_health_status(&metrics);
            let check = HealthCheckDetail {
                name: "system_load".to_string(),
                status: status.clone(),
                message: format_load_health_message(&metrics),
                metrics: if detailed { metrics } else { HashMap::new() },
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
            };
            checks.insert("system_load".to_string(), check);
            status
        }
        Ok(Err(e)) => {
            warn!("System load check failed: {}", e);
            let check = HealthCheckDetail {
                name: "system_load".to_string(),
                status: HealthLevel::Warning,
                message: format!("Load check failed: {}", e),
                metrics: HashMap::new(),
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
            };
            checks.insert("system_load".to_string(), check);
            HealthLevel::Warning
        }
        Err(_) => {
            let check = HealthCheckDetail {
                name: "system_load".to_string(),
                status: HealthLevel::Warning,
                message: "Load check timed out".to_string(),
                metrics: HashMap::new(),
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: timeout.as_millis() as u64,
            };
            checks.insert("system_load".to_string(), check);
            HealthLevel::Warning
        }
    };
    
    // Process consistency check results
    let consistency_status = match consistency_result {
        Ok(Ok(metrics)) => {
            let status = determine_consistency_health_status(&metrics);
            let check = HealthCheckDetail {
                name: "data_consistency".to_string(),
                status: status.clone(),
                message: format_consistency_health_message(&metrics),
                metrics: if detailed { metrics } else { HashMap::new() },
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
            };
            checks.insert("data_consistency".to_string(), check);
            status
        }
        Ok(Err(e)) => {
            warn!("Data consistency check failed: {}", e);
            let check = HealthCheckDetail {
                name: "data_consistency".to_string(),
                status: HealthLevel::Critical,
                message: format!("Consistency check failed: {}", e),
                metrics: HashMap::new(),
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: start_time.elapsed().unwrap_or_default().as_millis() as u64,
            };
            checks.insert("data_consistency".to_string(), check);
            HealthLevel::Critical
        }
        Err(_) => {
            let check = HealthCheckDetail {
                name: "data_consistency".to_string(),
                status: HealthLevel::Critical,
                message: "Consistency check timed out".to_string(),
                metrics: HashMap::new(),
                last_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                duration_ms: timeout.as_millis() as u64,
            };
            checks.insert("data_consistency".to_string(), check);
            HealthLevel::Critical
        }
    };
    
    // Determine overall health status
    let overall_status = determine_overall_health_status(&[disk_status, load_status, consistency_status]);
    
    // Calculate summary
    let total_checks = checks.len() as u32;
    let healthy_checks = checks.values().filter(|c| c.status == HealthLevel::Healthy).count() as u32;
    let warning_checks = checks.values().filter(|c| c.status == HealthLevel::Warning).count() as u32;
    let critical_checks = checks.values().filter(|c| c.status == HealthLevel::Critical).count() as u32;
    
    let summary = HealthSummary {
        total_checks,
        healthy_checks,
        warning_checks,
        critical_checks,
        uptime_seconds: 0, // Would be calculated from process start time
        last_full_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    };
    
    Ok(HealthResponse {
        status: overall_status,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        checks,
        summary,
        alerts,
    })
}

/// Determine health status based on disk metrics
fn determine_disk_health_status(metrics: &HashMap<String, f64>) -> HealthLevel {
    let usage_percent = metrics.get("usage_percent").unwrap_or(&0.0);
    let available_gb = metrics.get("available_gb").unwrap_or(&100.0);
    
    if *usage_percent > 95.0 || *available_gb < 1.0 {
        HealthLevel::Critical
    } else if *usage_percent > 85.0 || *available_gb < 10.0 {
        HealthLevel::Warning
    } else {
        HealthLevel::Healthy
    }
}

/// Determine health status based on system load metrics
fn determine_load_health_status(metrics: &HashMap<String, f64>) -> HealthLevel {
    let cpu_percent = metrics.get("cpu_percent").unwrap_or(&0.0);
    let memory_percent = metrics.get("memory_percent").unwrap_or(&0.0);
    let load_average = metrics.get("load_average_1m").unwrap_or(&0.0);
    
    if *cpu_percent > 90.0 || *memory_percent > 95.0 || *load_average > 8.0 {
        HealthLevel::Critical
    } else if *cpu_percent > 75.0 || *memory_percent > 85.0 || *load_average > 4.0 {
        HealthLevel::Warning
    } else {
        HealthLevel::Healthy
    }
}

/// Determine health status based on consistency metrics
fn determine_consistency_health_status(metrics: &HashMap<String, f64>) -> HealthLevel {
    let inconsistent_chunks = metrics.get("inconsistent_chunks").unwrap_or(&0.0);
    let missing_replicas = metrics.get("missing_replicas").unwrap_or(&0.0);
    let consistency_ratio = metrics.get("consistency_ratio").unwrap_or(&1.0);
    
    if *inconsistent_chunks > 0.0 || *missing_replicas > 0.0 || *consistency_ratio < 0.95 {
        HealthLevel::Critical
    } else if *consistency_ratio < 0.99 {
        HealthLevel::Warning
    } else {
        HealthLevel::Healthy
    }
}

/// Determine overall health status from individual check statuses
fn determine_overall_health_status(statuses: &[HealthLevel]) -> HealthLevel {
    if statuses.iter().any(|s| *s == HealthLevel::Critical) {
        HealthLevel::Critical
    } else if statuses.iter().any(|s| *s == HealthLevel::Warning) {
        HealthLevel::Warning
    } else if statuses.iter().all(|s| *s == HealthLevel::Healthy) {
        HealthLevel::Healthy
    } else {
        HealthLevel::Unknown
    }
}

/// Format disk health message
fn format_disk_health_message(metrics: &HashMap<String, f64>) -> String {
    let usage_percent = metrics.get("usage_percent").unwrap_or(&0.0);
    let available_gb = metrics.get("available_gb").unwrap_or(&0.0);
    let total_gb = metrics.get("total_gb").unwrap_or(&0.0);
    
    format!(
        "Disk usage: {:.1}% ({:.1} GB available of {:.1} GB total)",
        usage_percent, available_gb, total_gb
    )
}

/// Format system load health message
fn format_load_health_message(metrics: &HashMap<String, f64>) -> String {
    let cpu_percent = metrics.get("cpu_percent").unwrap_or(&0.0);
    let memory_percent = metrics.get("memory_percent").unwrap_or(&0.0);
    let load_average = metrics.get("load_average_1m").unwrap_or(&0.0);
    
    format!(
        "System load: CPU {:.1}%, Memory {:.1}%, Load avg {:.2}",
        cpu_percent, memory_percent, load_average
    )
}

/// Format data consistency health message
fn format_consistency_health_message(metrics: &HashMap<String, f64>) -> String {
    let consistency_ratio = metrics.get("consistency_ratio").unwrap_or(&1.0);
    let total_chunks = metrics.get("total_chunks").unwrap_or(&0.0);
    let inconsistent_chunks = metrics.get("inconsistent_chunks").unwrap_or(&0.0);
    
    format!(
        "Data consistency: {:.2}% ({} inconsistent out of {} total chunks)",
        consistency_ratio * 100.0, inconsistent_chunks as u64, total_chunks as u64
    )
}