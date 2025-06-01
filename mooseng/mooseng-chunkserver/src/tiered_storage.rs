use anyhow::{Result, Context};
use mooseng_common::types::{ChunkId, StorageClassId, now_micros};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, instrument};
use crate::erasure::ErasureConfig;

/// Storage tier types with different performance characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageTier {
    /// High-performance SSD storage for frequently accessed data
    Hot,
    /// Standard HDD storage for occasionally accessed data
    Warm,
    /// Object storage for infrequently accessed data
    Cold,
    /// Archive storage for long-term retention
    Archive,
}

impl StorageTier {
    /// Get the priority order of tiers (higher number = faster tier)
    pub fn priority(&self) -> u8 {
        match self {
            StorageTier::Hot => 3,
            StorageTier::Warm => 2,
            StorageTier::Cold => 1,
            StorageTier::Archive => 0,
        }
    }
    
    /// Check if this tier is faster than another
    pub fn is_faster_than(&self, other: &StorageTier) -> bool {
        self.priority() > other.priority()
    }
    
    /// Get expected latency characteristics for this tier
    pub fn expected_latency(&self) -> Duration {
        match self {
            StorageTier::Hot => Duration::from_millis(1),     // ~1ms for SSD
            StorageTier::Warm => Duration::from_millis(10),   // ~10ms for HDD
            StorageTier::Cold => Duration::from_secs(1),      // ~1s for object storage
            StorageTier::Archive => Duration::from_secs(5),   // ~5s for archive retrieval
        }
    }
    
    /// Get expected IOPS for this tier
    pub fn expected_iops(&self) -> u32 {
        match self {
            StorageTier::Hot => 50_000,    // High-end NVMe SSD
            StorageTier::Warm => 200,      // Standard HDD
            StorageTier::Cold => 100,      // Object storage
            StorageTier::Archive => 10,    // Archive storage
        }
    }
}

/// Configuration for a specific storage tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    /// The tier type
    pub tier: StorageTier,
    /// Base path for local storage (Hot/Warm tiers)
    pub base_path: Option<PathBuf>,
    /// Maximum capacity in bytes for this tier
    pub max_capacity: u64,
    /// Current used capacity in bytes
    pub used_capacity: u64,
    /// Cost per GB per month (for cost optimization)
    pub cost_per_gb_month: f64,
    /// Object storage configuration (Cold/Archive tiers)
    pub object_storage: Option<ObjectStorageConfig>,
    /// Erasure coding configuration for this tier
    pub erasure_config: Option<ErasureConfig>,
    /// Performance monitoring thresholds
    pub performance_thresholds: PerformanceThresholds,
}

/// Object storage configuration for cold and archive tiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectStorageConfig {
    /// Storage provider (S3, Azure, GCS, etc.)
    pub provider: String,
    /// Bucket or container name
    pub bucket: String,
    /// Region or location
    pub region: String,
    /// Access credentials configuration
    pub credentials: CredentialsConfig,
    /// Lifecycle management settings
    pub lifecycle: LifecycleConfig,
    /// Caching settings for frequently accessed objects
    pub cache: CacheConfig,
}

/// Credentials configuration for object storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialsConfig {
    /// Access key ID
    pub access_key_id: String,
    /// Secret access key
    pub secret_access_key: String,
    /// Optional session token
    pub session_token: Option<String>,
    /// Optional role ARN for assume role
    pub role_arn: Option<String>,
}

/// Lifecycle management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Days after which to transition to next tier
    pub transition_days: u32,
    /// Days after which to delete (0 = never delete)
    pub delete_days: u32,
    /// Enable versioning
    pub versioning: bool,
}

/// Cache configuration for object storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable local caching
    pub enabled: bool,
    /// Maximum cache size in bytes
    pub max_size: u64,
    /// Cache TTL in seconds
    pub ttl_seconds: u64,
    /// Local cache directory
    pub cache_dir: PathBuf,
}

// Note: ErasureConfig is imported from crate::erasure module
// This avoids duplication and naming conflicts

/// Performance monitoring thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable latency (ms)
    pub max_latency_ms: u64,
    /// Minimum acceptable IOPS
    pub min_iops: u32,
    /// Maximum acceptable error rate (0.0-1.0)
    pub max_error_rate: f64,
    /// Capacity threshold for warnings (0.0-1.0)
    pub capacity_warning_threshold: f64,
    /// Capacity threshold for alerts (0.0-1.0)
    pub capacity_alert_threshold: f64,
}

