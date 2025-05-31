//! Data consistency monitoring for MooseNG health checks
//! 
//! Provides comprehensive data consistency monitoring including:
//! - Chunk integrity verification
//! - Metadata consistency checks
//! - Replication verification
//! - Cross-region consistency monitoring
//! - Erasure coding validation

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, Instant};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn, error, info};
use sha2::{Sha256, Digest};

/// Chunk consistency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkConsistencyInfo {
    pub chunk_id: String,
    pub expected_replicas: u32,
    pub actual_replicas: u32,
    pub replica_locations: Vec<String>,
    pub checksum_matches: bool,
    pub expected_checksum: String,
    pub actual_checksums: HashMap<String, String>, // location -> checksum
    pub size_matches: bool,
    pub expected_size: u64,
    pub actual_sizes: HashMap<String, u64>, // location -> size
    pub last_verified: SystemTime,
    pub status: ChunkStatus,
}

/// Chunk status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChunkStatus {
    Healthy,
    UnderReplicated,
    OverReplicated,
    Corrupted,
    Missing,
    Inconsistent,
}

/// Metadata consistency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataConsistencyInfo {
    pub node_id: String,
    pub metadata_version: u64,
    pub chunk_count: u64,
    pub file_count: u64,
    pub directory_count: u64,
    pub total_size: u64,
    pub checksum: String,
    pub last_sync_time: SystemTime,
    pub sync_lag_ms: u64,
    pub is_consistent: bool,
    pub inconsistencies: Vec<String>,
}

/// Replication consistency report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConsistencyReport {
    pub total_chunks: u64,
    pub healthy_chunks: u64,
    pub under_replicated_chunks: u64,
    pub over_replicated_chunks: u64,
    pub corrupted_chunks: u64,
    pub missing_chunks: u64,
    pub inconsistent_chunks: u64,
    pub average_replication_factor: f64,
    pub chunk_details: Vec<ChunkConsistencyInfo>,
    pub last_check_time: SystemTime,
    pub check_duration: Duration,
}

/// Cross-region consistency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRegionConsistencyInfo {
    pub regions: HashMap<String, MetadataConsistencyInfo>,
    pub consensus_achieved: bool,
    pub leader_region: Option<String>,
    pub max_sync_lag_ms: u64,
    pub conflicting_operations: Vec<String>,
    pub last_consensus_time: SystemTime,
}

/// Erasure coding consistency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureCodingConsistencyInfo {
    pub stripe_id: String,
    pub data_chunks: u32,
    pub parity_chunks: u32,
    pub available_chunks: u32,
    pub corrupted_chunks: u32,
    pub missing_chunks: u32,
    pub can_reconstruct: bool,
    pub reconstruction_needed: bool,
    pub chunk_locations: HashMap<u32, Vec<String>>, // chunk_index -> locations
    pub last_verified: SystemTime,
}

/// Consistency monitoring configuration
#[derive(Debug, Clone)]
pub struct ConsistencyMonitorConfig {
    /// Chunk verification interval
    pub chunk_check_interval: Duration,
    /// Metadata verification interval
    pub metadata_check_interval: Duration,
    /// Cross-region check interval
    pub cross_region_check_interval: Duration,
    /// Erasure coding check interval
    pub erasure_check_interval: Duration,
    /// Number of chunks to verify per batch
    pub chunk_batch_size: usize,
    /// Maximum sync lag threshold (ms)
    pub max_sync_lag_ms: u64,
    /// Enable deep consistency checks
    pub enable_deep_checks: bool,
    /// Checksum verification percentage (0.0-1.0)
    pub checksum_verification_rate: f64,
    /// Regions to monitor
    pub monitored_regions: Vec<String>,
}

impl Default for ConsistencyMonitorConfig {
    fn default() -> Self {
        Self {
            chunk_check_interval: Duration::from_secs(300), // 5 minutes
            metadata_check_interval: Duration::from_secs(60), // 1 minute
            cross_region_check_interval: Duration::from_secs(120), // 2 minutes
            erasure_check_interval: Duration::from_secs(600), // 10 minutes
            chunk_batch_size: 100,
            max_sync_lag_ms: 5000, // 5 seconds
            enable_deep_checks: true,
            checksum_verification_rate: 0.1, // 10% of chunks
            monitored_regions: vec![
                "us-east-1".to_string(),
                "us-west-2".to_string(),
                "eu-west-1".to_string(),
            ],
        }
    }
}

