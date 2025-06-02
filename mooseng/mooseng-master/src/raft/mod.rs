pub mod state;
pub mod node;
pub mod log;
pub mod rpc;
pub mod election;
pub mod replication;
pub mod snapshot;
pub mod config;
pub mod safety;
pub mod membership;
pub mod read_scaling;
pub mod optimization;
pub mod multiregion_optimization;
pub mod test_runner;
pub mod block_replication;
pub mod resilience;

#[cfg(test)]
pub mod tests;

pub use state::{RaftState, NodeState, Term, LogIndex, NodeId};
pub use node::RaftNode;
pub use log::{LogEntry, RaftLog, LogCommand};
pub use rpc::{RaftRpc, RaftMessage};
pub use election::ElectionManager;
pub use replication::ReplicationManager;
pub use snapshot::SnapshotManager;
pub use config::RaftConfig;
pub use safety::{RaftSafetyChecker, RaftSafetyUtils};
pub use membership::{MembershipManager, ConfigChangeType, ClusterConfiguration, JointConfiguration};
pub use read_scaling::{ReadScalingManager, ReadConsistency, ReadLease, ReadRequest, ReadOnlyQuery, ReadOnlyResponse, NonVotingManager};
pub use optimization::{OptimizedRaftConsensus, RaftPerformanceMetrics, PerformanceTuningRecommendations};
pub use multiregion_optimization::{
    MultiregionOptimizedRaft, MultiregionRaftMetrics, RegionTimeouts, 
    TimeoutAdaptationAlgorithm, NetworkConditions
};
pub use test_runner::{RaftTestRunner, TestResult, run_raft_testing_and_optimization};
pub use resilience::{ResilienceManager, PeerHealthStatus, ResilienceStats, CircuitBreakerState};

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct RaftConsensus {
    node: Arc<RwLock<RaftNode>>,
    election_manager: Arc<ElectionManager>,
    replication_manager: Arc<ReplicationManager>,
    snapshot_manager: Arc<SnapshotManager>,
    safety_checker: Arc<RwLock<RaftSafetyChecker>>,
    membership_manager: Arc<MembershipManager>,
    read_scaling_manager: Arc<RwLock<ReadScalingManager>>,
    resilience_manager: Arc<ResilienceManager>,
}

