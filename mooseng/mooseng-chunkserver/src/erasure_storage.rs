use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use mooseng_common::types::{ChunkId, ChunkVersion};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::{
    chunk::{Chunk, ChunkMetadata, ChecksumType},
    config::ChunkServerConfig,
    error::{ChunkServerError, Result as ChunkResult},
    erasure::{ErasureCodedChunk, ErasureConfig, ErasureCoder, ErasureShard, ShardPlacement},
    storage::{ChunkStorage, StorageStats},
};

/// Storage implementation that handles erasure-coded chunks
pub struct ErasureCodedStorage {
    config: Arc<ChunkServerConfig>,
    erasure_config: ErasureConfig,
    base_storage: Arc<dyn ChunkStorage>,
    shard_placement: Arc<RwLock<ShardPlacement>>,
    /// Cache of shard locations: chunk_id -> shard_index -> server_ids
    shard_locations: Arc<DashMap<ChunkId, HashMap<usize, Vec<String>>>>,
}

impl ErasureCodedStorage {
    pub fn new(
        config: Arc<ChunkServerConfig>,
        erasure_config: ErasureConfig,
        base_storage: Arc<dyn ChunkStorage>,
    ) -> Self {
        let shard_placement = ShardPlacement::new(erasure_config.clone());
        
        Self {
            config,
            erasure_config: erasure_config.clone(),
            base_storage,
            shard_placement: Arc::new(RwLock::new(shard_placement)),
            shard_locations: Arc::new(DashMap::new()),
        }
    }
    
    /// Store a single shard
    async fn store_shard(
        &self,
        chunk_id: ChunkId,
        version: ChunkVersion,
        shard: &ErasureShard,
    ) -> ChunkResult<()> {
        let shard_chunk_id = self.shard_chunk_id(chunk_id, shard.index);
        
        // Create a chunk wrapper for the shard
        let shard_chunk = Chunk::new(
            shard_chunk_id,
            version,
            shard.data.clone(),
            crate::ChecksumType::Blake3,
            0, // storage class for shards
        );
        
        self.base_storage.store_chunk(&shard_chunk).await?;
        
        debug!("Stored shard {} of chunk {} v{}", shard.index, chunk_id, version);
        Ok(())
    }
    
