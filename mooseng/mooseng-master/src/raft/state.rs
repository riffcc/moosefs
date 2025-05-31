use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type Term = u64;
pub type LogIndex = u64;
pub type NodeId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
    NonVoting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftState {
    pub current_term: Term,
    
    pub voted_for: Option<NodeId>,
    
    pub commit_index: LogIndex,
    
    pub last_applied: LogIndex,
    
    pub node_state: NodeState,
    
    pub leader_id: Option<NodeId>,
    
    pub cluster_members: HashSet<NodeId>,
    
    pub non_voting_members: HashSet<NodeId>,
}

impl RaftState {
    pub fn new(_node_id: NodeId, initial_members: Vec<NodeId>) -> Self {
        let mut cluster_members = HashSet::new();
        for member in initial_members {
            cluster_members.insert(member);
        }
        
        Self {
            current_term: 0,
            voted_for: None,
            commit_index: 0,
            last_applied: 0,
            node_state: NodeState::Follower,
            leader_id: None,
            cluster_members,
            non_voting_members: HashSet::new(),
        }
    }
    
    pub fn increment_term(&mut self) {
        self.current_term += 1;
        self.voted_for = None;
    }
    
    pub fn become_follower(&mut self, term: Term, leader_id: Option<NodeId>) {
        self.current_term = term;
        self.node_state = NodeState::Follower;
        self.leader_id = leader_id;
        self.voted_for = None;
    }
    
    pub fn become_candidate(&mut self) {
        self.increment_term();
        self.node_state = NodeState::Candidate;
        self.leader_id = None;
    }
    
    pub fn become_leader(&mut self, node_id: NodeId) {
        self.node_state = NodeState::Leader;
        self.leader_id = Some(node_id);
    }
    
    pub fn is_voting_member(&self, node_id: &NodeId) -> bool {
        self.cluster_members.contains(node_id) && !self.non_voting_members.contains(node_id)
    }
    
    pub fn get_voting_members(&self) -> Vec<NodeId> {
        self.cluster_members
            .iter()
            .filter(|id| !self.non_voting_members.contains(*id))
            .cloned()
            .collect()
    }
    
    pub fn add_member(&mut self, node_id: NodeId, voting: bool) {
        self.cluster_members.insert(node_id.clone());
        if !voting {
            self.non_voting_members.insert(node_id);
        }
    }
    
    pub fn remove_member(&mut self, node_id: &NodeId) {
        self.cluster_members.remove(node_id);
        self.non_voting_members.remove(node_id);
    }
    
    pub fn majority_size(&self) -> usize {
        let voting_members = self.get_voting_members().len();
        (voting_members / 2) + 1
    }
}

#[derive(Debug, Clone)]
pub struct LeaderState {
    pub next_index: std::collections::HashMap<NodeId, LogIndex>,
    
    pub match_index: std::collections::HashMap<NodeId, LogIndex>,
    
    pub last_heartbeat: std::collections::HashMap<NodeId, std::time::Instant>,
}

impl LeaderState {
    pub fn new(members: &[NodeId], last_log_index: LogIndex) -> Self {
        let mut next_index = std::collections::HashMap::new();
        let mut match_index = std::collections::HashMap::new();
        let mut last_heartbeat = std::collections::HashMap::new();
        
        let now = std::time::Instant::now();
        for member in members {
            next_index.insert(member.clone(), last_log_index + 1);
            match_index.insert(member.clone(), 0);
            last_heartbeat.insert(member.clone(), now);
        }
        
        Self {
            next_index,
            match_index,
            last_heartbeat,
        }
    }
    
    pub fn update_match_index(&mut self, node_id: &NodeId, index: LogIndex) {
        if let Some(match_idx) = self.match_index.get_mut(node_id) {
            *match_idx = index;
        }
        if let Some(next_idx) = self.next_index.get_mut(node_id) {
            *next_idx = index + 1;
        }
    }
    
    pub fn decrement_next_index(&mut self, node_id: &NodeId) {
        if let Some(next_idx) = self.next_index.get_mut(node_id) {
            if *next_idx > 1 {
                *next_idx -= 1;
            }
        }
    }
    
    pub fn update_heartbeat(&mut self, node_id: &NodeId) {
        self.last_heartbeat.insert(node_id.clone(), std::time::Instant::now());
    }
}

#[derive(Debug, Clone)]
pub struct CandidateState {
    pub votes_received: HashSet<NodeId>,
    
    pub votes_needed: usize,
}

impl CandidateState {
    pub fn new(self_id: NodeId, majority_size: usize) -> Self {
        let mut votes_received = HashSet::new();
        votes_received.insert(self_id);
        
        Self {
            votes_received,
            votes_needed: majority_size,
        }
    }
    
    pub fn add_vote(&mut self, node_id: NodeId) -> bool {
        self.votes_received.insert(node_id);
        self.has_majority()
    }
    
    pub fn has_majority(&self) -> bool {
        self.votes_received.len() >= self.votes_needed
    }
}