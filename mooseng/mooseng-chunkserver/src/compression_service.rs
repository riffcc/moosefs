//! Compression service for transparent chunk compression/decompression
//! 
//! This module provides a service layer that integrates compression capabilities
//! into the chunk server operations, handling compression policies and automatic
//! algorithm selection.

use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use bytes::Bytes;

use mooseng_common::compression::{
    CompressionEngine, CompressionConfig, CompressionAlgorithm, 
    CompressionLevel, CompressedData, CompressionStats
};
use crate::chunk::{Chunk, ChunkMetadata, ChecksumType};
use crate::config::ChunkServerConfig;
use mooseng_common::types::{ChunkId, ChunkVersion};

/// Compression policy for determining when and how to compress chunks
#[derive(Debug, Clone)]
pub struct CompressionPolicy {
    /// Enable compression globally
    pub enabled: bool,
    /// Minimum chunk size to consider for compression
    pub min_chunk_size: usize,
    /// Maximum chunk size to attempt compression
    pub max_chunk_size: usize,
    /// Default compression algorithm
    pub default_algorithm: CompressionAlgorithm,
    /// Compression level
    pub level: CompressionLevel,
    /// Per-storage-class compression settings
    pub storage_class_overrides: HashMap<u8, CompressionConfig>,
    /// File extension patterns that should not be compressed
    pub skip_patterns: Vec<String>,
}

impl Default for CompressionPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            min_chunk_size: 4096,        // 4KB minimum
            max_chunk_size: 100 << 20,   // 100MB maximum
            default_algorithm: CompressionAlgorithm::Auto,
            level: CompressionLevel::Balanced,
            storage_class_overrides: HashMap::new(),
            skip_patterns: vec![
                "*.jpg".to_string(),
                "*.jpeg".to_string(),
                "*.png".to_string(),
                "*.gif".to_string(),
                "*.mp4".to_string(),
                "*.mp3".to_string(),
                "*.zip".to_string(),
                "*.gz".to_string(),
                "*.bz2".to_string(),
                "*.xz".to_string(),
            ],
        }
    }
}

/// Chunk compression service
pub struct ChunkCompressionService {
    /// Compression engine
    engine: CompressionEngine,
    /// Compression policy
    policy: CompressionPolicy,
    /// Per-storage-class configs
    storage_class_configs: Arc<RwLock<HashMap<u8, CompressionConfig>>>,
    /// Service statistics
    stats: Arc<RwLock<CompressionServiceStats>>,
}

/// Statistics for the compression service
#[derive(Debug, Clone, Default)]
pub struct CompressionServiceStats {
    pub chunks_considered: u64,
    pub chunks_compressed: u64,
    pub chunks_skipped: u64,
    pub chunks_decompressed: u64,
    pub total_bytes_saved: u64,
    pub compression_errors: u64,
    pub decompression_errors: u64,
    pub avg_compression_ratio: f64,
}

