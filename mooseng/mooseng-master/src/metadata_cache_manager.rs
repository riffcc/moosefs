use crate::cache_enhanced::{EnhancedMetadataCache, CacheInvalidation, PrefetchHint};
use crate::cache_configuration::CacheConfig;
use crate::metadata::MetadataStore;
use mooseng_common::types::{ChunkId, ChunkMetadata, FsEdge, FsNode, InodeId};
use mooseng_common::metrics::{MasterMetrics, MetricCollector};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, warn, trace, instrument};
use tokio::sync::RwLock;

/// Integrated metadata manager that combines persistent storage with intelligent caching
pub struct MetadataCacheManager {
    /// Underlying persistent metadata store
    store: Arc<MetadataStore>,
    /// Enhanced metadata cache for performance
    cache: Arc<EnhancedMetadataCache>,
    /// Cache configuration for dynamic tuning
    config: Arc<RwLock<CacheConfig>>,
    /// Prometheus metrics for monitoring
    metrics: Option<Arc<MasterMetrics>>,
}

impl MetadataCacheManager {
    /// Create a new metadata cache manager
    pub async fn new(
        store: Arc<MetadataStore>,
        config: CacheConfig,
    ) -> Result<Self> {
        Self::new_with_metrics(store, config, None).await
    }

    /// Create a new metadata cache manager with metrics
    pub async fn new_with_metrics(
        store: Arc<MetadataStore>,
        config: CacheConfig,
        metrics: Option<Arc<MasterMetrics>>,
    ) -> Result<Self> {
        // Validate configuration
        config.validate().map_err(|e| anyhow::anyhow!(e)).context("Invalid cache configuration")?;
        
        // Create cache with configuration
        let cache = Arc::new(EnhancedMetadataCache::new(
            config.ttl(),
            config.max_size,
            config.hot_threshold,
        ));
        
        // Start background maintenance if enabled
        if config.enable_lru {
            cache.clone().start_maintenance();
        }
        
        let manager = Self {
            store,
            cache,
            config: Arc::new(RwLock::new(config)),
            metrics,
        };
        
        // Start cache warmer if prefetching is enabled
        if manager.config.read().await.enable_prefetch {
            manager.start_cache_warmer().await;
        }
        
        Ok(manager)
    }
    
    /// Start the cache warmer for proactive prefetching
    async fn start_cache_warmer(&self) {
        use crate::cache_enhanced::CacheWarmer;
        
        let store = self.store.clone();
        let cache = self.cache.clone();
        
        // Create fetch function that loads from persistent store
        let fetch_fn = move |hint: PrefetchHint| -> Vec<FsNode> {
            let mut nodes = Vec::new();
            
            // Fetch requested inodes
            for inode in hint.inodes {
                if let Ok(Some(node)) = store.get_inode(inode) {
                    nodes.push(node);
                }
            }
            
            // Fetch nodes for requested paths
            for path in hint.paths {
                if let Ok(Some(inode)) = resolve_path_to_inode(&store, &path) {
                    if let Ok(Some(node)) = store.get_inode(inode) {
                        nodes.push(node);
                    }
                }
            }
            
            nodes
        };
        
        let warmer = CacheWarmer::new(cache, fetch_fn);
        
        // Get prefetch receiver (this would need to be implemented properly)
        // For now, we'll set up a simple prefetch system
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        warmer.start(rx).await;
        
        // Store the sender for later use (would need proper storage)
        // self.prefetch_tx = Some(tx);
    }
    
    // Enhanced inode operations with caching
    
    /// Get an inode with intelligent caching
    #[instrument(skip(self), fields(inode = %inode))]
    pub async fn get_inode(&self, inode: InodeId) -> Result<Option<FsNode>> {
        let start = Instant::now();
        trace!("Getting inode {}", inode);
        
        // Try cache first
        if let Some(node) = self.cache.get_inode(inode) {
            trace!("Cache hit for inode {}", inode);
            
            // Record cache hit metrics
            if let Some(ref metrics) = self.metrics {
                metrics.metadata_cache_hits.inc();
                metrics.metadata_operation_duration
                    .with_label_values(&["get_inode"])
                    .observe(start.elapsed().as_secs_f64());
                metrics.metadata_operations_total
                    .with_label_values(&["get_inode", "cache_hit"])
                    .inc();
            }
            
            return Ok(Some(node));
        }
        
        trace!("Cache miss for inode {}, fetching from store", inode);
        
        // Record cache miss metrics
        if let Some(ref metrics) = self.metrics {
            metrics.metadata_cache_misses.inc();
        }
        
        // Cache miss - fetch from persistent store
        let node = self.store.get_inode(inode)
            .context("Failed to fetch inode from store")?;
        
        // Cache the result if found
        if let Some(ref node) = node {
            self.cache.insert_inode(node.clone());
            debug!("Cached inode {} after store fetch", inode);
        }
        
        // Record operation metrics
        if let Some(ref metrics) = self.metrics {
            let status = if node.is_some() { "success" } else { "not_found" };
            metrics.metadata_operations_total
                .with_label_values(&["get_inode", status])
                .inc();
            metrics.metadata_operation_duration
                .with_label_values(&["get_inode"])
                .observe(start.elapsed().as_secs_f64());
        }
        
        Ok(node)
    }
    
