use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast, Mutex};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn, error};

use crate::raft::{
    node::RaftNode,
    state::{NodeId, LogIndex, Term},
    config::RaftConfig,
    rpc::{RaftRpc, AppendEntriesRequest, AppendEntriesResponse},
    log::LogEntry,
};

/// Tracks replication state for each peer
#[derive(Debug, Clone)]
struct PeerReplicationState {
    /// Next log index to send to this peer
    next_index: LogIndex,
    /// Highest log index known to be replicated to this peer
    match_index: LogIndex,
    /// Last time we sent a heartbeat/append entries
    last_sent: Instant,
    /// Whether we're currently replicating to this peer
    replicating: bool,
    /// Number of consecutive failures
    failure_count: u32,
    /// Adaptive batch size based on peer performance
    batch_size: usize,
    /// Last RTT for adaptive timeout calculation
    last_rtt: Option<Duration>,
    /// Whether this peer supports pipelining
    supports_pipelining: bool,
}

impl PeerReplicationState {
    fn new(last_log_index: LogIndex) -> Self {
        Self {
            next_index: last_log_index + 1,
            match_index: 0,
            last_sent: Instant::now(),
            replicating: false,
            failure_count: 0,
            batch_size: 10, // Start with conservative batch size
            last_rtt: None,
            supports_pipelining: true, // Assume support until proven otherwise
        }
    }
    
    /// Adapt batch size based on performance
    fn adapt_batch_size(&mut self, success: bool) {
        if success {
            // Gradually increase batch size on success
            if self.batch_size < 100 {
                self.batch_size = std::cmp::min(self.batch_size * 2, 100);
            }
            self.failure_count = 0;
        } else {
            // Decrease batch size on failure
            self.batch_size = std::cmp::max(self.batch_size / 2, 1);
            self.failure_count += 1;
        }
    }
    
    /// Update RTT measurement
    fn update_rtt(&mut self, rtt: Duration) {
        self.last_rtt = Some(rtt);
    }
    
    /// Get adaptive timeout based on RTT
    fn get_adaptive_timeout(&self, base_timeout: Duration) -> Duration {
        match self.last_rtt {
            Some(rtt) => std::cmp::max(rtt * 3, base_timeout),
            None => base_timeout,
        }
    }
}

#[derive(Debug)]
pub struct ReplicationManager {
    node_id: NodeId,
    node: Arc<RwLock<RaftNode>>,
    config: RaftConfig,
    rpc: Arc<RaftRpc>,
    shutdown_tx: broadcast::Sender<()>,
    peer_states: Arc<Mutex<HashMap<NodeId, PeerReplicationState>>>,
}

