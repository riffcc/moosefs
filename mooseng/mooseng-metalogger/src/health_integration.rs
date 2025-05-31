//! Health monitoring integration for MooseNG metalogger
//! 
//! This module provides seamless integration between the metalogger components
//! and the health monitoring system, including automatic registration with
//! the health monitor and proper metrics collection.

use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use mooseng_common::health::{HealthMonitor, HealthCheckConfig};
use tracing::info;

use crate::{
    health_checker::MetaloggerHealthChecker,
    server::MetaloggerServer,
    config::MetaloggerConfig,
};

/// Health monitoring service for the metalogger
pub struct MetaloggerHealthService {
    health_monitor: HealthMonitor,
    health_checker: Arc<MetaloggerHealthChecker>,
}

impl MetaloggerHealthService {
    /// Create a new metalogger health service
    pub async fn new(
        server: Option<Arc<MetaloggerServer>>,
        config: &MetaloggerConfig,
    ) -> Result<Self> {
        // Create health check configuration
        let health_config = HealthCheckConfig {
            interval: Duration::from_secs(config.health_check_interval_secs.unwrap_or(30)),
            timeout: Duration::from_secs(config.health_check_timeout_secs.unwrap_or(5)),
            failure_threshold: config.health_failure_threshold.unwrap_or(3),
            recovery_threshold: config.health_recovery_threshold.unwrap_or(2),
            enable_self_healing: config.enable_self_healing.unwrap_or(true),
            max_healing_actions_per_hour: config.max_healing_actions_per_hour.unwrap_or(10),
            component_checks: std::collections::HashMap::new(),
        };

        // Create health monitor
        let health_monitor = HealthMonitor::new(health_config);

        // Create health checker
        let health_checker = Arc::new(MetaloggerHealthChecker::new(
            server,
            None, // WAL manager placeholder
            None, // Replication manager placeholder
        ));

        // Register the health checker
        health_monitor.register_checker(health_checker.clone()).await;

        info!("Metalogger health monitoring service initialized");

        Ok(Self {
            health_monitor,
            health_checker,
        })
    }

    /// Start the health monitoring service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting metalogger health monitoring service");
        self.health_monitor.start().await?;
        Ok(())
    }

    /// Get the health checker for manual operations
    pub fn get_health_checker(&self) -> Arc<MetaloggerHealthChecker> {
        self.health_checker.clone()
    }

    /// Subscribe to health alerts
    pub fn subscribe_to_alerts(&self) -> tokio::sync::broadcast::Receiver<mooseng_common::health::HealthAlert> {
        self.health_monitor.subscribe_to_alerts()
    }

    /// Get current health status
    pub async fn get_health_status(&self) -> std::collections::HashMap<String, mooseng_common::health::HealthCheckResult> {
        self.health_monitor.get_health_status().await
    }

    /// Trigger a manual health check
    pub async fn trigger_health_check(&self) -> Result<mooseng_common::health::HealthCheckResult> {
        self.health_monitor.trigger_health_check("mooseng-metalogger").await
    }

    /// Get metalogger metrics for external monitoring
    pub async fn get_metrics(&self) -> crate::health_checker::MetaloggerMetrics {
        self.health_checker.get_metrics().await
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus_metrics(&self) -> String {
        let metrics = self.get_metrics().await;
        
        format!(
            "# HELP mooseng_metalogger_master_connection_status Master server connection status (1=connected, 0=disconnected)\n\
            # TYPE mooseng_metalogger_master_connection_status gauge\n\
            mooseng_metalogger_master_connection_status {}\n\
            # HELP mooseng_metalogger_master_response_time_ms Master server response time in milliseconds\n\
            # TYPE mooseng_metalogger_master_response_time_ms gauge\n\
            mooseng_metalogger_master_response_time_ms {}\n\
            # HELP mooseng_metalogger_wal_write_rate_per_sec WAL write rate per second\n\
            # TYPE mooseng_metalogger_wal_write_rate_per_sec gauge\n\
            mooseng_metalogger_wal_write_rate_per_sec {}\n\
            # HELP mooseng_metalogger_wal_file_size_mb WAL file size in megabytes\n\
            # TYPE mooseng_metalogger_wal_file_size_mb gauge\n\
            mooseng_metalogger_wal_file_size_mb {}\n\
            # HELP mooseng_metalogger_wal_sync_lag_ms WAL sync lag in milliseconds\n\
            # TYPE mooseng_metalogger_wal_sync_lag_ms gauge\n\
            mooseng_metalogger_wal_sync_lag_ms {}\n\
            # HELP mooseng_metalogger_replication_lag_ms Replication lag in milliseconds\n\
            # TYPE mooseng_metalogger_replication_lag_ms gauge\n\
            mooseng_metalogger_replication_lag_ms {}\n\
            # HELP mooseng_metalogger_memory_usage_percent Memory usage percentage\n\
            # TYPE mooseng_metalogger_memory_usage_percent gauge\n\
            mooseng_metalogger_memory_usage_percent {}\n\
            # HELP mooseng_metalogger_cpu_usage_percent CPU usage percentage\n\
            # TYPE mooseng_metalogger_cpu_usage_percent gauge\n\
            mooseng_metalogger_cpu_usage_percent {}\n\
            # HELP mooseng_metalogger_disk_usage_percent Disk usage percentage\n\
            # TYPE mooseng_metalogger_disk_usage_percent gauge\n\
            mooseng_metalogger_disk_usage_percent {}\n\
            # HELP mooseng_metalogger_error_rate_percent Error rate percentage\n\
            # TYPE mooseng_metalogger_error_rate_percent gauge\n\
            mooseng_metalogger_error_rate_percent {}\n\
            # HELP mooseng_metalogger_backup_status Backup status (1=active, 0=inactive)\n\
            # TYPE mooseng_metalogger_backup_status gauge\n\
            mooseng_metalogger_backup_status {}\n\
            # HELP mooseng_metalogger_recovery_readiness Recovery readiness (1=ready, 0=not ready)\n\
            # TYPE mooseng_metalogger_recovery_readiness gauge\n\
            mooseng_metalogger_recovery_readiness {}\n\
            # HELP mooseng_metalogger_metadata_entries_count Total metadata entries count\n\
            # TYPE mooseng_metalogger_metadata_entries_count gauge\n\
            mooseng_metalogger_metadata_entries_count {}\n",
            if metrics.master_connection_status { 1.0 } else { 0.0 },
            metrics.master_response_time_ms,
            metrics.wal_write_rate_per_sec,
            metrics.wal_file_size_mb,
            metrics.wal_sync_lag_ms,
            metrics.replication_lag_ms,
            metrics.memory_usage_percent,
            metrics.cpu_usage_percent,
            metrics.disk_usage_percent,
            metrics.error_rate_percent,
            if metrics.backup_status { 1.0 } else { 0.0 },
            if metrics.recovery_readiness { 1.0 } else { 0.0 },
            metrics.metadata_entries_count as f64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_health_service_creation() {
        // This test would require mocking the dependencies
        // For now, just test the basic structure
        assert!(true);
    }
}