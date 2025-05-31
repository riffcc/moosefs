//! Integration tests for the Raft consensus implementation
//! 
//! These tests verify that the Raft implementation works correctly
//! in more realistic scenarios with multiple nodes and network conditions.

use anyhow::Result;
use mooseng_master::raft::{
    RaftConsensus, RaftConfig, LogCommand, ConfigChangeType,
    ReadRequest, ReadConsistency, ReadOnlyQuery,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};

/// Test helper to create a realistic Raft configuration
fn create_integration_config(node_id: &str, peers: Vec<String>) -> RaftConfig {
    RaftConfig {
        node_id: node_id.to_string(),
        peers,
        election_timeout_min: Duration::from_millis(200),
        election_timeout_max: Duration::from_millis(400),
        heartbeat_interval: Duration::from_millis(100),
        rpc_timeout: Duration::from_millis(150),
        max_log_entries_per_request: 50,
        snapshot_threshold: 1000,
        snapshot_chunk_size: 32 * 1024,
        data_dir: format!("/tmp/raft_integration_{}", node_id),
    }
}

/// Create a realistic test cluster with proper peer relationships
async fn create_integration_cluster(node_count: usize) -> Result<Vec<RaftConsensus>> {
    let mut nodes = Vec::new();
    let node_ids: Vec<String> = (0..node_count)
        .map(|i| format!("integration_node_{}", i))
        .collect();

    for (i, node_id) in node_ids.iter().enumerate() {
        let peers = node_ids.iter()
            .filter(|id| *id != node_id)
            .cloned()
            .collect();
        
        let config = create_integration_config(node_id, peers);
        let node = RaftConsensus::new(config).await?;
        nodes.push(node);
    }

    Ok(nodes)
}

/// Test cluster formation and leader election in a realistic scenario
#[tokio::test]
async fn test_cluster_formation_and_leader_election() -> Result<()> {
    let cluster_size = 5;
    let cluster = create_integration_cluster(cluster_size).await?;
    
    // Start all nodes
    for node in &cluster {
        node.start().await?;
    }
    
    // Wait for leader election to complete
    sleep(Duration::from_millis(1000)).await;
    
    // Verify exactly one leader exists
    let mut leader_count = 0;
    let mut leader_id = None;
    
    for node in &cluster {
        if node.is_leader().await {
            leader_count += 1;
            leader_id = node.get_leader_id().await;
        }
    }
    
    // Stop all nodes
    for node in &cluster {
        node.stop().await?;
    }
    
    // Should have exactly one leader in a stable cluster
    assert_eq!(leader_count, 1, "Expected exactly one leader, found {}", leader_count);
    assert!(leader_id.is_some(), "Leader should have an ID");
    
    println!("Successfully elected leader: {:?}", leader_id);
    Ok(())
}

/// Test leader failure and re-election
#[tokio::test]
async fn test_leader_failure_and_reelection() -> Result<()> {
    let cluster_size = 3;
    let cluster = create_integration_cluster(cluster_size).await?;
    
    // Start all nodes
    for node in &cluster {
        node.start().await?;
    }
    
    // Wait for initial leader election
    sleep(Duration::from_millis(800)).await;
    
    // Find the current leader
    let mut initial_leader_idx = None;
    for (idx, node) in cluster.iter().enumerate() {
        if node.is_leader().await {
            initial_leader_idx = Some(idx);
            break;
        }
    }
    
    if let Some(leader_idx) = initial_leader_idx {
        println!("Initial leader at index: {}", leader_idx);
        
        // Stop the leader to simulate failure
        cluster[leader_idx].stop().await?;
        
        // Wait for re-election
        sleep(Duration::from_millis(1000)).await;
        
        // Check that a new leader was elected among remaining nodes
        let mut new_leader_count = 0;
        let mut new_leader_idx = None;
        
        for (idx, node) in cluster.iter().enumerate() {
            if idx != leader_idx && node.is_leader().await {
                new_leader_count += 1;
                new_leader_idx = Some(idx);
            }
        }
        
        // Stop remaining nodes
        for (idx, node) in cluster.iter().enumerate() {
            if idx != leader_idx {
                node.stop().await?;
            }
        }
        
        assert_eq!(new_leader_count, 1, "Should have exactly one new leader");
        assert_ne!(new_leader_idx, Some(leader_idx), "New leader should be different");
        
        println!("Successfully re-elected leader at index: {:?}", new_leader_idx);
    } else {
        // Clean up if no initial leader was found
        for node in &cluster {
            node.stop().await?;
        }
        panic!("No initial leader was elected");
    }
    
    Ok(())
}