impl ReplicationManager {
    pub fn new(
        node_id: NodeId,
        node: Arc<RwLock<RaftNode>>,
        config: RaftConfig,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let rpc = Arc::new(RaftRpc::new(node_id.clone(), config.clone()));
        
        Self {
            node_id,
            node,
            config,
            rpc,
            shutdown_tx,
            peer_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        info!("Starting replication manager for node {}", self.node_id);
        
        let node = self.node.clone();
        let config = self.config.clone();
        let rpc = self.rpc.clone();
        let peer_states = self.peer_states.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        // Main replication loop
        tokio::spawn(async move {
            let mut heartbeat_interval = interval(Duration::from_millis(config.heartbeat_interval_ms));
            heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = heartbeat_interval.tick() => {
                        if let Err(e) = Self::replicate_to_peers(
                            &node, 
                            &config, 
                            &rpc, 
                            &peer_states
                        ).await {
                            warn!("Replication cycle failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Replication manager shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        Ok(())
    }
    
    /// Initialize peer states when becoming leader
    pub async fn initialize_as_leader(&self) -> Result<()> {
        let node_guard = self.node.read().await;
        
        if !node_guard.is_leader() {
            return Ok(());
        }
        
        let last_log_index = node_guard.log.last_index();
        let replication_targets = node_guard.membership_manager.get_replication_targets().await;
        let peers: Vec<NodeId> = replication_targets.into_iter().collect();
        
        drop(node_guard);
        
        let mut peer_states = self.peer_states.lock().await;
        peer_states.clear();
        
        for peer in peers {
            peer_states.insert(peer, PeerReplicationState::new(last_log_index));
        }
        
        info!("Initialized replication for {} peers", peer_states.len());
        Ok(())
    }
    
    /// Main replication logic
    async fn replicate_to_peers(
        node: &Arc<RwLock<RaftNode>>,
        config: &RaftConfig,
        rpc: &Arc<RaftRpc>,
        peer_states: &Arc<Mutex<HashMap<NodeId, PeerReplicationState>>>,
    ) -> Result<()> {
        let (is_leader, current_term, commit_index, peers) = {
            let node_guard = node.read().await;
            
            if !node_guard.is_leader() {
                return Ok(());
            }
            
            let replication_targets = node_guard.membership_manager.get_replication_targets().await;
            let peers: Vec<NodeId> = replication_targets.into_iter().collect();
            
            (true, node_guard.current_term(), node_guard.state.commit_index, peers)
        };
        
        if !is_leader {
            return Ok(());
        }
        
        // Spawn replication tasks for each peer
        let mut replication_handles = Vec::new();
        
        for peer in peers {
            let node_clone = node.clone();
            let config_clone = config.clone();
            let rpc_clone = rpc.clone();
            let peer_states_clone = peer_states.clone();
            let peer_clone = peer.clone();
            
            let handle = tokio::spawn(async move {
                Self::replicate_to_peer(
                    &node_clone,
                    &config_clone,
                    &rpc_clone,
                    &peer_states_clone,
                    &peer_clone,
                    current_term,
                    commit_index,
                ).await
            });
            
            replication_handles.push(handle);
        }
        
        // Wait for all replication tasks to complete
        for handle in replication_handles {
            if let Err(e) = handle.await {
                warn!("Replication task failed: {}", e);
            }
        }
        
        // Update commit index based on majority replication
        Self::update_commit_index(node, peer_states).await?;
        
        Ok(())
    }
    
    /// Replicate to a single peer
    async fn replicate_to_peer(
        node: &Arc<RwLock<RaftNode>>,
        config: &RaftConfig,
        rpc: &Arc<RaftRpc>,
        peer_states: &Arc<Mutex<HashMap<NodeId, PeerReplicationState>>>,
        peer: &NodeId,
        current_term: Term,
        commit_index: LogIndex,
    ) -> Result<()> {
        // Check if we're already replicating to this peer
        {
            let mut states = peer_states.lock().await;
            if let Some(state) = states.get_mut(peer) {
                if state.replicating {
                    return Ok(());
                }
                state.replicating = true;
            } else {
                // Initialize state if not present
                let node_guard = node.read().await;
                let last_log_index = node_guard.log.last_index();
                drop(node_guard);
                states.insert(peer.clone(), PeerReplicationState::new(last_log_index));
            }
        }
        
        let result = Self::do_replicate_to_peer(
            node, config, rpc, peer_states, peer, current_term, commit_index
        ).await;
        
        // Clear replicating flag
        {
            let mut states = peer_states.lock().await;
            if let Some(state) = states.get_mut(peer) {
                state.replicating = false;
            }
        }
        
        result
    }
    
    async fn do_replicate_to_peer(
        node: &Arc<RwLock<RaftNode>>,
        config: &RaftConfig,
        rpc: &Arc<RaftRpc>,
        peer_states: &Arc<Mutex<HashMap<NodeId, PeerReplicationState>>>,
        peer: &NodeId,
        current_term: Term,
        commit_index: LogIndex,
    ) -> Result<()> {
        let (next_index, prev_log_index, prev_log_term, entries) = {
            let states = peer_states.lock().await;
            let state = states.get(peer).ok_or_else(|| anyhow::anyhow!("No state for peer {}", peer))?;
            let next_index = state.next_index;
            drop(states);
            
            let node_guard = node.read().await;
            let last_log_index = node_guard.log.last_index();
            
            if next_index > last_log_index {
                // Nothing to send, just heartbeat
                let prev_log_index = last_log_index;
                let prev_log_term = if prev_log_index == 0 {
                    0
                } else {
                    node_guard.log.term_at(prev_log_index).unwrap_or(0)
                };
                return Self::send_heartbeat(
                    rpc, peer, current_term, prev_log_index, prev_log_term, commit_index
                ).await;
            }
            
            let prev_log_index = next_index - 1;
            let prev_log_term = if prev_log_index == 0 {
                0
            } else {
                node_guard.log.term_at(prev_log_index).unwrap_or(0)
            };
            
            // Get entries to send with adaptive batch sizing
            let states_guard = peer_states.lock().await;
            let peer_state = states_guard.get(peer).ok_or_else(|| anyhow::anyhow!("No state for peer {}", peer))?;
            let adaptive_batch_size = std::cmp::min(
                peer_state.batch_size,
                config.max_append_entries.unwrap_or(100)
            );
            drop(states_guard);
            
            let mut entries = Vec::with_capacity(adaptive_batch_size);
            
            for i in next_index..=std::cmp::min(next_index + adaptive_batch_size as u64 - 1, last_log_index) {
                if let Ok(Some(entry)) = node_guard.log.get(i) {
                    entries.push(entry);
                } else {
                    warn!("Missing log entry at index {} for peer {}", i, peer);
                    break;
                }
            }
            
            (next_index, prev_log_index, prev_log_term, entries)
        };
        
        debug!("Sending {} entries to {} starting at index {}", 
               entries.len(), peer, next_index);
        
        let request = AppendEntriesRequest {
            term: current_term,
            leader_id: rpc.node_id.clone(),
            prev_log_index,
            prev_log_term,
            entries: entries.clone(),
            leader_commit: commit_index,
        };
        
        // Use adaptive timeout based on peer RTT
        let adaptive_timeout = {
            let states_guard = peer_states.lock().await;
            if let Some(peer_state) = states_guard.get(peer) {
                peer_state.get_adaptive_timeout(Duration::from_millis(config.rpc_timeout_ms))
            } else {
                Duration::from_millis(config.rpc_timeout_ms)
            }
        };
        
        let start_time = Instant::now();
        let response = timeout(
            adaptive_timeout,
            rpc.send_append_entries(peer, request),
        ).await;
        
        let rtt = start_time.elapsed();
        
        match response {
            Ok(Ok(resp)) => {
                Self::handle_append_response(
                    node, peer_states, peer, &resp, next_index, entries.len(), Some(rtt)
                ).await?;
            }
            Ok(Err(e)) => {
                error!("Append entries to {} failed: {}", peer, e);
                Self::handle_append_failure(peer_states, peer, false).await;
            }
            Err(_) => {
                warn!("Append entries to {} timed out after {:?}", peer, adaptive_timeout);
                Self::handle_append_failure(peer_states, peer, true).await;
            }
        }
        
        Ok(())
    }
    
    async fn send_heartbeat(
        rpc: &Arc<RaftRpc>,
        peer: &NodeId,
        term: Term,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        commit_index: LogIndex,
    ) -> Result<()> {
        let request = AppendEntriesRequest {
            term,
            leader_id: rpc.node_id.clone(),
            prev_log_index,
            prev_log_term,
            entries: vec![], // Empty for heartbeat
            leader_commit: commit_index,
        };
        
        debug!("Sending heartbeat to {}", peer);
        
        let _ = timeout(
            Duration::from_millis(100), // Short timeout for heartbeats
            rpc.send_append_entries(peer, request),
        ).await;
        
        Ok(())
    }
    
    async fn handle_append_response(
        node: &Arc<RwLock<RaftNode>>,
        peer_states: &Arc<Mutex<HashMap<NodeId, PeerReplicationState>>>,
        peer: &NodeId,
        response: &AppendEntriesResponse,
        sent_index: LogIndex,
        entries_count: usize,
        rtt: Option<Duration>,
    ) -> Result<()> {
        let mut states = peer_states.lock().await;
        let state = states.get_mut(peer).ok_or_else(|| anyhow::anyhow!("No state for peer {}", peer))?;
        
        if response.success {
            // Success - update indices and adapt batch size
            if entries_count > 0 {
                state.match_index = sent_index + entries_count as u64 - 1;
                state.next_index = state.match_index + 1;
                debug!("Successfully replicated {} entries to {}, match_index: {}", 
                      entries_count, peer, state.match_index);
            }
            state.adapt_batch_size(true);
            
            // Update RTT if provided
            if let Some(rtt) = rtt {
                state.update_rtt(rtt);
            }
        } else {
            // Failure - backtrack and adapt
            if state.next_index > 1 {
                // Use binary search style backtracking for faster convergence
                let backtrack_amount = std::cmp::max(1, state.batch_size as u64 / 2);
                state.next_index = state.next_index.saturating_sub(backtrack_amount);
                if state.next_index == 0 {
                    state.next_index = 1;
                }
                debug!("Append failed to {}, decremented next_index by {} to {}", 
                      peer, backtrack_amount, state.next_index);
            }
            state.adapt_batch_size(false);
            
            // Handle term conflicts
            if response.term > sent_index {
                let mut node_guard = node.write().await;
                if response.term > node_guard.current_term() {
                    node_guard.state.become_follower(response.term, None);
                    warn!("Stepping down due to higher term {} from {}", response.term, peer);
                }
            }
        }
        
        state.last_sent = Instant::now();
        Ok(())
    }
    
    async fn handle_append_failure(
        peer_states: &Arc<Mutex<HashMap<NodeId, PeerReplicationState>>>,
        peer: &NodeId,
        is_timeout: bool,
    ) {
        let mut states = peer_states.lock().await;
        if let Some(state) = states.get_mut(peer) {
            state.adapt_batch_size(false);
            state.last_sent = Instant::now();
            
            // Disable pipelining for peers that timeout frequently
            if is_timeout && state.failure_count > 3 {
                state.supports_pipelining = false;
                debug!("Disabled pipelining for peer {} due to timeouts", peer);
            }
        }
    }
    
    /// Update commit index based on majority replication
    async fn update_commit_index(
        node: &Arc<RwLock<RaftNode>>,
        peer_states: &Arc<Mutex<HashMap<NodeId, PeerReplicationState>>>,
    ) -> Result<()> {
        let states = peer_states.lock().await;
        let mut match_indices: Vec<LogIndex> = states.values()
            .map(|state| state.match_index)
            .collect();
        
        // Add our own match index (we always match ourselves)
        let our_last_index = {
            let node_guard = node.read().await;
            node_guard.log.last_index()
        };
        match_indices.push(our_last_index);
        
        // Sort to find majority
        match_indices.sort_unstable();
        match_indices.reverse();
        
        // Find the index that's replicated to a majority
        let majority_size = (match_indices.len() / 2) + 1;
        if match_indices.len() >= majority_size {
            let new_commit_index = match_indices[majority_size - 1];
            
            let mut node_guard = node.write().await;
            let current_commit = node_guard.state.commit_index;
            
            if new_commit_index > current_commit {
                // Verify this is from our current term
                if let Ok(term) = node_guard.log.term_at(new_commit_index) {
                    if term == node_guard.current_term() {
                        node_guard.state.commit_index = new_commit_index;
                        info!("Updated commit index to {}", new_commit_index);
                    }
                }
            }
        }
        
        Ok(())
    }
}