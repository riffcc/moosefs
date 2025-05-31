# Enhanced Metadata Cache Developer Guide

## Overview

The Enhanced Metadata Cache (`cache_enhanced.rs`) provides a high-performance, feature-rich caching system for MooseNG filesystem metadata. It includes LRU eviction, cache warming/prefetching, distributed invalidation, and comprehensive monitoring capabilities.

## Key Features

- **LRU Eviction Policy**: Automatically evicts least recently used entries with hot entry protection
- **Cache Warming**: Proactive prefetching of frequently accessed metadata  
- **Distributed Invalidation**: Pub/sub-based cache invalidation across cluster nodes
- **Performance Monitoring**: Detailed statistics and metrics collection
- **Background Maintenance**: Automated cleanup of expired entries
- **Thread-Safe**: Designed for high-concurrency environments

## Core Components

### EnhancedMetadataCache

The main cache structure that stores filesystem metadata with intelligent management.

```rust
use mooseng_master::cache_enhanced::EnhancedMetadataCache;
use std::time::Duration;
use std::sync::Arc;

// Create a new cache instance
let cache = Arc::new(EnhancedMetadataCache::new(
    Duration::from_secs(300),  // 5 minute TTL
    100_000,                   // Max 100k entries
    5,                         // Hot threshold (5 accesses)
));
```

### Basic Cache Operations

#### Inserting Inodes

```rust
use mooseng_common::types::{FsNode, FsNodeType, now_micros};

let node = FsNode {
    inode: 12345,
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

cache.insert_inode(node);
```

#### Retrieving Inodes

```rust
// Get an inode from cache
if let Some(node) = cache.get_inode(12345) {
    println!("Found inode: {}", node.inode);
} else {
    println!("Cache miss for inode 12345");
}
```

### Cache Invalidation

The cache supports distributed invalidation through a pub/sub mechanism:

```rust
use mooseng_master::cache_enhanced::CacheInvalidation;

// Subscribe to invalidation events
let mut invalidation_rx = cache.subscribe_invalidations();

// Invalidate specific entries
cache.invalidate(CacheInvalidation::InvalidateInode(12345)).await;
cache.invalidate(CacheInvalidation::InvalidatePath("/some/path".to_string())).await;
cache.invalidate(CacheInvalidation::InvalidateAll).await;

// Handle invalidation events
tokio::spawn(async move {
    while let Ok(msg) = invalidation_rx.recv().await {
        match msg {
            CacheInvalidation::InvalidateInode(inode) => {
                println!("Inode {} invalidated", inode);
            }
            CacheInvalidation::InvalidateAll => {
                println!("All cache entries invalidated");
            }
            _ => {}
        }
    }
});
```

### Cache Warming and Prefetching

Proactively load frequently accessed data:

```rust
use mooseng_master::cache_enhanced::{PrefetchHint, CacheWarmer};

// Create prefetch hint
let hint = PrefetchHint {
    inodes: vec![1, 2, 3, 4, 5],
    chunks: vec![],
    paths: vec!["/hot/path".to_string()],
};

// Request prefetching
cache.request_prefetch(hint).await;

// Set up cache warmer with custom fetch function
let fetch_fn = |hint: PrefetchHint| -> Vec<FsNode> {
    // Your logic to fetch data from storage
    hint.inodes.into_iter().map(|inode| {
        // Fetch FsNode for this inode from your data store
        fetch_node_from_storage(inode)
    }).collect()
};

let warmer = CacheWarmer::new(cache.clone(), fetch_fn);
let (tx, rx) = tokio::sync::mpsc::channel(100);
warmer.start(rx).await;
```

### Background Maintenance

Start automatic background tasks for cache maintenance:

```rust
// Start background maintenance (TTL cleanup and stats reporting)
cache.clone().start_maintenance();
```

This automatically starts:
- **TTL Cleanup**: Runs every 60 seconds to remove expired entries
- **Stats Reporting**: Logs cache statistics every 5 minutes

### Statistics and Monitoring

Monitor cache performance with comprehensive statistics:

```rust
// Get current statistics
let stats = cache.stats().get_stats();
println!("Cache hits: {}", stats["hits"]);
println!("Cache misses: {}", stats["misses"]);
println!("Hit rate: {:.2}%", cache.stats().hit_rate() * 100.0);

// Check hot entries
println!("Hot entries: {}", stats["hot_entries"]);
println!("Total evictions: {}", stats["evictions"]);
```

### LRU Eviction and Hot Entry Protection

The cache automatically manages memory usage through LRU eviction:

```rust
// Entries accessed frequently (>= hot_threshold) are protected from eviction
for _ in 0..10 {
    cache.get_inode(important_inode); // Makes it "hot"
}

// When cache reaches capacity, it evicts 5% of LRU entries
// Hot entries are moved to front instead of being evicted
```

