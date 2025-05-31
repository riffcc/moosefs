//! Tiered Storage Policies Module
//! 
//! This module implements various policies for data movement between storage tiers,
//! including time-based policies, access-pattern policies, and cost optimization policies.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::types::{ChunkId, TierType};
use super::{AccessPattern, MigrationRecommendation, MigrationReason, TierType as LocalTierType};

/// Policy engine for determining data movement between tiers
pub struct PolicyEngine {
    /// Active policies
    policies: Vec<Box<dyn TieringPolicy + Send + Sync>>,
    /// Policy configuration
    config: PolicyConfig,
}

/// Configuration for policy engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Enable automatic policy execution
    pub auto_execute: bool,
    /// Policy evaluation interval
    pub evaluation_interval: Duration,
    /// Maximum migrations per hour
    pub max_migrations_per_hour: u32,
    /// Minimum time between policy evaluations for same chunk
    pub min_evaluation_interval: Duration,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            auto_execute: true,
            evaluation_interval: Duration::from_secs(3600), // 1 hour
            max_migrations_per_hour: 100,
            min_evaluation_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Trait for tiering policies
pub trait TieringPolicy {
    /// Get policy name
    fn name(&self) -> &str;
    
    /// Get policy priority (higher = more important)
    fn priority(&self) -> u8;
    
    /// Evaluate whether a chunk should be migrated
    fn evaluate(&self, chunk_id: ChunkId, current_tier: LocalTierType, access_pattern: &AccessPattern) -> Option<MigrationRecommendation>;
    
    /// Check if policy is enabled
    fn is_enabled(&self) -> bool;
}

