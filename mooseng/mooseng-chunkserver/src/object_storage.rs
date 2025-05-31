//! Object storage backend integration for cold and archive tiers
//! 
//! This module provides implementations for integrating with various object storage
//! providers (AWS S3, Azure Blob Storage, Google Cloud Storage) using the object_store crate.

use anyhow::{Result, Context, anyhow};
use mooseng_common::types::{ChunkId, now_micros};
use object_store::{
    ObjectStore, ObjectMeta, GetResult, ListResult,
    aws::AmazonS3Builder,
    azure::MicrosoftAzureBuilder, 
    gcp::GoogleCloudStorageBuilder,
    local::LocalFileSystem,
    memory::InMemory,
    path::Path as ObjectPath,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn, error, instrument};
use bytes::Bytes;

use crate::tiered_storage::{
    ObjectStorageConfig, CacheConfig, CredentialsConfig, LifecycleConfig
};

/// Object storage provider types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageProvider {
    /// Amazon S3
    S3,
    /// Azure Blob Storage
    Azure,
    /// Google Cloud Storage
    GCS,
    /// Local filesystem (for testing)
    Local,
    /// In-memory storage (for testing)
    Memory,
}

/// Object storage backend configuration
#[derive(Debug, Clone)]
pub struct ObjectStorageBackend {
    /// Storage provider type
    provider: StorageProvider,
    /// Object store implementation
    store: Arc<dyn ObjectStore>,
    /// Configuration
    config: ObjectStorageConfig,
    /// Local cache for frequently accessed objects
    cache: Option<Arc<ObjectCache>>,
    /// Upload/download semaphore for rate limiting
    operation_semaphore: Arc<Semaphore>,
    /// Performance metrics
    metrics: Arc<RwLock<ObjectStorageMetrics>>,
}

/// Local object cache for frequently accessed data
#[derive(Debug)]
pub struct ObjectCache {
    /// Cache directory
    cache_dir: PathBuf,
    /// Maximum cache size in bytes
    max_size: u64,
    /// Current cache size
    current_size: Arc<RwLock<u64>>,
    /// Cache metadata tracking
    metadata: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Cache configuration
    config: CacheConfig,
}

/// Cache entry metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Cache file path
    path: PathBuf,
    /// Object size in bytes
    size: u64,
    /// Last access time
    last_accessed: SystemTime,
    /// Creation time
    created_at: SystemTime,
    /// Access count
    access_count: u64,
    /// ETag or version identifier
    etag: Option<String>,
}

/// Object storage performance metrics
#[derive(Debug, Clone, Default)]
pub struct ObjectStorageMetrics {
    /// Upload operations
    pub uploads_total: u64,
    pub uploads_success: u64,
    pub uploads_failed: u64,
    pub upload_bytes_total: u64,
    pub avg_upload_duration_ms: f64,
    
    /// Download operations
    pub downloads_total: u64,
    pub downloads_success: u64,
    pub downloads_failed: u64,
    pub download_bytes_total: u64,
    pub avg_download_duration_ms: f64,
    
    /// Cache statistics
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_evictions: u64,
    pub cache_size_bytes: u64,
    
    /// Error tracking
    pub network_errors: u64,
    pub auth_errors: u64,
    pub timeout_errors: u64,
    pub other_errors: u64,
}

/// Object metadata for chunk storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkObjectMetadata {
    /// Chunk ID
    pub chunk_id: ChunkId,
    /// Object path in storage
    pub object_path: String,
    /// Object size in bytes
    pub size: u64,
    /// Upload timestamp
    pub uploaded_at: u64,
    /// ETag or version identifier
    pub etag: Option<String>,
    /// Checksum for integrity verification
    pub checksum: String,
    /// Encryption metadata
    pub encryption: Option<EncryptionMetadata>,
    /// Lifecycle metadata
    pub lifecycle: LifecycleMetadata,
}

/// Encryption metadata for objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    /// Encryption algorithm used
    pub algorithm: String,
    /// Key ID or reference
    pub key_id: String,
    /// Initialization vector
    pub iv: Option<String>,
}

/// Lifecycle metadata for objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleMetadata {
    /// Storage class
    pub storage_class: String,
    /// Transition rules applied
    pub transitions: Vec<String>,
    /// Expiration date
    pub expires_at: Option<u64>,
}

