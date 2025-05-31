use moka::future::Cache;
use mooseng_common::types::{FileAttr, InodeId, FsNode};
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use bytes::Bytes;

/// Cache key for file data blocks
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DataCacheKey {
    pub inode: InodeId,
    pub block_offset: u64,
}

/// Cached directory entry
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub inode: InodeId,
    pub file_type: mooseng_common::types::FileType,
}

/// Directory listing cache entry
#[derive(Clone, Debug)]
pub struct DirListing {
    pub entries: Vec<DirEntry>,
    pub cached_at: std::time::Instant,
}

/// Client-side cache for filesystem metadata and data
#[derive(Clone)]
pub struct ClientCache {
    /// File attribute cache
    attr_cache: Cache<InodeId, FileAttr>,
    
    /// Directory listing cache
    dir_cache: Cache<InodeId, DirListing>,
    
    /// File data cache (block-based)
    data_cache: Cache<DataCacheKey, Bytes>,
    
    /// Negative cache for non-existent files
    negative_cache: Cache<(InodeId, String), ()>,
    
    /// Open file handles cache
    open_files: Arc<DashMap<u64, Arc<FsNode>>>,
    
    /// Next file handle ID
    next_fh: Arc<std::sync::atomic::AtomicU64>,
}

impl ClientCache {
    pub fn new(
        attr_cache_size: usize,
        attr_ttl: Duration,
        dir_cache_size: usize,
        dir_ttl: Duration,
        data_cache_size: u64,
        data_ttl: Duration,
        enable_negative_cache: bool,
    ) -> Self {
        let attr_cache = Cache::builder()
            .max_capacity(attr_cache_size as u64)
            .time_to_live(attr_ttl)
            .build();
            
        let dir_cache = Cache::builder()
            .max_capacity(dir_cache_size as u64)
            .time_to_live(dir_ttl)
            .build();
            
        let data_cache = Cache::builder()
            .weigher(|_key: &DataCacheKey, value: &Bytes| -> u32 {
                value.len() as u32
            })
            .max_capacity(data_cache_size)
            .time_to_live(data_ttl)
            .build();
            
        let negative_cache = if enable_negative_cache {
            Cache::builder()
                .max_capacity(1000)
                .time_to_live(Duration::from_secs(5))
                .build()
        } else {
            Cache::builder().max_capacity(0).build()
        };
        
        Self {
            attr_cache,
            dir_cache,
            data_cache,
            negative_cache,
            open_files: Arc::new(DashMap::new()),
            next_fh: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }
    
    /// Get file attributes from cache
    pub async fn get_attr(&self, inode: InodeId) -> Option<FileAttr> {
        self.attr_cache.get(&inode).await
    }
    
    /// Cache file attributes
    pub async fn put_attr(&self, inode: InodeId, attr: FileAttr) {
        self.attr_cache.insert(inode, attr).await;
    }
    
    /// Invalidate cached attributes
    pub async fn invalidate_attr(&self, inode: InodeId) {
        self.attr_cache.invalidate(&inode).await;
    }
    
    /// Get directory listing from cache
    pub async fn get_dir(&self, inode: InodeId) -> Option<DirListing> {
        self.dir_cache.get(&inode).await
    }
    
    /// Cache directory listing
    pub async fn put_dir(&self, inode: InodeId, listing: DirListing) {
        self.dir_cache.insert(inode, listing).await;
    }
    
    /// Invalidate cached directory listing
    pub async fn invalidate_dir(&self, inode: InodeId) {
        self.dir_cache.invalidate(&inode).await;
    }
    
    /// Get data block from cache
    pub async fn get_data(&self, inode: InodeId, block_offset: u64) -> Option<Bytes> {
        let key = DataCacheKey { inode, block_offset };
        self.data_cache.get(&key).await
    }
    
    /// Cache data block
    pub async fn put_data(&self, inode: InodeId, block_offset: u64, data: Bytes) {
        let key = DataCacheKey { inode, block_offset };
        self.data_cache.insert(key, data).await;
    }
    
    /// Invalidate cached data for inode
    pub async fn invalidate_data(&self, _inode: InodeId) {
        // Note: moka doesn't have range invalidation, so we'd need to track keys
        // For now, we'll just let the TTL handle it
        // TODO: Implement a key tracking mechanism for range invalidation
    }
    
    /// Check negative cache
    pub async fn is_negative_cached(&self, parent: InodeId, name: &str) -> bool {
        self.negative_cache.get(&(parent, name.to_string())).await.is_some()
    }
    
    /// Add to negative cache
    pub async fn add_negative(&self, parent: InodeId, name: String) {
        self.negative_cache.insert((parent, name), ()).await;
    }
    
    /// Remove from negative cache
    pub async fn remove_negative(&self, parent: InodeId, name: &str) {
        self.negative_cache.invalidate(&(parent, name.to_string())).await;
    }
    
    /// Add an open file handle
    pub fn add_open_file(&self, node: Arc<FsNode>) -> u64 {
        let fh = self.next_fh.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.open_files.insert(fh, node);
        fh
    }
    
    /// Get open file handle
    pub fn get_open_file(&self, fh: u64) -> Option<Arc<FsNode>> {
        self.open_files.get(&fh).map(|entry| entry.clone())
    }
    
    /// Remove open file handle
    pub fn remove_open_file(&self, fh: u64) -> Option<Arc<FsNode>> {
        self.open_files.remove(&fh).map(|(_, node)| node)
    }
    
    /// Clear all caches
    pub async fn clear_all(&self) {
        self.attr_cache.invalidate_all();
        self.dir_cache.invalidate_all();
        self.data_cache.invalidate_all();
        self.negative_cache.invalidate_all();
        self.open_files.clear();
    }
    
    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        CacheStats {
            attr_cache_size: self.attr_cache.entry_count(),
            dir_cache_size: self.dir_cache.entry_count(),
            data_cache_size: self.data_cache.entry_count(),
            data_cache_bytes: self.data_cache.weighted_size(),
            negative_cache_size: self.negative_cache.entry_count(),
            open_files_count: self.open_files.len() as u64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub attr_cache_size: u64,
    pub dir_cache_size: u64,
    pub data_cache_size: u64,
    pub data_cache_bytes: u64,
    pub negative_cache_size: u64,
    pub open_files_count: u64,
}