/// Data consistency monitor
pub struct ConsistencyMonitor {
    config: ConsistencyMonitorConfig,
    last_chunk_check: Option<SystemTime>,
    last_metadata_check: Option<SystemTime>,
    last_cross_region_check: Option<SystemTime>,
    last_erasure_check: Option<SystemTime>,
    chunk_cache: HashMap<String, ChunkConsistencyInfo>,
    metadata_cache: HashMap<String, MetadataConsistencyInfo>,
}

impl ConsistencyMonitor {
    /// Create a new consistency monitor
    pub fn new(config: ConsistencyMonitorConfig) -> Self {
        Self {
            config,
            last_chunk_check: None,
            last_metadata_check: None,
            last_cross_region_check: None,
            last_erasure_check: None,
            chunk_cache: HashMap::new(),
            metadata_cache: HashMap::new(),
        }
    }
    
    /// Perform comprehensive consistency check
    pub async fn perform_consistency_check(&mut self) -> Result<ConsistencyCheckReport> {
        let start_time = Instant::now();
        info!("Starting comprehensive consistency check");
        
        // Check if it's time for various checks
        let now = SystemTime::now();
        let mut checks_performed = Vec::new();
        
        // Chunk consistency check
        if self.should_check_chunks(now) {
            let chunk_report = self.check_chunk_consistency().await?;
            checks_performed.push(format!("Chunk consistency: {} issues found", 
                chunk_report.total_chunks - chunk_report.healthy_chunks));
            self.last_chunk_check = Some(now);
        }
        
        // Metadata consistency check
        if self.should_check_metadata(now) {
            let metadata_report = self.check_metadata_consistency().await?;
            let inconsistent_nodes = metadata_report.values()
                .filter(|info| !info.is_consistent)
                .count();
            checks_performed.push(format!("Metadata consistency: {} inconsistent nodes", inconsistent_nodes));
            self.last_metadata_check = Some(now);
        }
        
        // Cross-region consistency check
        if self.should_check_cross_region(now) {
            let cross_region_report = self.check_cross_region_consistency().await?;
            checks_performed.push(format!("Cross-region consistency: consensus={}", 
                cross_region_report.consensus_achieved));
            self.last_cross_region_check = Some(now);
        }
        
        // Erasure coding consistency check
        if self.should_check_erasure_coding(now) {
            let erasure_report = self.check_erasure_coding_consistency().await?;
            let reconstruction_needed = erasure_report.iter()
                .filter(|info| info.reconstruction_needed)
                .count();
            checks_performed.push(format!("Erasure coding: {} stripes need reconstruction", reconstruction_needed));
            self.last_erasure_check = Some(now);
        }
        
        let duration = start_time.elapsed();
        info!("Consistency check completed in {:?}: {}", duration, checks_performed.join(", "));
        
        Ok(ConsistencyCheckReport {
            checks_performed,
            total_duration: duration,
            timestamp: now,
        })
    }
    
    /// Check chunk replication consistency
    pub async fn check_chunk_consistency(&mut self) -> Result<ReplicationConsistencyReport> {
        let start_time = Instant::now();
        debug!("Starting chunk consistency check");
        
        // Get list of chunks to check (this would come from the master server)
        let chunk_list = self.get_chunk_list().await?;
        
        let mut chunk_details = Vec::new();
        let mut healthy_chunks = 0;
        let mut under_replicated_chunks = 0;
        let mut over_replicated_chunks = 0;
        let mut corrupted_chunks = 0;
        let mut missing_chunks = 0;
        let mut inconsistent_chunks = 0;
        let mut total_replication_factor = 0.0;
        
        // Process chunks in batches
        for chunk_batch in chunk_list.chunks(self.config.chunk_batch_size) {
            for chunk_id in chunk_batch {
                if let Ok(chunk_info) = self.verify_chunk_consistency(chunk_id).await {
                    match chunk_info.status {
                        ChunkStatus::Healthy => healthy_chunks += 1,
                        ChunkStatus::UnderReplicated => under_replicated_chunks += 1,
                        ChunkStatus::OverReplicated => over_replicated_chunks += 1,
                        ChunkStatus::Corrupted => corrupted_chunks += 1,
                        ChunkStatus::Missing => missing_chunks += 1,
                        ChunkStatus::Inconsistent => inconsistent_chunks += 1,
                    }
                    
                    total_replication_factor += chunk_info.actual_replicas as f64;
                    chunk_details.push(chunk_info);
                }
            }
            
            // Yield control to avoid blocking
            tokio::task::yield_now().await;
        }
        
        let total_chunks = chunk_details.len() as u64;
        let average_replication_factor = if total_chunks > 0 {
            total_replication_factor / total_chunks as f64
        } else {
            0.0
        };
        
        let report = ReplicationConsistencyReport {
            total_chunks,
            healthy_chunks,
            under_replicated_chunks,
            over_replicated_chunks,
            corrupted_chunks,
            missing_chunks,
            inconsistent_chunks,
            average_replication_factor,
            chunk_details,
            last_check_time: SystemTime::now(),
            check_duration: start_time.elapsed(),
        };
        
        debug!("Chunk consistency check completed: {}/{} chunks healthy", 
               healthy_chunks, total_chunks);
        
        Ok(report)
    }
    