impl ObjectStorageBackend {
    /// Create a new object storage backend
    pub async fn new(
        provider: StorageProvider,
        config: ObjectStorageConfig,
    ) -> Result<Self> {
        let store = Self::create_store(&provider, &config).await?;
        
        let cache = if config.cache.enabled {
            Some(Arc::new(ObjectCache::new(config.cache.clone()).await?))
        } else {
            None
        };
        
        let operation_semaphore = Arc::new(Semaphore::new(10)); // Default 10 concurrent ops
        let metrics = Arc::new(RwLock::new(ObjectStorageMetrics::default()));
        
        Ok(Self {
            provider,
            store,
            config,
            cache,
            operation_semaphore,
            metrics,
        })
    }
    
    /// Create object store implementation based on provider
    async fn create_store(
        provider: &StorageProvider,
        config: &ObjectStorageConfig,
    ) -> Result<Arc<dyn ObjectStore>> {
        match provider {
            StorageProvider::S3 => {
                let mut builder = AmazonS3Builder::new()
                    .with_bucket_name(&config.bucket)
                    .with_region(&config.region);
                
                if !config.credentials.access_key_id.is_empty() {
                    builder = builder.with_access_key_id(&config.credentials.access_key_id);
                }
                
                if !config.credentials.secret_access_key.is_empty() {
                    builder = builder.with_secret_access_key(&config.credentials.secret_access_key);
                }
                
                if let Some(ref token) = config.credentials.session_token {
                    builder = builder.with_token(token);
                }
                
                let store = builder.build()?;
                Ok(Arc::new(store))
            }
            
            StorageProvider::Azure => {
                let mut builder = MicrosoftAzureBuilder::new()
                    .with_container_name(&config.bucket);
                
                if !config.credentials.access_key_id.is_empty() {
                    builder = builder.with_account(&config.credentials.access_key_id);
                }
                
                if !config.credentials.secret_access_key.is_empty() {
                    builder = builder.with_access_key(&config.credentials.secret_access_key);
                }
                
                let store = builder.build()?;
                Ok(Arc::new(store))
            }
            
            StorageProvider::GCS => {
                let mut builder = GoogleCloudStorageBuilder::new()
                    .with_bucket_name(&config.bucket);
                
                if !config.credentials.secret_access_key.is_empty() {
                    // Assume secret_access_key contains service account JSON
                    builder = builder.with_service_account_key(&config.credentials.secret_access_key);
                }
                
                let store = builder.build()?;
                Ok(Arc::new(store))
            }
            
            StorageProvider::Local => {
                let store = LocalFileSystem::new_with_prefix(&config.bucket)?;
                Ok(Arc::new(store))
            }
            
            StorageProvider::Memory => {
                let store = InMemory::new();
                Ok(Arc::new(store))
            }
        }
    }
    
    /// Upload chunk data to object storage
    #[instrument(skip(self, data))]
    pub async fn upload_chunk(
        &self,
        chunk_id: ChunkId,
        data: Bytes,
    ) -> Result<ChunkObjectMetadata> {
        let _permit = self.operation_semaphore.acquire().await?;
        let start_time = std::time::Instant::now();
        
        // Generate object path
        let object_path = self.generate_object_path(chunk_id);
        let path = ObjectPath::from(object_path.clone());
        
        // Calculate checksum
        let checksum = blake3::hash(&data).to_hex().to_string();
        
        info!("Uploading chunk {} to object storage (size: {} bytes)", chunk_id, data.len());
        
        // Perform upload
        let result = self.store.put(&path, data.clone()).await;
        
        let mut metrics = self.metrics.write().await;
        metrics.uploads_total += 1;
        
        match result {
            Ok(_) => {
                metrics.uploads_success += 1;
                metrics.upload_bytes_total += data.len() as u64;
                let duration_ms = start_time.elapsed().as_millis() as f64;
                metrics.avg_upload_duration_ms = 
                    (metrics.avg_upload_duration_ms * (metrics.uploads_success - 1) as f64 + duration_ms) 
                    / metrics.uploads_success as f64;
                
                let metadata = ChunkObjectMetadata {
                    chunk_id,
                    object_path,
                    size: data.len() as u64,
                    uploaded_at: now_micros() / 1_000_000,
                    etag: None, // object_store 0.5.4 doesn't return etag directly
                    checksum,
                    encryption: None, // TODO: Implement encryption
                    lifecycle: LifecycleMetadata {
                        storage_class: "STANDARD".to_string(),
                        transitions: Vec::new(),
                        expires_at: if self.config.lifecycle.delete_days > 0 {
                            Some(now_micros() / 1_000_000 + (self.config.lifecycle.delete_days as u64 * 86400))
                        } else {
                            None
                        },
                    },
                };
                
                info!("Successfully uploaded chunk {} in {:.2}ms", chunk_id, duration_ms);
                Ok(metadata)
            }
            
            Err(e) => {
                metrics.uploads_failed += 1;
                self.classify_error(&e, &mut metrics).await;
                error!("Failed to upload chunk {}: {}", chunk_id, e);
                Err(anyhow!("Upload failed: {}", e))
            }
        }
    }
    
