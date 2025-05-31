//! Data consistency monitoring for MooseNG
//!
//! Provides comprehensive data consistency checking including:
//! - Chunk replica verification across servers
//! - Cross-region data consistency validation
//! - Erasure coding stripe verification
//! - Metadata consistency checks
//! - Automatic inconsistency detection and reporting

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn, error, info};

/// Data consistency checker
pub struct DataConsistencyChecker {
    config: ConsistencyConfig,
    last_check_results: Option<ConsistencyReport>,
}

/// Configuration for consistency checking
#[derive(Debug, Clone)]
pub struct ConsistencyConfig {
    /// Enable deep consistency checks (slower but more thorough)
    pub enable_deep_checks: bool,
    /// Number of chunks to sample for consistency checks
    pub sample_size: usize,
    /// Timeout for individual chunk checks
    pub chunk_check_timeout: Duration,
    /// Enable cross-region consistency checks
    pub check_cross_region: bool,
    /// Enable erasure coding verification
    pub verify_erasure_coding: bool,
    /// Enable metadata consistency checks
    pub check_metadata_consistency: bool,
    /// Parallelism level for consistency checks
    pub parallelism: usize,
}

impl Default for ConsistencyConfig {
    fn default() -> Self {
        Self {
            enable_deep_checks: false,
            sample_size: 1000,
            chunk_check_timeout: Duration::from_secs(10),
            check_cross_region: true,
            verify_erasure_coding: true,
            check_metadata_consistency: true,
            parallelism: 4,
        }
    }
}

/// Overall consistency report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyReport {
    pub timestamp: SystemTime,
    pub total_chunks_checked: u64,
    pub consistent_chunks: u64,
    pub inconsistent_chunks: u64,
    pub missing_replicas: u64,
    pub extra_replicas: u64,
    pub corrupted_chunks: u64,
    pub consistency_ratio: f64,
    pub regions_checked: Vec<String>,
    pub chunk_issues: Vec<ChunkIssue>,
    pub metadata_issues: Vec<MetadataIssue>,
    pub erasure_coding_issues: Vec<ErasureCodingIssue>,
    pub check_duration: Duration,
    pub deep_check_performed: bool,
}

/// Individual chunk consistency issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkIssue {
    pub chunk_id: String,
    pub issue_type: ChunkIssueType,
    pub description: String,
    pub affected_servers: Vec<String>,
    pub affected_regions: Vec<String>,
    pub severity: IssueSeverity,
    pub detected_at: SystemTime,
}

/// Types of chunk issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkIssueType {
    MissingReplica,
    ExtraReplica,
    CorruptedData,
    InconsistentChecksum,
    ErasureCodingMismatch,
    RegionInconsistency,
}

/// Metadata consistency issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataIssue {
    pub issue_type: MetadataIssueType,
    pub description: String,
    pub affected_paths: Vec<String>,
    pub severity: IssueSeverity,
    pub detected_at: SystemTime,
}

/// Types of metadata issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetadataIssueType {
    OrphanedChunk,
    MissingChunk,
    InconsistentAttributes,
    DuplicateInodes,
    BrokenSymlinks,
}

/// Erasure coding consistency issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureCodingIssue {
    pub stripe_id: String,
    pub issue_type: ErasureCodingIssueType,
    pub description: String,
    pub missing_shards: Vec<u32>,
    pub corrupted_shards: Vec<u32>,
    pub severity: IssueSeverity,
    pub detected_at: SystemTime,
}

/// Types of erasure coding issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErasureCodingIssueType {
    InsufficientShards,
    CorruptedShard,
    MissingShard,
    InconsistentStripe,
}

/// Issue severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Low,      // Cosmetic issues, no data loss risk
    Medium,   // Potential performance impact
    High,     // Data integrity risk
    Critical, // Immediate data loss risk
}

