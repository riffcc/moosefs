use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use crate::raft::{
    state::{Term, LogIndex, NodeId},
    log::LogEntry,
    config::RaftConfig,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRequest {
    pub term: Term,
    pub candidate_id: NodeId,
    pub last_log_index: LogIndex,
    pub last_log_term: Term,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    pub term: Term,
    pub vote_granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    pub term: Term,
    pub leader_id: NodeId,
    pub prev_log_index: LogIndex,
    pub prev_log_term: Term,
    pub entries: Vec<LogEntry>,
    pub leader_commit: LogIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub term: Term,
    pub success: bool,
    pub match_index: LogIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotRequest {
    pub term: Term,
    pub leader_id: NodeId,
    pub last_included_index: LogIndex,
    pub last_included_term: Term,
    pub offset: u64,
    pub data: Vec<u8>,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotResponse {
    pub term: Term,
}

#[derive(Debug, Clone)]
pub enum RaftMessage {
    VoteRequest(VoteRequest),
    VoteResponse(VoteResponse),
    AppendEntries(AppendEntriesRequest),
    AppendEntriesResponse(AppendEntriesResponse),
    InstallSnapshot(InstallSnapshotRequest),
    InstallSnapshotResponse(InstallSnapshotResponse),
}

#[derive(Debug)]
pub struct RaftRpc {
    pub node_id: NodeId,
    config: RaftConfig,
    connections: Arc<RwLock<HashMap<NodeId, RaftConnection>>>,
    // Connection pooling and rate limiting
    connection_semaphore: Arc<Semaphore>,
    request_metrics: Arc<RwLock<HashMap<NodeId, RequestMetrics>>>,
}

#[derive(Debug, Clone)]
struct RequestMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    avg_latency: Duration,
    last_request: Instant,
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_latency: Duration::from_millis(0),
            last_request: Instant::now(),
        }
    }
}

#[derive(Debug)]
struct RaftConnection {
    endpoint: String,
    last_connected: std::time::Instant,
    connection_failures: u32,
    last_failure: Option<std::time::Instant>,
    is_healthy: bool,
    avg_latency: Option<Duration>,
}

