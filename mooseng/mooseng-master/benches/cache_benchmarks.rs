use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mooseng_master::cache_configuration::CacheConfig;
use mooseng_master::cache_enhanced::{EnhancedMetadataCache, CacheInvalidation, PrefetchHint};
use mooseng_master::cached_metadata::CachedMetadataManager;
use mooseng_master::metadata::MetadataStore;
use mooseng_common::types::{FsNode, FsNodeType, InodeId, now_micros};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create a test node for benchmarking
fn create_test_node(inode: InodeId) -> FsNode {
    FsNode {
        inode,
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
            chunk_ids: vec![inode],
            session_id: None,
        },
    }
}

/// Benchmark basic cache operations
fn bench_cache_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_operations");
    
    // Test different cache sizes
    for cache_size in [1_000, 10_000, 100_000].iter() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(300),
            *cache_size,
            5,
        ));
        
        // Pre-populate cache
        for i in 1..=(*cache_size / 2) {
            let node = create_test_node(i as InodeId);
            cache.insert_inode(node);
        }
        
        group.throughput(Throughput::Elements(1));
        
        // Benchmark cache hits
        group.bench_with_input(
            BenchmarkId::new("cache_hit", cache_size),
            cache_size,
            |b, _| {
                b.iter(|| {
                    let inode = (fastrand::u64(1..=(*cache_size / 2) as u64)) as InodeId;
                    black_box(cache.get_inode(inode))
                })
            },
        );
        
        // Benchmark cache misses
        group.bench_with_input(
            BenchmarkId::new("cache_miss", cache_size),
            cache_size,
            |b, _| {
                b.iter(|| {
                    let inode = (fastrand::u64((*cache_size / 2 + 1) as u64..=*cache_size as u64)) as InodeId;
                    black_box(cache.get_inode(inode))
                })
            },
        );
        
        // Benchmark cache inserts
        group.bench_with_input(
            BenchmarkId::new("cache_insert", cache_size),
            cache_size,
            |b, _| {
                let mut counter = *cache_size as InodeId + 1;
                b.iter(|| {
                    let node = create_test_node(counter);
                    counter += 1;
                    black_box(cache.insert_inode(node))
                })
            },
        );
        
        // Benchmark cache invalidation
        group.bench_with_input(
            BenchmarkId::new("cache_invalidation", cache_size),
            cache_size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let inode = (fastrand::u64(1..=(*cache_size / 2) as u64)) as InodeId;
                    black_box(cache.invalidate(CacheInvalidation::InvalidateInode(inode)).await)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent cache access
fn bench_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_access");
    
    // Test different concurrency levels
    for concurrency in [1, 4, 8, 16, 32].iter() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(300),
            100_000,
            5,
        ));
        
        // Pre-populate cache
        for i in 1..=50_000 {
            let node = create_test_node(i);
            cache.insert_inode(node);
        }
        
        group.throughput(Throughput::Elements(*concurrency as u64));
        
        group.bench_with_input(
            BenchmarkId::new("concurrent_reads", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for _ in 0..concurrency {
                        let cache_clone = cache.clone();
                        handles.push(tokio::spawn(async move {
                            for _ in 0..100 {
                                let inode = fastrand::u64(1..=50_000) as InodeId;
                                black_box(cache_clone.get_inode(inode));
                            }
                        }));
                    }
                    
                    for handle in handles {
                        handle.await.unwrap();
                    }
                })
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("concurrent_writes", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for task_id in 0..concurrency {
                        let cache_clone = cache.clone();
                        handles.push(tokio::spawn(async move {
                            for i in 0..100 {
                                let inode = (task_id * 100000 + i) as InodeId;
                                let node = create_test_node(inode);
                                black_box(cache_clone.insert_inode(node));
                            }
                        }));
                    }
                    
                    for handle in handles {
                        handle.await.unwrap();
                    }
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark LRU eviction performance
fn bench_lru_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_eviction");
    
    // Test eviction with different cache sizes
    for cache_size in [1_000, 5_000, 10_000].iter() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(300),
            *cache_size,
            5,
        ));
        
        group.throughput(Throughput::Elements(1));
        
        group.bench_with_input(
            BenchmarkId::new("eviction_pressure", cache_size),
            cache_size,
            |b, &cache_size| {
                b.iter(|| {
                    // Fill cache beyond capacity to trigger eviction
                    for i in 1..=(cache_size * 2) {
                        let node = create_test_node(i as InodeId);
                        black_box(cache.insert_inode(node));
                    }
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark cache warming and prefetching
fn bench_cache_warming(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_warming");
    
    for prefetch_size in [10, 100, 1000].iter() {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(300),
            100_000,
            5,
        ));
        
        group.throughput(Throughput::Elements(*prefetch_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("prefetch_request", prefetch_size),
            prefetch_size,
            |b, &prefetch_size| {
                b.to_async(&rt).iter(|| async {
                    let inodes: Vec<InodeId> = (1..=prefetch_size as InodeId).collect();
                    let hint = PrefetchHint {
                        inodes,
                        chunks: vec![],
                        paths: vec![],
                    };
                    black_box(cache.request_prefetch(hint).await)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark cached metadata manager
fn bench_cached_metadata_manager(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cached_metadata_manager");
    
    // Create test setup
    let temp_dir = TempDir::new().unwrap();
    let storage = rt.block_on(async {
        Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap())
    });
    
    let config = CacheConfig {
        ttl_secs: 300,
        max_size: 50_000,
        hot_threshold: 5,
        enable_lru: true,
        enable_prefetch: true,
        enable_invalidation: true,
        ..Default::default()
    };
    
    let manager = rt.block_on(async {
        Arc::new(CachedMetadataManager::new(storage, config).await.unwrap())
    });
    
    // Pre-populate with test data
    rt.block_on(async {
        for i in 1..=25_000 {
            let node = create_test_node(i);
            manager.save_inode(&node).await.unwrap();
        }
    });
    
    group.throughput(Throughput::Elements(1));
    
    // Benchmark cached reads (should be fast)
    group.bench_function("cached_read", |b| {
        b.to_async(&rt).iter(|| async {
            let inode = fastrand::u64(1..=25_000) as InodeId;
            black_box(manager.get_inode(inode).await.unwrap())
        })
    });
    
    // Benchmark cache miss reads (slower, includes storage access)
    group.bench_function("uncached_read", |b| {
        b.to_async(&rt).iter(|| async {
            let inode = fastrand::u64(25_001..=50_000) as InodeId;
            let node = create_test_node(inode);
            manager.save_inode(&node).await.unwrap();
            black_box(manager.get_inode(inode).await.unwrap())
        })
    });
    
    // Benchmark writes with caching
    group.bench_function("cached_write", |b| {
        let mut counter = 100_000;
        b.to_async(&rt).iter(|| async {
            let node = create_test_node(counter);
            counter += 1;
            black_box(manager.save_inode(&node).await.unwrap())
        })
    });
    
    group.finish();
}

/// Benchmark memory usage characteristics
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    
    // Test memory efficiency with different entry sizes
    for entry_count in [1_000, 10_000, 50_000].iter() {
        group.throughput(Throughput::Elements(*entry_count as u64));
        
        group.bench_with_input(
            BenchmarkId::new("memory_allocation", entry_count),
            entry_count,
            |b, &entry_count| {
                b.iter(|| {
                    let cache = Arc::new(EnhancedMetadataCache::new(
                        Duration::from_secs(300),
                        entry_count,
                        5,
                    ));
                    
                    // Fill cache
                    for i in 1..=entry_count {
                        let node = create_test_node(i as InodeId);
                        cache.insert_inode(node);
                    }
                    
                    // Access all entries to ensure they're in memory
                    for i in 1..=entry_count {
                        black_box(cache.get_inode(i as InodeId));
                    }
                    
                    black_box(cache)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark cache statistics collection performance
fn bench_cache_statistics(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_statistics");
    
    let cache = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(300),
        100_000,
        5,
    ));
    
    // Pre-populate cache and generate some activity
    for i in 1..=50_000 {
        let node = create_test_node(i);
        cache.insert_inode(node);
    }
    
    // Generate hits and misses
    for _ in 0..10_000 {
        cache.get_inode(fastrand::u64(1..=50_000) as InodeId);
        cache.get_inode(fastrand::u64(50_001..=100_000) as InodeId);
    }
    
    group.throughput(Throughput::Elements(1));
    
    group.bench_function("get_stats", |b| {
        b.iter(|| black_box(cache.stats().get_stats()))
    });
    
    group.bench_function("hit_rate_calculation", |b| {
        b.iter(|| black_box(cache.stats().hit_rate()))
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_cache_operations,
    bench_concurrent_access,
    bench_lru_eviction,
    bench_cache_warming,
    bench_cached_metadata_manager,
    bench_memory_usage,
    bench_cache_statistics
);
criterion_main!(benches);