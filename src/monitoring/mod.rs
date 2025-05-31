//! Monitoring module for MooseNG
//! 
//! This module provides system monitoring capabilities including:
//! - System resource monitoring (CPU, memory, disk I/O)
//! - Data consistency checking
//! - Performance metrics collection
//! - Health status reporting

pub mod system_monitor;
pub mod consistency_checker;
pub mod health_integration;

// Re-export commonly used types
pub use system_monitor::{SystemMonitor, SystemMetrics, SystemMonitorConfig};
pub use consistency_checker::{
    DataConsistencyChecker, ConsistencyReport, ChunkIssue, MetadataIssue, 
    ErasureCodingIssue, ConsistencyConfig
};
pub use health_integration::{
    UnifiedHealthMonitor, UnifiedHealthConfig, SystemHealthReport, HealthSummary, HealthAlert, AlertLevel
};

// For compatibility with health module, re-export DiskSpaceMonitor
pub use crate::health::disk_monitor::{DiskMonitor as DiskSpaceMonitor, DiskMonitorConfig as DiskSpaceMonitorConfig};

/// Initialize monitoring components
pub async fn initialize_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Initializing MooseNG monitoring components");
    
    // Initialize system monitor
    let _system_monitor = SystemMonitor::new(SystemMonitorConfig::default()).await?;
    tracing::info!("System monitor initialized");
    
    // Initialize consistency checker
    let _consistency_checker = DataConsistencyChecker::new(ConsistencyConfig::default());
    tracing::info!("Data consistency checker initialized");
    
    // Initialize unified health monitor
    let _unified_health = UnifiedHealthMonitor::new(UnifiedHealthConfig::default()).await?;
    tracing::info!("Unified health monitor initialized");
    
    tracing::info!("All monitoring components initialized successfully");
    Ok(())
}

/// Initialize comprehensive monitoring with health integration
pub async fn initialize_monitoring_with_health(
    health_config: Option<UnifiedHealthConfig>,
) -> Result<UnifiedHealthMonitor, Box<dyn std::error::Error>> {
    tracing::info!("Initializing comprehensive MooseNG monitoring with health integration");
    
    // Initialize system monitor
    let _system_monitor = SystemMonitor::new(SystemMonitorConfig::default()).await?;
    tracing::info!("System monitor initialized");
    
    // Initialize consistency checker
    let _consistency_checker = DataConsistencyChecker::new(ConsistencyConfig::default());
    tracing::info!("Data consistency checker initialized");
    
    // Initialize unified health monitor
    let mut unified_health = UnifiedHealthMonitor::new(
        health_config.unwrap_or_default()
    ).await?;
    unified_health.start().await?;
    tracing::info!("Unified health monitor initialized and started");
    
    tracing::info!("Comprehensive monitoring system initialized successfully");
    Ok(unified_health)
}