## Advanced Usage Patterns

### Multi-Tenant Cache Setup

```rust
use std::collections::HashMap;

struct MultiTenantCache {
    caches: HashMap<String, Arc<EnhancedMetadataCache>>,
}

impl MultiTenantCache {
    pub fn new() -> Self {
        Self {
            caches: HashMap::new(),
        }
    }
    
    pub fn get_cache(&mut self, tenant: &str) -> Arc<EnhancedMetadataCache> {
        self.caches.entry(tenant.to_string()).or_insert_with(|| {
            Arc::new(EnhancedMetadataCache::new(
                Duration::from_secs(300),
                50_000,  // Smaller per-tenant cache
                3,
            ))
        }).clone()
    }
}
```

### Custom Cache Policies

```rust
// Configure cache behavior based on workload
let high_throughput_cache = Arc::new(EnhancedMetadataCache::new(
    Duration::from_secs(60),   // Shorter TTL for frequently changing data
    1_000_000,                 // Large cache for high throughput
    10,                        // Higher hot threshold
));

let low_latency_cache = Arc::new(EnhancedMetadataCache::new(
    Duration::from_secs(600),  // Longer TTL for stable data
    10_000,                    // Smaller cache for predictable memory usage
    2,                         // Lower hot threshold
));
```

### Integration with Metadata Operations

```rust
use mooseng_master::cache_enhanced::EnhancedMetadataCache;

pub struct MetadataManager {
    cache: Arc<EnhancedMetadataCache>,
    storage: Arc<dyn MetadataStorage>,
}

impl MetadataManager {
    pub async fn get_inode(&self, inode: InodeId) -> Result<FsNode, Error> {
        // Try cache first
        if let Some(node) = self.cache.get_inode(inode) {
            return Ok(node);
        }
        
        // Cache miss - fetch from storage
        let node = self.storage.get_inode(inode).await?;
        
        // Cache the result
        self.cache.insert_inode(node.clone());
        
        Ok(node)
    }
    
    pub async fn update_inode(&self, node: FsNode) -> Result<(), Error> {
        // Update storage
        self.storage.update_inode(&node).await?;
        
        // Update cache
        self.cache.insert_inode(node.clone());
        
        // Invalidate across cluster
        self.cache.invalidate(CacheInvalidation::InvalidateInode(node.inode)).await;
        
        Ok(())
    }
}
```

## Performance Considerations

### Memory Usage

- Each cache entry has overhead (~200 bytes + data size)
- Monitor `cache_utilization` to avoid memory pressure
- Consider enabling compression for large entries (future feature)

### Concurrency

- All operations are thread-safe using `DashMap` and `RwLock`
- High read concurrency with minimal lock contention
- Write operations may briefly block readers during LRU updates

### Tuning Parameters

```rust
// For read-heavy workloads
let read_optimized = EnhancedMetadataCache::new(
    Duration::from_secs(900),  // Long TTL
    200_000,                   // Large cache
    3,                         // Low hot threshold
);

// For write-heavy workloads  
let write_optimized = EnhancedMetadataCache::new(
    Duration::from_secs(60),   // Short TTL
    50_000,                    // Moderate cache
    8,                         // High hot threshold
);
```

## Error Handling

The cache is designed to be resilient:

```rust
// Cache operations never fail - they degrade gracefully
let result = cache.get_inode(12345); // Always returns Option, never panics

// Background tasks handle errors internally and log them
cache.clone().start_maintenance(); // Safe to call multiple times
```

## Best Practices

1. **Initialize Early**: Create cache instances during application startup
2. **Use Arc**: Always wrap cache in `Arc` for sharing across threads
3. **Monitor Stats**: Regularly check hit rates and adjust configuration
4. **Handle Invalidation**: Subscribe to invalidation events in distributed setups
5. **Tune for Workload**: Adjust TTL, size, and hot threshold based on access patterns
6. **Start Maintenance**: Always call `start_maintenance()` for production deployments

## Integration Testing

The cache includes comprehensive integration tests in `tests/cache_integration.rs`:

```bash
# Run cache-specific tests
cargo test cache_integration

# Run with output to see performance stats
cargo test cache_integration -- --nocapture
```

## Troubleshooting

### Low Hit Rate
- Increase cache size or TTL
- Check if data is changing too frequently
- Monitor access patterns

### High Memory Usage
- Reduce cache size
- Lower TTL for faster cleanup
- Check for memory leaks in custom fetch functions

### Poor Concurrency
- Verify using `Arc<EnhancedMetadataCache>`
- Check for blocking operations in cache access paths
- Monitor lock contention in profiling tools

## Future Enhancements

- Compression support for large entries
- Tiered storage integration
- Metrics export to Prometheus
- Custom eviction policies
- Write-through caching mode