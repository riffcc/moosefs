# Cache Configuration Guide

## Overview

The Cache Configuration system (`cache_config.rs`) provides comprehensive configuration management for the Enhanced Metadata Cache system in MooseNG. It includes configuration validation, performance metrics calculation, and intelligent recommendations for optimal cache performance.

## Configuration Structure

### CacheConfig

The main configuration struct that defines all cache behavior parameters:

```rust
use mooseng_master::cache_config::CacheConfig;
use std::time::Duration;

// Create default configuration
let config = CacheConfig::default();

// Create custom configuration
let config = CacheConfig {
    ttl_secs: 600,                    // 10 minutes TTL
    max_size: 200_000,                // 200k entries
    hot_threshold: 10,                // 10 accesses = hot
    enable_lru: true,
    enable_prefetch: true,
    enable_invalidation: true,
    cleanup_interval_secs: 120,       // 2 minutes cleanup
    stats_interval_secs: 600,         // 10 minutes stats
    eviction_percentage: 8,           // Evict 8% when full
    enable_compression: false,
    compression_threshold: 8192,      // 8KB compression threshold
};
```

## Configuration Parameters

### Core Parameters

#### `ttl_secs` (Time-To-Live)
- **Default**: 300 seconds (5 minutes)
- **Range**: 1 - unlimited
- **Purpose**: How long entries remain valid in cache
- **Tuning**: 
  - Increase for stable metadata (600-3600s)
  - Decrease for frequently changing data (60-300s)

```rust
// For read-heavy workloads with stable metadata
let config = CacheConfig {
    ttl_secs: 1800, // 30 minutes
    ..Default::default()
};

// For write-heavy workloads with frequent changes
let config = CacheConfig {
    ttl_secs: 60, // 1 minute
    ..Default::default()
};
```

#### `max_size` (Maximum Cache Size)
- **Default**: 100,000 entries
- **Range**: 1 - unlimited (limited by memory)
- **Purpose**: Maximum number of entries in cache
- **Memory Estimation**: ~200-500 bytes per entry + data size

```rust
// Calculate memory usage
let estimated_memory_mb = (config.max_size * 400) / (1024 * 1024);
println!("Estimated cache memory: {}MB", estimated_memory_mb);
```

#### `hot_threshold` (Hot Entry Threshold)
- **Default**: 5 accesses
- **Range**: 1 - unlimited
- **Purpose**: Number of accesses before entry is considered "hot"
- **Effect**: Hot entries are protected from LRU eviction

### Behavioral Flags

#### `enable_lru` (LRU Eviction)
- **Default**: true
- **Purpose**: Enable least-recently-used eviction policy
- **Recommendation**: Always keep enabled for production

#### `enable_prefetch` (Cache Warming)
- **Default**: true  
- **Purpose**: Enable proactive prefetching of data
- **Use Case**: Disable for memory-constrained environments

#### `enable_invalidation` (Distributed Invalidation)
- **Default**: true
- **Purpose**: Enable pub/sub cache invalidation
- **Requirement**: Must be enabled for multi-node deployments

### Maintenance Parameters

#### `cleanup_interval_secs` (Background Cleanup)
- **Default**: 60 seconds
- **Purpose**: How often to clean expired entries
- **Tuning**: Balance between CPU usage and memory cleanup

#### `stats_interval_secs` (Statistics Reporting)
- **Default**: 300 seconds (5 minutes)
- **Purpose**: How often to log cache statistics
- **Monitoring**: Decrease for more frequent monitoring

#### `eviction_percentage` (Eviction Batch Size)
- **Default**: 5% 
- **Range**: 1-100%
- **Purpose**: Percentage of cache to evict when full
- **Tuning**: Higher values reduce eviction frequency but cause larger pauses

### Advanced Parameters

#### `enable_compression` (Entry Compression)
- **Default**: false
- **Purpose**: Compress large cache entries to save memory
- **Trade-off**: CPU usage vs memory savings
- **Future Feature**: Currently placeholder for implementation

#### `compression_threshold` (Compression Size Limit)
- **Default**: 4096 bytes (4KB)
- **Purpose**: Minimum entry size to trigger compression
- **Tuning**: Balance compression overhead vs memory savings

## Configuration Validation

All configurations are automatically validated:

```rust
let config = CacheConfig {
    ttl_secs: 0,  // Invalid!
    ..Default::default()
};

match config.validate() {
    Ok(()) => println!("Configuration valid"),
    Err(msg) => println!("Invalid configuration: {}", msg),
}
```

### Validation Rules

- `ttl_secs` > 0
- `max_size` > 0  
- `hot_threshold` > 0
- `eviction_percentage` ≤ 100
- All intervals > 0

## Performance Metrics and Monitoring

### CacheMetrics

Comprehensive performance analysis:

```rust
use mooseng_master::cache_config::CacheMetrics;

let metrics = CacheMetrics::calculate(
    hits,              // Number of cache hits
    misses,            // Number of cache misses  
    evictions,         // Number of evictions
    total_entries,     // Current entries in cache
    hot_entries,       // Number of hot entries
    max_size,          // Maximum cache size
    total_size_bytes,  // Total memory usage
);

println!("Hit rate: {:.2}%", metrics.hit_rate * 100.0);
println!("Cache utilization: {:.2}%", metrics.cache_utilization);
println!("Hot entry percentage: {:.2}%", metrics.hot_entry_percentage);
```

### Health Monitoring

```rust
// Check if cache is performing well
if metrics.is_healthy() {
    println!("Cache is performing optimally");
} else {
    println!("Cache performance issues detected");
    
    // Get specific recommendations
    for recommendation in metrics.recommendations() {
        println!("Recommendation: {}", recommendation);
    }
}
```

## Workload-Specific Configurations

### High-Throughput Workload

```rust
let high_throughput_config = CacheConfig {
    ttl_secs: 300,              // Standard TTL
    max_size: 500_000,          // Large cache
    hot_threshold: 3,           // Lower hot threshold
    cleanup_interval_secs: 30,  // Frequent cleanup
    eviction_percentage: 10,    // Larger eviction batches
    enable_prefetch: true,      // Aggressive prefetching
    ..Default::default()
};
```

### Memory-Constrained Environment

```rust
let memory_constrained_config = CacheConfig {
    ttl_secs: 180,              // Shorter TTL
    max_size: 25_000,           // Smaller cache
    hot_threshold: 8,           // Higher hot threshold
    cleanup_interval_secs: 60,  // Standard cleanup
    eviction_percentage: 15,    // Aggressive eviction
    enable_prefetch: false,     // Disable prefetching
    ..Default::default()
};
```

### Write-Heavy Workload

```rust
let write_heavy_config = CacheConfig {
    ttl_secs: 60,               // Short TTL
    max_size: 100_000,          // Standard size
    hot_threshold: 15,          // Very high hot threshold
    cleanup_interval_secs: 30,  // Frequent cleanup
    stats_interval_secs: 120,   // Frequent monitoring
    enable_invalidation: true,  // Critical for consistency
    ..Default::default()
};
```

### Read-Heavy Workload

```rust
let read_heavy_config = CacheConfig {
    ttl_secs: 1800,             // Long TTL
    max_size: 300_000,          // Large cache
    hot_threshold: 2,           // Low hot threshold
    cleanup_interval_secs: 300, // Less frequent cleanup
    eviction_percentage: 3,     // Small eviction batches
    enable_prefetch: true,      // Aggressive prefetching
    ..Default::default()
};
```

## Integration Examples

### Basic Integration

```rust
use mooseng_master::cache_config::CacheConfig;
use mooseng_master::cache_enhanced::EnhancedMetadataCache;

// Load configuration
let config = CacheConfig::default();
config.validate().expect("Invalid cache configuration");

// Create cache with configuration
let cache = Arc::new(EnhancedMetadataCache::new(
    config.ttl(),
    config.max_size,
    config.hot_threshold,
));

// Start maintenance if enabled
if config.enable_lru {
    cache.clone().start_maintenance();
}
```

### Configuration from File

```rust
use serde_json;
use std::fs;

// Load from JSON file
let config_json = fs::read_to_string("cache_config.json")?;
let config: CacheConfig = serde_json::from_str(&config_json)?;

// Validate before use
config.validate()?;
```

