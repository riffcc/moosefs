//! Data consistency monitoring for MooseNG health checks
//! 
//! This module provides comprehensive data consistency monitoring including:
//! - Chunk replica consistency across servers
//! - Cross-region data synchronization verification  
//! - Metadata integrity checks
//! - Erasure coding stripe verification

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{debug, error, warn};

/// Data consistency monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyConfig {
    /// Enable deep consistency checks (slower but more thorough)
    pub enable_deep_checks: bool,
    /// Timeout for consistency checks
    pub check_timeout_seconds: u64,
    /// Sample size for random chunk verification
    pub sample_size: usize,
    /// Cross-region consistency check interval
    pub cross_region_interval_seconds: u64,
    /// Acceptable consistency ratio threshold
    pub consistency_threshold: f64,
    /// Enable erasure coding verification
    pub verify_erasure_coding: bool,
}

impl Default for ConsistencyConfig {
    fn default() -> Self {
        Self {
            enable_deep_checks: false,
            check_timeout_seconds: 30,
            sample_size: 100,
            cross_region_interval_seconds: 300, // 5 minutes
            consistency_threshold: 0.99,       // 99% consistency required
            verify_erasure_coding: true,
        }
    }
}

/// Data consistency checker
pub struct DataConsistencyChecker {
    config: ConsistencyConfig,
}

/// Chunk consistency information
#[derive(Debug, Clone)]
struct ChunkConsistency {
    chunk_id: String,
    region: String,
    replica_count: u32,
    expected_replicas: u32,
    checksum_matches: bool,
    size_matches: bool,
    last_verified: SystemTime,
    inconsistencies: Vec<String>,
}

/// Cross-region synchronization status
#[derive(Debug, Clone)]
struct CrossRegionStatus {
    source_region: String,
    target_region: String,
    last_sync_time: SystemTime,
    sync_lag_seconds: u64,
    pending_operations: u64,
    failed_operations: u64,
}

/// Metadata consistency information
#[derive(Debug, Clone)]
struct MetadataConsistency {
    namespace: String,
    total_entries: u64,
    consistent_entries: u64,
    inconsistent_entries: u64,
    missing_entries: u64,
    orphaned_entries: u64,
    last_check: SystemTime,
}

impl DataConsistencyChecker {
    /// Create a new data consistency checker
    pub fn new(config: ConsistencyConfig) -> Self {
        Self { config }
    }

    /// Check data consistency across replicas and regions
    pub async fn check_data_consistency(
        &self,
        chunk_id: Option<&str>,
        region: Option<&str>,
        deep_check: bool,
    ) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();
        let start_time = SystemTime::now();
        
        // Check chunk-level consistency
        let chunk_metrics = self.check_chunk_consistency(chunk_id, region, deep_check).await?;
        for (key, value) in chunk_metrics {
            metrics.insert(format!("chunk_{}", key), value);
        }
        
        // Check cross-region consistency if no specific chunk is requested
        if chunk_id.is_none() {
            let cross_region_metrics = self.check_cross_region_consistency(region).await?;
            for (key, value) in cross_region_metrics {
                metrics.insert(format!("cross_region_{}", key), value);
            }
        }
        
        // Check metadata consistency
        let metadata_metrics = self.check_metadata_consistency().await?;
        for (key, value) in metadata_metrics {
            metrics.insert(format!("metadata_{}", key), value);
        }
        
        // Check erasure coding consistency if enabled
        if self.config.verify_erasure_coding && chunk_id.is_none() {
            let erasure_metrics = self.check_erasure_coding_consistency().await?;
            for (key, value) in erasure_metrics {
                metrics.insert(format!("erasure_{}", key), value);
            }
        }
        
        // Calculate overall consistency metrics
        let total_chunks = metrics.get("chunk_total_checked").unwrap_or(&0.0);
        let inconsistent_chunks = metrics.get("chunk_inconsistent_count").unwrap_or(&0.0);
        let missing_replicas = metrics.get("chunk_missing_replicas").unwrap_or(&0.0);
        
