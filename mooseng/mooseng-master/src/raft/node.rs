use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::raft::{
    state::{RaftState, NodeState, LeaderState, CandidateState, Term, LogIndex, NodeId},
    log::{RaftLog, LogEntry, LogCommand},
    config::RaftConfig,
    membership::{MembershipManager, ConfigChangeType, JointConfiguration},
};

#[derive(Debug)]
pub struct RaftNode {
    pub node_id: NodeId,
    pub state: RaftState,
    pub log: RaftLog,
    pub config: RaftConfig,
    pub leader_state: Option<LeaderState>,
    pub candidate_state: Option<CandidateState>,
    pub membership_manager: MembershipManager,
    last_heartbeat: std::time::Instant,
}

impl RaftNode {
    pub fn new(config: RaftConfig) -> Result<Self> {
        let node_id = config.node_id.clone();
        let initial_members = config.initial_members.clone();
        
        let state = RaftState::new(node_id.clone(), initial_members);
        let log = RaftLog::new(config.data_dir.join("raft-log"))?;
        let membership_manager = MembershipManager::new(node_id.clone(), config.clone());
        
        Ok(Self {
            node_id,
            state,
            log,
            config,
            leader_state: None,
            candidate_state: None,
            membership_manager,
            last_heartbeat: std::time::Instant::now(),
        })
    }
    
    pub fn is_leader(&self) -> bool {
        self.state.node_state == NodeState::Leader
    }
    
    pub fn is_candidate(&self) -> bool {
        self.state.node_state == NodeState::Candidate
    }
    
    pub fn is_follower(&self) -> bool {
        self.state.node_state == NodeState::Follower
    }
    
    pub fn get_leader_id(&self) -> Option<String> {
        self.state.leader_id.clone()
    }
    
    pub fn current_term(&self) -> Term {
        self.state.current_term
    }
    
    pub fn reset_heartbeat(&mut self) {
        self.last_heartbeat = std::time::Instant::now();
    }
    
    pub fn heartbeat_elapsed(&self) -> std::time::Duration {
        self.last_heartbeat.elapsed()
    }
    
    pub async fn start_election(&mut self) -> Result<()> {
        info!("Node {} starting election for term {}", self.node_id, self.state.current_term + 1);
        
        self.state.become_candidate();
        self.state.voted_for = Some(self.node_id.clone());
        
        let majority_size = self.membership_manager.get_majority_size().await;
        self.candidate_state = Some(CandidateState::new(self.node_id.clone(), majority_size));
        
        self.reset_heartbeat();
        
        Ok(())
    }
    
    pub async fn become_leader(&mut self) -> Result<()> {
        info!("Node {} becoming leader for term {}", self.node_id, self.state.current_term);
        
        self.state.become_leader(self.node_id.clone());
        
        let replication_targets = self.membership_manager.get_replication_targets().await;
        let members: Vec<NodeId> = replication_targets.into_iter().collect();
        
        let last_log_index = self.log.last_index();
        self.leader_state = Some(LeaderState::new(&members, last_log_index));
        self.candidate_state = None;
        
        self.append_noop_entry()?;
        
        Ok(())
    }
    
    pub fn step_down(&mut self, term: Term, leader_id: Option<NodeId>) {
        info!("Node {} stepping down to follower at term {}", self.node_id, term);
        
        self.state.become_follower(term, leader_id);
        self.leader_state = None;
        self.candidate_state = None;
        self.reset_heartbeat();
    }
    
