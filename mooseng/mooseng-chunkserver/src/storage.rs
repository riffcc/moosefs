use crate::{
    chunk::{Chunk, ChunkMetadata, ChunkChecksum, ChecksumType},
    config::ChunkServerConfig,
    error::{ChunkServerError, Result},
};
use mooseng_common::types::{ChunkId, ChunkVersion};
use bytes::Bytes;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use dashmap::DashMap;
use tracing::{debug, error, info, warn};

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
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_chunks: u64,
    pub total_bytes: u64,
    pub free_space_bytes: u64,
    pub corrupted_chunks: u64,
}

/// File-based chunk storage implementation
pub struct FileStorage {
    config: Arc<ChunkServerConfig>,
    chunk_locks: DashMap<(ChunkId, ChunkVersion), tokio::sync::Mutex<()>>,
}

impl FileStorage {
    /// Create a new file storage instance
    pub fn new(config: Arc<ChunkServerConfig>) -> Self {
        Self {
            config,
            chunk_locks: DashMap::new(),
        }
    }
    
    /// Get a lock for a specific chunk to prevent concurrent access
    async fn get_chunk_lock(&self, chunk_id: ChunkId, version: ChunkVersion) -> tokio::sync::MutexGuard<'_, ()> {
        let key = (chunk_id, version);
        let mutex = self.chunk_locks.entry(key).or_insert_with(|| tokio::sync::Mutex::new(()));
        mutex.value().lock().await
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
    
    /// Read chunk data from file
    async fn read_chunk_data(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<Bytes> {
        let chunk_path = self.config.chunk_file_path(chunk_id, version);
        
        if !chunk_path.exists() {
            return Err(ChunkServerError::ChunkNotFound { chunk_id });
        }
        
        let data = fs::read(&chunk_path).await.map_err(|e| {
            error!("Failed to read chunk {} v{} from {}: {}", 
                   chunk_id, version, chunk_path.display(), e);
            ChunkServerError::Io(e)
        })?;
        
        debug!("Read {} bytes for chunk {} v{} from {}", 
               data.len(), chunk_id, version, chunk_path.display());
        Ok(Bytes::from(data))
    }
    
    /// Read chunk metadata from file
    async fn read_chunk_metadata(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<ChunkMetadata> {
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
        
        debug!("Read metadata for chunk {} v{} from {}", 
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
}

#[async_trait::async_trait]
impl ChunkStorage for FileStorage {
    async fn store_chunk(&self, chunk: &Chunk) -> Result<()> {
        let _lock = self.get_chunk_lock(chunk.id(), chunk.version()).await;
        
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
        let _lock = self.get_chunk_lock(chunk_id, version).await;
        
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
        let _lock = self.get_chunk_lock(chunk_id, version).await;
        
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
        let _lock = self.get_chunk_lock(metadata.chunk_id, metadata.version).await;
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
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(primary_storage: Arc<dyn ChunkStorage>) -> Self {
        Self { primary_storage }
    }
    
    /// Get the primary storage backend
    pub fn primary(&self) -> &Arc<dyn ChunkStorage> {
        &self.primary_storage
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
        Chunk::new(12345, 1, data, ChecksumType::Blake3, 1)
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
}