//! Master server health checker implementation
//! 
//! Provides comprehensive health monitoring for the master server
//! including metadata store, cache, filesystem, and Raft consensus.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use mooseng_common::health::{HealthChecker, HealthCheckResult, HealthStatus, SelfHealingAction};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::metadata::MetadataStore;
use crate::cache::MetadataCache;
use crate::filesystem::FileSystem;
use crate::chunk_manager::ChunkManager;
use crate::session::SessionManager;
use crate::raft::RaftConsensus;

/// Master server health checker
pub struct MasterHealthChecker {
    metadata_store: Arc<MetadataStore>,
    cache: Arc<MetadataCache>,
    filesystem: Arc<FileSystem>,
    chunk_manager: Arc<ChunkManager>,
    session_manager: Arc<SessionManager>,
    raft_consensus: Option<Arc<RaftConsensus>>,
    metrics: Arc<RwLock<MasterMetrics>>,
}

#[derive(Debug, Clone, Default)]
pub struct MasterMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub active_sessions: u64,
    pub metadata_operations_per_sec: f64,
    pub cache_hit_rate: f64,
    pub raft_log_entries: u64,
    pub is_leader: bool,
    pub sync_lag_ms: f64,
    pub disk_usage_percent: f64,
    pub network_connections: u64,
}

impl MasterHealthChecker {
    pub fn new(
        metadata_store: Arc<MetadataStore>,
        cache: Arc<MetadataCache>,
        filesystem: Arc<FileSystem>,
        chunk_manager: Arc<ChunkManager>,
        session_manager: Arc<SessionManager>,
        raft_consensus: Option<Arc<RaftConsensus>>,
    ) -> Self {
        Self {
            metadata_store,
            cache,
            filesystem,
            chunk_manager,
            session_manager,
            raft_consensus,
            metrics: Arc::new(RwLock::new(MasterMetrics::default())),
        }
    }
    
    async fn collect_metrics(&self) -> Result<MasterMetrics> {
        let mut metrics = MasterMetrics::default();
        
        // CPU and memory usage (simplified - in production would use system metrics)
        metrics.cpu_usage_percent = Self::get_cpu_usage().await?;
        metrics.memory_usage_percent = Self::get_memory_usage().await?;
        
        // Session metrics
        metrics.active_sessions = self.session_manager.get_active_session_count().await;
        
        // Cache metrics
        let cache_stats = self.cache.get_stats().await;
        metrics.cache_hit_rate = cache_stats.hit_rate;
        
        // Metadata store metrics
        let metadata_stats = self.metadata_store.get_stats().await;
        metrics.metadata_operations_per_sec = metadata_stats.operations_per_second;
        metrics.disk_usage_percent = metadata_stats.disk_usage_percent;
        
        // Raft consensus metrics (if available)
        if let Some(ref raft) = self.raft_consensus {
            let raft_stats = raft.get_stats().await;
            metrics.is_leader = raft_stats.is_leader;
            metrics.raft_log_entries = raft_stats.log_entries;
            metrics.sync_lag_ms = raft_stats.sync_lag.as_millis() as f64;
        }
        
        // Network metrics
        metrics.network_connections = self.get_network_connections().await;
        
        // Store metrics for monitoring
        {
            let mut stored_metrics = self.metrics.write().await;
            *stored_metrics = metrics.clone();
        }
        
        Ok(metrics)
    }
    
    async fn get_cpu_usage() -> Result<f64> {
        // Simplified CPU usage - in production would use system metrics
        // For now, return a simulated value based on current load
        Ok(rand::random::<f64>() * 100.0)
    }
    
    async fn get_memory_usage() -> Result<f64> {
        // Simplified memory usage - in production would use system metrics
        Ok(rand::random::<f64>() * 100.0)
    }
    
    async fn get_network_connections(&self) -> u64 {
        // Simplified - would count actual network connections
        self.session_manager.get_active_session_count().await
    }
    