/// Chunk information for consistency checking
#[derive(Debug, Clone)]
struct ChunkInfo {
    chunk_id: String,
    expected_replicas: u32,
    actual_replicas: Vec<ReplicaInfo>,
    checksum: Option<String>,
    size: u64,
    erasure_coded: bool,
    stripe_id: Option<String>,
}

/// Individual replica information
#[derive(Debug, Clone)]
struct ReplicaInfo {
    server_id: String,
    region: String,
    checksum: String,
    size: u64,
    last_modified: SystemTime,
}

impl DataConsistencyChecker {
    /// Create a new data consistency checker
    pub fn new(config: ConsistencyConfig) -> Self {
        Self {
            config,
            last_check_results: None,
        }
    }
    
    /// Perform comprehensive data consistency check
    pub async fn check_data_consistency(
        &mut self,
        chunk_id: Option<&str>,
        region: Option<&str>,
        deep_check: bool,
    ) -> Result<HashMap<String, f64>> {
        let start_time = SystemTime::now();
        info!("Starting data consistency check");
        
        // Determine which chunks to check
        let chunks_to_check = if let Some(id) = chunk_id {
            vec![id.to_string()]
        } else {
            self.get_sample_chunks(region).await?
        };
        
        let mut report = ConsistencyReport {
            timestamp: start_time,
            total_chunks_checked: 0,
            consistent_chunks: 0,
            inconsistent_chunks: 0,
            missing_replicas: 0,
            extra_replicas: 0,
            corrupted_chunks: 0,
            consistency_ratio: 0.0,
            regions_checked: Vec::new(),
            chunk_issues: Vec::new(),
            metadata_issues: Vec::new(),
            erasure_coding_issues: Vec::new(),
            check_duration: Duration::from_secs(0),
            deep_check_performed: deep_check || self.config.enable_deep_checks,
        };
        
        // Check individual chunks
        for chunk_id in chunks_to_check {
            match self.check_chunk_consistency(&chunk_id, deep_check).await {
                Ok(chunk_result) => {
                    report.total_chunks_checked += 1;
                    
                    if chunk_result.is_consistent {
                        report.consistent_chunks += 1;
                    } else {
                        report.inconsistent_chunks += 1;
                        report.chunk_issues.extend(chunk_result.issues);
                    }
                    
                    report.missing_replicas += chunk_result.missing_replicas;
                    report.extra_replicas += chunk_result.extra_replicas;
                    if chunk_result.is_corrupted {
                        report.corrupted_chunks += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to check chunk {}: {}", chunk_id, e);
                }
            }
        }
        
        // Perform metadata consistency checks
        if self.config.check_metadata_consistency {
            match self.check_metadata_consistency().await {
                Ok(metadata_issues) => {
                    report.metadata_issues = metadata_issues;
                }
                Err(e) => {
                    error!("Failed to check metadata consistency: {}", e);
                }
            }
        }
        
        // Perform erasure coding checks
        if self.config.verify_erasure_coding {
            match self.check_erasure_coding_consistency().await {
                Ok(erasure_issues) => {
                    report.erasure_coding_issues = erasure_issues;
                }
                Err(e) => {
                    error!("Failed to check erasure coding consistency: {}", e);
                }
            }
        }
        
        // Calculate overall consistency ratio
        report.consistency_ratio = if report.total_chunks_checked > 0 {
            report.consistent_chunks as f64 / report.total_chunks_checked as f64
        } else {
            1.0
        };
        
        report.check_duration = start_time.elapsed().unwrap_or_default();
        
        info!(
            "Consistency check completed: {}/{} chunks consistent ({:.2}%)",
            report.consistent_chunks,
            report.total_chunks_checked,
            report.consistency_ratio * 100.0
        );
        
        self.last_check_results = Some(report.clone());
        
        // Convert to metrics format
        let mut metrics = HashMap::new();
        metrics.insert("total_chunks".to_string(), report.total_chunks_checked as f64);
        metrics.insert("consistent_chunks".to_string(), report.consistent_chunks as f64);
        metrics.insert("inconsistent_chunks".to_string(), report.inconsistent_chunks as f64);
        metrics.insert("missing_replicas".to_string(), report.missing_replicas as f64);
        metrics.insert("extra_replicas".to_string(), report.extra_replicas as f64);
        metrics.insert("corrupted_chunks".to_string(), report.corrupted_chunks as f64);
        metrics.insert("consistency_ratio".to_string(), report.consistency_ratio);
        metrics.insert("metadata_issues_count".to_string(), report.metadata_issues.len() as f64);
        metrics.insert("erasure_coding_issues_count".to_string(), report.erasure_coding_issues.len() as f64);
        metrics.insert("check_duration_seconds".to_string(), report.check_duration.as_secs_f64());
        
        Ok(metrics)
    }
    
    /// Get a sample of chunks to check
    async fn get_sample_chunks(&self, region: Option<&str>) -> Result<Vec<String>> {
        // In a real implementation, this would query the metadata store
        // For now, we'll simulate some chunk IDs
        let mut chunks = Vec::new();
        
        for i in 0..self.config.sample_size {
            chunks.push(format!("chunk_{:06}", i));
        }
        
        // If region is specified, filter chunks for that region
        if let Some(_region) = region {
            // In a real implementation, filter by region
        }
        
        Ok(chunks)
    }
    
    /// Check consistency of a single chunk
    async fn check_chunk_consistency(&self, chunk_id: &str, deep_check: bool) -> Result<ChunkConsistencyResult> {
        let mut result = ChunkConsistencyResult {
            chunk_id: chunk_id.to_string(),
            is_consistent: true,
            is_corrupted: false,
            missing_replicas: 0,
            extra_replicas: 0,
            issues: Vec::new(),
        };
        
        // Get chunk information (simulated)
        let chunk_info = self.get_chunk_info(chunk_id).await?;
        
        // Check replica count
        let expected_replicas = chunk_info.expected_replicas as usize;
        let actual_replicas = chunk_info.actual_replicas.len();
        
        if actual_replicas < expected_replicas {
            result.missing_replicas = (expected_replicas - actual_replicas) as u64;
            result.is_consistent = false;
            
            result.issues.push(ChunkIssue {
                chunk_id: chunk_id.to_string(),
                issue_type: ChunkIssueType::MissingReplica,
                description: format!(
                    "Expected {} replicas, found {}",
                    expected_replicas, actual_replicas
                ),
                affected_servers: chunk_info.actual_replicas.iter()
                    .map(|r| r.server_id.clone())
                    .collect(),
                affected_regions: chunk_info.actual_replicas.iter()
                    .map(|r| r.region.clone())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect(),
                severity: if actual_replicas == 0 {
                    IssueSeverity::Critical
                } else {
                    IssueSeverity::High
                },
                detected_at: SystemTime::now(),
            });
        } else if actual_replicas > expected_replicas {
            result.extra_replicas = (actual_replicas - expected_replicas) as u64;
            result.is_consistent = false;
            
            result.issues.push(ChunkIssue {
                chunk_id: chunk_id.to_string(),
                issue_type: ChunkIssueType::ExtraReplica,
                description: format!(
                    "Expected {} replicas, found {}",
                    expected_replicas, actual_replicas
                ),
                affected_servers: chunk_info.actual_replicas.iter()
                    .map(|r| r.server_id.clone())
                    .collect(),
                affected_regions: chunk_info.actual_replicas.iter()
                    .map(|r| r.region.clone())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect(),
                severity: IssueSeverity::Medium,
                detected_at: SystemTime::now(),
            });
        }
        
        // Check replica checksums
        if deep_check && chunk_info.actual_replicas.len() > 1 {
            let first_checksum = &chunk_info.actual_replicas[0].checksum;
            let first_size = chunk_info.actual_replicas[0].size;
            
            for replica in &chunk_info.actual_replicas[1..] {
                if replica.checksum != *first_checksum || replica.size != first_size {
                    result.is_consistent = false;
                    result.is_corrupted = true;
                    
                    result.issues.push(ChunkIssue {
                        chunk_id: chunk_id.to_string(),
                        issue_type: ChunkIssueType::InconsistentChecksum,
                        description: format!(
                            "Replica checksum mismatch between {} and {}",
                            chunk_info.actual_replicas[0].server_id,
                            replica.server_id
                        ),
                        affected_servers: vec![
                            chunk_info.actual_replicas[0].server_id.clone(),
                            replica.server_id.clone(),
                        ],
                        affected_regions: vec![
                            chunk_info.actual_replicas[0].region.clone(),
                            replica.region.clone(),
                        ],
                        severity: IssueSeverity::Critical,
                        detected_at: SystemTime::now(),
                    });
                }
            }
        }
        
        // Check cross-region consistency
        if self.config.check_cross_region {
            let regions: HashSet<String> = chunk_info.actual_replicas.iter()
                .map(|r| r.region.clone())
                .collect();
            
            if regions.len() > 1 && deep_check {
                // Additional cross-region checks would go here
                // For now, we assume cross-region replicas are consistent if checksums match
            }
        }
        
        Ok(result)
    }
    
    /// Get information about a specific chunk (simulated)
    async fn get_chunk_info(&self, chunk_id: &str) -> Result<ChunkInfo> {
        // In a real implementation, this would query the metadata store
        // For simulation, we'll generate some data
        
        let replicas = vec![
            ReplicaInfo {
                server_id: "server-001".to_string(),
                region: "us-east-1".to_string(),
                checksum: "abcd1234".to_string(),
                size: 1024 * 1024, // 1MB
                last_modified: SystemTime::now(),
            },
            ReplicaInfo {
                server_id: "server-002".to_string(),
                region: "us-west-1".to_string(),
                checksum: "abcd1234".to_string(),
                size: 1024 * 1024, // 1MB
                last_modified: SystemTime::now(),
            },
        ];
        
        Ok(ChunkInfo {
            chunk_id: chunk_id.to_string(),
            expected_replicas: 2,
            actual_replicas: replicas,
            checksum: Some("abcd1234".to_string()),
            size: 1024 * 1024,
            erasure_coded: false,
            stripe_id: None,
        })
    }
    
    /// Check metadata consistency
    async fn check_metadata_consistency(&self) -> Result<Vec<MetadataIssue>> {
        let mut issues = Vec::new();
        
        // Simulate some metadata checks
        // In a real implementation, this would check:
        // - Orphaned chunks (chunks without file references)
        // - Missing chunks (file references without chunks)
        // - Inconsistent file attributes across replicas
        // - Duplicate inodes
        // - Broken symlinks
        
        debug!("Performing metadata consistency checks");
        
        // Example: Check for orphaned chunks
        // This would query the metadata store to find chunks not referenced by any file
        
        Ok(issues)
    }
    
    /// Check erasure coding consistency
    async fn check_erasure_coding_consistency(&self) -> Result<Vec<ErasureCodingIssue>> {
        let mut issues = Vec::new();
        
        if !self.config.verify_erasure_coding {
            return Ok(issues);
        }
        
        debug!("Performing erasure coding consistency checks");
        
        // In a real implementation, this would:
        // - Verify that all shards in a stripe are present
        // - Check that parity shards can reconstruct missing data shards
        // - Validate stripe metadata consistency
        
        Ok(issues)
    }
    
    /// Get the latest consistency report
    pub fn get_latest_report(&self) -> Option<&ConsistencyReport> {
        self.last_check_results.as_ref()
    }
    
    /// Check if the system has critical consistency issues
    pub fn has_critical_issues(&self) -> bool {
        if let Some(report) = &self.last_check_results {
            report.consistency_ratio < 0.95 || 
            report.corrupted_chunks > 0 ||
            report.chunk_issues.iter().any(|issue| matches!(issue.severity, IssueSeverity::Critical)) ||
            report.metadata_issues.iter().any(|issue| matches!(issue.severity, IssueSeverity::Critical)) ||
            report.erasure_coding_issues.iter().any(|issue| matches!(issue.severity, IssueSeverity::Critical))
        } else {
            false
        }
    }
    
    /// Get health summary
    pub fn get_health_summary(&self) -> String {
        if let Some(report) = &self.last_check_results {
            if self.has_critical_issues() {
                format!(
                    "CRITICAL: {:.1}% consistency ratio, {} corrupted chunks",
                    report.consistency_ratio * 100.0,
                    report.corrupted_chunks
                )
            } else if report.inconsistent_chunks > 0 {
                format!(
                    "WARNING: {}/{} chunks have issues",
                    report.inconsistent_chunks,
                    report.total_chunks_checked
                )
            } else {
                format!(
                    "HEALTHY: {:.1}% consistency ratio",
                    report.consistency_ratio * 100.0
                )
            }
        } else {
            "UNKNOWN: No consistency data available".to_string()
        }
    }
}

/// Result of checking a single chunk's consistency
#[derive(Debug)]
struct ChunkConsistencyResult {
    chunk_id: String,
    is_consistent: bool,
    is_corrupted: bool,
    missing_replicas: u64,
    extra_replicas: u64,
    issues: Vec<ChunkIssue>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_consistency_checker_creation() {
        let config = ConsistencyConfig::default();
        let checker = DataConsistencyChecker::new(config);
        
        assert!(checker.last_check_results.is_none());
    }
    
