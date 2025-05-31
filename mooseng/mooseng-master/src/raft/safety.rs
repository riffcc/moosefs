/// Raft safety mechanisms and invariant checks
/// Implements critical safety properties for Raft consensus algorithm

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::{debug, warn, error};

use crate::raft::{
    state::{RaftState, NodeState, Term, LogIndex, NodeId},
    log::{RaftLog, LogEntry},
    config::RaftConfig,
};

/// Safety checker that validates Raft invariants
#[derive(Debug)]
pub struct RaftSafetyChecker {
    node_id: NodeId,
    config: RaftConfig,
}

impl RaftSafetyChecker {
    pub fn new(node_id: NodeId, config: RaftConfig) -> Self {
        Self { node_id, config }
    }
    
    /// Check all safety invariants for the current state
    pub fn check_safety_invariants(
        &self,
        state: &RaftState,
        log: &RaftLog,
    ) -> Result<()> {
        self.check_election_safety(state)?;
        self.check_leader_append_only(state, log)?;
        self.check_log_matching(state, log)?;
        self.check_leader_completeness(state, log)?;
        self.check_state_machine_safety(state, log)?;
        
        Ok(())
    }
    
    /// Election Safety: At most one leader can be elected in a given term
    fn check_election_safety(&self, state: &RaftState) -> Result<()> {
        // This is enforced by requiring majority votes
        // We can add additional checks here for distributed scenarios
        
        if state.node_state == NodeState::Leader {
            if state.leader_id.as_ref() != Some(&self.node_id) {
                return Err(anyhow!(
                    "Safety violation: Node {} thinks it's leader but leader_id is {:?}",
                    self.node_id, state.leader_id
                ));
            }
        }
        
        Ok(())
    }
    
    /// Leader Append-Only: A leader never overwrites or deletes entries in its log
    fn check_leader_append_only(&self, state: &RaftState, log: &RaftLog) -> Result<()> {
        // This is enforced by implementation - leaders only append
        // Additional validation can be added here
        
        if state.node_state == NodeState::Leader {
            let last_index = log.last_index();
            if state.commit_index > last_index {
                return Err(anyhow!(
                    "Safety violation: commit_index {} > last_log_index {}",
                    state.commit_index, last_index
                ));
            }
        }
        
        Ok(())
    }
    
    /// Log Matching: If two logs contain an entry with the same index and term,
    /// then the logs are identical in all entries up through the given index
    fn check_log_matching(&self, _state: &RaftState, log: &RaftLog) -> Result<()> {
        // Verify internal log consistency
        let mut prev_index = 0;
        let mut prev_term = 0;
        
        for i in 1..=log.last_index() {
            if let Ok(Some(entry)) = log.get(i) {
                if entry.index != i {
                    return Err(anyhow!(
                        "Log matching violation: entry has index {} but stored at {}",
                        entry.index, i
                    ));
                }
                
                if entry.term < prev_term {
                    return Err(anyhow!(
                        "Log matching violation: term decreased from {} to {} at index {}",
                        prev_term, entry.term, i
                    ));
                }
                
                prev_index = i;
                prev_term = entry.term;
            } else if i <= log.last_index() {
                return Err(anyhow!(
                    "Log matching violation: missing entry at index {}",
                    i
                ));
            }
        }
        
        Ok(())
    }
    
    /// Leader Completeness: If a log entry is committed in a given term,
    /// then that entry will be present in the logs of the leaders for all higher terms
    fn check_leader_completeness(&self, state: &RaftState, log: &RaftLog) -> Result<()> {
        // Ensure all committed entries are present
        for i in 1..=state.commit_index {
            if log.get(i)?.is_none() {
                return Err(anyhow!(
                    "Leader completeness violation: missing committed entry at index {}",
                    i
                ));
            }
        }
        
        Ok(())
    }
    
