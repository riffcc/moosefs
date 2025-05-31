use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::raft::{
    state::{NodeId, Term, LogIndex},
    node::RaftNode,
};

/// Read consistency levels for scaling reads across the cluster
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReadConsistency {
    /// Only leader can serve reads (strongest consistency)
    Strong,
    /// Followers can serve reads with lease validation
    Lease,
    /// Followers can serve reads without lease validation (eventual consistency)
    Eventual,
}

/// Lease information for read scaling
#[derive(Debug, Clone)]
pub struct ReadLease {
    pub granted_at: Instant,
    pub duration: Duration,
    pub leader_term: Term,
    pub leader_id: NodeId,
    pub commit_index_at_grant: LogIndex,
}

impl ReadLease {
    pub fn new(
        leader_id: NodeId,
        leader_term: Term,
        commit_index: LogIndex,
        duration: Duration,
    ) -> Self {
        Self {
            granted_at: Instant::now(),
            duration,
            leader_term,
            leader_id,
            commit_index_at_grant: commit_index,
        }
    }
    
    pub fn is_valid(&self) -> bool {
        self.granted_at.elapsed() < self.duration
    }
    
    pub fn remaining_duration(&self) -> Duration {
        self.duration.saturating_sub(self.granted_at.elapsed())
    }
}

/// Read request with consistency requirements
#[derive(Debug, Clone)]
pub struct ReadRequest {
    pub consistency: ReadConsistency,
    pub min_commit_index: Option<LogIndex>,
}

impl ReadRequest {
    pub fn strong() -> Self {
        Self {
            consistency: ReadConsistency::Strong,
            min_commit_index: None,
        }
    }
    
    pub fn lease() -> Self {
        Self {
            consistency: ReadConsistency::Lease,
            min_commit_index: None,
        }
    }
    
    pub fn eventual() -> Self {
        Self {
            consistency: ReadConsistency::Eventual,
            min_commit_index: None,
        }
    }
    
    pub fn with_min_commit_index(mut self, index: LogIndex) -> Self {
        self.min_commit_index = Some(index);
        self
    }
}

/// Manager for read scaling operations
#[derive(Debug)]
pub struct ReadScalingManager {
    node_id: NodeId,
    current_lease: Option<ReadLease>,
    lease_duration: Duration,
    lease_renewal_threshold: f64, // Fraction of lease duration at which to renew
}

impl ReadScalingManager {
    pub fn new(node_id: NodeId, lease_duration: Duration) -> Self {
        Self {
            node_id,
            current_lease: None,
            lease_duration,
            lease_renewal_threshold: 0.75, // Renew when 75% of lease has elapsed
        }
    }
    
