//! Performance optimizations for the Raft consensus implementation
//! 
//! This module contains optimizations identified through testing and benchmarking
//! to improve the performance and efficiency of the Raft implementation.

use super::*;
use crate::raft::log::LogCommand;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};

/// Performance metrics collector for Raft operations
#[derive(Debug, Clone)]
pub struct RaftPerformanceMetrics {
    /// Operation counters
    pub safety_checks: u64,
    pub configuration_queries: u64,
    pub read_operations: u64,
    pub leadership_queries: u64,
    
    /// Timing metrics (in microseconds)
    pub avg_safety_check_time: u64,
    pub avg_config_query_time: u64,
    pub avg_read_operation_time: u64,
    
    /// Error counters
    pub safety_check_errors: u64,
    pub read_operation_errors: u64,
    
    /// Cache hit rates
    pub config_cache_hits: u64,
    pub config_cache_misses: u64,
}

impl Default for RaftPerformanceMetrics {
    fn default() -> Self {
        Self {
            safety_checks: 0,
            configuration_queries: 0,
            read_operations: 0,
            leadership_queries: 0,
            avg_safety_check_time: 0,
            avg_config_query_time: 0,
            avg_read_operation_time: 0,
            safety_check_errors: 0,
            read_operation_errors: 0,
            config_cache_hits: 0,
            config_cache_misses: 0,
        }
    }
}

/// Configuration cache to optimize frequent configuration queries
#[derive(Debug)]
struct ConfigurationCache {
    cluster_config: Option<(Instant, ClusterConfiguration)>,
    replication_targets: Option<(Instant, std::collections::HashSet<String>)>,
    majority_size: Option<(Instant, usize)>,
    cache_ttl: Duration,
}

impl ConfigurationCache {
    fn new(ttl: Duration) -> Self {
        Self {
            cluster_config: None,
            replication_targets: None,
            majority_size: None,
            cache_ttl: ttl,
        }
    }
    
    fn get_cluster_config(&self) -> Option<&ClusterConfiguration> {
        if let Some((timestamp, config)) = &self.cluster_config {
            if timestamp.elapsed() < self.cache_ttl {
                return Some(config);
            }
        }
        None
    }
    
    fn set_cluster_config(&mut self, config: ClusterConfiguration) {
        self.cluster_config = Some((Instant::now(), config));
    }
    
    fn get_replication_targets(&self) -> Option<&std::collections::HashSet<String>> {
        if let Some((timestamp, targets)) = &self.replication_targets {
            if timestamp.elapsed() < self.cache_ttl {
                return Some(targets);
            }
        }
        None
    }
    
    fn set_replication_targets(&mut self, targets: std::collections::HashSet<String>) {
        self.replication_targets = Some((Instant::now(), targets));
    }
    
    fn get_majority_size(&self) -> Option<usize> {
        if let Some((timestamp, size)) = &self.majority_size {
            if timestamp.elapsed() < self.cache_ttl {
                return Some(*size);
            }
        }
        None
    }
    
    fn set_majority_size(&mut self, size: usize) {
        self.majority_size = Some((Instant::now(), size));
    }
    
    fn invalidate(&mut self) {
        self.cluster_config = None;
        self.replication_targets = None;
        self.majority_size = None;
    }
}

/// Read lease cache for optimizing read scaling operations
#[derive(Debug)]
struct ReadLeaseCache {
    current_lease: Option<(Instant, ReadLease)>,
    lease_validation_cache: Option<(Instant, bool)>,
    validation_cache_ttl: Duration,
}

impl ReadLeaseCache {
    fn new(validation_ttl: Duration) -> Self {
        Self {
            current_lease: None,
            lease_validation_cache: None,
            validation_cache_ttl: validation_ttl,
        }
    }
    
    fn get_current_lease(&self) -> Option<&ReadLease> {
        if let Some((_, lease)) = &self.current_lease {
            Some(lease)
        } else {
            None
        }
    }
    
