//! MooseFS + MooseNG Integration Library
//! 
//! This library provides comprehensive integration between MooseFS and MooseNG,
//! including health monitoring, tiered storage management, and unified data paths.

pub mod health;
pub mod monitoring;
pub mod tiered_storage;
pub mod core;

// Re-export main functionality
pub use health::{endpoints::HealthResponse, integration::HealthIntegrationManager};
pub use tiered_storage::{TieredStorageManager, TierType, AccessType};
pub use core::{MooseIntegrationManager, IntegrationConfig};

/// Initialize the complete MooseFS + MooseNG integration system
pub async fn initialize_integration() -> Result<MooseIntegrationManager, Box<dyn std::error::Error>> {
    use tracing::info;
    
    // Create default integration configuration
    let config = IntegrationConfig::default();
    
    // Create integration manager
    let manager = MooseIntegrationManager::new(config);
    
    // Start the integration system
    manager.start().await?;
    
    info!("MooseFS + MooseNG integration system initialized successfully");
    Ok(manager)
}

/// Initialize health monitoring only
pub async fn initialize_health_monitoring() -> Result<HealthIntegrationManager, Box<dyn std::error::Error>> {
    use health::integration::{HealthIntegrationManager, HealthIntegrationConfig};
    use tracing::info;
    
    // Create health integration manager
    let config = HealthIntegrationConfig::default();
    let manager = HealthIntegrationManager::new(config);
    
    // Start health monitoring
    manager.start().await?;
    
    info!("Health monitoring system initialized successfully");
    Ok(manager)
}

/// Initialize tiered storage only
pub async fn initialize_tiered_storage() -> Result<TieredStorageManager, Box<dyn std::error::Error>> {
    use tracing::info;
    
    // Create tiered storage manager
    let manager = TieredStorageManager::new();
    
    info!("Tiered storage system initialized successfully");
    Ok(manager)
}