impl ChunkCompressionService {
    /// Create a new compression service
    pub fn new(policy: CompressionPolicy) -> Self {
        let config = CompressionConfig {
            algorithm: policy.default_algorithm,
            level: policy.level.clone(),
            min_size: policy.min_chunk_size,
            max_size: policy.max_chunk_size,
            ..Default::default()
        };

        let engine = CompressionEngine::new(config);

        Self {
            engine,
            policy,
            storage_class_configs: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CompressionServiceStats::default())),
        }
    }

    /// Create from chunk server config
    pub fn from_config(_config: &ChunkServerConfig) -> Self {
        // Extract compression policy from config (would be implemented based on actual config structure)
        let policy = CompressionPolicy::default(); // TODO: Read from config
        Self::new(policy)
    }

    /// Compress a chunk if beneficial
    pub async fn compress_chunk(
        &self,
        chunk_id: ChunkId,
        version: ChunkVersion,
        data: Bytes,
        checksum_type: ChecksumType,
        storage_class_id: u8,
    ) -> Result<Chunk> {
        let mut stats = self.stats.write().await;
        stats.chunks_considered += 1;
        drop(stats);

        // Check if compression is enabled and chunk meets criteria
        if !self.policy.enabled {
            debug!("Compression disabled, creating uncompressed chunk {}", chunk_id);
            return Ok(Chunk::new(chunk_id, version, data, checksum_type, storage_class_id));
        }

        if data.len() < self.policy.min_chunk_size {
            debug!("Chunk {} too small for compression ({} bytes)", chunk_id, data.len());
            let mut stats = self.stats.write().await;
            stats.chunks_skipped += 1;
            return Ok(Chunk::new(chunk_id, version, data, checksum_type, storage_class_id));
        }

        if data.len() > self.policy.max_chunk_size {
            warn!("Chunk {} too large for compression ({} bytes)", chunk_id, data.len());
            let mut stats = self.stats.write().await;
            stats.chunks_skipped += 1;
            return Ok(Chunk::new(chunk_id, version, data, checksum_type, storage_class_id));
        }

        // Get the appropriate compression engine
        let engine = self.get_engine_for_storage_class(storage_class_id).await;

        // Attempt compression
        match engine.compress(&data) {
            Ok(compressed_data) => {
                // Only use compression if it actually saves space
                if compressed_data.data.len() < data.len() {
                    info!("Compressed chunk {} from {} to {} bytes (ratio: {:.2}x)",
                          chunk_id, data.len(), compressed_data.data.len(), compressed_data.ratio);

                    let mut stats = self.stats.write().await;
                    stats.chunks_compressed += 1;
                    stats.total_bytes_saved += (data.len() - compressed_data.data.len()) as u64;
                    stats.avg_compression_ratio = (stats.avg_compression_ratio + compressed_data.ratio) / 2.0;
                    drop(stats);

                    Ok(Chunk::new_compressed(
                        chunk_id,
                        version,
                        compressed_data,
                        checksum_type,
                        storage_class_id,
                    ))
                } else {
                    debug!("Compression of chunk {} not beneficial, storing uncompressed", chunk_id);
                    let mut stats = self.stats.write().await;
                    stats.chunks_skipped += 1;
                    Ok(Chunk::new(chunk_id, version, data, checksum_type, storage_class_id))
                }
            }
            Err(e) => {
                error!("Compression failed for chunk {}: {}", chunk_id, e);
                let mut stats = self.stats.write().await;
                stats.compression_errors += 1;
                stats.chunks_skipped += 1;
                Ok(Chunk::new(chunk_id, version, data, checksum_type, storage_class_id))
            }
        }
    }

    /// Decompress chunk data if needed
    pub async fn decompress_chunk(&self, chunk: &Chunk) -> Result<Bytes> {
        if !chunk.is_compressed() {
            return Ok(chunk.data.clone());
        }

        debug!("Decompressing chunk {} ({} -> {} bytes expected)",
               chunk.id(), chunk.data.len(), chunk.original_size());

        let compressed_data = CompressedData {
            data: chunk.data.clone(),
            algorithm: chunk.metadata.compression,
            original_size: chunk.metadata.original_size as usize,
            ratio: chunk.metadata.compression_ratio,
        };

        // Get the appropriate engine
        let engine = self.get_engine_for_storage_class(chunk.metadata.storage_class_id).await;

        match engine.decompress(&compressed_data) {
            Ok(decompressed) => {
                let mut stats = self.stats.write().await;
                stats.chunks_decompressed += 1;
                
                info!("Decompressed chunk {} from {} to {} bytes",
                      chunk.id(), chunk.data.len(), decompressed.len());
                
                Ok(decompressed)
            }
            Err(e) => {
                error!("Decompression failed for chunk {}: {}", chunk.id(), e);
                let mut stats = self.stats.write().await;
                stats.decompression_errors += 1;
                Err(e)
            }
        }
    }

    /// Get compression statistics
    pub async fn get_stats(&self) -> CompressionServiceStats {
        self.stats.read().await.clone()
    }

    /// Get compression engine statistics
    pub fn get_engine_stats(&self) -> CompressionStats {
        self.engine.stats()
    }

    /// Reset service statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = CompressionServiceStats::default();
        self.engine.reset_stats();
    }

    /// Update compression policy
    pub async fn update_policy(&mut self, new_policy: CompressionPolicy) {
        info!("Updating compression policy");
        self.policy = new_policy;
        
        // Clear storage class configs to force recreation with new policy
        let mut configs = self.storage_class_configs.write().await;
        configs.clear();
    }

    /// Check if a file should be compressed based on patterns
    pub fn should_compress_file(&self, filename: &str) -> bool {
        if !self.policy.enabled {
            return false;
        }

        // Check skip patterns
        for pattern in &self.policy.skip_patterns {
            if filename.ends_with(&pattern[1..]) { // Remove the '*' from pattern
                return false;
            }
        }

        true
    }

    /// Get engine for specific storage class
    async fn get_engine_for_storage_class(&self, storage_class_id: u8) -> CompressionEngine {
        let engines = self.storage_class_configs.read().await;
        
        if let Some(config) = engines.get(&storage_class_id) {
            return CompressionEngine::new(config.clone());
        }
        drop(engines);

        // Create new engine for this storage class
        let mut engines = self.storage_class_configs.write().await;
        
        // Double-check in case another task created it
        if let Some(config) = engines.get(&storage_class_id) {
            return CompressionEngine::new(config.clone());
        }

        let config = if let Some(override_config) = self.policy.storage_class_overrides.get(&storage_class_id) {
            override_config.clone()
        } else {
            CompressionConfig {
                algorithm: self.policy.default_algorithm,
                level: self.policy.level.clone(),
                min_size: self.policy.min_chunk_size,
                max_size: self.policy.max_chunk_size,
                ..Default::default()
            }
        };

        let engine = CompressionEngine::new(config.clone());
        engines.insert(storage_class_id, config);
        
        debug!("Created compression engine for storage class {}", storage_class_id);
        engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mooseng_common::types::ChunkId;

    #[tokio::test]
    async fn test_compression_service_creation() {
        let policy = CompressionPolicy::default();
        let service = ChunkCompressionService::new(policy);
        
        let stats = service.get_stats().await;
        assert_eq!(stats.chunks_considered, 0);
    }

    #[tokio::test]
    async fn test_small_chunk_skipping() {
        let policy = CompressionPolicy {
            min_chunk_size: 1000,
            ..Default::default()
        };
        let service = ChunkCompressionService::new(policy);
        
        let small_data = Bytes::from(vec![b'A'; 500]);
        let chunk = service.compress_chunk(
            1,
            1,
            small_data,
            ChecksumType::Blake3,
            0,
        ).await.unwrap();
        
        assert!(!chunk.is_compressed());
        
        let stats = service.get_stats().await;
        assert_eq!(stats.chunks_skipped, 1);
    }

    #[tokio::test]
    async fn test_compression_and_decompression() {
        let policy = CompressionPolicy {
            min_chunk_size: 100,
            default_algorithm: CompressionAlgorithm::Lz4,
            ..Default::default()
        };
        let service = ChunkCompressionService::new(policy);
        
        // Create compressible data
        let data = Bytes::from(vec![b'A'; 5000]);
        let chunk = service.compress_chunk(
            1,
            1,
            data.clone(),
            ChecksumType::Blake3,
            0,
        ).await.unwrap();
        
        if chunk.is_compressed() {
            let decompressed = service.decompress_chunk(&chunk).await.unwrap();
            assert_eq!(decompressed, data);
            
            let stats = service.get_stats().await;
            assert_eq!(stats.chunks_compressed, 1);
            assert_eq!(stats.chunks_decompressed, 1);
        }
    }

    #[tokio::test]
    async fn test_file_pattern_matching() {
        let service = ChunkCompressionService::new(CompressionPolicy::default());
        
        assert!(!service.should_compress_file("image.jpg"));
        assert!(!service.should_compress_file("video.mp4"));
        assert!(!service.should_compress_file("archive.zip"));
        assert!(service.should_compress_file("document.txt"));
        assert!(service.should_compress_file("data.csv"));
    }

    #[tokio::test]
    async fn test_compression_disabled() {
        let policy = CompressionPolicy {
            enabled: false,
            ..Default::default()
        };
        let service = ChunkCompressionService::new(policy);
        
        let data = Bytes::from(vec![b'A'; 5000]);
        let chunk = service.compress_chunk(
            1,
            1,
            data,
            ChecksumType::Blake3,
            0,
        ).await.unwrap();
        
        assert!(!chunk.is_compressed());
    }
}