//! Monitoring dashboard for MooseNG health metrics
//! 
//! This module provides a web-based dashboard for visualizing:
//! - Real-time health status across all components
//! - System performance metrics and trends
//! - Disk utilization and storage tier status
//! - Data consistency metrics and alerts
//! - Historical data and performance trends

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::health::{HealthServerState, HealthLevel};

/// Dashboard server state
#[derive(Clone)]
pub struct DashboardState {
    health_state: HealthServerState,
    metrics_history: Arc<tokio::sync::RwLock<MetricsHistory>>,
}

/// Historical metrics storage
#[derive(Debug, Clone)]
struct MetricsHistory {
    disk_metrics: Vec<TimestampedMetrics>,
    system_metrics: Vec<TimestampedMetrics>,
    consistency_metrics: Vec<TimestampedMetrics>,
    max_history_points: usize,
}

/// Timestamped metrics point
#[derive(Debug, Clone, Serialize)]
struct TimestampedMetrics {
    timestamp: u64,
    metrics: HashMap<String, f64>,
}

/// Dashboard query parameters
#[derive(Debug, Deserialize)]
struct DashboardQuery {
    /// Time range for metrics (1h, 6h, 24h, 7d)
    range: Option<String>,
    /// Refresh interval in seconds
    refresh: Option<u64>,
    /// Component filter
    component: Option<String>,
}

/// Real-time metrics update
#[derive(Debug, Serialize)]
struct MetricsUpdate {
    timestamp: u64,
    overall_health: HealthLevel,
    disk_health: HealthLevel,
    system_health: HealthLevel,
    consistency_health: HealthLevel,
    key_metrics: HashMap<String, f64>,
    alerts: Vec<DashboardAlert>,
}

/// Dashboard alert
#[derive(Debug, Clone, Serialize)]
struct DashboardAlert {
    id: String,
    level: String,
    component: String,
    message: String,
    timestamp: u64,
    acknowledged: bool,
}

impl MetricsHistory {
    fn new() -> Self {
        Self {
            disk_metrics: Vec::new(),
            system_metrics: Vec::new(),
            consistency_metrics: Vec::new(),
            max_history_points: 1440, // 24 hours at 1-minute intervals
        }
    }

    fn add_disk_metrics(&mut self, metrics: HashMap<String, f64>) {
        self.disk_metrics.push(TimestampedMetrics {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            metrics,
        });

        if self.disk_metrics.len() > self.max_history_points {
            self.disk_metrics.drain(0..self.disk_metrics.len() - self.max_history_points);
        }
    }

    fn add_system_metrics(&mut self, metrics: HashMap<String, f64>) {
        self.system_metrics.push(TimestampedMetrics {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            metrics,
        });

        if self.system_metrics.len() > self.max_history_points {
            self.system_metrics.drain(0..self.system_metrics.len() - self.max_history_points);
        }
    }

    fn add_consistency_metrics(&mut self, metrics: HashMap<String, f64>) {
        self.consistency_metrics.push(TimestampedMetrics {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            metrics,
        });

        if self.consistency_metrics.len() > self.max_history_points {
            self.consistency_metrics.drain(0..self.consistency_metrics.len() - self.max_history_points);
        }
    }

    fn get_metrics_in_range(&self, metric_type: &str, hours: u64) -> Vec<TimestampedMetrics> {
        let cutoff = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - (hours * 3600);
        
        let metrics_vec = match metric_type {
            "disk" => &self.disk_metrics,
            "system" => &self.system_metrics,
            "consistency" => &self.consistency_metrics,
            _ => return Vec::new(),
        };

        metrics_vec.iter()
            .filter(|m| m.timestamp >= cutoff)
            .cloned()
            .collect()
    }
}

