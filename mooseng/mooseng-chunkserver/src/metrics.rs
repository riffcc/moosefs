// Prometheus metrics collection for MooseNG ChunkServer
use lazy_static::lazy_static;
use prometheus::{
    register_gauge_vec, register_histogram_vec, register_int_counter_vec,
    register_int_gauge_vec, GaugeVec, HistogramVec, IntCounterVec, IntGaugeVec,
    Encoder, TextEncoder,
};
use std::time::Duration;

lazy_static! {
    // Storage metrics
    pub static ref CHUNK_COUNT: IntGaugeVec = register_int_gauge_vec!(
        "mooseng_chunkserver_chunks_total",
        "Total number of chunks stored",
        &["server_id", "storage_class"]
    ).unwrap();
    
    pub static ref STORAGE_BYTES: IntGaugeVec = register_int_gauge_vec!(
        "mooseng_chunkserver_storage_bytes",
        "Total bytes of storage used",
        &["server_id", "type"]
    ).unwrap();
    
    pub static ref STORAGE_CAPACITY: IntGaugeVec = register_int_gauge_vec!(
        "mooseng_chunkserver_storage_capacity_bytes",
        "Total storage capacity in bytes",
        &["server_id", "type"]
    ).unwrap();
    
    // Operation metrics
    pub static ref CHUNK_OPS: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_operations_total",
        "Total number of chunk operations",
        &["server_id", "operation", "status"]
    ).unwrap();
    
    pub static ref CHUNK_OP_DURATION: HistogramVec = register_histogram_vec!(
        "mooseng_chunkserver_operation_duration_seconds",
        "Duration of chunk operations",
        &["server_id", "operation"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
    ).unwrap();
    
    pub static ref CHUNK_OP_BYTES: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_operation_bytes_total",
        "Total bytes processed by operations",
        &["server_id", "operation"]
    ).unwrap();
    
    // Network metrics
    pub static ref NETWORK_BYTES: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_network_bytes_total",
        "Total network bytes transferred",
        &["server_id", "direction", "protocol"]
    ).unwrap();
    
    pub static ref ACTIVE_CONNECTIONS: IntGaugeVec = register_int_gauge_vec!(
        "mooseng_chunkserver_connections_active",
        "Number of active connections",
        &["server_id", "type"]
    ).unwrap();
    
    // Replication metrics
    pub static ref REPLICATION_OPS: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_replication_operations_total",
        "Total number of replication operations",
        &["server_id", "direction", "status"]
    ).unwrap();
    
    pub static ref REPLICATION_BYTES: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_replication_bytes_total",
        "Total bytes replicated",
        &["server_id", "direction"]
    ).unwrap();
    
    pub static ref REPLICATION_LAG: GaugeVec = register_gauge_vec!(
        "mooseng_chunkserver_replication_lag_seconds",
        "Replication lag in seconds",
        &["server_id", "peer_id"]
    ).unwrap();
    
    // Erasure coding metrics
    pub static ref ERASURE_OPS: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_erasure_operations_total",
        "Total erasure coding operations",
        &["server_id", "operation", "config", "status"]
    ).unwrap();
    
    pub static ref ERASURE_DURATION: HistogramVec = register_histogram_vec!(
        "mooseng_chunkserver_erasure_duration_seconds",
        "Duration of erasure coding operations",
        &["server_id", "operation", "config"],
        vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    ).unwrap();
    
    pub static ref ERASURE_SHARDS: IntGaugeVec = register_int_gauge_vec!(
        "mooseng_chunkserver_erasure_shards_total",
        "Total number of erasure coded shards",
        &["server_id", "type", "config"]
    ).unwrap();
    
    // Cache metrics
    pub static ref CACHE_HITS: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_cache_hits_total",
        "Total cache hits",
        &["server_id", "cache_type"]
    ).unwrap();
    
    pub static ref CACHE_MISSES: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_cache_misses_total",
        "Total cache misses",
        &["server_id", "cache_type"]
    ).unwrap();
    
    pub static ref CACHE_SIZE: IntGaugeVec = register_int_gauge_vec!(
        "mooseng_chunkserver_cache_size_bytes",
        "Current cache size in bytes",
        &["server_id", "cache_type"]
    ).unwrap();
    
    // Health and performance metrics
    pub static ref CHECKSUM_FAILURES: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_checksum_failures_total",
        "Total checksum verification failures",
        &["server_id"]
    ).unwrap();
    
    pub static ref DISK_IO_ERRORS: IntCounterVec = register_int_counter_vec!(
        "mooseng_chunkserver_disk_io_errors_total",
        "Total disk I/O errors",
        &["server_id", "operation"]
    ).unwrap();
    
    pub static ref CPU_USAGE: GaugeVec = register_gauge_vec!(
        "mooseng_chunkserver_cpu_usage_percent",
        "CPU usage percentage",
        &["server_id"]
    ).unwrap();
    
    pub static ref MEMORY_USAGE: IntGaugeVec = register_int_gauge_vec!(
        "mooseng_chunkserver_memory_usage_bytes",
        "Memory usage in bytes",
        &["server_id", "type"]
    ).unwrap();
    
    pub static ref DISK_USAGE: GaugeVec = register_gauge_vec!(
        "mooseng_chunkserver_disk_usage_percent",
        "Disk usage percentage",
        &["server_id", "mount_point"]
    ).unwrap();
}

