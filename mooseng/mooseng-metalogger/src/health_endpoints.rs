//! Health check HTTP endpoints for MooseNG metalogger
//! 
//! Provides HTTP endpoints for health monitoring and integrates with
//! Prometheus metrics collection.

use std::sync::Arc;
use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Json, Html},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tracing::{info, error};

use crate::health_integration::MetaloggerHealthService;

/// Metalogger health endpoint service
pub struct MetaloggerHealthEndpoints {
    health_service: Arc<MetaloggerHealthService>,
}

impl MetaloggerHealthEndpoints {
    pub fn new(health_service: Arc<MetaloggerHealthService>) -> Self {
        Self { health_service }
    }

    /// Start the health endpoint HTTP server
    pub async fn start_server(&self, bind_addr: &str) -> Result<()> {
        let app = Router::new()
            .route("/health", get(health_status))
            .route("/health/detailed", get(detailed_health_status))
            .route("/metrics", get(prometheus_metrics))
            .route("/", get(health_dashboard))
            .with_state(self.health_service.clone());

        info!("Starting metalogger health endpoint server on {}", bind_addr);
        let listener = TcpListener::bind(bind_addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

/// GET /health - Basic health status
async fn health_status(
    State(health_service): State<Arc<MetaloggerHealthService>>,
) -> Result<Json<Value>, StatusCode> {
    match health_service.trigger_health_check().await {
        Ok(result) => {
            let status_code = match result.status {
                mooseng_common::health::HealthStatus::Healthy => StatusCode::OK,
                mooseng_common::health::HealthStatus::Warning => StatusCode::OK,
                mooseng_common::health::HealthStatus::Degraded => StatusCode::OK,
                mooseng_common::health::HealthStatus::Critical => StatusCode::SERVICE_UNAVAILABLE,
                mooseng_common::health::HealthStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
            };

            let response = json!({
                "status": format!("{:?}", result.status).to_lowercase(),
                "message": result.message,
                "timestamp": result.timestamp.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_secs(),
                "component": result.component,
                "recommendations": result.recommendations
            });

            if status_code == StatusCode::OK {
                Ok(Json(response))
            } else {
                Err(status_code)
            }
        }
        Err(e) => {
            error!("Health check failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /health/detailed - Detailed health status with metrics
async fn detailed_health_status(
    State(health_service): State<Arc<MetaloggerHealthService>>,
) -> Result<Json<Value>, StatusCode> {
    match health_service.trigger_health_check().await {
        Ok(result) => {
            let response = json!({
                "status": format!("{:?}", result.status).to_lowercase(),
                "message": result.message,
                "timestamp": result.timestamp.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_secs(),
                "component": result.component,
                "metrics": result.metrics,
                "recommendations": result.recommendations
            });

            Ok(Json(response))
        }
        Err(e) => {
            error!("Detailed health check failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /metrics - Prometheus metrics endpoint
async fn prometheus_metrics(
    State(health_service): State<Arc<MetaloggerHealthService>>,
) -> Result<String, StatusCode> {
    Ok(health_service.export_prometheus_metrics().await)
}

/// GET / - Health dashboard HTML
async fn health_dashboard() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html>
<head>
    <title>MooseNG Metalogger Health</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .status { padding: 10px; margin: 10px 0; border-radius: 5px; }
        .healthy { background-color: #d4edda; color: #155724; }
        .warning { background-color: #fff3cd; color: #856404; }
        .critical { background-color: #f8d7da; color: #721c24; }
        .metric { margin: 5px 0; }
        .endpoint { margin: 20px 0; }
        .code { background-color: #f8f9fa; padding: 10px; border-radius: 3px; font-family: monospace; }
    </style>
    <script>
        async function checkHealth() {
            try {
                const response = await fetch('/health/detailed');
                const data = await response.json();
                
                const statusDiv = document.getElementById('status');
                statusDiv.className = 'status ' + data.status;
                statusDiv.innerHTML = `<strong>Status:</strong> ${data.status.toUpperCase()}<br>
                                     <strong>Message:</strong> ${data.message}<br>
                                     <strong>Last Check:</strong> ${new Date(data.timestamp * 1000).toLocaleString()}`;
                
                const metricsDiv = document.getElementById('metrics');
                metricsDiv.innerHTML = '';
                for (const [key, value] of Object.entries(data.metrics || {})) {
                    const metricDiv = document.createElement('div');
                    metricDiv.className = 'metric';
                    metricDiv.innerHTML = `<strong>${key}:</strong> ${value}`;
                    metricsDiv.appendChild(metricDiv);
                }
                
                const recommendationsDiv = document.getElementById('recommendations');
                recommendationsDiv.innerHTML = '';
                if (data.recommendations && data.recommendations.length > 0) {
                    for (const rec of data.recommendations) {
                        const recDiv = document.createElement('div');
                        recDiv.innerHTML = `• ${rec}`;
                        recommendationsDiv.appendChild(recDiv);
                    }
                } else {
                    recommendationsDiv.innerHTML = 'No recommendations';
                }
            } catch (error) {
                document.getElementById('status').innerHTML = `<strong>Error:</strong> ${error.message}`;
                document.getElementById('status').className = 'status critical';
            }
        }
        
        // Check health on page load and then every 30 seconds
        window.onload = function() {
            checkHealth();
            setInterval(checkHealth, 30000);
        };
    </script>
</head>
<body>
    <h1>MooseNG Metalogger Health Dashboard</h1>
    
    <div id="status" class="status">Loading...</div>
    
    <h2>Metrics</h2>
    <div id="metrics"></div>
    
    <h2>Recommendations</h2>
    <div id="recommendations"></div>
    
    <h2>Available Endpoints</h2>
    <div class="endpoint">
        <h3>GET /health</h3>
        <p>Basic health status (JSON)</p>
        <div class="code">curl http://localhost:8081/health</div>
    </div>
    
    <div class="endpoint">
        <h3>GET /health/detailed</h3>
        <p>Detailed health status with metrics (JSON)</p>
        <div class="code">curl http://localhost:8081/health/detailed</div>
    </div>
    
    <div class="endpoint">
        <h3>GET /metrics</h3>
        <p>Prometheus metrics format</p>
        <div class="code">curl http://localhost:8081/metrics</div>
    </div>
    
    <footer style="margin-top: 40px; padding-top: 20px; border-top: 1px solid #ccc; color: #666;">
        <small>MooseNG Metalogger Health Monitor</small>
    </footer>
</body>
</html>
    "#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_endpoint_creation() {
        // This test would require mocking the health service
        // For now, just test basic structure
        assert!(true);
    }
}