    #[tokio::test]
    async fn test_sample_chunks_generation() {
        let config = ConsistencyConfig {
            sample_size: 10,
            ..Default::default()
        };
        let checker = DataConsistencyChecker::new(config);
        
        let chunks = checker.get_sample_chunks(None).await.unwrap();
        assert_eq!(chunks.len(), 10);
        
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk, &format!("chunk_{:06}", i));
        }
    }
    
    #[tokio::test]
    async fn test_chunk_info_simulation() {
        let checker = DataConsistencyChecker::new(ConsistencyConfig::default());
        
        let chunk_info = checker.get_chunk_info("test_chunk").await.unwrap();
        
        assert_eq!(chunk_info.chunk_id, "test_chunk");
        assert_eq!(chunk_info.expected_replicas, 2);
        assert_eq!(chunk_info.actual_replicas.len(), 2);
        assert!(!chunk_info.erasure_coded);
    }
    
    #[tokio::test]
    async fn test_consistency_check_with_sample() {
        let config = ConsistencyConfig {
            sample_size: 5,
            enable_deep_checks: false,
            ..Default::default()
        };
        let mut checker = DataConsistencyChecker::new(config);
        
        let metrics = checker.check_data_consistency(None, None, false).await.unwrap();
        
        assert!(metrics.contains_key("total_chunks"));
        assert!(metrics.contains_key("consistency_ratio"));
        assert_eq!(metrics["total_chunks"], 5.0);
        
        // Check that we have a report
        assert!(checker.last_check_results.is_some());
        let report = checker.last_check_results.unwrap();
        assert_eq!(report.total_chunks_checked, 5);
    }
    
    #[test]
    fn test_issue_severity_levels() {
        let critical_issue = ChunkIssue {
            chunk_id: "test".to_string(),
            issue_type: ChunkIssueType::CorruptedData,
            description: "Test".to_string(),
            affected_servers: vec![],
            affected_regions: vec![],
            severity: IssueSeverity::Critical,
            detected_at: SystemTime::now(),
        };
        
        assert!(matches!(critical_issue.severity, IssueSeverity::Critical));
    }
}