    /// Save an inode with cache invalidation
    #[instrument(skip(self, node), fields(inode = %node.inode))]
    pub async fn save_inode(&self, node: &FsNode) -> Result<()> {
        let start = Instant::now();
        trace!("Saving inode {}", node.inode);
        
        // Save to persistent store first
        let store_result = self.store.save_inode(node)
            .context("Failed to save inode to store");
        
        match store_result {
            Ok(()) => {
                // Update cache
                self.cache.insert_inode(node.clone());
                
                // Invalidate across cluster if enabled
                let config = self.config.read().await;
                if config.enable_invalidation {
                    self.cache.invalidate(CacheInvalidation::InvalidateInode(node.inode)).await;
                    debug!("Sent invalidation for inode {}", node.inode);
                }
                
                // Record success metrics
                if let Some(ref metrics) = self.metrics {
                    metrics.metadata_operations_total
                        .with_label_values(&["save_inode", "success"])
                        .inc();
                    metrics.metadata_operation_duration
                        .with_label_values(&["save_inode"])
                        .observe(start.elapsed().as_secs_f64());
                }
                
                debug!("Successfully saved and cached inode {}", node.inode);
                Ok(())
            }
            Err(e) => {
                // Record error metrics
                if let Some(ref metrics) = self.metrics {
                    metrics.metadata_operations_total
                        .with_label_values(&["save_inode", "error"])
                        .inc();
                    metrics.metadata_operation_duration
                        .with_label_values(&["save_inode"])
                        .observe(start.elapsed().as_secs_f64());
                }
                Err(e)
            }
        }
    }
    
    /// Delete an inode with cache cleanup
    #[instrument(skip(self), fields(inode = %inode))]
    pub async fn delete_inode(&self, inode: InodeId) -> Result<()> {
        let start = Instant::now();
        trace!("Deleting inode {}", inode);
        
        // Delete from persistent store
        let delete_result = self.store.delete_inode(inode)
            .context("Failed to delete inode from store");
        
        match delete_result {
            Ok(()) => {
                // Remove from cache and invalidate
                self.cache.invalidate(CacheInvalidation::InvalidateInode(inode)).await;
                
                // Record success metrics
                if let Some(ref metrics) = self.metrics {
                    metrics.metadata_operations_total
                        .with_label_values(&["delete_inode", "success"])
                        .inc();
                    metrics.metadata_operation_duration
                        .with_label_values(&["delete_inode"])
                        .observe(start.elapsed().as_secs_f64());
                }
                
                debug!("Successfully deleted inode {}", inode);
                Ok(())
            }
            Err(e) => {
                // Record error metrics
                if let Some(ref metrics) = self.metrics {
                    metrics.metadata_operations_total
                        .with_label_values(&["delete_inode", "error"])
                        .inc();
                    metrics.metadata_operation_duration
                        .with_label_values(&["delete_inode"])
                        .observe(start.elapsed().as_secs_f64());
                }
                Err(e)
            }
        }
    }
    
    /// List inodes with intelligent prefetching
    #[instrument(skip(self))]
    pub async fn list_inodes(&self) -> Result<Vec<FsNode>> {
        trace!("Listing all inodes");
        
        // This operation always goes to store since it's a bulk operation
        let nodes = self.store.list_inodes()
            .context("Failed to list inodes from store")?;
        
        // Cache the results for future access
        for node in &nodes {
            self.cache.insert_inode(node.clone());
        }
        
        debug!("Listed and cached {} inodes", nodes.len());
        Ok(nodes)
    }
    
    // Enhanced directory operations
    
