//! Comprehensive health monitoring integration for MooseNG
//! 
//! This module integrates the individual component health checkers with
//! the overall monitoring system and provides a unified view of system health.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use mooseng_common::health::{HealthMonitor, HealthCheckConfig, HealthCheckResult, HealthStatus};
use tracing::{info, warn, error};

/// Unified health monitoring service for the entire MooseNG system
pub struct UnifiedHealthMonitor {
    health_monitor: HealthMonitor,
    component_endpoints: HashMap<String, String>,
}

impl UnifiedHealthMonitor {
    /// Create a new unified health monitor
    pub async fn new(config: UnifiedHealthConfig) -> Result<Self> {
        let health_config = HealthCheckConfig {
            interval: Duration::from_secs(config.check_interval_secs),
            timeout: Duration::from_secs(config.check_timeout_secs),
            failure_threshold: config.failure_threshold,
            recovery_threshold: config.recovery_threshold,
            enable_self_healing: config.enable_self_healing,
            max_healing_actions_per_hour: config.max_healing_actions_per_hour,
            component_checks: HashMap::new(),
        };

        let health_monitor = HealthMonitor::new(health_config);
        
        Ok(Self {
            health_monitor,
            component_endpoints: config.component_endpoints,
        })
    }

    /// Start the unified health monitoring
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting unified health monitoring system");
        self.health_monitor.start().await?;
        Ok(())
    }

    /// Get health status for all registered components
    pub async fn get_system_health(&self) -> SystemHealthReport {
        let component_health = self.health_monitor.get_health_status().await;
        
        let mut report = SystemHealthReport {
            overall_status: HealthStatus::Healthy,
            components: component_health.clone(),
            summary: HealthSummary::default(),
            alerts: Vec::new(),
            timestamp: std::time::SystemTime::now(),
        };

        // Calculate overall health status
        let mut has_critical = false;
        let mut has_warning = false;
        let mut has_degraded = false;

        for (component, health) in &component_health {
            match health.status {
                HealthStatus::Critical => {
                    has_critical = true;
                    report.alerts.push(HealthAlert {
                        component: component.clone(),
                        level: AlertLevel::Critical,
                        message: health.message.clone(),
                        timestamp: health.timestamp,
                    });
                }
                HealthStatus::Warning => {
                    has_warning = true;
                    report.alerts.push(HealthAlert {
                        component: component.clone(),
                        level: AlertLevel::Warning,
                        message: health.message.clone(),
                        timestamp: health.timestamp,
                    });
                }
                HealthStatus::Degraded => {
                    has_degraded = true;
                    report.alerts.push(HealthAlert {
                        component: component.clone(),
                        level: AlertLevel::Info,
                        message: health.message.clone(),
                        timestamp: health.timestamp,
                    });
                }
                HealthStatus::Unknown => {
                    report.alerts.push(HealthAlert {
                        component: component.clone(),
                        level: AlertLevel::Warning,
                        message: "Component health status unknown".to_string(),
                        timestamp: health.timestamp,
                    });
                }
                HealthStatus::Healthy => {}
            }
        }

        // Determine overall status
        report.overall_status = if has_critical {
            HealthStatus::Critical
        } else if has_warning {
            HealthStatus::Warning
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        // Calculate summary
        report.summary = HealthSummary {
            total_components: component_health.len() as u32,
            healthy_components: component_health.values()
                .filter(|h| h.status == HealthStatus::Healthy).count() as u32,
            warning_components: component_health.values()
                .filter(|h| h.status == HealthStatus::Warning).count() as u32,
            critical_components: component_health.values()
                .filter(|h| h.status == HealthStatus::Critical).count() as u32,
            degraded_components: component_health.values()
                .filter(|h| h.status == HealthStatus::Degraded).count() as u32,
            unknown_components: component_health.values()
                .filter(|h| h.status == HealthStatus::Unknown).count() as u32,
        };

        report
    }

    /// Subscribe to health alerts from all components
    pub fn subscribe_to_alerts(&self) -> tokio::sync::broadcast::Receiver<mooseng_common::health::HealthAlert> {
        self.health_monitor.subscribe_to_alerts()
    }

    /// Export unified Prometheus metrics
    pub async fn export_prometheus_metrics(&self) -> String {
        let system_health = self.get_system_health().await;
        let mut metrics = String::new();

        // System-wide health metrics
        metrics.push_str("# HELP mooseng_system_health_status Overall system health status (0=unknown, 1=healthy, 2=degraded, 3=warning, 4=critical)\n");
        metrics.push_str("# TYPE mooseng_system_health_status gauge\n");
        let status_value = match system_health.overall_status {
            HealthStatus::Unknown => 0,
            HealthStatus::Healthy => 1,
            HealthStatus::Degraded => 2,
            HealthStatus::Warning => 3,
            HealthStatus::Critical => 4,
        };
        metrics.push_str(&format!("mooseng_system_health_status {}\n", status_value));

        // Component count metrics
        metrics.push_str("# HELP mooseng_system_components_total Total number of components\n");
        metrics.push_str("# TYPE mooseng_system_components_total gauge\n");
        metrics.push_str(&format!("mooseng_system_components_total {}\n", system_health.summary.total_components));

        metrics.push_str("# HELP mooseng_system_healthy_components Number of healthy components\n");
        metrics.push_str("# TYPE mooseng_system_healthy_components gauge\n");
        metrics.push_str(&format!("mooseng_system_healthy_components {}\n", system_health.summary.healthy_components));

        metrics.push_str("# HELP mooseng_system_warning_components Number of components in warning state\n");
        metrics.push_str("# TYPE mooseng_system_warning_components gauge\n");
        metrics.push_str(&format!("mooseng_system_warning_components {}\n", system_health.summary.warning_components));

        metrics.push_str("# HELP mooseng_system_critical_components Number of components in critical state\n");
        metrics.push_str("# TYPE mooseng_system_critical_components gauge\n");
        metrics.push_str(&format!("mooseng_system_critical_components {}\n", system_health.summary.critical_components));

        // Component-specific health status
        metrics.push_str("# HELP mooseng_component_health_status Component health status (0=unknown, 1=healthy, 2=degraded, 3=warning, 4=critical)\n");
        metrics.push_str("# TYPE mooseng_component_health_status gauge\n");
        for (component, health) in &system_health.components {
            let status_value = match health.status {
                HealthStatus::Unknown => 0,
                HealthStatus::Healthy => 1,
                HealthStatus::Degraded => 2,
                HealthStatus::Warning => 3,
                HealthStatus::Critical => 4,
            };
            metrics.push_str(&format!(
                "mooseng_component_health_status{{component=\"{}\"}} {}\n",
                component, status_value
            ));
        }

        metrics
    }

    /// Generate health report for external consumption
    pub async fn generate_health_report(&self) -> Result<String> {
        let system_health = self.get_system_health().await;
        
        let report = serde_json::to_string_pretty(&HealthReportJson {
            overall_status: format!("{:?}", system_health.overall_status).to_lowercase(),
            timestamp: system_health.timestamp.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().as_secs(),
            summary: system_health.summary,
            components: system_health.components.into_iter()
                .map(|(name, health)| ComponentHealthJson {
                    name,
                    status: format!("{:?}", health.status).to_lowercase(),
                    message: health.message,
                    metrics: health.metrics,
                    recommendations: health.recommendations,
                    last_check: health.timestamp.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_secs(),
                })
                .collect(),
            alerts: system_health.alerts.into_iter()
                .map(|alert| AlertJson {
                    component: alert.component,
                    level: format!("{:?}", alert.level).to_lowercase(),
                    message: alert.message,
                    timestamp: alert.timestamp.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_secs(),
                })
                .collect(),
        })?;

        Ok(report)
    }
}