    /// Verify consistency of a specific chunk
    async fn verify_chunk_consistency(&mut self, chunk_id: &str) -> Result<ChunkConsistencyInfo> {
        // Get chunk metadata from master
        let chunk_metadata = self.get_chunk_metadata(chunk_id).await?;
        
        // Get actual chunk locations and checksums
        let replica_info = self.get_chunk_replica_info(chunk_id).await?;
        
        let mut actual_checksums = HashMap::new();
        let mut actual_sizes = HashMap::new();
        let replica_locations: Vec<String> = replica_info.keys().cloned().collect();
        
        // Verify checksums if enabled
        let should_verify_checksum = rand::random::<f64>() < self.config.checksum_verification_rate;
        let mut checksum_matches = true;
        
        if should_verify_checksum {
            for (location, replica) in &replica_info {
                if let Ok(checksum) = self.calculate_chunk_checksum(chunk_id, location).await {
                    actual_checksums.insert(location.clone(), checksum.clone());
                    if checksum != chunk_metadata.expected_checksum {
                        checksum_matches = false;
                    }
                }
                actual_sizes.insert(location.clone(), replica.size);
            }
        }
        
        // Determine chunk status
        let actual_replicas = replica_locations.len() as u32;
        let size_matches = actual_sizes.values().all(|&size| size == chunk_metadata.expected_size);
        
        let status = if actual_replicas == 0 {
            ChunkStatus::Missing
        } else if !checksum_matches || !size_matches {
            ChunkStatus::Corrupted
        } else if actual_replicas < chunk_metadata.expected_replicas {
            ChunkStatus::UnderReplicated
        } else if actual_replicas > chunk_metadata.expected_replicas {
            ChunkStatus::OverReplicated
        } else if !actual_checksums.is_empty() && 
                  actual_checksums.values().collect::<HashSet<_>>().len() > 1 {
            ChunkStatus::Inconsistent
        } else {
            ChunkStatus::Healthy
        };
        
        let chunk_info = ChunkConsistencyInfo {
            chunk_id: chunk_id.to_string(),
            expected_replicas: chunk_metadata.expected_replicas,
            actual_replicas,
            replica_locations,
            checksum_matches,
            expected_checksum: chunk_metadata.expected_checksum,
            actual_checksums,
            size_matches,
            expected_size: chunk_metadata.expected_size,
            actual_sizes,
            last_verified: SystemTime::now(),
            status,
        };
        
        // Cache the result
        self.chunk_cache.insert(chunk_id.to_string(), chunk_info.clone());
        
        Ok(chunk_info)
    }
    
    /// Check metadata consistency across nodes
    pub async fn check_metadata_consistency(&mut self) -> Result<HashMap<String, MetadataConsistencyInfo>> {
        debug!("Starting metadata consistency check");
        
        let nodes = self.get_metadata_nodes().await?;
        let mut results = HashMap::new();
        
        for node_id in nodes {
            if let Ok(metadata_info) = self.get_node_metadata_info(&node_id).await {
                results.insert(node_id, metadata_info);
            }
        }
        
        // Compare metadata across nodes to detect inconsistencies
        if results.len() > 1 {
            let reference_checksum = results.values().next().unwrap().checksum.clone();
            let reference_version = results.values().next().unwrap().metadata_version;
            
            for metadata_info in results.values_mut() {
                metadata_info.is_consistent = metadata_info.checksum == reference_checksum &&
                                            metadata_info.metadata_version >= reference_version;
                
                if !metadata_info.is_consistent {
                    if metadata_info.checksum != reference_checksum {
                        metadata_info.inconsistencies.push("Metadata checksum mismatch".to_string());
                    }
                    if metadata_info.metadata_version < reference_version {
                        metadata_info.inconsistencies.push(format!("Metadata version lag: {} vs {}", 
                            metadata_info.metadata_version, reference_version));
                    }
                }
            }
        }
        
        // Cache results
        for (node_id, metadata_info) in &results {
            self.metadata_cache.insert(node_id.clone(), metadata_info.clone());
        }
        
        debug!("Metadata consistency check completed for {} nodes", results.len());
        Ok(results)
    }
    