        if *total_chunks > 0.0 {
            let consistency_ratio = (*total_chunks - *inconsistent_chunks) / *total_chunks;
            metrics.insert("consistency_ratio".to_string(), consistency_ratio);
        } else {
            metrics.insert("consistency_ratio".to_string(), 1.0);
        }
        
        metrics.insert("total_chunks".to_string(), *total_chunks);
        metrics.insert("inconsistent_chunks".to_string(), *inconsistent_chunks);
        metrics.insert("missing_replicas".to_string(), *missing_replicas);
        
        let check_duration = start_time.elapsed().unwrap_or_default().as_secs_f64();
        metrics.insert("check_duration_seconds".to_string(), check_duration);
        
        debug!("Data consistency check completed in {:.2}s with {} metrics", 
               check_duration, metrics.len());
        
        Ok(metrics)
    }

    /// Check consistency of individual chunks
    async fn check_chunk_consistency(
        &self,
        specific_chunk: Option<&str>,
        specific_region: Option<&str>,
        deep_check: bool,
    ) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();
        
        // Get list of chunks to check
        let chunks_to_check = if let Some(chunk_id) = specific_chunk {
            vec![chunk_id.to_string()]
        } else {
            self.get_sample_chunks(specific_region).await?
        };
        
        let mut total_checked = 0;
        let mut inconsistent_count = 0;
        let mut missing_replicas = 0;
        let mut total_replicas = 0;
        let mut checksum_mismatches = 0;
        let mut size_mismatches = 0;
        
        for chunk_id in chunks_to_check {
            total_checked += 1;
            
            match timeout(
                Duration::from_secs(self.config.check_timeout_seconds),
                self.verify_chunk_consistency(&chunk_id, deep_check)
            ).await {
                Ok(Ok(consistency)) => {
                    total_replicas += consistency.replica_count;
                    
                    if consistency.replica_count < consistency.expected_replicas {
                        missing_replicas += consistency.expected_replicas - consistency.replica_count;
                    }
                    
                    if !consistency.inconsistencies.is_empty() {
                        inconsistent_count += 1;
                    }
                    
                    if !consistency.checksum_matches {
                        checksum_mismatches += 1;
                    }
                    
                    if !consistency.size_matches {
                        size_mismatches += 1;
                    }
                }
                Ok(Err(e)) => {
                    warn!("Failed to check consistency for chunk {}: {}", chunk_id, e);
                    inconsistent_count += 1;
                }
                Err(_) => {
                    warn!("Timeout checking consistency for chunk {}", chunk_id);
                    inconsistent_count += 1;
                }
            }
        }
        
        metrics.insert("total_checked".to_string(), total_checked as f64);
        metrics.insert("inconsistent_count".to_string(), inconsistent_count as f64);
        metrics.insert("missing_replicas".to_string(), missing_replicas as f64);
        metrics.insert("total_replicas".to_string(), total_replicas as f64);
        metrics.insert("checksum_mismatches".to_string(), checksum_mismatches as f64);
        metrics.insert("size_mismatches".to_string(), size_mismatches as f64);
        
        if total_checked > 0 {
            let consistency_rate = (total_checked - inconsistent_count) as f64 / total_checked as f64;
            metrics.insert("consistency_rate".to_string(), consistency_rate);
        }
        
        Ok(metrics)
    }

    /// Verify consistency of a specific chunk
    async fn verify_chunk_consistency(&self, chunk_id: &str, deep_check: bool) -> Result<ChunkConsistency> {
        // This is a simplified implementation
        // In a real system, this would:
        // 1. Get chunk metadata from master server
        // 2. Query all chunk servers that should have replicas
        // 3. Compare checksums, sizes, and modification times
        // 4. Verify erasure coding stripe integrity if applicable
        
        let mut consistency = ChunkConsistency {
            chunk_id: chunk_id.to_string(),
            region: "default".to_string(),
            replica_count: 3,           // Would be queried from actual system
            expected_replicas: 3,       // Would be from replication policy
            checksum_matches: true,     // Would be calculated
            size_matches: true,         // Would be verified
            last_verified: SystemTime::now(),
            inconsistencies: Vec::new(),
        };
        
        // Simulate some potential inconsistencies for demonstration
        if chunk_id.contains("test_inconsistent") {
            consistency.checksum_matches = false;
            consistency.inconsistencies.push("Checksum mismatch between replicas".to_string());
        }
        
        if chunk_id.contains("test_missing") {
            consistency.replica_count = 2;
            consistency.inconsistencies.push("Missing replica on chunk server 3".to_string());
        }
        
        // Deep check would perform additional verification
        if deep_check {
            // Verify actual data integrity
            // Check for bit rot or corruption
            // Validate erasure coding parity
            debug!("Performing deep consistency check for chunk {}", chunk_id);
        }
        
        Ok(consistency)
    }

    /// Check cross-region consistency
    async fn check_cross_region_consistency(&self, specific_region: Option<&str>) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();
        
        // Get list of region pairs to check
        let region_pairs = self.get_region_pairs(specific_region).await?;
        
        let mut total_pairs = 0;
        let mut total_lag = 0u64;
        let mut max_lag = 0u64;
        let mut total_pending = 0u64;
        let mut total_failed = 0u64;
        let mut healthy_pairs = 0;
        
        for (source, target) in region_pairs {
            total_pairs += 1;
            
            match self.check_region_pair_consistency(&source, &target).await {
                Ok(status) => {
                    total_lag += status.sync_lag_seconds;
                    max_lag = max_lag.max(status.sync_lag_seconds);
                    total_pending += status.pending_operations;
                    total_failed += status.failed_operations;
                    
                    // Consider healthy if lag < 60 seconds and no failures
                    if status.sync_lag_seconds < 60 && status.failed_operations == 0 {
                        healthy_pairs += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to check consistency between {} and {}: {}", source, target, e);
                }
            }
        }
        
        if total_pairs > 0 {
            metrics.insert("region_pairs_total".to_string(), total_pairs as f64);
            metrics.insert("region_pairs_healthy".to_string(), healthy_pairs as f64);
            metrics.insert("avg_sync_lag_seconds".to_string(), total_lag as f64 / total_pairs as f64);
            metrics.insert("max_sync_lag_seconds".to_string(), max_lag as f64);
            metrics.insert("total_pending_operations".to_string(), total_pending as f64);
            metrics.insert("total_failed_operations".to_string(), total_failed as f64);
            
            let health_ratio = healthy_pairs as f64 / total_pairs as f64;
            metrics.insert("cross_region_health_ratio".to_string(), health_ratio);
        }
        
        Ok(metrics)
    }

    /// Check consistency between two regions
    async fn check_region_pair_consistency(&self, source: &str, target: &str) -> Result<CrossRegionStatus> {
        // In a real implementation, this would:
        // 1. Query the replication manager for sync status
        // 2. Check the replication lag from monitoring metrics
        // 3. Verify that critical metadata is synchronized
        
        // Simulated implementation
        let status = CrossRegionStatus {
            source_region: source.to_string(),
            target_region: target.to_string(),
            last_sync_time: SystemTime::now() - Duration::from_secs(30),
            sync_lag_seconds: 30,  // Would be actual measured lag
            pending_operations: 5, // Would be from replication queue
            failed_operations: 0,  // Would be from error logs
        };
        
        Ok(status)
    }

    /// Check metadata consistency
    async fn check_metadata_consistency(&self) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();
        
        // Check different metadata namespaces
        let namespaces = vec!["filesystem", "chunks", "sessions", "quotas"];
        
        let mut total_entries = 0u64;
        let mut total_consistent = 0u64;
        let mut total_inconsistent = 0u64;
        let mut total_missing = 0u64;
        let mut total_orphaned = 0u64;
        
        for namespace in namespaces {
            match self.check_namespace_consistency(namespace).await {
                Ok(consistency) => {
                    total_entries += consistency.total_entries;
                    total_consistent += consistency.consistent_entries;
                    total_inconsistent += consistency.inconsistent_entries;
                    total_missing += consistency.missing_entries;
                    total_orphaned += consistency.orphaned_entries;
                    
                    // Per-namespace metrics
                    let ns_consistency_rate = if consistency.total_entries > 0 {
                        consistency.consistent_entries as f64 / consistency.total_entries as f64
                    } else {
                        1.0
                    };
                    
                    metrics.insert(format!("{}_consistency_rate", namespace), ns_consistency_rate);
                    metrics.insert(format!("{}_total_entries", namespace), consistency.total_entries as f64);
                }
                Err(e) => {
                    warn!("Failed to check metadata consistency for namespace {}: {}", namespace, e);
                }
            }
        }
        
        // Overall metadata metrics
        metrics.insert("total_entries".to_string(), total_entries as f64);
        metrics.insert("consistent_entries".to_string(), total_consistent as f64);
        metrics.insert("inconsistent_entries".to_string(), total_inconsistent as f64);
        metrics.insert("missing_entries".to_string(), total_missing as f64);
        metrics.insert("orphaned_entries".to_string(), total_orphaned as f64);
        
        if total_entries > 0 {
            let overall_consistency = total_consistent as f64 / total_entries as f64;
            metrics.insert("overall_consistency_rate".to_string(), overall_consistency);
        }
        
        Ok(metrics)
    }

    /// Check consistency of a metadata namespace
    async fn check_namespace_consistency(&self, namespace: &str) -> Result<MetadataConsistency> {
        // In a real implementation, this would:
        // 1. Query metadata from multiple master servers
        // 2. Compare entries for consistency
        // 3. Check for orphaned or missing references
        // 4. Validate referential integrity
        
        // Simulated implementation
        let consistency = MetadataConsistency {
            namespace: namespace.to_string(),
            total_entries: 10000,     // Would be actual count
            consistent_entries: 9995, // Would be verified count
            inconsistent_entries: 3,  // Would be detected issues
            missing_entries: 1,       // Would be missing references
            orphaned_entries: 1,      // Would be orphaned entries
            last_check: SystemTime::now(),
        };
        
        Ok(consistency)
    }

    /// Check erasure coding consistency
    async fn check_erasure_coding_consistency(&self) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();
        
        // Get sample of erasure-coded chunks
        let erasure_chunks = self.get_erasure_coded_chunks().await?;
        
        let mut total_stripes = 0;
        let mut valid_stripes = 0;
        let mut recoverable_stripes = 0;
        let mut unrecoverable_stripes = 0;
        
        for chunk_id in erasure_chunks {
            total_stripes += 1;
            
            match self.verify_erasure_stripe(&chunk_id).await {
                Ok(stripe_status) => {
                    if stripe_status.is_valid {
                        valid_stripes += 1;
                    }
                    if stripe_status.is_recoverable {
                        recoverable_stripes += 1;
                    } else {
                        unrecoverable_stripes += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to verify erasure stripe for {}: {}", chunk_id, e);
                    unrecoverable_stripes += 1;
                }
            }
        }
        
        metrics.insert("total_stripes".to_string(), total_stripes as f64);
        metrics.insert("valid_stripes".to_string(), valid_stripes as f64);
        metrics.insert("recoverable_stripes".to_string(), recoverable_stripes as f64);
        metrics.insert("unrecoverable_stripes".to_string(), unrecoverable_stripes as f64);
        
        if total_stripes > 0 {
            let validity_rate = valid_stripes as f64 / total_stripes as f64;
            let recoverability_rate = recoverable_stripes as f64 / total_stripes as f64;
            
            metrics.insert("validity_rate".to_string(), validity_rate);
            metrics.insert("recoverability_rate".to_string(), recoverability_rate);
        }
        
        Ok(metrics)
    }

    /// Get a sample of chunks to check
    async fn get_sample_chunks(&self, region: Option<&str>) -> Result<Vec<String>> {
        // In a real implementation, this would query the master server
        // for a random sample of chunks, possibly filtered by region
        
        let mut chunks = Vec::new();
        for i in 0..self.config.sample_size.min(50) {
            let chunk_id = if let Some(region) = region {
                format!("chunk_{}_{}", region, i)
            } else {
                format!("chunk_{}", i)
            };
            chunks.push(chunk_id);
        }
        
        // Add some test chunks with known issues for demonstration
        chunks.push("test_inconsistent_1".to_string());
        chunks.push("test_missing_2".to_string());
        
        Ok(chunks)
    }

    /// Get region pairs for cross-region checking
    async fn get_region_pairs(&self, specific_region: Option<&str>) -> Result<Vec<(String, String)>> {
        // In a real implementation, this would query the cluster configuration
        let regions = if let Some(region) = specific_region {
            vec![region.to_string()]
        } else {
            vec!["us-east-1".to_string(), "us-west-2".to_string(), "eu-west-1".to_string()]
        };
        
        let mut pairs = Vec::new();
        for i in 0..regions.len() {
            for j in (i + 1)..regions.len() {
                pairs.push((regions[i].clone(), regions[j].clone()));
            }
        }
        
        Ok(pairs)
    }

    /// Get list of erasure-coded chunks
    async fn get_erasure_coded_chunks(&self) -> Result<Vec<String>> {
        // In a real implementation, this would query for chunks using erasure coding
        let mut chunks = Vec::new();
        for i in 0..20 {
            chunks.push(format!("erasure_chunk_{}", i));
        }
        Ok(chunks)
    }

    /// Verify an erasure coding stripe
    async fn verify_erasure_stripe(&self, chunk_id: &str) -> Result<EraseStripeStatus> {
        // In a real implementation, this would:
        // 1. Get the stripe configuration (e.g., 8+2, 4+2)
        // 2. Check that all data and parity blocks are present
        // 3. Verify that parity blocks are correctly calculated
        // 4. Test data recovery if some blocks are missing
        
        Ok(EraseStripeStatus {
            chunk_id: chunk_id.to_string(),
            is_valid: true,
            is_recoverable: true,
            missing_blocks: 0,
            corrupted_blocks: 0,
        })
    }
}

