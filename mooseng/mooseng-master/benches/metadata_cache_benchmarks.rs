use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mooseng_master::cache_enhanced::{EnhancedMetadataCache, CacheInvalidation, PrefetchHint};
use mooseng_master::cache_configuration::CacheConfig;
use mooseng_master::metadata_cache_manager::{MetadataCacheManager, MetadataCacheMetricsCollector};
use mooseng_master::metadata::MetadataStore;
use mooseng_common::types::{FsNode, FsNodeType, InodeId, now_micros};
use mooseng_common::metrics::{MetricsRegistry, MasterMetrics};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

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
            chunk_ids: vec![],
            session_id: None,
        },
    }
}

fn bench_cache_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_operations");
    
    // Test different cache sizes
    for cache_size in [1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(*cache_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("insert", cache_size),
            cache_size,
            |b, &size| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(300),
                    size,
                    5,
                ));
                
                let nodes: Vec<FsNode> = (1..=size as InodeId)
                    .map(create_test_node)
                    .collect();
                
                let mut idx = 0;
                b.iter(|| {
                    cache.insert_inode(nodes[idx % nodes.len()].clone());
                    idx += 1;
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("get_hit", cache_size),
            cache_size,
            |b, &size| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(300),
                    size,
                    5,
                ));
                
                // Pre-populate cache
                for i in 1..=size as InodeId {
                    cache.insert_inode(create_test_node(i));
                }
                
                let mut idx = 1;
                b.iter(|| {
                    let _ = cache.get_inode(idx);
                    idx = (idx % size as InodeId) + 1;
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("get_miss", cache_size),
            cache_size,
            |b, &size| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(300),
                    size,
                    5,
                ));
                
                let mut idx = size as InodeId + 1;
                b.iter(|| {
                    let _ = cache.get_inode(idx);
                    idx += 1;
                });
            },
        );
    }
    
    group.finish();
}

fn bench_lru_eviction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("lru_eviction");
    
    // Test LRU eviction under different load patterns
    for pattern in ["sequential", "random", "hotspot"].iter() {
        group.bench_function(
            BenchmarkId::new("eviction", pattern),
            |b| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(300),
                    1000, // Small cache to trigger evictions
                    5,
                ));
                
                b.iter(|| match *pattern {
                    "sequential" => {
                        for i in 1..=1500 {
                            cache.insert_inode(create_test_node(i));
                        }
                    }
                    "random" => {
                        for i in 1..=1500 {
                            let inode = ((i * 17) % 10000) + 1; // Pseudo-random
                            cache.insert_inode(create_test_node(inode));
                        }
                    }
                    "hotspot" => {
                        // 80% access to first 20% of range
                        for i in 1..=1500 {
                            let inode = if i % 5 == 0 {
                                ((i * 17) % 8000) + 301 // 20% range
                            } else {
                                (i % 300) + 1 // 80% access to hotspot
                            };
                            cache.insert_inode(create_test_node(inode));
                            // Access recently inserted to make it hot
                            cache.get_inode(inode);
                        }
                    }
                    _ => unreachable!(),
                });
            },
        );
    }
    
    group.finish();
}

fn bench_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_access");
    
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_function(
            BenchmarkId::new("mixed_operations", thread_count),
            |b| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(300),
                    10000,
                    5,
                ));
                
                // Pre-populate cache
                for i in 1..=5000 {
                    cache.insert_inode(create_test_node(i));
                }
                
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];
                    
                    for task_id in 0..*thread_count {
                        let cache_clone = cache.clone();
                        let handle = tokio::spawn(async move {
                            let base = task_id * 1000;
                            
                            // Mixed read/write operations
                            for i in 0..100 {
                                let inode = base + i;
                                
                                if i % 3 == 0 {
                                    // Insert new
                                    cache_clone.insert_inode(create_test_node(inode + 10000));
                                } else {
                                    // Read existing
                                    cache_clone.get_inode((inode % 5000) + 1);
                                }
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn bench_invalidation_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("invalidation");
    
    group.bench_function("single_invalidation", |b| {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(300),
            10000,
            5,
        ));
        
        // Pre-populate cache
        for i in 1..=5000 {
            cache.insert_inode(create_test_node(i));
        }
        
        let mut idx = 1;
        b.to_async(&rt).iter(|| async {
            cache.invalidate(CacheInvalidation::InvalidateInode(idx)).await;
            idx = (idx % 5000) + 1;
        });
    });
    
    group.bench_function("broadcast_invalidation", |b| {
        let cache = Arc::new(EnhancedMetadataCache::new(
            Duration::from_secs(300),
            10000,
            5,
        ));
        
        // Setup multiple subscribers
        let _rx1 = cache.subscribe_invalidations();
        let _rx2 = cache.subscribe_invalidations();
        let _rx3 = cache.subscribe_invalidations();
        
        // Pre-populate cache
        for i in 1..=5000 {
            cache.insert_inode(create_test_node(i));
        }
        
        let mut idx = 1;
        b.to_async(&rt).iter(|| async {
            cache.invalidate(CacheInvalidation::InvalidateInode(idx)).await;
            idx = (idx % 5000) + 1;
        });
    });
    
    group.finish();
}

fn bench_prefetching_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("prefetching");
    
    for batch_size in [10, 50, 100].iter() {
        group.bench_function(
            BenchmarkId::new("prefetch_request", batch_size),
            |b| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(300),
                    10000,
                    5,
                ));
                
                let mut base_id = 1;
                b.to_async(&rt).iter(|| async {
                    let hint = PrefetchHint {
                        inodes: (base_id..base_id + *batch_size).collect(),
                        chunks: vec![],
                        paths: vec![],
                    };
                    
                    cache.request_prefetch(hint).await;
                    base_id += *batch_size;
                });
            },
        );
    }
    
    group.finish();
}

