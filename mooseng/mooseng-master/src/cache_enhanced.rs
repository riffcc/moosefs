use dashmap::DashMap;
use mooseng_common::types::{ChunkId, ChunkMetadata, FsNode, InodeId, StorageClassDef};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc};
use tokio::time;
use tracing::{debug, trace, warn};

/// LRU eviction data for tracking access order
#[derive(Default)]
struct LruData {
    access_order: VecDeque<InodeId>,
    access_counts: HashMap<InodeId, usize>,
}

/// Cache invalidation message types
#[derive(Clone, Debug)]
pub enum CacheInvalidation {
    InvalidateInode(InodeId),
    InvalidateChunk(ChunkId),
    InvalidatePath(String),
    InvalidateStorageClass(u8),
    InvalidateAll,
}

/// Prefetch hint for cache warming
#[derive(Clone, Debug)]
pub struct PrefetchHint {
    pub inodes: Vec<InodeId>,
    pub chunks: Vec<ChunkId>,
    pub paths: Vec<String>,
}

/// Cache entry with timestamp and access tracking
#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    inserted_at: Instant,
    last_accessed: Instant,
    access_count: usize,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        let now = Instant::now();
        Self {
            value,
            inserted_at: now,
            last_accessed: now,
            access_count: 1,
        }
    }
    
    fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
    
    fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() > ttl
    }
    
    fn is_hot(&self, threshold: usize) -> bool {
        self.access_count >= threshold
    }
}

/// Enhanced metadata cache with LRU eviction, prefetching, and invalidation
pub struct EnhancedMetadataCache {
    // Core cache storage
    inodes: Arc<DashMap<InodeId, CacheEntry<FsNode>>>,
    chunks: Arc<DashMap<ChunkId, CacheEntry<ChunkMetadata>>>,
    storage_classes: Arc<DashMap<u8, CacheEntry<StorageClassDef>>>,
    path_to_inode: Arc<DashMap<String, CacheEntry<InodeId>>>,
    
    // LRU tracking
    inode_lru: Arc<RwLock<LruData>>,
    chunk_lru: Arc<RwLock<LruData>>,
    
    // Cache configuration
    ttl: Duration,
    max_size: usize,
    hot_threshold: usize,
    
    // Statistics
    stats: Arc<CacheStats>,
    
    // Invalidation channel
    invalidation_tx: broadcast::Sender<CacheInvalidation>,
    
    // Prefetch channel
    prefetch_tx: mpsc::Sender<PrefetchHint>,
}

/// Statistics for enhanced cache monitoring
#[derive(Default)]
pub struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
    updates: AtomicU64,
    evictions: AtomicU64,
    invalidations: AtomicU64,
    prefetches: AtomicU64,
    hot_entries: AtomicUsize,
}

