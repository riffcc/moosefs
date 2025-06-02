use crate::raft::{
    state::{NodeId, LogIndex, Term, RaftState},
    log::{LogEntry, LogCommand, RaftLog},
    config::RaftConfig,
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use std::sync::Arc;

/// Configuration change types for Raft membership
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigChangeType {
    AddVotingNode(NodeId),
    AddNonVotingNode(NodeId),
    RemoveNode(NodeId),
    PromoteToVoting(NodeId),
    DemoteToNonVoting(NodeId),
}

/// Joint consensus configuration for safe membership changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointConfiguration {
    /// Old configuration (before change)
    pub old_config: ClusterConfiguration,
    /// New configuration (after change)  
    pub new_config: ClusterConfiguration,
    /// Whether this is in joint consensus mode
    pub joint_mode: bool,
}

/// Cluster configuration containing voting and non-voting members
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterConfiguration {
    pub voting_members: HashSet<NodeId>,
    pub non_voting_members: HashSet<NodeId>,
}

impl ClusterConfiguration {
    pub fn new(voting_members: HashSet<NodeId>) -> Self {
        Self {
            voting_members,
            non_voting_members: HashSet::new(),
        }
    }

    pub fn all_members(&self) -> HashSet<NodeId> {
        let mut all = self.voting_members.clone();
        all.extend(self.non_voting_members.clone());
        all
    }

    pub fn majority_size(&self) -> usize {
        (self.voting_members.len() / 2) + 1
    }

    pub fn is_voting_member(&self, node_id: &NodeId) -> bool {
        self.voting_members.contains(node_id)
    }

    pub fn contains_member(&self, node_id: &NodeId) -> bool {
        self.voting_members.contains(node_id) || self.non_voting_members.contains(node_id)
    }
}

impl JointConfiguration {
    pub fn new(old_config: ClusterConfiguration, new_config: ClusterConfiguration) -> Self {
        Self {
            old_config,
            new_config,
            joint_mode: true,
        }
    }

    /// Check if we have majority in both old and new configurations
    pub fn has_joint_majority(&self, votes: &HashSet<NodeId>) -> bool {
        let old_majority = self.old_config.majority_size();
        let new_majority = self.new_config.majority_size();

        let old_votes = votes.iter()
            .filter(|id| self.old_config.is_voting_member(id))
            .count();
        
        let new_votes = votes.iter()
            .filter(|id| self.new_config.is_voting_member(id))
            .count();

        old_votes >= old_majority && new_votes >= new_majority
    }

    /// Transition from joint consensus to new configuration
    pub fn commit_new_config(self) -> ClusterConfiguration {
        self.new_config
    }
}

/// Membership manager for handling configuration changes
#[derive(Debug)]
pub struct MembershipManager {
    node_id: NodeId,
    config: RaftConfig,
    current_config: Arc<RwLock<ClusterConfiguration>>,
    pending_config_change: Arc<RwLock<Option<JointConfiguration>>>,
}

impl MembershipManager {
    pub fn new(node_id: NodeId, config: RaftConfig) -> Self {
        let initial_voting = config.initial_members.iter().cloned().collect();
        let current_config = Arc::new(RwLock::new(
            ClusterConfiguration::new(initial_voting)
        ));

        Self {
            node_id,
            config,
            current_config,
            pending_config_change: Arc::new(RwLock::new(None)),
        }
    }