/// Configuration for unified health monitoring
#[derive(Debug, Clone)]
pub struct UnifiedHealthConfig {
    pub check_interval_secs: u64,
    pub check_timeout_secs: u64,
    pub failure_threshold: u32,
    pub recovery_threshold: u32,
    pub enable_self_healing: bool,
    pub max_healing_actions_per_hour: u32,
    pub component_endpoints: HashMap<String, String>,
}

impl Default for UnifiedHealthConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 30,
            check_timeout_secs: 5,
            failure_threshold: 3,
            recovery_threshold: 2,
            enable_self_healing: true,
            max_healing_actions_per_hour: 10,
            component_endpoints: HashMap::new(),
        }
    }
}

/// System-wide health report
#[derive(Debug, Clone)]
pub struct SystemHealthReport {
    pub overall_status: HealthStatus,
    pub components: HashMap<String, HealthCheckResult>,
    pub summary: HealthSummary,
    pub alerts: Vec<HealthAlert>,
    pub timestamp: std::time::SystemTime,
}

/// Health summary statistics
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HealthSummary {
    pub total_components: u32,
    pub healthy_components: u32,
    pub warning_components: u32,
    pub critical_components: u32,
    pub degraded_components: u32,
    pub unknown_components: u32,
}

/// Health alert
#[derive(Debug, Clone)]
pub struct HealthAlert {
    pub component: String,
    pub level: AlertLevel,
    pub message: String,
    pub timestamp: std::time::SystemTime,
}

/// Alert severity levels
#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

// JSON serialization types for API responses
#[derive(serde::Serialize)]
struct HealthReportJson {
    overall_status: String,
    timestamp: u64,
    summary: HealthSummary,
    components: Vec<ComponentHealthJson>,
    alerts: Vec<AlertJson>,
}

#[derive(serde::Serialize)]
struct ComponentHealthJson {
    name: String,
    status: String,
    message: String,
    metrics: HashMap<String, f64>,
    recommendations: Vec<String>,
    last_check: u64,
}

#[derive(serde::Serialize)]
struct AlertJson {
    component: String,
    level: String,
    message: String,
    timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_health_monitor_creation() {
        let config = UnifiedHealthConfig::default();
        let result = UnifiedHealthMonitor::new(config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_health_summary_calculation() {
        let summary = HealthSummary {
            total_components: 4,
            healthy_components: 2,
            warning_components: 1,
            critical_components: 1,
            degraded_components: 0,
            unknown_components: 0,
        };

        assert_eq!(summary.total_components, 4);
        assert_eq!(summary.healthy_components, 2);
        assert_eq!(summary.warning_components, 1);
        assert_eq!(summary.critical_components, 1);
    }
}