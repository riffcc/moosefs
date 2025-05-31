//! Example usage of the async runtime module

use anyhow::Result;
use mooseng_common::{
    AsyncRuntime, RuntimeConfig, ShutdownCoordinator,
    CircuitBreaker, CircuitBreakerConfig,
    RateLimiter, RetryConfig, retry_with_backoff,
    channels, streams, timeouts,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Example 1: Create and use async runtime
    let runtime_config = RuntimeConfig {
        name: "example-runtime".to_string(),
        worker_threads: Some(4),
        ..Default::default()
    };
    
    let runtime = AsyncRuntime::new(runtime_config)?;
    info!("Created async runtime");
    
    // Example 2: Use channels for communication
    let (tx, mut rx) = channels::bounded::<String>(10);
    
    runtime.spawn_named("producer".to_string(), async move {
        for i in 0..5 {
            tx.send(format!("Message {}", i)).await.unwrap();
            sleep(Duration::from_millis(100)).await;
        }
    });
    
    runtime.spawn_named("consumer".to_string(), async move {
        while let Some(msg) = rx.recv().await {
            info!("Received: {}", msg);
        }
    });
    
    // Example 3: Use retry with backoff
    let result = retry_with_backoff(
        RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            ..Default::default()
        },
        || async {
            // Simulated operation that might fail
            static mut COUNTER: u32 = 0;
            unsafe {
                COUNTER += 1;
                if COUNTER < 2 {
                    Err("Temporary failure")
                } else {
                    Ok("Success!")
                }
            }
        },
    ).await?;
    
    info!("Retry result: {}", result);
    
    // Example 4: Use circuit breaker
    let circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
    
    for i in 0..10 {
        let result = circuit_breaker.call(|| async {
            if i < 5 {
                Err(mooseng_common::error::MooseNGError::Other("Service error".into()))
            } else {
                Ok(format!("Success {}", i))
            }
        }).await;
        
        match result {
            Ok(msg) => info!("Circuit breaker call succeeded: {}", msg),
            Err(e) => error!("Circuit breaker call failed: {:?}", e),
        }
        
        sleep(Duration::from_millis(500)).await;
    }
    
    // Example 5: Use rate limiter
    let rate_limiter = RateLimiter::new(5, 2); // 5 requests per second, burst of 2
    
    for i in 0..10 {
        match rate_limiter.acquire().await {
            Ok(()) => info!("Rate limiter: Request {} allowed", i),
            Err(e) => error!("Rate limiter: Request {} blocked: {:?}", i, e),
        }
        sleep(Duration::from_millis(100)).await;
    }
    
    // Example 6: Use graceful shutdown
    let shutdown_coordinator = ShutdownCoordinator::new();
    let mut shutdown_signal = shutdown_coordinator.shutdown_signal();
    
    let task_handle = tokio::spawn(async move {
        tokio::select! {
            _ = shutdown_signal.recv() => {
                info!("Task received shutdown signal");
                Ok(())
            }
            _ = async {
                loop {
                    info!("Task is running...");
                    sleep(Duration::from_secs(1)).await;
                }
            } => {
                Ok(())
            }
        }
    });
    
    shutdown_coordinator.register_task("example-task", task_handle).await;
    
    // Wait a bit then trigger shutdown
    sleep(Duration::from_secs(3)).await;
    shutdown_coordinator.shutdown(Duration::from_secs(5)).await?;
    
    info!("Example completed successfully");
    Ok(())
}