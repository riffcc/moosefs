pub mod types;
pub mod error;
pub mod config;
pub mod async_runtime;
pub mod networking;
pub mod health;
pub mod compression;
pub mod metrics;
pub mod metrics_server;

pub use types::*;
pub use async_runtime::{
    AsyncRuntime, RuntimeConfig, ShutdownCoordinator,
    CircuitBreaker, CircuitBreakerConfig, CircuitState,
    RateLimiter, RetryConfig, retry_with_backoff,
    channels, streams, timeouts,
    TaskSupervisor, GenericSupervisor, RestartPolicy,
};
pub use networking::{
    ConnectionPool, ConnectionPoolConfig, ManagedConnection, PoolStats,
    compression as network_compression, batching,
};
pub use health::{
    HealthMonitor, HealthChecker, BasicHealthChecker, HealthCheckConfig,
    HealthStatus, HealthCheckResult, SelfHealingAction, HealthAlert,
};
pub use compression::{
    CompressionEngine, CompressionConfig, CompressionAlgorithm, CompressionLevel,
    CompressedData, CompressionStats, quick,
};
pub use metrics::{
    MetricsRegistry, MetricCollector, Timer, CommonMetrics, MasterMetrics, 
    ChunkServerMetrics, ClientMetrics, metrics_handler,
    LATENCY_BUCKETS, SIZE_BUCKETS,
};
pub use metrics_server::MetricsServer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(CHUNK_SIZE, 0x04000000);
        assert_eq!(BLOCK_SIZE, 0x10000);
        assert_eq!(BLOCKS_IN_CHUNK, 0x400);
    }
}