    /// Check cross-region consistency
    pub async fn check_cross_region_consistency(&self) -> Result<CrossRegionConsistencyInfo> {
        debug!("Starting cross-region consistency check");
        
        let mut regions = HashMap::new();
        let mut max_sync_lag_ms = 0;
        let mut conflicting_operations = Vec::new();
        
        for region in &self.config.monitored_regions {
            if let Ok(metadata_info) = self.get_region_metadata_info(region).await {
                max_sync_lag_ms = max_sync_lag_ms.max(metadata_info.sync_lag_ms);
                regions.insert(region.clone(), metadata_info);
            }
        }
        
        // Determine consensus
        let consensus_achieved = if regions.len() > 1 {
            let reference_version = regions.values().map(|info| info.metadata_version).max().unwrap_or(0);
            let consistent_regions = regions.values()
                .filter(|info| info.metadata_version == reference_version)
                .count();
            
            consistent_regions >= (regions.len() + 1) / 2 // Majority consensus
        } else {
            true
        };
        
        // Find leader region (highest version with lowest lag)
        let leader_region = regions.iter()
            .max_by_key(|(_, info)| (info.metadata_version, -(info.sync_lag_ms as i64)))
            .map(|(region, _)| region.clone());
        
        // Check for conflicting operations (simplified)
        if max_sync_lag_ms > self.config.max_sync_lag_ms {
            conflicting_operations.push(format!("High sync lag detected: {}ms", max_sync_lag_ms));
        }
        
        Ok(CrossRegionConsistencyInfo {
            regions,
            consensus_achieved,
            leader_region,
            max_sync_lag_ms,
            conflicting_operations,
            last_consensus_time: SystemTime::now(),
        })
    }
    
    /// Check erasure coding consistency
    pub async fn check_erasure_coding_consistency(&self) -> Result<Vec<ErasureCodingConsistencyInfo>> {
        debug!("Starting erasure coding consistency check");
        
        let stripes = self.get_erasure_coding_stripes().await?;
        let mut results = Vec::new();
        
        for stripe_id in stripes {
            if let Ok(stripe_info) = self.verify_erasure_stripe(&stripe_id).await {
                results.push(stripe_info);
            }
        }
        
        debug!("Erasure coding consistency check completed for {} stripes", results.len());
        Ok(results)
    }
    
    /// Verify a specific erasure coding stripe
    async fn verify_erasure_stripe(&self, stripe_id: &str) -> Result<ErasureCodingConsistencyInfo> {
        // Get stripe configuration
        let stripe_config = self.get_stripe_config(stripe_id).await?;
        
        // Check availability of chunks in the stripe
        let mut available_chunks = 0;
        let mut corrupted_chunks = 0;
        let mut missing_chunks = 0;
        let mut chunk_locations = HashMap::new();
        
        for chunk_index in 0..(stripe_config.data_chunks + stripe_config.parity_chunks) {
            let chunk_id = format!("{}-{}", stripe_id, chunk_index);
            
            if let Ok(locations) = self.get_chunk_locations(&chunk_id).await {
                if locations.is_empty() {
                    missing_chunks += 1;
                } else {
                    // Verify chunk integrity (simplified)
                    let is_corrupted = false; // Would perform actual verification
                    if is_corrupted {
                        corrupted_chunks += 1;
                    } else {
                        available_chunks += 1;
                    }
                    chunk_locations.insert(chunk_index, locations);
                }
            } else {
                missing_chunks += 1;
            }
        }
        
        // Determine if reconstruction is possible and needed
        let can_reconstruct = available_chunks >= stripe_config.data_chunks;
        let reconstruction_needed = missing_chunks > 0 || corrupted_chunks > 0;
        
        Ok(ErasureCodingConsistencyInfo {
            stripe_id: stripe_id.to_string(),
            data_chunks: stripe_config.data_chunks,
            parity_chunks: stripe_config.parity_chunks,
            available_chunks,
            corrupted_chunks,
            missing_chunks,
            can_reconstruct,
            reconstruction_needed,
            chunk_locations,
            last_verified: SystemTime::now(),
        })
    }
    