impl RaftRpc {
    pub fn new(node_id: NodeId, config: RaftConfig) -> Self {
        // Limit concurrent connections to prevent resource exhaustion
        let max_connections = config.initial_members.len() * 2;
        
        Self {
            node_id,
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            request_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn send_vote_request(
        &self,
        target: &NodeId,
        request: VoteRequest,
    ) -> Result<VoteResponse> {
        debug!("Sending vote request to {} for term {}", target, request.term);
        
        // Check if peer is healthy before sending request
        if !self.is_peer_healthy(target).await {
            return Err(anyhow!("Peer {} is marked as unhealthy", target));
        }
        
        let start_time = Instant::now();
        let _permit = self.connection_semaphore.acquire().await
            .map_err(|_| anyhow!("Failed to acquire connection permit"))?;
        
        let endpoint = self.get_endpoint(target).await?;
        
        let result = self.simulate_rpc_call(endpoint, RaftMessage::VoteRequest(request)).await;
        
        // Update metrics
        self.update_request_metrics(target, &result, start_time.elapsed()).await;
        
        result
    }
    
    pub async fn send_append_entries(
        &self,
        target: &NodeId,
        request: AppendEntriesRequest,
    ) -> Result<AppendEntriesResponse> {
        debug!("Sending append entries to {} with {} entries", 
               target, request.entries.len());
        
        // Check if peer is healthy before sending request
        if !self.is_peer_healthy(target).await {
            return Err(anyhow!("Peer {} is marked as unhealthy", target));
        }
        
        let start_time = Instant::now();
        let _permit = self.connection_semaphore.acquire().await
            .map_err(|_| anyhow!("Failed to acquire connection permit"))?;
        
        let endpoint = self.get_endpoint(target).await?;
        
        let result = self.simulate_rpc_call(endpoint, RaftMessage::AppendEntries(request)).await;
        
        // Update metrics and connection health
        self.update_request_metrics(target, &result, start_time.elapsed()).await;
        
        result
    }
    
    pub async fn send_install_snapshot(
        &self,
        target: &NodeId,
        request: InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse> {
        debug!("Sending install snapshot to {} (offset: {}, done: {})", 
               target, request.offset, request.done);
        
        let endpoint = self.get_endpoint(target).await?;
        
        self.simulate_rpc_call(endpoint, RaftMessage::InstallSnapshot(request)).await
    }
    
    pub fn create_append_entries_request(
        &self,
        term: Term,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: Vec<LogEntry>,
        leader_commit: LogIndex,
    ) -> AppendEntriesRequest {
        AppendEntriesRequest {
            term,
            leader_id: self.node_id.clone(),
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit,
        }
    }
    
    async fn get_endpoint(&self, node_id: &NodeId) -> Result<String> {
        let connections = self.connections.read().await;
        
        if let Some(conn) = connections.get(node_id) {
            return Ok(conn.endpoint.clone());
        }
        
        drop(connections);
        
        // Look up endpoint from configuration
        let endpoint = self.lookup_endpoint(node_id)?;
        
        let mut connections = self.connections.write().await;
        connections.insert(node_id.clone(), RaftConnection {
            endpoint: endpoint.clone(),
            last_connected: std::time::Instant::now(),
            connection_failures: 0,
            last_failure: None,
            is_healthy: true,
            avg_latency: None,
        });
        
        Ok(endpoint)
    }
    
    fn lookup_endpoint(&self, node_id: &NodeId) -> Result<String> {
        // In a real implementation, this would look up the endpoint from
        // configuration or a discovery service
        
        // For now, we'll use a simple mapping with better error handling
        let port = match node_id.as_str() {
            "node1" => self.config.raft_port,
            "node2" => self.config.raft_port + 1,
            "node3" => self.config.raft_port + 2,
            _ => {
                warn!("Unknown node ID: {}. Available nodes: {:?}", node_id, self.config.initial_members);
                return Err(anyhow!("Unknown node: {}", node_id));
            }
        };
        
        Ok(format!("127.0.0.1:{}", port))
    }
    
    /// Check if a peer is healthy based on recent request history
    async fn is_peer_healthy(&self, peer_id: &NodeId) -> bool {
        let connections = self.connections.read().await;
        
        if let Some(conn) = connections.get(peer_id) {
            // Mark as unhealthy if too many recent failures
            if conn.connection_failures > 5 {
                if let Some(last_failure) = conn.last_failure {
                    // Allow retry after 30 seconds
                    return last_failure.elapsed() > Duration::from_secs(30);
                }
            }
            conn.is_healthy
        } else {
            true // Unknown peers are considered healthy until proven otherwise
        }
    }
    
    /// Update request metrics and connection health
    async fn update_request_metrics<T>(
        &self,
        peer_id: &NodeId,
        result: &Result<T>,
        latency: Duration,
    ) {
        // Update connection health
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.get_mut(peer_id) {
                match result {
                    Ok(_) => {
                        conn.connection_failures = 0;
                        conn.is_healthy = true;
                        conn.last_connected = Instant::now();
                        
                        // Update average latency
                        conn.avg_latency = match conn.avg_latency {
                            Some(avg) => Some((avg + latency) / 2),
                            None => Some(latency),
                        };
                    }
                    Err(_) => {
                        conn.connection_failures += 1;
                        conn.last_failure = Some(Instant::now());
                        
                        if conn.connection_failures > 3 {
                            conn.is_healthy = false;
                            warn!("Marking peer {} as unhealthy after {} failures", 
                                 peer_id, conn.connection_failures);
                        }
                    }
                }
            }
        }
        
        // Update request metrics
        {
            let mut metrics = self.request_metrics.write().await;
            let peer_metrics = metrics.entry(peer_id.clone()).or_default();
            
            peer_metrics.total_requests += 1;
            peer_metrics.last_request = Instant::now();
            
            match result {
                Ok(_) => {
                    peer_metrics.successful_requests += 1;
                    
                    // Update running average latency
                    let total_successful = peer_metrics.successful_requests;
                    if total_successful == 1 {
                        peer_metrics.avg_latency = latency;
                    } else {
                        let old_avg = peer_metrics.avg_latency;
                        peer_metrics.avg_latency = Duration::from_nanos(
                            (old_avg.as_nanos() as u64 * (total_successful - 1) + latency.as_nanos() as u64) / total_successful
                        );
                    }
                }
                Err(_) => {
                    peer_metrics.failed_requests += 1;
                }
            }
        }
    }
    
    /// Get request metrics for a peer
    pub async fn get_peer_metrics(&self, peer_id: &NodeId) -> Option<RequestMetrics> {
        let metrics = self.request_metrics.read().await;
        metrics.get(peer_id).cloned()
    }
    
    /// Get overall RPC health status
    pub async fn get_health_status(&self) -> HashMap<NodeId, bool> {
        let connections = self.connections.read().await;
        connections.iter()
            .map(|(id, conn)| (id.clone(), conn.is_healthy))
            .collect()
    }
    
    async fn simulate_rpc_call<T>(&self, _endpoint: String, message: RaftMessage) -> Result<T>
    where
        T: From<RaftMessage>,
    {
        // Simulate realistic network conditions
        let base_delay = 5;
        let jitter = rand::random::<u64>() % 10; // 0-10ms jitter
        tokio::time::sleep(Duration::from_millis(base_delay + jitter)).await;
        
        // Simulate occasional network failures (5% failure rate)
        if rand::random::<f64>() < 0.05 {
            return Err(anyhow!("Simulated network failure"));
        }
        
        // In a real implementation, this would make an actual network call
        // For now, we'll simulate responses with realistic behavior
        let response = match message {
            RaftMessage::VoteRequest(req) => {
                // Simulate vote response with 70% grant rate
                RaftMessage::VoteResponse(VoteResponse {
                    term: req.term,
                    vote_granted: rand::random::<f64>() < 0.7,
                })
            }
            RaftMessage::AppendEntries(req) => {
                // Simulate append entries response with 85% success rate
                let success = rand::random::<f64>() < 0.85;
                RaftMessage::AppendEntriesResponse(AppendEntriesResponse {
                    term: req.term,
                    success,
                    match_index: if success {
                        req.prev_log_index + req.entries.len() as u64
                    } else {
                        req.prev_log_index.saturating_sub(1)
                    },
                })
            }
            RaftMessage::InstallSnapshot(req) => {
                // Simulate install snapshot response
                RaftMessage::InstallSnapshotResponse(InstallSnapshotResponse {
                    term: req.term,
                })
            }
            _ => return Err(anyhow!("Unexpected message type")),
        };
        
        Ok(T::from(response))
    }
}

// Implement conversions for simulated responses
impl From<RaftMessage> for VoteResponse {
    fn from(msg: RaftMessage) -> Self {
        match msg {
            RaftMessage::VoteResponse(resp) => resp,
            _ => panic!("Invalid message type"),
        }
    }
}

impl From<RaftMessage> for AppendEntriesResponse {
    fn from(msg: RaftMessage) -> Self {
        match msg {
            RaftMessage::AppendEntriesResponse(resp) => resp,
            _ => panic!("Invalid message type"),
        }
    }
}

impl From<RaftMessage> for InstallSnapshotResponse {
    fn from(msg: RaftMessage) -> Self {
        match msg {
            RaftMessage::InstallSnapshotResponse(resp) => resp,
            _ => panic!("Invalid message type"),
        }
    }
}

// gRPC service implementation for Raft
#[tonic::async_trait]
pub trait RaftService: Send + Sync + 'static {
    async fn request_vote(
        &self,
        request: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status>;
    
    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status>;
    
    async fn install_snapshot(
        &self,
        request: Request<InstallSnapshotRequest>,
    ) -> Result<Response<InstallSnapshotResponse>, Status>;
}