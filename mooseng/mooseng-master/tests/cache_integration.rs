use mooseng_master::cache_enhanced::{EnhancedMetadataCache, CacheInvalidation, PrefetchHint, CacheWarmer};
use mooseng_master::cache_configuration::{CacheConfig, CacheMetrics};
use mooseng_master::metadata_cache_manager::{MetadataCacheManager, MetadataCacheMetricsCollector};
use mooseng_master::metadata::MetadataStore;
use mooseng_common::types::{ChunkId, FsNode, FsNodeType, InodeId, now_micros};
use mooseng_common::metrics::{MetricsRegistry, MasterMetrics, MetricCollector};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use tempfile::TempDir;

/// Integration test for the complete caching system
#[tokio::test]
async fn test_cache_system_integration() {
    let config = CacheConfig {
        ttl_secs: 1, // Short TTL for testing
        max_size: 100,
        hot_threshold: 3,
        enable_lru: true,
        enable_prefetch: true,
        enable_invalidation: true,
        cleanup_interval_secs: 1,
        stats_interval_secs: 5,
        eviction_percentage: 10,
        enable_compression: false,
        compression_threshold: 4096,
    };

    assert!(config.validate().is_ok());

    let cache = Arc::new(EnhancedMetadataCache::new(
        config.ttl(),
        config.max_size,
        config.hot_threshold,
    ));

    // Test basic cache operations
    let node = create_test_node(1);
    cache.insert_inode(node.clone());

    // Verify insertion
    let retrieved = cache.get_inode(1);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().inode, 1);

    // Test cache statistics
    let stats = cache.stats().get_stats();
    assert_eq!(stats["inserts"], 1);
    assert_eq!(stats["hits"], 1);

    // Test cache invalidation
    cache.invalidate(CacheInvalidation::InvalidateInode(1)).await;
    assert!(cache.get_inode(1).is_none());

    let stats = cache.stats().get_stats();
    assert_eq!(stats["invalidations"], 1);
}

#[tokio::test]
async fn test_cache_with_background_maintenance() {
    let cache = Arc::new(EnhancedMetadataCache::new(
        Duration::from_millis(500), // Short TTL
        1000,
        3,
    ));

    // Start background maintenance
    cache.clone().start_maintenance();

    // Insert nodes
    for i in 1..=10 {
        let node = create_test_node(i);
        cache.insert_inode(node);
    }

    // Verify all nodes are cached
    for i in 1..=10 {
        assert!(cache.get_inode(i).is_some());
    }

    // Wait for TTL expiration and cleanup
    time::sleep(Duration::from_millis(1000)).await;

    // Entries should be expired and cleaned up
    for i in 1..=10 {
        assert!(cache.get_inode(i).is_none());
    }

    let stats = cache.stats().get_stats();
    assert!(stats["evictions"] >= 10);
}

#[tokio::test]
async fn test_lru_eviction_integration() {
    let cache = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(60),
        10, // Small cache size
        5,  // Hot threshold
    ));

    // Fill cache to capacity
    for i in 1..=15 {
        let node = create_test_node(i);
        cache.insert_inode(node);
    }

    // Make some nodes "hot" by accessing them multiple times
    for _ in 0..6 {
        cache.get_inode(13);
        cache.get_inode(14);
        cache.get_inode(15);
    }

    // Some early nodes should have been evicted
    let mut evicted_count = 0;
    for i in 1..=12 {
        if cache.get_inode(i).is_none() {
            evicted_count += 1;
        }
    }
    assert!(evicted_count > 0);

    // Hot nodes should still be present
    assert!(cache.get_inode(13).is_some());
    assert!(cache.get_inode(14).is_some());
    assert!(cache.get_inode(15).is_some());

    let stats = cache.stats().get_stats();
    assert!(stats["evictions"] > 0);
    assert!(stats["hot_entries"] >= 3);
}

