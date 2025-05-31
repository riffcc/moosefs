use dashmap::DashMap;
use mooseng_common::types::{ChunkId, ChunkMetadata, FsNode, InodeId, StorageClassDef};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, trace};

/// Cache entry with timestamp for TTL support
#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    inserted_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            inserted_at: Instant::now(),
        }
    }
    
    fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() > ttl
    }
}

/// Statistics for cache performance monitoring
#[derive(Default)]
pub struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
    updates: AtomicU64,
    evictions: AtomicU64,
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
    
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
            self.inserts.load(Ordering::Relaxed),
            self.updates.load(Ordering::Relaxed),
            self.evictions.load(Ordering::Relaxed),
        )
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

/// Metadata cache using DashMap for thread-safe concurrent access
#[derive(Clone)]
pub struct MetadataCache {
    inodes: Arc<DashMap<InodeId, CacheEntry<FsNode>>>,
    chunks: Arc<DashMap<ChunkId, CacheEntry<ChunkMetadata>>>,
    storage_classes: Arc<DashMap<u8, CacheEntry<StorageClassDef>>>,
    path_to_inode: Arc<DashMap<String, CacheEntry<InodeId>>>,
    stats: Arc<CacheStats>,
    ttl: Duration,
    max_size: usize,
}

impl MetadataCache {
    /// Create a new metadata cache with specified TTL and max size
    pub fn new(ttl: Duration, max_size: usize) -> Self {
        Self {
            inodes: Arc::new(DashMap::with_capacity(max_size / 4)),
            chunks: Arc::new(DashMap::with_capacity(max_size / 2)),
            storage_classes: Arc::new(DashMap::with_capacity(256)),
            path_to_inode: Arc::new(DashMap::with_capacity(max_size / 4)),
            stats: Arc::new(CacheStats::default()),
            ttl,
            max_size,
        }
    }
    
    // Inode cache operations
    
    /// Get an inode from cache
    pub fn get_inode(&self, inode: InodeId) -> Option<FsNode> {
        match self.inodes.get(&inode) {
            Some(entry) => {
                if entry.is_expired(self.ttl) {
                    trace!("Inode {} expired in cache", inode);
                    self.inodes.remove(&inode);
                    self.stats.evict();
                    self.stats.miss();
                    None
                } else {
                    trace!("Cache hit for inode {}", inode);
                    self.stats.hit();
                    Some(entry.value.clone())
                }
            }
            None => {
                trace!("Cache miss for inode {}", inode);
                self.stats.miss();
                None
            }
        }
    }
    
    /// Insert or update an inode in cache
    pub fn insert_inode(&self, node: FsNode) {
        let inode = node.inode;
        
        // Check if we need to evict old entries
        if self.inodes.len() >= self.max_size / 4 {
            self.evict_old_inodes();
        }
        
        if self.inodes.contains_key(&inode) {
            self.stats.update();
            debug!("Updated inode {} in cache", inode);
        } else {
            self.stats.insert();
            debug!("Inserted inode {} in cache", inode);
        }
        
        self.inodes.insert(inode, CacheEntry::new(node));
    }
    
    /// Remove an inode from cache
    pub fn remove_inode(&self, inode: InodeId) {
        if self.inodes.remove(&inode).is_some() {
            debug!("Removed inode {} from cache", inode);
            self.stats.evict();
        }
    }
    
    /// Evict old inodes based on TTL
    fn evict_old_inodes(&self) {
        let mut to_remove = Vec::new();
        
        for entry in self.inodes.iter() {
            if entry.value().is_expired(self.ttl) {
                to_remove.push(*entry.key());
            }
        }
        
        for key in to_remove {
            self.inodes.remove(&key);
            self.stats.evict();
        }
    }
    
    // Chunk cache operations
    
    /// Get chunk metadata from cache
    pub fn get_chunk(&self, chunk_id: ChunkId) -> Option<ChunkMetadata> {
        match self.chunks.get(&chunk_id) {
            Some(entry) => {
                if entry.is_expired(self.ttl) {
                    trace!("Chunk {} expired in cache", chunk_id);
                    self.chunks.remove(&chunk_id);
                    self.stats.evict();
                    self.stats.miss();
                    None
                } else {
                    trace!("Cache hit for chunk {}", chunk_id);
                    self.stats.hit();
                    Some(entry.value.clone())
                }
            }
            None => {
                trace!("Cache miss for chunk {}", chunk_id);
                self.stats.miss();
                None
            }
        }
    }
    
