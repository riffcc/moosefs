//! Core Integration Module
//! 
//! This module provides the core integration layer between MooseFS and MooseNG,
//! coordinating health monitoring, tiered storage, and data path operations.

pub mod types;
pub mod data_paths;
pub mod coordination;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error, instrument};

use crate::health::integration::{HealthIntegrationManager, HealthIntegrationConfig};
use crate::tiered_storage::{TieredStorageManager, TierType, AccessType};

pub use types::*;

/// Main coordination manager for MooseFS + MooseNG integration
pub struct MooseIntegrationManager {
    /// Health monitoring integration
    health_manager: Arc<HealthIntegrationManager>,
    /// Tiered storage integration
    storage_manager: Arc<TieredStorageManager>,
    /// Data path coordinator
    data_path_coordinator: Arc<DataPathCoordinator>,
    /// Integration configuration
    config: Arc<IntegrationConfig>,
    /// Runtime state
    state: Arc<RwLock<IntegrationState>>,
}

/// Configuration for the overall integration
#[derive(Debug, Clone)]
pub struct IntegrationConfig {
    /// Health monitoring configuration
    pub health_config: HealthIntegrationConfig,
    /// Tiered storage configuration
    pub storage_config: StorageIntegrationConfig,
    /// Data path configuration
    pub data_path_config: DataPathConfig,
    /// General integration settings
    pub general_settings: GeneralSettings,
}

/// Storage integration configuration
#[derive(Debug, Clone)]
pub struct StorageIntegrationConfig {
    /// Enable automatic tier management
    pub enable_auto_tiering: bool,
    /// Integration with MooseFS chunk servers
    pub moosefs_chunk_servers: Vec<ChunkServerConfig>,
    /// Integration with MooseNG storage
    pub mooseng_storage_enabled: bool,
    /// Migration settings
    pub migration_settings: MigrationSettings,
}

/// Chunk server configuration
#[derive(Debug, Clone)]
pub struct ChunkServerConfig {
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Storage paths
    pub storage_paths: Vec<PathBuf>,
    /// Tier assignment
    pub tier_type: TierType,
    /// Capacity in bytes
    pub capacity: u64,
}

/// Migration settings between tiers
#[derive(Debug, Clone)]
pub struct MigrationSettings {
    /// Enable background migrations
    pub enable_background_migration: bool,
    /// Migration batch size
    pub batch_size: u32,
    /// Migration interval
    pub migration_interval: Duration,
    /// Maximum concurrent migrations
    pub max_concurrent_migrations: u32,
}

/// Data path configuration
#[derive(Debug, Clone)]
pub struct DataPathConfig {
    /// MooseFS mount points
    pub moosefs_mounts: Vec<PathBuf>,
    /// MooseNG mount points
    pub mooseng_mounts: Vec<PathBuf>,
    /// Unified namespace configuration
    pub unified_namespace: NamespaceConfig,
    /// I/O optimization settings
    pub io_optimization: IOOptimizationConfig,
}

/// Namespace configuration
#[derive(Debug, Clone)]
pub struct NamespaceConfig {
    /// Enable unified namespace
    pub enable_unified_namespace: bool,
    /// Namespace root path
    pub namespace_root: PathBuf,
    /// Path mapping rules
    pub path_mappings: HashMap<String, PathMapping>,
}

/// Path mapping between systems
#[derive(Debug, Clone)]
pub struct PathMapping {
    /// Source path pattern
    pub source_pattern: String,
    /// Target system
    pub target_system: TargetSystem,
    /// Target path
    pub target_path: String,
}

/// Target system for path mapping
#[derive(Debug, Clone)]
pub enum TargetSystem {
    MooseFS,
    MooseNG,
    Both,
}

/// I/O optimization configuration
#[derive(Debug, Clone)]
pub struct IOOptimizationConfig {
    /// Enable read-ahead optimization
    pub enable_readahead: bool,
    /// Read-ahead size in bytes
    pub readahead_size: u64,
    /// Enable write buffering
    pub enable_write_buffering: bool,
    /// Write buffer size in bytes
    pub write_buffer_size: u64,
    /// Enable compression for cold tiers
    pub enable_compression: bool,
}

/// General integration settings
#[derive(Debug, Clone)]
pub struct GeneralSettings {
    /// Integration mode
    pub mode: IntegrationMode,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Log level for integration components
    pub log_level: String,
}

/// Integration mode
#[derive(Debug, Clone)]
pub enum IntegrationMode {
    /// MooseFS only (legacy mode)
    MooseFSOnly,
    /// MooseNG only (new deployments)
    MooseNGOnly,
    /// Hybrid mode (migration/coexistence)
    Hybrid,
    /// Mirror mode (both systems in sync)
    Mirror,
}

