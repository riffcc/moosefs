//! Comprehensive async runtime module for MooseNG
//! 
//! This module provides:
//! - Configurable Tokio runtime initialization
//! - Async channel abstractions for inter-component communication
//! - Async utilities (timeouts, retries, circuit breakers)
//! - Async stream processing utilities
//! - Graceful shutdown mechanisms

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{broadcast, mpsc, oneshot, watch, Mutex, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{interval, sleep, timeout};
use tracing::{error, info, warn};

use crate::error::MooseNGError;

/// Runtime configuration
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Name of the runtime
    pub name: String,
    /// Number of worker threads (None for CPU count)
    pub worker_threads: Option<usize>,
    /// Size of the thread stack (in bytes)
    pub thread_stack_size: Option<usize>,
    /// Enable I/O driver
    pub enable_io: bool,
    /// Enable time driver
    pub enable_time: bool,
    /// Maximum blocking threads
    pub max_blocking_threads: Option<usize>,
    /// Thread keep-alive duration
    pub thread_keep_alive: Option<Duration>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            name: "mooseng-runtime".to_string(),
            worker_threads: None, // Use CPU count
            thread_stack_size: Some(2 * 1024 * 1024), // 2MB
            enable_io: true,
            enable_time: true,
            max_blocking_threads: Some(512),
            thread_keep_alive: Some(Duration::from_secs(10)),
        }
    }
}

/// Async runtime manager
pub struct AsyncRuntime {
    runtime: Arc<Runtime>,
    shutdown_signal: Arc<AtomicBool>,
    shutdown_tx: broadcast::Sender<()>,
}

impl AsyncRuntime {
    /// Create a new async runtime with the given configuration
    pub fn new(config: RuntimeConfig) -> Result<Self> {
        let mut builder = Builder::new_multi_thread();
        
        builder.thread_name(config.name);
        
        if let Some(threads) = config.worker_threads {
            builder.worker_threads(threads);
        }
        
        if let Some(stack_size) = config.thread_stack_size {
            builder.thread_stack_size(stack_size);
        }
        
        if config.enable_io {
            builder.enable_io();
        }
        
        if config.enable_time {
            builder.enable_time();
        }
        
        if let Some(max_blocking) = config.max_blocking_threads {
            builder.max_blocking_threads(max_blocking);
        }
        
        if let Some(keep_alive) = config.thread_keep_alive {
            builder.thread_keep_alive(keep_alive);
        }
        
        let runtime = builder
            .build()
            .context("Failed to build Tokio runtime")?;
        
        let (shutdown_tx, _) = broadcast::channel(1);
        
        Ok(Self {
            runtime: Arc::new(runtime),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            shutdown_tx,
        })
    }
    
    /// Get a reference to the runtime
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
    
    /// Spawn a named task
    pub fn spawn_named<F>(&self, _name: String, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let handle = self.runtime.spawn(future);
        // Note: We can't store the handle directly as JoinHandle is not Clone
        // This is a limitation we'll have to work around
        handle
    }
    
    /// Subscribe to shutdown signal
    pub fn shutdown_signal(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }
    
    /// Trigger graceful shutdown
    pub async fn shutdown(&self) {
        info!("Initiating graceful shutdown");
        self.shutdown_signal.store(true, Ordering::SeqCst);
        let _ = self.shutdown_tx.send(());
        
        // Since we can't store JoinHandles, we'll just signal shutdown
        // and rely on tasks to handle the shutdown signal
        
        info!("Graceful shutdown signaled");
    }
    
    /// Check if shutdown has been requested
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_signal.load(Ordering::SeqCst)
    }
}

/// Async channel types
pub mod channels {
    use super::*;
    
    /// Bounded MPSC channel
    pub fn bounded<T>(buffer: usize) -> (mpsc::Sender<T>, mpsc::Receiver<T>) {
        mpsc::channel(buffer)
    }
    
    /// Unbounded MPSC channel
    pub fn unbounded<T>() -> (mpsc::UnboundedSender<T>, mpsc::UnboundedReceiver<T>) {
        mpsc::unbounded_channel()
    }
    
    /// Oneshot channel for single value communication
    pub fn oneshot<T>() -> (oneshot::Sender<T>, oneshot::Receiver<T>) {
        oneshot::channel()
    }
    
    /// Watch channel for state broadcasting
    pub fn watch<T>(initial: T) -> (watch::Sender<T>, watch::Receiver<T>) {
        watch::channel(initial)
    }
    