    /// Insert or update chunk metadata in cache
    pub fn insert_chunk(&self, chunk: ChunkMetadata) {
        let chunk_id = chunk.chunk_id;
        
        // Check if we need to evict old entries
        if self.chunks.len() >= self.max_size / 2 {
            self.evict_old_chunks();
        }
        
        if self.chunks.contains_key(&chunk_id) {
            self.stats.update();
            debug!("Updated chunk {} in cache", chunk_id);
        } else {
            self.stats.insert();
            debug!("Inserted chunk {} in cache", chunk_id);
        }
        
        self.chunks.insert(chunk_id, CacheEntry::new(chunk));
    }
    
    /// Remove a chunk from cache
    pub fn remove_chunk(&self, chunk_id: ChunkId) {
        if self.chunks.remove(&chunk_id).is_some() {
            debug!("Removed chunk {} from cache", chunk_id);
            self.stats.evict();
        }
    }
    
    /// Evict old chunks based on TTL
    fn evict_old_chunks(&self) {
        let mut to_remove = Vec::new();
        
        for entry in self.chunks.iter() {
            if entry.value().is_expired(self.ttl) {
                to_remove.push(*entry.key());
            }
        }
        
        for key in to_remove {
            self.chunks.remove(&key);
            self.stats.evict();
        }
    }
    
    // Storage class cache operations
    
    /// Get storage class from cache
    pub fn get_storage_class(&self, id: u8) -> Option<StorageClassDef> {
        match self.storage_classes.get(&id) {
            Some(entry) => {
                if entry.is_expired(self.ttl) {
                    trace!("Storage class {} expired in cache", id);
                    self.storage_classes.remove(&id);
                    self.stats.evict();
                    self.stats.miss();
                    None
                } else {
                    trace!("Cache hit for storage class {}", id);
                    self.stats.hit();
                    Some(entry.value.clone())
                }
            }
            None => {
                trace!("Cache miss for storage class {}", id);
                self.stats.miss();
                None
            }
        }
    }
    
    /// Insert or update storage class in cache
    pub fn insert_storage_class(&self, sc: StorageClassDef) {
        let id = sc.id;
        
        if self.storage_classes.contains_key(&id) {
            self.stats.update();
            debug!("Updated storage class {} in cache", id);
        } else {
            self.stats.insert();
            debug!("Inserted storage class {} in cache", id);
        }
        
        self.storage_classes.insert(id, CacheEntry::new(sc));
    }
    
    // Path to inode cache operations
    
    /// Get inode ID for a path from cache
    pub fn get_path_inode(&self, path: &str) -> Option<InodeId> {
        match self.path_to_inode.get(path) {
            Some(entry) => {
                if entry.is_expired(self.ttl) {
                    trace!("Path {} expired in cache", path);
                    self.path_to_inode.remove(path);
                    self.stats.evict();
                    self.stats.miss();
                    None
                } else {
                    trace!("Cache hit for path {}", path);
                    self.stats.hit();
                    Some(entry.value)
                }
            }
            None => {
                trace!("Cache miss for path {}", path);
                self.stats.miss();
                None
            }
        }
    }
    
    /// Cache path to inode mapping
    pub fn insert_path_inode(&self, path: String, inode: InodeId) {
        // Check if we need to evict old entries
        if self.path_to_inode.len() >= self.max_size / 4 {
            self.evict_old_paths();
        }
        
        if self.path_to_inode.contains_key(&path) {
            self.stats.update();
            debug!("Updated path {} -> inode {} in cache", path, inode);
        } else {
            self.stats.insert();
            debug!("Inserted path {} -> inode {} in cache", path, inode);
        }
        
        self.path_to_inode.insert(path, CacheEntry::new(inode));
    }
    
    /// Remove a path from cache
    pub fn remove_path(&self, path: &str) {
        if self.path_to_inode.remove(path).is_some() {
            debug!("Removed path {} from cache", path);
            self.stats.evict();
        }
    }
    