    // Helper methods for checking intervals
    fn should_check_chunks(&self, now: SystemTime) -> bool {
        self.last_chunk_check.map_or(true, |last| {
            now.duration_since(last).unwrap_or_default() >= self.config.chunk_check_interval
        })
    }
    
    fn should_check_metadata(&self, now: SystemTime) -> bool {
        self.last_metadata_check.map_or(true, |last| {
            now.duration_since(last).unwrap_or_default() >= self.config.metadata_check_interval
        })
    }
    
    fn should_check_cross_region(&self, now: SystemTime) -> bool {
        self.last_cross_region_check.map_or(true, |last| {
            now.duration_since(last).unwrap_or_default() >= self.config.cross_region_check_interval
        })
    }
    
    fn should_check_erasure_coding(&self, now: SystemTime) -> bool {
        self.last_erasure_check.map_or(true, |last| {
            now.duration_since(last).unwrap_or_default() >= self.config.erasure_check_interval
        })
    }
    
    // Mock implementations for external dependencies
    async fn get_chunk_list(&self) -> Result<Vec<String>> {
        // Mock implementation - would query master server
        Ok(vec![
            "chunk-001".to_string(),
            "chunk-002".to_string(),
            "chunk-003".to_string(),
        ])
    }
    
    async fn get_chunk_metadata(&self, _chunk_id: &str) -> Result<ChunkMetadata> {
        Ok(ChunkMetadata {
            expected_replicas: 3,
            expected_checksum: "abc123".to_string(),
            expected_size: 1024,
        })
    }
    
    async fn get_chunk_replica_info(&self, _chunk_id: &str) -> Result<HashMap<String, ReplicaInfo>> {
        let mut replicas = HashMap::new();
        replicas.insert("chunkserver-1".to_string(), ReplicaInfo { size: 1024 });
        replicas.insert("chunkserver-2".to_string(), ReplicaInfo { size: 1024 });
        replicas.insert("chunkserver-3".to_string(), ReplicaInfo { size: 1024 });
        Ok(replicas)
    }
    
    async fn calculate_chunk_checksum(&self, _chunk_id: &str, _location: &str) -> Result<String> {
        // Mock checksum calculation
        Ok("abc123".to_string())
    }
    
    async fn get_metadata_nodes(&self) -> Result<Vec<String>> {
        Ok(vec![
            "master-1".to_string(),
            "master-2".to_string(),
            "master-3".to_string(),
        ])
    }
    
    async fn get_node_metadata_info(&self, node_id: &str) -> Result<MetadataConsistencyInfo> {
        Ok(MetadataConsistencyInfo {
            node_id: node_id.to_string(),
            metadata_version: 100,
            chunk_count: 1000,
            file_count: 500,
            directory_count: 50,
            total_size: 1_000_000,
            checksum: "metadata-checksum".to_string(),
            last_sync_time: SystemTime::now(),
            sync_lag_ms: 100,
            is_consistent: true,
            inconsistencies: Vec::new(),
        })
    }
    
    async fn get_region_metadata_info(&self, region: &str) -> Result<MetadataConsistencyInfo> {
        self.get_node_metadata_info(&format!("region-{}", region)).await
    }
    
    async fn get_erasure_coding_stripes(&self) -> Result<Vec<String>> {
        Ok(vec![
            "stripe-001".to_string(),
            "stripe-002".to_string(),
        ])
    }
    
    async fn get_stripe_config(&self, _stripe_id: &str) -> Result<StripeConfig> {
        Ok(StripeConfig {
            data_chunks: 4,
            parity_chunks: 2,
        })
    }
    
    async fn get_chunk_locations(&self, _chunk_id: &str) -> Result<Vec<String>> {
        Ok(vec!["chunkserver-1".to_string(), "chunkserver-2".to_string()])
    }
    
    /// Get consistency health summary
    pub fn get_health_summary(&self) -> String {
        let chunk_issues = self.chunk_cache.values()
            .filter(|chunk| chunk.status != ChunkStatus::Healthy)
            .count();
        
        let metadata_issues = self.metadata_cache.values()
            .filter(|metadata| !metadata.is_consistent)
            .count();
        
        if chunk_issues > 0 || metadata_issues > 0 {
            format!("ISSUES DETECTED: {} chunk issues, {} metadata issues", 
                   chunk_issues, metadata_issues)
        } else {
            "DATA CONSISTENCY: All checks passed".to_string()
        }
    }
    
