use crate::{
    chunk::{Chunk, ChunkMetadata, ChecksumType},
    config::ChunkServerConfig,
    error::{ChunkServerError, Result},
    block_allocator::{DynamicBlockAllocator, AllocationStrategy, AllocatedBlock, WorkloadHint},
};
// TODO: Fix zero_copy module import
// use crate::zero_copy::ZeroCopyTransfer;
use mooseng_common::types::{ChunkId, ChunkVersion};
use bytes::Bytes;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use std::sync::Arc;
use dashmap::DashMap;
use tracing::{debug, error, info};
use std::time::Instant;
use tokio::sync::{RwLock, Semaphore};
use lru::LruCache;
use std::num::NonZeroUsize;

/// Trait for chunk storage operations
#[async_trait::async_trait]
pub trait ChunkStorage: Send + Sync {
    /// Store a chunk to persistent storage
    async fn store_chunk(&self, chunk: &Chunk) -> Result<()>;
    
    /// Retrieve a chunk from persistent storage
    async fn get_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<Chunk>;
    
    /// Check if a chunk exists
    async fn chunk_exists(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool>;
    
    /// Delete a chunk from persistent storage
    async fn delete_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<()>;
    
    /// List all chunks
    async fn list_chunks(&self) -> Result<Vec<(ChunkId, ChunkVersion)>>;
    
    /// Get chunk metadata without loading the full chunk
    async fn get_chunk_metadata(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<ChunkMetadata>;
    
    /// Update chunk metadata
    async fn update_chunk_metadata(&self, metadata: &ChunkMetadata) -> Result<()>;
    
    /// Verify chunk integrity
    async fn verify_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool>;
    
    /// Get storage statistics
    async fn get_stats(&self) -> Result<StorageStats>;
    
    // New performance-optimized methods
    
    /// Store multiple chunks in a batch operation
    async fn store_chunks_batch(&self, chunks: &[&Chunk]) -> Result<Vec<Result<()>>>;
    
    /// Retrieve multiple chunks in a batch operation
    async fn get_chunks_batch(&self, chunk_ids: &[(ChunkId, ChunkVersion)]) -> Result<Vec<Result<Chunk>>>;
    
    /// Fast integrity verification using quick checksums
    async fn verify_chunk_fast(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool>;
    
    /// Read a chunk slice without loading the entire chunk
    async fn read_chunk_slice(&self, chunk_id: ChunkId, version: ChunkVersion, offset: u64, length: u64) -> Result<Bytes>;
    
    /// Preload chunks into cache
    async fn preload_chunks(&self, chunk_ids: &[(ChunkId, ChunkVersion)]) -> Result<()>;
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_chunks: u64,
    pub total_bytes: u64,
    pub free_space_bytes: u64,
    pub corrupted_chunks: u64,
}

/// Comprehensive storage statistics including tiered storage
#[derive(Debug, Clone)]
pub struct ComprehensiveStorageStats {
    pub primary_storage: StorageStats,
    pub allocation: crate::block_allocator::FreeSpaceInfo,
    pub tiered_storage: Option<crate::tiered_storage::PerformanceMetrics>,
    pub movement: Option<crate::tier_movement::MovementStats>,
}

/// File-based chunk storage implementation with performance optimizations
pub struct FileStorage {
    config: Arc<ChunkServerConfig>,
    chunk_locks: Arc<DashMap<(ChunkId, ChunkVersion), Arc<tokio::sync::Mutex<()>>>>,
    // Performance components
    // TODO: Re-enable zero_copy when module is fixed
    // zero_copy: Arc<ZeroCopyTransfer>,
    metadata_cache: Arc<RwLock<LruCache<(ChunkId, ChunkVersion), ChunkMetadata>>>,
    io_semaphore: Arc<Semaphore>,
    batch_size_limit: usize,
    // Performance metrics
    metrics: Arc<StorageMetrics>,
}

/// Performance metrics for storage operations
#[derive(Debug, Default)]
pub struct StorageMetrics {
    pub read_count: std::sync::atomic::AtomicU64,
    pub write_count: std::sync::atomic::AtomicU64,
    pub cache_hits: std::sync::atomic::AtomicU64,
    pub cache_misses: std::sync::atomic::AtomicU64,
    pub fast_verifications: std::sync::atomic::AtomicU64,
    pub batch_operations: std::sync::atomic::AtomicU64,
    pub total_read_time_micros: std::sync::atomic::AtomicU64,
    pub total_write_time_micros: std::sync::atomic::AtomicU64,
}

impl FileStorage {
    /// Create a new file storage instance
    pub fn new(config: Arc<ChunkServerConfig>) -> Self {
        // TODO: Re-enable zero_copy when module is fixed
        // let mmap_manager = Arc::new(MmapManager::new(Default::default()));
        // let zero_copy = Arc::new(ZeroCopyTransfer::new(mmap_manager));
        
        Self {
            config,
            chunk_locks: Arc::new(DashMap::new()),
            // zero_copy,
            metadata_cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))), // Cache last 1000 metadata entries
            io_semaphore: Arc::new(Semaphore::new(64)), // Limit concurrent I/O operations
            batch_size_limit: 32, // Maximum chunks per batch operation
            metrics: Arc::new(StorageMetrics::default()),
        }
    }
    