/// Start the monitoring dashboard server
pub async fn start_dashboard_server(
    bind_addr: &str,
    health_state: HealthServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let dashboard_state = DashboardState {
        health_state,
        metrics_history: Arc::new(tokio::sync::RwLock::new(MetricsHistory::new())),
    };

    // Start metrics collection background task
    let metrics_state = dashboard_state.clone();
    tokio::spawn(async move {
        metrics_collection_task(metrics_state).await;
    });

    let app = Router::new()
        .route("/", get(dashboard_home))
        .route("/api/metrics/current", get(current_metrics))
        .route("/api/metrics/history", get(historical_metrics))
        .route("/api/health/summary", get(health_summary))
        .route("/api/alerts", get(get_alerts))
        .route("/api/alerts/:alert_id/ack", post(acknowledge_alert))
        .with_state(dashboard_state);

    info!("Starting monitoring dashboard on {}", bind_addr);
    let listener = TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// Background task to collect metrics periodically
async fn metrics_collection_task(state: DashboardState) {
    let mut interval = tokio::time::interval(Duration::from_secs(60)); // Collect every minute
    
    loop {
        interval.tick().await;
        
        // Collect disk metrics
        if let Ok(disk_metrics) = state.health_state.disk_monitor.check_disk_space(None).await {
            let mut history = state.metrics_history.write().await;
            history.add_disk_metrics(disk_metrics);
        }
        
        // Collect system metrics
        if let Ok(system_metrics) = state.health_state.system_monitor.check_system_load(Duration::from_secs(300)).await {
            let mut history = state.metrics_history.write().await;
            history.add_system_metrics(system_metrics);
        }
        
        // Collect consistency metrics
        if let Ok(consistency_metrics) = state.health_state.consistency_checker.check_data_consistency(None, None, false).await {
            let mut history = state.metrics_history.write().await;
            history.add_consistency_metrics(consistency_metrics);
        }
    }
}

/// GET / - Dashboard home page
async fn dashboard_home() -> Html<String> {
    let html = include_str!("dashboard.html");
    Html(html.to_string())
}

/// GET /api/metrics/current - Current metrics
async fn current_metrics(
    State(state): State<DashboardState>,
) -> Result<Json<MetricsUpdate>, StatusCode> {
    // Collect current metrics from all components
    let disk_result = state.health_state.disk_monitor.check_disk_space(None).await;
    let system_result = state.health_state.system_monitor.check_system_load(Duration::from_secs(300)).await;
    let consistency_result = state.health_state.consistency_checker.check_data_consistency(None, None, false).await;
    
    let mut key_metrics = HashMap::new();
    let mut overall_health = HealthLevel::Healthy;
    let mut disk_health = HealthLevel::Healthy;
    let mut system_health = HealthLevel::Healthy;
    let mut consistency_health = HealthLevel::Healthy;
    
    // Process disk metrics
    if let Ok(disk_metrics) = disk_result {
        let usage_percent = disk_metrics.get("usage_percent").unwrap_or(&0.0);
        key_metrics.insert("disk_usage_percent".to_string(), *usage_percent);
        
        if *usage_percent > 95.0 {
            disk_health = HealthLevel::Critical;
        } else if *usage_percent > 85.0 {
            disk_health = HealthLevel::Warning;
        }
    } else {
        disk_health = HealthLevel::Critical;
    }
    
    // Process system metrics
    if let Ok(system_metrics) = system_result {
        let cpu_percent = system_metrics.get("cpu_percent").unwrap_or(&0.0);
        let memory_percent = system_metrics.get("memory_percent").unwrap_or(&0.0);
        let load_avg = system_metrics.get("load_average_1m").unwrap_or(&0.0);
        
        key_metrics.insert("cpu_percent".to_string(), *cpu_percent);
        key_metrics.insert("memory_percent".to_string(), *memory_percent);
        key_metrics.insert("load_average".to_string(), *load_avg);
        
        if *cpu_percent > 90.0 || *memory_percent > 95.0 {
            system_health = HealthLevel::Critical;
        } else if *cpu_percent > 75.0 || *memory_percent > 85.0 {
            system_health = HealthLevel::Warning;
        }
    } else {
        system_health = HealthLevel::Critical;
    }
    
    // Process consistency metrics
    if let Ok(consistency_metrics) = consistency_result {
        let consistency_ratio = consistency_metrics.get("consistency_ratio").unwrap_or(&1.0);
        key_metrics.insert("consistency_ratio".to_string(), *consistency_ratio);
        
        if *consistency_ratio < 0.95 {
            consistency_health = HealthLevel::Critical;
        } else if *consistency_ratio < 0.99 {
            consistency_health = HealthLevel::Warning;
        }
    } else {
        consistency_health = HealthLevel::Critical;
    }
    
    // Determine overall health
    overall_health = match (&disk_health, &system_health, &consistency_health) {
        (HealthLevel::Critical, _, _) | (_, HealthLevel::Critical, _) | (_, _, HealthLevel::Critical) => HealthLevel::Critical,
        (HealthLevel::Warning, _, _) | (_, HealthLevel::Warning, _) | (_, _, HealthLevel::Warning) => HealthLevel::Warning,
        _ => HealthLevel::Healthy,
    };
    
    let update = MetricsUpdate {
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        overall_health,
        disk_health,
        system_health,
        consistency_health,
        key_metrics,
        alerts: vec![], // Would be populated with actual alerts
    };
    
    Ok(Json(update))
}

/// GET /api/metrics/history - Historical metrics
async fn historical_metrics(
    Query(params): Query<DashboardQuery>,
    State(state): State<DashboardState>,
) -> Result<Json<HashMap<String, Vec<TimestampedMetrics>>>, StatusCode> {
    let hours = match params.range.as_deref() {
        Some("1h") => 1,
        Some("6h") => 6,
        Some("24h") => 24,
        Some("7d") => 24 * 7,
        _ => 24, // Default to 24 hours
    };
    
    let history = state.metrics_history.read().await;
    let mut response = HashMap::new();
    
    response.insert("disk".to_string(), history.get_metrics_in_range("disk", hours));
    response.insert("system".to_string(), history.get_metrics_in_range("system", hours));
    response.insert("consistency".to_string(), history.get_metrics_in_range("consistency", hours));
    
    Ok(Json(response))
}

/// GET /api/health/summary - Health summary
async fn health_summary(
    State(state): State<DashboardState>,
) -> Result<Json<HashMap<String, serde_json::Value>>, StatusCode> {
    let mut summary = HashMap::new();
    
    // Get current health status from all components
    if let Ok(disk_metrics) = state.health_state.disk_monitor.check_disk_space(None).await {
        summary.insert("disk".to_string(), serde_json::to_value(disk_metrics).unwrap());
    }
    
    if let Ok(system_metrics) = state.health_state.system_monitor.check_system_load(Duration::from_secs(300)).await {
        summary.insert("system".to_string(), serde_json::to_value(system_metrics).unwrap());
    }
    
    if let Ok(consistency_metrics) = state.health_state.consistency_checker.check_data_consistency(None, None, false).await {
        summary.insert("consistency".to_string(), serde_json::to_value(consistency_metrics).unwrap());
    }
    
    Ok(Json(summary))
}

/// GET /api/alerts - Get current alerts
async fn get_alerts(
    State(_state): State<DashboardState>,
) -> Result<Json<Vec<DashboardAlert>>, StatusCode> {
    // In a real implementation, this would fetch from an alert store
    let alerts = vec![
        // Sample alerts for demonstration
    ];
    
    Ok(Json(alerts))
}

/// POST /api/alerts/:alert_id/ack - Acknowledge an alert
async fn acknowledge_alert(
    axum::extract::Path(alert_id): axum::extract::Path<String>,
    State(_state): State<DashboardState>,
) -> Result<StatusCode, StatusCode> {
    // In a real implementation, this would update the alert store
    info!("Alert {} acknowledged via dashboard", alert_id);
    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{SystemMonitor, SystemMonitorConfig, DiskSpaceMonitor, DiskMonitorConfig, DataConsistencyChecker, ConsistencyConfig};

    #[tokio::test]
    async fn test_metrics_history() {
        let mut history = MetricsHistory::new();
        
        let mut metrics = HashMap::new();
        metrics.insert("test_metric".to_string(), 42.0);
        
        history.add_disk_metrics(metrics.clone());
        history.add_system_metrics(metrics.clone());
        history.add_consistency_metrics(metrics);
        
        assert_eq!(history.disk_metrics.len(), 1);
        assert_eq!(history.system_metrics.len(), 1);
        assert_eq!(history.consistency_metrics.len(), 1);
    }

    #[tokio::test]
    async fn test_dashboard_state() {
        let system_monitor = Arc::new(SystemMonitor::new(SystemMonitorConfig::default()).unwrap());
        let disk_monitor = Arc::new(DiskSpaceMonitor::new(DiskMonitorConfig::default()));
        let consistency_checker = Arc::new(DataConsistencyChecker::new(ConsistencyConfig::default()));
        
        let health_state = HealthServerState {
            system_monitor,
            disk_monitor,
            consistency_checker,
        };
        
        let dashboard_state = DashboardState {
            health_state,
            metrics_history: Arc::new(tokio::sync::RwLock::new(MetricsHistory::new())),
        };
        
        // Test that dashboard state can be created
        assert!(dashboard_state.metrics_history.read().await.disk_metrics.is_empty());
    }
}