    /// Check if this node can serve a read request
    pub fn can_serve_read(&self, request: &ReadRequest, node: &RaftNode) -> Result<bool> {
        match request.consistency {
            ReadConsistency::Strong => {
                // Only leader can serve strong reads
                Ok(node.is_leader())
            }
            ReadConsistency::Lease => {
                // Check if we have a valid lease from current leader
                if let Some(lease) = &self.current_lease {
                    if lease.is_valid() {
                        // Ensure we have the required commit index if specified
                        if let Some(min_index) = request.min_commit_index {
                            Ok(node.state.commit_index >= min_index)
                        } else {
                            Ok(true)
                        }
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            ReadConsistency::Eventual => {
                // Any node can serve eventual reads
                if let Some(min_index) = request.min_commit_index {
                    Ok(node.state.commit_index >= min_index)
                } else {
                    Ok(true)
                }
            }
        }
    }
    
    /// Grant a lease to a follower (leader only)
    pub fn grant_lease(
        &self,
        follower_id: &NodeId,
        leader_term: Term,
        commit_index: LogIndex,
    ) -> ReadLease {
        info!(
            "Leader {} granting read lease to follower {} for term {} at commit index {}",
            self.node_id, follower_id, leader_term, commit_index
        );
        
        ReadLease::new(
            self.node_id.clone(),
            leader_term,
            commit_index,
            self.lease_duration,
        )
    }
    
    /// Update the current lease (follower only)
    pub fn update_lease(&mut self, lease: ReadLease) -> Result<()> {
        debug!(
            "Node {} updating read lease from leader {} for term {}",
            self.node_id, lease.leader_id, lease.leader_term
        );
        
        self.current_lease = Some(lease);
        Ok(())
    }
    
    /// Invalidate current lease (when stepping down or term changes)
    pub fn invalidate_lease(&mut self) {
        if self.current_lease.is_some() {
            debug!("Node {} invalidating read lease", self.node_id);
            self.current_lease = None;
        }
    }
    
    /// Check if lease needs renewal
    pub fn needs_lease_renewal(&self) -> bool {
        if let Some(lease) = &self.current_lease {
            let elapsed_fraction = lease.granted_at.elapsed().as_secs_f64() 
                / lease.duration.as_secs_f64();
            elapsed_fraction >= self.lease_renewal_threshold
        } else {
            true
        }
    }
    
    /// Get current lease information
    pub fn get_current_lease(&self) -> Option<&ReadLease> {
        self.current_lease.as_ref()
    }
    
    /// Validate that a lease is still valid for the current term
    pub fn validate_lease_for_term(&mut self, current_term: Term) -> bool {
        if let Some(lease) = &self.current_lease {
            if lease.leader_term == current_term && lease.is_valid() {
                return true;
            } else {
                debug!(
                    "Invalidating lease: term mismatch ({} vs {}) or expired",
                    lease.leader_term, current_term
                );
                self.current_lease = None;
            }
        }
        false
    }
}

/// Read-only query that doesn't modify state
#[derive(Debug, Clone)]
pub struct ReadOnlyQuery {
    pub query_id: String,
    pub consistency: ReadConsistency,
    pub data: Vec<u8>, // Query data
}

/// Response to a read-only query
#[derive(Debug, Clone)]
pub struct ReadOnlyResponse {
    pub query_id: String,
    pub success: bool,
    pub data: Vec<u8>, // Response data
    pub commit_index: LogIndex,
    pub term: Term,
}

/// Manager for non-voting members in read scaling
pub struct NonVotingManager {
    non_voting_members: HashMap<NodeId, NonVotingMemberInfo>,
}

#[derive(Debug, Clone)]
pub struct NonVotingMemberInfo {
    pub node_id: NodeId,
    pub last_heartbeat: Instant,
    pub match_index: LogIndex,
    pub next_index: LogIndex,
    pub can_be_promoted: bool,
}

impl NonVotingManager {
    pub fn new() -> Self {
        Self {
            non_voting_members: HashMap::new(),
        }
    }
    
    /// Add a non-voting member
    pub fn add_non_voting_member(&mut self, node_id: NodeId, current_log_index: LogIndex) {
        let member_info = NonVotingMemberInfo {
            node_id: node_id.clone(),
            last_heartbeat: Instant::now(),
            match_index: 0,
            next_index: current_log_index + 1,
            can_be_promoted: false,
        };
        
        info!("Adding non-voting member: {}", node_id);
        self.non_voting_members.insert(node_id, member_info);
    }
    
    /// Remove a non-voting member
    pub fn remove_non_voting_member(&mut self, node_id: &NodeId) -> bool {
        info!("Removing non-voting member: {}", node_id);
        self.non_voting_members.remove(node_id).is_some()
    }
    
    /// Update replication status for a non-voting member
    pub fn update_replication_status(
        &mut self,
        node_id: &NodeId,
        match_index: LogIndex,
        next_index: LogIndex,
    ) {
        if let Some(member) = self.non_voting_members.get_mut(node_id) {
            member.match_index = match_index;
            member.next_index = next_index;
            member.last_heartbeat = Instant::now();
            
            // Consider for promotion if caught up
            member.can_be_promoted = match_index >= next_index.saturating_sub(10);
        }
    }
    
    /// Get non-voting members ready for promotion
    pub fn get_promotion_candidates(&self, min_catchup_threshold: LogIndex) -> Vec<NodeId> {
        self.non_voting_members
            .values()
            .filter(|member| {
                member.can_be_promoted && 
                member.match_index >= min_catchup_threshold &&
                member.last_heartbeat.elapsed() < Duration::from_secs(30)
            })
            .map(|member| member.node_id.clone())
            .collect()
    }
    
    /// Get all non-voting members for replication
    pub fn get_all_non_voting_members(&self) -> Vec<NodeId> {
        self.non_voting_members.keys().cloned().collect()
    }
    
    /// Check if a node is a non-voting member
    pub fn is_non_voting_member(&self, node_id: &NodeId) -> bool {
        self.non_voting_members.contains_key(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_read_lease_validity() {
        let lease = ReadLease::new(
            "leader1".to_string(),
            1,
            100,
            Duration::from_secs(5),
        );
        
        assert!(lease.is_valid());
        assert!(lease.remaining_duration() <= Duration::from_secs(5));
    }
    
    #[test]
    fn test_read_request_creation() {
        let strong_read = ReadRequest::strong();
        assert_eq!(strong_read.consistency, ReadConsistency::Strong);
        
        let lease_read = ReadRequest::lease().with_min_commit_index(10);
        assert_eq!(lease_read.consistency, ReadConsistency::Lease);
        assert_eq!(lease_read.min_commit_index, Some(10));
    }
    
    #[test]
    fn test_non_voting_manager() {
        let mut manager = NonVotingManager::new();
        let node_id = "non_voter1".to_string();
        
        manager.add_non_voting_member(node_id.clone(), 100);
        assert!(manager.is_non_voting_member(&node_id));
        
        manager.update_replication_status(&node_id, 95, 96);
        let candidates = manager.get_promotion_candidates(90);
        assert!(candidates.contains(&node_id));
        
        assert!(manager.remove_non_voting_member(&node_id));
        assert!(!manager.is_non_voting_member(&node_id));
    }
}