    /// Initiate a configuration change (only allowed on leader)
    pub async fn propose_config_change(
        &self,
        change: ConfigChangeType,
        raft_log: &mut RaftLog,
        raft_state: &RaftState,
    ) -> Result<LogIndex> {
        // Only leader can propose configuration changes
        if raft_state.node_state != crate::raft::state::NodeState::Leader {
            return Err(anyhow!("Only leader can propose configuration changes"));
        }

        // Check if there's already a pending configuration change
        let pending = self.pending_config_change.read().await;
        if pending.is_some() {
            return Err(anyhow!("Cannot propose new config change while one is pending"));
        }
        drop(pending);

        // Get current configuration
        let current_config = self.current_config.read().await.clone();
        
        // Validate the proposed change
        self.validate_config_change(&change, &current_config)?;

        // Create new configuration by applying the change
        let new_config = self.create_new_config(&current_config, &change)?;
        
        // Create joint configuration
        let joint_config = JointConfiguration::new(current_config, new_config);

        // Create log entry for configuration change
        let config_entry = LogEntry::new(
            0, // Will be set by append
            raft_state.current_term,
            LogCommand::ConfigChange {
                change_type: change,
                joint_config: joint_config.clone(),
            },
        );

        // Store pending configuration change
        let mut pending = self.pending_config_change.write().await;
        *pending = Some(joint_config);
        drop(pending);

        // Append to log
        raft_log.append(config_entry)
    }

    /// Apply a committed configuration change
    pub async fn apply_config_change(
        &self,
        change: &ConfigChangeType,
        joint_config: &JointConfiguration,
    ) -> Result<()> {
        match change {
            ConfigChangeType::AddVotingNode(_) |
            ConfigChangeType::AddNonVotingNode(_) |
            ConfigChangeType::RemoveNode(_) |
            ConfigChangeType::PromoteToVoting(_) |
            ConfigChangeType::DemoteToNonVoting(_) => {
                // If we're in joint consensus mode, transition to new config
                if joint_config.joint_mode {
                    let mut current_config = self.current_config.write().await;
                    *current_config = joint_config.new_config.clone();
                    
                    let mut pending = self.pending_config_change.write().await;
                    *pending = None;
                }
            }
        }
        
        Ok(())
    }

    /// Check if node should participate in voting
    pub async fn can_vote(&self, node_id: &NodeId) -> bool {
        let current_config = self.current_config.read().await;
        let pending = self.pending_config_change.read().await;

        if let Some(joint_config) = pending.as_ref() {
            // In joint consensus, must be voting member in both configs
            joint_config.old_config.is_voting_member(node_id) &&
            joint_config.new_config.is_voting_member(node_id)
        } else {
            current_config.is_voting_member(node_id)
        }
    }

    /// Calculate required majority for leader election
    pub async fn get_majority_size(&self) -> usize {
        let current_config = self.current_config.read().await;
        let pending = self.pending_config_change.read().await;

        if let Some(joint_config) = pending.as_ref() {
            // In joint consensus, need majority in both configs
            std::cmp::max(
                joint_config.old_config.majority_size(),
                joint_config.new_config.majority_size()
            )
        } else {
            current_config.majority_size()
        }
    }

    /// Check if we have majority for leader election
    pub async fn has_election_majority(&self, votes: &HashSet<NodeId>) -> bool {
        let current_config = self.current_config.read().await;
        let pending = self.pending_config_change.read().await;

        if let Some(joint_config) = pending.as_ref() {
            joint_config.has_joint_majority(votes)
        } else {
            let voting_votes = votes.iter()
                .filter(|id| current_config.is_voting_member(id))
                .count();
            voting_votes >= current_config.majority_size()
        }
    }

    /// Get all members that should receive replication
    pub async fn get_replication_targets(&self) -> HashSet<NodeId> {
        let current_config = self.current_config.read().await;
        let pending = self.pending_config_change.read().await;

        if let Some(joint_config) = pending.as_ref() {
            // During joint consensus, replicate to all members in both configs
            let mut targets = joint_config.old_config.all_members();
            targets.extend(joint_config.new_config.all_members());
            targets.remove(&self.node_id); // Don't replicate to self
            targets
        } else {
            let mut targets = current_config.all_members();
            targets.remove(&self.node_id); // Don't replicate to self
            targets
        }
    }

