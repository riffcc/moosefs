//! Region-aware data placement for MooseNG multiregion support
//! 
//! This module implements intelligent data placement strategies that consider
//! region locality, latency, availability zones, and access patterns to optimize
//! data distribution across multiple geographic regions.

use super::{MultiregionConfig, RegionPeer, ConsistencyLevel};
use crate::multiregion::hybrid_clock::HLCTimestamp;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use std::sync::Arc;

/// Unique identifier for a data chunk
pub type ChunkId = u64;

/// Unique identifier for a region
pub type RegionId = u32;

/// Unique identifier for an availability zone within a region
pub type AvailabilityZoneId = String;

/// Data placement policies for different scenarios
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementPolicy {
    /// Place data close to users (latency optimization)
    LatencyOptimized,
    
    /// Distribute data for maximum availability
    AvailabilityOptimized,
    
    /// Optimize for cost (prefer cheaper regions)
    CostOptimized,
    
    /// Comply with data residency requirements
    ComplianceOptimized,
    
    /// Balance between latency, availability, and cost
    Balanced,
    
    /// Custom policy with specific rules
    Custom(PlacementRules),
}

/// Custom placement rules for fine-grained control
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementRules {
    /// Preferred regions (in order of preference)
    pub preferred_regions: Vec<RegionId>,
    
    /// Regions to avoid (e.g., for compliance)
    pub excluded_regions: Vec<RegionId>,
    
    /// Minimum number of regions for placement
    pub min_regions: usize,
    
    /// Maximum number of regions for placement
    pub max_regions: usize,
    
    /// Require placement in specific availability zones
    pub required_zones: Vec<AvailabilityZoneId>,
    
    /// Maximum allowed latency for reads (milliseconds)
    pub max_read_latency_ms: u32,
    
    /// Data residency constraints
    pub residency_constraints: Vec<DataResidencyRule>,
}

/// Data residency rules for compliance requirements
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataResidencyRule {
    /// Type of data this rule applies to
    pub data_type: String,
    
    /// Allowed regions for this data type
    pub allowed_regions: Vec<RegionId>,
    
    /// Whether data can transit through other regions
    pub allow_transit: bool,
    
    /// Encryption requirements
    pub encryption_required: bool,
}

/// Information about data access patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPattern {
    /// Regions where this data is frequently accessed
    pub hot_regions: HashMap<RegionId, f64>, // region_id -> access frequency (0.0-1.0)
    
    /// Time-based access patterns
    pub temporal_pattern: TemporalAccessPattern,
    
    /// Read vs write ratio
    pub read_write_ratio: f64, // reads / (reads + writes)
    
    /// Average access latency requirements
    pub latency_requirement: Duration,
    
    /// Data criticality level
    pub criticality: DataCriticality,
}

/// Temporal access patterns for data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalAccessPattern {
    /// Peak access hours (24-hour format, UTC)
    pub peak_hours: Vec<u8>,
    
    /// Access timezone (for time-zone aware placement)
    pub primary_timezone: String,
    
    /// Seasonal patterns (if applicable)
    pub seasonal_factor: Option<f64>,
}

/// Data criticality levels affecting placement decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataCriticality {
    /// Critical data requiring maximum availability
    Critical,
    
    /// Important data with high availability needs
    High,
    
    /// Standard data with normal availability
    Standard,
    
    /// Archive data with lower availability requirements
    Archive,
    
    /// Temporary data that can be recreated
    Temporary,
}

/// Placement decision for a specific chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementDecision {
    /// Chunk being placed
    pub chunk_id: ChunkId,
    
    /// Primary region for the data
    pub primary_region: RegionId,
    
    /// Replica regions (in order of preference)
    pub replica_regions: Vec<RegionId>,
    
    /// Specific availability zones within regions
    pub zone_placement: HashMap<RegionId, Vec<AvailabilityZoneId>>,
    
    /// Placement policy used for this decision
    pub policy: PlacementPolicy,
    
    /// Expected read latency from each region
    pub expected_latencies: HashMap<RegionId, Duration>,
    
    /// Placement score (higher is better)
    pub placement_score: f64,
    
    /// Timestamp when placement decision was made
    pub decision_time: HLCTimestamp,
    
    /// Reason for the placement decision
    pub rationale: String,
}

