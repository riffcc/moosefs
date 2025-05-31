//! Tiered Storage Integration Module
//! 
//! This module provides the integration layer between MooseFS and MooseNG tiered storage,
//! handling data movement, tier coordination, and performance optimization.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, Instant};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, Mutex};
use tokio::time::interval;
use tracing::{debug, info, warn, error, instrument};

use crate::core::types::{ChunkId, Size, HealthStatus, ComponentHealthResult};
use super::{TieredStorageManager, AccessType, TierType, MigrationRecommendation, AccessPattern};
use super::policies::{PolicyEngine, create_default_policy_engine};

/// Integration coordinator between MooseFS and MooseNG tiered storage
pub struct TieredStorageIntegrator {
    /// Core tiered storage manager
    storage_manager: Arc<TieredStorageManager>,
    /// Policy engine for migration decisions
    policy_engine: Arc<RwLock<PolicyEngine>>,
    /// Integration configuration
    config: Arc<RwLock<IntegrationConfig>>,
    /// Active migrations tracking
    active_migrations: Arc<RwLock<HashMap<String, MigrationTask>>>,
    /// Performance metrics
    metrics: Arc<RwLock<IntegrationMetrics>>,
    /// Background task handles
    background_tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

/// Configuration for the integration layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// Enable automatic tier migrations
    pub auto_migration_enabled: bool,
    /// Migration evaluation interval
    pub evaluation_interval: Duration,
    /// Maximum concurrent migrations
    pub max_concurrent_migrations: u32,
    /// Migration bandwidth limit (bytes per second)
    pub migration_bandwidth_limit: u64,
    /// Enable performance monitoring
    pub performance_monitoring: bool,
    /// Performance monitoring interval
    pub monitoring_interval: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Migration timeout
    pub migration_timeout: Duration,
    /// Enable compression for cold tier migrations
    pub enable_compression: bool,
    /// Chunk size for migrations (bytes)
    pub migration_chunk_size: usize,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            auto_migration_enabled: true,
            evaluation_interval: Duration::from_secs(3600), // 1 hour
            max_concurrent_migrations: 5,
            migration_bandwidth_limit: 100 * 1024 * 1024, // 100 MB/s
            performance_monitoring: true,
            monitoring_interval: Duration::from_secs(300), // 5 minutes
            health_check_interval: Duration::from_secs(30),
            migration_timeout: Duration::from_secs(3600), // 1 hour
            enable_compression: true,
            migration_chunk_size: 1024 * 1024, // 1MB chunks
        }
    }
}

/// Migration task tracking
#[derive(Debug, Clone)]
pub struct MigrationTask {
    pub request_id: String,
    pub chunk_id: ChunkId,
    pub source_tier: TierType,
    pub target_tier: TierType,
    pub started_at: SystemTime,
    pub progress_bytes: u64,
    pub total_bytes: u64,
    pub status: MigrationStatus,
    pub error_message: Option<String>,
}

/// Migration status
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Integration performance metrics
#[derive(Debug, Clone, Default)]
pub struct IntegrationMetrics {
    /// Total migrations completed
    pub migrations_completed: u64,
    /// Total migrations failed
    pub migrations_failed: u64,
    /// Total bytes migrated
    pub bytes_migrated: u64,
    /// Average migration speed (bytes per second)
    pub avg_migration_speed: f64,
    /// Current active migrations
    pub active_migrations_count: u32,
    /// Tier utilization percentages
    pub tier_utilization: HashMap<TierType, f64>,
    /// Last update timestamp
    pub last_updated: SystemTime,
    /// Performance samples for trend analysis
    pub performance_history: Vec<PerformanceSample>,
}

/// Performance sample for trend analysis
#[derive(Debug, Clone)]
pub struct PerformanceSample {
    pub timestamp: SystemTime,
    pub migration_speed_mbps: f64,
    pub active_migrations: u32,
    pub tier_utilization: HashMap<TierType, f64>,
}

