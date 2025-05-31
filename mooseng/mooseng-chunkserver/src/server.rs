//! Main Chunk Server implementation
//! 
//! This module provides the main ChunkServer struct that coordinates
//! all chunk operations, including storage, caching, and network communication.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::cache::{ChunkCache, CacheConfig};
use crate::chunk::{Chunk, ChunkMetrics, ChecksumType};
use crate::config::ChunkServerConfig;
use crate::error::{ChunkServerError, Result};
use crate::storage::{ChunkStorage, FileStorage, StorageStats};
use mooseng_common::types::{ChunkId, ChunkVersion, ChunkServerId, now_micros};
use bytes::Bytes;

/// Main Chunk Server implementation
pub struct ChunkServer {
    /// Server configuration
    config: Arc<ChunkServerConfig>,
    
    /// Primary storage backend
    storage: Arc<dyn ChunkStorage>,
    
    /// In-memory cache for hot chunks
    cache: Arc<ChunkCache>,
    
    /// Semaphore to limit concurrent operations
    operation_semaphore: Arc<Semaphore>,
    
    /// Metrics collection
    metrics: Arc<RwLock<ChunkMetrics>>,
    
    /// Server state
    state: Arc<RwLock<ServerState>>,
}

/// Server state information
#[derive(Debug, Clone)]
struct ServerState {
    /// Server start time
    start_time: Instant,
    
    /// Whether server is running
    is_running: bool,
    
    /// Active chunk operations
    active_operations: HashMap<(ChunkId, ChunkVersion), String>,
    
    /// Last heartbeat to master
    last_heartbeat: Option<Instant>,
    
    /// Connection status to master
    master_connected: bool,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            is_running: false,
            active_operations: HashMap::new(),
            last_heartbeat: None,
            master_connected: false,
        }
    }
}

/// Operation context for tracking and limiting concurrent operations
#[derive(Debug)]
pub struct OperationContext {
    chunk_id: ChunkId,
    version: ChunkVersion,
    operation: String,
    start_time: Instant,
}

impl OperationContext {
    fn new(chunk_id: ChunkId, version: ChunkVersion, operation: &str) -> Self {
        Self {
            chunk_id,
            version,
            operation: operation.to_string(),
            start_time: Instant::now(),
        }
    }
    
    fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl ChunkServer {
    /// Create a new chunk server instance
    pub async fn new(config: ChunkServerConfig) -> Result<Self> {
        let config = Arc::new(config);
        
        info!("Initializing chunk server with config: {:?}", config);
        
        // Initialize storage
        let storage = Arc::new(FileStorage::new(config.clone()).await?);
        
        // Initialize cache
        let cache_config = CacheConfig {
            max_entries: (config.cache_size_bytes / 65536).max(100) as usize, // Estimate based on 64KB avg chunk size
            max_memory_bytes: config.cache_size_bytes,
            max_age_seconds: 3600, // 1 hour
            enable_metrics: config.enable_metrics,
        };
        let cache = Arc::new(ChunkCache::new(cache_config)?);
        
        // Initialize semaphore for operation limiting
        let operation_semaphore = Arc::new(Semaphore::new(config.max_concurrent_ops));
        
        info!("Chunk server initialized successfully");
        
        Ok(Self {
            config,
            storage,
            cache,
            operation_semaphore,
            metrics: Arc::new(RwLock::new(ChunkMetrics::new())),
            state: Arc::new(RwLock::new(ServerState::default())),
        })
    }
    
    /// Start the chunk server
    pub async fn start(&self) -> Result<()> {
        info!("Starting chunk server on {}:{}", self.config.bind_address, self.config.port);
        
        // Update server state
        {
            let mut state = self.state.write().await;
            state.is_running = true;
        }
        
        // Start background tasks
        self.start_background_tasks().await;
        
        info!("Chunk server started successfully");
        Ok(())
    }
    
    /// Stop the chunk server
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping chunk server");
        
        // Update server state
        {
            let mut state = self.state.write().await;
            state.is_running = false;
        }
        
        // Clear cache
        self.cache.clear().await;
        
        info!("Chunk server stopped");
        Ok(())
    }
    