    /// Validate a proposed configuration change
    fn validate_config_change(
        &self,
        change: &ConfigChangeType,
        current_config: &ClusterConfiguration,
    ) -> Result<()> {
        match change {
            ConfigChangeType::AddVotingNode(node_id) => {
                if current_config.contains_member(node_id) {
                    return Err(anyhow!("Node {} is already a cluster member", node_id));
                }
            }
            ConfigChangeType::AddNonVotingNode(node_id) => {
                if current_config.contains_member(node_id) {
                    return Err(anyhow!("Node {} is already a cluster member", node_id));
                }
            }
            ConfigChangeType::RemoveNode(node_id) => {
                if !current_config.contains_member(node_id) {
                    return Err(anyhow!("Node {} is not a cluster member", node_id));
                }
                // Don't allow removing the last voting member
                if current_config.voting_members.len() == 1 && 
                   current_config.is_voting_member(node_id) {
                    return Err(anyhow!("Cannot remove the last voting member"));
                }
            }
            ConfigChangeType::PromoteToVoting(node_id) => {
                if !current_config.non_voting_members.contains(node_id) {
                    return Err(anyhow!("Node {} is not a non-voting member", node_id));
                }
            }
            ConfigChangeType::DemoteToNonVoting(node_id) => {
                if !current_config.is_voting_member(node_id) {
                    return Err(anyhow!("Node {} is not a voting member", node_id));
                }
                // Don't allow demoting the last voting member
                if current_config.voting_members.len() == 1 {
                    return Err(anyhow!("Cannot demote the last voting member"));
                }
            }
        }
        Ok(())
    }

    /// Apply configuration change to create new configuration
    fn create_new_config(
        &self,
        current_config: &ClusterConfiguration,
        change: &ConfigChangeType,
    ) -> Result<ClusterConfiguration> {
        let mut new_config = current_config.clone();

        match change {
            ConfigChangeType::AddVotingNode(node_id) => {
                new_config.voting_members.insert(node_id.clone());
            }
            ConfigChangeType::AddNonVotingNode(node_id) => {
                new_config.non_voting_members.insert(node_id.clone());
            }
            ConfigChangeType::RemoveNode(node_id) => {
                new_config.voting_members.remove(node_id);
                new_config.non_voting_members.remove(node_id);
            }
            ConfigChangeType::PromoteToVoting(node_id) => {
                new_config.non_voting_members.remove(node_id);
                new_config.voting_members.insert(node_id.clone());
            }
            ConfigChangeType::DemoteToNonVoting(node_id) => {
                new_config.voting_members.remove(node_id);
                new_config.non_voting_members.insert(node_id.clone());
            }
        }

        Ok(new_config)
    }

    /// Get current cluster configuration
    pub async fn get_current_config(&self) -> ClusterConfiguration {
        self.current_config.read().await.clone()
    }

    /// Check if there's a pending configuration change
    pub async fn has_pending_config_change(&self) -> bool {
        self.pending_config_change.read().await.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_configuration() {
        let mut voting = HashSet::new();
        voting.insert("node1".to_string());
        voting.insert("node2".to_string());
        voting.insert("node3".to_string());

        let config = ClusterConfiguration::new(voting);
        
        assert_eq!(config.majority_size(), 2);
        assert!(config.is_voting_member(&"node1".to_string()));
        assert!(!config.is_voting_member(&"node4".to_string()));
    }

    #[test]
    fn test_joint_configuration() {
        let mut old_voting = HashSet::new();
        old_voting.insert("node1".to_string());
        old_voting.insert("node2".to_string());
        old_voting.insert("node3".to_string());

        let mut new_voting = HashSet::new();
        new_voting.insert("node1".to_string());
        new_voting.insert("node2".to_string());
        new_voting.insert("node4".to_string());

        let old_config = ClusterConfiguration::new(old_voting);
        let new_config = ClusterConfiguration::new(new_voting);
        let joint_config = JointConfiguration::new(old_config, new_config);

        let mut votes = HashSet::new();
        votes.insert("node1".to_string());
        votes.insert("node2".to_string());

        assert!(joint_config.has_joint_majority(&votes));
    }
}