    /// Retrieve a single shard
    async fn get_shard(
        &self,
        chunk_id: ChunkId,
        version: ChunkVersion,
        shard_index: usize,
    ) -> ChunkResult<Option<ErasureShard>> {
        let shard_chunk_id = self.shard_chunk_id(chunk_id, shard_index);
        
        match self.base_storage.get_chunk(shard_chunk_id, version).await {
            Ok(chunk) => {
                let is_data = shard_index < self.erasure_config.data_shards;
                let shard = ErasureShard::new(shard_index, is_data, chunk.data);
                
                if !shard.verify() {
                    warn!("Shard {} of chunk {} v{} failed verification", 
                          shard_index, chunk_id, version);
                    return Ok(None);
                }
                
                Ok(Some(shard))
            }
            Err(ChunkServerError::ChunkNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }
    
    /// Delete a single shard
    async fn delete_shard(
        &self,
        chunk_id: ChunkId,
        version: ChunkVersion,
        shard_index: usize,
    ) -> ChunkResult<()> {
        let shard_chunk_id = self.shard_chunk_id(chunk_id, shard_index);
        
        match self.base_storage.delete_chunk(shard_chunk_id, version).await {
            Ok(()) => Ok(()),
            Err(ChunkServerError::ChunkNotFound { .. }) => Ok(()), // Already deleted
            Err(e) => Err(e),
        }
    }
    
    /// Generate a unique chunk ID for a shard
    fn shard_chunk_id(&self, chunk_id: ChunkId, shard_index: usize) -> ChunkId {
        // Use high bits for shard index to avoid collisions
        (chunk_id & 0x00FFFFFFFFFFFFFF) | ((shard_index as u64) << 56)
    }
    
    /// Extract original chunk ID from shard chunk ID
    fn original_chunk_id(&self, shard_chunk_id: ChunkId) -> ChunkId {
        shard_chunk_id & 0x00FFFFFFFFFFFFFF
    }
    
    /// Extract shard index from shard chunk ID
    fn shard_index(&self, shard_chunk_id: ChunkId) -> usize {
        ((shard_chunk_id >> 56) & 0xFF) as usize
    }
    
    /// Store erasure-coded chunk shards
    async fn store_erasure_coded(&self, ec_chunk: &ErasureCodedChunk) -> ChunkResult<()> {
        let chunk_id = ec_chunk.metadata.chunk_id;
        let version = ec_chunk.metadata.version;
        
        // Store each shard
        let mut store_futures = Vec::new();
        for (index, shard_opt) in ec_chunk.shards.iter().enumerate() {
            if let Some(shard) = shard_opt {
                store_futures.push(self.store_shard(chunk_id, version, shard));
            }
        }
        
        // Wait for all shards to be stored
        let results = futures::future::join_all(store_futures).await;
        
        // Check for any failures
        let mut failed = false;
        for (i, result) in results.iter().enumerate() {
            if let Err(e) = result {
                error!("Failed to store shard {} of chunk {}: {}", i, chunk_id, e);
                failed = true;
            }
        }
        
        if failed {
            // Clean up any successfully stored shards
            for (i, result) in results.iter().enumerate() {
                if result.is_ok() {
                    let _ = self.delete_shard(chunk_id, version, i).await;
                }
            }
            return Err(ChunkServerError::Storage(
                format!("Failed to store erasure-coded chunk {}", chunk_id)
            ));
        }
        
        // Store metadata about the erasure coding
        let ec_metadata = ErasureMetadata {
            chunk_id,
            version,
            config: ec_chunk.config.clone(),
            original_size: ec_chunk.original_size,
            shard_count: ec_chunk.shards.len(),
        };
        
        self.store_erasure_metadata(&ec_metadata).await?;
        
        info!("Successfully stored erasure-coded chunk {} v{} ({} shards)",
              chunk_id, version, ec_chunk.shards.len());
        
        Ok(())
    }
    
    /// Retrieve erasure-coded chunk by reconstructing from shards
    async fn get_erasure_coded(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<Chunk> {
        // Get erasure metadata
        let ec_metadata = self.get_erasure_metadata(chunk_id, version).await?;
        
        // Try to retrieve all shards
        let mut shard_futures = Vec::new();
        for i in 0..ec_metadata.shard_count {
            shard_futures.push(self.get_shard(chunk_id, version, i));
        }
        
        let shard_results = futures::future::join_all(shard_futures).await;
        
        // Collect available shards
        let mut shards = Vec::with_capacity(ec_metadata.shard_count);
        let mut available_count = 0;
        
        for result in shard_results {
            match result {
                Ok(Some(shard)) => {
                    shards.push(Some(shard));
                    available_count += 1;
                }
                Ok(None) => shards.push(None),
                Err(e) => {
                    warn!("Error retrieving shard: {}", e);
                    shards.push(None);
                }
            }
        }
        
        debug!("Retrieved {} of {} shards for chunk {} v{}",
               available_count, ec_metadata.shard_count, chunk_id, version);
        
        // Check if we have enough shards to reconstruct
        if available_count < ec_metadata.config.data_shards {
            return Err(ChunkServerError::InsufficientShards {
                chunk_id,
                available: available_count,
                required: ec_metadata.config.data_shards,
            });
        }
        
        // Get original metadata
        let metadata = self.base_storage.get_chunk_metadata(chunk_id, version).await?;
        
        // Create erasure-coded chunk for reconstruction
        let ec_chunk = ErasureCodedChunk {
            metadata,
            config: ec_metadata.config,
            shards,
            original_size: ec_metadata.original_size,
        };
        
        // Reconstruct original chunk
        ec_chunk.reconstruct().await.map_err(|e| {
            ChunkServerError::Storage(format!("Failed to reconstruct chunk {}: {}", chunk_id, e))
        })
    }
    
    /// Store erasure metadata
    async fn store_erasure_metadata(&self, metadata: &ErasureMetadata) -> ChunkResult<()> {
        let path = self.erasure_metadata_path(metadata.chunk_id, metadata.version);
        let serialized = bincode::serialize(metadata)?;
        
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| ChunkServerError::Io(e))?;
        }
        
        fs::write(&path, serialized).await.map_err(|e| ChunkServerError::Io(e))?;
        Ok(())
    }
    
    /// Get erasure metadata
    async fn get_erasure_metadata(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<ErasureMetadata> {
        let path = self.erasure_metadata_path(chunk_id, version);
        let data = fs::read(&path).await.map_err(|e| ChunkServerError::Io(e))?;
        bincode::deserialize(&data).map_err(|e| e.into())
    }
    
    /// Get path for erasure metadata
    fn erasure_metadata_path(&self, chunk_id: ChunkId, version: ChunkVersion) -> PathBuf {
        let mut path = self.config.chunk_metadata_path(chunk_id, version);
        path.set_extension("erasure");
        path
    }
}

#[async_trait]
impl ChunkStorage for ErasureCodedStorage {
    async fn store_chunk(&self, chunk: &Chunk) -> ChunkResult<()> {
        // Convert to erasure-coded chunk
        let ec_chunk = ErasureCodedChunk::from_chunk(chunk.clone(), self.erasure_config.clone())
            .await
            .map_err(|e| ChunkServerError::Storage(format!("Failed to encode chunk: {}", e)))?;
        
        // Store the erasure-coded shards
        self.store_erasure_coded(&ec_chunk).await?;
        
        // Also store the original metadata for quick access
        self.base_storage.update_chunk_metadata(&chunk.metadata).await
    }
    
    async fn get_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<Chunk> {
        self.get_erasure_coded(chunk_id, version).await
    }
    
    async fn chunk_exists(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<bool> {
        // Check if erasure metadata exists
        let path = self.erasure_metadata_path(chunk_id, version);
        Ok(path.exists())
    }
    
    async fn delete_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<()> {
        // Get erasure metadata to know how many shards to delete
        match self.get_erasure_metadata(chunk_id, version).await {
            Ok(metadata) => {
                // Delete all shards
                let mut delete_futures = Vec::new();
                for i in 0..metadata.shard_count {
                    delete_futures.push(self.delete_shard(chunk_id, version, i));
                }
                
                let results = futures::future::join_all(delete_futures).await;
                for (i, result) in results.iter().enumerate() {
                    if let Err(e) = result {
                        warn!("Failed to delete shard {} of chunk {}: {}", i, chunk_id, e);
                    }
                }
                
                // Delete erasure metadata
                let meta_path = self.erasure_metadata_path(chunk_id, version);
                if let Err(e) = fs::remove_file(&meta_path).await {
                    warn!("Failed to delete erasure metadata: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to get erasure metadata for deletion: {}", e);
            }
        }
        
        // Delete original metadata
        self.base_storage.delete_chunk(chunk_id, version).await
    }
    
    async fn list_chunks(&self) -> ChunkResult<Vec<(ChunkId, ChunkVersion)>> {
        // List all chunks from base storage and filter out shard chunks
        let all_chunks = self.base_storage.list_chunks().await?;
        
        let mut chunks = Vec::new();
        let mut seen = std::collections::HashSet::new();
        
        for (chunk_id, version) in all_chunks {
            let original_id = self.original_chunk_id(chunk_id);
            if seen.insert((original_id, version)) {
                // Check if this is an erasure-coded chunk
                if self.erasure_metadata_path(original_id, version).exists() {
                    chunks.push((original_id, version));
                }
            }
        }
        
        chunks.sort_by_key(|(id, ver)| (*id, *ver));
        Ok(chunks)
    }
    
    async fn get_chunk_metadata(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<ChunkMetadata> {
        self.base_storage.get_chunk_metadata(chunk_id, version).await
    }
    
    async fn update_chunk_metadata(&self, metadata: &ChunkMetadata) -> ChunkResult<()> {
        self.base_storage.update_chunk_metadata(metadata).await
    }
    
    async fn verify_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<bool> {
        match self.get_chunk(chunk_id, version).await {
            Ok(chunk) => Ok(chunk.verify_integrity()),
            Err(ChunkServerError::ChecksumMismatch { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
    
    async fn get_stats(&self) -> ChunkResult<StorageStats> {
        // Get base storage stats and adjust for erasure coding
        let mut stats = self.base_storage.get_stats().await?;
        
        // Count erasure-coded chunks
        let chunks = self.list_chunks().await?;
        stats.total_chunks = chunks.len() as u64;
        
        // Adjust bytes to account for erasure coding overhead
        let overhead_factor = self.erasure_config.total_shards() as f64 / self.erasure_config.data_shards as f64;
        stats.total_bytes = (stats.total_bytes as f64 * overhead_factor) as u64;
        
        Ok(stats)
    }
    
    async fn store_chunks_batch(&self, chunks: &[&Chunk]) -> ChunkResult<Vec<ChunkResult<()>>> {
        // For now, delegate to base storage batch operation
        // TODO: Optimize for erasure coding batches
        self.base_storage.store_chunks_batch(chunks).await
    }
    
    async fn get_chunks_batch(&self, chunk_ids: &[(ChunkId, ChunkVersion)]) -> ChunkResult<Vec<ChunkResult<Chunk>>> {
        // For now, delegate to individual gets
        // TODO: Optimize for erasure coding batches
        let mut results = Vec::with_capacity(chunk_ids.len());
        for (chunk_id, version) in chunk_ids {
            results.push(self.get_chunk(*chunk_id, *version).await);
        }
        Ok(results)
    }
    
    async fn verify_chunk_fast(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<bool> {
        // Delegate to base storage fast verification
        self.base_storage.verify_chunk_fast(chunk_id, version).await
    }
    
    async fn read_chunk_slice(&self, chunk_id: ChunkId, version: ChunkVersion, offset: u64, length: u64) -> ChunkResult<Bytes> {
        // For erasure coded chunks, we need to read the whole chunk and slice it
        let chunk = self.get_chunk(chunk_id, version).await?;
        chunk.slice(offset, length)
    }
    
    async fn preload_chunks(&self, chunk_ids: &[(ChunkId, ChunkVersion)]) -> ChunkResult<()> {
        // Delegate to base storage preloading
        self.base_storage.preload_chunks(chunk_ids).await
    }
}

/// Metadata about erasure coding for a chunk
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ErasureMetadata {
    chunk_id: ChunkId,
    version: ChunkVersion,
    config: ErasureConfig,
    original_size: u64,
    shard_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::storage::FileStorage;
    
    fn create_test_setup() -> (Arc<ChunkServerConfig>, TempDir, Arc<dyn ChunkStorage>) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ChunkServerConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        let config = Arc::new(config);
        let base_storage = Arc::new(FileStorage::new(config.clone()));
        (config, temp_dir, base_storage)
    }
    
    #[tokio::test]
    async fn test_erasure_coded_storage() {
        let (config, _temp_dir, base_storage) = create_test_setup();
        let ec_config = ErasureConfig::config_4_2();
        let storage = ErasureCodedStorage::new(config, ec_config, base_storage);
        
        // Create test chunk
        let data = Bytes::from(vec![42u8; 1024 * 1024]); // 1MB
        let chunk = Chunk::new(12345, 1, data.clone(), crate::ChecksumType::Blake3, 1);
        
        // Store chunk
        storage.store_chunk(&chunk).await.unwrap();
        
        // Verify it exists
        assert!(storage.chunk_exists(chunk.id(), chunk.version()).await.unwrap());
        
        // Retrieve chunk
        let retrieved = storage.get_chunk(chunk.id(), chunk.version()).await.unwrap();
        assert_eq!(chunk.data, retrieved.data);
        assert_eq!(chunk.metadata.chunk_id, retrieved.metadata.chunk_id);
        
        // Delete chunk
        storage.delete_chunk(chunk.id(), chunk.version()).await.unwrap();
        assert!(!storage.chunk_exists(chunk.id(), chunk.version()).await.unwrap());
    }
}