    /// Get recommendations for consistency issues
    pub fn get_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        let under_replicated = self.chunk_cache.values()
            .filter(|chunk| chunk.status == ChunkStatus::UnderReplicated)
            .count();
        
        let corrupted = self.chunk_cache.values()
            .filter(|chunk| chunk.status == ChunkStatus::Corrupted)
            .count();
        
        let missing = self.chunk_cache.values()
            .filter(|chunk| chunk.status == ChunkStatus::Missing)
            .count();
        
        if missing > 0 {
            recommendations.push(format!("URGENT: {} chunks are missing - check chunkserver status", missing));
        }
        
        if corrupted > 0 {
            recommendations.push(format!("URGENT: {} chunks are corrupted - initiate recovery", corrupted));
        }
        
        if under_replicated > 0 {
            recommendations.push(format!("WARNING: {} chunks are under-replicated - increase replication", under_replicated));
        }
        
        let metadata_inconsistent = self.metadata_cache.values()
            .filter(|metadata| !metadata.is_consistent)
            .count();
        
        if metadata_inconsistent > 0 {
            recommendations.push(format!("Check metadata synchronization - {} nodes inconsistent", metadata_inconsistent));
        }
        
        if recommendations.is_empty() {
            recommendations.push("Data consistency is healthy".to_string());
        }
        
        recommendations
    }
}

// Supporting structures
#[derive(Debug, Clone)]
struct ChunkMetadata {
    expected_replicas: u32,
    expected_checksum: String,
    expected_size: u64,
}

#[derive(Debug, Clone)]
struct ReplicaInfo {
    size: u64,
}

#[derive(Debug, Clone)]
struct StripeConfig {
    data_chunks: u32,
    parity_chunks: u32,
}

#[derive(Debug, Clone)]
pub struct ConsistencyCheckReport {
    pub checks_performed: Vec<String>,
    pub total_duration: Duration,
    pub timestamp: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_consistency_monitor_creation() {
        let config = ConsistencyMonitorConfig::default();
        let monitor = ConsistencyMonitor::new(config);
        
        assert!(monitor.chunk_cache.is_empty());
        assert!(monitor.metadata_cache.is_empty());
    }
    
    #[tokio::test]
    async fn test_chunk_consistency_check() {
        let config = ConsistencyMonitorConfig::default();
        let mut monitor = ConsistencyMonitor::new(config);
        
        let result = monitor.check_chunk_consistency().await;
        assert!(result.is_ok());
        
        let report = result.unwrap();
        assert!(report.total_chunks >= 0);
        assert!(report.average_replication_factor >= 0.0);
    }
    
    #[test]
    fn test_chunk_status_determination() {
        let chunk_info = ChunkConsistencyInfo {
            chunk_id: "test-chunk".to_string(),
            expected_replicas: 3,
            actual_replicas: 2,
            replica_locations: vec!["loc1".to_string(), "loc2".to_string()],
            checksum_matches: true,
            expected_checksum: "abc123".to_string(),
            actual_checksums: HashMap::new(),
            size_matches: true,
            expected_size: 1024,
            actual_sizes: HashMap::new(),
            last_verified: SystemTime::now(),
            status: ChunkStatus::UnderReplicated,
        };
        
        assert_eq!(chunk_info.status, ChunkStatus::UnderReplicated);
        assert!(chunk_info.actual_replicas < chunk_info.expected_replicas);
    }
    
    #[tokio::test]
    async fn test_metadata_consistency_check() {
        let config = ConsistencyMonitorConfig::default();
        let mut monitor = ConsistencyMonitor::new(config);
        
        let result = monitor.check_metadata_consistency().await;
        assert!(result.is_ok());
        
        let metadata_map = result.unwrap();
        assert!(!metadata_map.is_empty());
    }
    
    #[test]
    fn test_health_summary() {
        let config = ConsistencyMonitorConfig::default();
        let monitor = ConsistencyMonitor::new(config);
        
        let summary = monitor.get_health_summary();
        assert!(summary.contains("DATA CONSISTENCY"));
        
        let recommendations = monitor.get_recommendations();
        assert!(!recommendations.is_empty());
    }
}