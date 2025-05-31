//! Tiered Storage Integration Module
//! 
//! This module provides integration between MooseNG's tiered storage system
//! and the core MooseFS data paths. It bridges the advanced tiered storage
//! capabilities from MooseNG with the traditional MooseFS storage layer.

pub mod policies;
pub mod integration;
pub mod management;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error, instrument};

/// Re-export key types from MooseNG for convenience
pub use crate::core::types::{ChunkId, StorageClassId};

/// Storage tier configuration for MooseFS integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MooseFSTierConfig {
    /// Traditional MooseFS storage paths
    pub moosefs_paths: Vec<PathBuf>,
    /// MooseNG tier configuration
    pub mooseng_config: TierConfiguration,
    /// Integration settings
    pub integration_settings: IntegrationSettings,
}

/// Tier configuration for the integration layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfiguration {
    /// Tier type
    pub tier_type: TierType,
    /// Maximum capacity in bytes
    pub max_capacity: u64,
    /// Performance characteristics
    pub performance_profile: PerformanceProfile,
    /// Cost optimization settings
    pub cost_settings: CostSettings,
}

/// Storage tier types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TierType {
    /// High-performance NVMe SSD storage
    HotSSD,
    /// Standard SSD storage
    WarmSSD,
    /// High-capacity HDD storage
    WarmHDD,
    /// Object storage (S3, etc.)
    Cold,
    /// Archive storage
    Archive,
}

/// Performance characteristics for a tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    /// Expected read latency (milliseconds)
    pub read_latency_ms: u64,
    /// Expected write latency (milliseconds)
    pub write_latency_ms: u64,
    /// Expected IOPS
    pub expected_iops: u32,
    /// Bandwidth capacity (MB/s)
    pub bandwidth_mbps: u32,
}

/// Cost optimization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSettings {
    /// Cost per GB per month
    pub cost_per_gb_month: f64,
    /// Access cost per operation
    pub access_cost_per_op: f64,
    /// Transfer cost per GB
    pub transfer_cost_per_gb: f64,
}

/// Integration settings between MooseFS and MooseNG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationSettings {
    /// Enable automatic tier movement
    pub enable_auto_migration: bool,
    /// Migration check interval
    pub migration_check_interval: Duration,
    /// Minimum file age for migration (seconds)
    pub min_age_for_migration: u64,
    /// Access threshold for hot tier (accesses per day)
    pub hot_tier_access_threshold: f64,
    /// Access threshold for warm tier (accesses per week)
    pub warm_tier_access_threshold: f64,
    /// Enable cost optimization
    pub enable_cost_optimization: bool,
    /// Maximum migration operations per hour
    pub max_migrations_per_hour: u32,
}

impl Default for IntegrationSettings {
    fn default() -> Self {
        Self {
            enable_auto_migration: true,
            migration_check_interval: Duration::from_secs(3600), // 1 hour
            min_age_for_migration: 86400, // 24 hours
            hot_tier_access_threshold: 10.0, // 10 accesses per day
            warm_tier_access_threshold: 2.0, // 2 accesses per week
            enable_cost_optimization: true,
            max_migrations_per_hour: 100,
        }
    }
}

/// Data access pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPattern {
    /// Number of reads in the last 24 hours
    pub reads_24h: u64,
    /// Number of writes in the last 24 hours
    pub writes_24h: u64,
    /// Number of reads in the last 7 days
    pub reads_7d: u64,
    /// Number of writes in the last 7 days
    pub writes_7d: u64,
    /// Last access timestamp
    pub last_access: SystemTime,
    /// File size
    pub file_size: u64,
    /// File age (creation time)
    pub created_at: SystemTime,
}

/// Migration recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecommendation {
    /// Chunk or file ID
    pub chunk_id: ChunkId,
    /// Current tier
    pub current_tier: TierType,
    /// Recommended tier
    pub recommended_tier: TierType,
    /// Reason for recommendation
    pub reason: MigrationReason,
    /// Priority score (higher = more urgent)
    pub priority_score: f64,
    /// Estimated cost savings per month
    pub cost_savings_monthly: f64,
}

/// Reasons for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationReason {
    /// Low access frequency
    InfrequentAccess,
    /// High access frequency
    FrequentAccess,
    /// Age-based policy
    AgeBased,
    /// Cost optimization
    CostOptimization,
    /// Performance optimization
    PerformanceOptimization,
    /// Capacity management
    CapacityManagement,
}

/// Tiered storage manager for MooseFS integration
pub struct TieredStorageManager {
    /// Tier configurations
    tier_configs: Arc<RwLock<HashMap<TierType, MooseFSTierConfig>>>,
    /// Access pattern tracking
    access_patterns: Arc<RwLock<HashMap<ChunkId, AccessPattern>>>,
    /// Integration settings
    settings: Arc<RwLock<IntegrationSettings>>,
    /// Migration queue
    migration_queue: Arc<RwLock<Vec<MigrationRecommendation>>>,
}

