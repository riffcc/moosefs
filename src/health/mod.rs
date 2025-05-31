//! Comprehensive health monitoring for MooseNG
//! 
//! This module provides a complete health monitoring solution including:
//! - Disk space utilization monitoring
//! - Server load and resource tracking  
//! - Data consistency verification across replicas
//! - HTTP endpoints for health checks
//! - Prometheus metrics integration
//!
//! The health monitoring system is designed to:
//! - Provide real-time health status
//! - Generate actionable alerts and recommendations
//! - Integrate with monitoring tools like Prometheus and Grafana
//! - Support automated self-healing actions

pub mod disk_monitor;
pub mod load_monitor;
pub mod consistency_monitor;
pub mod endpoints;

// Re-export main types for convenience
pub use disk_monitor::{DiskMonitor, DiskMonitorConfig, DiskSpaceInfo};
pub use load_monitor::{LoadMonitor, LoadMonitorConfig, ServerLoadInfo, CpuInfo, MemoryInfo};
pub use consistency_monitor::{ConsistencyMonitor, ConsistencyMonitorConfig, ReplicationConsistencyReport, ChunkConsistencyInfo};
pub use endpoints::{HealthService, HealthResponse, HealthCheck, HealthStatus};

use std::sync::Arc;
use anyhow::Result;

/// Initialize the complete health monitoring system
pub async fn initialize_health_monitoring(
    bind_addr: &str,
    system_config: Option<SystemMonitorConfig>,
    disk_config: Option<DiskMonitorConfig>,
    consistency_config: Option<ConsistencyConfig>,
) -> Result<()> {
    // Create monitoring components
    let system_monitor = Arc::new(SystemMonitor::new(
        system_config.unwrap_or_default()
    )?);
    
    let disk_monitor = Arc::new(DiskSpaceMonitor::new(
        disk_config.unwrap_or_default()
    ));
    
    let consistency_checker = Arc::new(DataConsistencyChecker::new(
        consistency_config.unwrap_or_default()
    ));
    
    // Create server state
    let state = HealthServerState {
        system_monitor,
        disk_monitor,
        consistency_checker,
    };
    
    // Start the health check server
    start_health_server(bind_addr, state).await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_monitoring_components() {
        // Test that all components can be created
        let system_monitor = SystemMonitor::new(SystemMonitorConfig::default()).unwrap();
        let disk_monitor = DiskSpaceMonitor::new(DiskMonitorConfig::default());
        let consistency_checker = DataConsistencyChecker::new(ConsistencyConfig::default());
        
        // Basic functionality test
        let _system_result = system_monitor.check_system_load(std::time::Duration::from_secs(60)).await;
        let _disk_result = disk_monitor.check_disk_space(None).await;
        let _consistency_result = consistency_checker.check_data_consistency(None, None, false).await;
        
        // All components should be creatable without error
    }
}