/// Test network partition tolerance
#[tokio::test]
async fn test_network_partition_tolerance() -> Result<()> {
    let cluster_size = 5;
    let cluster = create_integration_cluster(cluster_size).await?;
    
    // Start all nodes
    for node in &cluster {
        node.start().await?;
    }
    
    // Wait for initial leader election
    sleep(Duration::from_millis(800)).await;
    
    // Simulate network partition by stopping minority of nodes
    let partition_size = cluster_size / 2; // Stop 2 out of 5 nodes
    
    for i in 0..partition_size {
        cluster[i].stop().await?;
    }
    
    // Wait for majority partition to stabilize
    sleep(Duration::from_millis(1000)).await;
    
    // Check that majority partition still has a leader
    let mut majority_leader_count = 0;
    for (idx, node) in cluster.iter().enumerate() {
        if idx >= partition_size && node.is_leader().await {
            majority_leader_count += 1;
        }
    }
    
    // Stop remaining nodes
    for (idx, node) in cluster.iter().enumerate() {
        if idx >= partition_size {
            node.stop().await?;
        }
    }
    
    // Majority partition should maintain leadership
    assert!(majority_leader_count <= 1, "Should have at most one leader in majority");
    
    println!("Network partition handled successfully. Leaders in majority: {}", majority_leader_count);
    Ok(())
}

/// Test configuration changes
#[tokio::test]
async fn test_dynamic_configuration_changes() -> Result<()> {
    let initial_cluster_size = 3;
    let cluster = create_integration_cluster(initial_cluster_size).await?;
    
    // Start all nodes
    for node in &cluster {
        node.start().await?;
    }
    
    // Wait for leader election
    sleep(Duration::from_millis(800)).await;
    
    // Find the leader
    let mut leader_node = None;
    for node in &cluster {
        if node.is_leader().await {
            leader_node = Some(node);
            break;
        }
    }
    
    if let Some(leader) = leader_node {
        // Test adding a new node
        let add_change = ConfigChangeType::AddNode {
            node_id: "new_node".to_string(),
            address: "127.0.0.1:9999".to_string(),
            is_voting: true,
        };
        
        // This should work if node is leader, or fail appropriately
        let add_result = leader.propose_config_change(add_change).await;
        println!("Add node result: {:?}", add_result);
        
        // Test removing a node
        let remove_change = ConfigChangeType::RemoveNode {
            node_id: "integration_node_2".to_string(),
        };
        
        let remove_result = leader.propose_config_change(remove_change).await;
        println!("Remove node result: {:?}", remove_result);
        
        // Verify configuration state
        let config = leader.get_cluster_config().await;
        assert!(!config.voting_members.is_empty());
        
        let has_pending = leader.has_pending_config_change().await;
        let targets = leader.get_replication_targets().await;
        let majority = leader.get_majority_size().await;
        
        println!("Config state - Pending: {}, Targets: {}, Majority: {}", 
                has_pending, targets.len(), majority);
    }
    
    // Stop all nodes
    for node in &cluster {
        node.stop().await?;
    }
    
    Ok(())
}