#[tokio::test]
async fn test_distributed_invalidation() {
    let cache1 = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(60),
        1000,
        5,
    ));

    let cache2 = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(60),
        1000,
        5,
    ));

    // Subscribe to invalidation events
    let mut invalidation_rx1 = cache1.subscribe_invalidations();
    let mut invalidation_rx2 = cache2.subscribe_invalidations();

    // Add same data to both caches
    let node = create_test_node(42);
    cache1.insert_inode(node.clone());
    cache2.insert_inode(node);

    // Verify both have the data
    assert!(cache1.get_inode(42).is_some());
    assert!(cache2.get_inode(42).is_some());

    // Invalidate from cache1
    cache1.invalidate(CacheInvalidation::InvalidateInode(42)).await;

    // Wait for invalidation message
    let msg1 = invalidation_rx1.recv().await.unwrap();
    let msg2 = invalidation_rx2.recv().await.unwrap();

    // Both should receive the invalidation message
    match (msg1, msg2) {
        (CacheInvalidation::InvalidateInode(id1), CacheInvalidation::InvalidateInode(id2)) => {
            assert_eq!(id1, 42);
            assert_eq!(id2, 42);
        }
        _ => panic!("Expected InvalidateInode messages"),
    }

    // Simulate cache2 processing the invalidation
    cache2.invalidate(CacheInvalidation::InvalidateInode(42)).await;

    // Both caches should now be empty for this inode
    assert!(cache1.get_inode(42).is_none());
    assert!(cache2.get_inode(42).is_none());
}

#[tokio::test]
async fn test_cache_warmer_integration() {
    let cache = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(60),
        1000,
        5,
    ));

    // Create a mock fetch function
    let fetch_fn = |hint: PrefetchHint| -> Vec<FsNode> {
        hint.inodes.into_iter().map(create_test_node).collect()
    };

    let warmer = CacheWarmer::new(cache.clone(), fetch_fn);
    let (tx, rx) = mpsc::channel(100);

    // Start the cache warmer
    warmer.start(rx).await;

    // Request prefetching
    let hint = PrefetchHint {
        inodes: vec![100, 101, 102],
        chunks: vec![],
        paths: vec![],
    };

    tx.send(hint).await.unwrap();

    // Give some time for prefetching to complete
    time::sleep(Duration::from_millis(100)).await;

    // Verify prefetched data is in cache
    assert!(cache.get_inode(100).is_some());
    assert!(cache.get_inode(101).is_some());
    assert!(cache.get_inode(102).is_some());

    let stats = cache.stats().get_stats();
    assert_eq!(stats["inserts"], 3);
    assert_eq!(stats["hits"], 3);
}

#[tokio::test]
async fn test_cache_performance_monitoring() {
    let cache = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(60),
        1000,
        3,
    ));

    // Generate some cache activity
    for i in 1..=50 {
        let node = create_test_node(i);
        cache.insert_inode(node);
    }

    // Create hits and misses
    for i in 1..=30 {
        cache.get_inode(i); // Hits
    }

    for i in 51..=70 {
        cache.get_inode(i); // Misses
    }

    // Make some entries hot
    for _ in 0..5 {
        for i in 1..=10 {
            cache.get_inode(i);
        }
    }

    let stats = cache.stats().get_stats();
    
    // Calculate metrics
    let metrics = CacheMetrics::calculate(
        stats["hits"],
        stats["misses"],
        stats["evictions"],
        50, // total entries
        stats["hot_entries"] as usize,
        1000, // max size
        50 * 1024, // estimated total size
    );

    assert!(metrics.hit_rate > 0.5);
    assert!(metrics.miss_rate < 0.5);
    assert_eq!(metrics.hit_rate + metrics.miss_rate, 1.0);
    assert!(metrics.hot_entry_percentage > 0.0);
    assert!(metrics.cache_utilization < 100.0);

    // Test health check
    if metrics.hit_rate > 0.7 && metrics.eviction_rate < 0.1 {
        assert!(metrics.is_healthy());
    }

    // Test recommendations
    let recommendations = metrics.recommendations();
    println!("Cache recommendations: {:?}", recommendations);
}

#[tokio::test]
async fn test_cache_clear_and_recovery() {
    let cache = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(60),
        1000,
        5,
    ));

    // Fill cache with data
    for i in 1..=100 {
        let node = create_test_node(i);
        cache.insert_inode(node);
    }

    // Verify data exists
    for i in 1..=10 {
        assert!(cache.get_inode(i).is_some());
    }

    let stats_before = cache.stats().get_stats();
    assert_eq!(stats_before["inserts"], 100);

    // Clear entire cache
    cache.clear();

    // Verify cache is empty
    for i in 1..=10 {
        assert!(cache.get_inode(i).is_none());
    }

    let stats_after = cache.stats().get_stats();
    assert_eq!(stats_after["evictions"], 100);

    // Test recovery by re-adding data
    for i in 1..=20 {
        let node = create_test_node(i);
        cache.insert_inode(node);
    }

    // Verify cache is working again
    for i in 1..=20 {
        assert!(cache.get_inode(i).is_some());
    }
}