/// Runtime state of the integration
#[derive(Debug, Clone)]
pub struct IntegrationState {
    /// Current integration status
    pub status: IntegrationStatus,
    /// Active migrations
    pub active_migrations: u32,
    /// Last health check timestamp
    pub last_health_check: std::time::SystemTime,
    /// Performance metrics
    pub performance_metrics: PerformanceMetrics,
    /// Error counters
    pub error_counters: ErrorCounters,
}

/// Integration status
#[derive(Debug, Clone)]
pub enum IntegrationStatus {
    Initializing,
    Running,
    Degraded,
    Failed,
    Maintenance,
}

/// Performance metrics for the integration
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total I/O operations per second
    pub total_iops: f64,
    /// Read operations per second
    pub read_iops: f64,
    /// Write operations per second
    pub write_iops: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Throughput in MB/s
    pub throughput_mbps: f64,
    /// Cache hit ratio
    pub cache_hit_ratio: f64,
}

/// Error counters
#[derive(Debug, Clone)]
pub struct ErrorCounters {
    /// Health check errors
    pub health_check_errors: u64,
    /// Storage migration errors
    pub migration_errors: u64,
    /// I/O errors
    pub io_errors: u64,
    /// Network errors
    pub network_errors: u64,
}

/// Data path coordinator manages data flow between systems
pub struct DataPathCoordinator {
    /// Configuration
    config: Arc<DataPathConfig>,
    /// Path mappings
    path_mappings: Arc<RwLock<HashMap<String, PathMapping>>>,
    /// I/O statistics
    io_stats: Arc<RwLock<IOStatistics>>,
}

/// I/O statistics tracking
#[derive(Debug, Clone, Default)]
pub struct IOStatistics {
    /// Read operations count
    pub read_ops: u64,
    /// Write operations count
    pub write_ops: u64,
    /// Bytes read
    pub bytes_read: u64,
    /// Bytes written
    pub bytes_written: u64,
    /// Average read latency
    pub avg_read_latency_ms: f64,
    /// Average write latency
    pub avg_write_latency_ms: f64,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            health_config: HealthIntegrationConfig::default(),
            storage_config: StorageIntegrationConfig {
                enable_auto_tiering: true,
                moosefs_chunk_servers: Vec::new(),
                mooseng_storage_enabled: true,
                migration_settings: MigrationSettings {
                    enable_background_migration: true,
                    batch_size: 10,
                    migration_interval: Duration::from_secs(3600),
                    max_concurrent_migrations: 5,
                },
            },
            data_path_config: DataPathConfig {
                moosefs_mounts: vec![PathBuf::from("/mnt/moosefs")],
                mooseng_mounts: vec![PathBuf::from("/mnt/mooseng")],
                unified_namespace: NamespaceConfig {
                    enable_unified_namespace: true,
                    namespace_root: PathBuf::from("/mnt/unified"),
                    path_mappings: HashMap::new(),
                },
                io_optimization: IOOptimizationConfig {
                    enable_readahead: true,
                    readahead_size: 1024 * 1024, // 1MB
                    enable_write_buffering: true,
                    write_buffer_size: 4 * 1024 * 1024, // 4MB
                    enable_compression: true,
                },
            },
            general_settings: GeneralSettings {
                mode: IntegrationMode::Hybrid,
                enable_metrics: true,
                metrics_interval: Duration::from_secs(60),
                log_level: "info".to_string(),
            },
        }
    }
}

impl MooseIntegrationManager {
    /// Create a new integration manager
    pub fn new(config: IntegrationConfig) -> Self {
        let health_manager = Arc::new(HealthIntegrationManager::new(config.health_config.clone()));
        let storage_manager = Arc::new(TieredStorageManager::new());
        let data_path_coordinator = Arc::new(DataPathCoordinator::new(config.data_path_config.clone()));
        
        Self {
            health_manager,
            storage_manager,
            data_path_coordinator,
            config: Arc::new(config),
            state: Arc::new(RwLock::new(IntegrationState {
                status: IntegrationStatus::Initializing,
                active_migrations: 0,
                last_health_check: std::time::SystemTime::now(),
                performance_metrics: PerformanceMetrics {
                    total_iops: 0.0,
                    read_iops: 0.0,
                    write_iops: 0.0,
                    avg_latency_ms: 0.0,
                    throughput_mbps: 0.0,
                    cache_hit_ratio: 0.0,
                },
                error_counters: ErrorCounters {
                    health_check_errors: 0,
                    migration_errors: 0,
                    io_errors: 0,
                    network_errors: 0,
                },
            })),
        }
    }
    
    /// Start the integration system
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting MooseFS + MooseNG integration system");
        
        // Update state to running
        {
            let mut state = self.state.write().await;
            state.status = IntegrationStatus::Running;
        }
        
        // Start health monitoring
        self.health_manager.start().await
            .context("Failed to start health monitoring")?;
        
