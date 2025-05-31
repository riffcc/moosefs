pub mod types;
pub mod error;
pub mod config;
pub mod async_runtime;
pub mod networking;
pub mod health;

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
    compression, batching,
};
pub use health::{
    HealthMonitor, HealthChecker, BasicHealthChecker, HealthCheckConfig,
    HealthStatus, HealthCheckResult, SelfHealingAction, HealthAlert,
};

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