//! Minimal MooseNG Master Server for demo purposes
//! This is a simplified implementation that compiles and runs

use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::sync::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, warn};
use tracing_subscriber;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileInfo {
    id: u64,
    name: String,
    size: u64,
    chunks: Vec<ChunkInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkInfo {
    id: u64,
    server_id: u32,
    version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreateFileRequest {
    name: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreateFileResponse {
    file_id: u64,
    chunks: Vec<ChunkInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    node_id: u32,
    role: String,
}

struct MasterState {
    files: HashMap<u64, FileInfo>,
    next_file_id: u64,
    next_chunk_id: u64,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let state = Arc::new(RwLock::new(MasterState {
        files: HashMap::new(),
        next_file_id: 1,
        next_chunk_id: 1,
    }));

    // Build the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/files", post({
            let state = state.clone();
            move |req| create_file(state, req)
        }))
        .route("/api/v1/files", get({
            let state = state.clone();
            move || list_files(state)
        }));

    // Get the port from environment or use default
    let port = std::env::var("MOOSENG_CLIENT_PORT")
        .unwrap_or_else(|_| "9421".to_string())
        .parse::<u16>()
        .unwrap_or(9421);
    
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    info!("Minimal MooseNG Master starting on {}", addr);
    
    // Run the server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn health_check() -> Json<HealthResponse> {
    let node_id = std::env::var("MOOSENG_NODE_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u32>()
        .unwrap_or(1);
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        node_id,
        role: "master".to_string(),
    })
}

async fn create_file(
    state: Arc<RwLock<MasterState>>,
    Json(req): Json<CreateFileRequest>,
) -> Json<CreateFileResponse> {
    let mut state = state.write().await;
    
    let file_id = state.next_file_id;
    state.next_file_id += 1;
    
    // Create chunks (simplified: one chunk per 64MB)
    let chunk_count = (req.size + 67108863) / 67108864; // 64MB chunks
    let mut chunks = Vec::new();
    
    for i in 0..chunk_count {
        let chunk_id = state.next_chunk_id;
        state.next_chunk_id += 1;
        
        chunks.push(ChunkInfo {
            id: chunk_id,
            server_id: (i % 3 + 1) as u32, // Distribute across 3 chunkservers
            version: 1,
        });
    }
    
    let file_info = FileInfo {
        id: file_id,
        name: req.name.clone(),
        size: req.size,
        chunks: chunks.clone(),
    };
    
    state.files.insert(file_id, file_info);
    
    info!("Created file {} with {} chunks", file_id, chunks.len());
    
    Json(CreateFileResponse {
        file_id,
        chunks,
    })
}

async fn list_files(state: Arc<RwLock<MasterState>>) -> Json<Vec<FileInfo>> {
    let state = state.read().await;
    let files: Vec<FileInfo> = state.files.values().cloned().collect();
    Json(files)
}