    /// Create a new file storage instance with custom limits
    pub fn new_with_limits(
        config: Arc<ChunkServerConfig>, 
        cache_size: usize, 
        concurrent_io: usize,
        batch_limit: usize
    ) -> Self {
        // TODO: Re-enable zero_copy when module is fixed
        // let mmap_manager = Arc::new(MmapManager::new(Default::default()));
        // let zero_copy = Arc::new(ZeroCopyTransfer::new(mmap_manager));
        
        Self {
            config,
            chunk_locks: Arc::new(DashMap::new()),
            // zero_copy,
            metadata_cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(cache_size).unwrap()))),
            io_semaphore: Arc::new(Semaphore::new(concurrent_io)),
            batch_size_limit: batch_limit,
            metrics: Arc::new(StorageMetrics::default()),
        }
    }
    
    /// Get a lock for a specific chunk to prevent concurrent access
    async fn get_chunk_lock(&self, chunk_id: ChunkId, version: ChunkVersion) -> Arc<tokio::sync::Mutex<()>> {
        let key = (chunk_id, version);
        // Get or create mutex and return Arc to it
        self.chunk_locks
            .entry(key)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
    
    /// Ensure the directory structure exists for a chunk
    async fn ensure_chunk_directory(&self, chunk_id: ChunkId) -> Result<()> {
        let dir_path = self.config.chunk_dir_path(chunk_id);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path).await.map_err(|e| {
                ChunkServerError::Storage(format!("Failed to create chunk directory {}: {}", 
                                                  dir_path.display(), e))
            })?;
        }
        Ok(())
    }
    
    /// Write chunk data to file
    async fn write_chunk_data(&self, chunk: &Chunk) -> Result<()> {
        let chunk_path = self.config.chunk_file_path(chunk.id(), chunk.version());
        let mut temp_path = chunk_path.clone();
        temp_path.set_extension("tmp");
        
        // Write to temporary file first
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)
            .await
            .map_err(|e| ChunkServerError::Io(e))?;
            
        file.write_all(&chunk.data).await.map_err(|e| ChunkServerError::Io(e))?;
        file.sync_all().await.map_err(|e| ChunkServerError::Io(e))?;
        drop(file);
        
        // Atomically rename to final location
        fs::rename(&temp_path, &chunk_path).await.map_err(|e| {
            ChunkServerError::Storage(format!("Failed to rename {} to {}: {}", 
                                              temp_path.display(), chunk_path.display(), e))
        })?;
        
        debug!("Wrote chunk {} v{} to {}", chunk.id(), chunk.version(), chunk_path.display());
        Ok(())
    }
    
    /// Write chunk metadata to file
    async fn write_chunk_metadata(&self, metadata: &ChunkMetadata) -> Result<()> {
        let metadata_path = self.config.chunk_metadata_path(metadata.chunk_id, metadata.version);
        let mut temp_path = metadata_path.clone();
        temp_path.set_extension("meta.tmp");
        
        let serialized = bincode::serialize(metadata)?;
        
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)
            .await
            .map_err(|e| ChunkServerError::Io(e))?;
            
        file.write_all(&serialized).await.map_err(|e| ChunkServerError::Io(e))?;
        file.sync_all().await.map_err(|e| ChunkServerError::Io(e))?;
        drop(file);
        
        // Atomically rename to final location
        fs::rename(&temp_path, &metadata_path).await.map_err(|e| {
            ChunkServerError::Storage(format!("Failed to rename {} to {}: {}", 
                                              temp_path.display(), metadata_path.display(), e))
        })?;
        
        debug!("Wrote metadata for chunk {} v{} to {}", 
               metadata.chunk_id, metadata.version, metadata_path.display());
        Ok(())
    }
    
    /// Read chunk data from file using optimized I/O
    async fn read_chunk_data(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<Bytes> {
        let start_time = Instant::now();
        let _permit = self.io_semaphore.acquire().await.map_err(|_| 
            ChunkServerError::InvalidOperation("Failed to acquire I/O semaphore".to_string()))?;
        
        let chunk_path = self.config.chunk_file_path(chunk_id, version);
        
        if !chunk_path.exists() {
            return Err(ChunkServerError::ChunkNotFound { chunk_id });
        }
        
        // Read chunk data (TODO: Re-enable zero-copy optimization)
        let data = fs::read(&chunk_path).await.map_err(|e| {
            error!("Failed to read chunk {} v{} from {}: {}", 
                   chunk_id, version, chunk_path.display(), e);
            ChunkServerError::Io(e)
        })?;
        let data = Bytes::from(data);
        
        // Update metrics
        let duration = start_time.elapsed();
        self.metrics.read_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics.total_read_time_micros.fetch_add(
            duration.as_micros() as u64, 
            std::sync::atomic::Ordering::Relaxed
        );
        
        debug!("Read {} bytes for chunk {} v{} from {} in {:?}", 
               data.len(), chunk_id, version, chunk_path.display(), duration);
        Ok(data)
    }
    
    /// Read a slice of chunk data without loading the entire chunk
    async fn read_chunk_data_slice(&self, chunk_id: ChunkId, version: ChunkVersion, offset: u64, length: u64) -> Result<Bytes> {
        let start_time = Instant::now();
        let _permit = self.io_semaphore.acquire().await.map_err(|_| 
            ChunkServerError::InvalidOperation("Failed to acquire I/O semaphore".to_string()))?;
        
        let chunk_path = self.config.chunk_file_path(chunk_id, version);
        
        if !chunk_path.exists() {
            return Err(ChunkServerError::ChunkNotFound { chunk_id });
        }
        
        // Read chunk slice (TODO: Re-enable zero-copy optimization)
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let mut file = tokio::fs::File::open(&chunk_path).await.map_err(|e| ChunkServerError::Io(e))?;
        file.seek(tokio::io::SeekFrom::Start(offset)).await.map_err(|e| ChunkServerError::Io(e))?;
        
        let mut buffer = vec![0u8; length as usize];
        file.read_exact(&mut buffer).await.map_err(|e| ChunkServerError::Io(e))?;
        let data = Bytes::from(buffer);
        
        // Update metrics
        let duration = start_time.elapsed();
        self.metrics.read_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics.total_read_time_micros.fetch_add(
            duration.as_micros() as u64, 
            std::sync::atomic::Ordering::Relaxed
        );
        
        debug!("Read slice {} bytes (offset {}) for chunk {} v{} in {:?}", 
               data.len(), offset, chunk_id, version, duration);
        Ok(data)
    }
    
    /// Read chunk metadata from file with caching
    async fn read_chunk_metadata(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<ChunkMetadata> {
        let key = (chunk_id, version);
        
        // Check cache first
        {
            let cache = self.metadata_cache.read().await;
            if let Some(metadata) = cache.peek(&key) {
                self.metrics.cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                debug!("Cache hit for metadata chunk {} v{}", chunk_id, version);
                return Ok(metadata.clone());
            }
        }
        
        // Cache miss, read from disk
        self.metrics.cache_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        let metadata_path = self.config.chunk_metadata_path(chunk_id, version);
        
        if !metadata_path.exists() {
            return Err(ChunkServerError::ChunkNotFound { chunk_id });
        }
        
        let data = fs::read(&metadata_path).await.map_err(|e| {
            error!("Failed to read metadata for chunk {} v{} from {}: {}", 
                   chunk_id, version, metadata_path.display(), e);
            ChunkServerError::Io(e)
        })?;
        
        let metadata: ChunkMetadata = bincode::deserialize(&data)?;
        
        // Update cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.put(key, metadata.clone());
        }
        
        debug!("Read metadata for chunk {} v{} from {} (cached)", 
               chunk_id, version, metadata_path.display());
        Ok(metadata)
    }
    
    /// Delete chunk files
    async fn delete_chunk_files(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<()> {
        let chunk_path = self.config.chunk_file_path(chunk_id, version);
        let metadata_path = self.config.chunk_metadata_path(chunk_id, version);
        
        // Try to delete both files, continue even if one fails
        let mut errors = Vec::new();
        
        if chunk_path.exists() {
            if let Err(e) = fs::remove_file(&chunk_path).await {
                errors.push(format!("Failed to delete {}: {}", chunk_path.display(), e));
            } else {
                debug!("Deleted chunk file {}", chunk_path.display());
            }
        }
        
        if metadata_path.exists() {
            if let Err(e) = fs::remove_file(&metadata_path).await {
                errors.push(format!("Failed to delete {}: {}", metadata_path.display(), e));
            } else {
                debug!("Deleted metadata file {}", metadata_path.display());
            }
        }
        
        if !errors.is_empty() {
            return Err(ChunkServerError::Storage(errors.join("; ")));
        }
        
        Ok(())
    }
    
    /// Calculate available disk space
    async fn calculate_free_space(&self) -> Result<u64> {
        // Use statvfs to get filesystem statistics
        use nix::sys::statvfs::statvfs;
        
        let stats = statvfs(&self.config.data_dir).map_err(|e| {
            ChunkServerError::Storage(format!("Failed to get filesystem stats: {}", e))
        })?;
        
        let free_bytes = stats.blocks_available() * stats.block_size();
        Ok(free_bytes)
    }
    
    /// Get storage performance metrics
    pub fn get_metrics(&self) -> Arc<StorageMetrics> {
        self.metrics.clone()
    }
    
    /// Clear metadata cache
    pub async fn clear_cache(&self) {
        let mut cache = self.metadata_cache.write().await;
        cache.clear();
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.metadata_cache.read().await;
        (cache.len(), cache.cap().get())
    }
}

#[async_trait::async_trait]
impl ChunkStorage for FileStorage {
    async fn store_chunk(&self, chunk: &Chunk) -> Result<()> {
        let lock = self.get_chunk_lock(chunk.id(), chunk.version()).await;
        let _guard = lock.lock().await;
        
        // Check if chunk already exists
        if self.chunk_exists(chunk.id(), chunk.version()).await? {
            return Err(ChunkServerError::ChunkAlreadyExists { 
                chunk_id: chunk.id() 
            });
        }
        
        // Verify chunk integrity before storing
        if !chunk.verify_integrity() {
            return Err(ChunkServerError::ChecksumMismatch {
                chunk_id: chunk.id(),
                expected: chunk.metadata.checksum.hex(),
                actual: "invalid".to_string(),
            });
        }
        
        // Ensure directory structure exists
        self.ensure_chunk_directory(chunk.id()).await?;
        
        // Write chunk data and metadata
        self.write_chunk_data(chunk).await?;
        self.write_chunk_metadata(&chunk.metadata).await?;
        
        info!("Successfully stored chunk {} v{} ({} bytes)", 
              chunk.id(), chunk.version(), chunk.size());
        Ok(())
    }
    
    async fn get_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<Chunk> {
        let lock = self.get_chunk_lock(chunk_id, version).await;
        let _guard = lock.lock().await;
        
        // Read metadata first
        let mut metadata = self.read_chunk_metadata(chunk_id, version).await?;
        
        // Read chunk data
        let data = self.read_chunk_data(chunk_id, version).await?;
        
        // Verify size matches
        if data.len() as u64 != metadata.size {
            return Err(ChunkServerError::InvalidChunkSize {
                expected: metadata.size,
                actual: data.len() as u64,
            });
        }
        
        // Create chunk and verify integrity
        let chunk = Chunk {
            metadata: metadata.clone(),
            data,
        };
        
        if !chunk.verify_integrity() {
            metadata.mark_corrupted();
            self.update_chunk_metadata(&metadata).await?;
            
            return Err(ChunkServerError::ChecksumMismatch {
                chunk_id,
                expected: metadata.checksum.hex(),
                actual: "corrupted".to_string(),
            });
        }
        
        info!("Successfully retrieved chunk {} v{} ({} bytes)", 
              chunk_id, version, chunk.size());
        Ok(chunk)
    }
    
    async fn chunk_exists(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool> {
        let chunk_path = self.config.chunk_file_path(chunk_id, version);
        let metadata_path = self.config.chunk_metadata_path(chunk_id, version);
        
        Ok(chunk_path.exists() && metadata_path.exists())
    }
    
    async fn delete_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<()> {
        let lock = self.get_chunk_lock(chunk_id, version).await;
        let _guard = lock.lock().await;
        
        if !self.chunk_exists(chunk_id, version).await? {
            return Err(ChunkServerError::ChunkNotFound { chunk_id });
        }
        
        self.delete_chunk_files(chunk_id, version).await?;
        
        info!("Successfully deleted chunk {} v{}", chunk_id, version);
        Ok(())
    }
    
    async fn list_chunks(&self) -> Result<Vec<(ChunkId, ChunkVersion)>> {
        let mut chunks = Vec::new();
        let mut stack = vec![self.config.data_dir.clone()];
        
        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(&dir).await.map_err(|e| {
                ChunkServerError::Storage(format!("Failed to read directory {}: {}", dir.display(), e))
            })?;
            
            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                ChunkServerError::Storage(format!("Failed to read directory entry: {}", e))
            })? {
                let path = entry.path();
                
                if path.is_dir() {
                    stack.push(path);
                } else if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // Parse chunk filename: chunk_<chunk_id>_v<version>.dat
                    if filename.starts_with("chunk_") && filename.ends_with(".dat") {
                        if let Some(parsed) = parse_chunk_filename(filename) {
                            chunks.push(parsed);
                        }
                    }
                }
            }
        }
        
        chunks.sort_by_key(|(chunk_id, version)| (*chunk_id, *version));
        debug!("Found {} chunks", chunks.len());
        Ok(chunks)
    }
    
    async fn get_chunk_metadata(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<ChunkMetadata> {
        self.read_chunk_metadata(chunk_id, version).await
    }
    
    async fn update_chunk_metadata(&self, metadata: &ChunkMetadata) -> Result<()> {
        let lock = self.get_chunk_lock(metadata.chunk_id, metadata.version).await;
        let _guard = lock.lock().await;
        self.write_chunk_metadata(metadata).await
    }
    
    async fn verify_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool> {
        match self.get_chunk(chunk_id, version).await {
            Ok(chunk) => Ok(chunk.verify_integrity()),
            Err(ChunkServerError::ChecksumMismatch { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
    
    async fn get_stats(&self) -> Result<StorageStats> {
        let chunks = self.list_chunks().await?;
        let total_chunks = chunks.len() as u64;
        
        let mut total_bytes = 0u64;
        let mut corrupted_chunks = 0u64;
        
        for (chunk_id, version) in chunks {
            match self.get_chunk_metadata(chunk_id, version).await {
                Ok(metadata) => {
                    total_bytes += metadata.size;
                    if metadata.is_corrupted {
                        corrupted_chunks += 1;
                    }
                }
                Err(_) => {
                    // Count metadata read failures as corrupted
                    corrupted_chunks += 1;
                }
            }
        }
        
        let free_space_bytes = self.calculate_free_space().await.unwrap_or(0);
        
        Ok(StorageStats {
            total_chunks,
            total_bytes,
            free_space_bytes,
            corrupted_chunks,
        })
    }
    
    // New performance-optimized methods implementation
    
    async fn store_chunks_batch(&self, chunks: &[&Chunk]) -> Result<Vec<Result<()>>> {
        let start_time = Instant::now();
        self.metrics.batch_operations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        let mut results = Vec::with_capacity(chunks.len());
        let batch_size = self.batch_size_limit.min(chunks.len());
        
        for chunk_batch in chunks.chunks(batch_size) {
            let mut batch_futures = Vec::with_capacity(chunk_batch.len());
            
            for chunk in chunk_batch {
                let future = self.store_chunk(chunk);
                batch_futures.push(future);
            }
            
            let batch_results = futures::future::join_all(batch_futures).await;
            results.extend(batch_results);
        }
        
        let duration = start_time.elapsed();
        debug!("Stored {} chunks in batch in {:?}", chunks.len(), duration);
        
        Ok(results)
    }
    
    async fn get_chunks_batch(&self, chunk_ids: &[(ChunkId, ChunkVersion)]) -> Result<Vec<Result<Chunk>>> {
        let start_time = Instant::now();
        self.metrics.batch_operations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        let mut results = Vec::with_capacity(chunk_ids.len());
        let batch_size = self.batch_size_limit.min(chunk_ids.len());
        
        for batch in chunk_ids.chunks(batch_size) {
            let mut batch_futures = Vec::with_capacity(batch.len());
            
            for (chunk_id, version) in batch {
                let future = self.get_chunk(*chunk_id, *version);
                batch_futures.push(future);
            }
            
            let batch_results = futures::future::join_all(batch_futures).await;
            results.extend(batch_results);
        }
        
        let duration = start_time.elapsed();
        debug!("Retrieved {} chunks in batch in {:?}", chunk_ids.len(), duration);
        
        Ok(results)
    }
    
    async fn verify_chunk_fast(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool> {
        self.metrics.fast_verifications.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // Read metadata first to get checksum type
        let metadata = self.read_chunk_metadata(chunk_id, version).await?;
        
        // For hybrid checksums, use fast verification
        match metadata.checksum.checksum_type {
            ChecksumType::HybridFast | ChecksumType::HybridSecure => {
                let data = self.read_chunk_data(chunk_id, version).await?;
                Ok(metadata.checksum.verify_fast(&data))
            }
            _ => {
                // Fall back to full verification for other types
                self.verify_chunk(chunk_id, version).await
            }
        }
    }
    
    async fn read_chunk_slice(&self, chunk_id: ChunkId, version: ChunkVersion, offset: u64, length: u64) -> Result<Bytes> {
        self.read_chunk_data_slice(chunk_id, version, offset, length).await
    }
    
    async fn preload_chunks(&self, chunk_ids: &[(ChunkId, ChunkVersion)]) -> Result<()> {
        let start_time = Instant::now();
        
        // Preload metadata into cache
        let mut preload_futures = Vec::with_capacity(chunk_ids.len());
        
        for (chunk_id, version) in chunk_ids {
            let future = self.read_chunk_metadata(*chunk_id, *version);
            preload_futures.push(future);
        }
        
        let _results = futures::future::join_all(preload_futures).await;
        
        let duration = start_time.elapsed();
        debug!("Preloaded {} chunk metadata entries in {:?}", chunk_ids.len(), duration);
        
        Ok(())
    }
}

/// Parse chunk filename to extract chunk ID and version
fn parse_chunk_filename(filename: &str) -> Option<(ChunkId, ChunkVersion)> {
    // Format: chunk_<chunk_id>_v<version>.dat
    if !filename.starts_with("chunk_") || !filename.ends_with(".dat") {
        return None;
    }
    
    let without_prefix = &filename[6..]; // Remove "chunk_"
    let without_suffix = &without_prefix[..without_prefix.len() - 4]; // Remove ".dat"
    
    let parts: Vec<&str> = without_suffix.split("_v").collect();
    if parts.len() != 2 {
        return None;
    }
    
    let chunk_id = u64::from_str_radix(parts[0], 16).ok()?;
    let version = u32::from_str_radix(parts[1], 16).ok()?;
    
    Some((chunk_id, version))
}

/// Storage manager that coordinates multiple storage backends
pub struct StorageManager {
    primary_storage: Arc<dyn ChunkStorage>,
    block_allocator: Arc<DynamicBlockAllocator>,
    // Tiered storage integration
    tiered_storage_manager: Option<Arc<crate::tiered_storage::TieredStorageManager>>,
    movement_engine: Option<Arc<crate::tier_movement::DataMovementEngine>>,
    object_backends: std::collections::HashMap<crate::tiered_storage::StorageTier, Arc<crate::object_storage::ObjectStorageBackend>>,
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(primary_storage: Arc<dyn ChunkStorage>, total_storage_size: u64) -> Self {
        let block_allocator = Arc::new(DynamicBlockAllocator::new(total_storage_size));
        Self { 
            primary_storage,
            block_allocator,
            tiered_storage_manager: None,
            movement_engine: None,
            object_backends: std::collections::HashMap::new(),
        }
    }
    
    /// Create a new storage manager with tiered storage enabled
    pub async fn new_with_tiered_storage(
        primary_storage: Arc<dyn ChunkStorage>, 
        total_storage_size: u64,
        tiered_config: Option<crate::tiered_storage::TierConfig>
    ) -> Result<Self> {
        let block_allocator = Arc::new(DynamicBlockAllocator::new(total_storage_size));
        let mut manager = Self { 
            primary_storage,
            block_allocator,
            tiered_storage_manager: None,
            movement_engine: None,
            object_backends: std::collections::HashMap::new(),
        };
        
        if let Some(_config) = tiered_config {
            manager.initialize_tiered_storage().await?;
        }
        
        Ok(manager)
    }
    
    /// Initialize tiered storage components
    pub async fn initialize_tiered_storage(&mut self) -> Result<()> {
        use crate::tiered_storage::{TieredStorageManager, TierConfig, StorageTier};
        use crate::tier_movement::DataMovementEngine;
        use crate::object_storage::{ObjectStorageBackend, StorageProvider};
        use std::path::PathBuf;
        
        // Create tiered storage manager
        let tiered_storage_manager = Arc::new(TieredStorageManager::new());
        
        // Configure default tiers
        let hot_config = TierConfig::hot_tier(
            PathBuf::from("/tmp/mooseng/hot"),
            1024 * 1024 * 1024, // 1GB hot tier
        );
        tiered_storage_manager.add_tier_config(hot_config).await?;
        
        let warm_config = TierConfig::warm_tier(
            PathBuf::from("/tmp/mooseng/warm"),
            10 * 1024 * 1024 * 1024, // 10GB warm tier
        );
        tiered_storage_manager.add_tier_config(warm_config).await?;
        
        let cold_config = TierConfig::cold_tier(
            "mooseng-cold".to_string(),
            "us-west-2".to_string(),
            100 * 1024 * 1024 * 1024, // 100GB cold tier
        );
        tiered_storage_manager.add_tier_config(cold_config).await?;
        
        // Create movement engine
        let movement_engine = Arc::new(DataMovementEngine::new(tiered_storage_manager.clone()));
        
        // Create object storage backends for cold and archive tiers
        let memory_backend = Arc::new(
            ObjectStorageBackend::new(
                StorageProvider::Memory,
                cold_config.object_storage.clone().unwrap()
            ).await?
        );
        self.object_backends.insert(StorageTier::Cold, memory_backend);
        
        self.tiered_storage_manager = Some(tiered_storage_manager);
        self.movement_engine = Some(movement_engine);
        
        info!("Tiered storage initialized successfully");
        Ok(())
    }
    
    /// Check if tiered storage is enabled
    pub fn is_tiered_storage_enabled(&self) -> bool {
        self.tiered_storage_manager.is_some()
    }
    
    /// Get the tiered storage manager
    pub fn tiered_storage_manager(&self) -> Option<&Arc<crate::tiered_storage::TieredStorageManager>> {
        self.tiered_storage_manager.as_ref()
    }
    
    /// Get the movement engine
    pub fn movement_engine(&self) -> Option<&Arc<crate::tier_movement::DataMovementEngine>> {
        self.movement_engine.as_ref()
    }
    
    /// Store chunk with tiered storage awareness
    pub async fn store_chunk_tiered(&self, chunk: &Chunk) -> Result<()> {
        // Store to primary storage first
        self.primary_storage.store_chunk(chunk).await?;
        
        // If tiered storage is enabled, record access and classify
        if let Some(ref tiered_manager) = self.tiered_storage_manager {
            // Record the chunk access
            tiered_manager.record_chunk_access(chunk.id()).await?;
            
            // Classify the chunk and potentially schedule movement
            if let Some(ref movement_engine) = self.movement_engine {
                if let Ok(optimal_tier) = tiered_manager.get_optimal_tier(chunk.id()).await {
                    if let Ok(Some(target_tier)) = tiered_manager.should_move_chunk(chunk.id()).await {
                        movement_engine.schedule_movement(
                            chunk.id(),
                            target_tier,
                            crate::tier_movement::MovementReason::AutomaticAccess
                        ).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Retrieve chunk with tiered storage awareness
    pub async fn get_chunk_tiered(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<Chunk> {
        // If tiered storage is enabled, record access
        if let Some(ref tiered_manager) = self.tiered_storage_manager {
            tiered_manager.record_chunk_access(chunk_id).await?;
        }
        
        // Try primary storage first
        match self.primary_storage.get_chunk(chunk_id, version).await {
            Ok(chunk) => Ok(chunk),
            Err(ChunkServerError::ChunkNotFound { .. }) => {
                // If not found in primary storage, try object backends
                self.get_chunk_from_object_storage(chunk_id, version).await
            }
            Err(e) => Err(e),
        }
    }
    
    /// Attempt to retrieve chunk from object storage tiers
    async fn get_chunk_from_object_storage(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<Chunk> {
        for (tier, backend) in &self.object_backends {
            // This is a simplified approach - in practice, we'd need to track
            // which tier contains which chunks
            info!("Attempting to retrieve chunk {} v{} from tier {:?}", chunk_id, version, tier);
            
            // Generate object metadata (in practice, this would be stored)
            let object_metadata = crate::object_storage::ChunkObjectMetadata {
                chunk_id,
                object_path: format!("chunks/{:016x}_v{:08x}", chunk_id, version),
                size: 0, // Would be populated from stored metadata
                uploaded_at: 0,
                etag: None,
                checksum: String::new(),
                encryption: None,
                lifecycle: crate::tiered_storage::LifecycleMetadata {
                    storage_class: "STANDARD".to_string(),
                    transitions: Vec::new(),
                    expires_at: None,
                },
            };
            
            match backend.download_chunk(&object_metadata).await {
                Ok(data) => {
                    // Reconstruct chunk from downloaded data
                    let chunk = Chunk::new(
                        chunk_id,
                        version,
                        data,
                        crate::chunk::ChecksumType::Blake3,
                        1,
                    );
                    
                    info!("Successfully retrieved chunk {} v{} from tier {:?}", chunk_id, version, tier);
                    return Ok(chunk);
                }
                Err(_) => {
                    // Continue trying other tiers
                    continue;
                }
            }
        }
        
        Err(ChunkServerError::ChunkNotFound { chunk_id })
    }
    
    /// Get performance metrics including tiered storage stats
    pub async fn get_comprehensive_stats(&self) -> Result<ComprehensiveStorageStats> {
        let primary_stats = self.primary_storage.get_stats().await?;
        let allocation_stats = self.block_allocator.get_free_space_info().await?;
        
        let tiered_stats = if let Some(ref tiered_manager) = self.tiered_storage_manager {
            Some(tiered_manager.get_performance_metrics().await)
        } else {
            None
        };
        
        let movement_stats = if let Some(ref movement_engine) = self.movement_engine {
            Some(movement_engine.get_stats().await)
        } else {
            None
        };
        
        Ok(ComprehensiveStorageStats {
            primary_storage: primary_stats,
            allocation: allocation_stats,
            tiered_storage: tiered_stats,
            movement: movement_stats,
        })
    }
    
    /// Start tiered storage background services
    pub async fn start_tiered_storage_services(&self) -> Result<()> {
        if let Some(ref movement_engine) = self.movement_engine {
            movement_engine.clone().start().await?;
            info!("Started tiered storage movement engine");
        }
        
        Ok(())
    }
    
    /// Get the primary storage backend
    pub fn primary(&self) -> &Arc<dyn ChunkStorage> {
        &self.primary_storage
    }
    
    /// Get storage statistics
    pub async fn get_stats(&self) -> Result<StorageStats> {
        self.primary_storage.get_stats().await
    }
    
    /// Allocate storage space for a chunk with workload hint
    pub async fn allocate_chunk_space(&self, size: u64, hint: WorkloadHint) -> Result<AllocatedBlock> {
        self.block_allocator.allocate_with_hint(size, hint).await
    }
    
    /// Free previously allocated storage space
    pub async fn free_chunk_space(&self, block: &AllocatedBlock) -> Result<()> {
        self.block_allocator.free(block).await
    }
    
    /// Get current allocation strategy name
    pub async fn get_allocation_strategy(&self) -> String {
        self.block_allocator.get_current_strategy_name().await
    }
    
    /// Get free space information from the allocator
    pub async fn get_allocation_stats(&self) -> Result<crate::block_allocator::FreeSpaceInfo> {
        self.block_allocator.get_free_space_info().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn create_test_config() -> (ChunkServerConfig, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ChunkServerConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        (config, temp_dir)
    }
    
    fn create_test_chunk() -> Chunk {
        let data = Bytes::from("Hello, World!");
        Chunk::new(12345, 1, data, ChecksumType::HybridSecure, 1)
    }
    
    fn create_test_chunk_with_checksum(checksum_type: ChecksumType) -> Chunk {
        let data = Bytes::from("Hello, World! This is test data.");
        Chunk::new(12345, 1, data, checksum_type, 1)
    }
    
    #[tokio::test]
    async fn test_file_storage_store_and_retrieve() {
        let (config, _temp_dir) = create_test_config();
        let storage = FileStorage::new(Arc::new(config));
        let chunk = create_test_chunk();
        
        // Store chunk
        storage.store_chunk(&chunk).await.unwrap();
        
        // Verify it exists
        assert!(storage.chunk_exists(chunk.id(), chunk.version()).await.unwrap());
        
        // Retrieve chunk
        let retrieved = storage.get_chunk(chunk.id(), chunk.version()).await.unwrap();
        assert_eq!(chunk.data, retrieved.data);
        assert_eq!(chunk.metadata.chunk_id, retrieved.metadata.chunk_id);
        assert_eq!(chunk.metadata.version, retrieved.metadata.version);
    }
    
    #[tokio::test]
    async fn test_file_storage_delete() {
        let (config, _temp_dir) = create_test_config();
        let storage = FileStorage::new(Arc::new(config));
        let chunk = create_test_chunk();
        
        // Store and delete chunk
        storage.store_chunk(&chunk).await.unwrap();
        storage.delete_chunk(chunk.id(), chunk.version()).await.unwrap();
        
        // Verify it's gone
        assert!(!storage.chunk_exists(chunk.id(), chunk.version()).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_parse_chunk_filename() {
        assert_eq!(
            parse_chunk_filename("chunk_000000000000abcd_v00000001.dat"),
            Some((0xabcd, 1))
        );
        
        assert_eq!(
            parse_chunk_filename("chunk_123456789abcdef0_v00000042.dat"),
            Some((0x123456789abcdef0, 0x42))
        );
        
        assert_eq!(parse_chunk_filename("invalid.dat"), None);
        assert_eq!(parse_chunk_filename("chunk_invalid_v1.dat"), None);
    }
    
    #[tokio::test]
    async fn test_new_checksum_types() {
        let chunk_xxhash3 = create_test_chunk_with_checksum(ChecksumType::XxHash3);
        let chunk_hybrid_fast = create_test_chunk_with_checksum(ChecksumType::HybridFast);
        let chunk_hybrid_secure = create_test_chunk_with_checksum(ChecksumType::HybridSecure);
        
        // Verify all chunks have proper integrity
        assert!(chunk_xxhash3.verify_integrity());
        assert!(chunk_hybrid_fast.verify_integrity());
        assert!(chunk_hybrid_secure.verify_integrity());
        
        // Test fast verification on hybrid types
        assert!(chunk_hybrid_fast.metadata.checksum.verify_fast(&chunk_hybrid_fast.data));
        assert!(chunk_hybrid_secure.metadata.checksum.verify_fast(&chunk_hybrid_secure.data));
    }
    
    #[tokio::test]
    async fn test_batch_operations() {
        let (config, _temp_dir) = create_test_config();
        let storage = FileStorage::new(Arc::new(config));
        
        // Create multiple test chunks
        let chunks: Vec<Chunk> = (0..5).map(|i| {
            let data = Bytes::from(format!("Test data {}", i));
            Chunk::new(i as u64, 1, data, ChecksumType::HybridFast, 1)
        }).collect();
        
        let chunk_refs: Vec<&Chunk> = chunks.iter().collect();
        
        // Store chunks in batch
        let store_results = storage.store_chunks_batch(&chunk_refs).await.unwrap();
        assert_eq!(store_results.len(), 5);
        assert!(store_results.iter().all(|r| r.is_ok()));
        
        // Retrieve chunks in batch
        let chunk_ids: Vec<(ChunkId, ChunkVersion)> = chunks.iter()
            .map(|c| (c.id(), c.version()))
            .collect();
        
        let retrieve_results = storage.get_chunks_batch(&chunk_ids).await.unwrap();
        assert_eq!(retrieve_results.len(), 5);
        assert!(retrieve_results.iter().all(|r| r.is_ok()));
        
        // Verify retrieved data matches original
        for (original, retrieved_result) in chunks.iter().zip(retrieve_results.iter()) {
            let retrieved = retrieved_result.as_ref().unwrap();
            assert_eq!(original.data, retrieved.data);
            assert_eq!(original.metadata.chunk_id, retrieved.metadata.chunk_id);
        }
    }
    
    #[tokio::test]
    async fn test_fast_verification() {
        let (config, _temp_dir) = create_test_config();
        let storage = FileStorage::new(Arc::new(config));
        let chunk = create_test_chunk_with_checksum(ChecksumType::HybridSecure);
        
        // Store chunk
        storage.store_chunk(&chunk).await.unwrap();
        
        // Test fast verification
        let is_valid = storage.verify_chunk_fast(chunk.id(), chunk.version()).await.unwrap();
        assert!(is_valid);
        
        // Check metrics
        let metrics = storage.get_metrics();
        assert_eq!(metrics.fast_verifications.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
    
    #[tokio::test]
    async fn test_chunk_slice_reading() {
        let (config, _temp_dir) = create_test_config();
        let storage = FileStorage::new(Arc::new(config));
        
        let data = Bytes::from("0123456789ABCDEF"); // 16 bytes
        let chunk = Chunk::new(999, 1, data, ChecksumType::HybridFast, 1);
        
        // Store chunk
        storage.store_chunk(&chunk).await.unwrap();
        
        // Read slice
        let slice = storage.read_chunk_slice(999, 1, 4, 8).await.unwrap();
        assert_eq!(slice, "456789AB");
        
        // Read from beginning
        let beginning = storage.read_chunk_slice(999, 1, 0, 4).await.unwrap();
        assert_eq!(beginning, "0123");
        
        // Read to end
        let end = storage.read_chunk_slice(999, 1, 12, 4).await.unwrap();
        assert_eq!(end, "CDEF");
    }
}