    /// Find child with path-based caching
    #[instrument(skip(self), fields(parent = %parent, name = %name))]
    pub async fn find_child(&self, parent: InodeId, name: &str) -> Result<Option<InodeId>> {
        trace!("Finding child '{}' in parent {}", name, parent);
        
        // Try to get parent from cache first to potentially prefetch children
        if let Some(_parent_node) = self.cache.get_inode(parent) {
            trace!("Parent {} found in cache", parent);
        }
        
        // Find child in store (could be enhanced with path caching)
        let child_id = self.store.find_child(parent, name)
            .context("Failed to find child in store")?;
        
        if let Some(child_id) = child_id {
            // Prefetch the child node
            if let Ok(Some(child_node)) = self.store.get_inode(child_id) {
                self.cache.insert_inode(child_node);
                debug!("Prefetched child node {} ('{}' in {})", child_id, name, parent);
            }
        }
        
        Ok(child_id)
    }
    
    /// List children with intelligent prefetching
    #[instrument(skip(self), fields(parent = %parent))]
    pub async fn list_children(&self, parent: InodeId) -> Result<Vec<FsEdge>> {
        trace!("Listing children of parent {}", parent);
        
        // Get children from store
        let children = self.store.list_children(parent)
            .context("Failed to list children from store")?;
        
        // Prefetch child inodes that aren't already cached
        let config = self.config.read().await;
        if config.enable_prefetch && !children.is_empty() {
            let inode_ids: Vec<InodeId> = children.iter()
                .map(|edge| edge.child_inode)
                .collect();
            
            let hint = PrefetchHint {
                inodes: inode_ids,
                chunks: vec![],
                paths: vec![],
            };
            
            self.cache.request_prefetch(hint).await;
            debug!("Requested prefetch for {} child inodes of parent {}", children.len(), parent);
        }
        
        Ok(children)
    }
    
    // Enhanced chunk operations
    
    /// Get chunk metadata with caching
    #[instrument(skip(self), fields(chunk_id = %chunk_id))]
    pub async fn get_chunk(&self, chunk_id: ChunkId) -> Result<Option<ChunkMetadata>> {
        trace!("Getting chunk metadata for {}", chunk_id);
        
        // For now, delegate to store (chunk caching could be added later)
        self.store.get_chunk(chunk_id)
            .context("Failed to get chunk from store")
    }
    
    /// Save chunk metadata
    #[instrument(skip(self, metadata), fields(chunk_id = %metadata.chunk_id))]
    pub async fn save_chunk(&self, metadata: &ChunkMetadata) -> Result<()> {
        trace!("Saving chunk metadata for {}", metadata.chunk_id);
        
        // Save to store
        self.store.save_chunk(metadata)
            .context("Failed to save chunk to store")?;
        
        // Invalidate chunk-related caches if needed
        let config = self.config.read().await;
        if config.enable_invalidation {
            self.cache.invalidate(CacheInvalidation::InvalidateChunk(metadata.chunk_id)).await;
        }
        
        debug!("Successfully saved chunk metadata for {}", metadata.chunk_id);
        Ok(())
    }
    
    // Path resolution with caching
    
    /// Resolve a filesystem path to an inode with path caching
    #[instrument(skip(self), fields(path = %path))]
    pub async fn resolve_path(&self, path: &str) -> Result<Option<InodeId>> {
        trace!("Resolving path: {}", path);
        
        // Could implement path-to-inode caching here
        // For now, delegate to helper function
        resolve_path_to_inode(&self.store, path)
            .context("Failed to resolve path")
    }
    
    // Cache management operations
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> std::collections::HashMap<String, u64> {
        self.cache.stats().get_stats()
    }
    
    /// Get cache configuration
    pub async fn get_cache_config(&self) -> CacheConfig {
        self.config.read().await.clone()
    }
    
    /// Update cache configuration
    pub async fn update_cache_config(&self, new_config: CacheConfig) -> Result<()> {
        // Validate new configuration
        new_config.validate().map_err(|e| anyhow::anyhow!(e)).context("Invalid cache configuration")?;
        
        // Update stored configuration
        let mut config = self.config.write().await;
        *config = new_config;
        
        debug!("Updated cache configuration");
        Ok(())
    }
    
    /// Clear all caches
    pub async fn clear_caches(&self) {
        self.cache.clear();
        debug!("Cleared all caches");
    }
    
    /// Manually trigger cache invalidation
    pub async fn invalidate_inode(&self, inode: InodeId) {
        self.cache.invalidate(CacheInvalidation::InvalidateInode(inode)).await;
    }
    
