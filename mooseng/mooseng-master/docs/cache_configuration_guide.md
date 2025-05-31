# Cache Configuration Guide

## Overview

The Cache Configuration system (`cache_config.rs`) provides a comprehensive configuration framework for tuning MooseNG's metadata cache performance. It includes validation, metrics calculation, and performance recommendations.

## Configuration Structure

### CacheConfig

The main configuration struct with serialization support:

```rust
use mooseng_master::cache_config::CacheConfig;
use std::time::Duration;

// Create default configuration
let config = CacheConfig::default();

// Create custom configuration
let config = CacheConfig {
    ttl_secs: 600,              // 10 minutes
    max_size: 500_000,          // 500k entries
    hot_threshold: 8,           // 8 accesses to be "hot"
    enable_lru: true,
    enable_prefetch: true,
    enable_invalidation: true,
    cleanup_interval_secs: 30,   // 30 second cleanup
    stats_interval_secs: 120,    // 2 minute stats
    eviction_percentage: 10,     // Evict 10% when full
    enable_compression: false,
    compression_threshold: 8192, // 8KB threshold
};

// Validate configuration
assert!(config.validate().is_ok());
```

## Configuration Parameters

### Core Cache Settings

#### `ttl_secs` (Time-To-Live)
- **Default**: 300 seconds (5 minutes)
- **Range**: > 0
- **Purpose**: How long entries remain valid before expiring
- **Tuning**:
  - **Low values (60-300s)**: Fast-changing metadata, write-heavy workloads
  - **High values (600-3600s)**: Stable metadata, read-heavy workloads

```rust
// For write-heavy workloads
let config = CacheConfig {
    ttl_secs: 60,  // 1 minute - data changes frequently
    ..Default::default()
};

// For read-heavy workloads
let config = CacheConfig {
    ttl_secs: 1800,  // 30 minutes - data is stable
    ..Default::default()
};
```

#### `max_size` (Maximum Entries)
- **Default**: 100,000 entries
- **Range**: > 0
- **Purpose**: Maximum number of cached entries
- **Memory Impact**: ~200 bytes overhead per entry + data size

```rust
// Calculate memory usage
fn estimate_memory_usage(config: &CacheConfig, avg_entry_size: usize) -> usize {
    config.max_size * (200 + avg_entry_size) // bytes
}

// For high-memory systems
let high_memory_config = CacheConfig {
    max_size: 1_000_000,  // 1M entries (~1GB+ RAM)
    ..Default::default()
};

// For memory-constrained systems
let low_memory_config = CacheConfig {
    max_size: 10_000,     // 10k entries (~10MB RAM)
    ..Default::default()
};
```

#### `hot_threshold` (Hot Entry Threshold)
- **Default**: 5 accesses
- **Range**: > 0
- **Purpose**: Number of accesses before entry is considered "hot" and protected from LRU eviction

```rust
// For workloads with clear hot/cold patterns
let hot_cold_config = CacheConfig {
    hot_threshold: 3,  // Low threshold - protect frequently accessed data
    ..Default::default()
};

// For uniform access patterns
let uniform_config = CacheConfig {
    hot_threshold: 15, // High threshold - most data eligible for eviction
    ..Default::default()
};
```

### Feature Toggles

#### LRU Eviction (`enable_lru`)
- **Default**: true
- **Purpose**: Enable least-recently-used eviction policy
- **Disable when**: Using external cache management or testing

#### Prefetching (`enable_prefetch`)
- **Default**: true
- **Purpose**: Enable cache warming and prefetching
- **Disable when**: Memory-constrained or simple access patterns

#### Distributed Invalidation (`enable_invalidation`)
- **Default**: true
- **Purpose**: Enable pub/sub cache invalidation across cluster
- **Disable when**: Single-node deployment or external invalidation

### Maintenance Settings

#### `cleanup_interval_secs` (Cleanup Frequency)
- **Default**: 60 seconds
- **Purpose**: How often to clean up expired entries
- **Tuning**:
  - **Shorter intervals**: More CPU overhead, faster memory reclamation
  - **Longer intervals**: Less overhead, slower memory reclamation

#### `stats_interval_secs` (Statistics Reporting)
- **Default**: 300 seconds (5 minutes)
- **Purpose**: How often to log cache statistics
- **Production**: 300-900 seconds
- **Development**: 30-60 seconds