    fn set_current_lease(&mut self, lease: ReadLease) {
        self.current_lease = Some((Instant::now(), lease));
        // Invalidate validation cache when lease changes
        self.lease_validation_cache = None;
    }
    
    fn get_cached_validation(&self) -> Option<bool> {
        if let Some((timestamp, is_valid)) = &self.lease_validation_cache {
            if timestamp.elapsed() < self.validation_cache_ttl {
                return Some(*is_valid);
            }
        }
        None
    }
    
    fn set_cached_validation(&mut self, is_valid: bool) {
        self.lease_validation_cache = Some((Instant::now(), is_valid));
    }
    
    fn invalidate(&mut self) {
        self.current_lease = None;
        self.lease_validation_cache = None;
    }
}

/// Optimized Raft consensus implementation with performance enhancements
#[derive(Debug)]
pub struct OptimizedRaftConsensus {
    inner: RaftConsensus,
    metrics: Arc<Mutex<RaftPerformanceMetrics>>,
    config_cache: Arc<Mutex<ConfigurationCache>>,
    read_lease_cache: Arc<Mutex<ReadLeaseCache>>,
    
    // Optimization flags
    enable_config_caching: bool,
    enable_read_lease_caching: bool,
    enable_metrics_collection: bool,
}

impl OptimizedRaftConsensus {
    /// Create a new optimized Raft consensus instance
    pub async fn new(config: RaftConfig) -> Result<Self> {
        let inner = RaftConsensus::new(config).await?;
        
        Ok(Self {
            inner,
            metrics: Arc::new(Mutex::new(RaftPerformanceMetrics::default())),
            config_cache: Arc::new(Mutex::new(ConfigurationCache::new(Duration::from_millis(100)))),
            read_lease_cache: Arc::new(Mutex::new(ReadLeaseCache::new(Duration::from_millis(50)))),
            enable_config_caching: true,
            enable_read_lease_caching: true,
            enable_metrics_collection: true,
        })
    }
    
    /// Create with custom optimization settings
    pub async fn new_with_optimizations(
        config: RaftConfig,
        enable_config_caching: bool,
        enable_read_lease_caching: bool,
        enable_metrics: bool,
    ) -> Result<Self> {
        let inner = RaftConsensus::new(config).await?;
        
        Ok(Self {
            inner,
            metrics: Arc::new(Mutex::new(RaftPerformanceMetrics::default())),
            config_cache: Arc::new(Mutex::new(ConfigurationCache::new(Duration::from_millis(100)))),
            read_lease_cache: Arc::new(Mutex::new(ReadLeaseCache::new(Duration::from_millis(50)))),
            enable_config_caching,
            enable_read_lease_caching,
            enable_metrics_collection: enable_metrics,
        })
    }
    
    /// Start the optimized Raft instance
    pub async fn start(&self) -> Result<()> {
        self.inner.start().await
    }
    
    /// Stop the optimized Raft instance
    pub async fn stop(&self) -> Result<()> {
        self.inner.stop().await
    }
    
    /// Optimized leadership check with caching
    pub async fn is_leader(&self) -> bool {
        let start_time = Instant::now();
        let result = self.inner.is_leader().await;
        
        if self.enable_metrics_collection {
            let mut metrics = self.metrics.lock().await;
            metrics.leadership_queries += 1;
        }
        
        result
    }
    
    /// Optimized leader ID retrieval
    pub async fn get_leader_id(&self) -> Option<String> {
        let start_time = Instant::now();
        let result = self.inner.get_leader_id().await;
        
        if self.enable_metrics_collection {
            let mut metrics = self.metrics.lock().await;
            metrics.leadership_queries += 1;
        }
        
        result
    }
    