impl TieredStorageManager {
    /// Create a new tiered storage manager
    pub fn new() -> Self {
        Self {
            tier_configs: Arc::new(RwLock::new(HashMap::new())),
            access_patterns: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(IntegrationSettings::default())),
            migration_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a tier configuration
    #[instrument(skip(self))]
    pub async fn add_tier_config(&self, tier_type: TierType, config: MooseFSTierConfig) -> Result<()> {
        let mut configs = self.tier_configs.write().await;
        configs.insert(tier_type, config);
        info!("Added tier configuration for {:?}", tier_type);
        Ok(())
    }

    /// Record file access for tier management
    #[instrument(skip(self))]
    pub async fn record_access(&self, chunk_id: ChunkId, access_type: AccessType) -> Result<()> {
        let mut patterns = self.access_patterns.write().await;
        let now = SystemTime::now();
        
        match patterns.get_mut(&chunk_id) {
            Some(pattern) => {
                // Update existing pattern
                match access_type {
                    AccessType::Read => {
                        pattern.reads_24h += 1;
                        pattern.reads_7d += 1;
                    }
                    AccessType::Write => {
                        pattern.writes_24h += 1;
                        pattern.writes_7d += 1;
                    }
                }
                pattern.last_access = now;
            }
            None => {
                // Create new pattern
                let mut new_pattern = AccessPattern {
                    reads_24h: 0,
                    writes_24h: 0,
                    reads_7d: 0,
                    writes_7d: 0,
                    last_access: now,
                    file_size: 0, // Would be filled by caller
                    created_at: now,
                };
                
                match access_type {
                    AccessType::Read => {
                        new_pattern.reads_24h = 1;
                        new_pattern.reads_7d = 1;
                    }
                    AccessType::Write => {
                        new_pattern.writes_24h = 1;
                        new_pattern.writes_7d = 1;
                    }
                }
                
                patterns.insert(chunk_id, new_pattern);
            }
        }

        debug!("Recorded {:?} access for chunk {}", access_type, chunk_id);
        Ok(())
    }

    /// Get tier recommendation for a chunk
    #[instrument(skip(self))]
    pub async fn get_tier_recommendation(&self, chunk_id: ChunkId) -> Result<Option<TierType>> {
        let patterns = self.access_patterns.read().await;
        let settings = self.settings.read().await;
        
        if let Some(pattern) = patterns.get(&chunk_id) {
            let age_hours = pattern.created_at.elapsed()
                .unwrap_or(Duration::from_secs(0))
                .as_secs() / 3600;
            
            let daily_accesses = pattern.reads_24h + pattern.writes_24h;
            let weekly_accesses = (pattern.reads_7d + pattern.writes_7d) as f64 / 7.0;
            
            // Determine optimal tier based on access patterns
            let recommended_tier = if daily_accesses as f64 >= settings.hot_tier_access_threshold {
                TierType::HotSSD
            } else if weekly_accesses >= settings.warm_tier_access_threshold {
                if pattern.file_size > 100 * 1024 * 1024 { // > 100MB
                    TierType::WarmHDD
                } else {
                    TierType::WarmSSD
                }
            } else if age_hours < 24 * 30 { // Less than 30 days
                TierType::Cold
            } else {
                TierType::Archive
            };
            
            debug!("Recommended tier for chunk {}: {:?} (daily: {}, weekly: {:.1}, age: {}h)", 
                   chunk_id, recommended_tier, daily_accesses, weekly_accesses, age_hours);
            
            Ok(Some(recommended_tier))
        } else {
            Ok(None)
        }
    }

    /// Get migration recommendations
    pub async fn get_migration_recommendations(&self) -> Result<Vec<MigrationRecommendation>> {
        let queue = self.migration_queue.read().await;
        Ok(queue.clone())
    }

    /// Process migration queue
    #[instrument(skip(self))]
    pub async fn process_migrations(&self) -> Result<usize> {
        let mut queue = self.migration_queue.write().await;
        let migrations_to_process = queue.len();
        
        // Sort by priority (highest first)
        queue.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap_or(std::cmp::Ordering::Equal));
        
        // Process migrations (simplified for this integration layer)
        for migration in queue.iter() {
            info!("Processing migration for chunk {} from {:?} to {:?}", 
                  migration.chunk_id, migration.current_tier, migration.recommended_tier);
            
            // In a real implementation, this would:
            // 1. Coordinate with MooseFS chunk servers
            // 2. Move data between tiers
            // 3. Update metadata
            // 4. Update MooseNG tiered storage system
        }
        
        queue.clear();
        Ok(migrations_to_process)
    }

    /// Update integration settings
    pub async fn update_settings(&self, settings: IntegrationSettings) -> Result<()> {
        let mut current_settings = self.settings.write().await;
        *current_settings = settings;
        info!("Updated tiered storage integration settings");
        Ok(())
    }