#### `eviction_percentage` (Eviction Batch Size)
- **Default**: 5%
- **Range**: 0-100
- **Purpose**: Percentage of cache to evict when full
- **Tuning**:
  - **Low values (1-5%)**: Gradual eviction, less performance impact
  - **High values (10-20%)**: Aggressive eviction, fewer eviction cycles

### Advanced Settings

#### Compression (`enable_compression`, `compression_threshold`)
- **Default**: Disabled, 4KB threshold
- **Purpose**: Compress large cache entries to save memory
- **Trade-off**: CPU overhead vs memory savings

```rust
// Enable compression for large entries
let compression_config = CacheConfig {
    enable_compression: true,
    compression_threshold: 2048,  // Compress entries > 2KB
    ..Default::default()
};
```

## Configuration Examples

### Development Environment
```rust
let dev_config = CacheConfig {
    ttl_secs: 60,                    // Short TTL for testing
    max_size: 1_000,                 // Small cache
    hot_threshold: 2,                // Low threshold
    cleanup_interval_secs: 10,       // Frequent cleanup
    stats_interval_secs: 30,         // Frequent stats
    eviction_percentage: 20,         // Aggressive eviction
    ..Default::default()
};
```

### Production High-Performance
```rust
let prod_config = CacheConfig {
    ttl_secs: 900,                   // 15 minute TTL
    max_size: 2_000_000,             // 2M entries
    hot_threshold: 5,                // Balanced threshold
    cleanup_interval_secs: 60,       // Standard cleanup
    stats_interval_secs: 300,        // Standard reporting
    eviction_percentage: 3,          // Gentle eviction
    enable_compression: true,
    compression_threshold: 4096,
};
```

### Memory-Constrained Environment
```rust
let constrained_config = CacheConfig {
    ttl_secs: 120,                   // Short TTL
    max_size: 5_000,                 // Very small cache
    hot_threshold: 8,                // High threshold
    cleanup_interval_secs: 30,       // Frequent cleanup
    eviction_percentage: 15,         // Aggressive eviction
    enable_compression: true,
    compression_threshold: 1024,     // Compress > 1KB
    ..Default::default()
};
```

### Read-Heavy Workload
```rust
let read_heavy_config = CacheConfig {
    ttl_secs: 1800,                  // Long TTL (30 min)
    max_size: 1_000_000,             // Large cache
    hot_threshold: 3,                // Low threshold (protect more data)
    cleanup_interval_secs: 120,      // Less frequent cleanup
    eviction_percentage: 2,          // Gentle eviction
    ..Default::default()
};
```

### Write-Heavy Workload
```rust
let write_heavy_config = CacheConfig {
    ttl_secs: 60,                    // Short TTL (1 min)
    max_size: 100_000,               // Moderate cache
    hot_threshold: 10,               // High threshold
    cleanup_interval_secs: 30,       // Frequent cleanup
    eviction_percentage: 10,         // Moderate eviction
    ..Default::default()
};
```

## Configuration Loading

### From File (JSON)
```rust
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
  "max_size": 500000,
  "hot_threshold": 5,
  "enable_lru": true,
  "enable_prefetch": true,
  "enable_invalidation": true,
  "cleanup_interval_secs": 60,
  "stats_interval_secs": 300,
  "eviction_percentage": 5,
  "enable_compression": false,
  "compression_threshold": 4096
}
```

### From Environment Variables
```rust
use std::env;

fn config_from_env() -> CacheConfig {
    CacheConfig {
        ttl_secs: env::var("CACHE_TTL_SECS")
            .unwrap_or_else(|_| "300".to_string())
            .parse().unwrap_or(300),
        max_size: env::var("CACHE_MAX_SIZE")
            .unwrap_or_else(|_| "100000".to_string())
            .parse().unwrap_or(100_000),
        // ... other fields
        ..Default::default()
    }
}
```

### Runtime Configuration
```rust
use mooseng_master::cache_config::CacheConfig;

pub struct ConfigManager {
    config: CacheConfig,
}

impl ConfigManager {
    pub fn update_config(&mut self, new_config: CacheConfig) -> Result<(), String> {
        // Validate new configuration
        new_config.validate()?;
        
        // Apply configuration
        self.config = new_config;
        Ok(())
    }
    
    pub fn get_config(&self) -> &CacheConfig {
        &self.config
    }
}
```

## Performance Metrics

### CacheMetrics