    /// Download chunk data from object storage
    #[instrument(skip(self))]
    pub async fn download_chunk(
        &self,
        metadata: &ChunkObjectMetadata,
    ) -> Result<Bytes> {
        let chunk_id = metadata.chunk_id;
        
        // Check cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached_data) = cache.get(&metadata.object_path).await? {
                debug!("Cache hit for chunk {}", chunk_id);
                let mut metrics = self.metrics.write().await;
                metrics.cache_hits += 1;
                return Ok(cached_data);
            } else {
                let mut metrics = self.metrics.write().await;
                metrics.cache_misses += 1;
            }
        }
        
        let _permit = self.operation_semaphore.acquire().await?;
        let start_time = std::time::Instant::now();
        
        info!("Downloading chunk {} from object storage", chunk_id);
        
        let path = ObjectPath::from(metadata.object_path.clone());
        let result = self.store.get(&path).await;
        
        let mut metrics = self.metrics.write().await;
        metrics.downloads_total += 1;
        
        match result {
            Ok(get_result) => {
                let data = get_result.bytes().await?;
                
                // Verify checksum
                let computed_checksum = blake3::hash(&data).to_hex().to_string();
                if computed_checksum != metadata.checksum {
                    metrics.downloads_failed += 1;
                    return Err(anyhow!("Checksum mismatch for chunk {}", chunk_id));
                }
                
                metrics.downloads_success += 1;
                metrics.download_bytes_total += data.len() as u64;
                let duration_ms = start_time.elapsed().as_millis() as f64;
                metrics.avg_download_duration_ms = 
                    (metrics.avg_download_duration_ms * (metrics.downloads_success - 1) as f64 + duration_ms) 
                    / metrics.downloads_success as f64;
                
                info!("Successfully downloaded chunk {} in {:.2}ms", chunk_id, duration_ms);
                
                // Store in cache if enabled
                if let Some(ref cache) = self.cache {
                    if let Err(e) = cache.put(&metadata.object_path, data.clone()).await {
                        warn!("Failed to cache chunk {}: {}", chunk_id, e);
                    }
                }
                
                Ok(data)
            }
            
            Err(e) => {
                metrics.downloads_failed += 1;
                self.classify_error(&e, &mut metrics).await;
                error!("Failed to download chunk {}: {}", chunk_id, e);
                Err(anyhow!("Download failed: {}", e))
            }
        }
    }
    
    /// Delete chunk from object storage
    #[instrument(skip(self))]
    pub async fn delete_chunk(&self, metadata: &ChunkObjectMetadata) -> Result<()> {
        let _permit = self.operation_semaphore.acquire().await?;
        let chunk_id = metadata.chunk_id;
        
        info!("Deleting chunk {} from object storage", chunk_id);
        
        let path = ObjectPath::from(metadata.object_path.clone());
        self.store.delete(&path).await
            .context(format!("Failed to delete chunk {}", chunk_id))?;
        
        // Remove from cache if present
        if let Some(ref cache) = self.cache {
            cache.remove(&metadata.object_path).await?;
        }
        
        info!("Successfully deleted chunk {}", chunk_id);
        Ok(())
    }
    
    /// List objects with pagination
    pub async fn list_objects(&self, prefix: Option<&str>) -> Result<Vec<ObjectMeta>> {
        use futures::stream::StreamExt;
        
        let list_stream = if let Some(prefix) = prefix {
            let path = ObjectPath::from(prefix);
            self.store.list(Some(&path))
        } else {
            self.store.list(None)
        };
        
        let mut objects = Vec::new();
        
        futures::pin_mut!(list_stream);
        while let Some(result) = list_stream.next().await {
            match result {
                Ok(object_meta) => objects.push(object_meta),
                Err(e) => {
                    error!("Error listing objects: {}", e);
                    return Err(anyhow!("List failed: {}", e));
                }
            }
        }
        
        Ok(objects)
    }
    
    /// Get storage metrics
    pub async fn get_metrics(&self) -> ObjectStorageMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Generate object path for chunk
    fn generate_object_path(&self, chunk_id: ChunkId) -> String {
        // Use hierarchical structure for better performance
        // e.g., chunks/00/01/000100000001.chunk
        format!("chunks/{:02x}/{:02x}/{:012x}.chunk", 
                (chunk_id >> 40) & 0xFF,
                (chunk_id >> 32) & 0xFF,
                chunk_id)
    }
    
    /// Classify error for metrics
    async fn classify_error(&self, error: &object_store::Error, metrics: &mut ObjectStorageMetrics) {
        match error {
            object_store::Error::NotFound { .. } => {
                // Not counted as error for downloads
            }
            object_store::Error::UnknownConfigurationKey { .. } => {
                metrics.auth_errors += 1;
            }
            _ => {
                // Check if it's likely a network error
                let error_str = error.to_string().to_lowercase();
                if error_str.contains("timeout") || error_str.contains("connection") {
                    metrics.timeout_errors += 1;
                } else if error_str.contains("network") || error_str.contains("dns") {
                    metrics.network_errors += 1;
                } else if error_str.contains("auth") || error_str.contains("credential") || error_str.contains("permission") {
                    metrics.auth_errors += 1;
                } else {
                    metrics.other_errors += 1;
                }
            }
        }
    }
    
    /// Health check for object storage backend
    pub async fn health_check(&self) -> Result<()> {
        // Try to list a small number of objects to verify connectivity
        let _objects = self.list_objects(Some("health")).await?;
        Ok(())
    }
}

