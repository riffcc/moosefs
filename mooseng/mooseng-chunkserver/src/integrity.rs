use crate::{
    chunk::{Chunk, ChunkMetadata, ChunkChecksum, ChecksumType},
    storage::ChunkStorage,
    error::{ChunkServerError, Result},
};
use mooseng_common::types::{ChunkId, ChunkVersion};
use bytes::Bytes;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use dashmap::DashMap;
use tracing::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};

/// Comprehensive data integrity verification system
pub struct IntegrityVerifier {
    storage: Arc<dyn ChunkStorage>,
    verification_semaphore: Arc<Semaphore>,
    corrupted_chunks: Arc<RwLock<HashSet<(ChunkId, ChunkVersion)>>>,
    repair_queue: Arc<RwLock<Vec<IntegrityIssue>>>,
    metrics: Arc<IntegrityMetrics>,
    config: IntegrityConfig,
}

/// Configuration for integrity verification
#[derive(Debug, Clone)]
pub struct IntegrityConfig {
    /// Maximum concurrent verification operations
    pub max_concurrent_verifications: usize,
    /// How often to run full integrity scans
    pub full_scan_interval: Duration,
    /// Timeout for individual chunk verification
    pub verification_timeout: Duration,
    /// Whether to auto-repair corrupted chunks
    pub auto_repair: bool,
    /// Chunk verification batch size
    pub batch_size: usize,
    /// Use fast verification first (for hybrid checksums)
    pub use_fast_verification: bool,
}

impl Default for IntegrityConfig {
    fn default() -> Self {
        Self {
            max_concurrent_verifications: 16,
            full_scan_interval: Duration::from_secs(24 * 3600), // 24 hours
            verification_timeout: Duration::from_secs(30),
            auto_repair: false,
            batch_size: 100,
            use_fast_verification: true,
        }
    }
}

/// Types of integrity issues that can be detected
#[derive(Debug, Clone, PartialEq)]
pub enum IntegrityIssue {
    ChecksumMismatch {
        chunk_id: ChunkId,
        version: ChunkVersion,
        expected_checksum: String,
        actual_checksum: String,
    },
    SizeMismatch {
        chunk_id: ChunkId,
        version: ChunkVersion,
        expected_size: u64,
        actual_size: u64,
    },
    MissingChunk {
        chunk_id: ChunkId,
        version: ChunkVersion,
    },
    MetadataCorruption {
        chunk_id: ChunkId,
        version: ChunkVersion,
        error: String,
    },
    ReadError {
        chunk_id: ChunkId,
        version: ChunkVersion,
        error: String,
    },
}

/// Metrics for integrity verification operations
#[derive(Debug, Default)]
pub struct IntegrityMetrics {
    pub chunks_verified: std::sync::atomic::AtomicU64,
    pub fast_verifications: std::sync::atomic::AtomicU64,
    pub full_verifications: std::sync::atomic::AtomicU64,
    pub checksum_failures: std::sync::atomic::AtomicU64,
    pub size_mismatches: std::sync::atomic::AtomicU64,
    pub missing_chunks: std::sync::atomic::AtomicU64,
    pub metadata_corruptions: std::sync::atomic::AtomicU64,
    pub read_errors: std::sync::atomic::AtomicU64,
    pub auto_repairs: std::sync::atomic::AtomicU64,
    pub verification_time_total_micros: std::sync::atomic::AtomicU64,
    pub last_full_scan: std::sync::atomic::AtomicU64,
}

/// Result of integrity verification
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub chunk_id: ChunkId,
    pub version: ChunkVersion,
    pub is_valid: bool,
    pub issues: Vec<IntegrityIssue>,
    pub verification_time: Duration,
    pub used_fast_verification: bool,
}

/// Summary of integrity scan results
#[derive(Debug, Clone)]
pub struct IntegrityScanSummary {
    pub total_chunks: u64,
    pub verified_chunks: u64,
    pub corrupted_chunks: u64,
    pub issues_found: Vec<IntegrityIssue>,
    pub scan_duration: Duration,
    pub chunks_per_second: f64,
}

impl IntegrityVerifier {
    /// Create a new integrity verifier
    pub fn new(storage: Arc<dyn ChunkStorage>, config: IntegrityConfig) -> Self {
        Self {
            storage,
            verification_semaphore: Arc::new(Semaphore::new(config.max_concurrent_verifications)),
            corrupted_chunks: Arc::new(RwLock::new(HashSet::new())),
            repair_queue: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(IntegrityMetrics::default()),
            config,
        }
    }

