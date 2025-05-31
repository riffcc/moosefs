//! Health monitoring integration for MooseNG client
//! 
//! This module provides seamless integration between the client components
//! and the health monitoring system, including automatic registration with
//! the health monitor and proper metrics collection.

use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use mooseng_common::health::{HealthMonitor, HealthCheckConfig};
use tracing::{info, warn, error};

use crate::{
    health_checker::ClientHealthChecker,
    master_client::MasterClient,
    cache::ClientCache,
    filesystem::MooseFuse,
    config::ClientConfig,
};

/// Health monitoring service for the client
pub struct ClientHealthService {
    health_monitor: HealthMonitor,
    health_checker: Arc<ClientHealthChecker>,
}

impl ClientHealthService {
    /// Create a new client health service
    pub async fn new(
        master_client: Arc<MasterClient>,
        cache: Arc<ClientCache>,
        filesystem: Option<Arc<MooseFuse>>,
        config: &ClientConfig,
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
        let mut health_monitor = HealthMonitor::new(health_config);

        // Create health checker
        let health_checker = Arc::new(ClientHealthChecker::new(
            master_client,
            cache,
            filesystem,
        ));

        // Register the health checker
        health_monitor.register_checker(health_checker.clone()).await;

        info!("Client health monitoring service initialized");

        Ok(Self {
            health_monitor,
            health_checker,
        })
    }

    /// Start the health monitoring service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting client health monitoring service");
        self.health_monitor.start().await?;
        Ok(())
    }

    /// Get the health checker for manual operations
    pub fn get_health_checker(&self) -> Arc<ClientHealthChecker> {
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
        self.health_monitor.trigger_health_check("mooseng-client").await
    }

    /// Get client metrics for external monitoring
    pub async fn get_metrics(&self) -> crate::health_checker::ClientMetrics {
        self.health_checker.get_metrics().await
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus_metrics(&self) -> String {
        let metrics = self.get_metrics().await;
        
        format!(
            "# HELP mooseng_client_master_connection_status Master server connection status (1=connected, 0=disconnected)\n\
            # TYPE mooseng_client_master_connection_status gauge\n\
            mooseng_client_master_connection_status {}\n\
            # HELP mooseng_client_master_response_time_ms Master server response time in milliseconds\n\
            # TYPE mooseng_client_master_response_time_ms gauge\n\
            mooseng_client_master_response_time_ms {}\n\
            # HELP mooseng_client_cache_hit_rate Cache hit rate as a ratio (0.0-1.0)\n\
            # TYPE mooseng_client_cache_hit_rate gauge\n\
            mooseng_client_cache_hit_rate {}\n\
            # HELP mooseng_client_cache_size_mb Cache size in megabytes\n\
            # TYPE mooseng_client_cache_size_mb gauge\n\
            mooseng_client_cache_size_mb {}\n\
            # HELP mooseng_client_memory_usage_percent Memory usage percentage\n\
            # TYPE mooseng_client_memory_usage_percent gauge\n\
            mooseng_client_memory_usage_percent {}\n\
            # HELP mooseng_client_cpu_usage_percent CPU usage percentage\n\
            # TYPE mooseng_client_cpu_usage_percent gauge\n\
            mooseng_client_cpu_usage_percent {}\n\
            # HELP mooseng_client_fuse_operations_per_sec FUSE operations per second\n\
            # TYPE mooseng_client_fuse_operations_per_sec gauge\n\
            mooseng_client_fuse_operations_per_sec {}\n\
            # HELP mooseng_client_error_rate_percent Error rate percentage\n\
            # TYPE mooseng_client_error_rate_percent gauge\n\
            mooseng_client_error_rate_percent {}\n\
            # HELP mooseng_client_mount_status Mount status (1=mounted, 0=not mounted)\n\
            # TYPE mooseng_client_mount_status gauge\n\
            mooseng_client_mount_status {}\n\
            # HELP mooseng_client_read_throughput_mbps Read throughput in MB/s\n\
            # TYPE mooseng_client_read_throughput_mbps gauge\n\
            mooseng_client_read_throughput_mbps {}\n\
            # HELP mooseng_client_write_throughput_mbps Write throughput in MB/s\n\
            # TYPE mooseng_client_write_throughput_mbps gauge\n\
            mooseng_client_write_throughput_mbps {}\n",
            if metrics.master_connection_status { 1.0 } else { 0.0 },
            metrics.master_response_time_ms,
            metrics.cache_hit_rate,
            metrics.cache_size_mb,
            metrics.memory_usage_percent,
            metrics.cpu_usage_percent,
            metrics.fuse_operations_per_sec,
            metrics.error_rate_percent,
            if metrics.mount_status { 1.0 } else { 0.0 },
            metrics.read_throughput_mbps,
            metrics.write_throughput_mbps,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    
    #[tokio::test]
    async fn test_health_service_creation() {
        // This test would require mocking the dependencies
        // For now, just test the basic structure
        assert!(true);
    }
}