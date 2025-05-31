use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the metadata cache system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Time-to-live for cache entries in seconds
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
    
    /// Maximum number of entries in the cache
    #[serde(default = "default_max_size")]
    pub max_size: usize,
    
    /// Number of accesses before an entry is considered "hot"
    #[serde(default = "default_hot_threshold")]
    pub hot_threshold: usize,
    
    /// Enable LRU eviction policy
    #[serde(default = "default_enable_lru")]
    pub enable_lru: bool,
    
    /// Enable cache warming/prefetching
    #[serde(default = "default_enable_prefetch")]
    pub enable_prefetch: bool,
    
    /// Enable distributed cache invalidation
    #[serde(default = "default_enable_invalidation")]
    pub enable_invalidation: bool,
    
    /// Cleanup interval in seconds
    #[serde(default = "default_cleanup_interval_secs")]
    pub cleanup_interval_secs: u64,
    
    /// Stats reporting interval in seconds
    #[serde(default = "default_stats_interval_secs")]
    pub stats_interval_secs: u64,
    
    /// Percentage of cache to evict when full (0-100)
    #[serde(default = "default_eviction_percentage")]
    pub eviction_percentage: u8,
    
    /// Enable cache compression for large entries
    #[serde(default = "default_enable_compression")]
    pub enable_compression: bool,
    
    /// Minimum entry size for compression in bytes
    #[serde(default = "default_compression_threshold")]
    pub compression_threshold: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl_secs: default_ttl_secs(),
            max_size: default_max_size(),
            hot_threshold: default_hot_threshold(),
            enable_lru: default_enable_lru(),
            enable_prefetch: default_enable_prefetch(),
            enable_invalidation: default_enable_invalidation(),
            cleanup_interval_secs: default_cleanup_interval_secs(),
            stats_interval_secs: default_stats_interval_secs(),
            eviction_percentage: default_eviction_percentage(),
            enable_compression: default_enable_compression(),
            compression_threshold: default_compression_threshold(),
        }
    }
}

impl CacheConfig {
    /// Get the TTL as a Duration
    pub fn ttl(&self) -> Duration {
        Duration::from_secs(self.ttl_secs)
    }
    
    /// Get the cleanup interval as a Duration
    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_secs(self.cleanup_interval_secs)
    }
    
    /// Get the stats interval as a Duration
    pub fn stats_interval(&self) -> Duration {
        Duration::from_secs(self.stats_interval_secs)
    }
    
    /// Calculate the number of entries to evict based on percentage
    pub fn eviction_count(&self) -> usize {
        (self.max_size as f64 * self.eviction_percentage as f64 / 100.0) as usize
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.ttl_secs == 0 {
            return Err("TTL must be greater than 0".to_string());
        }
        
        if self.max_size == 0 {
            return Err("Max size must be greater than 0".to_string());
        }
        
        if self.eviction_percentage > 100 {
            return Err("Eviction percentage must be between 0 and 100".to_string());
        }
        
        if self.hot_threshold == 0 {
            return Err("Hot threshold must be greater than 0".to_string());
        }
        
        Ok(())
    }
}

// Default value functions for serde

fn default_ttl_secs() -> u64 {
    300 // 5 minutes
}

fn default_max_size() -> usize {
    100_000 // 100k entries
}

fn default_hot_threshold() -> usize {
    5 // 5 accesses to be considered hot
}

fn default_enable_lru() -> bool {
    true
}

fn default_enable_prefetch() -> bool {
    true
}

fn default_enable_invalidation() -> bool {
    true
}

fn default_cleanup_interval_secs() -> u64 {
    60 // 1 minute
}

fn default_stats_interval_secs() -> u64 {
    300 // 5 minutes
}

fn default_eviction_percentage() -> u8 {
    5 // Evict 5% when full
}

fn default_enable_compression() -> bool {
    false // Disabled by default
}

fn default_compression_threshold() -> usize {
    4096 // 4KB
}

/// Cache performance metrics
#[derive(Debug, Clone, Default)]
pub struct CacheMetrics {
    pub hit_rate: f64,
    pub miss_rate: f64,
    pub eviction_rate: f64,
    pub avg_entry_size: usize,
    pub hot_entry_percentage: f64,
    pub cache_utilization: f64,
}

impl CacheMetrics {
    /// Calculate metrics from raw statistics
    pub fn calculate(
        hits: u64,
        misses: u64,
        evictions: u64,
        total_entries: usize,
        hot_entries: usize,
        max_size: usize,
        total_size_bytes: usize,
    ) -> Self {
        let total_requests = hits + misses;
        let hit_rate = if total_requests > 0 {
            hits as f64 / total_requests as f64
        } else {
            0.0
        };
        
        let miss_rate = if total_requests > 0 {
            misses as f64 / total_requests as f64
        } else {
            0.0
        };
        
        let eviction_rate = if total_requests > 0 {
            evictions as f64 / total_requests as f64
        } else {
            0.0
        };
        
        let avg_entry_size = if total_entries > 0 {
            total_size_bytes / total_entries
        } else {
            0
        };
        
        let hot_entry_percentage = if total_entries > 0 {
            hot_entries as f64 / total_entries as f64 * 100.0
        } else {
            0.0
        };
        
        let cache_utilization = if max_size > 0 {
            total_entries as f64 / max_size as f64 * 100.0
        } else {
            0.0
        };
        
        Self {
            hit_rate,
            miss_rate,
            eviction_rate,
            avg_entry_size,
            hot_entry_percentage,
            cache_utilization,
        }
    }
    
