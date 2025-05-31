//! Prometheus metrics collection and export for MooseNG
//! 
//! This module provides a comprehensive metrics framework that can be used
//! across all MooseNG components to expose operational metrics to Prometheus.

use anyhow::Result;
use prometheus::{
    Gauge, Histogram, HistogramVec, HistogramOpts,
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry, TextEncoder, Encoder,
    linear_buckets,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::error;

/// Standard histogram buckets for latency measurements (in seconds)
pub const LATENCY_BUCKETS: &[f64] = &[
    0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0
];

/// Standard histogram buckets for size measurements (in bytes)
pub const SIZE_BUCKETS: &[f64] = &[
    1024.0, 4096.0, 16384.0, 65536.0, 262144.0, 1048576.0, 4194304.0, 16777216.0, 67108864.0
];

/// Metrics registry for a MooseNG component
pub struct MetricsRegistry {
    /// Prometheus registry
    registry: Registry,
    /// Component name
    component: String,
    /// Metric collectors
    collectors: Arc<RwLock<HashMap<String, Box<dyn MetricCollector + Send + Sync>>>>,
}

impl MetricsRegistry {
    /// Create a new metrics registry for a component
    pub fn new(component: &str) -> Result<Self> {
        let registry = Registry::new();
        
        Ok(Self {
            registry,
            component: component.to_string(),
            collectors: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Register a metric collector
    pub async fn register_collector<T: MetricCollector + Send + Sync + 'static>(
        &self,
        name: String,
        collector: T,
    ) -> Result<()> {
        let mut collectors = self.collectors.write().await;
        collectors.insert(name, Box::new(collector));
        Ok(())
    }

    /// Get the Prometheus registry
    pub fn prometheus_registry(&self) -> &Registry {
        &self.registry
    }

    /// Export metrics in Prometheus text format
    pub async fn export_metrics(&self) -> Result<String> {
        // Update all dynamic collectors
        let collectors = self.collectors.read().await;
        for (name, collector) in collectors.iter() {
            if let Err(e) = collector.collect().await {
                error!("Failed to collect metrics for {}: {}", name, e);
            }
        }
        drop(collectors);

        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }

    /// Create a new counter
    pub fn create_counter(&self, name: &str, help: &str) -> Result<IntCounter> {
        let opts = Opts::new(name, help)
            .const_label("component", &self.component);
        let counter = IntCounter::with_opts(opts)?;
        self.registry.register(Box::new(counter.clone()))?;
        Ok(counter)
    }

    /// Create a new counter vector
    pub fn create_counter_vec(&self, name: &str, help: &str, labels: &[&str]) -> Result<IntCounterVec> {
        let opts = Opts::new(name, help)
            .const_label("component", &self.component);
        let counter_vec = IntCounterVec::new(opts, labels)?;
        self.registry.register(Box::new(counter_vec.clone()))?;
        Ok(counter_vec)
    }

    /// Create a new gauge
    pub fn create_gauge(&self, name: &str, help: &str) -> Result<IntGauge> {
        let opts = Opts::new(name, help)
            .const_label("component", &self.component);
        let gauge = IntGauge::with_opts(opts)?;
        self.registry.register(Box::new(gauge.clone()))?;
        Ok(gauge)
    }

    /// Create a new gauge vector
    pub fn create_gauge_vec(&self, name: &str, help: &str, labels: &[&str]) -> Result<IntGaugeVec> {
        let opts = Opts::new(name, help)
            .const_label("component", &self.component);
        let gauge_vec = IntGaugeVec::new(opts, labels)?;
        self.registry.register(Box::new(gauge_vec.clone()))?;
        Ok(gauge_vec)
    }

    /// Create a new float gauge
    pub fn create_float_gauge(&self, name: &str, help: &str) -> Result<Gauge> {
        let opts = Opts::new(name, help)
            .const_label("component", &self.component);
        let gauge = Gauge::with_opts(opts)?;
        self.registry.register(Box::new(gauge.clone()))?;
        Ok(gauge)
    }

    /// Create a new histogram
    pub fn create_histogram(&self, name: &str, help: &str, buckets: Vec<f64>) -> Result<Histogram> {
        let opts = HistogramOpts::new(name, help)
            .const_label("component", &self.component)
            .buckets(buckets);
        let histogram = Histogram::with_opts(opts)?;
        self.registry.register(Box::new(histogram.clone()))?;
        Ok(histogram)
    }

    /// Create a new histogram vector
    pub fn create_histogram_vec(&self, name: &str, help: &str, labels: &[&str], buckets: Vec<f64>) -> Result<HistogramVec> {
        let opts = HistogramOpts::new(name, help)
            .const_label("component", &self.component)
            .buckets(buckets);
        let histogram_vec = HistogramVec::new(opts, labels)?;
        self.registry.register(Box::new(histogram_vec.clone()))?;
        Ok(histogram_vec)
    }
}

/// Trait for custom metric collectors
#[async_trait::async_trait]
pub trait MetricCollector {
    async fn collect(&self) -> Result<()>;
}

/// Timer for measuring operation duration
pub struct Timer {
    start: Instant,
    histogram: Histogram,
}

impl Timer {
    /// Create a new timer
    pub fn new(histogram: Histogram) -> Self {
        Self {
            start: Instant::now(),
            histogram,
        }
    }

    /// Record the elapsed time and consume the timer
    pub fn finish(self) {
        let duration = self.start.elapsed();
        self.histogram.observe(duration.as_secs_f64());
    }
}

/// Common metrics for all MooseNG components
pub struct CommonMetrics {
    // Process metrics
    pub process_start_time: IntGauge,
    pub process_cpu_usage: Gauge,
    pub process_memory_usage: IntGauge,
    pub process_open_files: IntGauge,
    
    // HTTP metrics
    pub http_requests_total: IntCounterVec,
    pub http_request_duration: HistogramVec,
    pub http_response_size: HistogramVec,
    
    // gRPC metrics
    pub grpc_requests_total: IntCounterVec,
    pub grpc_request_duration: HistogramVec,
    pub grpc_connections_active: IntGauge,
    
    // Error metrics
    pub errors_total: IntCounterVec,
    pub panics_total: IntCounter,
    
    // System health
    pub health_check_status: IntGaugeVec,
    pub last_health_check: IntGauge,
}

impl CommonMetrics {
    /// Create common metrics for a component
    pub fn new(registry: &MetricsRegistry) -> Result<Self> {
        Ok(Self {
            process_start_time: registry.create_gauge(
                "process_start_time_seconds",
                "Unix timestamp of process start time"
            )?,
            process_cpu_usage: registry.create_float_gauge(
                "process_cpu_usage_ratio",
                "Current CPU usage ratio (0-1)"
            )?,
            process_memory_usage: registry.create_gauge(
                "process_memory_usage_bytes",
                "Current memory usage in bytes"
            )?,
            process_open_files: registry.create_gauge(
                "process_open_files",
                "Number of open file descriptors"
            )?,
            
            http_requests_total: registry.create_counter_vec(
                "http_requests_total",
                "Total number of HTTP requests",
                &["method", "endpoint", "status"]
            )?,
            http_request_duration: registry.create_histogram_vec(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
                &["method", "endpoint"],
                LATENCY_BUCKETS.to_vec()
            )?,
            http_response_size: registry.create_histogram_vec(
                "http_response_size_bytes",
                "HTTP response size in bytes",
                &["method", "endpoint"],
                SIZE_BUCKETS.to_vec()
            )?,
            
            grpc_requests_total: registry.create_counter_vec(
                "grpc_requests_total",
                "Total number of gRPC requests",
                &["service", "method", "status"]
            )?,
            grpc_request_duration: registry.create_histogram_vec(
                "grpc_request_duration_seconds",
                "gRPC request duration in seconds",
                &["service", "method"],
                LATENCY_BUCKETS.to_vec()
            )?,
            grpc_connections_active: registry.create_gauge(
                "grpc_connections_active",
                "Number of active gRPC connections"
            )?,
            
            errors_total: registry.create_counter_vec(
                "errors_total",
                "Total number of errors",
                &["type", "severity"]
            )?,
            panics_total: registry.create_counter(
                "panics_total",
                "Total number of panics"
            )?,
            
            health_check_status: registry.create_gauge_vec(
                "health_check_status",
                "Health check status (1=healthy, 0=unhealthy)",
                &["check_name"]
            )?,
            last_health_check: registry.create_gauge(
                "last_health_check_timestamp_seconds",
                "Unix timestamp of last health check"
            )?,
        })
    }

    /// Initialize process metrics
    pub fn init_process_metrics(&self) {
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.process_start_time.set(start_time);
    }

    /// Update process metrics
    pub fn update_process_metrics(&self) {
        // These would be updated with actual system metrics
        // For now, we'll use placeholder values
        
        // Memory usage (would use actual memory reading)
        self.process_memory_usage.set(0);
        
        // Open files (would use actual file descriptor count)
        self.process_open_files.set(0);
        
        // CPU usage (would use actual CPU measurement)
        self.process_cpu_usage.set(0.0);
    }

    /// Record an HTTP request
    pub fn record_http_request(&self, method: &str, endpoint: &str, status: &str, duration: Duration) {
        self.http_requests_total
            .with_label_values(&[method, endpoint, status])
            .inc();
        
        self.http_request_duration
            .with_label_values(&[method, endpoint])
            .observe(duration.as_secs_f64());
    }

    /// Record a gRPC request
    pub fn record_grpc_request(&self, service: &str, method: &str, status: &str, duration: Duration) {
        self.grpc_requests_total
            .with_label_values(&[service, method, status])
            .inc();
        
        self.grpc_request_duration
            .with_label_values(&[service, method])
            .observe(duration.as_secs_f64());
    }

    /// Record an error
    pub fn record_error(&self, error_type: &str, severity: &str) {
        self.errors_total
            .with_label_values(&[error_type, severity])
            .inc();
    }

    /// Record a panic
    pub fn record_panic(&self) {
        self.panics_total.inc();
    }

    /// Update health check status
    pub fn update_health_status(&self, check_name: &str, healthy: bool) {
        let status = if healthy { 1 } else { 0 };
        self.health_check_status
            .with_label_values(&[check_name])
            .set(status);
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.last_health_check.set(now);
    }
}

/// Metrics for the Master Server component
pub struct MasterMetrics {
    pub common: CommonMetrics,
    
    // Metadata operations
    pub metadata_operations_total: IntCounterVec,
    pub metadata_operation_duration: HistogramVec,
    pub metadata_cache_size: IntGauge,
    pub metadata_cache_hits: IntCounter,
    pub metadata_cache_misses: IntCounter,
    
    // Chunk management
    pub chunks_total: IntGauge,
    pub chunks_created: IntCounter,
    pub chunks_deleted: IntCounter,
    pub chunk_replicas_total: IntGauge,
    
    // Client sessions
    pub active_sessions: IntGauge,
    pub session_operations_total: IntCounterVec,
    
    // Raft consensus
    pub raft_term: IntGauge,
    pub raft_log_entries: IntGauge,
    pub raft_leader_elections: IntCounter,
    pub raft_heartbeats_sent: IntCounter,
    
    // Multi-region
    pub multiregion_operations_total: IntCounterVec,
    pub multiregion_latency: HistogramVec,
    pub regions_active: IntGauge,
}

impl MasterMetrics {
    pub fn new(registry: &MetricsRegistry) -> Result<Self> {
        Ok(Self {
            common: CommonMetrics::new(registry)?,
            
            metadata_operations_total: registry.create_counter_vec(
                "master_metadata_operations_total",
                "Total metadata operations",
                &["operation", "status"]
            )?,
            metadata_operation_duration: registry.create_histogram_vec(
                "master_metadata_operation_duration_seconds",
                "Metadata operation duration",
                &["operation"],
                LATENCY_BUCKETS.to_vec()
            )?,
            metadata_cache_size: registry.create_gauge(
                "master_metadata_cache_size_bytes",
                "Size of metadata cache in bytes"
            )?,
            metadata_cache_hits: registry.create_counter(
                "master_metadata_cache_hits_total",
                "Total metadata cache hits"
            )?,
            metadata_cache_misses: registry.create_counter(
                "master_metadata_cache_misses_total",
                "Total metadata cache misses"
            )?,
            
            chunks_total: registry.create_gauge(
                "master_chunks_total",
                "Total number of chunks managed"
            )?,
            chunks_created: registry.create_counter(
                "master_chunks_created_total",
                "Total chunks created"
            )?,
            chunks_deleted: registry.create_counter(
                "master_chunks_deleted_total",
                "Total chunks deleted"
            )?,
            chunk_replicas_total: registry.create_gauge(
                "master_chunk_replicas_total",
                "Total number of chunk replicas"
            )?,
            
            active_sessions: registry.create_gauge(
                "master_active_sessions",
                "Number of active client sessions"
            )?,
            session_operations_total: registry.create_counter_vec(
                "master_session_operations_total",
                "Total session operations",
                &["operation", "status"]
            )?,
            
            raft_term: registry.create_gauge(
                "master_raft_term",
                "Current Raft term"
            )?,
            raft_log_entries: registry.create_gauge(
                "master_raft_log_entries",
                "Number of entries in Raft log"
            )?,
            raft_leader_elections: registry.create_counter(
                "master_raft_leader_elections_total",
                "Total Raft leader elections"
            )?,
            raft_heartbeats_sent: registry.create_counter(
                "master_raft_heartbeats_sent_total",
                "Total Raft heartbeats sent"
            )?,
            
            multiregion_operations_total: registry.create_counter_vec(
                "master_multiregion_operations_total",
                "Total multi-region operations",
                &["operation", "region", "status"]
            )?,
            multiregion_latency: registry.create_histogram_vec(
                "master_multiregion_latency_seconds",
                "Multi-region operation latency",
                &["operation", "region"],
                LATENCY_BUCKETS.to_vec()
            )?,
            regions_active: registry.create_gauge(
                "master_regions_active",
                "Number of active regions"
            )?,
        })
    }
}

/// Metrics for the Chunk Server component
pub struct ChunkServerMetrics {
    pub common: CommonMetrics,
    
    // Chunk operations
    pub chunk_operations_total: IntCounterVec,
    pub chunk_operation_duration: HistogramVec,
    pub chunks_stored: IntGauge,
    pub chunk_storage_size: IntGauge,
    
    // I/O operations
    pub disk_read_bytes: IntCounter,
    pub disk_write_bytes: IntCounter,
    pub disk_operations_total: IntCounterVec,
    pub disk_operation_duration: HistogramVec,
    
    // Cache metrics
    pub cache_size_bytes: IntGauge,
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,
    pub cache_evictions: IntCounter,
    
    // Compression metrics
    pub compression_operations_total: IntCounterVec,
    pub compression_ratio: Histogram,
    pub compression_savings_bytes: IntCounter,
    
    // Erasure coding
    pub erasure_operations_total: IntCounterVec,
    pub erasure_reconstruction_time: Histogram,
    
    // Health metrics
    pub disk_space_total_bytes: IntGauge,
    pub disk_space_used_bytes: IntGauge,
    pub disk_space_available_bytes: IntGauge,
}

impl ChunkServerMetrics {
    pub fn new(registry: &MetricsRegistry) -> Result<Self> {
        Ok(Self {
            common: CommonMetrics::new(registry)?,
            
            chunk_operations_total: registry.create_counter_vec(
                "chunkserver_chunk_operations_total",
                "Total chunk operations",
                &["operation", "status"]
            )?,
            chunk_operation_duration: registry.create_histogram_vec(
                "chunkserver_chunk_operation_duration_seconds",
                "Chunk operation duration",
                &["operation"],
                LATENCY_BUCKETS.to_vec()
            )?,
            chunks_stored: registry.create_gauge(
                "chunkserver_chunks_stored",
                "Number of chunks stored"
            )?,
            chunk_storage_size: registry.create_gauge(
                "chunkserver_chunk_storage_size_bytes",
                "Total size of stored chunks"
            )?,
            
            disk_read_bytes: registry.create_counter(
                "chunkserver_disk_read_bytes_total",
                "Total bytes read from disk"
            )?,
            disk_write_bytes: registry.create_counter(
                "chunkserver_disk_write_bytes_total",
                "Total bytes written to disk"
            )?,
            disk_operations_total: registry.create_counter_vec(
                "chunkserver_disk_operations_total",
                "Total disk operations",
                &["operation", "status"]
            )?,
            disk_operation_duration: registry.create_histogram_vec(
                "chunkserver_disk_operation_duration_seconds",
                "Disk operation duration",
                &["operation"],
                LATENCY_BUCKETS.to_vec()
            )?,
            
            cache_size_bytes: registry.create_gauge(
                "chunkserver_cache_size_bytes",
                "Cache size in bytes"
            )?,
            cache_hits: registry.create_counter(
                "chunkserver_cache_hits_total",
                "Total cache hits"
            )?,
            cache_misses: registry.create_counter(
                "chunkserver_cache_misses_total",
                "Total cache misses"
            )?,
            cache_evictions: registry.create_counter(
                "chunkserver_cache_evictions_total",
                "Total cache evictions"
            )?,
            
            compression_operations_total: registry.create_counter_vec(
                "chunkserver_compression_operations_total",
                "Total compression operations",
                &["operation", "algorithm", "status"]
            )?,
            compression_ratio: registry.create_histogram(
                "chunkserver_compression_ratio",
                "Compression ratio achieved",
                linear_buckets(1.0, 0.5, 10).unwrap()
            )?,
            compression_savings_bytes: registry.create_counter(
                "chunkserver_compression_savings_bytes_total",
                "Total bytes saved by compression"
            )?,
            
            erasure_operations_total: registry.create_counter_vec(
                "chunkserver_erasure_operations_total",
                "Total erasure coding operations",
                &["operation", "status"]
            )?,
            erasure_reconstruction_time: registry.create_histogram(
                "chunkserver_erasure_reconstruction_duration_seconds",
                "Erasure coding reconstruction time",
                LATENCY_BUCKETS.to_vec()
            )?,
            
            disk_space_total_bytes: registry.create_gauge(
                "chunkserver_disk_space_total_bytes",
                "Total disk space"
            )?,
            disk_space_used_bytes: registry.create_gauge(
                "chunkserver_disk_space_used_bytes",
                "Used disk space"
            )?,
            disk_space_available_bytes: registry.create_gauge(
                "chunkserver_disk_space_available_bytes",
                "Available disk space"
            )?,
        })
    }
}

/// Metrics for the Client component
pub struct ClientMetrics {
    pub common: CommonMetrics,
    
    // File operations
    pub file_operations_total: IntCounterVec,
    pub file_operation_duration: HistogramVec,
    pub files_open: IntGauge,
    
    // I/O operations
    pub bytes_read: IntCounter,
    pub bytes_written: IntCounter,
    pub read_operations_total: IntCounter,
    pub write_operations_total: IntCounter,
    
    // Cache metrics
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,
    pub cache_size_bytes: IntGauge,
    
    // Connection metrics
    pub master_connections: IntGauge,
    pub chunkserver_connections: IntGauge,
    pub connection_errors: IntCounterVec,
}

impl ClientMetrics {
    pub fn new(registry: &MetricsRegistry) -> Result<Self> {
        Ok(Self {
            common: CommonMetrics::new(registry)?,
            
            file_operations_total: registry.create_counter_vec(
                "client_file_operations_total",
                "Total file operations",
                &["operation", "status"]
            )?,
            file_operation_duration: registry.create_histogram_vec(
                "client_file_operation_duration_seconds",
                "File operation duration",
                &["operation"],
                LATENCY_BUCKETS.to_vec()
            )?,
            files_open: registry.create_gauge(
                "client_files_open",
                "Number of open files"
            )?,
            
            bytes_read: registry.create_counter(
                "client_bytes_read_total",
                "Total bytes read"
            )?,
            bytes_written: registry.create_counter(
                "client_bytes_written_total",
                "Total bytes written"
            )?,
            read_operations_total: registry.create_counter(
                "client_read_operations_total",
                "Total read operations"
            )?,
            write_operations_total: registry.create_counter(
                "client_write_operations_total",
                "Total write operations"
            )?,
            
            cache_hits: registry.create_counter(
                "client_cache_hits_total",
                "Total cache hits"
            )?,
            cache_misses: registry.create_counter(
                "client_cache_misses_total",
                "Total cache misses"
            )?,
            cache_size_bytes: registry.create_gauge(
                "client_cache_size_bytes",
                "Cache size in bytes"
            )?,
            
            master_connections: registry.create_gauge(
                "client_master_connections",
                "Number of master connections"
            )?,
            chunkserver_connections: registry.create_gauge(
                "client_chunkserver_connections",
                "Number of chunkserver connections"
            )?,
            connection_errors: registry.create_counter_vec(
                "client_connection_errors_total",
                "Total connection errors",
                &["type", "target"]
            )?,
        })
    }
}

/// Metrics for the Metalogger component
pub struct MetaloggerMetrics {
    pub common: CommonMetrics,
    
    // Log operations
    pub log_entries_total: IntCounterVec,
    pub log_size_bytes: IntGauge,
    pub log_replication_lag: Histogram,
    
    // Backup operations
    pub backup_operations_total: IntCounterVec,
    pub backup_duration: HistogramVec,
    pub backup_size_bytes: HistogramVec,
    
    // Recovery operations
    pub recovery_operations_total: IntCounterVec,
    pub recovery_duration: HistogramVec,
}

impl MetaloggerMetrics {
    pub fn new(registry: &MetricsRegistry) -> Result<Self> {
        Ok(Self {
            common: CommonMetrics::new(registry)?,
            
            log_entries_total: registry.create_counter_vec(
                "metalogger_log_entries_total",
                "Total log entries processed",
                &["operation", "status"]
            )?,
            log_size_bytes: registry.create_gauge(
                "metalogger_log_size_bytes",
                "Current log size in bytes"
            )?,
            log_replication_lag: registry.create_histogram(
                "metalogger_log_replication_lag_seconds",
                "Log replication lag",
                LATENCY_BUCKETS.to_vec()
            )?,
            
            backup_operations_total: registry.create_counter_vec(
                "metalogger_backup_operations_total",
                "Total backup operations",
                &["operation", "status"]
            )?,
            backup_duration: registry.create_histogram_vec(
                "metalogger_backup_duration_seconds",
                "Backup operation duration",
                &["operation"],
                LATENCY_BUCKETS.to_vec()
            )?,
            backup_size_bytes: registry.create_histogram_vec(
                "metalogger_backup_size_bytes",
                "Backup operation size",
                &["operation"],
                SIZE_BUCKETS.to_vec()
            )?,
            
            recovery_operations_total: registry.create_counter_vec(
                "metalogger_recovery_operations_total",
                "Total recovery operations",
                &["operation", "status"]
            )?,
            recovery_duration: registry.create_histogram_vec(
                "metalogger_recovery_duration_seconds",
                "Recovery operation duration",
                &["operation"],
                LATENCY_BUCKETS.to_vec()
            )?,
        })
    }
}

/// Component metrics collector factory
pub struct MetricsCollectorFactory;

impl MetricsCollectorFactory {
    /// Create a metrics collector for the specified component type
    pub fn create_collector(
        component_type: &str,
        registry: &MetricsRegistry,
    ) -> Result<Box<dyn MetricCollector + Send + Sync>> {
        match component_type {
            "master" => {
                let metrics = Arc::new(MasterMetrics::new(registry)?);
                Ok(Box::new(ComponentMetricsCollector::new("master", metrics)))
            }
            "chunkserver" => {
                let metrics = Arc::new(ChunkServerMetrics::new(registry)?);
                Ok(Box::new(ComponentMetricsCollector::new("chunkserver", metrics)))
            }
            "client" => {
                let metrics = Arc::new(ClientMetrics::new(registry)?);
                Ok(Box::new(ComponentMetricsCollector::new("client", metrics)))
            }
            "metalogger" => {
                let metrics = Arc::new(MetaloggerMetrics::new(registry)?);
                Ok(Box::new(ComponentMetricsCollector::new("metalogger", metrics)))
            }
            _ => Err(anyhow::anyhow!("Unknown component type: {}", component_type)),
        }
    }
}

/// Generic component metrics collector
pub struct ComponentMetricsCollector<T> {
    component_name: String,
    metrics: Arc<T>,
}

impl<T> ComponentMetricsCollector<T> {
    pub fn new(component_name: impl Into<String>, metrics: Arc<T>) -> Self {
        Self {
            component_name: component_name.into(),
            metrics,
        }
    }
}

/// Implementation for different component types
#[async_trait::async_trait]
impl MetricCollector for ComponentMetricsCollector<MasterMetrics> {
    async fn collect(&self) -> Result<()> {
        // Update process metrics
        self.metrics.common.update_process_metrics();
        
        // Update health check timestamp
        self.metrics.common.update_health_status("component_alive", true);
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl MetricCollector for ComponentMetricsCollector<ChunkServerMetrics> {
    async fn collect(&self) -> Result<()> {
        // Update process metrics
        self.metrics.common.update_process_metrics();
        
        // Update health check timestamp
        self.metrics.common.update_health_status("component_alive", true);
        
        // Update disk space metrics (would be implemented with actual disk monitoring)
        self.metrics.disk_space_total_bytes.set(1000000000); // 1GB placeholder
        self.metrics.disk_space_used_bytes.set(500000000);   // 500MB placeholder
        self.metrics.disk_space_available_bytes.set(500000000); // 500MB placeholder
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl MetricCollector for ComponentMetricsCollector<ClientMetrics> {
    async fn collect(&self) -> Result<()> {
        // Update process metrics
        self.metrics.common.update_process_metrics();
        
        // Update health check timestamp
        self.metrics.common.update_health_status("component_alive", true);
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl MetricCollector for ComponentMetricsCollector<MetaloggerMetrics> {
    async fn collect(&self) -> Result<()> {
        // Update process metrics
        self.metrics.common.update_process_metrics();
        
        // Update health check timestamp
        self.metrics.common.update_health_status("component_alive", true);
        
        // Update log size (would be implemented with actual log monitoring)
        self.metrics.log_size_bytes.set(100000000); // 100MB placeholder
        
        Ok(())
    }
}

/// HTTP endpoint for serving metrics
pub async fn metrics_handler(registry: &MetricsRegistry) -> Result<String, String> {
    registry.export_metrics().await
        .map_err(|e| format!("Failed to export metrics: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_registry_creation() {
        let registry = MetricsRegistry::new("test").unwrap();
        assert_eq!(registry.component, "test");
    }

    #[tokio::test]
    async fn test_common_metrics_creation() {
        let registry = MetricsRegistry::new("test").unwrap();
        let metrics = CommonMetrics::new(&registry).unwrap();
        
        // Test that metrics can be used
        metrics.record_error("test_error", "warning");
        metrics.record_panic();
        
        let exported = registry.export_metrics().await.unwrap();
        assert!(exported.contains("errors_total"));
        assert!(exported.contains("panics_total"));
    }

    #[tokio::test]
    async fn test_master_metrics_creation() {
        let registry = MetricsRegistry::new("master").unwrap();
        let _metrics = MasterMetrics::new(&registry).unwrap();
        
        let exported = registry.export_metrics().await.unwrap();
        assert!(exported.contains("master_chunks_total"));
        assert!(exported.contains("master_raft_term"));
    }

    #[tokio::test]
    async fn test_timer_functionality() {
        let registry = MetricsRegistry::new("test").unwrap();
        let histogram = registry.create_histogram(
            "test_duration",
            "Test duration",
            LATENCY_BUCKETS.to_vec()
        ).unwrap();
        
        let timer = Timer::new(histogram);
        // Simulate some work
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        timer.finish();
        
        let exported = registry.export_metrics().await.unwrap();
        assert!(exported.contains("test_duration"));
    }

    #[tokio::test]
    async fn test_chunkserver_metrics_creation() {
        let registry = MetricsRegistry::new("chunkserver").unwrap();
        let _metrics = ChunkServerMetrics::new(&registry).unwrap();
        
        let exported = registry.export_metrics().await.unwrap();
        assert!(exported.contains("chunkserver_chunks_stored"));
        assert!(exported.contains("chunkserver_disk_space_total_bytes"));
    }

    #[tokio::test]
    async fn test_client_metrics_creation() {
        let registry = MetricsRegistry::new("client").unwrap();
        let _metrics = ClientMetrics::new(&registry).unwrap();
        
        let exported = registry.export_metrics().await.unwrap();
        assert!(exported.contains("client_files_open"));
        assert!(exported.contains("client_bytes_read_total"));
    }

    #[tokio::test]
    async fn test_metalogger_metrics_creation() {
        let registry = MetricsRegistry::new("metalogger").unwrap();
        let _metrics = MetaloggerMetrics::new(&registry).unwrap();
        
        let exported = registry.export_metrics().await.unwrap();
        assert!(exported.contains("metalogger_log_entries_total"));
        assert!(exported.contains("metalogger_log_size_bytes"));
    }

    #[tokio::test]
    async fn test_metrics_collector_factory() {
        let registry = MetricsRegistry::new("test").unwrap();
        
        // Test creating collectors for different component types
        let master_collector = MetricsCollectorFactory::create_collector("master", &registry).unwrap();
        let chunkserver_collector = MetricsCollectorFactory::create_collector("chunkserver", &registry).unwrap();
        let client_collector = MetricsCollectorFactory::create_collector("client", &registry).unwrap();
        let metalogger_collector = MetricsCollectorFactory::create_collector("metalogger", &registry).unwrap();
        
        // Test that collectors can be used
        master_collector.collect().await.unwrap();
        chunkserver_collector.collect().await.unwrap();
        client_collector.collect().await.unwrap();
        metalogger_collector.collect().await.unwrap();
        
        // Test unknown component type
        let result = MetricsCollectorFactory::create_collector("unknown", &registry);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_component_metrics_collector() {
        let registry = MetricsRegistry::new("test_component").unwrap();
        let metrics = Arc::new(MasterMetrics::new(&registry).unwrap());
        let collector = ComponentMetricsCollector::new("test_master", metrics.clone());
        
        // Test metrics collection
        collector.collect().await.unwrap();
        
        // Verify metrics are updated
        let exported = registry.export_metrics().await.unwrap();
        assert!(exported.contains("health_check_status"));
        assert!(exported.contains("component=\"test_component\""));
    }
}