    /// Verify integrity of a single chunk
    pub async fn verify_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<VerificationResult> {
        let start_time = Instant::now();
        let _permit = self.verification_semaphore.acquire().await.map_err(|_| 
            ChunkServerError::InvalidOperation("Failed to acquire verification semaphore".to_string()))?;

        self.metrics.chunks_verified.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let mut issues = Vec::new();
        let mut is_valid = true;
        let mut used_fast_verification = false;

        // First, try to get chunk metadata
        let metadata = match self.storage.get_chunk_metadata(chunk_id, version).await {
            Ok(metadata) => metadata,
            Err(e) => {
                let issue = IntegrityIssue::MetadataCorruption {
                    chunk_id,
                    version,
                    error: e.to_string(),
                };
                issues.push(issue.clone());
                self.record_issue(&issue).await;
                let duration = start_time.elapsed();
                return Ok(VerificationResult {
                    chunk_id,
                    version,
                    is_valid: false,
                    issues,
                    verification_time: duration,
                    used_fast_verification: false,
                });
            }
        };

        // Try fast verification first if configured and supported
        if self.config.use_fast_verification {
            match metadata.checksum.checksum_type {
                ChecksumType::HybridFast | ChecksumType::HybridSecure => {
                    match self.storage.verify_chunk_fast(chunk_id, version).await {
                        Ok(true) => {
                            used_fast_verification = true;
                            self.metrics.fast_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let duration = start_time.elapsed();
                            return Ok(VerificationResult {
                                chunk_id,
                                version,
                                is_valid: true,
                                issues: Vec::new(),
                                verification_time: duration,
                                used_fast_verification: true,
                            });
                        }
                        Ok(false) => {
                            // Fast verification failed, continue with full verification
                            is_valid = false;
                        }
                        Err(_) => {
                            // Fast verification error, continue with full verification
                        }
                    }
                }
                _ => {
                    // Fast verification not supported for this checksum type
                }
            }
        }

        // Full verification
        self.metrics.full_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Try to read the chunk
        let chunk = match self.storage.get_chunk(chunk_id, version).await {
            Ok(chunk) => chunk,
            Err(ChunkServerError::ChunkNotFound { .. }) => {
                let issue = IntegrityIssue::MissingChunk { chunk_id, version };
                issues.push(issue.clone());
                self.record_issue(&issue).await;
                is_valid = false;
                let duration = start_time.elapsed();
                return Ok(VerificationResult {
                    chunk_id,
                    version,
                    is_valid: false,
                    issues,
                    verification_time: duration,
                    used_fast_verification,
                });
            }
            Err(e) => {
                let issue = IntegrityIssue::ReadError {
                    chunk_id,
                    version,
                    error: e.to_string(),
                };
                issues.push(issue.clone());
                self.record_issue(&issue).await;
                is_valid = false;
                let duration = start_time.elapsed();
                return Ok(VerificationResult {
                    chunk_id,
                    version,
                    is_valid: false,
                    issues,
                    verification_time: duration,
                    used_fast_verification,
                });
            }
        };

        // Verify size consistency
        if chunk.data.len() as u64 != metadata.size {
            let issue = IntegrityIssue::SizeMismatch {
                chunk_id,
                version,
                expected_size: metadata.size,
                actual_size: chunk.data.len() as u64,
            };
            issues.push(issue.clone());
            self.record_issue(&issue).await;
            is_valid = false;
        }

        // Verify checksum
        if !chunk.verify_integrity() {
            let issue = IntegrityIssue::ChecksumMismatch {
                chunk_id,
                version,
                expected_checksum: metadata.checksum.hex(),
                actual_checksum: "corrupted".to_string(),
            };
            issues.push(issue.clone());
            self.record_issue(&issue).await;
            is_valid = false;
        }

        // Update corruption tracking
        if !is_valid {
            let mut corrupted = self.corrupted_chunks.write().await;
            corrupted.insert((chunk_id, version));
        } else {
            let mut corrupted = self.corrupted_chunks.write().await;
            corrupted.remove(&(chunk_id, version));
        }

        let duration = start_time.elapsed();
        self.metrics.verification_time_total_micros.fetch_add(
            duration.as_micros() as u64,
            std::sync::atomic::Ordering::Relaxed
        );

        Ok(VerificationResult {
            chunk_id,
            version,
            is_valid,
            issues,
            verification_time: duration,
            used_fast_verification,
        })
    }

