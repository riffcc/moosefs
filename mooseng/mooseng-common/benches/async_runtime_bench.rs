//! Benchmarks for async runtime components

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use mooseng_common::{
    channels, RateLimiter, CircuitBreaker, CircuitBreakerConfig,
    retry_with_backoff, RetryConfig,
};
use std::time::Duration;
use tokio::runtime::Runtime;

fn channel_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("channel_throughput");
    
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("bounded_mpsc", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async move {
                let (tx, mut rx) = channels::bounded::<u64>(100);
                
                tokio::spawn(async move {
                    for i in 0..size {
                        tx.send(i).await.unwrap();
                    }
                });
                
                let mut sum = 0u64;
                while let Some(val) = rx.recv().await {
                    sum += val;
                }
                black_box(sum);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("unbounded_mpsc", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async move {
                let (tx, mut rx) = channels::unbounded::<u64>();
                
                tokio::spawn(async move {
                    for i in 0..size {
                        tx.send(i).unwrap();
                    }
                });
                
                let mut sum = 0u64;
                while let Some(val) = rx.recv().await {
                    sum += val;
                }
                black_box(sum);
            });
        });
    }
    
    group.finish();
}

fn rate_limiter_bench(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("rate_limiter");
    
    group.bench_function("acquire_under_limit", |b| {
        b.to_async(&rt).iter(|| async {
            let limiter = RateLimiter::new(1000, 100); // High limit
            limiter.acquire().await.unwrap();
        });
    });
    
    group.bench_function("try_acquire_under_limit", |b| {
        b.to_async(&rt).iter(|| async {
            let limiter = RateLimiter::new(1000, 100); // High limit
            limiter.try_acquire().unwrap();
        });
    });
    
    group.finish();
}

fn circuit_breaker_bench(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("circuit_breaker");
    
    group.bench_function("successful_call", |b| {
        b.to_async(&rt).iter(|| async {
            let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
            cb.call(|| async { Ok::<_, mooseng_common::error::MooseNGError>(42) }).await.unwrap();
        });
    });
    
    group.bench_function("state_check", |b| {
        b.to_async(&rt).iter(|| async {
            let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
            black_box(cb.state().await);
        });
    });
    
    group.finish();
}

fn retry_bench(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("retry");
    
    group.bench_function("immediate_success", |b| {
        b.to_async(&rt).iter(|| async {
            retry_with_backoff(
                RetryConfig {
                    max_attempts: 3,
                    initial_backoff: Duration::from_millis(1),
                    ..Default::default()
                },
                || async { Ok::<_, &str>(42) },
            ).await.unwrap();
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    channel_throughput,
    rate_limiter_bench,
    circuit_breaker_bench,
    retry_bench
);
criterion_main!(benches);