    /// State Machine Safety: If a server has applied a log entry at a given index
    /// to its state machine, no other server will ever apply a different log entry
    /// for the same index
    fn check_state_machine_safety(&self, state: &RaftState, log: &RaftLog) -> Result<()> {
        // Ensure last_applied doesn't exceed commit_index
        if state.last_applied > state.commit_index {
            return Err(anyhow!(
                "State machine safety violation: last_applied {} > commit_index {}",
                state.last_applied, state.commit_index
            ));
        }
        
        // Ensure all applied entries exist in log
        for i in 1..=state.last_applied {
            if log.get(i)?.is_none() {
                return Err(anyhow!(
                    "State machine safety violation: applied entry {} missing from log",
                    i
                ));
            }
        }
        
        Ok(())
    }
    
    /// Validate a vote request against safety requirements
    pub fn validate_vote_request(
        &self,
        state: &RaftState,
        log: &RaftLog,
        candidate_term: Term,
        candidate_id: &NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) -> Result<bool> {
        // Term must be valid
        if candidate_term <= state.current_term {
            debug!("Rejecting vote: candidate term {} <= current term {}", 
                   candidate_term, state.current_term);
            return Ok(false);
        }
        
        // Check if we already voted in this term
        if let Some(voted_for) = &state.voted_for {
            if state.current_term == candidate_term && voted_for != candidate_id {
                debug!("Rejecting vote: already voted for {} in term {}", 
                       voted_for, candidate_term);
                return Ok(false);
            }
        }
        
        // Log must be at least as up-to-date as ours
        let (our_last_index, our_last_term) = log.last_entry_info();
        
        let candidate_log_ok = last_log_term > our_last_term ||
            (last_log_term == our_last_term && last_log_index >= our_last_index);
        
        if !candidate_log_ok {
            debug!("Rejecting vote: candidate log not up-to-date. Candidate: ({}, {}), Ours: ({}, {})",
                   last_log_term, last_log_index, our_last_term, our_last_index);
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Validate an append entries request
    pub fn validate_append_entries(
        &self,
        state: &RaftState,
        log: &RaftLog,
        leader_term: Term,
        leader_id: &NodeId,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: &[LogEntry],
        leader_commit: LogIndex,
    ) -> Result<bool> {
        // Term must be valid
        if leader_term < state.current_term {
            debug!("Rejecting append entries: leader term {} < current term {}",
                   leader_term, state.current_term);
            return Ok(false);
        }
        
        // Check log consistency at prev_log_index
        if prev_log_index > 0 {
            match log.get(prev_log_index)? {
                Some(entry) => {
                    if entry.term != prev_log_term {
                        debug!("Rejecting append entries: log mismatch at index {}. Expected term {}, got {}",
                               prev_log_index, prev_log_term, entry.term);
                        return Ok(false);
                    }
                }
                None => {
                    debug!("Rejecting append entries: missing entry at index {}", prev_log_index);
                    return Ok(false);
                }
            }
        }
        
        // Validate entries are consistent
        for (i, entry) in entries.iter().enumerate() {
            if entry.term < leader_term {
                warn!("Entry {} has term {} < leader term {}", 
                      entry.index, entry.term, leader_term);
            }
            
            let expected_index = prev_log_index + 1 + i as u64;
            if entry.index != 0 && entry.index != expected_index {
                return Err(anyhow!(
                    "Invalid entry index: expected {}, got {}",
                    expected_index, entry.index
                ));
            }
        }
        
        // Leader commit index should not be too high
        if leader_commit > prev_log_index + entries.len() as u64 {
            warn!("Leader commit index {} higher than expected", leader_commit);
        }
        
        Ok(true)
    }
    
    /// Check if it's safe to become a candidate
    pub fn can_start_election(&self, state: &RaftState, log: &RaftLog) -> Result<bool> {
        // Must be a follower or candidate
        if state.node_state == NodeState::Leader {
            return Ok(false);
        }
        
        // Check if we're part of the cluster
        if !state.cluster_members.contains(&self.node_id) {
            debug!("Cannot start election: not a cluster member");
            return Ok(false);
        }
        
        // Must be a voting member
        if !state.is_voting_member(&self.node_id) {
            debug!("Cannot start election: not a voting member");
            return Ok(false);
        }
        
        // Check basic safety invariants
        self.check_safety_invariants(state, log)?;
        
        Ok(true)
    }
    
    /// Check if it's safe to become leader
    pub fn can_become_leader(
        &self,
        state: &RaftState,
        log: &RaftLog,
        votes_received: &std::collections::HashSet<NodeId>,
    ) -> Result<bool> {
        // Must have majority votes
        let voting_members = state.get_voting_members();
        let majority_size = (voting_members.len() / 2) + 1;
        
        if votes_received.len() < majority_size {
            debug!("Cannot become leader: insufficient votes ({}/{})", 
                   votes_received.len(), majority_size);
            return Ok(false);
        }
        
        // All votes must be from valid voting members
        for voter in votes_received {
            if !state.is_voting_member(voter) {
                return Err(anyhow!(
                    "Safety violation: received vote from non-voting member {}",
                    voter
                ));
            }
        }
        
        // Check safety invariants
        self.check_safety_invariants(state, log)?;
        
        Ok(true)
    }
}

/// Additional safety utilities
pub struct RaftSafetyUtils;

impl RaftSafetyUtils {
    /// Check if two log entries conflict (same index, different terms)
    pub fn entries_conflict(entry1: &LogEntry, entry2: &LogEntry) -> bool {
        entry1.index == entry2.index && entry1.term != entry2.term
    }
    
    /// Find the highest index where two logs agree
    pub fn find_common_index(log1: &RaftLog, log2: &RaftLog) -> LogIndex {
        let min_index = std::cmp::min(log1.last_index(), log2.last_index());
        
        for i in (1..=min_index).rev() {
            if let (Ok(Some(entry1)), Ok(Some(entry2))) = (log1.get(i), log2.get(i)) {
                if entry1.term == entry2.term {
                    return i;
                }
            }
        }
        
        0
    }
    
    /// Validate cluster membership changes are safe
    pub fn validate_membership_change(
        current_members: &std::collections::HashSet<NodeId>,
        new_members: &std::collections::HashSet<NodeId>,
    ) -> Result<()> {
        // Only allow single-member changes for safety
        let added: Vec<_> = new_members.difference(current_members).collect();
        let removed: Vec<_> = current_members.difference(new_members).collect();
        
        let total_changes = added.len() + removed.len();
        if total_changes > 1 {
            return Err(anyhow!(
                "Unsafe membership change: {} members changed (max 1 allowed)",
                total_changes
            ));
        }
        
        // Ensure we don't remove all members
        if new_members.is_empty() {
            return Err(anyhow!("Cannot remove all cluster members"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_safety_checker_creation() {
        let config = RaftConfig::default();
        let checker = RaftSafetyChecker::new("node1".to_string(), config);
        assert_eq!(checker.node_id, "node1");
    }
    
    #[test]
    fn test_entries_conflict() {
        let entry1 = LogEntry::new(1, 1, crate::raft::log::LogCommand::Noop);
        let entry2 = LogEntry::new(1, 2, crate::raft::log::LogCommand::Noop);
        let entry3 = LogEntry::new(2, 1, crate::raft::log::LogCommand::Noop);
        
        assert!(RaftSafetyUtils::entries_conflict(&entry1, &entry2));
        assert!(!RaftSafetyUtils::entries_conflict(&entry1, &entry3));
    }
    
    #[test]
    fn test_membership_change_validation() {
        let mut current = std::collections::HashSet::new();
        current.insert("node1".to_string());
        current.insert("node2".to_string());
        current.insert("node3".to_string());
        
        let mut new_single_add = current.clone();
        new_single_add.insert("node4".to_string());
        
        let mut new_single_remove = current.clone();
        new_single_remove.remove("node3");
        
        let mut new_multiple_changes = current.clone();
        new_multiple_changes.insert("node4".to_string());
        new_multiple_changes.remove("node3");
        
        assert!(RaftSafetyUtils::validate_membership_change(&current, &new_single_add).is_ok());
        assert!(RaftSafetyUtils::validate_membership_change(&current, &new_single_remove).is_ok());
        assert!(RaftSafetyUtils::validate_membership_change(&current, &new_multiple_changes).is_err());
        
        let empty = std::collections::HashSet::new();
        assert!(RaftSafetyUtils::validate_membership_change(&current, &empty).is_err());
    }
}