/// Metadata tracking for chunks in tiered storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkTierMetadata {
    /// The chunk ID
    pub chunk_id: ChunkId,
    /// Current storage tier
    pub current_tier: StorageTier,
    /// Original storage tier when chunk was created
    pub original_tier: StorageTier,
    /// Timestamp when chunk was last accessed
    pub last_accessed: u64,
    /// Timestamp when chunk was created
    pub created_at: u64,
    /// Timestamp when chunk was last moved between tiers
    pub last_moved: Option<u64>,
    /// Number of times chunk has been accessed
    pub access_count: u64,
    /// Size of the chunk in bytes
    pub size: u64,
    /// Storage class ID associated with this chunk
    pub storage_class_id: StorageClassId,
    /// Cost optimization metadata
    pub cost_metadata: CostMetadata,
    /// Tier movement history
    pub movement_history: Vec<TierMovement>,
}

/// Cost optimization metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMetadata {
    /// Total storage cost to date
    pub total_cost: f64,
    /// Monthly storage cost
    pub monthly_cost: f64,
    /// Access cost per operation
    pub access_cost: f64,
    /// Data transfer cost
    pub transfer_cost: f64,
}

/// Record of tier movement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierMovement {
    /// Source tier
    pub from_tier: StorageTier,
    /// Destination tier
    pub to_tier: StorageTier,
    /// Timestamp of movement
    pub moved_at: u64,
    /// Reason for movement
    pub reason: MovementReason,
    /// Duration of movement operation
    pub duration_ms: u64,
}

/// Reason for tier movement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MovementReason {
    /// Automatic movement based on access patterns
    AutomaticAccess,
    /// Automatic movement based on age
    AutomaticAge,
    /// Manual movement requested by user
    Manual,
    /// Movement for cost optimization
    CostOptimization,
    /// Movement for performance optimization
    PerformanceOptimization,
    /// Movement due to capacity constraints
    CapacityConstrained,
}

/// Data classification based on access patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataClassification {
    /// Access frequency over different time periods
    pub access_frequency: AccessFrequency,
    /// Data importance score (0.0-1.0)
    pub importance_score: f64,
    /// Data age in days
    pub age_days: u32,
    /// Predicted future access pattern
    pub predicted_access: PredictedAccess,
    /// Cost sensitivity (how much cost optimization to apply)
    pub cost_sensitivity: f64,
}

/// Access frequency analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessFrequency {
    /// Accesses in last 24 hours
    pub last_24h: u64,
    /// Accesses in last 7 days
    pub last_7d: u64,
    /// Accesses in last 30 days
    pub last_30d: u64,
    /// Accesses in last 365 days
    pub last_365d: u64,
    /// Average time between accesses (seconds)
    pub avg_access_interval: f64,
}

/// Predicted access pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedAccess {
    /// Probability of access in next 24 hours (0.0-1.0)
    pub next_24h_probability: f64,
    /// Probability of access in next 7 days (0.0-1.0)
    pub next_7d_probability: f64,
    /// Probability of access in next 30 days (0.0-1.0)
    pub next_30d_probability: f64,
    /// Confidence level of predictions (0.0-1.0)
    pub confidence: f64,
}

// Re-export LifecycleMetadata from object_storage module
pub use crate::object_storage::LifecycleMetadata;

/// Lifecycle transition rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleTransition {
    /// Days after creation to transition
    pub days: u32,
    /// Target storage class
    pub storage_class: String,
}

/// Tiered storage manager responsible for data classification and movement
#[derive(Debug)]
pub struct TieredStorageManager {
    /// Configuration for each storage tier
    tier_configs: RwLock<HashMap<StorageTier, TierConfig>>,
    /// Metadata tracking for all chunks
    chunk_metadata: RwLock<HashMap<ChunkId, ChunkTierMetadata>>,
    /// Classification policies
    classification_policies: RwLock<ClassificationPolicies>,
    /// Performance metrics
    performance_metrics: RwLock<PerformanceMetrics>,
}

