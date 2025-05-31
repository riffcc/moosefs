//! Performance benchmarks for the Raft consensus implementation
//! 
//! These benchmarks help identify performance bottlenecks and ensure
//! the Raft implementation meets performance requirements.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use mooseng_master::raft::{RaftConsensus, RaftConfig, LogCommand, Term, LogIndex};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Helper to create a benchmark-optimized Raft configuration
fn create_bench_config(node_id: &str, peer_count: usize) -> RaftConfig {
    let peers: Vec<String> = (0..peer_count)
        .map(|i| format!("peer_{}", i))
        .collect();
    
    RaftConfig {
        node_id: node_id.to_string(),
        peers,
        election_timeout_min: Duration::from_millis(150),
        election_timeout_max: Duration::from_millis(300),
        heartbeat_interval: Duration::from_millis(50),
        rpc_timeout: Duration::from_millis(100),
        max_log_entries_per_request: 100,
        snapshot_threshold: 10000, // Higher threshold for benchmarks
        snapshot_chunk_size: 64 * 1024,
        data_dir: "/tmp/raft_bench".to_string(),
    }
}

/// Benchmark Raft instance creation
fn bench_raft_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("raft_creation_single_peer", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = create_bench_config("bench_node", 1);
                let raft = RaftConsensus::new(config).await.unwrap();
                black_box(raft);
            });
        });
    });
    
    // Benchmark with different peer counts
    let mut group = c.benchmark_group("raft_creation_scaling");
    for peer_count in [1, 3, 5, 7, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("peers", peer_count),
            peer_count,
            |b, &peer_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = create_bench_config("bench_node", peer_count);
                        let raft = RaftConsensus::new(config).await.unwrap();
                        black_box(raft);
                    });
                });
            },
        );
    }
    group.finish();
}

/// Benchmark safety check operations
fn bench_safety_checks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let raft = rt.block_on(async {
        let config = create_bench_config("bench_node", 3);
        RaftConsensus::new(config).await.unwrap()
    });
    
    c.bench_function("safety_check", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = raft.check_safety().await;
                black_box(result);
            });
        });
    });
    
    c.bench_function("can_start_election", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = raft.can_start_election().await;
                black_box(result);
            });
        });
    });
    
    c.bench_function("validate_vote_request", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = raft.validate_vote_request(
                    Term(1),
                    "candidate",
                    LogIndex(0),
                    Term(0),
                ).await;
                black_box(result);
            });
        });
    });
}

/// Benchmark configuration management operations
fn bench_configuration_management(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let raft = rt.block_on(async {
        let config = create_bench_config("bench_node", 5);
        RaftConsensus::new(config).await.unwrap()
    });
    
    c.bench_function("get_cluster_config", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = raft.get_cluster_config().await;
                black_box(config);
            });
        });
    });
    
    c.bench_function("has_pending_config_change", |b| {
        b.iter(|| {
            rt.block_on(async {
                let pending = raft.has_pending_config_change().await;
                black_box(pending);
            });
        });
    });
    
    c.bench_function("get_replication_targets", |b| {
        b.iter(|| {
            rt.block_on(async {
                let targets = raft.get_replication_targets().await;
                black_box(targets);
            });
        });
    });
    
    c.bench_function("get_majority_size", |b| {
        b.iter(|| {
            rt.block_on(async {
                let majority = raft.get_majority_size().await;
                black_box(majority);
            });
        });
    });
}