    /// Broadcast channel for multi-consumer scenarios
    pub fn broadcast<T: Clone>(capacity: usize) -> (broadcast::Sender<T>, broadcast::Receiver<T>) {
        broadcast::channel(capacity)
    }
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts
    pub max_attempts: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
    /// Add jitter to backoff
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Retry with exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    config: RetryConfig,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut backoff = config.initial_backoff;
    
    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt == config.max_attempts => {
                error!("All {} retry attempts failed: {}", config.max_attempts, e);
                return Err(e);
            }
            Err(e) => {
                warn!("Attempt {} failed: {}, retrying after {:?}", attempt, e, backoff);
                
                let mut delay = backoff;
                if config.jitter {
                    use rand::Rng;
                    let jitter = rand::thread_rng().gen_range(0..=backoff.as_millis() / 4) as u64;
                    delay += Duration::from_millis(jitter);
                }
                
                sleep(delay).await;
                
                backoff = Duration::from_secs_f64(
                    (backoff.as_secs_f64() * config.multiplier).min(config.max_backoff.as_secs_f64())
                );
            }
        }
    }
    
    unreachable!()
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open circuit
    pub failure_threshold: u32,
    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,
    /// Timeout before attempting to close circuit
    pub timeout: Duration,
    /// Time window for counting failures
    pub window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
            window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<AtomicU64>,
    success_count: Arc<AtomicU64>,
    last_failure_time: Arc<Mutex<Option<Instant>>>,
    last_state_change: Arc<Mutex<Instant>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
            last_failure_time: Arc::new(Mutex::new(None)),
            last_state_change: Arc::new(Mutex::new(Instant::now())),
        }
    }
    
    /// Get current circuit state
    pub async fn state(&self) -> CircuitState {
        let state = *self.state.read().await;
        
        if state == CircuitState::Open {
            let last_change = *self.last_state_change.lock().await;
            if last_change.elapsed() >= self.config.timeout {
                *self.state.write().await = CircuitState::HalfOpen;
                *self.last_state_change.lock().await = Instant::now();
                self.failure_count.store(0, Ordering::SeqCst);
                self.success_count.store(0, Ordering::SeqCst);
                return CircuitState::HalfOpen;
            }
        }
        
        state
    }
    
    /// Call through circuit breaker
    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: From<MooseNGError>,
    {
        match self.state().await {
            CircuitState::Open => {
                Err(MooseNGError::ResourceExhausted("Circuit breaker is open".into()).into())
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                match operation().await {
                    Ok(result) => {
                        self.on_success().await;
                        Ok(result)
                    }
                    Err(e) => {
                        self.on_failure().await;
                        Err(e)
                    }
                }
            }
        }
    }
    
    /// Record success
    async fn on_success(&self) {
        let state = *self.state.read().await;
        
        match state {
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.success_threshold as u64 {
                    *self.state.write().await = CircuitState::Closed;
                    *self.last_state_change.lock().await = Instant::now();
                    self.failure_count.store(0, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                    info!("Circuit breaker closed after {} successes", count);
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            _ => {}
        }
    }
    
    /// Record failure
    async fn on_failure(&self) {
        let now = Instant::now();
        let mut last_failure = self.last_failure_time.lock().await;
        
        // Check if we're within the window
        if let Some(last) = *last_failure {
            if now.duration_since(last) > self.config.window {
                self.failure_count.store(0, Ordering::SeqCst);
            }
        }
        
        *last_failure = Some(now);
        drop(last_failure);
        
        let state = *self.state.read().await;
        
        match state {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.failure_threshold as u64 {
                    *self.state.write().await = CircuitState::Open;
                    *self.last_state_change.lock().await = Instant::now();
                    warn!("Circuit breaker opened after {} failures", count);
                }
            }
            CircuitState::HalfOpen => {
                *self.state.write().await = CircuitState::Open;
                *self.last_state_change.lock().await = Instant::now();
                self.success_count.store(0, Ordering::SeqCst);
                warn!("Circuit breaker reopened on failure in half-open state");
            }
            _ => {}
        }
    }
}

/// Stream processing utilities
pub mod streams {
    use super::*;
    
    /// Batch items from a stream
    pub fn batch<S, T>(
        stream: S,
        size: usize,
        timeout: Duration,
    ) -> impl Stream<Item = Vec<T>>
    where
        S: Stream<Item = T> + Send + 'static,
        T: Send + 'static,
    {
        tokio_stream::StreamExt::chunks_timeout(stream, size, timeout)
            .map(|chunk: Vec<T>| chunk)
    }
    
    /// Rate limit a stream
    pub fn rate_limit<S, T>(
        stream: S,
        items_per_second: u32,
    ) -> impl Stream<Item = T>
    where
        S: Stream<Item = T> + Send + 'static,
        T: Send + 'static,
    {
        let interval_duration = Duration::from_secs_f64(1.0 / items_per_second as f64);
        
        tokio_stream::StreamExt::throttle(stream, interval_duration)
    }
    