    /// Verify integrity of multiple chunks in batch
    pub async fn verify_chunks_batch(&self, chunk_ids: &[(ChunkId, ChunkVersion)]) -> Result<Vec<VerificationResult>> {
        let start_time = Instant::now();
        
        let mut results = Vec::with_capacity(chunk_ids.len());
        let batch_size = self.config.batch_size.min(chunk_ids.len());
        
        for batch in chunk_ids.chunks(batch_size) {
            let mut batch_futures = Vec::with_capacity(batch.len());
            
            for (chunk_id, version) in batch {
                let future = self.verify_chunk(*chunk_id, *version);
                batch_futures.push(future);
            }
            
            let batch_results = futures::future::join_all(batch_futures).await;
            for result in batch_results {
                match result {
                    Ok(verification_result) => results.push(verification_result),
                    Err(e) => {
                        error!("Batch verification error: {}", e);
                        // Continue with other chunks
                    }
                }
            }
        }
        
        let duration = start_time.elapsed();
        debug!("Batch verification of {} chunks completed in {:?}", chunk_ids.len(), duration);
        
        Ok(results)
    }

    /// Run a full integrity scan of all chunks
    pub async fn run_full_scan(&self) -> Result<IntegrityScanSummary> {
        let start_time = Instant::now();
        info!("Starting full integrity scan");

        // Get list of all chunks
        let all_chunks = self.storage.list_chunks().await?;
        let total_chunks = all_chunks.len() as u64;
        
        info!("Found {} chunks to verify", total_chunks);

        // Verify all chunks in batches
        let verification_results = self.verify_chunks_batch(&all_chunks).await?;
        
        // Analyze results
        let verified_chunks = verification_results.len() as u64;
        let mut corrupted_chunks = 0u64;
        let mut all_issues = Vec::new();

        for result in verification_results {
            if !result.is_valid {
                corrupted_chunks += 1;
                all_issues.extend(result.issues);
            }
        }

        let duration = start_time.elapsed();
        let chunks_per_second = if duration.as_secs() > 0 {
            verified_chunks as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        // Update metrics
        self.metrics.last_full_scan.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            std::sync::atomic::Ordering::Relaxed
        );

        let summary = IntegrityScanSummary {
            total_chunks,
            verified_chunks,
            corrupted_chunks,
            issues_found: all_issues,
            scan_duration: duration,
            chunks_per_second,
        };

        info!("Full integrity scan completed: {} chunks verified, {} corrupted, {:.2} chunks/sec", 
              verified_chunks, corrupted_chunks, chunks_per_second);

        Ok(summary)
    }

    /// Get list of currently known corrupted chunks
    pub async fn get_corrupted_chunks(&self) -> Vec<(ChunkId, ChunkVersion)> {
        let corrupted = self.corrupted_chunks.read().await;
        corrupted.iter().cloned().collect()
    }

    /// Get pending repair queue
    pub async fn get_repair_queue(&self) -> Vec<IntegrityIssue> {
        let queue = self.repair_queue.read().await;
        queue.clone()
    }

