use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::raft::state::NodeId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    pub node_id: NodeId,
    
    pub data_dir: PathBuf,
    
    pub initial_members: Vec<NodeId>,
    
    pub election_timeout_ms: u64,
    
    pub heartbeat_interval_ms: u64,
    
    pub rpc_timeout_ms: u64,
    
    pub snapshot_interval: u64,
    
    pub raft_port: u16,
    
    pub max_log_entries: usize,
    
    pub max_snapshot_size: usize,
    
    pub batch_size: usize,
    
    pub pipeline_enabled: bool,
    
    pub pre_vote_enabled: bool,
    
    pub check_quorum: bool,
    
    pub leader_lease_timeout_ms: u64,
    
    pub max_append_entries: Option<u64>,
    
    /// Minimum timeout for election (randomized between min and max)
    pub election_timeout_min_ms: u64,
    
    /// Maximum timeout for election (randomized between min and max)  
    pub election_timeout_max_ms: u64,
    
    /// Enable read scaling with lease-based reads
    pub read_scaling_enabled: bool,
    
    /// Duration for read leases in milliseconds
    pub read_lease_duration_ms: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: "node1".to_string(),
            data_dir: PathBuf::from("/var/lib/mooseng/raft"),
            initial_members: vec!["node1".to_string()],
            election_timeout_ms: 1000,
            heartbeat_interval_ms: 150,
            rpc_timeout_ms: 500,
            snapshot_interval: 10000,
            raft_port: 9425,
            max_log_entries: 100000,
            max_snapshot_size: 1024 * 1024 * 1024, // 1GB
            batch_size: 100,
            pipeline_enabled: true,
            pre_vote_enabled: true,
            check_quorum: true,
            leader_lease_timeout_ms: 2000,
            max_append_entries: Some(100),
            election_timeout_min_ms: 800,
            election_timeout_max_ms: 1200,
            read_scaling_enabled: true,
            read_lease_duration_ms: 30000, // 30 second read leases
        }
    }
}

impl RaftConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.node_id.is_empty() {
            return Err("Node ID cannot be empty".to_string());
        }
        
        if self.initial_members.is_empty() {
            return Err("Initial members list cannot be empty".to_string());
        }
        
        if !self.initial_members.contains(&self.node_id) {
            return Err("Node ID must be in initial members list".to_string());
        }
        
        if self.election_timeout_ms < self.heartbeat_interval_ms * 2 {
            return Err("Election timeout must be at least 2x heartbeat interval".to_string());
        }
        
        if self.heartbeat_interval_ms < 50 {
            return Err("Heartbeat interval too small (minimum 50ms)".to_string());
        }
        
        if self.rpc_timeout_ms > self.election_timeout_ms {
            return Err("RPC timeout should be less than election timeout".to_string());
        }
        
        Ok(())
    }
    
    pub fn is_single_node(&self) -> bool {
        self.initial_members.len() == 1
    }
    
    pub fn get_quorum_size(&self) -> usize {
        (self.initial_members.len() / 2) + 1
    }
    
    pub fn from_master_config(config: &mooseng_common::config::MasterConfig) -> Self {
        Self {
            node_id: config.node_id.clone(),
            data_dir: config.data_dir.join("raft"),
            initial_members: config.ha_peers.iter()
                .map(|peer| peer.split(':').next().unwrap_or("unknown").to_string())
                .collect(),
            election_timeout_ms: config.raft_election_timeout_ms,
            heartbeat_interval_ms: config.raft_heartbeat_ms,
            rpc_timeout_ms: 500,
            snapshot_interval: config.raft_snapshot_interval,
            raft_port: config.raft_port,
            max_log_entries: 100000,
            max_snapshot_size: 1024 * 1024 * 1024,
            batch_size: 100,
            pipeline_enabled: true,
            pre_vote_enabled: true,
            check_quorum: true,
            leader_lease_timeout_ms: config.leader_lease_duration_ms,
            max_append_entries: Some(100),
            election_timeout_min_ms: config.raft_election_timeout_ms * 8 / 10, // 80% of base timeout
            election_timeout_max_ms: config.raft_election_timeout_ms * 12 / 10, // 120% of base timeout
            read_scaling_enabled: config.ha_enabled, // Enable read scaling when HA is enabled
            read_lease_duration_ms: config.leader_lease_duration_ms,
        }
    }
}