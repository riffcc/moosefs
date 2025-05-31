//! LRU cache implementation for hot chunks
//! 
//! This module provides an in-memory cache to keep frequently accessed
//! chunks for faster retrieval, reducing disk I/O operations.

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use lru::LruCache;
use std::num::NonZeroUsize;
use crate::chunk::Chunk;
use crate::error::{ChunkServerError, Result};
use mooseng_common::types::{ChunkId, ChunkVersion};
use tracing::{debug, info, warn};

/// Cache entry containing chunk data and metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    chunk: Chunk,
    inserted_at: Instant,
    last_accessed: Instant,
    access_count: u64,
}

impl CacheEntry {
    fn new(chunk: Chunk) -> Self {
        let now = Instant::now();
        Self {
            chunk,
            inserted_at: now,
            last_accessed: now,
            access_count: 1,
        }
    }
    
    fn access(&mut self) -> &Chunk {
        self.last_accessed = Instant::now();
        self.access_count += 1;
        &self.chunk
    }
    
    fn age(&self) -> std::time::Duration {
        self.last_accessed.elapsed()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub current_size: usize,
    pub max_size: usize,
    pub memory_usage_bytes: u64,
}

impl CacheStats {
    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
    
    pub fn memory_usage_mb(&self) -> f64 {
        self.memory_usage_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// Configuration for the chunk cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of chunks to cache
    pub max_entries: usize,
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    /// Maximum age for cache entries (in seconds)
    pub max_age_seconds: u64,
    /// Enable cache metrics collection
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            max_age_seconds: 3600, // 1 hour
            enable_metrics: true,
        }
    }
}

/// Thread-safe LRU cache for chunks
pub struct ChunkCache {
    cache: Arc<RwLock<LruCache<(ChunkId, ChunkVersion), CacheEntry>>>,
    config: CacheConfig,
    stats: Arc<RwLock<CacheStats>>,
}

impl ChunkCache {
    /// Create a new chunk cache
    pub fn new(config: CacheConfig) -> Result<Self> {
        let max_entries = NonZeroUsize::new(config.max_entries)
            .ok_or_else(|| ChunkServerError::CacheError { 
                message: "Cache max_entries must be greater than 0".to_string() 
            })?;
        
        let cache = Arc::new(RwLock::new(LruCache::new(max_entries)));
        let mut stats = CacheStats::default();
        stats.max_size = config.max_entries;
        
        info!("Created chunk cache with max_entries={}, max_memory={}MB", 
              config.max_entries, config.max_memory_bytes / (1024 * 1024));
        
        Ok(Self {
            cache,
            config,
            stats: Arc::new(RwLock::new(stats)),
        })
    }
    
    /// Store a chunk in the cache
    pub async fn put(&self, chunk: Chunk) -> Result<()> {
        let key = (chunk.id(), chunk.version());
        let chunk_size = chunk.size() as u64;
        
        // Check if adding this chunk would exceed memory limit
        if chunk_size > self.config.max_memory_bytes {
            warn!("Chunk {} v{} size ({} bytes) exceeds cache memory limit", 
                  chunk.id(), chunk.version(), chunk_size);
            return Ok(()); // Skip caching large chunks
        }
        
        let entry = CacheEntry::new(chunk);
        
        // Update cache
        let mut cache = self.cache.write().await;
        let evicted = cache.put(key, entry);
        
        // Update statistics
        if self.config.enable_metrics {
            let mut stats = self.stats.write().await;
            stats.current_size = cache.len();
            stats.memory_usage_bytes += chunk_size;
            
            if evicted.is_some() {
                stats.evictions += 1;
                // Subtract memory of evicted chunk
                if let Some(evicted_entry) = evicted {
                    stats.memory_usage_bytes = stats.memory_usage_bytes
                        .saturating_sub(evicted_entry.chunk.size() as u64);
                }
            }
        }
        
        debug!("Cached chunk {} v{} ({} bytes)", key.0, key.1, chunk_size);
        Ok(())
    }
    
    /// Retrieve a chunk from the cache
    pub async fn get(&self, chunk_id: ChunkId, version: ChunkVersion) -> Option<Chunk> {
        let key = (chunk_id, version);
        
        let mut cache = self.cache.write().await;
        let result = cache.get_mut(&key).map(|entry| entry.access().clone());
        
        // Update statistics
        if self.config.enable_metrics {
            let mut stats = self.stats.write().await;
            if result.is_some() {
                stats.hits += 1;
                debug!("Cache hit for chunk {} v{}", chunk_id, version);
            } else {
                stats.misses += 1;
                debug!("Cache miss for chunk {} v{}", chunk_id, version);
            }
        }
        
        result
    }
    