    pub async fn handle_vote_request(
        &mut self,
        candidate_id: &NodeId,
        term: Term,
        last_log_index: LogIndex,
        last_log_term: Term,
    ) -> Result<bool> {
        debug!("Node {} handling vote request from {} for term {}", 
               self.node_id, candidate_id, term);
        
        // Only voting members can request votes
        if !self.membership_manager.can_vote(candidate_id).await {
            debug!("Rejecting vote: {} is not a voting member", candidate_id);
            return Ok(false);
        }
        
        if term < self.state.current_term {
            debug!("Rejecting vote: term {} < current term {}", term, self.state.current_term);
            return Ok(false);
        }
        
        if term > self.state.current_term {
            self.step_down(term, None);
        }
        
        let vote_granted = match &self.state.voted_for {
            None => true,
            Some(voted_id) => voted_id == candidate_id,
        };
        
        if !vote_granted {
            debug!("Rejecting vote: already voted for {:?}", self.state.voted_for);
            return Ok(false);
        }
        
        let (our_last_index, our_last_term) = self.log.last_entry_info();
        let log_is_up_to_date = 
            last_log_term > our_last_term ||
            (last_log_term == our_last_term && last_log_index >= our_last_index);
        
        if !log_is_up_to_date {
            debug!("Rejecting vote: candidate log not up to date");
            return Ok(false);
        }
        
        self.state.voted_for = Some(candidate_id.clone());
        self.reset_heartbeat();
        
        info!("Node {} granting vote to {} for term {}", self.node_id, candidate_id, term);
        Ok(true)
    }
    
    pub async fn handle_vote_response(
        &mut self,
        from: &NodeId,
        term: Term,
        vote_granted: bool,
    ) -> Result<bool> {
        if term > self.state.current_term {
            self.step_down(term, None);
            return Ok(false);
        }
        
        if term < self.state.current_term || !self.is_candidate() {
            return Ok(false);
        }
        
        if let Some(candidate_state) = &mut self.candidate_state {
            if vote_granted {
                info!("Node {} received vote from {}", self.node_id, from);
                if candidate_state.add_vote(from.clone()) {
                    info!("Node {} won election with {} votes", 
                          self.node_id, candidate_state.votes_received.len());
                    self.become_leader().await?;
                    return Ok(true);
                }
            } else {
                debug!("Node {} vote rejected by {}", self.node_id, from);
            }
        }
        
        Ok(false)
    }
    
    pub fn handle_append_entries(
        &mut self,
        leader_id: &NodeId,
        term: Term,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: Vec<LogEntry>,
        leader_commit: LogIndex,
    ) -> Result<(bool, LogIndex)> {
        debug!("Node {} handling append entries from {} at term {}", 
               self.node_id, leader_id, term);
        
        if term < self.state.current_term {
            return Ok((false, 0));
        }
        
        if term > self.state.current_term || self.is_candidate() {
            self.step_down(term, Some(leader_id.clone()));
        }
        
        self.reset_heartbeat();
        
        if !self.log.matches_at(prev_log_index, prev_log_term) {
            debug!("Log mismatch at index {}", prev_log_index);
            return Ok((false, 0));
        }
        
        if !entries.is_empty() {
            let last_new_index = self.log.append_entries(prev_log_index, entries)?;
            debug!("Appended entries up to index {}", last_new_index);
        }
        
        if leader_commit > self.state.commit_index {
            let last_log_index = self.log.last_index();
            self.state.commit_index = std::cmp::min(leader_commit, last_log_index);
            debug!("Updated commit index to {}", self.state.commit_index);
        }
        
        Ok((true, self.log.last_index()))
    }
    
    pub async fn handle_append_entries_response(
        &mut self,
        from: &NodeId,
        term: Term,
        success: bool,
        match_index: LogIndex,
    ) -> Result<()> {
        if term > self.state.current_term {
            self.step_down(term, None);
            return Ok(());
        }
        
        if !self.is_leader() || term != self.state.current_term {
            return Ok(());
        }
        
        if let Some(leader_state) = &mut self.leader_state {
            if success {
                leader_state.update_match_index(from, match_index);
                self.try_advance_commit_index().await?;
            } else {
                leader_state.decrement_next_index(from);
            }
            leader_state.update_heartbeat(from);
        }
        
        Ok(())
    }
    