    /// Store a chunk
    pub async fn store_chunk(
        &self,
        chunk_id: ChunkId,
        version: ChunkVersion,
        data: Bytes,
        checksum_type: ChecksumType,
        storage_class_id: u8,
    ) -> Result<()> {
        let _permit = self.operation_semaphore.acquire().await
            .map_err(|_| ChunkServerError::Internal("Failed to acquire operation permit".to_string()))?;
        
        let ctx = OperationContext::new(chunk_id, version, "store");
        
        // Track operation
        {
            let mut state = self.state.write().await;
            state.active_operations.insert((chunk_id, version), ctx.operation.clone());
        }
        
        let start_time = Instant::now();
        
        let result = async {
            debug!("Storing chunk {} v{} ({} bytes)", chunk_id, version, data.len());
            
            // Create chunk
            let chunk = Chunk::new(chunk_id, version, data, checksum_type, storage_class_id);
            
            // Store to persistent storage
            self.storage.store_chunk(&chunk).await?;
            
            // Cache the chunk if it's small enough
            if chunk.size() <= (self.config.cache_size_bytes / 100) {
                if let Err(e) = self.cache.put(chunk).await {
                    warn!("Failed to cache chunk {} v{}: {}", chunk_id, version, e);
                }
            }
            
            Ok(())
        }.await;
        
        let elapsed = start_time.elapsed();
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            if result.is_ok() {
                metrics.record_write(elapsed.as_micros() as u64);
            }
        }
        
        // Remove from active operations
        {
            let mut state = self.state.write().await;
            state.active_operations.remove(&(chunk_id, version));
        }
        
        if let Err(ref e) = result {
            error!("Failed to store chunk {} v{}: {}", chunk_id, version, e);
        } else {
            debug!("Successfully stored chunk {} v{} in {:?}", chunk_id, version, elapsed);
        }
        