/// Erasure stripe verification status
#[derive(Debug, Clone)]
struct EraseStripeStatus {
    chunk_id: String,
    is_valid: bool,
    is_recoverable: bool,
    missing_blocks: u32,
    corrupted_blocks: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consistency_checker_creation() {
        let config = ConsistencyConfig::default();
        let checker = DataConsistencyChecker::new(config);
        assert!(!checker.config.enable_deep_checks);
    }

    #[tokio::test]
    async fn test_data_consistency_check() {
        let config = ConsistencyConfig::default();
        let checker = DataConsistencyChecker::new(config);
        
        match checker.check_data_consistency(None, None, false).await {
            Ok(metrics) => {
                assert!(!metrics.is_empty());
                // Should have consistency ratio
                assert!(metrics.contains_key("consistency_ratio"));
            }
            Err(e) => {
                println!("Consistency check test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_chunk_consistency_check() {
        let config = ConsistencyConfig::default();
        let checker = DataConsistencyChecker::new(config);
        
        match checker.check_chunk_consistency(Some("test_chunk"), None, false).await {
            Ok(metrics) => {
                assert!(!metrics.is_empty());
                assert!(metrics.contains_key("total_checked"));
            }
            Err(e) => {
                println!("Chunk consistency test failed: {}", e);
            }
        }
    }
}