Example `cache_config.json`:
```json
{
  "ttl_secs": 600,
  "max_size": 200000,
  "hot_threshold": 8,
  "enable_lru": true,
  "enable_prefetch": true,
  "enable_invalidation": true,
  "cleanup_interval_secs": 120,
  "stats_interval_secs": 300,
  "eviction_percentage": 5,
  "enable_compression": false,
  "compression_threshold": 4096
}
```

### Dynamic Configuration Updates

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

struct DynamicCacheManager {
    config: Arc<RwLock<CacheConfig>>,
    cache: Arc<EnhancedMetadataCache>,
}

impl DynamicCacheManager {
    pub async fn update_config(&self, new_config: CacheConfig) -> Result<(), String> {
        // Validate new configuration
        new_config.validate()?;
        
        // Update stored configuration
        let mut config = self.config.write().await;
        *config = new_config;
        
        // Note: Cache instance parameters cannot be changed at runtime
        // Would need to create new cache instance for size/TTL changes
        
        Ok(())
    }
    
    pub async fn get_current_metrics(&self) -> CacheMetrics {
        let config = self.config.read().await;
        let stats = self.cache.stats().get_stats();
        
        CacheMetrics::calculate(
            stats["hits"],
            stats["misses"], 
            stats["evictions"],
            self.cache.inodes.len(), // Would need accessor method
            stats["hot_entries"] as usize,
            config.max_size,
            0, // Would need size tracking
        )
    }
}
```

## Performance Tuning Guidelines

### Memory Tuning

1. **Monitor Memory Usage**:
   ```rust
   let memory_mb = (config.max_size * 400) / (1024 * 1024);
   if memory_mb > available_memory_mb * 0.3 {
       println!("Warning: Cache may use too much memory");
   }
   ```

2. **Adjust Cache Size**:
   - Start with 10-20% of available memory
   - Monitor `cache_utilization` metric
   - Increase if utilization consistently > 80%

### Performance Tuning

1. **Hit Rate Optimization**:
   - Target hit rate > 70%
   - Increase `max_size` if hit rate < 50%
   - Increase `ttl_secs` for stable workloads

2. **Eviction Tuning**:
   - Monitor `eviction_rate` metric
   - Increase `max_size` if eviction rate > 20%
   - Adjust `hot_threshold` based on access patterns

3. **Cleanup Optimization**:
   - Balance `cleanup_interval_secs` vs CPU usage
   - Shorter intervals for write-heavy workloads
   - Longer intervals for read-heavy workloads

## Best Practices

1. **Always Validate**: Call `config.validate()` before using configuration
2. **Monitor Health**: Regularly check `metrics.is_healthy()`
3. **Follow Recommendations**: Implement suggested optimizations from `metrics.recommendations()`
4. **Test Configuration**: Use different configs for different environments
5. **Document Changes**: Keep configuration changes in version control
6. **Monitor Impact**: Watch performance metrics after configuration changes

## Troubleshooting

### Common Issues

#### Low Hit Rate
```rust
if metrics.hit_rate < 0.5 {
    // Consider increasing cache size or TTL
    let new_config = CacheConfig {
        max_size: config.max_size * 2,
        ttl_secs: config.ttl_secs * 2,
        ..config
    };
}
```

#### High Memory Usage
```rust
if metrics.cache_utilization > 95.0 {
    // Reduce cache size or increase eviction rate
    let new_config = CacheConfig {
        max_size: config.max_size / 2,
        eviction_percentage: config.eviction_percentage * 2,
        ..config
    };
}
```

#### High Eviction Rate
```rust
if metrics.eviction_rate > 0.2 {
    // Cache too small for workload
    let new_config = CacheConfig {
        max_size: config.max_size * 2,
        ..config
    };
}
```

### Debugging Configuration

```rust
// Log current configuration
println!("Cache Configuration:");
println!("  TTL: {:?}", config.ttl());
println!("  Max Size: {}", config.max_size);
println!("  Hot Threshold: {}", config.hot_threshold);
println!("  Eviction %: {}", config.eviction_percentage);

// Log performance metrics
let metrics = get_current_metrics();
println!("Performance Metrics:");
println!("  Hit Rate: {:.2}%", metrics.hit_rate * 100.0);
println!("  Utilization: {:.2}%", metrics.cache_utilization);
println!("  Hot Entries: {:.2}%", metrics.hot_entry_percentage);
```

This comprehensive configuration system ensures optimal cache performance across diverse workloads and deployment scenarios.