/// Age-based policy - moves data based on age
pub struct AgeBasedPolicy {
    config: AgeBasedConfig,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeBasedConfig {
    /// Days after which to move from hot to warm
    pub hot_to_warm_days: u32,
    /// Days after which to move from warm to cold
    pub warm_to_cold_days: u32,
    /// Days after which to move from cold to archive
    pub cold_to_archive_days: u32,
    /// Minimum file size to consider for archiving (bytes)
    pub min_archive_size: u64,
}

impl Default for AgeBasedConfig {
    fn default() -> Self {
        Self {
            hot_to_warm_days: 30,
            warm_to_cold_days: 90,
            cold_to_archive_days: 365,
            min_archive_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

impl AgeBasedPolicy {
    pub fn new(config: AgeBasedConfig) -> Self {
        Self {
            config,
            enabled: true,
        }
    }
    
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    
    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

impl TieringPolicy for AgeBasedPolicy {
    fn name(&self) -> &str {
        "age_based"
    }
    
    fn priority(&self) -> u8 {
        50 // Medium priority
    }
    
    fn evaluate(&self, chunk_id: ChunkId, current_tier: LocalTierType, access_pattern: &AccessPattern) -> Option<MigrationRecommendation> {
        if !self.enabled {
            return None;
        }
        
        let age_days = access_pattern.created_at
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
            .as_secs() / 86400;
        
        let recommended_tier = match current_tier {
            LocalTierType::HotSSD => {
                if age_days >= self.config.hot_to_warm_days as u64 {
                    if access_pattern.file_size > 100 * 1024 * 1024 { // > 100MB
                        LocalTierType::WarmHDD
                    } else {
                        LocalTierType::WarmSSD
                    }
                } else {
                    return None;
                }
            }
            LocalTierType::WarmSSD | LocalTierType::WarmHDD => {
                if age_days >= self.config.warm_to_cold_days as u64 {
                    LocalTierType::Cold
                } else {
                    return None;
                }
            }
            LocalTierType::Cold => {
                if age_days >= self.config.cold_to_archive_days as u64 && 
                   access_pattern.file_size >= self.config.min_archive_size {
                    LocalTierType::Archive
                } else {
                    return None;
                }
            }
            LocalTierType::Archive => return None, // Already at lowest tier
        };
        
        // Calculate priority based on how old the data is
        let priority_score = (age_days as f64 / self.config.hot_to_warm_days as f64).min(10.0);
        
        Some(MigrationRecommendation {
            chunk_id,
            current_tier,
            recommended_tier,
            reason: MigrationReason::AgeBased,
            priority_score,
            cost_savings_monthly: self.estimate_cost_savings(current_tier, recommended_tier, access_pattern.file_size),
        })
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl AgeBasedPolicy {
    fn estimate_cost_savings(&self, from_tier: LocalTierType, to_tier: LocalTierType, file_size: u64) -> f64 {
        // Simplified cost estimation
        let size_gb = file_size as f64 / 1024.0 / 1024.0 / 1024.0;
        
        let from_cost = match from_tier {
            LocalTierType::HotSSD => 0.50 * size_gb,
            LocalTierType::WarmSSD => 0.30 * size_gb,
            LocalTierType::WarmHDD => 0.10 * size_gb,
            LocalTierType::Cold => 0.02 * size_gb,
            LocalTierType::Archive => 0.005 * size_gb,
        };
        
        let to_cost = match to_tier {
            LocalTierType::HotSSD => 0.50 * size_gb,
            LocalTierType::WarmSSD => 0.30 * size_gb,
            LocalTierType::WarmHDD => 0.10 * size_gb,
            LocalTierType::Cold => 0.02 * size_gb,
            LocalTierType::Archive => 0.005 * size_gb,
        };
        
        (from_cost - to_cost).max(0.0)
    }
}

/// Access frequency based policy
pub struct AccessFrequencyPolicy {
    config: AccessFrequencyConfig,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessFrequencyConfig {
    /// Minimum accesses per day to keep in hot tier
    pub hot_tier_min_daily_accesses: f64,
    /// Minimum accesses per week to keep in warm tier
    pub warm_tier_min_weekly_accesses: f64,
    /// Days to track for access frequency calculation
    pub tracking_days: u32,
}

impl Default for AccessFrequencyConfig {
    fn default() -> Self {
        Self {
            hot_tier_min_daily_accesses: 5.0,
            warm_tier_min_weekly_accesses: 2.0,
            tracking_days: 30,
        }
    }
}

impl AccessFrequencyPolicy {
    pub fn new(config: AccessFrequencyConfig) -> Self {
        Self {
            config,
            enabled: true,
        }
    }
    
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    
    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

impl TieringPolicy for AccessFrequencyPolicy {
    fn name(&self) -> &str {
        "access_frequency"
    }
    
    fn priority(&self) -> u8 {
        80 // High priority - access patterns are important
    }
    
    fn evaluate(&self, chunk_id: ChunkId, current_tier: LocalTierType, access_pattern: &AccessPattern) -> Option<MigrationRecommendation> {
        if !self.enabled {
            return None;
        }
        
        let daily_accesses = (access_pattern.reads_24h + access_pattern.writes_24h) as f64;
        let weekly_accesses = (access_pattern.reads_7d + access_pattern.writes_7d) as f64 / 7.0;
        
        let recommended_tier = if daily_accesses >= self.config.hot_tier_min_daily_accesses {
            // High access frequency - should be in hot tier
            if current_tier != LocalTierType::HotSSD {
                LocalTierType::HotSSD
            } else {
                return None; // Already in correct tier
            }
        } else if weekly_accesses >= self.config.warm_tier_min_weekly_accesses {
            // Medium access frequency - should be in warm tier
            let target_tier = if access_pattern.file_size > 100 * 1024 * 1024 {
                LocalTierType::WarmHDD
            } else {
                LocalTierType::WarmSSD
            };
            
            if current_tier != target_tier && current_tier != LocalTierType::HotSSD {
                target_tier
            } else if current_tier == LocalTierType::Cold || current_tier == LocalTierType::Archive {
                target_tier
            } else {
                return None;
            }
        } else {
            // Low access frequency - move to cold tier if not already there
            if current_tier == LocalTierType::HotSSD || current_tier == LocalTierType::WarmSSD || current_tier == LocalTierType::WarmHDD {
                LocalTierType::Cold
            } else {
                return None;
            }
        };
        
        let reason = if daily_accesses >= self.config.hot_tier_min_daily_accesses {
            MigrationReason::FrequentAccess
        } else if weekly_accesses < self.config.warm_tier_min_weekly_accesses {
            MigrationReason::InfrequentAccess
        } else {
            MigrationReason::PerformanceOptimization
        };
        
        // Priority based on access frequency delta
        let priority_score = if daily_accesses >= self.config.hot_tier_min_daily_accesses {
            9.0 // High priority for promoting frequently accessed data
        } else if weekly_accesses < self.config.warm_tier_min_weekly_accesses {
            7.0 // High priority for demoting infrequently accessed data
        } else {
            5.0 // Medium priority
        };
        
        Some(MigrationRecommendation {
            chunk_id,
            current_tier,
            recommended_tier,
            reason,
            priority_score,
            cost_savings_monthly: self.estimate_cost_savings(current_tier, recommended_tier, access_pattern.file_size),
        })
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl AccessFrequencyPolicy {
    fn estimate_cost_savings(&self, from_tier: LocalTierType, to_tier: LocalTierType, file_size: u64) -> f64 {
        // Similar to AgeBasedPolicy
        let size_gb = file_size as f64 / 1024.0 / 1024.0 / 1024.0;
        
        let from_cost = match from_tier {
            LocalTierType::HotSSD => 0.50 * size_gb,
            LocalTierType::WarmSSD => 0.30 * size_gb,
            LocalTierType::WarmHDD => 0.10 * size_gb,
            LocalTierType::Cold => 0.02 * size_gb,
            LocalTierType::Archive => 0.005 * size_gb,
        };
        
        let to_cost = match to_tier {
            LocalTierType::HotSSD => 0.50 * size_gb,
            LocalTierType::WarmSSD => 0.30 * size_gb,
            LocalTierType::WarmHDD => 0.10 * size_gb,
            LocalTierType::Cold => 0.02 * size_gb,
            LocalTierType::Archive => 0.005 * size_gb,
        };
        
        (from_cost - to_cost).max(0.0)
    }
}

/// Capacity management policy - moves data when tiers are getting full
pub struct CapacityManagementPolicy {
    config: CapacityConfig,
    tier_utilization: HashMap<LocalTierType, f64>,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityConfig {
    /// Utilization threshold to trigger migrations (percentage)
    pub utilization_threshold: f64,
    /// Target utilization after migrations (percentage)
    pub target_utilization: f64,
    /// Prefer moving larger files first
    pub prefer_large_files: bool,
}

impl Default for CapacityConfig {
    fn default() -> Self {
        Self {
            utilization_threshold: 85.0,
            target_utilization: 75.0,
            prefer_large_files: true,
        }
    }
}

impl CapacityManagementPolicy {
    pub fn new(config: CapacityConfig) -> Self {
        Self {
            config,
            tier_utilization: HashMap::new(),
            enabled: true,
        }
    }
    
    pub fn update_tier_utilization(&mut self, tier: LocalTierType, utilization_percent: f64) {
        self.tier_utilization.insert(tier, utilization_percent);
    }
    
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    
    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

impl TieringPolicy for CapacityManagementPolicy {
    fn name(&self) -> &str {
        "capacity_management"
    }
    
    fn priority(&self) -> u8 {
        90 // Highest priority - capacity management is critical
    }
    
    fn evaluate(&self, chunk_id: ChunkId, current_tier: LocalTierType, access_pattern: &AccessPattern) -> Option<MigrationRecommendation> {
        if !self.enabled {
            return None;
        }
        
        // Check if current tier is over threshold
        let current_utilization = self.tier_utilization.get(&current_tier).copied().unwrap_or(0.0);
        
        if current_utilization < self.config.utilization_threshold {
            return None; // No capacity pressure
        }
        
        // Determine next lower tier
        let recommended_tier = match current_tier {
            LocalTierType::HotSSD => {
                if access_pattern.file_size > 100 * 1024 * 1024 {
                    LocalTierType::WarmHDD
                } else {
                    LocalTierType::WarmSSD
                }
            }
            LocalTierType::WarmSSD | LocalTierType::WarmHDD => LocalTierType::Cold,
            LocalTierType::Cold => LocalTierType::Archive,
            LocalTierType::Archive => return None, // Can't move lower
        };
        
        // Check if target tier has capacity
        let target_utilization = self.tier_utilization.get(&recommended_tier).copied().unwrap_or(0.0);
        if target_utilization > self.config.utilization_threshold {
            return None; // Target tier is also full
        }
        
        // Priority based on current utilization and file size
        let utilization_factor = (current_utilization - self.config.utilization_threshold) / 
                                (100.0 - self.config.utilization_threshold);
        let size_factor = if self.config.prefer_large_files {
            (access_pattern.file_size as f64 / (1024.0 * 1024.0 * 1024.0)).min(10.0) // Size in GB, capped at 10
        } else {
            1.0
        };
        
        let priority_score = (8.0 + utilization_factor * 2.0) * size_factor;
        
        Some(MigrationRecommendation {
            chunk_id,
            current_tier,
            recommended_tier,
            reason: MigrationReason::CapacityManagement,
            priority_score,
            cost_savings_monthly: self.estimate_cost_savings(current_tier, recommended_tier, access_pattern.file_size),
        })
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl CapacityManagementPolicy {
    fn estimate_cost_savings(&self, from_tier: LocalTierType, to_tier: LocalTierType, file_size: u64) -> f64 {
        let size_gb = file_size as f64 / 1024.0 / 1024.0 / 1024.0;
        
        let from_cost = match from_tier {
            LocalTierType::HotSSD => 0.50 * size_gb,
            LocalTierType::WarmSSD => 0.30 * size_gb,
            LocalTierType::WarmHDD => 0.10 * size_gb,
            LocalTierType::Cold => 0.02 * size_gb,
            LocalTierType::Archive => 0.005 * size_gb,
        };
        
        let to_cost = match to_tier {
            LocalTierType::HotSSD => 0.50 * size_gb,
            LocalTierType::WarmSSD => 0.30 * size_gb,
            LocalTierType::WarmHDD => 0.10 * size_gb,
            LocalTierType::Cold => 0.02 * size_gb,
            LocalTierType::Archive => 0.005 * size_gb,
        };
        
        (from_cost - to_cost).max(0.0)
    }
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new(config: PolicyConfig) -> Self {
        Self {
            policies: Vec::new(),
            config,
        }
    }
    
    /// Add a policy to the engine
    pub fn add_policy(&mut self, policy: Box<dyn TieringPolicy + Send + Sync>) {
        info!("Adding tiering policy: {}", policy.name());
        self.policies.push(policy);
        
        // Sort policies by priority (highest first)
        self.policies.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }
    
    /// Evaluate all policies for a chunk
    pub fn evaluate_chunk(&self, chunk_id: ChunkId, current_tier: LocalTierType, access_pattern: &AccessPattern) -> Vec<MigrationRecommendation> {
        let mut recommendations = Vec::new();
        
        for policy in &self.policies {
            if let Some(recommendation) = policy.evaluate(chunk_id, current_tier, access_pattern) {
                debug!("Policy '{}' recommends migration: {:?} -> {:?} (score: {:.2})",
                       policy.name(), recommendation.current_tier, recommendation.recommended_tier, recommendation.priority_score);
                recommendations.push(recommendation);
            }
        }
        
        // Sort by priority score (highest first)
        recommendations.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap_or(std::cmp::Ordering::Equal));
        
        recommendations
    }
    
    /// Get the best recommendation for a chunk
    pub fn get_best_recommendation(&self, chunk_id: ChunkId, current_tier: LocalTierType, access_pattern: &AccessPattern) -> Option<MigrationRecommendation> {
        self.evaluate_chunk(chunk_id, current_tier, access_pattern)
            .into_iter()
            .next()
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: PolicyConfig) {
        self.config = config;
        info!("Updated policy engine configuration");
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> &PolicyConfig {
        &self.config
    }
    
    /// Get list of active policies
    pub fn get_policy_names(&self) -> Vec<String> {
        self.policies.iter().map(|p| p.name().to_string()).collect()
    }
}

/// Default policy engine with common policies
pub fn create_default_policy_engine() -> PolicyEngine {
    let mut engine = PolicyEngine::new(PolicyConfig::default());
    
    // Add default policies
    engine.add_policy(Box::new(CapacityManagementPolicy::new(CapacityConfig::default())));
    engine.add_policy(Box::new(AccessFrequencyPolicy::new(AccessFrequencyConfig::default())));
    engine.add_policy(Box::new(AgeBasedPolicy::new(AgeBasedConfig::default())));
    
    engine
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    
    fn create_test_access_pattern(reads_24h: u64, reads_7d: u64, age_days: u64, size: u64) -> AccessPattern {
        AccessPattern {
            reads_24h,
            writes_24h: 0,
            reads_7d,
            writes_7d: 0,
            last_access: SystemTime::now(),
            file_size: size,
            created_at: SystemTime::now() - Duration::from_secs(age_days * 86400),
        }
    }
    
    #[test]
    fn test_age_based_policy() {
        let policy = AgeBasedPolicy::new(AgeBasedConfig::default());
        
        // Test old file in hot tier
        let old_pattern = create_test_access_pattern(0, 0, 60, 1024 * 1024); // 60 days old, 1MB
        let recommendation = policy.evaluate(123, LocalTierType::HotSSD, &old_pattern);
        
        assert!(recommendation.is_some());
        let rec = recommendation.unwrap();
        assert_eq!(rec.current_tier, LocalTierType::HotSSD);
        assert!(matches!(rec.recommended_tier, LocalTierType::WarmSSD));
        assert!(matches!(rec.reason, MigrationReason::AgeBased));
    }
    
    #[test]
    fn test_access_frequency_policy() {
        let policy = AccessFrequencyPolicy::new(AccessFrequencyConfig::default());
        
        // Test frequently accessed file in cold tier
        let hot_pattern = create_test_access_pattern(10, 70, 1, 1024 * 1024); // 10 reads/day
        let recommendation = policy.evaluate(456, LocalTierType::Cold, &hot_pattern);
        
        assert!(recommendation.is_some());
        let rec = recommendation.unwrap();
        assert_eq!(rec.current_tier, LocalTierType::Cold);
        assert_eq!(rec.recommended_tier, LocalTierType::HotSSD);
        assert!(matches!(rec.reason, MigrationReason::FrequentAccess));
    }
    
    #[test]
    fn test_capacity_management_policy() {
        let mut policy = CapacityManagementPolicy::new(CapacityConfig::default());
        policy.update_tier_utilization(LocalTierType::HotSSD, 90.0); // Over threshold
        
        let pattern = create_test_access_pattern(1, 7, 1, 100 * 1024 * 1024); // 100MB file
        let recommendation = policy.evaluate(789, LocalTierType::HotSSD, &pattern);
        
        assert!(recommendation.is_some());
        let rec = recommendation.unwrap();
        assert_eq!(rec.current_tier, LocalTierType::HotSSD);
        assert!(matches!(rec.reason, MigrationReason::CapacityManagement));
    }
    
    #[test]
    fn test_policy_engine() {
        let mut engine = create_default_policy_engine();
        
        let pattern = create_test_access_pattern(1, 7, 45, 50 * 1024 * 1024); // Moderate activity, 45 days old
        let recommendations = engine.evaluate_chunk(999, LocalTierType::HotSSD, &pattern);
        
        assert!(!recommendations.is_empty());
        
        // Should be sorted by priority
        for i in 1..recommendations.len() {
            assert!(recommendations[i-1].priority_score >= recommendations[i].priority_score);
        }
    }
    
    #[test]
    fn test_policy_priorities() {
        let capacity_policy = CapacityManagementPolicy::new(CapacityConfig::default());
        let access_policy = AccessFrequencyPolicy::new(AccessFrequencyConfig::default());
        let age_policy = AgeBasedPolicy::new(AgeBasedConfig::default());
        
        assert!(capacity_policy.priority() > access_policy.priority());
        assert!(access_policy.priority() > age_policy.priority());
    }
}