    /// Buffer stream with backpressure
    pub fn buffered_with_backpressure<T>(
        buffer_size: usize,
    ) -> (mpsc::Sender<T>, impl Stream<Item = T>)
    where
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::channel(buffer_size);
        (tx, tokio_stream::wrappers::ReceiverStream::new(rx))
    }
    
    /// Concurrent stream processing with limited parallelism
    pub fn concurrent_process<S, T, F, Fut, U>(
        stream: S,
        concurrency: usize,
        processor: F,
    ) -> impl Stream<Item = U>
    where
        S: Stream<Item = T> + Send + 'static,
        T: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = U> + Send,
        U: Send + 'static,
    {
        use futures::StreamExt;
        stream.map(processor).buffer_unordered(concurrency)
    }
}

/// Timeout utilities
pub mod timeouts {
    use super::*;
    
    /// Execute a future with timeout
    pub async fn with_timeout<F, T>(duration: Duration, future: F) -> Result<T>
    where
        F: Future<Output = T>,
    {
        timeout(duration, future)
            .await
            .map_err(|_| MooseNGError::Timeout.into())
    }
    
    /// Execute with timeout and default value
    pub async fn with_timeout_or_default<F, T>(duration: Duration, future: F, default: T) -> T
    where
        F: Future<Output = T>,
    {
        timeout(duration, future).await.unwrap_or(default)
    }
}

/// Graceful shutdown coordinator
#[derive(Clone)]
pub struct ShutdownCoordinator {
    shutdown_tx: broadcast::Sender<()>,
    tasks: Arc<Mutex<Vec<(&'static str, JoinHandle<Result<()>>)>>>,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            shutdown_tx,
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Register a task for graceful shutdown
    pub async fn register_task(&self, name: &'static str, handle: JoinHandle<Result<()>>) {
        self.tasks.lock().await.push((name, handle));
    }
    
    /// Get shutdown signal receiver
    pub fn shutdown_signal(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }
    
    /// Trigger shutdown and wait for all tasks
    pub async fn shutdown(self, timeout_duration: Duration) -> Result<()> {
        info!("Initiating coordinated shutdown");
        
        // Send shutdown signal
        let _ = self.shutdown_tx.send(());
        
        // Wait for all tasks with timeout
        let mut tasks = self.tasks.lock().await;
        let mut failed_tasks = Vec::new();
        
        // Take ownership of all handles
        let task_handles: Vec<_> = tasks.drain(..).collect();
        drop(tasks); // Release the lock
        
        for (name, handle) in task_handles {
            match timeout(timeout_duration, handle).await {
                Ok(Ok(Ok(()))) => info!("Task '{}' shutdown successfully", name),
                Ok(Ok(Err(e))) => {
                    error!("Task '{}' failed during shutdown: {:?}", name, e);
                    failed_tasks.push(name);
                }
                Ok(Err(e)) => {
                    error!("Task '{}' panicked during shutdown: {:?}", name, e);
                    failed_tasks.push(name);
                }
                Err(_) => {
                    error!("Task '{}' timed out during shutdown", name);
                    failed_tasks.push(name);
                }
            }
        }
        
        if failed_tasks.is_empty() {
            info!("All tasks shutdown successfully");
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to shutdown tasks: {:?}",
                failed_tasks
            ))
        }
    }
}

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    _refill_task: JoinHandle<()>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(rate: u32, burst: u32) -> Self {
        let semaphore = Arc::new(Semaphore::new(burst as usize));
        let sem_clone = semaphore.clone();
        
        let refill_interval = Duration::from_secs_f64(1.0 / rate as f64);
        let refill_task = tokio::spawn(async move {
            let mut interval = interval(refill_interval);
            // Important: skip the first immediate tick
            interval.tick().await;
            loop {
                interval.tick().await;
                if sem_clone.available_permits() < burst as usize {
                    sem_clone.add_permits(1);
                }
            }
        });
        
        Self {
            semaphore,
            _refill_task: refill_task,
        }
    }
    
    /// Acquire a permit
    pub async fn acquire(&self) -> Result<()> {
        match self.semaphore.acquire().await {
            Ok(permit) => {
                // Important: we need to forget the permit to consume it
                permit.forget();
                Ok(())
            }
            Err(_) => Err(MooseNGError::ResourceExhausted("Rate limit exceeded".into()).into()),
        }
    }
    
    /// Try to acquire a permit without waiting
    pub fn try_acquire(&self) -> Result<()> {
        match self.semaphore.try_acquire() {
            Ok(permit) => {
                // Important: we need to forget the permit to consume it
                permit.forget();
                Ok(())
            }
            Err(_) => Err(MooseNGError::ResourceExhausted("Rate limit exceeded".into()).into()),
        }
    }
}

/// Task supervisor trait
#[async_trait]
pub trait TaskSupervisor: Send + Sync {
    /// Start the supervised task
    async fn start(&mut self) -> Result<()>;
    