/// Region information for placement decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo {
    /// Region identifier
    pub region_id: RegionId,
    
    /// Human-readable region name
    pub region_name: String,
    
    /// Available availability zones
    pub availability_zones: Vec<AvailabilityZoneId>,
    
    /// Region capacity information
    pub capacity: RegionCapacity,
    
    /// Network latency to other regions
    pub inter_region_latency: HashMap<RegionId, Duration>,
    
    /// Current load on this region
    pub current_load: RegionLoad,
    
    /// Geographic coordinates (for proximity calculations)
    pub coordinates: Option<(f64, f64)>, // (latitude, longitude)
    
    /// Compliance and regulatory information
    pub compliance_info: ComplianceInfo,
}

/// Capacity information for a region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionCapacity {
    /// Total storage capacity (bytes)
    pub total_storage: u64,
    
    /// Used storage (bytes)
    pub used_storage: u64,
    
    /// Available bandwidth (bytes/second)
    pub bandwidth: u64,
    
    /// Number of available nodes
    pub node_count: u32,
    
    /// Storage cost per GB per month
    pub storage_cost: f64,
    
    /// Bandwidth cost per GB
    pub bandwidth_cost: f64,
}

/// Current load metrics for a region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionLoad {
    /// CPU utilization (0.0-1.0)
    pub cpu_utilization: f64,
    
    /// Memory utilization (0.0-1.0)
    pub memory_utilization: f64,
    
    /// Network utilization (0.0-1.0)
    pub network_utilization: f64,
    
    /// Current IOPS
    pub current_iops: u64,
    
    /// Average latency for operations
    pub avg_latency: Duration,
}

/// Compliance and regulatory information for a region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceInfo {
    /// Data sovereignty requirements
    pub data_sovereignty: bool,
    
    /// Supported compliance frameworks
    pub compliance_frameworks: Vec<String>,
    
    /// Encryption requirements
    pub encryption_at_rest: bool,
    pub encryption_in_transit: bool,
    
    /// Audit and logging requirements
    pub audit_logging: bool,
}

/// Region-aware data placement manager
pub struct RegionPlacementManager {
    /// Configuration for multiregion setup
    config: MultiregionConfig,
    
    /// Information about all regions
    regions: Arc<RwLock<HashMap<RegionId, RegionInfo>>>,
    
    /// Current placement decisions
    placements: Arc<RwLock<HashMap<ChunkId, PlacementDecision>>>,
    
    /// Access pattern cache
    access_patterns: Arc<RwLock<HashMap<ChunkId, AccessPattern>>>,
    
    /// Placement policy cache
    policy_cache: Arc<RwLock<HashMap<String, PlacementPolicy>>>,
}