    /// Manually trigger cache invalidation for a path
    pub async fn invalidate_path(&self, path: &str) {
        self.cache.invalidate(CacheInvalidation::InvalidatePath(path.to_string())).await;
    }
    
    /// Request prefetching for specific inodes
    pub async fn prefetch_inodes(&self, inodes: Vec<InodeId>) {
        let hint = PrefetchHint {
            inodes,
            chunks: vec![],
            paths: vec![],
        };
        self.cache.request_prefetch(hint).await;
    }
    
    /// Request prefetching for specific paths
    pub async fn prefetch_paths(&self, paths: Vec<String>) {
        let hint = PrefetchHint {
            inodes: vec![],
            chunks: vec![],
            paths,
        };
        self.cache.request_prefetch(hint).await;
    }
    
    // Health monitoring
    
    /// Check cache health and get recommendations
    pub async fn get_cache_health(&self) -> (bool, Vec<String>) {
        use crate::cache_configuration::CacheMetrics;
        
        let stats = self.cache.stats().get_stats();
        let config = self.config.read().await;
        
        // Estimate cache size (would need proper implementation)
        let estimated_entries = stats["inserts"] - stats["evictions"];
        
        let metrics = CacheMetrics::calculate(
            stats["hits"],
            stats["misses"],
            stats["evictions"],
            estimated_entries as usize,
            stats["hot_entries"] as usize,
            config.max_size,
            estimated_entries as usize * 1024, // Rough estimate
        );
        
        let is_healthy = metrics.is_healthy();
        let recommendations = metrics.recommendations();
        
        if !is_healthy {
            warn!("Cache health issues detected: {:?}", recommendations);
        }
        
        (is_healthy, recommendations)
    }
}

// Helper functions