    async fn try_advance_commit_index(&mut self) -> Result<()> {
        if let Some(leader_state) = &self.leader_state {
            let mut match_indices: Vec<LogIndex> = leader_state.match_index
                .values()
                .cloned()
                .collect();
            match_indices.push(self.log.last_index());
            match_indices.sort();
            
            let majority_size = self.membership_manager.get_majority_size().await;
            let majority_idx = majority_size - 1;
            if majority_idx < match_indices.len() {
                let new_commit_index = match_indices[match_indices.len() - 1 - majority_idx];
                
                if new_commit_index > self.state.commit_index &&
                   self.log.term_at(new_commit_index)? == self.state.current_term {
                    self.state.commit_index = new_commit_index;
                    info!("Advanced commit index to {}", new_commit_index);
                }
            }
        }
        
        Ok(())
    }
    
    fn append_noop_entry(&mut self) -> Result<()> {
        let entry = LogEntry::new_noop(self.state.current_term);
        self.log.append(entry)?;
        Ok(())
    }
    
    pub async fn apply_committed_entries<F>(&mut self, mut apply_fn: F) -> Result<()>
    where
        F: FnMut(&LogEntry) -> Result<()>,
    {
        while self.state.last_applied < self.state.commit_index {
            self.state.last_applied += 1;
            let entry = self.log.get(self.state.last_applied)?
                .ok_or_else(|| anyhow!("Missing log entry at index {}", self.state.last_applied))?;
            
            // Handle configuration changes
            if let LogCommand::ConfigChange { change_type, joint_config } = &entry.command {
                self.membership_manager.apply_config_change(change_type, joint_config).await?;
            }
            
            apply_fn(&entry)?;
            debug!("Applied log entry at index {}", self.state.last_applied);
        }
        
        Ok(())
    }

    /// Propose a configuration change (only allowed for leaders)
    pub async fn propose_config_change(&mut self, change: ConfigChangeType) -> Result<LogIndex> {
        if !self.is_leader() {
            return Err(anyhow!("Only leader can propose configuration changes"));
        }

        self.membership_manager.propose_config_change(change, &mut self.log, &self.state).await
    }

    /// Read data with eventual consistency (can be served by any node)
    pub fn read_eventually_consistent(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // Search through committed entries for the latest value of the key
        for index in (1..=self.state.commit_index).rev() {
            if let Some(entry) = self.log.get(index)? {
                match &entry.command {
                    LogCommand::SetMetadata { key: entry_key, value } if entry_key == key => {
                        return Ok(Some(value.clone()));
                    }
                    LogCommand::DeleteMetadata { key: entry_key } if entry_key == key => {
                        return Ok(None);
                    }
                    _ => continue,
                }
            }
        }
        Ok(None)
    }

    /// Read data with linearizable consistency (leader-only or with lease)
    pub async fn read_linearizable(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if self.is_leader() {
            // Leader can serve reads immediately if it's still the leader
            self.read_eventually_consistent(key)
        } else {
            // Non-leaders must forward to leader
            Err(anyhow!("Linearizable reads must be forwarded to leader"))
        }
    }

    /// Check if this node can serve reads based on read consistency requirements
    pub fn can_serve_read(&self, require_linearizable: bool) -> bool {
        if require_linearizable {
            self.is_leader()
        } else {
            // Any node with committed data can serve eventually consistent reads
            self.state.commit_index > 0
        }
    }

    /// Get the current read lease expiration (for leader lease-based reads)
    pub fn get_read_lease_expiration(&self) -> Option<std::time::Instant> {
        if self.is_leader() {
            // Leader lease is valid as long as we've received responses from majority recently
            let lease_duration = std::time::Duration::from_millis(self.config.election_timeout_min_ms / 2);
            Some(self.last_heartbeat + lease_duration)
        } else {
            None
        }
    }

    /// Check if the leader lease is still valid for serving reads
    pub fn has_valid_read_lease(&self) -> bool {
        if let Some(lease_expiration) = self.get_read_lease_expiration() {
            std::time::Instant::now() < lease_expiration
        } else {
            false
        }
    }
}