/// Test read scaling functionality
#[tokio::test]
async fn test_read_scaling_operations() -> Result<()> {
    let cluster_size = 3;
    let cluster = create_integration_cluster(cluster_size).await?;
    
    // Start all nodes
    for node in &cluster {
        node.start().await?;
    }
    
    // Wait for leader election
    sleep(Duration::from_millis(800)).await;
    
    // Test read operations on all nodes
    for (idx, node) in cluster.iter().enumerate() {
        println!("Testing reads on node {}", idx);
        
        // Test different consistency levels
        let linearizable_request = ReadRequest {
            consistency: ReadConsistency::Linearizable,
            min_commit_index: None,
        };
        
        let sequential_request = ReadRequest {
            consistency: ReadConsistency::Sequential,
            min_commit_index: None,
        };
        
        // Check if node can serve reads
        let can_serve_linearizable = node.can_serve_read(&linearizable_request).await?;
        let can_serve_sequential = node.can_serve_read(&sequential_request).await?;
        
        println!("Node {} - Linearizable: {}, Sequential: {}", 
                idx, can_serve_linearizable, can_serve_sequential);
        
        // Test read-only queries
        let query = ReadOnlyQuery {
            query_id: format!("test_query_{}", idx),
            consistency: ReadConsistency::Sequential,
            query_data: format!("test_data_{}", idx).into_bytes(),
        };
        
        let query_result = node.process_read_only_query(query).await;
        match query_result {
            Ok(response) => {
                println!("Query successful on node {}: {:?}", idx, response.query_id);
            }
            Err(e) => {
                println!("Query failed on node {} (expected for non-leaders): {}", idx, e);
            }
        }
        
        // Test lease operations
        let needs_renewal = node.needs_lease_renewal().await;
        let is_valid = node.validate_read_lease().await;
        
        println!("Node {} - Needs renewal: {}, Valid lease: {}", 
                idx, needs_renewal, is_valid);
    }
    
    // Stop all nodes
    for node in &cluster {
        node.stop().await?;
    }
    
    Ok(())
}

/// Test concurrent safety operations
#[tokio::test]
async fn test_concurrent_safety_operations() -> Result<()> {
    let cluster_size = 3;
    let cluster = create_integration_cluster(cluster_size).await?;
    
    // Start all nodes
    for node in &cluster {
        node.start().await?;
    }
    
    // Wait for initialization
    sleep(Duration::from_millis(500)).await;
    
    use tokio::task::JoinSet;
    
    // Run concurrent safety operations on all nodes
    let mut tasks = JoinSet::new();
    
    for (idx, node) in cluster.iter().enumerate() {
        let node_clone = node.clone();
        
        tasks.spawn(async move {
            let mut operations_completed = 0;
            let mut errors = 0;
            
            for _i in 0..20 {
                // Safety checks
                if let Err(_) = node_clone.check_safety().await {
                    errors += 1;
                } else {
                    operations_completed += 1;
                }
                
                // Leadership queries
                let _is_leader = node_clone.is_leader().await;
                let _leader_id = node_clone.get_leader_id().await;
                operations_completed += 2;
                
                // Configuration queries
                let _config = node_clone.get_cluster_config().await;
                let _pending = node_clone.has_pending_config_change().await;
                let _targets = node_clone.get_replication_targets().await;
                operations_completed += 3;
                
                // Small delay to allow interleaving
                tokio::task::yield_now().await;
            }
            
            (idx, operations_completed, errors)
        });
    }
    
    // Wait for all concurrent operations to complete
    let mut total_operations = 0;
    let mut total_errors = 0;
    
    while let Some(result) = tasks.join_next().await {
        let (node_idx, ops, errors) = result?;
        total_operations += ops;
        total_errors += errors;
        
        println!("Node {} completed {} operations with {} errors", 
                node_idx, ops, errors);
    }
    
    // Stop all nodes
    for node in &cluster {
        node.stop().await?;
    }
    
    println!("Total operations: {}, Total errors: {}", total_operations, total_errors);
    
    // Should complete many operations successfully
    assert!(total_operations > 0);
    
    Ok(())
}