/// Classification policies for determining optimal tier placement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationPolicies {
    /// Hot tier access threshold (accesses per day)
    pub hot_tier_access_threshold: f64,
    /// Warm tier access threshold (accesses per week)
    pub warm_tier_access_threshold: f64,
    /// Cold tier age threshold (days)
    pub cold_tier_age_threshold: u32,
    /// Archive tier age threshold (days)
    pub archive_tier_age_threshold: u32,
    /// Minimum importance score for hot tier (0.0-1.0)
    pub hot_tier_importance_threshold: f64,
    /// Cost optimization weight (0.0-1.0, higher = more cost sensitive)
    pub cost_optimization_weight: f64,
    /// Performance optimization weight (0.0-1.0, higher = more performance sensitive)
    pub performance_optimization_weight: f64,
}

/// Performance metrics for tiered storage
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    /// Total chunks per tier
    pub chunks_per_tier: HashMap<StorageTier, u64>,
    /// Total capacity per tier
    pub capacity_per_tier: HashMap<StorageTier, u64>,
    /// Average latency per tier
    pub avg_latency_per_tier: HashMap<StorageTier, Duration>,
    /// IOPS per tier
    pub iops_per_tier: HashMap<StorageTier, u32>,
    /// Error rate per tier
    pub error_rate_per_tier: HashMap<StorageTier, f64>,
    /// Cost per tier
    pub cost_per_tier: HashMap<StorageTier, f64>,
    /// Movement statistics
    pub movements_last_24h: u64,
    pub movements_last_7d: u64,
    pub movements_last_30d: u64,
}

impl Default for ClassificationPolicies {
    fn default() -> Self {
        Self {
            hot_tier_access_threshold: 10.0,        // 10+ accesses per day
            warm_tier_access_threshold: 2.0,        // 2+ accesses per week
            cold_tier_age_threshold: 30,            // 30 days
            archive_tier_age_threshold: 365,        // 1 year
            hot_tier_importance_threshold: 0.8,     // High importance
            cost_optimization_weight: 0.3,          // 30% cost weight
            performance_optimization_weight: 0.7,   // 70% performance weight
        }
    }
}

impl TieredStorageManager {
    /// Create a new tiered storage manager
    pub fn new() -> Self {
        Self {
            tier_configs: RwLock::new(HashMap::new()),
            chunk_metadata: RwLock::new(HashMap::new()),
            classification_policies: RwLock::new(ClassificationPolicies::default()),
            performance_metrics: RwLock::new(PerformanceMetrics::default()),
        }
    }
    
    /// Add configuration for a storage tier
    pub async fn add_tier_config(&self, config: TierConfig) -> Result<()> {
        let tier = config.tier;
        let mut configs = self.tier_configs.write().await;
        configs.insert(tier, config);
        info!("Added configuration for tier {:?}", tier);
        Ok(())
    }
    
    /// Get configuration for a storage tier
    pub async fn get_tier_config(&self, tier: StorageTier) -> Result<TierConfig> {
        let configs = self.tier_configs.read().await;
        configs.get(&tier)
            .cloned()
            .context(format!("No configuration found for tier {:?}", tier))
    }
    
    /// Classify data and determine optimal tier placement
    #[instrument(skip(self))]
    pub async fn classify_chunk(&self, chunk_id: ChunkId, access_patterns: &AccessFrequency, importance: f64) -> Result<StorageTier> {
        let policies = self.classification_policies.read().await;
        let metadata = self.get_chunk_metadata(chunk_id).await?;
        
        // Calculate access rate per day
        let access_rate_daily = access_patterns.last_24h as f64;
        let access_rate_weekly = access_patterns.last_7d as f64 / 7.0;
        
        // Age of the data
        let now = now_micros() / 1_000_000; // Convert to seconds
        let age_days = (now - metadata.created_at) / (24 * 3600);
        
        // Determine tier based on access patterns and policies
        let optimal_tier = if access_rate_daily >= policies.hot_tier_access_threshold 
            && importance >= policies.hot_tier_importance_threshold {
            StorageTier::Hot
        } else if access_rate_weekly >= policies.warm_tier_access_threshold 
            && age_days < policies.cold_tier_age_threshold as u64 {
            StorageTier::Warm
        } else if age_days < policies.archive_tier_age_threshold as u64 {
            StorageTier::Cold
        } else {
            StorageTier::Archive
        };
        
        debug!("Classified chunk {} to tier {:?} (access_daily: {}, age_days: {})", 
               chunk_id, optimal_tier, access_rate_daily, age_days);
        
        Ok(optimal_tier)
    }
    