impl RaftConsensus {
    pub async fn new(config: RaftConfig) -> Result<Self> {
        let node_id = config.node_id.clone();
        let node = Arc::new(RwLock::new(RaftNode::new(config.clone())?));
        
        let election_manager = Arc::new(ElectionManager::new(
            node_id.clone(),
            node.clone(),
            config.clone(),
        ));
        
        let replication_manager = Arc::new(ReplicationManager::new(
            node_id.clone(),
            node.clone(),
            config.clone(),
        ));
        
        let snapshot_manager = Arc::new(SnapshotManager::new(
            node_id.clone(),
            node.clone(),
            config.clone(),
        ));
        
        let safety_checker = Arc::new(RwLock::new(RaftSafetyChecker::new(
            node_id.clone(),
            config.clone(),
        )));
        
        let membership_manager = Arc::new(MembershipManager::new(
            node_id.clone(),
            config.clone(),
        ));
        
        let read_scaling_manager = Arc::new(RwLock::new(ReadScalingManager::new(
            node_id.clone(),
            std::time::Duration::from_secs(30), // 30 second lease duration
        )));
        
        let resilience_manager = Arc::new(ResilienceManager::new(config.clone()));
        
        // Initialize resilience monitoring for cluster members
        let peer_ids: Vec<NodeId> = config.initial_members.iter()
            .filter(|id| **id != node_id)
            .cloned()
            .collect();
        resilience_manager.initialize_peers(&peer_ids).await;
        
        Ok(Self {
            node,
            election_manager,
            replication_manager,
            snapshot_manager,
            safety_checker,
            membership_manager,
            read_scaling_manager,
            resilience_manager,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        self.election_manager.start().await?;
        self.replication_manager.start().await?;
        self.snapshot_manager.start().await?;
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        self.election_manager.stop().await?;
        self.replication_manager.stop().await?;
        self.snapshot_manager.stop().await?;
        Ok(())
    }
    
    pub async fn is_leader(&self) -> bool {
        let node = self.node.read().await;
        node.is_leader()
    }
    
    pub async fn get_leader_id(&self) -> Option<String> {
        let node = self.node.read().await;
        node.get_leader_id()
    }
    
    /// Call this when a node becomes leader to initialize replication
    pub async fn on_leader_elected(&self) -> Result<()> {
        self.replication_manager.initialize_as_leader().await
    }
    
    /// Append a log entry (only allowed on leader)
    pub async fn append_entry(&self, command: crate::raft::log::LogCommand) -> Result<crate::raft::state::LogIndex> {
        let mut node = self.node.write().await;
        
        if !node.is_leader() {
            return Err(anyhow::anyhow!("Only leader can append entries"));
        }
        
        let entry = crate::raft::log::LogEntry::new(
            0, // Will be set by append()
            node.current_term(),
            command,
        );
        
        node.log.append(entry)
    }
    
    /// Perform comprehensive safety checks
    pub async fn check_safety(&self) -> Result<()> {
        let node = self.node.read().await;
        let mut safety_checker = self.safety_checker.write().await;
        safety_checker.check_safety_invariants(&node.state, &node.log)
    }
    
    /// Check if this node can safely start an election
    pub async fn can_start_election(&self) -> Result<bool> {
        let node = self.node.read().await;
        let safety_checker = self.safety_checker.read().await;
        safety_checker.can_start_election(&node.state, &node.log)
    }
    
    /// Validate a vote request before processing
    pub async fn validate_vote_request(
        &self,
        candidate_term: crate::raft::state::Term,
        candidate_id: &crate::raft::state::NodeId,
        last_log_index: crate::raft::state::LogIndex,
        last_log_term: crate::raft::state::Term,
    ) -> Result<bool> {
        let node = self.node.read().await;
        let safety_checker = self.safety_checker.read().await;
        safety_checker.validate_vote_request(
            &node.state,
            &node.log,
            candidate_term,
            candidate_id,
            last_log_index,
            last_log_term,
        )
    }

    /// Propose a configuration change (leader only)
    pub async fn propose_config_change(
        &self,
        change: ConfigChangeType,
    ) -> Result<crate::raft::state::LogIndex> {
        let mut node = self.node.write().await;
        let state_clone = node.state.clone();
        self.membership_manager.propose_config_change(
            change,
            &mut node.log,
            &state_clone,
        ).await
    }

    /// Apply a committed configuration change
    pub async fn apply_config_change(
        &self,
        change: &ConfigChangeType,
        joint_config: &JointConfiguration,
    ) -> Result<()> {
        self.membership_manager.apply_config_change(change, joint_config).await
    }

    /// Check if node can participate in voting
    pub async fn can_vote(&self, node_id: &crate::raft::state::NodeId) -> bool {
        self.membership_manager.can_vote(node_id).await
    }

    /// Get current cluster configuration
    pub async fn get_cluster_config(&self) -> ClusterConfiguration {
        self.membership_manager.get_current_config().await
    }

    /// Check if there's a pending configuration change
    pub async fn has_pending_config_change(&self) -> bool {
        self.membership_manager.has_pending_config_change().await
    }

    /// Get replication targets for current configuration
    pub async fn get_replication_targets(&self) -> std::collections::HashSet<String> {
        self.membership_manager.get_replication_targets().await
    }

    /// Get required majority size for current configuration
    pub async fn get_majority_size(&self) -> usize {
        self.membership_manager.get_majority_size().await
    }

    /// Check if we have majority for leader election
    pub async fn has_election_majority(&self, votes: &std::collections::HashSet<String>) -> bool {
        self.membership_manager.has_election_majority(votes).await
    }

    /// Check if this node can serve a read request with the given consistency
    pub async fn can_serve_read(&self, request: &ReadRequest) -> Result<bool> {
        let node = self.node.read().await;
        let read_manager = self.read_scaling_manager.read().await;
        read_manager.can_serve_read(request, &node)
    }

    /// Grant a read lease to a follower (leader only)
    pub async fn grant_read_lease(&self, follower_id: &crate::raft::state::NodeId) -> Result<ReadLease> {
        let node = self.node.read().await;
        if !node.is_leader() {
            return Err(anyhow::anyhow!("Only leader can grant read leases"));
        }

        let read_manager = self.read_scaling_manager.read().await;
        let lease = read_manager.grant_lease(
            follower_id,
            node.current_term(),
            node.state.commit_index,
        );
        Ok(lease)
    }

    /// Update the current read lease (follower only)
    pub async fn update_read_lease(&self, lease: ReadLease) -> Result<()> {
        let mut read_manager = self.read_scaling_manager.write().await;
        read_manager.update_lease(lease)
    }

    /// Invalidate current read lease (when stepping down or term changes)
    pub async fn invalidate_read_lease(&self) {
        let mut read_manager = self.read_scaling_manager.write().await;
        read_manager.invalidate_lease();
    }

    /// Check if read lease needs renewal
    pub async fn needs_lease_renewal(&self) -> bool {
        let read_manager = self.read_scaling_manager.read().await;
        read_manager.needs_lease_renewal()
    }

    /// Validate read lease for current term
    pub async fn validate_read_lease(&self) -> bool {
        let node = self.node.read().await;
        let mut read_manager = self.read_scaling_manager.write().await;
        read_manager.validate_lease_for_term(node.current_term())
    }

    /// Process a read-only query
    pub async fn process_read_only_query(
        &self,
        query: ReadOnlyQuery,
    ) -> Result<ReadOnlyResponse> {
        let request = ReadRequest {
            consistency: query.consistency,
            min_commit_index: None,
        };

        if !self.can_serve_read(&request).await? {
            return Err(anyhow::anyhow!("Cannot serve read with requested consistency"));
        }

        let node = self.node.read().await;
        
        // For now, return a placeholder response
        // In a real implementation, this would apply the query to the state machine
        Ok(ReadOnlyResponse {
            query_id: query.query_id,
            success: true,
            data: vec![], // Would contain actual query results
            commit_index: node.state.commit_index,
            term: node.current_term(),
        })
    }
    
    /// Get cluster resilience status
    pub async fn get_resilience_status(&self) -> ResilienceStats {
        self.resilience_manager.get_resilience_stats().await
    }
    
    /// Check if the cluster is healthy from a resilience perspective
    pub async fn is_cluster_resilient(&self) -> bool {
        self.resilience_manager.is_cluster_healthy().await
    }
    
    /// Get health status for all peers
    pub async fn get_peer_health_status(&self) -> std::collections::HashMap<NodeId, PeerHealthStatus> {
        self.resilience_manager.get_cluster_health_status().await
    }
    
    /// Detect potential network partitions
    pub async fn detect_network_partitions(&self) -> Option<Vec<NodeId>> {
        self.resilience_manager.detect_network_partition().await
    }
    
    /// Record successful communication with a peer (for resilience tracking)
    pub async fn record_peer_success(&self, peer_id: &NodeId, rtt: std::time::Duration) {
        self.resilience_manager.record_success(peer_id, rtt).await;
    }
    
    /// Record failed communication with a peer (for resilience tracking)
    pub async fn record_peer_failure(&self, peer_id: &NodeId, is_timeout: bool) {
        self.resilience_manager.record_failure(peer_id, is_timeout).await;
    }
}