impl CacheStats {
    pub fn hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn insert(&self) {
        self.inserts.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn update(&self) {
        self.updates.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn evict(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn invalidate(&self) {
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn prefetch(&self) {
        self.prefetches.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn set_hot_entries(&self, count: usize) {
        self.hot_entries.store(count, Ordering::Relaxed);
    }
    
    pub fn get_stats(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();
        stats.insert("hits".to_string(), self.hits.load(Ordering::Relaxed));
        stats.insert("misses".to_string(), self.misses.load(Ordering::Relaxed));
        stats.insert("inserts".to_string(), self.inserts.load(Ordering::Relaxed));
        stats.insert("updates".to_string(), self.updates.load(Ordering::Relaxed));
        stats.insert("evictions".to_string(), self.evictions.load(Ordering::Relaxed));
        stats.insert("invalidations".to_string(), self.invalidations.load(Ordering::Relaxed));
        stats.insert("prefetches".to_string(), self.prefetches.load(Ordering::Relaxed));
        stats.insert("hot_entries".to_string(), self.hot_entries.load(Ordering::Relaxed) as u64);
        stats
    }
    
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }
}

impl EnhancedMetadataCache {
    /// Create a new enhanced metadata cache
    pub fn new(ttl: Duration, max_size: usize, hot_threshold: usize) -> Self {
        let (invalidation_tx, _) = broadcast::channel(1000);
        let (prefetch_tx, _) = mpsc::channel(100);
        
        Self {
            inodes: Arc::new(DashMap::with_capacity(max_size / 4)),
            chunks: Arc::new(DashMap::with_capacity(max_size / 2)),
            storage_classes: Arc::new(DashMap::with_capacity(256)),
            path_to_inode: Arc::new(DashMap::with_capacity(max_size / 4)),
            inode_lru: Arc::new(RwLock::new(LruData::default())),
            chunk_lru: Arc::new(RwLock::new(LruData::default())),
            ttl,
            max_size,
            hot_threshold,
            stats: Arc::new(CacheStats::default()),
            invalidation_tx,
            prefetch_tx,
        }
    }
    
    /// Subscribe to cache invalidation events
    pub fn subscribe_invalidations(&self) -> broadcast::Receiver<CacheInvalidation> {
        self.invalidation_tx.subscribe()
    }
    
    /// Get the prefetch channel receiver
    pub fn get_prefetch_receiver(&self) -> mpsc::Receiver<PrefetchHint> {
        // This would be called once during initialization
        // In a real implementation, we'd store the receiver separately
        panic!("Prefetch receiver should be obtained during initialization")
    }
    
    /// Invalidate cache entries
    pub async fn invalidate(&self, msg: CacheInvalidation) {
        match &msg {
            CacheInvalidation::InvalidateInode(inode) => {
                if self.inodes.remove(inode).is_some() {
                    self.stats.invalidate();
                    debug!("Invalidated inode {} from cache", inode);
                }
            }
            CacheInvalidation::InvalidateChunk(chunk_id) => {
                if self.chunks.remove(chunk_id).is_some() {
                    self.stats.invalidate();
                    debug!("Invalidated chunk {} from cache", chunk_id);
                }
            }
            CacheInvalidation::InvalidatePath(path) => {
                if self.path_to_inode.remove(path).is_some() {
                    self.stats.invalidate();
                    debug!("Invalidated path {} from cache", path);
                }
            }
            CacheInvalidation::InvalidateStorageClass(id) => {
                if self.storage_classes.remove(id).is_some() {
                    self.stats.invalidate();
                    debug!("Invalidated storage class {} from cache", id);
                }
            }
            CacheInvalidation::InvalidateAll => {
                self.clear();
                debug!("Invalidated all cache entries");
            }
        }
        
        // Broadcast invalidation to other nodes/components
        let _ = self.invalidation_tx.send(msg);
    }
    
    /// Request prefetching of entries
    pub async fn request_prefetch(&self, hint: PrefetchHint) {
        self.stats.prefetch();
        let _ = self.prefetch_tx.send(hint).await;
    }
    
    // Enhanced inode operations with LRU tracking
    
    /// Get an inode from cache with LRU tracking
    pub fn get_inode(&self, inode: InodeId) -> Option<FsNode> {
        match self.inodes.get_mut(&inode) {
            Some(mut entry) => {
                if entry.is_expired(self.ttl) {
                    trace!("Inode {} expired in cache", inode);
                    drop(entry);
                    self.inodes.remove(&inode);
                    self.remove_from_lru(inode);
                    self.stats.evict();
                    self.stats.miss();
                    None
                } else {
                    entry.touch();
                    let is_hot = entry.is_hot(self.hot_threshold);
                    let value = entry.value.clone();
                    drop(entry);
                    
                    self.update_lru(inode);
                    if is_hot {
                        trace!("Cache hit for hot inode {}", inode);
                    } else {
                        trace!("Cache hit for inode {}", inode);
                    }
                    self.stats.hit();
                    Some(value)
                }
            }
            None => {
                trace!("Cache miss for inode {}", inode);
                self.stats.miss();
                None
            }
        }
    }
    
    /// Insert or update an inode with LRU tracking
    pub fn insert_inode(&self, node: FsNode) {
        let inode = node.inode;
        
        // Check if we need to evict based on LRU
        if self.inodes.len() >= self.max_size / 4 {
            self.evict_lru_inodes();
        }
        
        if self.inodes.contains_key(&inode) {
            self.stats.update();
            debug!("Updated inode {} in cache", inode);
        } else {
            self.stats.insert();
            self.add_to_lru(inode);
            debug!("Inserted inode {} in cache", inode);
        }
        
        self.inodes.insert(inode, CacheEntry::new(node));
    }
    
    /// Update LRU tracking for an inode
    fn update_lru(&self, inode: InodeId) {
        if let Ok(mut lru) = self.inode_lru.write() {
            // Remove from current position
            if let Some(pos) = lru.access_order.iter().position(|&id| id == inode) {
                lru.access_order.remove(pos);
            }
            // Add to front (most recently used)
            lru.access_order.push_front(inode);
            
            // Update access count
            *lru.access_counts.entry(inode).or_insert(0) += 1;
        }
    }
    
    /// Add a new inode to LRU tracking
    fn add_to_lru(&self, inode: InodeId) {
        if let Ok(mut lru) = self.inode_lru.write() {
            lru.access_order.push_front(inode);
            lru.access_counts.insert(inode, 1);
        }
    }
    
    /// Remove an inode from LRU tracking
    fn remove_from_lru(&self, inode: InodeId) {
        if let Ok(mut lru) = self.inode_lru.write() {
            if let Some(pos) = lru.access_order.iter().position(|&id| id == inode) {
                lru.access_order.remove(pos);
            }
            lru.access_counts.remove(&inode);
        }
    }
    
    /// Evict least recently used inodes
    fn evict_lru_inodes(&self) {
        if let Ok(mut lru) = self.inode_lru.write() {
            let target_evictions = self.max_size / 20; // Evict 5% at a time
            let mut evicted = 0;
            
            while evicted < target_evictions && !lru.access_order.is_empty() {
                // Get LRU inode from back of deque
                if let Some(inode) = lru.access_order.pop_back() {
                    // Skip hot entries
                    if let Some(&count) = lru.access_counts.get(&inode) {
                        if count >= self.hot_threshold {
                            // Move hot entry back to front
                            lru.access_order.push_front(inode);
                            continue;
                        }
                    }
                    
                    // Evict the entry
                    if self.inodes.remove(&inode).is_some() {
                        lru.access_counts.remove(&inode);
                        self.stats.evict();
                        evicted += 1;
                        trace!("LRU evicted inode {}", inode);
                    }
                }
            }
            
            if evicted > 0 {
                debug!("LRU evicted {} inodes", evicted);
            }
        }
    }
    
    /// Get cache statistics including hot entry count
    pub fn stats(&self) -> &CacheStats {
        // Update hot entry count
        if let Ok(lru) = self.inode_lru.read() {
            let hot_count = lru.access_counts.values()
                .filter(|&&count| count >= self.hot_threshold)
                .count();
            self.stats.set_hot_entries(hot_count);
        }
        
        &self.stats
    }
    
    /// Clear all caches
    pub fn clear(&self) {
        let evicted = self.inodes.len() + self.chunks.len() + 
                     self.storage_classes.len() + self.path_to_inode.len();
        
        self.inodes.clear();
        self.chunks.clear();
        self.storage_classes.clear();
        self.path_to_inode.clear();
        
        if let Ok(mut lru) = self.inode_lru.write() {
            lru.access_order.clear();
            lru.access_counts.clear();
        }
        
        if let Ok(mut lru) = self.chunk_lru.write() {
            lru.access_order.clear();
            lru.access_counts.clear();
        }
        
        for _ in 0..evicted {
            self.stats.evict();
        }
        
        debug!("Cleared all caches ({} entries evicted)", evicted);
    }
    
    /// Start background maintenance tasks
    pub fn start_maintenance(self: Arc<Self>) {
        // Periodic TTL cleanup
        let cache = self.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                cache.cleanup_expired_entries();
            }
        });
        
        // Periodic stats reporting
        let cache = self.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                let stats = cache.stats().get_stats();
                let hit_rate = cache.stats().hit_rate();
                debug!("Cache stats: {:?}, hit_rate: {:.2}%", stats, hit_rate * 100.0);
            }
        });
    }
    
    /// Clean up expired entries
    fn cleanup_expired_entries(&self) {
        let mut expired_count = 0;
        
        // Clean up expired inodes
        let expired_inodes: Vec<InodeId> = self.inodes.iter()
            .filter(|entry| entry.value().is_expired(self.ttl))
            .map(|entry| *entry.key())
            .collect();
            
        for inode in expired_inodes {
            if self.inodes.remove(&inode).is_some() {
                self.remove_from_lru(inode);
                self.stats.evict();
                expired_count += 1;
            }
        }
        
        // Clean up expired chunks
        let expired_chunks: Vec<ChunkId> = self.chunks.iter()
            .filter(|entry| entry.value().is_expired(self.ttl))
            .map(|entry| *entry.key())
            .collect();
            
        for chunk_id in expired_chunks {
            if self.chunks.remove(&chunk_id).is_some() {
                self.stats.evict();
                expired_count += 1;
            }
        }
        
        // Clean up expired paths
        let expired_paths: Vec<String> = self.path_to_inode.iter()
            .filter(|entry| entry.value().is_expired(self.ttl))
            .map(|entry| entry.key().clone())
            .collect();
            
        for path in expired_paths {
            if self.path_to_inode.remove(&path).is_some() {
                self.stats.evict();
                expired_count += 1;
            }
        }
        
        if expired_count > 0 {
            debug!("Cleaned up {} expired cache entries", expired_count);
        }
    }
}