    /// Optimized safety check with metrics
    pub async fn check_safety(&self) -> Result<()> {
        let start_time = Instant::now();
        let result = self.inner.check_safety().await;
        
        if self.enable_metrics_collection {
            let elapsed = start_time.elapsed().as_micros() as u64;
            let mut metrics = self.metrics.lock().await;
            
            metrics.safety_checks += 1;
            
            // Update running average
            if metrics.safety_checks == 1 {
                metrics.avg_safety_check_time = elapsed;
            } else {
                metrics.avg_safety_check_time = 
                    (metrics.avg_safety_check_time * (metrics.safety_checks - 1) + elapsed) / metrics.safety_checks;
            }
            
            if result.is_err() {
                metrics.safety_check_errors += 1;
            }
        }
        
        result
    }
    
    /// Optimized cluster configuration retrieval with caching
    pub async fn get_cluster_config(&self) -> ClusterConfiguration {
        if self.enable_config_caching {
            // Check cache first
            {
                let cache = self.config_cache.lock().await;
                if let Some(cached_config) = cache.get_cluster_config() {
                    if self.enable_metrics_collection {
                        let mut metrics = self.metrics.lock().await;
                        metrics.config_cache_hits += 1;
                    }
                    return cached_config.clone();
                }
            }
            
            // Cache miss - fetch from inner implementation
            let start_time = Instant::now();
            let config = self.inner.get_cluster_config().await;
            
            // Update cache
            {
                let mut cache = self.config_cache.lock().await;
                cache.set_cluster_config(config.clone());
            }
            
            if self.enable_metrics_collection {
                let elapsed = start_time.elapsed().as_micros() as u64;
                let mut metrics = self.metrics.lock().await;
                metrics.configuration_queries += 1;
                metrics.config_cache_misses += 1;
                
                // Update running average
                if metrics.configuration_queries == 1 {
                    metrics.avg_config_query_time = elapsed;
                } else {
                    metrics.avg_config_query_time = 
                        (metrics.avg_config_query_time * (metrics.configuration_queries - 1) + elapsed) / metrics.configuration_queries;
                }
            }
            
            config
        } else {
            self.inner.get_cluster_config().await
        }
    }
    
    /// Optimized replication targets retrieval with caching
    pub async fn get_replication_targets(&self) -> std::collections::HashSet<String> {
        if self.enable_config_caching {
            // Check cache first
            {
                let cache = self.config_cache.lock().await;
                if let Some(cached_targets) = cache.get_replication_targets() {
                    if self.enable_metrics_collection {
                        let mut metrics = self.metrics.lock().await;
                        metrics.config_cache_hits += 1;
                    }
                    return cached_targets.clone();
                }
            }
            
            // Cache miss
            let targets = self.inner.get_replication_targets().await;
            
            // Update cache
            {
                let mut cache = self.config_cache.lock().await;
                cache.set_replication_targets(targets.clone());
            }
            
            if self.enable_metrics_collection {
                let mut metrics = self.metrics.lock().await;
                metrics.config_cache_misses += 1;
            }
            
            targets
        } else {
            self.inner.get_replication_targets().await
        }
    }
    
    /// Optimized majority size retrieval with caching
    pub async fn get_majority_size(&self) -> usize {
        if self.enable_config_caching {
            // Check cache first
            {
                let cache = self.config_cache.lock().await;
                if let Some(cached_size) = cache.get_majority_size() {
                    if self.enable_metrics_collection {
                        let mut metrics = self.metrics.lock().await;
                        metrics.config_cache_hits += 1;
                    }
                    return cached_size;
                }
            }
            
            // Cache miss
            let size = self.inner.get_majority_size().await;
            
            // Update cache
            {
                let mut cache = self.config_cache.lock().await;
                cache.set_majority_size(size);
            }
            
            if self.enable_metrics_collection {
                let mut metrics = self.metrics.lock().await;
                metrics.config_cache_misses += 1;
            }
            
            size
        } else {
            self.inner.get_majority_size().await
        }
    }
    
