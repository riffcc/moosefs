/// Raft safety mechanisms and invariant checks
/// Implements critical safety properties for Raft consensus algorithm

use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
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
    // Performance optimizations
    last_safety_check: Option<Instant>,
    cached_safety_state: Option<(Term, LogIndex, bool)>, // (term, last_index, is_safe)
}

/// Utility functions for safety checks
pub struct RaftSafetyUtils;

impl RaftSafetyUtils {
    /// Fast check for log consistency without full validation
    pub fn quick_log_check(log: &RaftLog, start_index: LogIndex, end_index: LogIndex) -> Result<bool> {
        if start_index > end_index {
            return Ok(true);
        }
        
        let mut prev_term = 0;
        for i in start_index..=std::cmp::min(end_index, log.last_index()) {
            if let Ok(Some(entry)) = log.get(i) {
                if entry.term < prev_term {
                    return Ok(false);
                }
                prev_term = entry.term;
            }
        }
        Ok(true)
    }
    
    /// Check if a set of votes constitutes a valid majority
    pub fn is_valid_majority(votes: &HashSet<NodeId>, all_members: &HashSet<NodeId>) -> bool {
        let voting_members: HashSet<_> = all_members.iter().collect();
        let valid_votes: HashSet<_> = votes.iter().filter(|v| voting_members.contains(v)).collect();
        
        let majority_size = (voting_members.len() / 2) + 1;
        valid_votes.len() >= majority_size
    }
}

impl RaftSafetyChecker {
    pub fn new(node_id: NodeId, config: RaftConfig) -> Self {
        Self { 
            node_id, 
            config,
            last_safety_check: None,
            cached_safety_state: None,
        }
    }
    
    /// Check all safety invariants for the current state with caching
    pub fn check_safety_invariants(
        &mut self,
        state: &RaftState,
        log: &RaftLog,
    ) -> Result<()> {
        let current_term = state.current_term;
        let last_index = log.last_index();
        
        // Use cached result if state hasn't changed significantly
        if let Some((cached_term, cached_index, cached_result)) = self.cached_safety_state {
            if cached_term == current_term && cached_index == last_index {
                if let Some(last_check) = self.last_safety_check {
                    // Use cache if checked within last 100ms
                    if last_check.elapsed().as_millis() < 100 {
                        return if cached_result { Ok(()) } else { 
                            Err(anyhow!("Cached safety check failed")) 
                        };
                    }
                }
            }
        }
        
        // Perform full safety check
        let start_time = Instant::now();
        
        let result = self.do_safety_check(state, log);
        
        // Update cache
        self.last_safety_check = Some(start_time);
        self.cached_safety_state = Some((current_term, last_index, result.is_ok()));
        
        result
    }
    
    /// Perform the actual safety checks
    fn do_safety_check(&self, state: &RaftState, log: &RaftLog) -> Result<()> {
        self.check_election_safety(state)?;
        self.check_leader_append_only(state, log)?;
        self.check_log_matching_optimized(state, log)?;
        self.check_leader_completeness(state, log)?;
        self.check_state_machine_safety(state, log)?;
        self.check_cluster_membership_safety(state)?;
        
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
    
    /// Optimized log matching check with sampling for large logs
    fn check_log_matching_optimized(&self, _state: &RaftState, log: &RaftLog) -> Result<()> {
        let last_index = log.last_index();
        
        if last_index == 0 {
            return Ok(());
        }
        
        // For large logs, use sampling to reduce check time
        let sample_size = if last_index > 1000 {
            // Check every 10th entry for large logs
            std::cmp::max(last_index / 10, 100)
        } else {
            last_index
        };
        
        let step = if sample_size < last_index {
            last_index / sample_size
        } else {
            1
        };
        
        let mut prev_term = 0;
        let mut indices_to_check: Vec<LogIndex> = (1..=last_index).step_by(step as usize).collect();
        
        // Always check the last few entries
        for i in (last_index.saturating_sub(5))..=last_index {
            if !indices_to_check.contains(&i) {
                indices_to_check.push(i);
            }
        }
        
        indices_to_check.sort_unstable();
        
        for i in indices_to_check {
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
    
    /// Check if this node can safely start an election
    pub fn can_start_election(&self, state: &RaftState, log: &RaftLog) -> Result<bool> {
        // Cannot start election if already a leader
        if state.node_state == NodeState::Leader {
            return Ok(false);
        }
        
        // Ensure we have a reasonably recent log
        let last_index = log.last_index();
        if last_index > 0 {
            let (_, last_term) = log.last_entry_info();
            
            // Don't start election if our log is very old
            if state.current_term > last_term + 5 {
                debug!("Refusing election: log too old (current_term: {}, last_log_term: {})",
                      state.current_term, last_term);
                return Ok(false);
            }
        }
        
        // Check cluster size (must have enough nodes for meaningful election)
        if state.cluster_members.len() < 3 && !self.config.is_single_node() {
            debug!("Refusing election: insufficient cluster size");
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Validate whether this node can become leader with given votes
    pub fn can_become_leader(
        &self,
        state: &RaftState,
        log: &RaftLog,
        votes: &HashSet<NodeId>,
    ) -> Result<bool> {
        // Must have majority votes
        if !RaftSafetyUtils::is_valid_majority(votes, &state.cluster_members) {
            return Ok(false);
        }
        
        // Must be in candidate state
        if state.node_state != NodeState::Candidate {
            return Ok(false);
        }
        
        // Must have voted for ourselves
        if !votes.contains(&self.node_id) {
            return Err(anyhow!("Cannot become leader without self-vote"));
        }
        
        // Additional safety checks
        self.do_safety_check(state, log)?;
        
        Ok(true)
    }
    
    /// Check cluster membership safety
    fn check_cluster_membership_safety(&self, state: &RaftState) -> Result<()> {
        // Node must be a member of the cluster
        if !state.cluster_members.contains(&self.node_id) {
            return Err(anyhow!(
                "Membership safety violation: node {} not in cluster {:?}",
                self.node_id, state.cluster_members
            ));
        }
        
        // Cluster must have reasonable size
        if state.cluster_members.is_empty() {
            return Err(anyhow!("Membership safety violation: empty cluster"));
        }
        
        // Check for duplicate members
        if state.cluster_members.len() != state.cluster_members.iter().collect::<HashSet<_>>().len() {
            return Err(anyhow!("Membership safety violation: duplicate cluster members"));
        }
        
        Ok(())
    }
    
    /// Fast safety check for critical operations
    pub fn quick_safety_check(&self, state: &RaftState, log: &RaftLog) -> Result<bool> {
        // Check basic invariants quickly
        if state.commit_index > log.last_index() {
            return Ok(false);
        }
        
        if state.last_applied > state.commit_index {
            return Ok(false);
        }
        
        if state.cluster_members.is_empty() || !state.cluster_members.contains(&self.node_id) {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Get safety statistics for monitoring
    pub fn get_safety_stats(&self) -> SafetyStats {
        SafetyStats {
            last_check_time: self.last_safety_check,
            cached_state: self.cached_safety_state,
        }
    }
    
    /// Clear safety cache (call when state changes significantly)
    pub fn invalidate_cache(&mut self) {
        self.cached_safety_state = None;
        self.last_safety_check = None;
    }
}

/// Safety statistics for monitoring
#[derive(Debug, Clone)]
pub struct SafetyStats {
    pub last_check_time: Option<Instant>,
    pub cached_state: Option<(Term, LogIndex, bool)>,
}