impl ObjectCache {
    /// Create a new object cache
    pub async fn new(config: CacheConfig) -> Result<Self> {
        tokio::fs::create_dir_all(&config.cache_dir).await
            .context("Failed to create cache directory")?;
        
        Ok(Self {
            cache_dir: config.cache_dir.clone(),
            max_size: config.max_size,
            current_size: Arc::new(RwLock::new(0)),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }
    
    /// Get data from cache
    pub async fn get(&self, object_path: &str) -> Result<Option<Bytes>> {
        let mut metadata = self.metadata.write().await;
        
        if let Some(entry) = metadata.get_mut(object_path) {
            // Check if entry is still valid
            if entry.created_at.elapsed().unwrap_or_default() > Duration::from_secs(self.config.ttl_seconds) {
                // Entry expired, remove it
                let _ = tokio::fs::remove_file(&entry.path).await;
                let mut current_size = self.current_size.write().await;
                *current_size = current_size.saturating_sub(entry.size);
                metadata.remove(object_path);
                return Ok(None);
            }
            
            // Update access time and count
            entry.last_accessed = SystemTime::now();
            entry.access_count += 1;
            
            // Read cached data
            match tokio::fs::read(&entry.path).await {
                Ok(data) => Ok(Some(Bytes::from(data))),
                Err(_) => {
                    // File doesn't exist, remove from metadata
                    let mut current_size = self.current_size.write().await;
                    *current_size = current_size.saturating_sub(entry.size);
                    metadata.remove(object_path);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
    
    /// Store data in cache
    pub async fn put(&self, object_path: &str, data: Bytes) -> Result<()> {
        let size = data.len() as u64;
        
        // Check if we need to make space
        self.ensure_capacity(size).await?;
        
        // Generate cache file path
        let cache_file = self.cache_dir.join(format!("{}.cache", blake3::hash(object_path.as_bytes()).to_hex()));
        
        // Write data to cache file
        tokio::fs::write(&cache_file, &data).await
            .context("Failed to write cache file")?;
        
        // Update metadata
        let entry = CacheEntry {
            path: cache_file,
            size,
            last_accessed: SystemTime::now(),
            created_at: SystemTime::now(),
            access_count: 1,
            etag: None,
        };
        
        let mut metadata = self.metadata.write().await;
        metadata.insert(object_path.to_string(), entry);
        
        let mut current_size = self.current_size.write().await;
        *current_size += size;
        
        Ok(())
    }
    
    /// Remove object from cache
    pub async fn remove(&self, object_path: &str) -> Result<()> {
        let mut metadata = self.metadata.write().await;
        
        if let Some(entry) = metadata.remove(object_path) {
            let _ = tokio::fs::remove_file(&entry.path).await;
            let mut current_size = self.current_size.write().await;
            *current_size = current_size.saturating_sub(entry.size);
        }
        
        Ok(())
    }
    
    /// Ensure we have capacity for new data
    async fn ensure_capacity(&self, needed: u64) -> Result<()> {
        let current_size = *self.current_size.read().await;
        
        if current_size + needed <= self.max_size {
            return Ok(());
        }
        
        // Need to evict some entries
        let mut metadata = self.metadata.write().await;
        let mut entries: Vec<_> = metadata.iter().collect();
        
        // Sort by last access time (least recently used first)
        entries.sort_by_key(|(_, entry)| entry.last_accessed);
        
        let mut freed = 0u64;
        let mut to_remove = Vec::new();
        
        for (path, entry) in entries {
            if freed >= needed {
                break;
            }
            
            let _ = tokio::fs::remove_file(&entry.path).await;
            freed += entry.size;
            to_remove.push(path.clone());
        }
        
        // Remove from metadata
        for path in to_remove {
            metadata.remove(&path);
        }
        
        let mut current_size = self.current_size.write().await;
        *current_size = current_size.saturating_sub(freed);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_local_object_storage() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = ObjectStorageConfig {
            provider: "local".to_string(),
            bucket: temp_dir.path().to_string_lossy().to_string(),
            region: "local".to_string(),
            credentials: CredentialsConfig {
                access_key_id: "".to_string(),
                secret_access_key: "".to_string(),
                session_token: None,
                role_arn: None,
            },
            lifecycle: LifecycleConfig {
                transition_days: 30,
                delete_days: 0,
                versioning: false,
            },
            cache: CacheConfig {
                enabled: false,
                max_size: 0,
                ttl_seconds: 0,
                cache_dir: PathBuf::new(),
            },
        };
        
        let backend = ObjectStorageBackend::new(StorageProvider::Local, config).await.unwrap();
        
        // Test upload
        let chunk_data = Bytes::from(vec![1, 2, 3, 4, 5]);
        let metadata = backend.upload_chunk(123, chunk_data.clone()).await.unwrap();
        
        assert_eq!(metadata.chunk_id, 123);
        assert_eq!(metadata.size, 5);
        
        // Test download
        let downloaded = backend.download_chunk(&metadata).await.unwrap();
        assert_eq!(downloaded, chunk_data);
        
        // Test delete
        backend.delete_chunk(&metadata).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_object_cache() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = CacheConfig {
            enabled: true,
            max_size: 1024,
            ttl_seconds: 3600,
            cache_dir: temp_dir.path().to_path_buf(),
        };
        
        let cache = ObjectCache::new(config).await.unwrap();
        
        // Test put and get
        let data = Bytes::from(vec![1, 2, 3, 4]);
        cache.put("test/object", data.clone()).await.unwrap();
        
        let cached = cache.get("test/object").await.unwrap();
        assert_eq!(cached, Some(data));
        
        // Test miss
        let missing = cache.get("missing/object").await.unwrap();
        assert_eq!(missing, None);
    }
    
    #[tokio::test]
    async fn test_memory_object_storage() {
        let config = ObjectStorageConfig {
            provider: "memory".to_string(),
            bucket: "test-bucket".to_string(),
            region: "memory".to_string(),
            credentials: CredentialsConfig {
                access_key_id: "".to_string(),
                secret_access_key: "".to_string(),
                session_token: None,
                role_arn: None,
            },
            lifecycle: LifecycleConfig {
                transition_days: 30,
                delete_days: 0,
                versioning: false,
            },
            cache: CacheConfig {
                enabled: false,
                max_size: 0,
                ttl_seconds: 0,
                cache_dir: PathBuf::new(),
            },
        };
        
        let backend = ObjectStorageBackend::new(StorageProvider::Memory, config).await.unwrap();
        
        // Test multiple operations
        let chunks = vec![
            (1, Bytes::from(b"chunk1".to_vec())),
            (2, Bytes::from(b"chunk2".to_vec())),
            (3, Bytes::from(b"chunk3".to_vec())),
        ];
        
        let mut metadata_list = Vec::new();
        
        // Upload all chunks
        for (chunk_id, data) in chunks.iter() {
            let metadata = backend.upload_chunk(*chunk_id, data.clone()).await.unwrap();
            metadata_list.push(metadata);
        }
        
        // Download and verify all chunks
        for (i, metadata) in metadata_list.iter().enumerate() {
            let downloaded = backend.download_chunk(metadata).await.unwrap();
            assert_eq!(downloaded, chunks[i].1);
        }
        
        // Test metrics
        let metrics = backend.get_metrics().await;
        assert_eq!(metrics.uploads_success, 3);
        assert_eq!(metrics.downloads_success, 3);
    }
}