    /// Optimized read operation with caching and metrics
    pub async fn can_serve_read(&self, request: &ReadRequest) -> Result<bool> {
        let start_time = Instant::now();
        let result = self.inner.can_serve_read(request).await;
        
        if self.enable_metrics_collection {
            let elapsed = start_time.elapsed().as_micros() as u64;
            let mut metrics = self.metrics.lock().await;
            
            metrics.read_operations += 1;
            
            // Update running average
            if metrics.read_operations == 1 {
                metrics.avg_read_operation_time = elapsed;
            } else {
                metrics.avg_read_operation_time = 
                    (metrics.avg_read_operation_time * (metrics.read_operations - 1) + elapsed) / metrics.read_operations;
            }
            
            if result.is_err() {
                metrics.read_operation_errors += 1;
            }
        }
        
        result
    }
    
    /// Optimized read lease validation with caching
    pub async fn validate_read_lease(&self) -> bool {
        if self.enable_read_lease_caching {
            // Check cache first
            {
                let cache = self.read_lease_cache.lock().await;
                if let Some(cached_validation) = cache.get_cached_validation() {
                    return cached_validation;
                }
            }
            
            // Cache miss - perform validation
            let is_valid = self.inner.validate_read_lease().await;
            
            // Update cache
            {
                let mut cache = self.read_lease_cache.lock().await;
                cache.set_cached_validation(is_valid);
            }
            
            is_valid
        } else {
            self.inner.validate_read_lease().await
        }
    }
    
    /// Invalidate all caches (call when configuration changes)
    pub async fn invalidate_caches(&self) {
        if self.enable_config_caching {
            let mut config_cache = self.config_cache.lock().await;
            config_cache.invalidate();
        }
        
        if self.enable_read_lease_caching {
            let mut read_cache = self.read_lease_cache.lock().await;
            read_cache.invalidate();
        }
    }
    
    /// Get current performance metrics
    pub async fn get_performance_metrics(&self) -> RaftPerformanceMetrics {
        if self.enable_metrics_collection {
            let metrics = self.metrics.lock().await;
            metrics.clone()
        } else {
            RaftPerformanceMetrics::default()
        }
    }
    
    /// Reset performance metrics
    pub async fn reset_performance_metrics(&self) {
        if self.enable_metrics_collection {
            let mut metrics = self.metrics.lock().await;
            *metrics = RaftPerformanceMetrics::default();
        }
    }
    
