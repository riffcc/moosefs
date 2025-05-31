use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use mooseng_master::cache_enhanced::{EnhancedMetadataCache, CacheInvalidation, PrefetchHint};
use mooseng_master::cache_configuration::{CacheConfig, CacheMetrics};
use mooseng_master::metadata_cache_manager::MetadataCacheManager;
use mooseng_master::MetadataStore;
use mooseng_common::types::{FsNode, FsNodeType, InodeId, now_micros};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create a test FsNode
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

/// Create cache with specific configuration
fn create_cache(config: &CacheConfig) -> Arc<EnhancedMetadataCache> {
    Arc::new(EnhancedMetadataCache::new(
        config.ttl(),
        config.max_size,
        config.hot_threshold,
    ))
}

/// Create cache manager for integration tests
async fn create_cache_manager(config: CacheConfig) -> (MetadataCacheManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
    let manager = MetadataCacheManager::new(store, config).await.unwrap();
    (manager, temp_dir)
}

/// Benchmark basic cache operations
fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_operations");
    
    let config = CacheConfig::default();
    let cache = create_cache(&config);
    
    // Benchmark insertion
    group.bench_function("insert", |b| {
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            let node = create_test_node(counter, Some(1));
            cache.insert_inode(black_box(node));
        });
    });
    
    // Prepare cache with data for other benchmarks
    for i in 1..=10000 {
        let node = create_test_node(i, Some(1));
        cache.insert_inode(node);
    }
    
    // Benchmark cache hits
    group.bench_function("get_hit", |b| {
        let mut counter = 1;
        b.iter(|| {
            let result = cache.get_inode(black_box(counter));
            counter = if counter >= 10000 { 1 } else { counter + 1 };
            black_box(result);
        });
    });
    
    // Benchmark cache misses
    group.bench_function("get_miss", |b| {
        let mut counter = 20000;
        b.iter(|| {
            counter += 1;
            let result = cache.get_inode(black_box(counter));
            black_box(result);
        });
    });
    
    group.finish();
}

/// Benchmark cache performance under different loads
fn bench_cache_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_scaling");
    
    let cache_sizes = [1000, 10000, 100000, 1000000];
    
    for size in cache_sizes {
        let config = CacheConfig {
            max_size: size,
            ..Default::default()
        };
        let cache = create_cache(&config);
        
        // Fill cache to capacity
        for i in 1..=size as InodeId {
            let node = create_test_node(i, Some(1));
            cache.insert_inode(node);
        }
        
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("get_performance", size),
            &size,
            |b, _| {
                let mut counter = 1;
                b.iter(|| {
                    let result = cache.get_inode(black_box(counter as InodeId));
                    counter = if counter >= size { 1 } else { counter + 1 };
                    black_box(result);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark LRU eviction performance
fn bench_lru_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("lru_eviction");
    
    let config = CacheConfig {
        max_size: 1000,
        hot_threshold: 5,
        ..Default::default()
    };
    let cache = create_cache(&config);
    
    // Fill cache to capacity
    for i in 1..=1000 {
        let node = create_test_node(i, Some(1));
        cache.insert_inode(node);
    }
    
    // Make some entries hot
    for _ in 0..6 {
        for i in 900..=1000 {
            cache.get_inode(i);
        }
    }
    
    // Benchmark eviction when inserting new entries
    group.bench_function("eviction_trigger", |b| {
        let mut counter = 2000;
        b.iter(|| {
            counter += 1;
            let node = create_test_node(counter, Some(1));
            cache.insert_inode(black_box(node));
        });
    });
    
    group.finish();
}

/// Benchmark cache invalidation performance
fn bench_cache_invalidation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("cache_invalidation");
    
    let config = CacheConfig::default();
    let cache = create_cache(&config);
    
    // Fill cache with data
    for i in 1..=10000 {
        let node = create_test_node(i, Some(1));
        cache.insert_inode(node);
    }
    
    // Benchmark single invalidation
    group.bench_function("invalidate_single", |b| {
        let mut counter = 1;
        b.iter(|| {
            counter += 1;
            if counter > 10000 {
                counter = 1;
            }
            
            rt.block_on(async {
                cache.invalidate(CacheInvalidation::InvalidateInode(black_box(counter))).await;
            });
        });
    });
    
    // Benchmark invalidate all
    group.bench_function("invalidate_all", |b| {
        b.iter(|| {
            rt.block_on(async {
                cache.invalidate(CacheInvalidation::InvalidateAll).await;
            });
            
            // Refill cache for next iteration
            for i in 1..=1000 {
                let node = create_test_node(i, Some(1));
                cache.insert_inode(node);
            }
        });
    });
    
    group.finish();
}

/// Benchmark concurrent cache access
fn bench_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_access");
    
    let config = CacheConfig {
        max_size: 100000,
        ..Default::default()
    };
    let cache = Arc::new(EnhancedMetadataCache::new(
        config.ttl(),
        config.max_size,
        config.hot_threshold,
    ));
    
    // Fill cache with data
    for i in 1..=50000 {
        let node = create_test_node(i, Some(1));
        cache.insert_inode(node);
    }
    
    // Benchmark concurrent reads
    group.bench_function("concurrent_reads", |b| {
        b.iter(|| {
            rt.block_on(async {
                let cache = cache.clone();
                let handles: Vec<_> = (0..8).map(|thread_id| {
                    let cache = cache.clone();
                    tokio::spawn(async move {
                        for i in 0..1000 {
                            let inode = (thread_id * 1000 + i + 1) % 50000 + 1;
                            let _ = cache.get_inode(inode);
                        }
                    })
                }).collect();
                
                for handle in handles {
                    handle.await.unwrap();
                }
            });
        });
    });
    
    // Benchmark mixed read/write operations
    group.bench_function("concurrent_mixed", |b| {
        b.iter(|| {
            rt.block_on(async {
                let cache = cache.clone();
                let handles: Vec<_> = (0..8).map(|thread_id| {
                    let cache = cache.clone();
                    tokio::spawn(async move {
                        for i in 0..500 {
                            let inode = (thread_id * 1000 + i + 1) % 50000 + 1;
                            
                            if i % 10 == 0 {
                                // 10% writes
                                let node = create_test_node(inode + 100000, Some(1));
                                cache.insert_inode(node);
                            } else {
                                // 90% reads
                                let _ = cache.get_inode(inode);
                            }
                        }
                    })
                }).collect();
                
                for handle in handles {
                    handle.await.unwrap();
                }
            });
        });
    });
    
    group.finish();
}

/// Benchmark integrated cache manager performance
fn bench_cache_manager_integration(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("cache_manager_integration");
    
    let config = CacheConfig {
        max_size: 50000,
        enable_prefetch: true,
        enable_invalidation: true,
        ..Default::default()
    };
    
    // Benchmark cache manager operations
    group.bench_function("manager_get_inode", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (manager, _temp_dir) = create_cache_manager(config.clone()).await;
                
                // Create and save test nodes
                for i in 1..=1000 {
                    let node = create_test_node(i, Some(1));
                    manager.save_inode(&node).await.unwrap();
                }
                
                // Benchmark gets (mix of cache hits and store fetches)
                for i in 1..=2000 {
                    let inode = if i <= 1000 { i } else { i + 10000 }; // Mix hits and misses
                    let _ = manager.get_inode(black_box(inode)).await;
                }
            });
        });
    });
    
    group.bench_function("manager_save_inode", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (manager, _temp_dir) = create_cache_manager(config.clone()).await;
                
                let mut counter = 1;
                for _ in 0..1000 {
                    counter += 1;
                    let node = create_test_node(counter, Some(1));
                    manager.save_inode(black_box(&node)).await.unwrap();
                }
            });
        });
    });
    
    group.finish();
}