        result
    }
    
    /// Retrieve a chunk
    pub async fn get_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<Chunk> {
        let _permit = self.operation_semaphore.acquire().await
            .map_err(|_| ChunkServerError::Internal("Failed to acquire operation permit".to_string()))?;
        
        let ctx = OperationContext::new(chunk_id, version, "get");
        
        // Track operation
        {
            let mut state = self.state.write().await;
            state.active_operations.insert((chunk_id, version), ctx.operation.clone());
        }
        
        let start_time = Instant::now();
        
        let result = async {
            debug!("Retrieving chunk {} v{}", chunk_id, version);
            
            // Try cache first
            if let Some(chunk) = self.cache.get(chunk_id, version).await {
                debug!("Cache hit for chunk {} v{}", chunk_id, version);
                return Ok(chunk);
            }
            
            // Cache miss, load from storage
            debug!("Cache miss for chunk {} v{}, loading from storage", chunk_id, version);
            let chunk = self.storage.get_chunk(chunk_id, version).await?;
            
            // Cache the chunk for future access
            if let Err(e) = self.cache.put(chunk.clone()).await {
                warn!("Failed to cache chunk {} v{}: {}", chunk_id, version, e);
            }
            
            Ok(chunk)
        }.await;
        
        let elapsed = start_time.elapsed();
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            if result.is_ok() {
                metrics.record_read(elapsed.as_micros() as u64);
            }
        }
        
        // Remove from active operations
        {
            let mut state = self.state.write().await;
            state.active_operations.remove(&(chunk_id, version));
        }
        
        if let Err(ref e) = result {
            error!("Failed to retrieve chunk {} v{}: {}", chunk_id, version, e);
        } else {
            debug!("Successfully retrieved chunk {} v{} in {:?}", chunk_id, version, elapsed);
        }
        
        result
    }
    
    /// Delete a chunk
    pub async fn delete_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<()> {
        let _permit = self.operation_semaphore.acquire().await
            .map_err(|_| ChunkServerError::Internal("Failed to acquire operation permit".to_string()))?;
        
        let ctx = OperationContext::new(chunk_id, version, "delete");
        
        // Track operation
        {
            let mut state = self.state.write().await;
            state.active_operations.insert((chunk_id, version), ctx.operation.clone());
        }
        
        let result = async {
            debug!("Deleting chunk {} v{}", chunk_id, version);
            
            // Remove from cache first
            self.cache.remove(chunk_id, version).await;
            
            // Delete from storage
            self.storage.delete_chunk(chunk_id, version).await?;
            
            Ok(())
        }.await;
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            if result.is_ok() {
                metrics.record_delete();
            }
        }
        
        // Remove from active operations
        {
            let mut state = self.state.write().await;
            state.active_operations.remove(&(chunk_id, version));
        }
        
        if let Err(ref e) = result {
            error!("Failed to delete chunk {} v{}: {}", chunk_id, version, e);
        } else {
            debug!("Successfully deleted chunk {} v{}", chunk_id, version);
        }
        
        result
    }
    
    /// Check if a chunk exists
    pub async fn chunk_exists(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool> {
        // Check cache first
        if self.cache.contains(chunk_id, version).await {
            return Ok(true);
        }
        
        // Check storage
        self.storage.chunk_exists(chunk_id, version).await
    }
    
    /// List all chunks
    pub async fn list_chunks(&self) -> Result<Vec<(ChunkId, ChunkVersion)>> {
        self.storage.list_chunks().await
    }
    
    /// Get server statistics
    pub async fn get_stats(&self) -> Result<ServerStats> {
        let storage_stats = self.storage.get_stats().await?;
        let cache_stats = self.cache.get_stats().await;
        let metrics = self.metrics.read().await.clone();
        let state = self.state.read().await;
        
        Ok(ServerStats {
            uptime_seconds: state.start_time.elapsed().as_secs(),
            is_running: state.is_running,
            active_operations: state.active_operations.len() as u64,
            master_connected: state.master_connected,
            storage: storage_stats,
            cache_hit_ratio: cache_stats.hit_ratio(),
            cache_memory_usage_mb: cache_stats.memory_usage_mb(),
            metrics,
        })
    }
    
    /// Verify chunk integrity
    pub async fn verify_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool> {
        match self.get_chunk(chunk_id, version).await {
            Ok(chunk) => {
                let is_valid = chunk.verify_integrity();
                
                if !is_valid {
                    let mut metrics = self.metrics.write().await;
                    metrics.record_checksum_failure();
                    warn!("Chunk {} v{} failed integrity verification", chunk_id, version);
                }
                
                Ok(is_valid)
            }
            Err(e) => {
                warn!("Failed to verify chunk {} v{}: {}", chunk_id, version, e);
                Err(e)
            }
        }
    }
    
    /// Get server configuration
    pub fn config(&self) -> &ChunkServerConfig {
        &self.config
    }
    
    /// Get server ID
    pub fn server_id(&self) -> ChunkServerId {
        self.config.server_id
    }
    
    /// Start background tasks
    async fn start_background_tasks(&self) {
        let cache = self.cache.clone();
        let config = self.config.clone();
        let state = self.state.clone();
        
        // Cache maintenance task
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                let is_running = {
                    let state = state.read().await;
                    state.is_running
                };
                
                if !is_running {
                    break;
                }
                
                cache.maintenance().await;
                
                if cache.check_memory_pressure().await {
                    cache.evict_for_memory_pressure().await;
                }
            }
        });
        
        // TODO: Add heartbeat task to master server
        // TODO: Add chunk verification background task
        // TODO: Add metrics collection task
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub uptime_seconds: u64,
    pub is_running: bool,
    pub active_operations: u64,
    pub master_connected: bool,
    pub storage: StorageStats,
    pub cache_hit_ratio: f64,
    pub cache_memory_usage_mb: f64,
    pub metrics: ChunkMetrics,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::chunk::ChecksumType;
    
    async fn create_test_server() -> (ChunkServer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ChunkServerConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        config.cache_size_bytes = 1024 * 1024; // 1MB cache
        config.max_concurrent_ops = 10;
        
        let server = ChunkServer::new(config).await.unwrap();
        (server, temp_dir)
    }
    
    #[tokio::test]
    async fn test_server_creation() {
        let (server, _temp_dir) = create_test_server().await;
        assert_eq!(server.server_id(), 1);
        assert!(!server.config().enable_mmap); // Default should be disabled in tests
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve_chunk() {
        let (server, _temp_dir) = create_test_server().await;
        
        let chunk_id = 12345;
        let version = 1;
        let data = Bytes::from("Hello, World!");
        
        // Store chunk
        server.store_chunk(chunk_id, version, data.clone(), ChecksumType::Blake3, 1).await.unwrap();
        
        // Verify it exists
        assert!(server.chunk_exists(chunk_id, version).await.unwrap());
        
        // Retrieve chunk
        let chunk = server.get_chunk(chunk_id, version).await.unwrap();
        assert_eq!(chunk.id(), chunk_id);
        assert_eq!(chunk.version(), version);
        assert_eq!(chunk.data(), &data);
    }
    
    #[tokio::test]
    async fn test_delete_chunk() {
        let (server, _temp_dir) = create_test_server().await;
        
        let chunk_id = 12345;
        let version = 1;
        let data = Bytes::from("test data");
        
        // Store and delete
        server.store_chunk(chunk_id, version, data, ChecksumType::Blake3, 1).await.unwrap();
        assert!(server.chunk_exists(chunk_id, version).await.unwrap());
        
        server.delete_chunk(chunk_id, version).await.unwrap();
        assert!(!server.chunk_exists(chunk_id, version).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_list_chunks() {
        let (server, _temp_dir) = create_test_server().await;
        
        // Store multiple chunks
        for i in 1..=5 {
            let data = Bytes::from(format!("data{}", i));
            server.store_chunk(i, 1, data, ChecksumType::Blake3, 1).await.unwrap();
        }
        
        let chunks = server.list_chunks().await.unwrap();
        assert_eq!(chunks.len(), 5);
    }
    
    #[tokio::test]
    async fn test_server_stats() {
        let (server, _temp_dir) = create_test_server().await;
        
        let stats = server.get_stats().await.unwrap();
        assert!(!stats.is_running); // Server not started
        assert_eq!(stats.active_operations, 0);
        assert_eq!(stats.storage.total_chunks, 0);
    }
    
    #[tokio::test]
    async fn test_concurrent_operations() {
        let (server, _temp_dir) = create_test_server().await;
        
        let server = Arc::new(server);
        let mut handles = Vec::new();
        
        // Start multiple concurrent operations
        for i in 1..=5 {
            let server_clone = server.clone();
            let handle = tokio::spawn(async move {
                let data = Bytes::from(format!("data{}", i));
                server_clone.store_chunk(i, 1, data, ChecksumType::Blake3, 1).await
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        // All chunks should be stored
        let chunks = server.list_chunks().await.unwrap();
        assert_eq!(chunks.len(), 5);
    }
    
    #[tokio::test]
    async fn test_chunk_verification() {
        let (server, _temp_dir) = create_test_server().await;
        
        let chunk_id = 12345;
        let version = 1;
        let data = Bytes::from("Hello, World!");
        
        server.store_chunk(chunk_id, version, data, ChecksumType::Blake3, 1).await.unwrap();
        
        // Verification should pass for valid chunk
        assert!(server.verify_chunk(chunk_id, version).await.unwrap());
    }
}