impl TieredStorageIntegrator {
    /// Create a new tiered storage integrator
    pub async fn new(config: IntegrationConfig) -> Result<Self> {
        let storage_manager = Arc::new(TieredStorageManager::new());
        let policy_engine = Arc::new(RwLock::new(create_default_policy_engine()));
        
        let integrator = Self {
            storage_manager,
            policy_engine,
            config: Arc::new(RwLock::new(config)),
            active_migrations: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(IntegrationMetrics::default())),
            background_tasks: Arc::new(Mutex::new(Vec::new())),
        };
        
        // Start background tasks
        integrator.start_background_tasks().await?;
        
        Ok(integrator)
    }
    
    /// Start background tasks for migration evaluation and monitoring
    #[instrument(skip(self))]
    async fn start_background_tasks(&self) -> Result<()> {
        let mut tasks = self.background_tasks.lock().await;
        
        // Migration evaluation task
        if self.config.read().await.auto_migration_enabled {
            let eval_task = self.spawn_evaluation_task().await;
            tasks.push(eval_task);
        }
        
        // Performance monitoring task
        if self.config.read().await.performance_monitoring {
            let monitor_task = self.spawn_monitoring_task().await;
            tasks.push(monitor_task);
        }
        
        // Health check task
        let health_task = self.spawn_health_check_task().await;
        tasks.push(health_task);
        
        info!("Started {} background tasks", tasks.len());
        Ok(())
    }
    
    /// Spawn migration evaluation task
    async fn spawn_evaluation_task(&self) -> tokio::task::JoinHandle<()> {
        let storage_manager = Arc::clone(&self.storage_manager);
        let policy_engine = Arc::clone(&self.policy_engine);
        let config = Arc::clone(&self.config);
        let active_migrations = Arc::clone(&self.active_migrations);
        
        tokio::spawn(async move {
            let mut interval = interval(config.read().await.evaluation_interval);
            
            loop {
                interval.tick().await;
                
                let config_guard = config.read().await;
                if !config_guard.auto_migration_enabled {
                    continue;
                }
                
                let max_concurrent = config_guard.max_concurrent_migrations;
                drop(config_guard);
                
                // Check if we can start new migrations
                let active_count = active_migrations.read().await.len() as u32;
                if active_count >= max_concurrent {
                    debug!("Skipping migration evaluation: {} active migrations (max: {})", active_count, max_concurrent);
                    continue;
                }
                
                // Evaluate migrations (simplified for this integration layer)
                if let Err(e) = Self::evaluate_and_queue_migrations(
                    &storage_manager,
                    &policy_engine,
                    &active_migrations,
                    max_concurrent - active_count,
                ).await {
                    error!("Migration evaluation failed: {}", e);
                }
            }
        })
    }
    
    /// Spawn performance monitoring task
    async fn spawn_monitoring_task(&self) -> tokio::task::JoinHandle<()> {
        let metrics = Arc::clone(&self.metrics);
        let config = Arc::clone(&self.config);
        let active_migrations = Arc::clone(&self.active_migrations);
        
        tokio::spawn(async move {
            let mut interval = interval(config.read().await.monitoring_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::update_performance_metrics(&metrics, &active_migrations).await {
                    error!("Performance monitoring failed: {}", e);
                }
            }
        })
    }
    
    /// Spawn health check task
    async fn spawn_health_check_task(&self) -> tokio::task::JoinHandle<()> {
        let config = Arc::clone(&self.config);
        let metrics = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = interval(config.read().await.health_check_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::perform_health_check(&metrics).await {
                    error!("Health check failed: {}", e);
                }
            }
        })
    }
    
    /// Evaluate migrations and queue them
    async fn evaluate_and_queue_migrations(
        storage_manager: &TieredStorageManager,
        policy_engine: &RwLock<PolicyEngine>,
        active_migrations: &RwLock<HashMap<String, MigrationTask>>,
        available_slots: u32,
    ) -> Result<()> {
        if available_slots == 0 {
            return Ok(());
        }
        
        // Get migration recommendations from storage manager
        let recommendations = storage_manager.get_migration_recommendations().await?;
        let engine = policy_engine.read().await;
        
        let mut queued_count = 0;
        for recommendation in recommendations.into_iter().take(available_slots as usize) {
            // Create migration task
            let task = MigrationTask {
                request_id: format!("migration_{}_{}_{}", 
                                   recommendation.chunk_id, 
                                   chrono::Utc::now().timestamp_millis(),
                                   fastrand::u32(1000..9999)),
                chunk_id: recommendation.chunk_id,
                source_tier: recommendation.current_tier,
                target_tier: recommendation.recommended_tier,
                started_at: SystemTime::now(),
                progress_bytes: 0,
                total_bytes: 0, // Would be filled from actual chunk size
                status: MigrationStatus::Queued,
                error_message: None,
            };
            
            // Queue the migration
            active_migrations.write().await.insert(task.request_id.clone(), task);
            queued_count += 1;
            
            info!("Queued migration: {} from {:?} to {:?}", 
                  recommendation.chunk_id, recommendation.current_tier, recommendation.recommended_tier);
        }
        
        if queued_count > 0 {
            info!("Queued {} new migrations", queued_count);
        }
        
        Ok(())
    }
    
    /// Update performance metrics
    async fn update_performance_metrics(
        metrics: &RwLock<IntegrationMetrics>,
        active_migrations: &RwLock<HashMap<String, MigrationTask>>,
    ) -> Result<()> {
        let mut metrics_guard = metrics.write().await;
        let migrations = active_migrations.read().await;
        
        // Update active migrations count
        metrics_guard.active_migrations_count = migrations.len() as u32;
        
        // Calculate average migration speed
        let mut total_speed = 0.0;
        let mut completed_migrations = 0;
        
        for migration in migrations.values() {
            if migration.status == MigrationStatus::InProgress {
                let elapsed = migration.started_at.elapsed().unwrap_or(Duration::from_secs(1));
                let speed = migration.progress_bytes as f64 / elapsed.as_secs_f64();
                total_speed += speed;
                completed_migrations += 1;
            }
        }
        
        if completed_migrations > 0 {
            metrics_guard.avg_migration_speed = total_speed / completed_migrations as f64;
        }
        
        // Create performance sample
        let sample = PerformanceSample {
            timestamp: SystemTime::now(),
            migration_speed_mbps: metrics_guard.avg_migration_speed / (1024.0 * 1024.0),
            active_migrations: metrics_guard.active_migrations_count,
            tier_utilization: metrics_guard.tier_utilization.clone(),
        };
        
        // Keep only last 1000 samples
        metrics_guard.performance_history.push(sample);
        if metrics_guard.performance_history.len() > 1000 {
            metrics_guard.performance_history.remove(0);
        }
        
        metrics_guard.last_updated = SystemTime::now();
        
        debug!("Updated performance metrics: {} active migrations, {:.2} MB/s avg speed",
               metrics_guard.active_migrations_count, 
               metrics_guard.avg_migration_speed / (1024.0 * 1024.0));
        
        Ok(())
    }
    
    /// Perform health check
    async fn perform_health_check(metrics: &RwLock<IntegrationMetrics>) -> Result<()> {
        let metrics_guard = metrics.read().await;
        
        // Check for performance issues
        let avg_speed_mbps = metrics_guard.avg_migration_speed / (1024.0 * 1024.0);
        
        if avg_speed_mbps < 10.0 && metrics_guard.active_migrations_count > 0 {
            warn!("Migration performance is below threshold: {:.2} MB/s", avg_speed_mbps);
        }
        
        // Check tier utilization
        for (tier, utilization) in &metrics_guard.tier_utilization {
            if *utilization > 90.0 {
                warn!("Tier {:?} utilization is high: {:.1}%", tier, utilization);
            }
        }
        
        debug!("Health check completed: {} metrics samples", metrics_guard.performance_history.len());
        
        Ok(())
    }
    
    /// Record file access for tier management
    #[instrument(skip(self))]
    pub async fn record_file_access(&self, chunk_id: ChunkId, access_type: AccessType) -> Result<()> {
        self.storage_manager.record_access(chunk_id, access_type).await
    }
    
    /// Get tier recommendation for a chunk
    #[instrument(skip(self))]
    pub async fn get_tier_recommendation(&self, chunk_id: ChunkId) -> Result<Option<TierType>> {
        self.storage_manager.get_tier_recommendation(chunk_id).await
    }
    
    /// Manually trigger migration evaluation
    #[instrument(skip(self))]
    pub async fn trigger_migration_evaluation(&self) -> Result<u32> {
        let config = self.config.read().await;
        let max_concurrent = config.max_concurrent_migrations;
        drop(config);
        
        let active_count = self.active_migrations.read().await.len() as u32;
        let available_slots = max_concurrent.saturating_sub(active_count);
        
        if available_slots == 0 {
            return Ok(0);
        }
        
        Self::evaluate_and_queue_migrations(
            &self.storage_manager,
            &self.policy_engine,
            &self.active_migrations,
            available_slots,
        ).await?;
        
        let new_active_count = self.active_migrations.read().await.len() as u32;
        Ok(new_active_count - active_count)
    }
    
    /// Get current integration metrics
    pub async fn get_metrics(&self) -> IntegrationMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Get active migrations
    pub async fn get_active_migrations(&self) -> Vec<MigrationTask> {
        self.active_migrations.read().await.values().cloned().collect()
    }
    
    /// Cancel a migration
    #[instrument(skip(self))]
    pub async fn cancel_migration(&self, request_id: &str) -> Result<bool> {
        let mut migrations = self.active_migrations.write().await;
        
        if let Some(migration) = migrations.get_mut(request_id) {
            if migration.status == MigrationStatus::InProgress || migration.status == MigrationStatus::Queued {
                migration.status = MigrationStatus::Cancelled;
                info!("Cancelled migration: {}", request_id);
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Update configuration
    #[instrument(skip(self))]
    pub async fn update_config(&self, new_config: IntegrationConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated integration configuration");
        Ok(())
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> IntegrationConfig {
        self.config.read().await.clone()
    }
    
    /// Get health status
    pub async fn get_health_status(&self) -> ComponentHealthResult {
        let metrics = self.metrics.read().await;
        let config = self.config.read().await;
        
        let mut health_metrics = HashMap::new();
        health_metrics.insert("active_migrations".to_string(), metrics.active_migrations_count as f64);
        health_metrics.insert("avg_migration_speed_mbps".to_string(), metrics.avg_migration_speed / (1024.0 * 1024.0));
        health_metrics.insert("migrations_completed".to_string(), metrics.migrations_completed as f64);
        health_metrics.insert("migrations_failed".to_string(), metrics.migrations_failed as f64);
        
        // Determine health status
        let status = if metrics.migrations_failed > metrics.migrations_completed / 2 {
            HealthStatus::Critical
        } else if metrics.avg_migration_speed / (1024.0 * 1024.0) < 10.0 && metrics.active_migrations_count > 0 {
            HealthStatus::Warning
        } else if !config.auto_migration_enabled {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
        
        let message = match status {
            HealthStatus::Healthy => "Tiered storage integration is operating normally".to_string(),
            HealthStatus::Warning => "Migration performance is below optimal levels".to_string(),
            HealthStatus::Critical => "High migration failure rate detected".to_string(),
            HealthStatus::Degraded => "Automatic migrations are disabled".to_string(),
            HealthStatus::Unknown => "Unable to determine status".to_string(),
        };
        
        let mut recommendations = Vec::new();
        if status == HealthStatus::Warning {
            recommendations.push("Consider reducing concurrent migrations or increasing bandwidth limits".to_string());
        }
        if status == HealthStatus::Critical {
            recommendations.push("Check network connectivity and storage tier health".to_string());
        }
        if status == HealthStatus::Degraded {
            recommendations.push("Enable automatic migrations for optimal performance".to_string());
        }
        
        ComponentHealthResult {
            component: "tiered_storage_integration".to_string(),
            status,
            message,
            metrics: health_metrics,
            timestamp: SystemTime::now(),
            duration_ms: 0, // Would be measured in real implementation
            recommendations,
        }
    }
    
    /// Shutdown the integrator and clean up resources
    #[instrument(skip(self))]
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down tiered storage integrator");
        
        // Cancel all background tasks
        let mut tasks = self.background_tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
        }
        
        // Cancel all active migrations
        let mut migrations = self.active_migrations.write().await;
        for migration in migrations.values_mut() {
            if migration.status == MigrationStatus::InProgress || migration.status == MigrationStatus::Queued {
                migration.status = MigrationStatus::Cancelled;
            }
        }
        
        info!("Tiered storage integrator shutdown complete");
        Ok(())
    }
}

impl Drop for TieredStorageIntegrator {
    fn drop(&mut self) {
        // Attempt graceful shutdown in a blocking context
        // In production, this should be handled more carefully
        debug!("TieredStorageIntegrator dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration as TokioDuration};
    
    #[tokio::test]
    async fn test_integrator_creation() {
        let config = IntegrationConfig::default();
        let integrator = TieredStorageIntegrator::new(config).await.unwrap();
        
        // Check that background tasks are running
        let tasks = integrator.background_tasks.lock().await;
        assert!(!tasks.is_empty());
    }
    
    #[tokio::test]
    async fn test_access_recording() {
        let config = IntegrationConfig::default();
        let integrator = TieredStorageIntegrator::new(config).await.unwrap();
        
        let result = integrator.record_file_access(123, AccessType::Read).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_metrics_collection() {
        let config = IntegrationConfig::default();
        let integrator = TieredStorageIntegrator::new(config).await.unwrap();
        
        // Wait a bit for background tasks to initialize
        tokio::time::sleep(TokioDuration::from_millis(100)).await;
        
        let metrics = integrator.get_metrics().await;
        assert_eq!(metrics.active_migrations_count, 0);
    }
    
    #[tokio::test]
    async fn test_health_status() {
        let config = IntegrationConfig::default();
        let integrator = TieredStorageIntegrator::new(config).await.unwrap();
        
        let health = integrator.get_health_status().await;
        assert_eq!(health.component, "tiered_storage_integration");
        assert!(matches!(health.status, HealthStatus::Healthy));
    }
    
    #[tokio::test]
    async fn test_config_update() {
        let config = IntegrationConfig::default();
        let integrator = TieredStorageIntegrator::new(config).await.unwrap();
        
        let mut new_config = integrator.get_config().await;
        new_config.max_concurrent_migrations = 10;
        
        let result = integrator.update_config(new_config).await;
        assert!(result.is_ok());
        
        let updated_config = integrator.get_config().await;
        assert_eq!(updated_config.max_concurrent_migrations, 10);
    }
    
    #[tokio::test]
    async fn test_migration_evaluation() {
        let config = IntegrationConfig::default();
        let integrator = TieredStorageIntegrator::new(config).await.unwrap();
        
        let queued_count = integrator.trigger_migration_evaluation().await.unwrap();
        // Should be 0 since no data is set up for migration
        assert_eq!(queued_count, 0);
    }
    
    #[tokio::test]
    async fn test_shutdown() {
        let config = IntegrationConfig::default();
        let integrator = TieredStorageIntegrator::new(config).await.unwrap();
        
        let result = integrator.shutdown().await;
        assert!(result.is_ok());
        
        // Verify background tasks are stopped
        let tasks = integrator.background_tasks.lock().await;
        for task in tasks.iter() {
            assert!(task.is_finished());
        }
    }
}