    /// Get chunk metadata, creating default if not exists
    async fn get_chunk_metadata(&self, chunk_id: ChunkId) -> Result<ChunkTierMetadata> {
        let metadata = self.chunk_metadata.read().await;
        match metadata.get(&chunk_id) {
            Some(meta) => Ok(meta.clone()),
            None => {
                // Create default metadata for new chunk
                Ok(ChunkTierMetadata {
                    chunk_id,
                    current_tier: StorageTier::Hot, // Default to hot tier
                    original_tier: StorageTier::Hot,
                    last_accessed: now_micros() / 1_000_000,
                    created_at: now_micros() / 1_000_000,
                    last_moved: None,
                    access_count: 0,
                    size: 0,
                    storage_class_id: 1,
                    cost_metadata: CostMetadata {
                        total_cost: 0.0,
                        monthly_cost: 0.0,
                        access_cost: 0.0,
                        transfer_cost: 0.0,
                    },
                    movement_history: Vec::new(),
                })
            }
        }
    }
    
    /// Record chunk access for tracking patterns
    #[instrument(skip(self))]
    pub async fn record_chunk_access(&self, chunk_id: ChunkId) -> Result<()> {
        let mut metadata = self.chunk_metadata.write().await;
        
        match metadata.get_mut(&chunk_id) {
            Some(meta) => {
                meta.last_accessed = now_micros() / 1_000_000;
                meta.access_count += 1;
                debug!("Recorded access for chunk {} (total: {})", chunk_id, meta.access_count);
            }
            None => {
                warn!("Attempted to record access for unknown chunk {}", chunk_id);
            }
        }
        
        Ok(())
    }
    
    /// Get performance metrics for all tiers
    pub async fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().await.clone()
    }
    
    /// Update performance metrics
    pub async fn update_performance_metrics(&self, tier: StorageTier, latency: Duration, success: bool) {
        let mut metrics = self.performance_metrics.write().await;
        
        // Update latency
        metrics.avg_latency_per_tier.insert(tier, latency);
        
        // Update error rate if operation failed
        if !success {
            let current_error_rate = *metrics.error_rate_per_tier.get(&tier).unwrap_or(&0.0);
            metrics.error_rate_per_tier.insert(tier, current_error_rate + 0.01);
        }
    }
    
    /// Get optimal tier for a chunk based on current classification
    pub async fn get_optimal_tier(&self, chunk_id: ChunkId) -> Result<StorageTier> {
        let metadata = self.get_chunk_metadata(chunk_id).await?;
        
        // Create access frequency from metadata
        let now = now_micros() / 1_000_000;
        let access_frequency = AccessFrequency {
            last_24h: if metadata.last_accessed + 86400 > now { metadata.access_count } else { 0 },
            last_7d: metadata.access_count, // Simplified for now
            last_30d: metadata.access_count,
            last_365d: metadata.access_count,
            avg_access_interval: if metadata.access_count > 0 {
                (now - metadata.created_at) as f64 / metadata.access_count as f64
            } else {
                f64::INFINITY
            },
        };
        
        // Calculate importance based on access patterns (simplified)
        let importance = (metadata.access_count as f64 / 100.0).min(1.0);
        
        self.classify_chunk(chunk_id, &access_frequency, importance).await
    }
    
    /// Check if a chunk should be moved to a different tier
    pub async fn should_move_chunk(&self, chunk_id: ChunkId) -> Result<Option<StorageTier>> {
        let metadata = self.get_chunk_metadata(chunk_id).await?;
        let optimal_tier = self.get_optimal_tier(chunk_id).await?;
        
        if optimal_tier != metadata.current_tier {
            info!("Chunk {} should move from {:?} to {:?}", 
                  chunk_id, metadata.current_tier, optimal_tier);
            Ok(Some(optimal_tier))
        } else {
            Ok(None)
        }
    }
    
    /// Get optimal erasure coding configuration for a tier
    pub async fn get_tier_erasure_config(&self, tier: StorageTier) -> Result<Option<ErasureConfig>> {
        let configs = self.tier_configs.read().await;
        if let Some(tier_config) = configs.get(&tier) {
            Ok(tier_config.erasure_config.clone())
        } else {
            Ok(None)
        }
    }
    
    /// Check if chunk needs re-encoding when moving between tiers
    pub async fn needs_reencoding(&self, chunk_id: ChunkId, from_tier: StorageTier, to_tier: StorageTier) -> Result<bool> {
        let from_config = self.get_tier_erasure_config(from_tier).await?;
        let to_config = self.get_tier_erasure_config(to_tier).await?;
        
        match (from_config, to_config) {
            (Some(from), Some(to)) => {
                // Re-encoding needed if configurations differ
                Ok(from.data_shards != to.data_shards || from.parity_shards != to.parity_shards)
            }
            (None, Some(_)) => Ok(true), // Moving to erasure-coded tier
            (Some(_), None) => Ok(true), // Moving from erasure-coded tier
            (None, None) => Ok(false),   // No erasure coding on either tier
        }
    }
    
    /// Calculate storage efficiency for a tier considering erasure coding
    pub async fn calculate_storage_efficiency(&self, tier: StorageTier) -> Result<f64> {
        if let Some(erasure_config) = self.get_tier_erasure_config(tier).await? {
            // With erasure coding, efficiency = data_shards / total_shards
            let efficiency = erasure_config.data_shards as f64 / 
                           (erasure_config.data_shards + erasure_config.parity_shards) as f64;
            Ok(efficiency)
        } else {
            // Without erasure coding, assume 3-way replication (33% efficiency)
            Ok(1.0 / 3.0)
        }
    }
    
    /// Estimate cost savings from moving to a tier with different erasure coding
    pub async fn estimate_cost_savings(
        &self, 
        chunk_id: ChunkId, 
        from_tier: StorageTier, 
        to_tier: StorageTier
    ) -> Result<f64> {
        let metadata = self.get_chunk_metadata(chunk_id).await?;
        let chunk_size = metadata.size as f64;
        
        let from_efficiency = self.calculate_storage_efficiency(from_tier).await?;
        let to_efficiency = self.calculate_storage_efficiency(to_tier).await?;
        
        let from_config = self.get_tier_config(from_tier).await?;
        let to_config = self.get_tier_config(to_tier).await?;
        
        // Calculate storage cost difference
        let from_storage_cost = (chunk_size / from_efficiency) * from_config.cost_per_gb_month / (1024.0 * 1024.0 * 1024.0);
        let to_storage_cost = (chunk_size / to_efficiency) * to_config.cost_per_gb_month / (1024.0 * 1024.0 * 1024.0);
        
        // Return monthly savings (positive = savings, negative = increased cost)
        Ok(from_storage_cost - to_storage_cost)
    }
}

