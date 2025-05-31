# Async Runtime Module Documentation

## Overview

The `async_runtime` module provides a comprehensive async runtime infrastructure for MooseNG components. It includes:

1. **Configurable Tokio Runtime** - Customizable thread pools and runtime settings
2. **Async Channel Abstractions** - Multiple channel types for inter-component communication
3. **Async Utilities** - Timeouts, retries with backoff, and circuit breakers
4. **Stream Processing** - Batching, rate limiting, and concurrent processing
5. **Graceful Shutdown** - Coordinated shutdown mechanisms

## Key Components

### AsyncRuntime

Main runtime manager that wraps a Tokio runtime with additional features:

```rust
let config = RuntimeConfig {
    name: "my-runtime".to_string(),
    worker_threads: Some(4),
    ..Default::default()
};
let runtime = AsyncRuntime::new(config)?;
```

### Channels

Multiple channel types for different use cases:

- `bounded` - Bounded MPSC channel with backpressure
- `unbounded` - Unbounded MPSC channel
- `oneshot` - Single-value communication
- `watch` - State broadcasting
- `broadcast` - Multi-consumer scenarios

### Circuit Breaker

Protects against cascading failures:

```rust
let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
let result = cb.call(|| async { 
    // Your operation here
}).await;
```

### Rate Limiter

Token bucket algorithm implementation:

```rust
let limiter = RateLimiter::new(100, 10); // 100/sec, burst of 10
limiter.acquire().await?;
```

### Retry with Backoff

Exponential backoff retry logic:

```rust
let result = retry_with_backoff(
    RetryConfig::default(),
    || async { /* operation */ }
).await?;
```

### Stream Processing

- `batch` - Group items with timeout
- `rate_limit` - Throttle stream items
- `concurrent_process` - Parallel processing with concurrency limit

### Graceful Shutdown

Coordinate shutdown across multiple components:

```rust
let coordinator = ShutdownCoordinator::new();
coordinator.register_task("my-task", task_handle).await;
// ... later ...
coordinator.shutdown(Duration::from_secs(30)).await?;
```

## Usage Examples

See `examples/async_runtime_example.rs` for comprehensive usage examples.

## Testing

The module includes comprehensive unit tests and integration tests. Run with:

```bash
cargo test -p mooseng-common async_runtime
```

## Performance

Benchmarks are available in `benches/async_runtime_bench.rs`. Run with:

```bash
cargo bench -p mooseng-common async_runtime
```