impl RegionPlacementManager {
    /// Create a new region placement manager
    pub fn new(config: MultiregionConfig) -> Self {
        Self {
            config,
            regions: Arc::new(RwLock::new(HashMap::new())),
            placements: Arc::new(RwLock::new(HashMap::new())),
            access_patterns: Arc::new(RwLock::new(HashMap::new())),
            policy_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a region with the placement manager
    pub async fn register_region(&self, region: RegionInfo) -> Result<()> {
        let mut regions = self.regions.write().await;
        regions.insert(region.region_id, region);
        Ok(())
    }
    
    /// Update region capacity and load information
    pub async fn update_region_metrics(&self, region_id: RegionId, capacity: RegionCapacity, load: RegionLoad) -> Result<()> {
        let mut regions = self.regions.write().await;
        if let Some(region) = regions.get_mut(&region_id) {
            region.capacity = capacity;
            region.current_load = load;
        } else {
            return Err(anyhow::anyhow!("Region {} not found", region_id));
        }
        Ok(())
    }
    
    /// Record access pattern for a chunk
    pub async fn record_access_pattern(&self, chunk_id: ChunkId, pattern: AccessPattern) -> Result<()> {
        let mut patterns = self.access_patterns.write().await;
        patterns.insert(chunk_id, pattern);
        Ok(())
    }
    
    /// Make a placement decision for a new chunk
    pub async fn place_chunk(
        &self,
        chunk_id: ChunkId,
        size: u64,
        policy: PlacementPolicy,
        consistency_level: ConsistencyLevel,
        access_hint: Option<AccessPattern>,
    ) -> Result<PlacementDecision> {
        let regions = self.regions.read().await;
        let patterns = self.access_patterns.read().await;
        
        // Get or create access pattern
        let access_pattern = access_hint.or_else(|| patterns.get(&chunk_id).cloned())
            .unwrap_or_else(|| self.default_access_pattern());
        
        // Calculate placement scores for each region
        let mut region_scores = HashMap::new();
        for (region_id, region) in regions.iter() {
            let score = self.calculate_placement_score(
                *region_id,
                region,
                size,
                &policy,
                &consistency_level,
                &access_pattern,
            ).await?;
            region_scores.insert(*region_id, score);
        }
        
        // Select optimal placement based on policy
        let placement = self.select_optimal_placement(
            chunk_id,
            &region_scores,
            &regions,
            policy,
            &access_pattern,
        ).await?;
        
        // Store the placement decision
        let mut placements = self.placements.write().await;
        placements.insert(chunk_id, placement.clone());
        
        Ok(placement)
    }
    
    /// Calculate placement score for a region
    async fn calculate_placement_score(
        &self,
        region_id: RegionId,
        region: &RegionInfo,
        chunk_size: u64,
        policy: &PlacementPolicy,
        consistency_level: &ConsistencyLevel,
        access_pattern: &AccessPattern,
    ) -> Result<f64> {
        let mut score = 0.0;
        
        match policy {
            PlacementPolicy::LatencyOptimized => {
                score += self.calculate_latency_score(region_id, region, access_pattern).await?;
            }
            PlacementPolicy::AvailabilityOptimized => {
                score += self.calculate_availability_score(region_id, region).await?;
            }
            PlacementPolicy::CostOptimized => {
                score += self.calculate_cost_score(region_id, region, chunk_size).await?;
            }
            PlacementPolicy::ComplianceOptimized => {
                score += self.calculate_compliance_score(region_id, region, access_pattern).await?;
            }
            PlacementPolicy::Balanced => {
                score += 0.3 * self.calculate_latency_score(region_id, region, access_pattern).await?;
                score += 0.3 * self.calculate_availability_score(region_id, region).await?;
                score += 0.2 * self.calculate_cost_score(region_id, region, chunk_size).await?;
                score += 0.2 * self.calculate_compliance_score(region_id, region, access_pattern).await?;
            }
            PlacementPolicy::Custom(rules) => {
                score += self.calculate_custom_score(region_id, region, rules, access_pattern).await?;
            }
        }
        
        // Apply consistency level penalties/bonuses
        score += self.calculate_consistency_score(region_id, region, consistency_level).await?;
        
        // Apply capacity constraints
        if region.capacity.used_storage + chunk_size > region.capacity.total_storage {
            score -= 1000.0; // Heavy penalty for insufficient capacity
        }
        
        // Apply load balancing
        score -= region.current_load.cpu_utilization * 100.0;
        score -= region.current_load.memory_utilization * 50.0;
        
        Ok(score)
    }
    
    /// Calculate latency-based score
    async fn calculate_latency_score(
        &self,
        region_id: RegionId,
        region: &RegionInfo,
        access_pattern: &AccessPattern,
    ) -> Result<f64> {
        let mut score = 0.0;
        
        // Score based on hot regions access frequency
        for (hot_region, frequency) in &access_pattern.hot_regions {
            if *hot_region == region_id {
                score += frequency * 1000.0; // High score for local access
            } else if let Some(latency) = region.inter_region_latency.get(hot_region) {
                let latency_ms = latency.as_millis() as f64;
                score += frequency * (1000.0 / (1.0 + latency_ms / 10.0));
            }
        }
        
        Ok(score)
    }
    
    /// Calculate availability-based score
    async fn calculate_availability_score(&self, region_id: RegionId, region: &RegionInfo) -> Result<f64> {
        let mut score = 0.0;
        
        // Score based on number of availability zones
        score += region.availability_zones.len() as f64 * 50.0;
        
        // Score based on region load (lower load = higher availability)
        score += (1.0 - region.current_load.cpu_utilization) * 100.0;
        score += (1.0 - region.current_load.memory_utilization) * 50.0;
        
        // Score based on capacity headroom
        let capacity_usage = region.capacity.used_storage as f64 / region.capacity.total_storage as f64;
        score += (1.0 - capacity_usage) * 200.0;
        
        Ok(score)
    }
    
    /// Calculate cost-based score
    async fn calculate_cost_score(&self, region_id: RegionId, region: &RegionInfo, chunk_size: u64) -> Result<f64> {
        let storage_cost = region.capacity.storage_cost * (chunk_size as f64 / 1_000_000_000.0);
        let score = 1000.0 / (1.0 + storage_cost);
        Ok(score)
    }
    
    /// Calculate compliance-based score
    async fn calculate_compliance_score(
        &self,
        region_id: RegionId,
        region: &RegionInfo,
        access_pattern: &AccessPattern,
    ) -> Result<f64> {
        let mut score = 0.0;
        
        // Score based on compliance frameworks
        score += region.compliance_info.compliance_frameworks.len() as f64 * 25.0;
        
        // Score based on data sovereignty
        if region.compliance_info.data_sovereignty {
            score += 100.0;
        }
        
        // Score based on encryption support
        if region.compliance_info.encryption_at_rest && region.compliance_info.encryption_in_transit {
            score += 50.0;
        }
        
        Ok(score)
    }
    
    /// Calculate custom policy score
    async fn calculate_custom_score(
        &self,
        region_id: RegionId,
        region: &RegionInfo,
        rules: &PlacementRules,
        access_pattern: &AccessPattern,
    ) -> Result<f64> {
        let mut score = 0.0;
        
        // Check preferred regions
        if let Some(position) = rules.preferred_regions.iter().position(|&r| r == region_id) {
            score += (rules.preferred_regions.len() - position) as f64 * 100.0;
        }
        
        // Check excluded regions
        if rules.excluded_regions.contains(&region_id) {
            score -= 10000.0; // Heavy penalty for excluded regions
        }
        
        // Check required zones
        if !rules.required_zones.is_empty() {
            let matching_zones = rules.required_zones.iter()
                .filter(|zone| region.availability_zones.contains(zone))
                .count();
            if matching_zones > 0 {
                score += matching_zones as f64 * 50.0;
            } else {
                score -= 1000.0; // Penalty for not having required zones
            }
        }
        
        // Check latency requirements
        if let Some(avg_latency) = region.inter_region_latency.values().map(|d| d.as_millis()).min() {
            if avg_latency <= rules.max_read_latency_ms as u128 {
                score += 200.0;
            } else {
                score -= (avg_latency - rules.max_read_latency_ms as u128) as f64;
            }
        }
        
        Ok(score)
    }
    
    /// Calculate consistency level score
    async fn calculate_consistency_score(
        &self,
        region_id: RegionId,
        region: &RegionInfo,
        consistency_level: &ConsistencyLevel,
    ) -> Result<f64> {
        let score = match consistency_level {
            ConsistencyLevel::Strong => {
                // Strong consistency prefers regions with low latency to primary
                if region_id == self.config.region_id {
                    200.0
                } else {
                    50.0
                }
            }
            ConsistencyLevel::BoundedStaleness(_) => {
                100.0 // Neutral score for bounded staleness
            }
            ConsistencyLevel::Session => {
                150.0 // Slight preference for session consistency
            }
            ConsistencyLevel::Eventual => {
                250.0 // High score for eventual consistency (more flexible)
            }
        };
        
        Ok(score)
    }
    
    /// Select optimal placement based on scores
    async fn select_optimal_placement(
        &self,
        chunk_id: ChunkId,
        region_scores: &HashMap<RegionId, f64>,
        regions: &HashMap<RegionId, RegionInfo>,
        policy: PlacementPolicy,
        access_pattern: &AccessPattern,
    ) -> Result<PlacementDecision> {
        // Sort regions by score (highest first)
        let mut sorted_regions: Vec<_> = region_scores.iter().collect();
        sorted_regions.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        
        if sorted_regions.is_empty() {
            return Err(anyhow::anyhow!("No suitable regions found for placement"));
        }
        
        let primary_region = *sorted_regions[0].0;
        let placement_score = *sorted_regions[0].1;
        
        // Select replica regions (top 2-3 after primary)
        let replica_regions: Vec<RegionId> = sorted_regions.iter()
            .skip(1)
            .take(2) // For now, use 2 replicas
            .filter(|(_, score)| **score > 0.0) // Only use regions with positive scores
            .map(|(region_id, _)| **region_id)
            .collect();
        
        // Calculate zone placement
        let mut zone_placement = HashMap::new();
        for &region_id in std::iter::once(&primary_region).chain(replica_regions.iter()) {
            if let Some(region) = regions.get(&region_id) {
                // For now, select all available zones (could be optimized)
                zone_placement.insert(region_id, region.availability_zones.clone());
            }
        }
        
        // Calculate expected latencies
        let mut expected_latencies = HashMap::new();
        for (region_id, region) in regions {
            if let Some(latency) = region.inter_region_latency.get(&self.config.region_id) {
                expected_latencies.insert(*region_id, *latency);
            } else if *region_id == self.config.region_id {
                expected_latencies.insert(*region_id, Duration::from_millis(1));
            }
        }
        
        let decision = PlacementDecision {
            chunk_id,
            primary_region,
            replica_regions,
            zone_placement,
            policy,
            expected_latencies,
            placement_score,
            decision_time: HLCTimestamp::now(),
            rationale: format!("Selected based on {} policy with score {:.2}", 
                             policy_to_string(&policy), placement_score),
        };
        
        Ok(decision)
    }
    
    /// Get placement decision for a chunk
    pub async fn get_placement(&self, chunk_id: ChunkId) -> Result<Option<PlacementDecision>> {
        let placements = self.placements.read().await;
        Ok(placements.get(&chunk_id).cloned())
    }
    
    /// Update placement for an existing chunk (e.g., based on new access patterns)
    pub async fn rebalance_chunk(
        &self,
        chunk_id: ChunkId,
        new_access_pattern: AccessPattern,
    ) -> Result<Option<PlacementDecision>> {
        // Update access pattern
        {
            let mut patterns = self.access_patterns.write().await;
            patterns.insert(chunk_id, new_access_pattern);
        }
        
        // Get current placement
        let current_placement = {
            let placements = self.placements.read().await;
            placements.get(&chunk_id).cloned()
        };
        
        if let Some(current) = current_placement {
            // Recalculate placement with new access pattern
            let new_placement = self.place_chunk(
                chunk_id,
                0, // Size doesn't matter for rebalancing
                current.policy,
                self.config.default_consistency,
                None, // Will use the updated access pattern
            ).await?;
            
            // Check if placement should change
            if new_placement.primary_region != current.primary_region ||
               new_placement.replica_regions != current.replica_regions {
                return Ok(Some(new_placement));
            }
        }
        
        Ok(None)
    }
    
    /// Get region statistics for monitoring
    pub async fn get_region_statistics(&self) -> Result<HashMap<RegionId, RegionStatistics>> {
        let regions = self.regions.read().await;
        let placements = self.placements.read().await;
        
        let mut stats = HashMap::new();
        
        for (region_id, region) in regions.iter() {
            let chunk_count = placements.values()
                .filter(|p| p.primary_region == *region_id || p.replica_regions.contains(region_id))
                .count();
            
            let primary_count = placements.values()
                .filter(|p| p.primary_region == *region_id)
                .count();
            
            let replica_count = chunk_count - primary_count;
            
            stats.insert(*region_id, RegionStatistics {
                region_id: *region_id,
                region_name: region.region_name.clone(),
                total_chunks: chunk_count,
                primary_chunks: primary_count,
                replica_chunks: replica_count,
                capacity_usage: region.capacity.used_storage as f64 / region.capacity.total_storage as f64,
                current_load: region.current_load.clone(),
                availability_zones: region.availability_zones.len(),
            });
        }
        
        Ok(stats)
    }
    
    /// Create default access pattern
    fn default_access_pattern(&self) -> AccessPattern {
        let mut hot_regions = HashMap::new();
        hot_regions.insert(self.config.region_id, 1.0);
        
        AccessPattern {
            hot_regions,
            temporal_pattern: TemporalAccessPattern {
                peak_hours: vec![9, 10, 11, 14, 15, 16], // Business hours
                primary_timezone: "UTC".to_string(),
                seasonal_factor: None,
            },
            read_write_ratio: 0.8, // 80% reads, 20% writes
            latency_requirement: Duration::from_millis(100),
            criticality: DataCriticality::Standard,
        }
    }
}

/// Statistics for a region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionStatistics {
    pub region_id: RegionId,
    pub region_name: String,
    pub total_chunks: usize,
    pub primary_chunks: usize,
    pub replica_chunks: usize,
    pub capacity_usage: f64,
    pub current_load: RegionLoad,
    pub availability_zones: usize,
}

/// Helper function to convert policy to string
fn policy_to_string(policy: &PlacementPolicy) -> &'static str {
    match policy {
        PlacementPolicy::LatencyOptimized => "latency-optimized",
        PlacementPolicy::AvailabilityOptimized => "availability-optimized",
        PlacementPolicy::CostOptimized => "cost-optimized",
        PlacementPolicy::ComplianceOptimized => "compliance-optimized",
        PlacementPolicy::Balanced => "balanced",
        PlacementPolicy::Custom(_) => "custom",
    }
}

/// Implement HLCTimestamp::now() for convenience
impl HLCTimestamp {
    pub fn now() -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        
        HLCTimestamp {
            physical: now.as_millis() as u64,
            logical: 0,
            node_id: Some(0), // This should be set properly in real implementation
        }
    }
}