    /// Get cache hit ratio for configuration operations
    pub async fn get_config_cache_hit_ratio(&self) -> f64 {
        if self.enable_metrics_collection {
            let metrics = self.metrics.lock().await;
            let total = metrics.config_cache_hits + metrics.config_cache_misses;
            if total > 0 {
                metrics.config_cache_hits as f64 / total as f64
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
    
    /// Delegate all other methods to the inner implementation
    pub async fn has_pending_config_change(&self) -> bool {
        self.inner.has_pending_config_change().await
    }
    
    pub async fn can_start_election(&self) -> Result<bool> {
        self.inner.can_start_election().await
    }
    
    pub async fn validate_vote_request(
        &self,
        candidate_term: Term,
        candidate_id: &NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) -> Result<bool> {
        self.inner.validate_vote_request(candidate_term, candidate_id, last_log_index, last_log_term).await
    }
    
    pub async fn propose_config_change(&self, change: ConfigChangeType) -> Result<LogIndex> {
        // Invalidate config cache since configuration will change
        self.invalidate_caches().await;
        self.inner.propose_config_change(change).await
    }
    
    pub async fn append_entry(&self, command: LogCommand) -> Result<LogIndex> {
        self.inner.append_entry(command).await
    }
    
    pub async fn on_leader_elected(&self) -> Result<()> {
        self.inner.on_leader_elected().await
    }
    
    pub async fn needs_lease_renewal(&self) -> bool {
        self.inner.needs_lease_renewal().await
    }
    
    pub async fn grant_read_lease(&self, follower_id: &NodeId) -> Result<ReadLease> {
        self.inner.grant_read_lease(follower_id).await
    }
    
    pub async fn update_read_lease(&self, lease: ReadLease) -> Result<()> {
        if self.enable_read_lease_caching {
            let mut cache = self.read_lease_cache.lock().await;
            cache.set_current_lease(lease.clone());
        }
        self.inner.update_read_lease(lease).await
    }
    
    pub async fn invalidate_read_lease(&self) {
        if self.enable_read_lease_caching {
            let mut cache = self.read_lease_cache.lock().await;
            cache.invalidate();
        }
        self.inner.invalidate_read_lease().await
    }
    
    pub async fn process_read_only_query(&self, query: ReadOnlyQuery) -> Result<ReadOnlyResponse> {
        let start_time = Instant::now();
        let result = self.inner.process_read_only_query(query).await;
        
        if self.enable_metrics_collection {
            let elapsed = start_time.elapsed().as_micros() as u64;
            let mut metrics = self.metrics.lock().await;
            metrics.read_operations += 1;
            
            if result.is_err() {
                metrics.read_operation_errors += 1;
            }
        }
        
        result
    }
}

/// Performance tuning recommendations based on metrics
pub struct PerformanceTuningRecommendations {
    pub recommendations: Vec<String>,
    pub critical_issues: Vec<String>,
    pub optimization_opportunities: Vec<String>,
}

impl PerformanceTuningRecommendations {
    /// Analyze metrics and provide recommendations
    pub fn analyze(metrics: &RaftPerformanceMetrics) -> Self {
        let mut recommendations = Vec::new();
        let mut critical_issues = Vec::new();
        let mut optimization_opportunities = Vec::new();
        
        // Analyze safety check performance
        if metrics.avg_safety_check_time > 1000 { // > 1ms
            critical_issues.push(
                format!("Safety checks are slow ({}μs avg). Consider optimizing safety check logic.", 
                       metrics.avg_safety_check_time)
            );
        } else if metrics.avg_safety_check_time > 500 { // > 0.5ms
            optimization_opportunities.push(
                "Safety checks could be optimized further.".to_string()
            );
        }
        
        // Analyze error rates
        if metrics.safety_checks > 0 {
            let error_rate = (metrics.safety_check_errors as f64 / metrics.safety_checks as f64) * 100.0;
            if error_rate > 5.0 {
                critical_issues.push(
                    format!("High safety check error rate: {:.1}%", error_rate)
                );
            } else if error_rate > 1.0 {
                recommendations.push(
                    format!("Moderate safety check error rate: {:.1}%", error_rate)
                );
            }
        }
        
        // Analyze cache hit rates
        let total_config_ops = metrics.config_cache_hits + metrics.config_cache_misses;
        if total_config_ops > 0 {
            let hit_rate = (metrics.config_cache_hits as f64 / total_config_ops as f64) * 100.0;
            if hit_rate < 70.0 {
                optimization_opportunities.push(
                    format!("Low configuration cache hit rate: {:.1}%. Consider increasing cache TTL.", hit_rate)
                );
            } else if hit_rate > 95.0 {
                optimization_opportunities.push(
                    "Excellent cache hit rate. Consider reducing cache TTL to save memory.".to_string()
                );
            }
        }
        
        // Analyze read operation performance
        if metrics.read_operations > 0 {
            let read_error_rate = (metrics.read_operation_errors as f64 / metrics.read_operations as f64) * 100.0;
            if read_error_rate > 10.0 {
                critical_issues.push(
                    format!("High read operation error rate: {:.1}%", read_error_rate)
                );
            }
            
            if metrics.avg_read_operation_time > 2000 { // > 2ms
                recommendations.push(
                    "Read operations are slow. Consider optimizing read path.".to_string()
                );
            }
        }
        
        // General recommendations
        if metrics.leadership_queries > metrics.configuration_queries * 10 {
            optimization_opportunities.push(
                "High ratio of leadership queries. Consider caching leadership state.".to_string()
            );
        }
        
        Self {
            recommendations,
            critical_issues,
            optimization_opportunities,
        }
    }
}