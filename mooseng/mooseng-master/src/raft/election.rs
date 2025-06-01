use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};

use crate::raft::{
    node::RaftNode,
    state::NodeId,
    config::RaftConfig,
    rpc::{RaftRpc, VoteRequest, VoteResponse},
    safety::RaftSafetyChecker,
};

pub struct ElectionManager {
    node_id: NodeId,
    node: Arc<RwLock<RaftNode>>,
    config: RaftConfig,
    rpc: Arc<RaftRpc>,
    safety_checker: Arc<RaftSafetyChecker>,
    shutdown_tx: broadcast::Sender<()>,
    election_timeout_generator: Box<dyn Fn() -> Duration + Send + Sync>,
}

impl std::fmt::Debug for ElectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElectionManager")
            .field("node_id", &self.node_id)
            .field("node", &self.node)
            .field("config", &self.config)
            .field("rpc", &self.rpc)
            .field("safety_checker", &self.safety_checker)
            .field("shutdown_tx", &self.shutdown_tx)
            .field("election_timeout_generator", &"<function>")
            .finish()
    }
}

impl ElectionManager {
    pub fn new(
        node_id: NodeId,
        node: Arc<RwLock<RaftNode>>,
        config: RaftConfig,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let rpc = Arc::new(RaftRpc::new(node_id.clone(), config.clone()));
        let safety_checker = Arc::new(RaftSafetyChecker::new(node_id.clone(), config.clone()));
        
        // Create election timeout generator with randomization
        let min_timeout = config.election_timeout_ms;
        let max_timeout = min_timeout * 2;
        let election_timeout_generator = Box::new(move || {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let timeout_ms = rng.gen_range(min_timeout..max_timeout);
            Duration::from_millis(timeout_ms)
        });
        
        Self {
            node_id,
            node,
            config,
            rpc,
            safety_checker,
            shutdown_tx,
            election_timeout_generator,
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        info!("Starting election manager for node {}", self.node_id);
        
        let node = self.node.clone();
        let config = self.config.clone();
        let rpc = self.rpc.clone();
        let safety_checker = self.safety_checker.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let timeout_gen = self.election_timeout_generator.as_ref();
        
        // Main election timer task
        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_millis(50));
            check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = check_interval.tick() => {
                        if let Err(e) = Self::check_election_timeout(&node, &config, &rpc, &safety_checker, timeout_gen).await {
                            warn!("Election timeout check failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Election manager shutting down");
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
    
    async fn check_election_timeout(
        node: &Arc<RwLock<RaftNode>>,
        config: &RaftConfig,
        rpc: &Arc<RaftRpc>,
        safety_checker: &Arc<RaftSafetyChecker>,
        timeout_gen: &(dyn Fn() -> Duration + Send + Sync),
    ) -> Result<()> {
        let (should_start_election, current_state) = {
            let node_guard = node.read().await;
            let elapsed = node_guard.heartbeat_elapsed();
            let timeout = timeout_gen();
            let state = format!("Node: {}, State: {:?}, Term: {}", 
                node_guard.node_id, node_guard.state.node_state, node_guard.current_term());
            
            (!node_guard.is_leader() && elapsed > timeout, state)
        };
        
        if should_start_election {
            debug!("Election timeout detected: {}", current_state);
            
            // Check safety before starting election with better error handling
            let can_start = {
                let node_guard = node.read().await;
                match safety_checker.can_start_election(&node_guard.state, &node_guard.log) {
                    Ok(can_start) => can_start,
                    Err(e) => {
                        warn!("Safety check failed with error: {}. Refusing to start election.", e);
                        return Ok(());
                    }
                }
            };
            
            if can_start {
                match Self::start_election_round(node, config, rpc, safety_checker).await {
                    Ok(_) => debug!("Election round started successfully"),
                    Err(e) => {
                        warn!("Election round failed: {}. Will retry on next timeout.", e);
                        // Don't propagate error to keep the election manager running
                    }
                }
            } else {
                debug!("Safety check failed, cannot start election: {}", current_state);
            }
        }
        
        Ok(())
    }
    
    async fn start_election_round(
        node: &Arc<RwLock<RaftNode>>,
        config: &RaftConfig,
        rpc: &Arc<RaftRpc>,
        safety_checker: &Arc<RaftSafetyChecker>,
    ) -> Result<()> {
        let (term, last_log_index, last_log_term, peers) = {
            let mut node_guard = node.write().await;
            
            // Start election
            node_guard.start_election().await?;
            
            let (last_log_index, last_log_term) = node_guard.log.last_entry_info();
            let term = node_guard.current_term();
            let peers: Vec<NodeId> = node_guard.state.cluster_members
                .iter()
                .filter(|id| **id != node_guard.node_id)
                .cloned()
                .collect();
            
            (term, last_log_index, last_log_term, peers)
        };
        
        info!("Starting election for term {}", term);
        
        // Request votes from all peers
        let vote_futures: Vec<_> = peers.iter().map(|peer| {
            let peer_id = peer.clone();
            let rpc_clone = rpc.clone();
            let vote_request = VoteRequest {
                term,
                candidate_id: rpc_clone.node_id.clone(),
                last_log_index,
                last_log_term,
            };
            
            async move {
                let result = timeout(
                    Duration::from_millis(config.rpc_timeout_ms),
                    rpc_clone.send_vote_request(&peer_id, vote_request)
                ).await;
                
                match result {
                    Ok(Ok(response)) => Some((peer_id, response)),
                    Ok(Err(e)) => {
                        debug!("Vote request to {} failed: {}", peer_id, e);
                        None
                    }
                    Err(_) => {
                        debug!("Vote request to {} timed out", peer_id);
                        None
                    }
                }
            }
        }).collect();
        
        // Collect vote responses with error tracking
        let responses = futures::future::join_all(vote_futures).await;
        let mut vote_errors = 0;
        let mut total_peers = peers.len();
        
        // Process responses with enhanced error handling
        for response in responses.into_iter() {
            match response {
                Some((peer_id, vote_response)) => {
                    let mut node_guard = node.write().await;
                    
                    // Check if we're still a candidate
                    if !node_guard.is_candidate() {
                        debug!("No longer candidate, stopping vote processing");
                        break;
                    }
                    
                    // Handle the vote response with error handling
                    match node_guard.handle_vote_response(
                        &peer_id,
                        vote_response.term,
                        vote_response.vote_granted,
                    ).await {
                        Ok(won_election) => {
                            if won_election {
                                // Additional safety check before becoming leader
                                if let Some(candidate_state) = &node_guard.candidate_state {
                                    match safety_checker.can_become_leader(
                                        &node_guard.state,
                                        &node_guard.log,
                                        &candidate_state.votes_received,
                                    ) {
                                        Ok(true) => {
                                            info!("Won election for term {} with {} votes - becoming leader", 
                                                node_guard.current_term(), candidate_state.votes_received.len());
                                            drop(node_guard);
                                            return Ok(());
                                        },
                                        Ok(false) => {
                                            warn!("Safety check failed - cannot become leader despite winning election");
                                            // Step down to follower
                                            let term = node_guard.current_term();
                                            node_guard.step_down(term, None);
                                        },
                                        Err(e) => {
                                            error!("Safety check error during leader validation: {}", e);
                                            let term = node_guard.current_term();
                                            node_guard.step_down(term, None);
                                        }
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            warn!("Error handling vote response from {}: {}", peer_id, e);
                        }
                    }
                },
                None => {
                    vote_errors += 1;
                }
            }
        }
        
        // Log election outcome
        if vote_errors > 0 {
            warn!("Election completed with {} errors out of {} peers", vote_errors, total_peers);
        } else {
            debug!("Election completed - did not win majority");
        }
        
        Ok(())
    }
    
    pub async fn broadcast_heartbeats(
        node: &Arc<RwLock<RaftNode>>,
        config: &RaftConfig,
        rpc: &Arc<RaftRpc>,
    ) -> Result<()> {
        let node_guard = node.read().await;
        if !node_guard.is_leader() {
            return Ok(());
        }
        
        let peers: Vec<NodeId> = node_guard.state.cluster_members
            .iter()
            .filter(|id| **id != node_guard.node_id)
            .cloned()
            .collect();
        
        drop(node_guard);
        
        for peer in peers {
            let node_clone = node.clone();
            let rpc_clone = rpc.clone();
            let config_clone = config.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::send_heartbeat_to_peer(
                    &node_clone,
                    &rpc_clone,
                    &config_clone,
                    &peer,
                ).await {
                    debug!("Failed to send heartbeat to {}: {}", peer, e);
                }
            });
        }
        
        Ok(())
    }
    
    async fn send_heartbeat_to_peer(
        node: &Arc<RwLock<RaftNode>>,
        rpc: &Arc<RaftRpc>,
        config: &RaftConfig,
        peer: &NodeId,
    ) -> Result<()> {
        let (is_leader, append_request) = {
            let node_guard = node.read().await;
            
            if !node_guard.is_leader() {
                return Ok(());
            }
            
            let term = node_guard.current_term();
            let commit_index = node_guard.state.commit_index;
            
            // For heartbeats, we send empty entries
            let append_request = rpc.create_append_entries_request(
                term,
                node_guard.log.last_index(),
                node_guard.log.last_entry_info().1,
                vec![],
                commit_index,
            );
            
            (true, append_request)
        };
        
        if !is_leader {
            return Ok(());
        }
        
        let response = timeout(
            Duration::from_millis(config.rpc_timeout_ms),
            rpc.send_append_entries(peer, append_request),
        ).await;
        
        match response {
            Ok(Ok(resp)) => {
                let mut node_guard = node.write().await;
                if node_guard.is_leader() {
                    match node_guard.handle_append_entries_response(
                        peer,
                        resp.term,
                        resp.success,
                        resp.match_index,
                    ).await {
                        Ok(_) => debug!("Heartbeat to {} successful", peer),
                        Err(e) => warn!("Error processing heartbeat response from {}: {}", peer, e),
                    }
                }
            }
            Ok(Err(e)) => {
                debug!("Heartbeat to {} failed: {}", peer, e);
                // Consider marking peer as temporarily unavailable
            }
            Err(_) => {
                debug!("Heartbeat to {} timed out", peer);
                // Consider exponential backoff for timed out peers
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_election_manager_creation() {
        let dir = tempdir().unwrap();
        let config = RaftConfig {
            node_id: "node1".to_string(),
            data_dir: dir.path().to_path_buf(),
            initial_members: vec!["node1".to_string(), "node2".to_string()],
            election_timeout_ms: 150,
            heartbeat_interval_ms: 50,
            rpc_timeout_ms: 100,
            snapshot_interval: 1000,
        };
        
        let node = Arc::new(RwLock::new(RaftNode::new(config.clone()).unwrap()));
        let election_mgr = ElectionManager::new("node1".to_string(), node, config);
        
        assert_eq!(election_mgr.node_id, "node1");
    }
}