    /// Mark a chunk as repaired and remove from corruption tracking
    pub async fn mark_chunk_repaired(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<()> {
        let mut corrupted = self.corrupted_chunks.write().await;
        corrupted.remove(&(chunk_id, version));

        // Remove from repair queue
        let mut queue = self.repair_queue.write().await;
        queue.retain(|issue| match issue {
            IntegrityIssue::ChecksumMismatch { chunk_id: id, version: v, .. } |
            IntegrityIssue::SizeMismatch { chunk_id: id, version: v, .. } |
            IntegrityIssue::MissingChunk { chunk_id: id, version: v } |
            IntegrityIssue::MetadataCorruption { chunk_id: id, version: v, .. } |
            IntegrityIssue::ReadError { chunk_id: id, version: v, .. } => {
                !(*id == chunk_id && *v == version)
            }
        });

        info!("Marked chunk {} v{} as repaired", chunk_id, version);
        Ok(())
    }

    /// Get integrity verification metrics
    pub fn get_metrics(&self) -> Arc<IntegrityMetrics> {
        self.metrics.clone()
    }

    /// Get configuration
    pub fn get_config(&self) -> &IntegrityConfig {
        &self.config
    }

    /// Record an integrity issue
    async fn record_issue(&self, issue: &IntegrityIssue) {
        match issue {
            IntegrityIssue::ChecksumMismatch { .. } => {
                self.metrics.checksum_failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            IntegrityIssue::SizeMismatch { .. } => {
                self.metrics.size_mismatches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            IntegrityIssue::MissingChunk { .. } => {
                self.metrics.missing_chunks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            IntegrityIssue::MetadataCorruption { .. } => {
                self.metrics.metadata_corruptions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            IntegrityIssue::ReadError { .. } => {
                self.metrics.read_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        // Add to repair queue if auto-repair is enabled
        if self.config.auto_repair {
            let mut queue = self.repair_queue.write().await;
            queue.push(issue.clone());
        }
    }
}

/// Background integrity scanner that runs periodic full scans
pub struct BackgroundIntegrityScanner {
    verifier: Arc<IntegrityVerifier>,
    scan_interval: Duration,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl BackgroundIntegrityScanner {
    pub fn new(verifier: Arc<IntegrityVerifier>, scan_interval: Duration) -> Self {
        Self {
            verifier,
            scan_interval,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the background scanner
    pub async fn start(&self) -> Result<()> {
        if self.is_running.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(ChunkServerError::InvalidOperation("Scanner already running".to_string()));
        }

        let verifier = self.verifier.clone();
        let interval = self.scan_interval;
        let running = self.is_running.clone();

        tokio::spawn(async move {
            info!("Starting background integrity scanner with interval {:?}", interval);
            
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                match verifier.run_full_scan().await {
                    Ok(summary) => {
                        info!("Background scan completed: {} chunks verified, {} corrupted", 
                              summary.verified_chunks, summary.corrupted_chunks);
                    }
                    Err(e) => {
                        error!("Background scan failed: {}", e);
                    }
                }

                // Sleep for the specified interval
                let mut remaining = interval;
                while remaining > Duration::ZERO && running.load(std::sync::atomic::Ordering::SeqCst) {
                    let sleep_time = remaining.min(Duration::from_secs(60)); // Check every minute
                    tokio::time::sleep(sleep_time).await;
                    remaining = remaining.saturating_sub(sleep_time);
                }
            }

            info!("Background integrity scanner stopped");
        });

        Ok(())
    }

    /// Stop the background scanner
    pub fn stop(&self) {
        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if the scanner is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::FileStorage;
    use crate::config::ChunkServerConfig;
    use crate::chunk::{Chunk, ChecksumType};
    use tempfile::TempDir;
    use bytes::Bytes;

    fn create_test_storage() -> (Arc<dyn ChunkStorage>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ChunkServerConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        let storage = Arc::new(FileStorage::new(Arc::new(config))) as Arc<dyn ChunkStorage>;
        (storage, temp_dir)
    }

    fn create_test_chunk(chunk_id: ChunkId, data: &str) -> Chunk {
        let data = Bytes::from(data.to_string());
        Chunk::new(chunk_id, 1, data, ChecksumType::HybridSecure, 1)
    }

    #[tokio::test]
    async fn test_verify_valid_chunk() {
        let (storage, _temp_dir) = create_test_storage();
        let verifier = IntegrityVerifier::new(storage.clone(), IntegrityConfig::default());
        
        let chunk = create_test_chunk(1, "test data");
        storage.store_chunk(&chunk).await.unwrap();
        
        let result = verifier.verify_chunk(1, 1).await.unwrap();
        assert!(result.is_valid);
        assert!(result.issues.is_empty());
    }

    #[tokio::test]
    async fn test_verify_missing_chunk() {
        let (storage, _temp_dir) = create_test_storage();
        let verifier = IntegrityVerifier::new(storage.clone(), IntegrityConfig::default());
        
        let result = verifier.verify_chunk(999, 1).await.unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.issues.len(), 1);
        assert!(matches!(result.issues[0], IntegrityIssue::MetadataCorruption { .. }));
    }

    #[tokio::test]
    async fn test_batch_verification() {
        let (storage, _temp_dir) = create_test_storage();
        let verifier = IntegrityVerifier::new(storage.clone(), IntegrityConfig::default());
        
        // Store multiple chunks
        for i in 0..5 {
            let chunk = create_test_chunk(i, &format!("test data {}", i));
            storage.store_chunk(&chunk).await.unwrap();
        }
        
        let chunk_ids: Vec<(ChunkId, ChunkVersion)> = (0..5).map(|i| (i, 1)).collect();
        let results = verifier.verify_chunks_batch(&chunk_ids).await.unwrap();
        
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.is_valid));
    }

    #[tokio::test]
    async fn test_full_scan() {
        let (storage, _temp_dir) = create_test_storage();
        let verifier = IntegrityVerifier::new(storage.clone(), IntegrityConfig::default());
        
        // Store test chunks
        for i in 0..10 {
            let chunk = create_test_chunk(i, &format!("test data {}", i));
            storage.store_chunk(&chunk).await.unwrap();
        }
        
        let summary = verifier.run_full_scan().await.unwrap();
        assert_eq!(summary.total_chunks, 10);
        assert_eq!(summary.verified_chunks, 10);
        assert_eq!(summary.corrupted_chunks, 0);
        assert!(summary.chunks_per_second > 0.0);
    }
}