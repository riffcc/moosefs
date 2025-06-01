//! Minimal MooseNG ChunkServer for demo purposes
//! This is a simplified implementation that compiles and runs

use axum::{
    routing::{get, post, put},
    extract::Path,
    body::Bytes,
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
struct ChunkMetadata {
    id: u64,
    size: u64,
    checksum: String,
    version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    server_id: u32,
    role: String,
    available_space: u64,
    used_space: u64,
}

struct ChunkServerState {
    chunks: HashMap<u64, Vec<u8>>,
    metadata: HashMap<u64, ChunkMetadata>,
    used_space: u64,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let state = Arc::new(RwLock::new(ChunkServerState {
        chunks: HashMap::new(),
        metadata: HashMap::new(),
        used_space: 0,
    }));

    // Build the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/chunks/:chunk_id", put({
            let state = state.clone();
            move |path, body| write_chunk(state, path, body)
        }))
        .route("/api/v1/chunks/:chunk_id", get({
            let state = state.clone();
            move |path| read_chunk(state, path)
        }))
        .route("/api/v1/chunks", get({
            let state = state.clone();
            move || list_chunks(state)
        }));

    // Get the port from environment or use default
    let port = std::env::var("MOOSENG_PORT")
        .unwrap_or_else(|_| "9420".to_string())
        .parse::<u16>()
        .unwrap_or(9420);
    
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    info!("Minimal MooseNG ChunkServer starting on {}", addr);
    
    // Run the server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn health_check() -> Json<HealthResponse> {
    let server_id = std::env::var("MOOSENG_SERVER_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u32>()
        .unwrap_or(1);
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        server_id,
        role: "chunkserver".to_string(),
        available_space: 100 * 1024 * 1024 * 1024, // 100GB
        used_space: 0,
    })
}

async fn write_chunk(
    state: Arc<RwLock<ChunkServerState>>,
    Path(chunk_id): Path<u64>,
    body: Bytes,
) -> Json<ChunkMetadata> {
    let mut state = state.write().await;
    
    let size = body.len() as u64;
    let checksum = format!("{:x}", md5::compute(&body));
    
    let metadata = ChunkMetadata {
        id: chunk_id,
        size,
        checksum: checksum.clone(),
        version: 1,
    };
    
    state.chunks.insert(chunk_id, body.to_vec());
    state.metadata.insert(chunk_id, metadata.clone());
    state.used_space += size;
    
    info!("Stored chunk {} ({} bytes)", chunk_id, size);
    
    Json(metadata)
}

async fn read_chunk(
    state: Arc<RwLock<ChunkServerState>>,
    Path(chunk_id): Path<u64>,
) -> Result<Bytes, &'static str> {
    let state = state.read().await;
    
    match state.chunks.get(&chunk_id) {
        Some(data) => {
            info!("Read chunk {} ({} bytes)", chunk_id, data.len());
            Ok(Bytes::from(data.clone()))
        },
        None => {
            warn!("Chunk {} not found", chunk_id);
            Err("Chunk not found")
        }
    }
}

async fn list_chunks(state: Arc<RwLock<ChunkServerState>>) -> Json<Vec<ChunkMetadata>> {
    let state = state.read().await;
    let chunks: Vec<ChunkMetadata> = state.metadata.values().cloned().collect();
    Json(chunks)
}