/// Default tier configurations for common deployment scenarios
impl TierConfig {
    /// Create a hot tier configuration (SSD-based)
    pub fn hot_tier(base_path: PathBuf, max_capacity: u64) -> Self {
        Self {
            tier: StorageTier::Hot,
            base_path: Some(base_path),
            max_capacity,
            used_capacity: 0,
            cost_per_gb_month: 0.50, // Higher cost for SSD
            object_storage: None,
            erasure_config: Some(ErasureConfig {
                data_shards: 8,
                parity_shards: 2,
                max_shard_size: 64 * 1024 * 1024, // 64MB
            }),
            performance_thresholds: PerformanceThresholds {
                max_latency_ms: 5,
                min_iops: 10_000,
                max_error_rate: 0.001,
                capacity_warning_threshold: 0.8,
                capacity_alert_threshold: 0.9,
            },
        }
    }
    
    /// Create a warm tier configuration (HDD-based)
    pub fn warm_tier(base_path: PathBuf, max_capacity: u64) -> Self {
        Self {
            tier: StorageTier::Warm,
            base_path: Some(base_path),
            max_capacity,
            used_capacity: 0,
            cost_per_gb_month: 0.10, // Lower cost for HDD
            object_storage: None,
            erasure_config: Some(ErasureConfig {
                data_shards: 6,
                parity_shards: 3,
                max_shard_size: 64 * 1024 * 1024, // 64MB
            }),
            performance_thresholds: PerformanceThresholds {
                max_latency_ms: 50,
                min_iops: 100,
                max_error_rate: 0.01,
                capacity_warning_threshold: 0.85,
                capacity_alert_threshold: 0.95,
            },
        }
    }
    