/// Cache warmer that handles prefetch requests
pub struct CacheWarmer<F> {
    cache: Arc<EnhancedMetadataCache>,
    fetch_fn: F,
}

impl<F> CacheWarmer<F>
where
    F: Fn(PrefetchHint) -> Vec<FsNode> + Send + Sync + 'static,
{
    pub fn new(cache: Arc<EnhancedMetadataCache>, fetch_fn: F) -> Self {
        Self { cache, fetch_fn }
    }
    
    pub async fn start(self, mut receiver: mpsc::Receiver<PrefetchHint>) {
        tokio::spawn(async move {
            while let Some(hint) = receiver.recv().await {
                debug!("Processing prefetch hint: {:?}", hint);
                
                // Fetch the data using the provided function
                let nodes = (self.fetch_fn)(hint);
                
                // Insert into cache
                for node in nodes {
                    self.cache.insert_inode(node);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mooseng_common::types::{FsNodeType, now_micros};
    
    #[tokio::test]
    async fn test_enhanced_cache_lru() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(60),
            4, // Small size to test eviction
            3, // Hot threshold
        ));
        
        // Insert 5 nodes (exceeds capacity)
        for i in 1..=5 {
            let node = FsNode {
                inode: i,
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
                    length: 0,
                    chunk_ids: vec![],
                    session_id: None,
                },
            };
            cache.insert_inode(node);
        }
        
        // Access some nodes to make them "hot"
        for _ in 0..3 {
            cache.get_inode(2);
            cache.get_inode(3);
        }
        
        // Node 1 should have been evicted (LRU)
        assert!(cache.get_inode(1).is_none());
        
        // Nodes 2 and 3 should still be present (hot)
        assert!(cache.get_inode(2).is_some());
        assert!(cache.get_inode(3).is_some());
    }
    
    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(60),
            1000,
            5,
        ));
        
        let node = FsNode {
            inode: 42,
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
                length: 0,
                chunk_ids: vec![],
                session_id: None,
            },
        };
        
        cache.insert_inode(node);
        assert!(cache.get_inode(42).is_some());
        
        // Invalidate the inode
        cache.invalidate(CacheInvalidation::InvalidateInode(42)).await;
        assert!(cache.get_inode(42).is_none());
        
        let stats = cache.stats().get_stats();
        assert_eq!(stats["invalidations"], 1);
    }
    
    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_millis(100), // Very short TTL
            1000,
            5,
        ));
        
        let node = FsNode {
            inode: 100,
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
                length: 0,
                chunk_ids: vec![],
                session_id: None,
            },
        };
        
        cache.insert_inode(node);
        assert!(cache.get_inode(100).is_some());
        
        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should be expired and removed
        assert!(cache.get_inode(100).is_none());
    }
    
    #[tokio::test]
    async fn test_cache_stats() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(60),
            1000,
            3,
        ));
        
        let node = FsNode {
            inode: 123,
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
                length: 0,
                chunk_ids: vec![],
                session_id: None,
            },
        };
        
        // Test miss
        assert!(cache.get_inode(123).is_none());
        
        // Test insert and hit
        cache.insert_inode(node);
        assert!(cache.get_inode(123).is_some());
        assert!(cache.get_inode(123).is_some()); // Second hit
        
        let stats = cache.stats().get_stats();
        assert_eq!(stats["misses"], 1);
        assert_eq!(stats["hits"], 2);
        assert_eq!(stats["inserts"], 1);
        
        let hit_rate = cache.stats().hit_rate();
        assert!((hit_rate - 0.666).abs() < 0.01); // 2 hits out of 3 total requests
    }
    
    #[tokio::test]
    async fn test_prefetch_request() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(60),
            1000,
            5,
        ));
        
        let hint = PrefetchHint {
            inodes: vec![1, 2, 3],
            chunks: vec![],
            paths: vec!["test/path".to_string()],
        };
        
        // This should not panic and should increment prefetch stats
        cache.request_prefetch(hint).await;
        
        let stats = cache.stats().get_stats();
        assert_eq!(stats["prefetches"], 1);
    }
    
    #[tokio::test]
    async fn test_clear_cache() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(60),
            1000,
            5,
        ));
        
        // Add some entries
        for i in 1..=10 {
            let node = FsNode {
                inode: i,
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
                    length: 0,
                    chunk_ids: vec![],
                    session_id: None,
                },
            };
            cache.insert_inode(node);
        }
        
        // Verify entries exist
        assert!(cache.get_inode(5).is_some());
        
        // Clear cache
        cache.clear();
        
        // Verify all entries are gone
        assert!(cache.get_inode(5).is_none());
        
        let stats = cache.stats().get_stats();
        assert_eq!(stats["evictions"], 10); // All entries should be counted as evicted
    }
}