/// Helper functions for recording metrics
pub struct MetricsRecorder {
    server_id: String,
}

impl MetricsRecorder {
    pub fn new(server_id: String) -> Self {
        Self { server_id }
    }
    
    /// Record a chunk operation
    pub fn record_chunk_op(&self, operation: &str, status: &str, duration: Duration, bytes: u64) {
        CHUNK_OPS
            .with_label_values(&[&self.server_id, operation, status])
            .inc();
        
        CHUNK_OP_DURATION
            .with_label_values(&[&self.server_id, operation])
            .observe(duration.as_secs_f64());
        
        if bytes > 0 {
            CHUNK_OP_BYTES
                .with_label_values(&[&self.server_id, operation])
                .inc_by(bytes);
        }
    }
    
    /// Record network transfer
    pub fn record_network_transfer(&self, direction: &str, protocol: &str, bytes: u64) {
        NETWORK_BYTES
            .with_label_values(&[&self.server_id, direction, protocol])
            .inc_by(bytes);
    }
    
    /// Record replication operation
    pub fn record_replication(&self, direction: &str, status: &str, bytes: u64) {
        REPLICATION_OPS
            .with_label_values(&[&self.server_id, direction, status])
            .inc();
        
        if bytes > 0 && status == "success" {
            REPLICATION_BYTES
                .with_label_values(&[&self.server_id, direction])
                .inc_by(bytes);
        }
    }
    
    /// Record erasure coding operation
    pub fn record_erasure_op(&self, operation: &str, config: &str, status: &str, duration: Duration) {
        ERASURE_OPS
            .with_label_values(&[&self.server_id, operation, config, status])
            .inc();
        
        if status == "success" {
            ERASURE_DURATION
                .with_label_values(&[&self.server_id, operation, config])
                .observe(duration.as_secs_f64());
        }
    }
    
    /// Record cache access
    pub fn record_cache_access(&self, cache_type: &str, hit: bool) {
        if hit {
            CACHE_HITS
                .with_label_values(&[&self.server_id, cache_type])
                .inc();
        } else {
            CACHE_MISSES
                .with_label_values(&[&self.server_id, cache_type])
                .inc();
        }
    }
    
    /// Update storage metrics
    pub fn update_storage_metrics(&self, chunks: i64, bytes: i64, capacity: i64) {
        CHUNK_COUNT
            .with_label_values(&[&self.server_id, "all"])
            .set(chunks);
        
        STORAGE_BYTES
            .with_label_values(&[&self.server_id, "used"])
            .set(bytes);
        
        STORAGE_CAPACITY
            .with_label_values(&[&self.server_id, "total"])
            .set(capacity);
    }
    
    /// Update connection count
    pub fn update_connections(&self, connection_type: &str, count: i64) {
        ACTIVE_CONNECTIONS
            .with_label_values(&[&self.server_id, connection_type])
            .set(count);
    }
    
    /// Record checksum failure
    pub fn record_checksum_failure(&self) {
        CHECKSUM_FAILURES
            .with_label_values(&[&self.server_id])
            .inc();
    }
    
    /// Record disk I/O error
    pub fn record_disk_io_error(&self, operation: &str) {
        DISK_IO_ERRORS
            .with_label_values(&[&self.server_id, operation])
            .inc();
    }
    
    /// Update system metrics
    pub fn update_system_metrics(&self, cpu_percent: f64, memory_bytes: i64, disk_percent: f64) {
        CPU_USAGE
            .with_label_values(&[&self.server_id])
            .set(cpu_percent);
        
        MEMORY_USAGE
            .with_label_values(&[&self.server_id, "used"])
            .set(memory_bytes);
        
        DISK_USAGE
            .with_label_values(&[&self.server_id, "data"])
            .set(disk_percent);
    }
}

/// Export metrics in Prometheus format
pub fn export_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_metrics_recording() {
        let recorder = MetricsRecorder::new("test-server".to_string());
        
        // Record some operations
        recorder.record_chunk_op("write", "success", Duration::from_millis(100), 1024);
        recorder.record_chunk_op("read", "success", Duration::from_millis(50), 2048);
        recorder.record_chunk_op("delete", "failure", Duration::from_millis(10), 0);
        
        // Record network transfers
        recorder.record_network_transfer("ingress", "grpc", 4096);
        recorder.record_network_transfer("egress", "grpc", 8192);
        
        // Export and check metrics exist
        let metrics = export_metrics();
        assert!(metrics.contains("mooseng_chunkserver_operations_total"));
        assert!(metrics.contains("mooseng_chunkserver_operation_duration_seconds"));
    }
}