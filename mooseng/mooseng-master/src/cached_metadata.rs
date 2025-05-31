use anyhow::{Context, Result};
use mooseng_common::types::{
    ChunkId, ChunkMetadata, FsEdge, FsNode, InodeId, StorageClassDef, SystemConfig,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::cache_configuration::CacheConfig;
use crate::cache_enhanced::{
    CacheInvalidation, CacheWarmer, EnhancedMetadataCache, PrefetchHint,
};
use crate::metadata::MetadataStore;

/// Cached metadata manager that wraps the underlying MetadataStore with intelligent caching
pub struct CachedMetadataManager {
    /// Underlying storage layer
    storage: Arc<MetadataStore>,
    /// Enhanced metadata cache
    cache: Arc<EnhancedMetadataCache>,
    /// Cache configuration
    config: CacheConfig,
    /// Prefetch channel sender
    prefetch_tx: Option<mpsc::Sender<PrefetchHint>>,
}

impl CachedMetadataManager {
    /// Create a new cached metadata manager
    pub async fn new(
        storage: Arc<MetadataStore>,
        config: CacheConfig,
    ) -> Result<Self> {
        // Validate configuration
        config.validate().map_err(|e| anyhow::anyhow!("Invalid cache configuration: {}", e))?;

        // Create enhanced cache
        let cache = Arc::new(EnhancedMetadataCache::new(
            config.ttl(),
            config.max_size,
            config.hot_threshold,
        ));

        // Set up cache warmer if prefetching is enabled
        let prefetch_tx = if config.enable_prefetch {
            let (tx, rx) = mpsc::channel(100);
            
            // Create fetch function that uses the storage layer
            let storage_clone = storage.clone();
            let fetch_fn = move |hint: PrefetchHint| -> Vec<FsNode> {
                let mut nodes = Vec::new();
                
                // Fetch requested inodes
                for inode in hint.inodes {
                    if let Ok(Some(node)) = storage_clone.get_inode(inode) {
                        nodes.push(node);
                    }
                }
                
                // Fetch inodes for requested paths (simplified)
                for path in hint.paths {
                    if let Some(inode) = resolve_path_to_inode(&storage_clone, &path) {
                        if let Ok(Some(node)) = storage_clone.get_inode(inode) {
                            nodes.push(node);
                        }
                    }
                }
                
                nodes
            };
            
            let warmer = CacheWarmer::new(cache.clone(), fetch_fn);
            warmer.start(rx).await;
            
            Some(tx)
        } else {
            None
        };

        // Start background maintenance if enabled
        if config.enable_lru {
            cache.clone().start_maintenance();
        }

        let manager = Self {
            storage,
            cache,
            config,
            prefetch_tx,
        };

        info!("CachedMetadataManager initialized with cache size: {}, TTL: {}s", 
              manager.config.max_size, manager.config.ttl_secs);

        Ok(manager)
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> std::collections::HashMap<String, u64> {
        self.cache.stats().get_stats()
    }

    /// Get cache hit rate
    pub fn cache_hit_rate(&self) -> f64 {
        self.cache.stats().hit_rate()
    }

    /// Clear the entire cache
    pub fn clear_cache(&self) {
        self.cache.clear();
        info!("Cache cleared");
    }

    /// Subscribe to cache invalidation events
    pub fn subscribe_invalidations(&self) -> tokio::sync::broadcast::Receiver<CacheInvalidation> {
        self.cache.subscribe_invalidations()
    }

    // Inode operations with caching

    /// Get an FsNode by inode ID (cached)
    pub async fn get_inode(&self, inode: InodeId) -> Result<Option<FsNode>> {
        // Try cache first
        if let Some(node) = self.cache.get_inode(inode) {
            debug!("Cache hit for inode {}", inode);
            return Ok(Some(node));
        }

        // Cache miss - fetch from storage
        debug!("Cache miss for inode {}, fetching from storage", inode);
        let node = self.storage.get_inode(inode)?;

        // Cache the result if found
        if let Some(ref node) = node {
            self.cache.insert_inode(node.clone());
        }

        Ok(node)
    }

    /// Save an FsNode (with cache update and invalidation)
    pub async fn save_inode(&self, node: &FsNode) -> Result<()> {
        // Save to storage first
        self.storage.save_inode(node)?;

        // Update cache
        self.cache.insert_inode(node.clone());

        // Invalidate across cluster if enabled
        if self.config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidateInode(node.inode)).await;
        }

        debug!("Saved and cached inode {}", node.inode);
        Ok(())
    }

    /// Delete an FsNode (with cache invalidation)
    pub async fn delete_inode(&self, inode: InodeId) -> Result<()> {
        // Delete from storage
        self.storage.delete_inode(inode)?;

        // Invalidate cache
        if self.config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidateInode(inode)).await;
        }

        debug!("Deleted inode {} and invalidated cache", inode);
        Ok(())
    }

    /// List all inodes (bypasses cache due to size)
    pub async fn list_inodes(&self) -> Result<Vec<FsNode>> {
        // For large operations like listing all inodes, bypass cache
        self.storage.list_inodes()
    }

    // Edge operations

    /// Find child inode by parent and name (with caching via path resolution)
    pub async fn find_child(&self, parent: InodeId, name: &str) -> Result<Option<InodeId>> {
        // For now, delegate to storage - could be enhanced with path caching
        self.storage.find_child(parent, name)
    }

    /// Save an FsEdge (with path invalidation)
    pub async fn save_edge(&self, edge: &FsEdge) -> Result<()> {
        // Save to storage
        self.storage.save_edge(edge)?;

        // Invalidate parent directory cache entry
        if self.config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidateInode(edge.parent_inode)).await;
        }

        debug!("Saved edge {} and invalidated parent cache", edge.edge_id);
        Ok(())
    }

    /// Delete an edge (with path invalidation)
    pub async fn delete_edge(&self, edge: &FsEdge) -> Result<()> {
        // Delete from storage
        self.storage.delete_edge(edge)?;

        // Invalidate parent directory cache entry
        if self.config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidateInode(edge.parent_inode)).await;
        }

        debug!("Deleted edge {} and invalidated parent cache", edge.edge_id);
        Ok(())
    }

    /// List children of a directory
    pub async fn list_children(&self, parent: InodeId) -> Result<Vec<FsEdge>> {
        // Could be enhanced with edge caching in the future
        self.storage.list_children(parent)
    }

    // Chunk operations (with chunk-specific caching)

    /// Get chunk metadata (storage-only for now)
    pub async fn get_chunk(&self, chunk_id: ChunkId) -> Result<Option<ChunkMetadata>> {
        // Chunk metadata could be cached separately in the future
        self.storage.get_chunk(chunk_id)
    }

    /// Save chunk metadata
    pub async fn save_chunk(&self, chunk: &ChunkMetadata) -> Result<()> {
        self.storage.save_chunk(chunk)
    }

    /// Delete chunk metadata
    pub async fn delete_chunk(&self, chunk_id: ChunkId) -> Result<()> {
        self.storage.delete_chunk(chunk_id)
    }

    /// List all chunks
    pub async fn list_chunks(&self) -> Result<Vec<ChunkMetadata>> {
        self.storage.list_chunks()
    }

    // Storage class operations (with storage class caching)

    /// Get a storage class definition (storage-only for now)
    pub async fn get_storage_class(&self, id: u8) -> Result<Option<StorageClassDef>> {
        // Storage classes could benefit from caching due to frequent access
        self.storage.get_storage_class(id)
    }

    /// Save a storage class definition
    pub async fn save_storage_class(&self, sc: &StorageClassDef) -> Result<()> {
        // Save to storage
        self.storage.save_storage_class(sc)?;

        // Invalidate storage class cache if enabled
        if self.config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidateStorageClass(sc.id)).await;
        }

        Ok(())
    }

    /// List all storage classes
    pub async fn list_storage_classes(&self) -> Result<Vec<StorageClassDef>> {
        self.storage.list_storage_classes()
    }

    // System configuration

    /// Get system configuration
    pub async fn get_system_config(&self) -> Result<SystemConfig> {
        self.storage.get_system_config()
    }

    /// Save system configuration
    pub async fn save_system_config(&self, config: &SystemConfig) -> Result<()> {
        self.storage.save_system_config(config)
    }

    // High-level operations

    /// Prefetch commonly accessed inodes
    pub async fn prefetch_hot_inodes(&self, inodes: Vec<InodeId>) -> Result<()> {
        if let Some(ref tx) = self.prefetch_tx {
            let hint = PrefetchHint {
                inodes,
                chunks: vec![],
                paths: vec![],
            };
            
            tx.send(hint).await.context("Failed to send prefetch hint")?;
            debug!("Requested prefetch for hot inodes");
        }
        Ok(())
    }

    /// Prefetch directory tree for a given path
    pub async fn prefetch_directory_tree(&self, path: String) -> Result<()> {
        if let Some(ref tx) = self.prefetch_tx {
            let hint = PrefetchHint {
                inodes: vec![],
                chunks: vec![],
                paths: vec![path],
            };
            
            tx.send(hint).await.context("Failed to send prefetch hint")?;
            debug!("Requested prefetch for directory tree");
        }
        Ok(())
    }

    /// Invalidate cache for a specific path
    pub async fn invalidate_path(&self, path: String) {
        if self.config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidatePath(path)).await;
        }
    }

    /// Invalidate entire cache
    pub async fn invalidate_all(&self) {
        if self.config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidateAll).await;
        }
    }

    // Delegation methods to storage

    /// Get the next available inode ID
    pub fn next_inode_id(&self) -> Result<InodeId> {
        self.storage.next_inode_id()
    }

    /// Get the next available edge ID
    pub fn next_edge_id(&self) -> Result<u64> {
        self.storage.next_edge_id()
    }

    /// Get the next available chunk ID
    pub fn next_chunk_id(&self) -> Result<ChunkId> {
        self.storage.next_chunk_id()
    }

    /// Get an FsEdge by edge ID
    pub async fn get_edge(&self, edge_id: u64) -> Result<Option<FsEdge>> {
        self.storage.get_edge(edge_id)
    }

    /// Flush all pending writes to disk
    pub async fn flush(&self) -> Result<()> {
        self.storage.flush().await
    }

    /// Get database size
    pub fn size(&self) -> Result<u64> {
        self.storage.size()
    }

    /// Verify metadata checksum
    pub async fn verify_checksum(&self) -> Result<()> {
        self.storage.verify_checksum().await
    }

    /// Save metadata to disk
    pub async fn save_metadata(&self) -> Result<()> {
        self.storage.save_metadata().await
    }
}