    /// Create a cold tier configuration (Object storage)
    pub fn cold_tier(bucket: String, region: String, max_capacity: u64) -> Self {
        Self {
            tier: StorageTier::Cold,
            base_path: None,
            max_capacity,
            used_capacity: 0,
            cost_per_gb_month: 0.02, // Very low storage cost
            object_storage: Some(ObjectStorageConfig {
                provider: "s3".to_string(),
                bucket,
                region,
                credentials: CredentialsConfig {
                    access_key_id: "".to_string(),
                    secret_access_key: "".to_string(),
                    session_token: None,
                    role_arn: None,
                },
                lifecycle: LifecycleConfig {
                    transition_days: 30,
                    delete_days: 0,
                    versioning: false,
                },
                cache: CacheConfig {
                    enabled: true,
                    max_size: 10 * 1024 * 1024 * 1024, // 10GB cache
                    ttl_seconds: 3600,
                    cache_dir: PathBuf::from("/tmp/mooseng-cold-cache"),
                },
            }),
            erasure_config: Some(ErasureConfig {
                data_shards: 4,
                parity_shards: 2,
                max_shard_size: 64 * 1024 * 1024, // 64MB
            }),
            performance_thresholds: PerformanceThresholds {
                max_latency_ms: 5000,
                min_iops: 10,
                max_error_rate: 0.05,
                capacity_warning_threshold: 0.9,
                capacity_alert_threshold: 0.95,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    
    #[tokio::test]
    async fn test_tier_priority() {
        assert!(StorageTier::Hot.is_faster_than(&StorageTier::Warm));
        assert!(StorageTier::Warm.is_faster_than(&StorageTier::Cold));
        assert!(StorageTier::Cold.is_faster_than(&StorageTier::Archive));
        assert!(!StorageTier::Archive.is_faster_than(&StorageTier::Hot));
    }
    
    #[tokio::test]
    async fn test_tier_manager_creation() {
        let manager = TieredStorageManager::new();
        
        // Add hot tier config
        let hot_config = TierConfig::hot_tier(
            PathBuf::from("/data/hot"), 
            1024 * 1024 * 1024 * 1024 // 1TB
        );
        manager.add_tier_config(hot_config).await.unwrap();
        
        // Verify config was added
        let retrieved_config = manager.get_tier_config(StorageTier::Hot).await.unwrap();
        assert_eq!(retrieved_config.tier, StorageTier::Hot);
        assert_eq!(retrieved_config.max_capacity, 1024 * 1024 * 1024 * 1024);
    }
    
    #[tokio::test]
    async fn test_chunk_classification() {
        let manager = TieredStorageManager::new();
        
        // Simulate high-access chunk (should be hot tier)
        let access_patterns = AccessFrequency {
            last_24h: 50,
            last_7d: 200,
            last_30d: 500,
            last_365d: 1000,
            avg_access_interval: 1800.0, // 30 minutes
        };
        
        let tier = manager.classify_chunk(1, &access_patterns, 0.9).await.unwrap();
        assert_eq!(tier, StorageTier::Hot);
        
        // Simulate low-access chunk (should be cold tier)
        let access_patterns = AccessFrequency {
            last_24h: 0,
            last_7d: 1,
            last_30d: 2,
            last_365d: 5,
            avg_access_interval: 86400.0 * 7.0, // Weekly
        };
        
        let tier = manager.classify_chunk(2, &access_patterns, 0.3).await.unwrap();
        assert!(tier == StorageTier::Cold || tier == StorageTier::Archive);
    }
    
    #[tokio::test]
    async fn test_access_recording() {
        let manager = TieredStorageManager::new();
        
        // Record access for a chunk
        manager.record_chunk_access(123).await.unwrap();
        
        // Check that access was recorded (would need to expose metadata for testing)
        // This is a simplified test
    }
    
    #[tokio::test]
    async fn test_tier_configurations() {
        let hot_config = TierConfig::hot_tier(PathBuf::from("/ssd"), 1000000);
        assert_eq!(hot_config.tier, StorageTier::Hot);
        assert!(hot_config.base_path.is_some());
        assert!(hot_config.cost_per_gb_month > 0.0);
        
        let cold_config = TierConfig::cold_tier("bucket".to_string(), "us-west-2".to_string(), 1000000);
        assert_eq!(cold_config.tier, StorageTier::Cold);
        assert!(cold_config.object_storage.is_some());
        assert!(cold_config.cost_per_gb_month < hot_config.cost_per_gb_month);
    }
}