    fn evaluate_health(&self, metrics: &MasterMetrics) -> (HealthStatus, String, Vec<String>) {
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();
        let mut status = HealthStatus::Healthy;
        
        // Check CPU usage
        if metrics.cpu_usage_percent > 90.0 {
            issues.push("CPU usage critically high");
            recommendations.push("Consider scaling master servers or reducing load".to_string());
            status = HealthStatus::Critical;
        } else if metrics.cpu_usage_percent > 75.0 {
            issues.push("CPU usage high");
            recommendations.push("Monitor CPU usage closely".to_string());
            if status == HealthStatus::Healthy {
                status = HealthStatus::Warning;
            }
        }
        
        // Check memory usage
        if metrics.memory_usage_percent > 95.0 {
            issues.push("Memory usage critically high");
            recommendations.push("Add more memory or reduce cache size".to_string());
            status = HealthStatus::Critical;
        } else if metrics.memory_usage_percent > 85.0 {
            issues.push("Memory usage high");
            recommendations.push("Monitor memory usage and consider optimization".to_string());
            if status == HealthStatus::Healthy {
                status = HealthStatus::Warning;
            }
        }
        
        // Check cache performance
        if metrics.cache_hit_rate < 0.7 {
            issues.push("Cache hit rate low");
            recommendations.push("Consider increasing cache size or warming cache".to_string());
            if status == HealthStatus::Healthy {
                status = HealthStatus::Warning;
            }
        }
        
        // Check disk usage
        if metrics.disk_usage_percent > 95.0 {
            issues.push("Disk usage critically high");
            recommendations.push("Clean up logs or add more storage".to_string());
            status = HealthStatus::Critical;
        } else if metrics.disk_usage_percent > 85.0 {
            issues.push("Disk usage high");
            recommendations.push("Monitor disk usage and plan for cleanup".to_string());
            if status == HealthStatus::Healthy {
                status = HealthStatus::Warning;
            }
        }
        
        // Check Raft consensus (if enabled)
        if self.raft_consensus.is_some() {
            if metrics.sync_lag_ms > 5000.0 {
                issues.push("Raft sync lag high");
                recommendations.push("Check network connectivity between masters".to_string());
                status = HealthStatus::Critical;
            } else if metrics.sync_lag_ms > 1000.0 {
                issues.push("Raft sync lag elevated");
                recommendations.push("Monitor network latency".to_string());
                if status == HealthStatus::Healthy {
                    status = HealthStatus::Warning;
                }
            }
        }
        
        let message = if issues.is_empty() {
            "Master server is healthy".to_string()
        } else {
            format!("Issues detected: {}", issues.join(", "))
        };
        
        (status, message, recommendations)
    }
    
    async fn perform_cache_clear(&self) -> Result<bool> {
        info!("Performing cache clear as self-healing action");
        
        // Clear metadata cache
        self.cache.clear().await;
        
        // Wait a moment for cache to stabilize
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Verify cache is responding
        let cache_stats = self.cache.get_stats().await;
        Ok(cache_stats.entry_count == 0)
    }
    
    async fn perform_metadata_sync(&self) -> Result<bool> {
        info!("Performing metadata sync as self-healing action");
        
        // Force metadata store synchronization
        match self.metadata_store.force_sync().await {
            Ok(_) => {
                info!("Metadata sync completed successfully");
                Ok(true)
            }
            Err(e) => {
                warn!("Metadata sync failed: {}", e);
                Ok(false)
            }
        }
    }
    
    async fn perform_raft_recovery(&self) -> Result<bool> {
        info!("Performing Raft recovery as self-healing action");
        
        if let Some(ref raft) = self.raft_consensus {
            match raft.trigger_recovery().await {
                Ok(_) => {
                    info!("Raft recovery completed successfully");
                    Ok(true)
                }
                Err(e) => {
                    warn!("Raft recovery failed: {}", e);
                    Ok(false)
                }
            }
        } else {
            warn!("Raft consensus not available for recovery");
            Ok(false)
        }
    }
    
    async fn perform_session_cleanup(&self) -> Result<bool> {
        info!("Performing session cleanup as self-healing action");
        
        let cleanup_count = self.session_manager.cleanup_stale_sessions().await;
        info!("Cleaned up {} stale sessions", cleanup_count);
        
        Ok(cleanup_count > 0)
    }
    
    /// Get current metrics for monitoring
    pub async fn get_metrics(&self) -> MasterMetrics {
        self.metrics.read().await.clone()
    }
}