/// Benchmark different cache configurations
fn bench_cache_configurations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_configurations");
    
    let configurations = [
        ("default", CacheConfig::default()),
        ("high_performance", CacheConfig {
            ttl_secs: 1800,
            max_size: 1_000_000,
            hot_threshold: 3,
            enable_lru: true,
            enable_prefetch: true,
            enable_invalidation: true,
            cleanup_interval_secs: 300,
            stats_interval_secs: 600,
            eviction_percentage: 5,
            enable_compression: false,
            compression_threshold: 8192,
        }),
        ("memory_constrained", CacheConfig {
            ttl_secs: 180,
            max_size: 25_000,
            hot_threshold: 8,
            enable_lru: true,
            enable_prefetch: false,
            enable_invalidation: true,
            cleanup_interval_secs: 30,
            stats_interval_secs: 300,
            eviction_percentage: 20,
            enable_compression: false,
            compression_threshold: 2048,
        }),
        ("write_heavy", CacheConfig {
            ttl_secs: 60,
            max_size: 200_000,
            hot_threshold: 10,
            enable_lru: true,
            enable_prefetch: false,
            enable_invalidation: true,
            cleanup_interval_secs: 30,
            stats_interval_secs: 120,
            eviction_percentage: 15,
            enable_compression: false,
            compression_threshold: 4096,
        }),
    ];
    
    for (config_name, config) in configurations {
        let cache = create_cache(&config);
        
        // Fill cache
        let fill_size = std::cmp::min(config.max_size / 2, 10000);
        for i in 1..=fill_size as InodeId {
            let node = create_test_node(i, Some(1));
            cache.insert_inode(node);
        }
        
        group.bench_with_input(
            BenchmarkId::new("mixed_workload", config_name),
            &config_name,
            |b, _| {
                let mut counter = 1;
                b.iter(|| {
                    // 80% reads, 20% writes
                    if counter % 5 == 0 {
                        // Write operation
                        let node = create_test_node(counter + 100000, Some(1));
                        cache.insert_inode(black_box(node));
                    } else {
                        // Read operation
                        let inode = (counter % fill_size as InodeId) + 1;
                        let _ = cache.get_inode(black_box(inode));
                    }
                    counter += 1;
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark cache performance metrics calculation
fn bench_performance_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_metrics");
    
    // Test different scale metrics calculations
    let test_cases = [
        ("small", 1000, 800, 50, 500, 50, 1000, 512000),
        ("medium", 50000, 40000, 2000, 25000, 2500, 50000, 25600000),
        ("large", 1000000, 700000, 50000, 500000, 50000, 1000000, 512000000),
    ];
    
    for (size_name, hits, misses, evictions, total_entries, hot_entries, max_size, total_size_bytes) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("metrics_calculation", size_name),
            &size_name,
            |b, _| {
                b.iter(|| {
                    let metrics = CacheMetrics::calculate(
                        black_box(hits),
                        black_box(misses),
                        black_box(evictions),
                        black_box(total_entries),
                        black_box(hot_entries),
                        black_box(max_size),
                        black_box(total_size_bytes),
                    );
                    black_box(metrics);
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_cache_operations,
    bench_cache_scaling,
    bench_lru_eviction,
    bench_cache_invalidation,
    bench_concurrent_access,
    bench_cache_manager_integration,
    bench_cache_configurations,
    bench_performance_metrics
);

criterion_main!(benches);