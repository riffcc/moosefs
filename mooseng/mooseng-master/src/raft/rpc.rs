use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

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
}

#[derive(Debug)]
struct RaftConnection {
    endpoint: String,
    last_connected: std::time::Instant,
}

impl RaftRpc {
    pub fn new(node_id: NodeId, config: RaftConfig) -> Self {
        Self {
            node_id,
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn send_vote_request(
        &self,
        target: &NodeId,
        request: VoteRequest,
    ) -> Result<VoteResponse> {
        debug!("Sending vote request to {} for term {}", target, request.term);
        
        let endpoint = self.get_endpoint(target).await?;
        
        // In a real implementation, this would make an actual RPC call
        // For now, we'll simulate the response
        self.simulate_rpc_call(endpoint, RaftMessage::VoteRequest(request)).await
    }
    
    pub async fn send_append_entries(
        &self,
        target: &NodeId,
        request: AppendEntriesRequest,
    ) -> Result<AppendEntriesResponse> {
        debug!("Sending append entries to {} with {} entries", 
               target, request.entries.len());
        
        let endpoint = self.get_endpoint(target).await?;
        
        self.simulate_rpc_call(endpoint, RaftMessage::AppendEntries(request)).await
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
        });
        
        Ok(endpoint)
    }
    
    fn lookup_endpoint(&self, node_id: &NodeId) -> Result<String> {
        // In a real implementation, this would look up the endpoint from
        // configuration or a discovery service
        
        // For now, we'll use a simple mapping
        let port = match node_id.as_str() {
            "node1" => self.config.raft_port,
            "node2" => self.config.raft_port + 1,
            "node3" => self.config.raft_port + 2,
            _ => return Err(anyhow!("Unknown node: {}", node_id)),
        };
        
        Ok(format!("127.0.0.1:{}", port))
    }
    
    async fn simulate_rpc_call<T>(&self, _endpoint: String, message: RaftMessage) -> Result<T>
    where
        T: From<RaftMessage>,
    {
        // Simulate network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        
        // In a real implementation, this would make an actual network call
        // For now, we'll simulate responses
        let response = match message {
            RaftMessage::VoteRequest(req) => {
                // Simulate vote response
                RaftMessage::VoteResponse(VoteResponse {
                    term: req.term,
                    vote_granted: rand::random::<bool>(),
                })
            }
            RaftMessage::AppendEntries(req) => {
                // Simulate append entries response
                RaftMessage::AppendEntriesResponse(AppendEntriesResponse {
                    term: req.term,
                    success: rand::random::<bool>(),
                    match_index: req.prev_log_index + req.entries.len() as u64,
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