    /// Check if cache performance is healthy
    pub fn is_healthy(&self) -> bool {
        self.hit_rate > 0.7 && // At least 70% hit rate
        self.eviction_rate < 0.1 && // Less than 10% evictions
        self.cache_utilization < 95.0 // Not overly full
    }
    
    /// Get performance recommendations
    pub fn recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if self.hit_rate < 0.5 {
            recommendations.push(
                "Low hit rate detected. Consider increasing cache size or TTL.".to_string()
            );
        }
        
        if self.eviction_rate > 0.2 {
            recommendations.push(
                "High eviction rate. Cache may be too small for workload.".to_string()
            );
        }
        
        if self.cache_utilization > 90.0 {
            recommendations.push(
                "Cache nearly full. Consider increasing max_size.".to_string()
            );
        }
        
        if self.hot_entry_percentage > 50.0 {
            recommendations.push(
                "Many hot entries. Consider implementing a two-tier cache.".to_string()
            );
        }
        
        if self.avg_entry_size > 10240 { // 10KB
            recommendations.push(
                "Large average entry size. Consider enabling compression.".to_string()
            );
        }
        
        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.ttl_secs, 300);
        assert_eq!(config.max_size, 100_000);
        assert!(config.enable_lru);
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_cache_config_validation() {
        let mut config = CacheConfig::default();
        
        config.ttl_secs = 0;
        assert!(config.validate().is_err());
        
        config.ttl_secs = 300;
        config.eviction_percentage = 101;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_cache_metrics() {
        let metrics = CacheMetrics::calculate(
            700,  // hits
            300,  // misses
            50,   // evictions
            8000, // total entries
            400,  // hot entries
            10000, // max size
            8192000, // total size bytes
        );
        
        assert_eq!(metrics.hit_rate, 0.7);
        assert_eq!(metrics.miss_rate, 0.3);
        assert_eq!(metrics.eviction_rate, 0.05);
        assert_eq!(metrics.avg_entry_size, 1024);
        assert_eq!(metrics.hot_entry_percentage, 5.0);
        assert_eq!(metrics.cache_utilization, 80.0);
        assert!(metrics.is_healthy());
    }
    
    #[test]
    fn test_performance_recommendations() {
        let metrics = CacheMetrics {
            hit_rate: 0.3,
            miss_rate: 0.7,
            eviction_rate: 0.25,
            avg_entry_size: 15000,
            hot_entry_percentage: 60.0,
            cache_utilization: 95.0,
        };
        
        let recommendations = metrics.recommendations();
        assert!(!recommendations.is_empty());
        assert!(recommendations.iter().any(|r| r.contains("Low hit rate")));
        assert!(recommendations.iter().any(|r| r.contains("High eviction rate")));
        assert!(recommendations.iter().any(|r| r.contains("Cache nearly full")));
        assert!(recommendations.iter().any(|r| r.contains("hot entries")));
        assert!(recommendations.iter().any(|r| r.contains("compression")));
    }
    
    #[test]
    fn test_cache_config_durations() {
        let config = CacheConfig::default();
        
        assert_eq!(config.ttl(), Duration::from_secs(300));
        assert_eq!(config.cleanup_interval(), Duration::from_secs(60));
        assert_eq!(config.stats_interval(), Duration::from_secs(300));
    }
    
    #[test]
    fn test_eviction_count_calculation() {
        let config = CacheConfig {
            max_size: 1000,
            eviction_percentage: 10,
            ..Default::default()
        };
        
        assert_eq!(config.eviction_count(), 100);
        
        let config2 = CacheConfig {
            max_size: 1000,
            eviction_percentage: 5,
            ..Default::default()
        };
        
        assert_eq!(config2.eviction_count(), 50);
    }
    
    #[test]
    fn test_cache_config_edge_cases() {
        let mut config = CacheConfig::default();
        
        // Test hot threshold validation
        config.hot_threshold = 0;
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("Hot threshold"));
        
        // Test max size validation
        config.hot_threshold = 5;
        config.max_size = 0;
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("Max size"));
    }
    
    #[test]
    fn test_cache_metrics_edge_cases() {
        // Test with zero values
        let metrics = CacheMetrics::calculate(0, 0, 0, 0, 0, 0, 0);
        assert_eq!(metrics.hit_rate, 0.0);
        assert_eq!(metrics.miss_rate, 0.0);
        assert_eq!(metrics.eviction_rate, 0.0);
        assert_eq!(metrics.avg_entry_size, 0);
        assert_eq!(metrics.hot_entry_percentage, 0.0);
        assert_eq!(metrics.cache_utilization, 0.0);
        
        // Test healthy cache
        let healthy_metrics = CacheMetrics::calculate(
            800, 200, 10, 5000, 100, 10000, 5120000
        );
        assert!(healthy_metrics.is_healthy());
        
        // Test unhealthy cache (low hit rate)
        let unhealthy_metrics = CacheMetrics::calculate(
            200, 800, 150, 9500, 100, 10000, 5120000
        );
        assert!(!unhealthy_metrics.is_healthy());
    }
    
    #[test]
    fn test_serde_serialization() {
        let config = CacheConfig::default();
        
        // Test that the config can be serialized and deserialized
        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        assert!(!serialized.is_empty());
        
        let deserialized: CacheConfig = serde_json::from_str(&serialized)
            .expect("Failed to deserialize");
        
        assert_eq!(config.ttl_secs, deserialized.ttl_secs);
        assert_eq!(config.max_size, deserialized.max_size);
        assert_eq!(config.hot_threshold, deserialized.hot_threshold);
    }
}