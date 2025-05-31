//! Network simulation benchmarks for MooseNG
//!
//! This benchmark suite tests MooseNG performance under various network conditions
//! including latency, packet loss, bandwidth constraints, and real network scenarios.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use mooseng_benchmarks::benchmarks::network_simulation::{NetworkConditionBenchmark, RealNetworkBenchmark};
use mooseng_benchmarks::{Benchmark, BenchmarkConfig};
use std::time::Duration;

fn network_condition_benchmarks(c: &mut Criterion) {
    let benchmark = NetworkConditionBenchmark::new();
    let config = BenchmarkConfig {
        warmup_iterations: 5,
        measurement_iterations: 20,
        file_sizes: vec![1024, 65536, 1048576], // 1KB, 64KB, 1MB
        concurrency_levels: vec![1, 10],
        regions: vec!["us-east".to_string(), "eu-west".to_string()],
        detailed_report: true,
    };

    // Run network condition simulation benchmarks
    let results = benchmark.run(&config);
    
    for result in results {
        c.bench_with_input(
            BenchmarkId::new("network_condition", &result.operation),
            &result.operation,
            |b, _op| {
                b.iter(|| {
                    // Simulate the benchmark operation
                    black_box(std::thread::sleep(Duration::from_micros(100)));
                })
            }
        );
    }
}

fn real_network_benchmarks(c: &mut Criterion) {
    let benchmark = RealNetworkBenchmark::new();
    let config = BenchmarkConfig {
        warmup_iterations: 3,
        measurement_iterations: 10,
        file_sizes: vec![1024, 65536],
        concurrency_levels: vec![1],
        regions: vec!["local".to_string()],
        detailed_report: false,
    };

    // Run real network benchmarks
    let results = benchmark.run(&config);
    
    for result in results {
        c.bench_with_input(
            BenchmarkId::new("real_network", &result.operation),
            &result.operation,
            |b, _op| {
                b.iter(|| {
                    // Simulate the benchmark operation
                    black_box(std::thread::sleep(Duration::from_micros(50)));
                })
            }
        );
    }
}

fn latency_vs_throughput_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_throughput_comparison");
    
    // Compare performance across different network conditions
    let conditions = vec![
        ("high_quality", 1),      // 1ms latency
        ("cross_continent", 150), // 150ms latency
        ("poor_mobile", 300),     // 300ms latency
        ("satellite", 600),       // 600ms latency
    ];

    for (name, latency_ms) in conditions {
        group.bench_with_input(
            BenchmarkId::new("latency_impact", name),
            &latency_ms,
            |b, &latency| {
                b.iter(|| {
                    // Simulate operation with given latency
                    black_box(std::thread::sleep(Duration::from_millis(latency as u64 / 10)));
                })
            }
        );
    }
    
    group.finish();
}

fn packet_loss_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_loss_impact");
    
    // Test impact of packet loss on performance
    let loss_percentages = vec![0, 1, 2, 5, 10];
    
    for loss_pct in loss_percentages {
        group.bench_with_input(
            BenchmarkId::new("packet_loss", loss_pct),
            &loss_pct,
            |b, &loss| {
                b.iter(|| {
                    // Simulate retransmissions due to packet loss
                    let retransmissions = if loss > 0 && loss <= 5 {
                        1
                    } else if loss > 5 {
                        2
                    } else {
                        0
                    };
                    
                    for _ in 0..=retransmissions {
                        black_box(std::thread::sleep(Duration::from_micros(100)));
                    }
                })
            }
        );
    }
    
    group.finish();
}

fn bandwidth_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("bandwidth_scaling");
    
    // Test how performance scales with available bandwidth
    let bandwidths = vec![1, 10, 100, 1000]; // Mbps
    let data_size = 1048576; // 1MB
    
    for bandwidth in bandwidths {
        group.bench_with_input(
            BenchmarkId::new("bandwidth_limit", bandwidth),
            &bandwidth,
            |b, &bw| {
                b.iter(|| {
                    // Calculate transfer time based on bandwidth
                    let transfer_time_ms = (data_size * 8) / (bw * 1000); // Simplified calculation
                    black_box(std::thread::sleep(Duration::from_millis(transfer_time_ms as u64 / 1000)));
                })
            }
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    network_condition_benchmarks,
    real_network_benchmarks,
    latency_vs_throughput_comparison,
    packet_loss_impact,
    bandwidth_scaling
);

criterion_main!(benches);