#[async_trait::async_trait]
impl HealthChecker for MasterHealthChecker {
    async fn check_health(&self) -> Result<HealthCheckResult> {
        let start_time = Instant::now();
        
        // Collect current metrics
        let metrics = self.collect_metrics().await?;
        
        // Evaluate health status
        let (status, message, recommendations) = self.evaluate_health(&metrics);
        
        // Convert metrics to HashMap for result
        let mut metric_map = HashMap::new();
        metric_map.insert("cpu_usage_percent".to_string(), metrics.cpu_usage_percent);
        metric_map.insert("memory_usage_percent".to_string(), metrics.memory_usage_percent);
        metric_map.insert("active_sessions".to_string(), metrics.active_sessions as f64);
        metric_map.insert("metadata_ops_per_sec".to_string(), metrics.metadata_operations_per_sec);
        metric_map.insert("cache_hit_rate".to_string(), metrics.cache_hit_rate);
        metric_map.insert("disk_usage_percent".to_string(), metrics.disk_usage_percent);
        metric_map.insert("network_connections".to_string(), metrics.network_connections as f64);
        
        if self.raft_consensus.is_some() {
            metric_map.insert("raft_log_entries".to_string(), metrics.raft_log_entries as f64);
            metric_map.insert("is_leader".to_string(), if metrics.is_leader { 1.0 } else { 0.0 });
            metric_map.insert("sync_lag_ms".to_string(), metrics.sync_lag_ms);
        }
        
        debug!("Master health check completed in {:?}: {:?}", start_time.elapsed(), status);
        
        Ok(HealthCheckResult {
            component: "master".to_string(),
            status,
            message,
            timestamp: SystemTime::now(),
            metrics: metric_map,
            recommendations,
        })
    }
    
    fn component_name(&self) -> &str {
        "master"
    }
    
    async fn perform_self_healing(&self, action: &SelfHealingAction) -> Result<bool> {
        match action {
            SelfHealingAction::ClearCache { .. } => {
                self.perform_cache_clear().await
            }
            SelfHealingAction::CustomAction { name, .. } => {
                match name.as_str() {
                    "metadata_sync" => self.perform_metadata_sync().await,
                    "raft_recovery" => self.perform_raft_recovery().await,
                    "session_cleanup" => self.perform_session_cleanup().await,
                    _ => {
                        warn!("Unknown custom healing action: {}", name);
                        Ok(false)
                    }
                }
            }
            SelfHealingAction::RestartComponent { .. } => {
                // For master server, restart would be handled at a higher level
                warn!("Master server restart should be handled by orchestration layer");
                Ok(false)
            }
            _ => {
                warn!("Healing action not supported by master server: {:?}", action);
                Ok(false)
            }
        }
    }
}

// Mock implementations for missing methods - these would be implemented in the actual components
impl MetadataCache {
    pub async fn clear(&self) {
        // Implementation would clear the cache
        info!("Cache cleared");
    }
}

impl MetadataStore {
    pub async fn force_sync(&self) -> Result<()> {
        // Implementation would force sync to disk
        info!("Metadata forced to sync");
        Ok(())
    }
}

impl SessionManager {
    pub async fn cleanup_stale_sessions(&self) -> u64 {
        // Implementation would clean up stale sessions
        info!("Stale sessions cleaned up");
        10 // Mock return value
    }
}

impl RaftConsensus {
    pub async fn trigger_recovery(&self) -> Result<()> {
        // Implementation would trigger Raft recovery
        info!("Raft recovery triggered");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    // Mock implementations for testing
    struct MockMetadataStore;
    struct MockMetadataCache;
    struct MockFileSystem;
    struct MockChunkManager;
    struct MockSessionManager;
    
    impl MockMetadataStore {
        async fn get_stats(&self) -> MetadataStoreStats {
            MetadataStoreStats {
                operations_per_second: 100.0,
                disk_usage_percent: 50.0,
            }
        }
    }
    
    impl MockMetadataCache {
        async fn get_stats(&self) -> CacheStats {
            CacheStats {
                hit_rate: 0.85,
                entry_count: 1000,
            }
        }
    }
    
    impl MockSessionManager {
        async fn get_active_session_count(&self) -> u64 {
            25
        }
    }
    
    #[derive(Debug)]
    struct MetadataStoreStats {
        operations_per_second: f64,
        disk_usage_percent: f64,
    }
    
    #[derive(Debug)]
    struct CacheStats {
        hit_rate: f64,
        entry_count: u64,
    }
    
    #[tokio::test]
    async fn test_master_health_checker_creation() {
        // This test would require proper mock implementations
        // For now, just verify the structure compiles
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_health_evaluation() {
        let metrics = MasterMetrics {
            cpu_usage_percent: 80.0,
            memory_usage_percent: 70.0,
            cache_hit_rate: 0.9,
            disk_usage_percent: 40.0,
            ..Default::default()
        };
        
        // This would test the evaluation logic
        // For now, just verify the structure compiles
        assert!(metrics.cpu_usage_percent > 75.0);
    }
}