#[tokio::test]
async fn test_concurrent_cache_operations() {
    let cache = Arc::new(EnhancedMetadataCache::new(
        Duration::from_secs(60),
        1000,
        5,
    ));

    let mut handles = vec![];

    // Spawn multiple tasks doing concurrent operations
    for task_id in 0..10 {
        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move {
            let base_id = task_id * 100;
            
            // Insert data
            for i in 0..50 {
                let node = create_test_node(base_id + i);
                cache_clone.insert_inode(node);
            }

            // Read data
            for i in 0..50 {
                cache_clone.get_inode(base_id + i);
            }

            // Invalidate some data
            for i in 0..10 {
                cache_clone.invalidate(CacheInvalidation::InvalidateInode(base_id + i)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    let stats = cache.stats().get_stats();
    
    // Should have significant activity
    assert!(stats["inserts"] >= 500);
    assert!(stats["hits"] >= 500);
    assert!(stats["invalidations"] >= 100);

    println!("Concurrent test stats: {:?}", stats);
    println!("Hit rate: {:.2}%", cache.stats().hit_rate() * 100.0);
}

/// Test Prometheus metrics integration with metadata cache manager
#[tokio::test]
async fn test_prometheus_metrics_integration() {
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
    
    // Create metrics registry and MasterMetrics
    let metrics_registry = MetricsRegistry::new("test_master").unwrap();
    let metrics = Arc::new(MasterMetrics::new(&metrics_registry).unwrap());
    
    // Create metadata cache manager with metrics
    let manager = Arc::new(
        MetadataCacheManager::new_with_metrics(store, config, Some(metrics.clone()))
            .await
            .unwrap()
    );
    
    // Create metrics collector
    let collector = MetadataCacheMetricsCollector::new(manager.clone(), metrics.clone());
    
    // Generate some cache activity to create metrics data
    for i in 1..=20 {
        let node = create_test_node(i);
        manager.save_inode(&node).await.unwrap();
    }
    
    // Access some nodes to generate cache hits
    for i in 1..=15 {
        manager.get_inode(i).await.unwrap();
    }
    
    // Try to access non-existent nodes to generate cache misses
    for i in 21..=25 {
        manager.get_inode(i).await.unwrap();
    }
    
    // Run the metrics collector
    collector.collect().await.unwrap();
    
    // Export metrics and verify they contain expected data
    let exported_metrics = metrics_registry.export_metrics().await.unwrap();
    
    // Verify cache-specific metrics are present
    assert!(exported_metrics.contains("master_metadata_cache_hits_total"));
    assert!(exported_metrics.contains("master_metadata_cache_misses_total"));
    assert!(exported_metrics.contains("master_metadata_cache_size_bytes"));
    assert!(exported_metrics.contains("master_metadata_operations_total"));
    assert!(exported_metrics.contains("master_metadata_operation_duration_seconds"));
    
    // Verify component label is present
    assert!(exported_metrics.contains("component=\"test_master\""));
    
    // Check that the metrics collector is working by running it multiple times
    for _ in 0..3 {
        // Generate more activity
        for i in 26..=30 {
            let node = create_test_node(i);
            manager.save_inode(&node).await.unwrap();
        }
        
        // Collect metrics
        collector.collect().await.unwrap();
        
        // Export and verify metrics are still present
        let metrics_output = metrics_registry.export_metrics().await.unwrap();
        assert!(metrics_output.contains("master_metadata_cache_hits_total"));
    }
    
    println!("Prometheus metrics integration test completed successfully");
    println!("Sample exported metrics:\n{}", &exported_metrics[..std::cmp::min(1000, exported_metrics.len())]);
}

/// Test cache health monitoring integration with metrics
#[tokio::test]
async fn test_cache_health_monitoring_with_metrics() {
    let temp_dir = TempDir::new().unwrap();
    let store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
    
    let config = CacheConfig {
        ttl_secs: 60,
        max_size: 100, // Small cache to trigger evictions
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
    
    // Create metrics registry and MasterMetrics
    let metrics_registry = MetricsRegistry::new("health_test_master").unwrap();
    let metrics = Arc::new(MasterMetrics::new(&metrics_registry).unwrap());
    
    // Create metadata cache manager with metrics
    let manager = Arc::new(
        MetadataCacheManager::new_with_metrics(store, config, Some(metrics.clone()))
            .await
            .unwrap()
    );
    
    // Fill cache beyond capacity to trigger evictions
    for i in 1..=150 {
        let node = create_test_node(i);
        manager.save_inode(&node).await.unwrap();
    }
    
    // Create some access patterns
    for _ in 0..5 {
        for i in 1..=50 {
            manager.get_inode(i).await.unwrap();
        }
    }
    
    // Check cache health
    let (is_healthy, recommendations) = manager.get_cache_health().await;
    
    // Create and run metrics collector
    let collector = MetadataCacheMetricsCollector::new(manager.clone(), metrics.clone());
    collector.collect().await.unwrap();
    
    // Export metrics and verify health-related information is captured
    let exported_metrics = metrics_registry.export_metrics().await.unwrap();
    
    // Verify metrics contain cache activity
    assert!(exported_metrics.contains("master_metadata_cache_hits_total"));
    assert!(exported_metrics.contains("master_metadata_cache_misses_total"));
    assert!(exported_metrics.contains("master_metadata_cache_size_bytes"));
    
    // Print health status for verification
    println!("Cache health status: healthy={}, recommendations={:?}", is_healthy, recommendations);
    
    // Get cache stats to verify metrics collection
    let cache_stats = manager.get_cache_stats().await;
    println!("Cache statistics: {:?}", cache_stats);
    
    // Verify that we have some meaningful activity
    assert!(cache_stats["hits"] > 0);
    assert!(cache_stats["inserts"] > 0);
    
    println!("Cache health monitoring with metrics test completed successfully");
}

/// Test metrics collection under concurrent load
#[tokio::test]
async fn test_concurrent_metrics_collection() {
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
    
    // Create metrics registry and MasterMetrics
    let metrics_registry = MetricsRegistry::new("concurrent_test_master").unwrap();
    let metrics = Arc::new(MasterMetrics::new(&metrics_registry).unwrap());
    
    // Create metadata cache manager with metrics
    let manager = Arc::new(
        MetadataCacheManager::new_with_metrics(store, config, Some(metrics.clone()))
            .await
            .unwrap()
    );
    
    // Create metrics collector
    let collector = Arc::new(MetadataCacheMetricsCollector::new(manager.clone(), metrics.clone()));
    
    let mut handles = vec![];
    
    // Spawn concurrent operations
    for task_id in 0..5 {
        let manager_clone = manager.clone();
        let collector_clone = collector.clone();
        
        let handle = tokio::spawn(async move {
            let base_id = task_id * 100;
            
            // Generate cache operations
            for i in 0..50 {
                let node = create_test_node(base_id + i);
                manager_clone.save_inode(&node).await.unwrap();
                
                // Interleave with reads
                if i % 5 == 0 {
                    manager_clone.get_inode(base_id + i).await.unwrap();
                }
            }
            
            // Collect metrics periodically
            if task_id % 2 == 0 {
                collector_clone.collect().await.unwrap();
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Final metrics collection
    collector.collect().await.unwrap();
    
    // Export and verify metrics
    let exported_metrics = metrics_registry.export_metrics().await.unwrap();
    
    // Verify expected metrics are present and have reasonable values
    assert!(exported_metrics.contains("master_metadata_cache_hits_total"));
    assert!(exported_metrics.contains("master_metadata_cache_misses_total"));
    assert!(exported_metrics.contains("master_metadata_operations_total"));
    
    // Get final cache stats
    let cache_stats = manager.get_cache_stats().await;
    
    // Should have significant activity from concurrent operations
    assert!(cache_stats["hits"] >= 50);  // At least some hits from reads
    assert!(cache_stats["inserts"] >= 250); // 5 tasks * 50 inserts each
    
    println!("Concurrent metrics collection test completed successfully");
    println!("Final cache stats: {:?}", cache_stats);
}

/// Helper function to create test nodes
fn create_test_node(inode: InodeId) -> FsNode {
    FsNode {
        inode,
        parent: Some(0), // Root parent
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
            chunk_ids: vec![inode as ChunkId], // Simple mapping for testing
            session_id: None,
        },
    }
}