    /// Remove a chunk from the cache
    pub async fn remove(&self, chunk_id: ChunkId, version: ChunkVersion) -> Option<Chunk> {
        let key = (chunk_id, version);
        
        let mut cache = self.cache.write().await;
        let removed = cache.pop(&key);
        
        if let Some(ref entry) = removed {
            // Update statistics
            if self.config.enable_metrics {
                let mut stats = self.stats.write().await;
                stats.current_size = cache.len();
                stats.memory_usage_bytes = stats.memory_usage_bytes
                    .saturating_sub(entry.chunk.size() as u64);
            }
            
            debug!("Removed chunk {} v{} from cache", chunk_id, version);
        }
        
        removed.map(|entry| entry.chunk)
    }
    
    /// Check if a chunk is in the cache
    pub async fn contains(&self, chunk_id: ChunkId, version: ChunkVersion) -> bool {
        let key = (chunk_id, version);
        let cache = self.cache.read().await;
        cache.contains(&key)
    }
    
    /// Clear all entries from the cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        
        if self.config.enable_metrics {
            let mut stats = self.stats.write().await;
            stats.current_size = 0;
            stats.memory_usage_bytes = 0;
        }
        
        info!("Cleared all entries from cache");
    }
    
    /// Get current cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        if self.config.enable_metrics {
            let stats = self.stats.read().await;
            stats.clone()
        } else {
            CacheStats::default()
        }
    }
    
    /// Perform cache maintenance (remove old entries)
    pub async fn maintenance(&self) {
        let mut to_remove = Vec::new();
        let max_age = std::time::Duration::from_secs(self.config.max_age_seconds);
        
        // Identify old entries
        {
            let cache = self.cache.read().await;
            for (key, entry) in cache.iter() {
                if entry.age() > max_age {
                    to_remove.push(*key);
                }
            }
        }
        
        // Remove old entries
        if !to_remove.is_empty() {
            let mut cache = self.cache.write().await;
            let mut removed_size = 0u64;
            
            for key in &to_remove {
                if let Some(entry) = cache.pop(key) {
                    removed_size += entry.chunk.size() as u64;
                }
            }
            
            // Update statistics
            if self.config.enable_metrics {
                let mut stats = self.stats.write().await;
                stats.current_size = cache.len();
                stats.memory_usage_bytes = stats.memory_usage_bytes.saturating_sub(removed_size);
                stats.evictions += to_remove.len() as u64;
            }
            
            info!("Removed {} old cache entries ({}MB freed)", 
                  to_remove.len(), removed_size / (1024 * 1024));
        }
    }
    
    /// Check if cache should evict entries due to memory pressure
    pub async fn check_memory_pressure(&self) -> bool {
        if self.config.enable_metrics {
            let stats = self.stats.read().await;
            stats.memory_usage_bytes > self.config.max_memory_bytes
        } else {
            false
        }
    }
    
    /// Evict entries due to memory pressure
    pub async fn evict_for_memory_pressure(&self) {
        if !self.check_memory_pressure().await {
            return;
        }
        
        let target_memory = (self.config.max_memory_bytes as f64 * 0.8) as u64; // 80% of max
        let mut cache = self.cache.write().await;
        let mut freed_memory = 0u64;
        let mut evicted_count = 0u64;
        
        // Evict LRU entries until we're under the target memory usage
        while freed_memory < (self.config.max_memory_bytes - target_memory) && !cache.is_empty() {
            if let Some((_, entry)) = cache.pop_lru() {
                freed_memory += entry.chunk.size() as u64;
                evicted_count += 1;
            } else {
                break;
            }
        }
        
        // Update statistics
        if self.config.enable_metrics {
            let mut stats = self.stats.write().await;
            stats.current_size = cache.len();
            stats.memory_usage_bytes = stats.memory_usage_bytes.saturating_sub(freed_memory);
            stats.evictions += evicted_count;
        }
        
        if evicted_count > 0 {
            info!("Evicted {} cache entries due to memory pressure ({}MB freed)", 
                  evicted_count, freed_memory / (1024 * 1024));
        }
    }
    
    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
    
    /// Get current cache size
    pub async fn len(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
    
    /// Check if cache is empty
    pub async fn is_empty(&self) -> bool {
        let cache = self.cache.read().await;
        cache.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{ChecksumType};
    use bytes::Bytes;
    use tokio::time::{sleep, Duration};
    
    fn create_test_chunk(chunk_id: ChunkId, version: ChunkVersion, data: &str) -> Chunk {
        Chunk::new(
            chunk_id,
            version,
            Bytes::from(data.to_string()),
            ChecksumType::Blake3,
            1,
        )
    }
    
    #[tokio::test]
    async fn test_cache_put_and_get() {
        let config = CacheConfig::default();
        let cache = ChunkCache::new(config).unwrap();
        
        let chunk = create_test_chunk(123, 1, "Hello, World!");
        let chunk_id = chunk.id();
        let version = chunk.version();
        
        // Initially should be empty
        assert!(cache.get(chunk_id, version).await.is_none());
        
        // Put chunk in cache
        cache.put(chunk.clone()).await.unwrap();
        
        // Should be able to retrieve it
        let cached_chunk = cache.get(chunk_id, version).await.unwrap();
        assert_eq!(cached_chunk.id(), chunk_id);
        assert_eq!(cached_chunk.version(), version);
        assert_eq!(cached_chunk.data(), chunk.data());
    }
    
    #[tokio::test]
    async fn test_cache_contains() {
        let config = CacheConfig::default();
        let cache = ChunkCache::new(config).unwrap();
        
        let chunk = create_test_chunk(123, 1, "test data");
        let chunk_id = chunk.id();
        let version = chunk.version();
        
        assert!(!cache.contains(chunk_id, version).await);
        
        cache.put(chunk).await.unwrap();
        assert!(cache.contains(chunk_id, version).await);
    }
    
    #[tokio::test]
    async fn test_cache_remove() {
        let config = CacheConfig::default();
        let cache = ChunkCache::new(config).unwrap();
        
        let chunk = create_test_chunk(123, 1, "test data");
        let chunk_id = chunk.id();
        let version = chunk.version();
        
        cache.put(chunk.clone()).await.unwrap();
        assert!(cache.contains(chunk_id, version).await);
        
        let removed = cache.remove(chunk_id, version).await.unwrap();
        assert_eq!(removed.id(), chunk_id);
        assert!(!cache.contains(chunk_id, version).await);
    }
    
    #[tokio::test]
    async fn test_cache_clear() {
        let config = CacheConfig::default();
        let cache = ChunkCache::new(config).unwrap();
        
        // Add multiple chunks
        for i in 1..=5 {
            let chunk = create_test_chunk(i, 1, &format!("data{}", i));
            cache.put(chunk).await.unwrap();
        }
        
        assert_eq!(cache.len().await, 5);
        
        cache.clear().await;
        assert_eq!(cache.len().await, 0);
        assert!(cache.is_empty().await);
    }
    
    #[tokio::test]
    async fn test_cache_lru_eviction() {
        let mut config = CacheConfig::default();
        config.max_entries = 3; // Small cache for testing
        
        let cache = ChunkCache::new(config).unwrap();
        
        // Fill cache to capacity
        for i in 1..=3 {
            let chunk = create_test_chunk(i, 1, &format!("data{}", i));
            cache.put(chunk).await.unwrap();
        }
        
        assert_eq!(cache.len().await, 3);
        
        // Add one more chunk, should evict the LRU
        let chunk = create_test_chunk(4, 1, "data4");
        cache.put(chunk).await.unwrap();
        
        assert_eq!(cache.len().await, 3);
        
        // First chunk should be evicted
        assert!(!cache.contains(1, 1).await);
        assert!(cache.contains(4, 1).await);
    }
    
    #[tokio::test]
    async fn test_cache_statistics() {
        let config = CacheConfig::default();
        let cache = ChunkCache::new(config).unwrap();
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        
        let chunk = create_test_chunk(123, 1, "test data");
        
        // Miss
        assert!(cache.get(123, 1).await.is_none());
        let stats = cache.get_stats().await;
        assert_eq!(stats.misses, 1);
        
        // Put and hit
        cache.put(chunk).await.unwrap();
        assert!(cache.get(123, 1).await.is_some());
        
        let stats = cache.get_stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_ratio(), 0.5);
    }
    
    #[tokio::test]
    async fn test_cache_memory_limit() {
        let mut config = CacheConfig::default();
        config.max_memory_bytes = 100; // Very small limit
        
        let cache = ChunkCache::new(config).unwrap();
        
        // Try to cache a large chunk
        let large_data = "x".repeat(200); // Larger than limit
        let chunk = create_test_chunk(123, 1, &large_data);
        
        cache.put(chunk).await.unwrap();
        
        // Should not be cached due to size limit
        let stats = cache.get_stats().await;
        assert_eq!(stats.current_size, 0);
    }
    
    #[tokio::test]
    async fn test_cache_maintenance() {
        let mut config = CacheConfig::default();
        config.max_age_seconds = 1; // Very short max age
        
        let cache = ChunkCache::new(config).unwrap();
        
        let chunk = create_test_chunk(123, 1, "test data");
        cache.put(chunk).await.unwrap();
        
        assert_eq!(cache.len().await, 1);
        
        // Wait for chunk to age out
        sleep(Duration::from_secs(2)).await;
        
        cache.maintenance().await;
        assert_eq!(cache.len().await, 0);
    }
}