    /// Stop the supervised task
    async fn stop(&mut self) -> Result<()>;
    
    /// Check if the task is healthy
    async fn health_check(&self) -> Result<()>;
    
    /// Restart the task
    async fn restart(&mut self) -> Result<()> {
        self.stop().await?;
        self.start().await
    }
}

/// Generic task supervisor implementation
pub struct GenericSupervisor<T> {
    name: String,
    task: T,
    handle: Option<JoinHandle<Result<()>>>,
    restart_policy: RestartPolicy,
}

/// Restart policy for supervised tasks
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    pub max_restarts: u32,
    pub restart_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            restart_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
        }
    }
}

impl<T> GenericSupervisor<T>
where
    T: TaskSupervisor + 'static,
{
    /// Create a new supervised task
    pub fn new(name: String, task: T, restart_policy: RestartPolicy) -> Self {
        Self {
            name,
            task,
            handle: None,
            restart_policy,
        }
    }
    
    /// Run the supervisor
    pub async fn run(&mut self, mut shutdown: broadcast::Receiver<()>) -> Result<()> {
        let mut restart_count = 0;
        let mut restart_delay = self.restart_policy.restart_delay;
        
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Supervisor {} received shutdown signal", self.name);
                    if self.handle.is_some() {
                        self.task.stop().await?;
                    }
                    break;
                }
                result = async {
                    if self.handle.is_none() {
                        self.task.start().await?;
                        self.handle = Some(tokio::spawn(async move {
                            // Task implementation would go here
                            Ok(())
                        }));
                    }
                    
                    // Monitor task health
                    sleep(Duration::from_secs(10)).await;
                    self.task.health_check().await
                } => {
                    match result {
                        Ok(()) => {
                            restart_count = 0;
                            restart_delay = self.restart_policy.restart_delay;
                        }
                        Err(e) => {
                            error!("Task {} failed health check: {:?}", self.name, e);
                            
                            if restart_count >= self.restart_policy.max_restarts {
                                error!("Task {} exceeded max restarts", self.name);
                                return Err(anyhow::anyhow!("Task exceeded max restarts"));
                            }
                            
                            warn!("Restarting task {} after {:?}", self.name, restart_delay);
                            sleep(restart_delay).await;
                            
                            if let Err(e) = self.task.restart().await {
                                error!("Failed to restart task {}: {:?}", self.name, e);
                            }
                            
                            restart_count += 1;
                            restart_delay = Duration::from_secs_f64(
                                restart_delay.as_secs_f64() * self.restart_policy.backoff_multiplier
                            );
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_runtime_creation() {
        let config = RuntimeConfig::default();
        let runtime = AsyncRuntime::new(config).unwrap();
        assert!(!runtime.is_shutting_down());
    }
    
    #[tokio::test]
    async fn test_retry_success() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let attempts = AtomicU32::new(0);
        let result = retry_with_backoff(
            RetryConfig {
                max_attempts: 3,
                initial_backoff: Duration::from_millis(10),
                ..Default::default()
            },
            || async {
                let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if count < 2 {
                    Err("Failed")
                } else {
                    Ok("Success")
                }
            },
        )
        .await;
        
        assert_eq!(result.unwrap(), "Success");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout: Duration::from_millis(100),
            ..Default::default()
        });
        
        // Should start closed
        assert_eq!(cb.state().await, CircuitState::Closed);
        
        // First failure
        let _ = cb.call::<_, _, (), MooseNGError>(|| async {
            Err(MooseNGError::Other("Test error".into()))
        }).await;
        
        // Still closed after one failure
        assert_eq!(cb.state().await, CircuitState::Closed);
        
        // Second failure should open circuit
        let _ = cb.call::<_, _, (), MooseNGError>(|| async {
            Err(MooseNGError::Other("Test error".into()))
        }).await;
        
        assert_eq!(cb.state().await, CircuitState::Open);
        
        // Wait for timeout
        sleep(Duration::from_millis(150)).await;
        
        // Should be half-open now
        assert_eq!(cb.state().await, CircuitState::HalfOpen);
    }
    
    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(10, 2); // 10/sec, burst of 2
        
        // Should allow initial burst without waiting
        let start = std::time::Instant::now();
        assert!(limiter.acquire().await.is_ok());
        assert!(limiter.acquire().await.is_ok());
        let burst_time = start.elapsed();
        assert!(burst_time < Duration::from_millis(50), "Burst should be immediate");
        
        // Exhaust permits and verify we're rate limited
        assert!(limiter.try_acquire().is_err(), "Should be out of permits");
        
        // Wait for a refill and try again
        sleep(Duration::from_millis(150)).await; // Wait for at least one refill
        assert!(limiter.try_acquire().is_ok(), "Should have refilled a permit");
    }
}