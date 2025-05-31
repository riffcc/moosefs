//! Integration tests for async runtime components

use mooseng_common::{
    AsyncRuntime, RuntimeConfig, ShutdownCoordinator,
    CircuitBreaker, CircuitBreakerConfig, CircuitState,
    RateLimiter, RetryConfig, retry_with_backoff,
    channels, streams::rate_limit,
    error::MooseNGError,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use futures::StreamExt;
use tokio::time::sleep;

#[tokio::test]
async fn test_runtime_lifecycle() {
    let config = RuntimeConfig {
        name: "test-runtime".to_string(),
        worker_threads: Some(2),
        ..Default::default()
    };
    
    let runtime = AsyncRuntime::new(config).unwrap();
    assert!(!runtime.is_shutting_down());
    
    // Spawn a task
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    
    runtime.spawn_named("counter".to_string(), async move {
        for _ in 0..5 {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            sleep(Duration::from_millis(10)).await;
        }
    });
    
    sleep(Duration::from_millis(100)).await;
    assert_eq!(counter.load(Ordering::SeqCst), 5);
    
    runtime.shutdown().await;
    assert!(runtime.is_shutting_down());
}

#[tokio::test]
async fn test_shutdown_coordinator() {
    let coordinator = ShutdownCoordinator::new();
    let mut shutdown_signal = coordinator.shutdown_signal();
    
    let task1_completed = Arc::new(AtomicU32::new(0));
    let task1_completed_clone = task1_completed.clone();
    
    let task1 = tokio::spawn(async move {
        tokio::select! {
            _ = shutdown_signal.recv() => {
                task1_completed_clone.store(1, Ordering::SeqCst);
                Ok(())
            }
            _ = sleep(Duration::from_secs(10)) => {
                Ok(())
            }
        }
    });
    
    coordinator.register_task("task1", task1).await;
    
    // Trigger shutdown
    coordinator.shutdown(Duration::from_secs(1)).await.unwrap();
    
    // Verify task completed
    assert_eq!(task1_completed.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_circuit_breaker_transitions() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 2,
        timeout: Duration::from_millis(100),
        window: Duration::from_secs(10),
    };
    
    let cb = CircuitBreaker::new(config);
    
    // Initial state should be closed
    assert_eq!(cb.state().await, CircuitState::Closed);
    
    // First failure
    let _ = cb.call::<_, _, (), MooseNGError>(|| async {
        Err(MooseNGError::Other("error".into()))
    }).await;
    assert_eq!(cb.state().await, CircuitState::Closed);
    
    // Second failure should open the circuit
    let _ = cb.call::<_, _, (), MooseNGError>(|| async {
        Err(MooseNGError::Other("error".into()))
    }).await;
    assert_eq!(cb.state().await, CircuitState::Open);
    
    // Calls should fail immediately when open
    let result = cb.call::<_, _, (), MooseNGError>(|| async {
        Ok(())
    }).await;
    assert!(result.is_err());
    
    // Wait for timeout
    sleep(Duration::from_millis(150)).await;
    assert_eq!(cb.state().await, CircuitState::HalfOpen);
    
    // Success in half-open state
    let _ = cb.call::<_, _, (), MooseNGError>(|| async {
        Ok(())
    }).await;
    
    // Second success should close the circuit
    let _ = cb.call::<_, _, (), MooseNGError>(|| async {
        Ok(())
    }).await;
    assert_eq!(cb.state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_rate_limiter() {
    let limiter = RateLimiter::new(10, 2); // 10/sec, burst of 2
    
    // Should allow initial burst
    let start = Instant::now();
    assert!(limiter.acquire().await.is_ok());
    assert!(limiter.acquire().await.is_ok());
    
    // Third should wait
    assert!(limiter.acquire().await.is_ok());
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(90)); // Should have waited ~100ms
}

#[tokio::test]
async fn test_retry_with_exponential_backoff() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    
    let start = Instant::now();
    let result = retry_with_backoff(
        RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(50),
            multiplier: 2.0,
            jitter: false,
        },
        || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err("Not yet")
                } else {
                    Ok("Success")
                }
            }
        },
    ).await;
    
    assert_eq!(result.unwrap(), "Success");
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    
    // Should have taken at least 10ms + 20ms = 30ms
    assert!(start.elapsed() >= Duration::from_millis(30));
}

#[tokio::test]
async fn test_channel_types() {
    // Test bounded channel
    let (tx, mut rx) = channels::bounded::<i32>(2);
    tx.send(1).await.unwrap();
    tx.send(2).await.unwrap();
    assert_eq!(rx.recv().await.unwrap(), 1);
    assert_eq!(rx.recv().await.unwrap(), 2);
    
    // Test unbounded channel
    let (tx, mut rx) = channels::unbounded::<i32>();
    for i in 0..100 {
        tx.send(i).unwrap();
    }
    for i in 0..100 {
        assert_eq!(rx.recv().await.unwrap(), i);
    }
    
    // Test oneshot channel
    let (tx, rx) = channels::oneshot::<i32>();
    tx.send(42).unwrap();
    assert_eq!(rx.await.unwrap(), 42);
    
    // Test watch channel
    let (tx, mut rx) = channels::watch(0);
    tx.send(1).unwrap();
    tx.send(2).unwrap();
    assert_eq!(*rx.borrow_and_update(), 2);
    
    // Test broadcast channel
    let (tx, mut rx1) = channels::broadcast::<i32>(10);
    let mut rx2 = tx.subscribe();
    tx.send(100).unwrap();
    assert_eq!(rx1.recv().await.unwrap(), 100);
    assert_eq!(rx2.recv().await.unwrap(), 100);
}

#[tokio::test]
async fn test_stream_rate_limiting() {
    let stream = futures::stream::iter(0..10);
    let start = Instant::now();
    
    let rate_limited = rate_limit(stream, 10); // 10 items per second
    let items: Vec<_> = rate_limited.collect().await;
    
    assert_eq!(items.len(), 10);
    assert!(start.elapsed() >= Duration::from_millis(900)); // Should take ~1 second
}