/// Benchmark read scaling operations
fn bench_read_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let raft = rt.block_on(async {
        let config = create_bench_config("bench_node", 3);
        RaftConsensus::new(config).await.unwrap()
    });
    
    use mooseng_master::raft::{ReadRequest, ReadConsistency, ReadOnlyQuery};
    
    let linearizable_request = ReadRequest {
        consistency: ReadConsistency::Linearizable,
        min_commit_index: None,
    };
    
    let sequential_request = ReadRequest {
        consistency: ReadConsistency::Sequential,
        min_commit_index: None,
    };
    
    c.bench_function("can_serve_read_linearizable", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = raft.can_serve_read(&linearizable_request).await;
                black_box(result);
            });
        });
    });
    
    c.bench_function("can_serve_read_sequential", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = raft.can_serve_read(&sequential_request).await;
                black_box(result);
            });
        });
    });
    
    c.bench_function("process_read_only_query", |b| {
        b.iter(|| {
            rt.block_on(async {
                let query = ReadOnlyQuery {
                    query_id: "bench_query".to_string(),
                    consistency: ReadConsistency::Sequential,
                    query_data: b"bench_data".to_vec(),
                };
                let result = raft.process_read_only_query(query).await;
                black_box(result);
            });
        });
    });
    
    c.bench_function("needs_lease_renewal", |b| {
        b.iter(|| {
            rt.block_on(async {
                let needs_renewal = raft.needs_lease_renewal().await;
                black_box(needs_renewal);
            });
        });
    });
    
    c.bench_function("validate_read_lease", |b| {
        b.iter(|| {
            rt.block_on(async {
                let is_valid = raft.validate_read_lease().await;
                black_box(is_valid);
            });
        });
    });
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let raft = rt.block_on(async {
        let config = create_bench_config("bench_node", 3);
        RaftConsensus::new(config).await.unwrap()
    });
    
    use std::sync::Arc;
    use tokio::task::JoinSet;
    
    let mut group = c.benchmark_group("concurrent_safety_checks");
    for concurrency in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_tasks", concurrency),
            concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    rt.block_on(async {
                        let raft = Arc::new(raft.clone());
                        let mut tasks = JoinSet::new();
                        
                        for _ in 0..concurrency {
                            let raft_clone = raft.clone();
                            tasks.spawn(async move {
                                for _ in 0..10 {
                                    let _ = raft_clone.check_safety().await;
                                    let _ = raft_clone.is_leader().await;
                                    let _ = raft_clone.get_cluster_config().await;
                                }
                            });
                        }
                        
                        while let Some(_) = tasks.join_next().await {}
                        black_box(());
                    });
                });
            },
        );
    }
    group.finish();
}

/// Benchmark leadership operations
fn bench_leadership_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let raft = rt.block_on(async {
        let config = create_bench_config("bench_node", 3);
        RaftConsensus::new(config).await.unwrap()
    });
    
    c.bench_function("is_leader", |b| {
        b.iter(|| {
            rt.block_on(async {
                let is_leader = raft.is_leader().await;
                black_box(is_leader);
            });
        });
    });
    
    c.bench_function("get_leader_id", |b| {
        b.iter(|| {
            rt.block_on(async {
                let leader_id = raft.get_leader_id().await;
                black_box(leader_id);
            });
        });
    });
}

/// Benchmark startup and shutdown operations
fn bench_lifecycle_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("raft_startup_shutdown", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = create_bench_config("bench_node", 3);
                let raft = RaftConsensus::new(config).await.unwrap();
                
                let _ = raft.start().await;
                let _ = raft.stop().await;
                
                black_box(raft);
            });
        });
    });
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("memory_allocation_pattern", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut rafts = Vec::new();
                
                // Create multiple instances to test memory patterns
                for i in 0..10 {
                    let config = create_bench_config(&format!("node_{}", i), 3);
                    let raft = RaftConsensus::new(config).await.unwrap();
                    rafts.push(raft);
                }
                
                // Perform operations on each
                for raft in &rafts {
                    let _ = raft.check_safety().await;
                    let _ = raft.is_leader().await;
                }
                
                black_box(rafts);
            });
        });
    });
}

criterion_group!(
    benches,
    bench_raft_creation,
    bench_safety_checks,
    bench_configuration_management,
    bench_read_scaling,
    bench_concurrent_operations,
    bench_leadership_operations,
    bench_lifecycle_operations,
    bench_memory_usage
);

criterion_main!(benches);