Monitor cache performance and get optimization recommendations:

```rust
use mooseng_master::cache_config::CacheMetrics;

// Calculate metrics from cache statistics
let metrics = CacheMetrics::calculate(
    800,    // hits
    200,    // misses  
    50,     // evictions
    10000,  // total entries
    500,    // hot entries
    50000,  // max size
    10485760, // total size bytes (10MB)
);

// Check cache health
if metrics.is_healthy() {
    println!("Cache is performing well");
} else {
    println!("Cache needs optimization");
}

// Get recommendations
for recommendation in metrics.recommendations() {
    println!("Recommendation: {}", recommendation);
}
```

### Key Metrics

#### Hit Rate
- **Target**: > 70%
- **Low hit rate causes**: Cache too small, TTL too short, changing workload

#### Eviction Rate  
- **Target**: < 10%
- **High eviction rate causes**: Cache too small for workload

#### Cache Utilization
- **Target**: 60-90%
- **High utilization (>95%)**: Increase cache size
- **Low utilization (<30%)**: Reduce cache size

#### Hot Entry Percentage
- **Typical**: 10-30%
- **High percentage (>50%)**: Consider two-tier caching

### Automated Recommendations

The system provides specific optimization suggestions:

```rust
let recommendations = metrics.recommendations();

// Example recommendations:
// - "Low hit rate detected. Consider increasing cache size or TTL."
// - "High eviction rate. Cache may be too small for workload."
// - "Cache nearly full. Consider increasing max_size."
// - "Many hot entries. Consider implementing a two-tier cache."
// - "Large average entry size. Consider enabling compression."
```

## Monitoring and Alerting

### Integration with Monitoring Systems

```rust
use mooseng_master::cache_config::{CacheConfig, CacheMetrics};

pub struct CacheMonitor {
    config: CacheConfig,
}

impl CacheMonitor {
    pub fn check_health(&self, metrics: &CacheMetrics) -> Vec<Alert> {
        let mut alerts = Vec::new();
        
        if metrics.hit_rate < 0.5 {
            alerts.push(Alert::LowHitRate(metrics.hit_rate));
        }
        
        if metrics.cache_utilization > 95.0 {
            alerts.push(Alert::HighMemoryUsage(metrics.cache_utilization));
        }
        
        if metrics.eviction_rate > 0.2 {
            alerts.push(Alert::HighEvictionRate(metrics.eviction_rate));
        }
        
        alerts
    }
}

#[derive(Debug)]
enum Alert {
    LowHitRate(f64),
    HighMemoryUsage(f64),
    HighEvictionRate(f64),
}
```

## Best Practices

### Configuration Management
1. **Use version control** for configuration files
2. **Validate configurations** before deployment
3. **Monitor metrics** after configuration changes
4. **Document rationale** for configuration choices

### Performance Tuning
1. **Start with defaults** and measure performance
2. **Adjust one parameter at a time**
3. **Monitor for 24-48 hours** before further changes
4. **Use A/B testing** for configuration validation

### Production Deployment
1. **Enable all features** by default
2. **Set conservative eviction percentage** (3-5%)
3. **Use appropriate TTL** for data volatility
4. **Monitor cache metrics** continuously

### Common Pitfalls
1. **TTL too short**: Reduces hit rate, increases storage load
2. **Cache too small**: High eviction rate, poor performance
3. **Hot threshold too low**: Protects too much data from eviction
4. **Cleanup too frequent**: Unnecessary CPU overhead
5. **Missing validation**: Runtime errors from invalid config

## Testing Configuration

```rust
#[cfg(test)]
mod config_tests {
    use super::*;
    
    #[test]
    fn test_production_config() {
        let config = CacheConfig {
            ttl_secs: 600,
            max_size: 1_000_000,
            hot_threshold: 5,
            ..Default::default()
        };
        
        assert!(config.validate().is_ok());
        assert_eq!(config.eviction_count(), 50_000); // 5% of 1M
    }
    
    #[test]
    fn test_memory_usage_calculation() {
        let config = CacheConfig {
            max_size: 100_000,
            ..Default::default()
        };
        
        let estimated_memory = config.max_size * 1024; // 1KB per entry
        assert!(estimated_memory < 200_000_000); // < 200MB
    }
}
```

This configuration system provides the flexibility to optimize cache performance for any workload while maintaining simplicity and safety through validation and monitoring.