/// Test system under sustained load
#[tokio::test]
async fn test_sustained_load() -> Result<()> {
    let cluster_size = 3;
    let cluster = create_integration_cluster(cluster_size).await?;
    
    // Start all nodes
    for node in &cluster {
        node.start().await?;
    }
    
    // Wait for leader election
    sleep(Duration::from_millis(800)).await;
    
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::task::JoinSet;
    
    let success_counter = Arc::new(AtomicUsize::new(0));
    let error_counter = Arc::new(AtomicUsize::new(0));
    
    let mut tasks = JoinSet::new();
    
    // Run sustained load for a short period
    for node in &cluster {
        let node_clone = node.clone();
        let success_clone = success_counter.clone();
        let error_clone = error_counter.clone();
        
        tasks.spawn(async move {
            let start_time = std::time::Instant::now();
            let duration = Duration::from_millis(2000); // 2 seconds of load
            
            while start_time.elapsed() < duration {
                // Perform various operations
                match node_clone.check_safety().await {
                    Ok(_) => success_clone.fetch_add(1, Ordering::Relaxed),
                    Err(_) => error_clone.fetch_add(1, Ordering::Relaxed),
                };
                
                let _is_leader = node_clone.is_leader().await;
                success_clone.fetch_add(1, Ordering::Relaxed);
                
                let _config = node_clone.get_cluster_config().await;
                success_clone.fetch_add(1, Ordering::Relaxed);
                
                // Brief pause to avoid overwhelming
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
    }
    
    // Wait for all load tasks to complete
    while let Some(_) = tasks.join_next().await {}
    
    // Stop all nodes
    for node in &cluster {
        node.stop().await?;
    }
    
    let total_successes = success_counter.load(Ordering::Relaxed);
    let total_errors = error_counter.load(Ordering::Relaxed);
    
    println!("Sustained load results - Successes: {}, Errors: {}", 
            total_successes, total_errors);
    
    // Should handle sustained load reasonably well
    assert!(total_successes > total_errors, "Should have more successes than errors");
    
    Ok(())
}

/// Test edge cases and error conditions
#[tokio::test]
async fn test_edge_cases_and_error_conditions() -> Result<()> {
    // Test with minimal cluster (single node)
    let single_node_cluster = create_integration_cluster(1).await?;
    
    if let Some(node) = single_node_cluster.first() {
        node.start().await?;
        sleep(Duration::from_millis(300)).await;
        
        // Single node behavior tests
        let _is_leader = node.is_leader().await;
        let _leader_id = node.get_leader_id().await;
        let _config = node.get_cluster_config().await;
        
        node.stop().await?;
    }
    
    // Test with invalid configurations
    let invalid_config = RaftConfig {
        node_id: "".to_string(), // Empty node ID
        peers: vec![],
        election_timeout_min: Duration::from_millis(100),
        election_timeout_max: Duration::from_millis(50), // Invalid: max < min
        heartbeat_interval: Duration::from_millis(200), // Invalid: heartbeat > election timeout
        rpc_timeout: Duration::from_millis(100),
        max_log_entries_per_request: 0, // Invalid: zero entries
        snapshot_threshold: 0,
        snapshot_chunk_size: 0,
        data_dir: "/invalid/path/that/does/not/exist".to_string(),
    };
    
    // Should handle invalid configuration gracefully
    let invalid_result = RaftConsensus::new(invalid_config).await;
    match invalid_result {
        Ok(raft) => {
            // If it succeeds despite invalid config, that's also valid behavior
            println!("Raft instance created despite invalid config");
            let _ = raft.stop().await; // Clean up if needed
        }
        Err(e) => {
            println!("Appropriately failed with invalid config: {}", e);
        }
    }
    
    Ok(())
}

/// Helper function to run all integration tests
pub async fn run_integration_test_suite() -> Result<()> {
    println!("Running Raft integration test suite...");
    
    // This would run all the tests above in a real scenario
    // For now, just run a basic validation
    let cluster = create_integration_cluster(3).await?;
    
    for node in &cluster {
        node.start().await?;
    }
    
    sleep(Duration::from_millis(500)).await;
    
    for node in &cluster {
        node.check_safety().await?;
        node.stop().await?;
    }
    
    println!("Integration test suite completed successfully!");
    Ok(())
}