/// Resolve a filesystem path to an inode ID
fn resolve_path_to_inode(store: &MetadataStore, path: &str) -> Result<Option<InodeId>> {
    if path == "/" {
        return Ok(Some(1)); // Root inode is typically 1
    }
    
    let components: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let mut current_inode = 1; // Start from root
    
    for component in components {
        if component.is_empty() {
            continue;
        }
        
        match store.find_child(current_inode, component)? {
            Some(child_inode) => {
                current_inode = child_inode;
            }
            None => return Ok(None), // Path component not found
        }
    }
    
    Ok(Some(current_inode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use mooseng_common::types::{FsNodeType, now_micros};
    
    async fn create_test_manager() -> (MetadataCacheManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
        
        let config = CacheConfig {
            ttl_secs: 60,
            max_size: 1000,
            hot_threshold: 3,
            enable_lru: true,
            enable_prefetch: true,
            enable_invalidation: true,
            cleanup_interval_secs: 30,
            stats_interval_secs: 60,
            eviction_percentage: 10,
            enable_compression: false,
            compression_threshold: 4096,
        };
        
        let manager = MetadataCacheManager::new(store, config).await.unwrap();
        (manager, temp_dir)
    }
    
    async fn create_test_manager_with_metrics() -> (MetadataCacheManager, Arc<MasterMetrics>, TempDir) {
        use mooseng_common::metrics::MetricsRegistry;
        
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
        
        let config = CacheConfig {
            ttl_secs: 60,
            max_size: 1000,
            hot_threshold: 3,
            enable_lru: true,
            enable_prefetch: true,
            enable_invalidation: true,
            cleanup_interval_secs: 30,
            stats_interval_secs: 60,
            eviction_percentage: 10,
            enable_compression: false,
            compression_threshold: 4096,
        };
        
        let metrics_registry = MetricsRegistry::new("test_master").unwrap();
        let metrics = Arc::new(MasterMetrics::new(&metrics_registry).unwrap());
        
        let manager = MetadataCacheManager::new_with_metrics(store, config, Some(metrics.clone())).await.unwrap();
        (manager, metrics, temp_dir)
    }
    
    fn create_test_node(inode: InodeId, parent: Option<InodeId>) -> FsNode {
        FsNode {
            inode,
            parent,
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
        }
    }
    
    #[tokio::test]
    async fn test_cache_integration_with_metadata() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        // Create and save a test node
        let node = create_test_node(42, Some(1));
        manager.save_inode(&node).await.unwrap();
        
        // First get should hit the cache (inserted during save)
        let retrieved = manager.get_inode(42).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().inode, 42);
        
        // Check cache stats
        let stats = manager.get_cache_stats().await;
        assert!(stats["hits"] >= 1);
        assert!(stats["inserts"] >= 1);
    }
    
    #[tokio::test]
    async fn test_cache_invalidation_on_delete() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        // Create and save a test node
        let node = create_test_node(123, Some(1));
        manager.save_inode(&node).await.unwrap();
        
        // Verify it's cached
        assert!(manager.get_inode(123).await.unwrap().is_some());
        
        // Delete the node
        manager.delete_inode(123).await.unwrap();
        
        // Should not be found
        assert!(manager.get_inode(123).await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_cache_health_monitoring() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        // Generate some cache activity
        for i in 1..=20 {
            let node = create_test_node(i, Some(1));
            manager.save_inode(&node).await.unwrap();
        }
        
        // Access some nodes to create hits
        for i in 1..=10 {
            manager.get_inode(i).await.unwrap();
        }
        
        // Check cache health
        let (is_healthy, recommendations) = manager.get_cache_health().await;
        println!("Cache healthy: {}, recommendations: {:?}", is_healthy, recommendations);
        
        // Should have some activity
        let stats = manager.get_cache_stats().await;
        assert!(stats["hits"] > 0);
        assert!(stats["inserts"] > 0);
    }
    
    #[tokio::test]
    async fn test_configuration_update() {
        let (manager, _temp_dir) = create_test_manager().await;
        
        // Get initial config
        let initial_config = manager.get_cache_config().await;
        assert_eq!(initial_config.max_size, 1000);
        
        // Update configuration
        let mut new_config = initial_config;
        new_config.max_size = 2000;
        new_config.ttl_secs = 120;
        
        manager.update_cache_config(new_config).await.unwrap();
        
        // Verify update
        let updated_config = manager.get_cache_config().await;
        assert_eq!(updated_config.max_size, 2000);
        assert_eq!(updated_config.ttl_secs, 120);
    }
    
    #[tokio::test]
    async fn test_metrics_integration() {
        let (manager, metrics, _temp_dir) = create_test_manager_with_metrics().await;
        
        // Create and save a test node to generate metrics
        let node = create_test_node(42, Some(1));
        manager.save_inode(&node).await.unwrap();
        
        // Get the node to generate cache hit metrics
        let _retrieved = manager.get_inode(42).await.unwrap();
        
        // Try to get a non-existent node to generate cache miss metrics
        let _not_found = manager.get_inode(999).await.unwrap();
        
        // Create and test the metrics collector
        let cache_manager_arc = Arc::new(manager);
        let collector = MetadataCacheMetricsCollector::new(
            cache_manager_arc.clone(),
            metrics.clone(),
        );
        
        // Run the collector
        collector.collect().await.unwrap();
        
        // Verify that some metrics were recorded
        // Note: In a real test environment with proper Prometheus setup,
        // we could check the actual metric values
        println!("Metrics integration test completed successfully");
    }
}

/// Metrics collector for metadata cache statistics
pub struct MetadataCacheMetricsCollector {
    cache_manager: Arc<MetadataCacheManager>,
    metrics: Arc<MasterMetrics>,
}

impl MetadataCacheMetricsCollector {
    pub fn new(
        cache_manager: Arc<MetadataCacheManager>,
        metrics: Arc<MasterMetrics>,
    ) -> Self {
        Self {
            cache_manager,
            metrics,
        }
    }
}

#[async_trait::async_trait]
impl MetricCollector for MetadataCacheMetricsCollector {
    async fn collect(&self) -> Result<()> {
        // Get cache statistics
        let cache_stats = self.cache_manager.get_cache_stats().await;
        
        // Update cache size metric (estimated)
        let estimated_entries = cache_stats.get("inserts").unwrap_or(&0) - cache_stats.get("evictions").unwrap_or(&0);
        let estimated_size_bytes = estimated_entries * 1024; // Rough estimate
        self.metrics.metadata_cache_size.set(estimated_size_bytes as i64);
        
        // Update cache statistics - these are already tracked by individual operations
        // but we can also update them here for consistency
        
        // Check cache health and update health status
        let (is_healthy, recommendations) = self.cache_manager.get_cache_health().await;
        
        if !is_healthy {
            warn!("Metadata cache health issues detected: {:?}", recommendations);
            // Could record specific metrics about cache health issues
            self.metrics.common.record_error("cache_health", "warning");
        }
        
        debug!("Updated metadata cache metrics: {} entries, {} bytes estimated", 
               estimated_entries, estimated_size_bytes);
        
        Ok(())
    }
}