fn bench_cache_manager_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_manager");
    
    group.bench_function("get_inode_with_fallback", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
            
            let config = CacheConfig {
                ttl_secs: 300,
                max_size: 10000,
                hot_threshold: 5,
                enable_lru: true,
                enable_prefetch: false, // Disable for benchmark
                enable_invalidation: false, // Disable for benchmark
                cleanup_interval_secs: 60,
                stats_interval_secs: 300,
                eviction_percentage: 10,
                enable_compression: false,
                compression_threshold: 4096,
            };
            
            let manager = MetadataCacheManager::new(store, config).await.unwrap();
            
            // Pre-populate store with test data
            for i in 1..=100 {
                let node = create_test_node(i);
                manager.save_inode(&node).await.unwrap();
            }
            
            // Benchmark cache hits and misses
            for i in 1..=200 {
                let _ = manager.get_inode(i).await.unwrap();
            }
        });
    });
    
    group.bench_function("save_inode_with_cache", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
            
            let config = CacheConfig {
                ttl_secs: 300,
                max_size: 10000,
                hot_threshold: 5,
                enable_lru: true,
                enable_prefetch: false,
                enable_invalidation: false,
                cleanup_interval_secs: 60,
                stats_interval_secs: 300,
                eviction_percentage: 10,
                enable_compression: false,
                compression_threshold: 4096,
            };
            
            let manager = MetadataCacheManager::new(store, config).await.unwrap();
            
            // Benchmark save operations
            for i in 1..=100 {
                let node = create_test_node(i);
                manager.save_inode(&node).await.unwrap();
            }
        });
    });
    
    group.finish();
}

fn bench_metrics_collection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("metrics_collection");
    
    group.bench_function("collect_cache_metrics", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
            
            let config = CacheConfig {
                ttl_secs: 300,
                max_size: 10000,
                hot_threshold: 5,
                enable_lru: true,
                enable_prefetch: true,
                enable_invalidation: true,
                cleanup_interval_secs: 60,
                stats_interval_secs: 300,
                eviction_percentage: 10,
                enable_compression: false,
                compression_threshold: 4096,
            };
            
            let metrics_registry = MetricsRegistry::new("bench_master").unwrap();
            let metrics = Arc::new(MasterMetrics::new(&metrics_registry).unwrap());
            
            let manager = Arc::new(
                MetadataCacheManager::new_with_metrics(store, config, Some(metrics.clone()))
                    .await
                    .unwrap()
            );
            
            // Generate some cache activity
            for i in 1..=100 {
                let node = create_test_node(i);
                manager.save_inode(&node).await.unwrap();
                manager.get_inode(i).await.unwrap();
            }
            
            let collector = MetadataCacheMetricsCollector::new(manager, metrics);
            
            // Benchmark metrics collection
            collector.collect().await.unwrap();
        });
    });
    
    group.finish();
}

fn bench_cache_configurations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cache_configurations");
    
    // Test different TTL values
    for ttl_secs in [60, 300, 1800].iter() {
        group.bench_function(
            BenchmarkId::new("ttl_impact", ttl_secs),
            |b| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(*ttl_secs),
                    10000,
                    5,
                ));
                
                // Pre-populate
                for i in 1..=5000 {
                    cache.insert_inode(create_test_node(i));
                }
                
                b.iter(|| {
                    // Mixed operations that should trigger TTL checks
                    for i in 1..=100 {
                        cache.get_inode((i % 5000) + 1);
                    }
                });
            },
        );
    }
    
    // Test different hot thresholds
    for hot_threshold in [1, 3, 5, 10].iter() {
        group.bench_function(
            BenchmarkId::new("hot_threshold", hot_threshold),
            |b| {
                let cache = Arc::new(EnhancedMetadataCache::new(
                    Duration::from_secs(300),
                    1000, // Small cache to trigger evictions
                    *hot_threshold,
                ));
                
                b.iter(|| {
                    // Pattern that creates hot entries
                    for i in 1..=2000 {
                        cache.insert_inode(create_test_node(i));
                        
                        // Make some entries hot
                        if i % 10 == 0 {
                            for _ in 0..*hot_threshold + 1 {
                                cache.get_inode(i);
                            }
                        }
                    }
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_cache_operations,
    bench_lru_eviction,
    bench_concurrent_access,
    bench_invalidation_performance,
    bench_prefetching_performance,
    bench_cache_manager_operations,
    bench_metrics_collection,
    bench_cache_configurations
);

criterion_main!(benches);