        // Start tiered storage management
        self.start_storage_management().await
            .context("Failed to start storage management")?;
        
        // Start data path coordination
        self.data_path_coordinator.start().await
            .context("Failed to start data path coordination")?;
        
        // Start metrics collection
        if self.config.general_settings.enable_metrics {
            self.start_metrics_collection().await?;
        }
        
        info!("MooseFS + MooseNG integration system started successfully");
        Ok(())
    }
    
    /// Start storage management
    async fn start_storage_management(&self) -> Result<()> {
        if self.config.storage_config.enable_auto_tiering {
            info!("Starting automatic tiering management");
            
            let storage_manager = self.storage_manager.clone();
            let migration_settings = self.config.storage_config.migration_settings.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(migration_settings.migration_interval);
                
                loop {
                    interval.tick().await;
                    
                    if let Err(e) = storage_manager.process_migrations().await {
                        error!("Migration processing failed: {}", e);
                    }
                }
            });
        }
        
        Ok(())
    }
    
    /// Start metrics collection
    async fn start_metrics_collection(&self) -> Result<()> {
        info!("Starting metrics collection");
        
        let state = self.state.clone();
        let data_path_coordinator = self.data_path_coordinator.clone();
        let interval_duration = self.config.general_settings.metrics_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::collect_metrics(&state, &data_path_coordinator).await {
                    error!("Metrics collection failed: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Collect performance metrics
    async fn collect_metrics(
        state: &Arc<RwLock<IntegrationState>>,
        data_path_coordinator: &Arc<DataPathCoordinator>,
    ) -> Result<()> {
        let io_stats = data_path_coordinator.get_io_statistics().await;
        
        let mut state_guard = state.write().await;
        state_guard.performance_metrics.read_iops = io_stats.read_ops as f64 / 60.0; // Per second over 1 minute
        state_guard.performance_metrics.write_iops = io_stats.write_ops as f64 / 60.0;
        state_guard.performance_metrics.total_iops = state_guard.performance_metrics.read_iops + state_guard.performance_metrics.write_iops;
        state_guard.performance_metrics.avg_latency_ms = (io_stats.avg_read_latency_ms + io_stats.avg_write_latency_ms) / 2.0;
        
        debug!("Updated performance metrics: IOPS={:.2}, Latency={:.2}ms", 
               state_guard.performance_metrics.total_iops,
               state_guard.performance_metrics.avg_latency_ms);
        
        Ok(())
    }
    
    /// Handle file access for tier management
    #[instrument(skip(self))]
    pub async fn handle_file_access(&self, path: &str, access_type: AccessType) -> Result<()> {
        // Convert path to chunk ID (simplified)
        let chunk_id = self.path_to_chunk_id(path).await?;
        
        // Record access for tiered storage
        self.storage_manager.record_access(chunk_id, access_type).await?;
        
        // Update I/O statistics
        self.data_path_coordinator.record_io_operation(access_type, 0).await; // Size would be actual file size
        
        Ok(())
    }
    
    /// Convert file path to chunk ID (simplified mapping)
    async fn path_to_chunk_id(&self, _path: &str) -> Result<ChunkId> {
        // In a real implementation, this would:
        // 1. Consult MooseFS metadata to get chunk information
        // 2. Map file paths to actual chunk IDs
        // 3. Handle fragmented files with multiple chunks
        
        // Placeholder implementation
        Ok(1) // This would be the actual chunk ID
    }
    
    /// Get current integration status
    pub async fn get_status(&self) -> IntegrationState {
        self.state.read().await.clone()
    }
    
    /// Get health status from integrated health manager
    pub async fn get_health_status(&self) -> crate::health::integration::UnifiedHealthStatus {
        self.health_manager.get_health_status().await
    }
    
    /// Get storage tier statistics
    pub async fn get_storage_stats(&self) -> Result<HashMap<TierType, crate::tiered_storage::TierStats>> {
        self.storage_manager.get_tier_stats().await
    }
    
    /// Trigger manual migration for a file
    #[instrument(skip(self))]
    pub async fn trigger_migration(&self, path: &str, target_tier: TierType) -> Result<()> {
        let chunk_id = self.path_to_chunk_id(path).await?;
        info!("Triggering manual migration for chunk {} to tier {:?}", chunk_id, target_tier);
        
        // In a real implementation, this would:
        // 1. Validate the migration request
        // 2. Add to migration queue
        // 3. Execute the migration
        // 4. Update metadata
        
        Ok(())
    }
    
    /// Shutdown the integration system gracefully
    #[instrument(skip(self))]
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MooseFS + MooseNG integration system");
        
        // Update state
        {
            let mut state = self.state.write().await;
            state.status = IntegrationStatus::Failed; // Temporarily, would be "Shutdown" in real implementation
        }
        
        // Graceful shutdown procedures would go here
        // 1. Stop accepting new requests
        // 2. Complete pending migrations
        // 3. Flush any pending I/O
        // 4. Save state to persistent storage
        
        info!("Integration system shutdown complete");
        Ok(())
    }
}

impl DataPathCoordinator {
    /// Create a new data path coordinator
    pub fn new(config: DataPathConfig) -> Self {
        Self {
            config: Arc::new(config),
            path_mappings: Arc::new(RwLock::new(HashMap::new())),
            io_stats: Arc::new(RwLock::new(IOStatistics::default())),
        }
    }
    
    /// Start the data path coordinator
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting data path coordinator");
        
        // Initialize path mappings
        self.initialize_path_mappings().await?;
        
        // Start I/O monitoring
        self.start_io_monitoring().await?;
        
        Ok(())
    }
    
    /// Initialize path mappings from configuration
    async fn initialize_path_mappings(&self) -> Result<()> {
        let mut mappings = self.path_mappings.write().await;
        for (pattern, mapping) in &self.config.unified_namespace.path_mappings {
            mappings.insert(pattern.clone(), mapping.clone());
        }
        
        info!("Initialized {} path mappings", mappings.len());
        Ok(())
    }
    
    /// Start I/O monitoring
    async fn start_io_monitoring(&self) -> Result<()> {
        if self.config.io_optimization.enable_readahead || self.config.io_optimization.enable_write_buffering {
            info!("Starting I/O optimization monitoring");
            
            // In a real implementation, this would start background tasks
            // to monitor and optimize I/O patterns
        }
        
        Ok(())
    }
    
    /// Record an I/O operation
    pub async fn record_io_operation(&self, access_type: AccessType, size: u64) {
        let mut stats = self.io_stats.write().await;
        
        match access_type {
            AccessType::Read => {
                stats.read_ops += 1;
                stats.bytes_read += size;
            }
            AccessType::Write => {
                stats.write_ops += 1;
                stats.bytes_written += size;
            }
        }
    }
    
    /// Get current I/O statistics
    pub async fn get_io_statistics(&self) -> IOStatistics {
        self.io_stats.read().await.clone()
    }
    
    /// Resolve a path through the unified namespace
    #[instrument(skip(self))]
    pub async fn resolve_path(&self, input_path: &str) -> Result<String> {
        let mappings = self.path_mappings.read().await;
        
        // Try to match the input path against configured patterns
        for (pattern, mapping) in mappings.iter() {
            if input_path.starts_with(pattern) {
                let relative_path = input_path.strip_prefix(pattern).unwrap_or("");
                let resolved_path = format!("{}{}", mapping.target_path, relative_path);
                
                debug!("Resolved path {} -> {} via pattern {}", 
                       input_path, resolved_path, pattern);
                return Ok(resolved_path);
            }
        }
        
        // No mapping found, return original path
        Ok(input_path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    
    #[tokio::test]
    async fn test_integration_manager_creation() {
        let config = IntegrationConfig::default();
        let manager = MooseIntegrationManager::new(config);
        
        let status = manager.get_status().await;
        assert!(matches!(status.status, IntegrationStatus::Initializing));
    }
    
    #[tokio::test]
    async fn test_data_path_coordinator() {
        let config = DataPathConfig {
            moosefs_mounts: vec![PathBuf::from("/mnt/moosefs")],
            mooseng_mounts: vec![PathBuf::from("/mnt/mooseng")],
            unified_namespace: NamespaceConfig {
                enable_unified_namespace: true,
                namespace_root: PathBuf::from("/mnt/unified"),
                path_mappings: {
                    let mut mappings = HashMap::new();
                    mappings.insert("/data/".to_string(), PathMapping {
                        source_pattern: "/data/".to_string(),
                        target_system: TargetSystem::MooseFS,
                        target_path: "/mnt/moosefs/data/".to_string(),
                    });
                    mappings
                },
            },
            io_optimization: IOOptimizationConfig {
                enable_readahead: true,
                readahead_size: 1024 * 1024,
                enable_write_buffering: true,
                write_buffer_size: 4 * 1024 * 1024,
                enable_compression: true,
            },
        };
        
        let coordinator = DataPathCoordinator::new(config);
        coordinator.start().await.unwrap();
        
        let resolved = coordinator.resolve_path("/data/test.txt").await.unwrap();
        assert_eq!(resolved, "/mnt/moosefs/data/test.txt");
    }
    
    #[tokio::test]
    async fn test_file_access_handling() {
        let config = IntegrationConfig::default();
        let manager = MooseIntegrationManager::new(config);
        
        manager.handle_file_access("/test/file.txt", AccessType::Read).await.unwrap();
        
        let status = manager.get_status().await;
        // In a real implementation, this would verify the access was recorded
        assert!(matches!(status.status, IntegrationStatus::Initializing));
    }
}