/// Helper function to resolve a path to an inode (simplified implementation)
fn resolve_path_to_inode(storage: &MetadataStore, path: &str) -> Option<InodeId> {
    // This is a simplified implementation
    // In a real system, this would traverse the directory tree
    if path == "/" {
        Some(1) // Root inode is typically 1
    } else {
        // For now, return None for non-root paths
        // This would be enhanced with proper path resolution
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataStore;
    use mooseng_common::types::{FsNodeType, now_micros};
    use std::time::Duration;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cached_metadata_manager() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
        
        let config = CacheConfig {
            ttl_secs: 60,
            max_size: 1000,
            hot_threshold: 3,
            enable_lru: true,
            enable_prefetch: true,
            enable_invalidation: true,
            ..Default::default()
        };

        let manager = CachedMetadataManager::new(storage, config).await.unwrap();

        // Test cached inode operations
        let inode_id = manager.next_inode_id().unwrap();
        let node = FsNode {
            inode: inode_id,
            parent: None,
            ctime: now_micros(),
            mtime: now_micros(),
            atime: now_micros(),
            uid: 1000,
            gid: 1000,
            mode: 0o644,
            flags: 0,
            winattr: 0,
            storage_class_id: 1,
            trash_retention: 7,
            node_type: FsNodeType::File {
                length: 1024,
                chunk_ids: vec![],
                session_id: None,
            },
        };

        // Save inode
        manager.save_inode(&node).await.unwrap();

        // First get should be a cache miss, but populate cache
        let retrieved1 = manager.get_inode(inode_id).await.unwrap();
        assert!(retrieved1.is_some());

        // Second get should be a cache hit
        let retrieved2 = manager.get_inode(inode_id).await.unwrap();
        assert!(retrieved2.is_some());

        // Check cache statistics
        let stats = manager.cache_stats();
        assert!(stats["hits"] > 0);
        assert!(stats["inserts"] > 0);

        // Test cache invalidation
        manager.delete_inode(inode_id).await.unwrap();
        let retrieved3 = manager.get_inode(inode_id).await.unwrap();
        assert!(retrieved3.is_none());
    }

    #[tokio::test]
    async fn test_cache_hit_rate() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
        
        let config = CacheConfig::default();
        let manager = CachedMetadataManager::new(storage, config).await.unwrap();

        // Create test nodes
        let mut inodes = Vec::new();
        for i in 0..10 {
            let inode_id = manager.next_inode_id().unwrap();
            let node = FsNode {
                inode: inode_id,
                parent: None,
                ctime: now_micros(),
                mtime: now_micros(),
                atime: now_micros(),
                uid: 1000,
                gid: 1000,
                mode: 0o644,
                flags: 0,
                winattr: 0,
                storage_class_id: 1,
                trash_retention: 7,
                node_type: FsNodeType::File {
                    length: 1024,
                    chunk_ids: vec![],
                    session_id: None,
                },
            };
            manager.save_inode(&node).await.unwrap();
            inodes.push(inode_id);
        }

        // Access inodes multiple times to generate hits
        for _ in 0..5 {
            for &inode in &inodes {
                manager.get_inode(inode).await.unwrap();
            }
        }

        // Check hit rate
        let hit_rate = manager.cache_hit_rate();
        assert!(hit_rate > 0.5); // Should have good hit rate
    }

    #[tokio::test]
    async fn test_prefetch_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
        
        let config = CacheConfig {
            enable_prefetch: true,
            ..Default::default()
        };
        let manager = CachedMetadataManager::new(storage, config).await.unwrap();

        // Create test nodes
        let mut inodes = Vec::new();
        for i in 0..5 {
            let inode_id = manager.next_inode_id().unwrap();
            let node = FsNode {
                inode: inode_id,
                parent: None,
                ctime: now_micros(),
                mtime: now_micros(),
                atime: now_micros(),
                uid: 1000,
                gid: 1000,
                mode: 0o644,
                flags: 0,
                winattr: 0,
                storage_class_id: 1,
                trash_retention: 7,
                node_type: FsNodeType::File {
                    length: 1024,
                    chunk_ids: vec![],
                    session_id: None,
                },
            };
            manager.save_inode(&node).await.unwrap();
            inodes.push(inode_id);
        }

        // Request prefetch
        manager.prefetch_hot_inodes(inodes.clone()).await.unwrap();

        // Give prefetch some time to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Access should now be cache hits
        for &inode in &inodes {
            manager.get_inode(inode).await.unwrap();
        }

        let stats = manager.cache_stats();
        assert!(stats["prefetches"] > 0);
    }
}