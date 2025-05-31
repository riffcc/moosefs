//! Metrics collection for the MooseNG client

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Client metrics collector
#[derive(Debug, Clone)]
pub struct ClientMetrics {
    inner: Arc<ClientMetricsInner>,
}

#[derive(Debug)]
struct ClientMetricsInner {
    // FUSE operation counters
    pub lookup_count: AtomicU64,
    pub read_count: AtomicU64,
    pub write_count: AtomicU64,
    pub create_count: AtomicU64,
    pub delete_count: AtomicU64,
    
    // Performance metrics
    pub total_bytes_read: AtomicU64,
    pub total_bytes_written: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    
    // Error counters
    pub connection_errors: AtomicU64,
    pub timeout_errors: AtomicU64,
    pub io_errors: AtomicU64,
}

/// Snapshot of current metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub lookup_count: u64,
    pub read_count: u64,
    pub write_count: u64,
    pub create_count: u64,
    pub delete_count: u64,
    pub total_bytes_read: u64,
    pub total_bytes_written: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub connection_errors: u64,
    pub timeout_errors: u64,
    pub io_errors: u64,
    pub cache_hit_rate: f64,
    pub timestamp: u64,
}

impl ClientMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ClientMetricsInner {
                lookup_count: AtomicU64::new(0),
                read_count: AtomicU64::new(0),
                write_count: AtomicU64::new(0),
                create_count: AtomicU64::new(0),
                delete_count: AtomicU64::new(0),
                total_bytes_read: AtomicU64::new(0),
                total_bytes_written: AtomicU64::new(0),
                cache_hits: AtomicU64::new(0),
                cache_misses: AtomicU64::new(0),
                connection_errors: AtomicU64::new(0),
                timeout_errors: AtomicU64::new(0),
                io_errors: AtomicU64::new(0),
            }),
        }
    }
    
    /// Increment lookup counter
    pub fn increment_lookup(&self) {
        self.inner.lookup_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Increment read counter and add bytes read
    pub fn increment_read(&self, bytes: u64) {
        self.inner.read_count.fetch_add(1, Ordering::Relaxed);
        self.inner.total_bytes_read.fetch_add(bytes, Ordering::Relaxed);
    }
    
    /// Increment write counter and add bytes written
    pub fn increment_write(&self, bytes: u64) {
        self.inner.write_count.fetch_add(1, Ordering::Relaxed);
        self.inner.total_bytes_written.fetch_add(bytes, Ordering::Relaxed);
    }
    
    /// Increment create counter
    pub fn increment_create(&self) {
        self.inner.create_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Increment delete counter
    pub fn increment_delete(&self) {
        self.inner.delete_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Increment cache hit counter
    pub fn increment_cache_hit(&self) {
        self.inner.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Increment cache miss counter
    pub fn increment_cache_miss(&self) {
        self.inner.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Increment connection error counter
    pub fn increment_connection_error(&self) {
        self.inner.connection_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Increment timeout error counter
    pub fn increment_timeout_error(&self) {
        self.inner.timeout_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Increment I/O error counter
    pub fn increment_io_error(&self) {
        self.inner.io_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        let cache_hits = self.inner.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.inner.cache_misses.load(Ordering::Relaxed);
        let total_cache_operations = cache_hits + cache_misses;
        
        let cache_hit_rate = if total_cache_operations > 0 {
            cache_hits as f64 / total_cache_operations as f64
        } else {
            0.0
        };
        
        MetricsSnapshot {
            lookup_count: self.inner.lookup_count.load(Ordering::Relaxed),
            read_count: self.inner.read_count.load(Ordering::Relaxed),
            write_count: self.inner.write_count.load(Ordering::Relaxed),
            create_count: self.inner.create_count.load(Ordering::Relaxed),
            delete_count: self.inner.delete_count.load(Ordering::Relaxed),
            total_bytes_read: self.inner.total_bytes_read.load(Ordering::Relaxed),
            total_bytes_written: self.inner.total_bytes_written.load(Ordering::Relaxed),
            cache_hits,
            cache_misses,
            connection_errors: self.inner.connection_errors.load(Ordering::Relaxed),
            timeout_errors: self.inner.timeout_errors.load(Ordering::Relaxed),
            io_errors: self.inner.io_errors.load(Ordering::Relaxed),
            cache_hit_rate,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.inner.lookup_count.store(0, Ordering::Relaxed);
        self.inner.read_count.store(0, Ordering::Relaxed);
        self.inner.write_count.store(0, Ordering::Relaxed);
        self.inner.create_count.store(0, Ordering::Relaxed);
        self.inner.delete_count.store(0, Ordering::Relaxed);
        self.inner.total_bytes_read.store(0, Ordering::Relaxed);
        self.inner.total_bytes_written.store(0, Ordering::Relaxed);
        self.inner.cache_hits.store(0, Ordering::Relaxed);
        self.inner.cache_misses.store(0, Ordering::Relaxed);
        self.inner.connection_errors.store(0, Ordering::Relaxed);
        self.inner.timeout_errors.store(0, Ordering::Relaxed);
        self.inner.io_errors.store(0, Ordering::Relaxed);
    }
}

impl Default for ClientMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer for measuring operation durations
pub struct Timer {
    start: Instant,
    operation: String,
}

impl Timer {
    /// Start a new timer for an operation
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            operation: operation.into(),
        }
    }
    
    /// Finish timing and return the duration
    pub fn finish(self) -> (String, Duration) {
        (self.operation, self.start.elapsed())
    }
}