    /// Get current tier utilization statistics
    pub async fn get_tier_stats(&self) -> Result<HashMap<TierType, TierStats>> {
        let configs = self.tier_configs.read().await;
        let mut stats = HashMap::new();
        
        for (tier_type, config) in configs.iter() {
            // In a real implementation, this would collect actual usage statistics
            let tier_stats = TierStats {
                total_capacity: config.mooseng_config.max_capacity,
                used_capacity: 0, // Would be calculated from actual usage
                file_count: 0,    // Would be calculated from actual files
                avg_access_frequency: 0.0, // Would be calculated from access patterns
                cost_per_month: 0.0, // Would be calculated from usage and cost settings
            };
            stats.insert(*tier_type, tier_stats);
        }
        
        Ok(stats)
    }
}

/// Access type for tracking
#[derive(Debug, Clone, Copy)]
pub enum AccessType {
    Read,
    Write,
}

/// Statistics for a storage tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierStats {
    /// Total capacity in bytes
    pub total_capacity: u64,
    /// Used capacity in bytes
    pub used_capacity: u64,
    /// Number of files in this tier
    pub file_count: u64,
    /// Average access frequency (accesses per day)
    pub avg_access_frequency: f64,
    /// Monthly cost for this tier
    pub cost_per_month: f64,
}

/// Default configurations for common tier types
impl MooseFSTierConfig {
    /// Create a hot SSD tier configuration
    pub fn hot_ssd_tier(paths: Vec<PathBuf>, capacity: u64) -> Self {
        Self {
            moosefs_paths: paths,
            mooseng_config: TierConfiguration {
                tier_type: TierType::HotSSD,
                max_capacity: capacity,
                performance_profile: PerformanceProfile {
                    read_latency_ms: 1,
                    write_latency_ms: 2,
                    expected_iops: 50_000,
                    bandwidth_mbps: 3_000,
                },
                cost_settings: CostSettings {
                    cost_per_gb_month: 0.50,
                    access_cost_per_op: 0.0001,
                    transfer_cost_per_gb: 0.01,
                },
            },
            integration_settings: IntegrationSettings::default(),
        }
    }

    /// Create a warm HDD tier configuration
    pub fn warm_hdd_tier(paths: Vec<PathBuf>, capacity: u64) -> Self {
        Self {
            moosefs_paths: paths,
            mooseng_config: TierConfiguration {
                tier_type: TierType::WarmHDD,
                max_capacity: capacity,
                performance_profile: PerformanceProfile {
                    read_latency_ms: 10,
                    write_latency_ms: 15,
                    expected_iops: 200,
                    bandwidth_mbps: 150,
                },
                cost_settings: CostSettings {
                    cost_per_gb_month: 0.10,
                    access_cost_per_op: 0.0001,
                    transfer_cost_per_gb: 0.01,
                },
            },
            integration_settings: IntegrationSettings::default(),
        }
    }

    /// Create a cold object storage tier configuration
    pub fn cold_tier(capacity: u64) -> Self {
        Self {
            moosefs_paths: Vec::new(), // No local paths for object storage
            mooseng_config: TierConfiguration {
                tier_type: TierType::Cold,
                max_capacity: capacity,
                performance_profile: PerformanceProfile {
                    read_latency_ms: 1000,
                    write_latency_ms: 2000,
                    expected_iops: 100,
                    bandwidth_mbps: 100,
                },
                cost_settings: CostSettings {
                    cost_per_gb_month: 0.02,
                    access_cost_per_op: 0.001,
                    transfer_cost_per_gb: 0.09,
                },
            },
            integration_settings: IntegrationSettings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_tier_manager_creation() {
        let manager = TieredStorageManager::new();
        
        let hot_config = MooseFSTierConfig::hot_ssd_tier(
            vec![PathBuf::from("/data/hot")],
            1024 * 1024 * 1024 * 1024 // 1TB
        );
        
        manager.add_tier_config(TierType::HotSSD, hot_config).await.unwrap();
        
        let stats = manager.get_tier_stats().await.unwrap();
        assert!(stats.contains_key(&TierType::HotSSD));
    }

    #[tokio::test]
    async fn test_access_recording() {
        let manager = TieredStorageManager::new();
        
        manager.record_access(123, AccessType::Read).await.unwrap();
        manager.record_access(123, AccessType::Write).await.unwrap();
        
        let recommendation = manager.get_tier_recommendation(123).await.unwrap();
        assert!(recommendation.is_some());
    }

    #[tokio::test]
    async fn test_tier_recommendation() {
        let manager = TieredStorageManager::new();
        
        // Simulate high-access pattern
        for _ in 0..20 {
            manager.record_access(456, AccessType::Read).await.unwrap();
        }
        
        let recommendation = manager.get_tier_recommendation(456).await.unwrap();
        assert_eq!(recommendation, Some(TierType::HotSSD));
    }
}