    /// Evict old paths based on TTL
    fn evict_old_paths(&self) {
        let mut to_remove = Vec::new();
        
        for entry in self.path_to_inode.iter() {
            if entry.value().is_expired(self.ttl) {
                to_remove.push(entry.key().clone());
            }
        }
        
        for key in to_remove {
            self.path_to_inode.remove(&key);
            self.stats.evict();
        }
    }
    
    /// Clear all caches
    pub fn clear(&self) {
        let evicted = self.inodes.len() + self.chunks.len() + 
                     self.storage_classes.len() + self.path_to_inode.len();
        
        self.inodes.clear();
        self.chunks.clear();
        self.storage_classes.clear();
        self.path_to_inode.clear();
        
        for _ in 0..evicted {
            self.stats.evict();
        }
        
        debug!("Cleared all caches ({} entries evicted)", evicted);
    }
    
    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }
    
    /// Get current cache sizes
    pub fn sizes(&self) -> (usize, usize, usize, usize) {
        (
            self.inodes.len(),
            self.chunks.len(),
            self.storage_classes.len(),
            self.path_to_inode.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mooseng_common::types::{FsNodeType, now_micros};
    
    #[test]
    fn test_cache_basic_operations() {
        let cache = MetadataCache::new(Duration::from_secs(60), 1000);
        
        // Test inode caching
        let node = FsNode {
            inode: 42,
            parent: Some(1),
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
        
        // Insert
        cache.insert_inode(node.clone());
        let (hits, misses, inserts, _, _) = cache.stats().get_stats();
        assert_eq!(inserts, 1);
        
        // Get (hit)
        let retrieved = cache.get_inode(42).unwrap();
        assert_eq!(retrieved.inode, 42);
        let (hits, misses, _, _, _) = cache.stats().get_stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 0);
        
        // Get non-existent (miss)
        assert!(cache.get_inode(999).is_none());
        let (_, misses, _, _, _) = cache.stats().get_stats();
        assert_eq!(misses, 1);
        
        // Remove
        cache.remove_inode(42);
        assert!(cache.get_inode(42).is_none());
    }
    
    #[test]
    fn test_cache_expiration() {
        let cache = MetadataCache::new(Duration::from_millis(100), 1000);
        
        let node = FsNode {
            inode: 42,
            parent: None,
            ctime: now_micros(),
            mtime: now_micros(),
            atime: now_micros(),
            uid: 1000,
            gid: 1000,
            mode: 0o755,
            flags: 0,
            winattr: 0,
            storage_class_id: 1,
            trash_retention: 7,
            node_type: FsNodeType::Directory {
                children: vec![],
                stats: Default::default(),
                quota: None,
            },
        };
        
        cache.insert_inode(node);
        
        // Should be available immediately
        assert!(cache.get_inode(42).is_some());
        
        // Wait for expiration
        std::thread::sleep(Duration::from_millis(150));
        
        // Should be expired
        assert!(cache.get_inode(42).is_none());
        let (_, _, _, _, evictions) = cache.stats().get_stats();
        assert_eq!(evictions, 1);
    }
    
    #[test]
    fn test_path_cache() {
        let cache = MetadataCache::new(Duration::from_secs(60), 1000);
        
        // Insert path mapping
        cache.insert_path_inode("/test/file.txt".to_string(), 42);
        
        // Get path (hit)
        assert_eq!(cache.get_path_inode("/test/file.txt"), Some(42));
        
        // Get non-existent path (miss)
        assert!(cache.get_path_inode("/other/file.txt").is_none());
        
        // Remove path
        cache.remove_path("/test/file.txt");
        assert!(cache.get_path_inode("/test/file.txt").is_none());
    }
    
    #[test]
    fn test_cache_stats() {
        let cache = MetadataCache::new(Duration::from_secs(60), 1000);
        
        // Initial state
        assert_eq!(cache.stats().hit_rate(), 0.0);
        
        // Generate some activity
        for i in 0..10 {
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
        
        // Generate hits
        for i in 0..5 {
            cache.get_inode(i);
        }
        
        // Generate misses
        for i in 10..15 {
            cache.get_inode(i);
        }
        
        let hit_rate = cache.stats().hit_rate();
        assert_eq!(hit_rate, 0.5); // 5 hits, 5 misses
    }
}