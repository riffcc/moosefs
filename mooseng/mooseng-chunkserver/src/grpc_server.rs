//! gRPC server implementation for ChunkServer
//! 
//! This module provides the gRPC interface for external communication
//! with the chunk server, implementing the ChunkServer service.

use std::sync::Arc;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info, debug};

use mooseng_protocol::{
    chunk_server_service_server::{ChunkServerService, ChunkServerServiceServer},
    ReadChunkRequest, ReadChunkResponse,
    WriteChunkRequest, WriteChunkResponse,
    DeleteChunkRequest, DeleteChunkResponse,
    ChunkExistsRequest, ChunkExistsResponse,
    ListChunksRequest, ListChunksResponse,
    GetStatsRequest, GetStatsResponse,
    ChunkInfo, ServerStats as ProtoServerStats,
};

use crate::server::ChunkServer;
use crate::chunk::ChecksumType;
use bytes::Bytes;

/// gRPC server implementation for ChunkServer
pub struct ChunkServerGrpc {
    server: Arc<ChunkServer>,
}

impl ChunkServerGrpc {
    /// Create a new gRPC server instance
    pub fn new(server: ChunkServer) -> Self {
        Self {
            server: Arc::new(server),
        }
    }
    
    /// Start serving gRPC requests
    pub async fn serve(
        self,
        addr: SocketAddr,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let service = ChunkServerServiceServer::new(self);
        
        Server::builder()
            .add_service(service)
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.recv().await;
                info!("Shutting down gRPC server");
            })
            .await?;
            
        Ok(())
    }
}

#[tonic::async_trait]
impl ChunkServerService for ChunkServerGrpc {
    /// Read a chunk from the server
    async fn read_chunk(
        &self,
        request: Request<ReadChunkRequest>,
    ) -> Result<Response<ReadChunkResponse>, Status> {
        let req = request.into_inner();
        debug!("Reading chunk {} v{}", req.chunk_id, req.version);
        
        match self.server.get_chunk(req.chunk_id, req.version).await {
            Ok(chunk) => {
                let response = ReadChunkResponse {
                    data: chunk.data().to_vec(),
                    checksum: chunk.checksum().to_vec(),
                    checksum_type: chunk.checksum_type() as i32,
                    storage_class_id: chunk.storage_class_id() as u32,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to read chunk: {}", e);
                Err(Status::internal(format!("Failed to read chunk: {}", e)))
            }
        }
    }
    
    /// Write a chunk to the server
    async fn write_chunk(
        &self,
        request: Request<WriteChunkRequest>,
    ) -> Result<Response<WriteChunkResponse>, Status> {
        let req = request.into_inner();
        debug!("Writing chunk {} v{} ({} bytes)", req.chunk_id, req.version, req.data.len());
        
        let checksum_type = match req.checksum_type {
            0 => ChecksumType::Blake3,
            1 => ChecksumType::Crc32,
            _ => return Err(Status::invalid_argument("Invalid checksum type")),
        };
        
        match self.server.store_chunk(
            req.chunk_id,
            req.version,
            Bytes::from(req.data),
            checksum_type,
            req.storage_class_id as u8,
        ).await {
            Ok(_) => {
                let response = WriteChunkResponse {
                    success: true,
                    message: "Chunk stored successfully".to_string(),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to write chunk: {}", e);
                Err(Status::internal(format!("Failed to write chunk: {}", e)))
            }
        }
    }
    
    /// Delete a chunk from the server
    async fn delete_chunk(
        &self,
        request: Request<DeleteChunkRequest>,
    ) -> Result<Response<DeleteChunkResponse>, Status> {
        let req = request.into_inner();
        debug!("Deleting chunk {} v{}", req.chunk_id, req.version);
        
        match self.server.delete_chunk(req.chunk_id, req.version).await {
            Ok(_) => {
                let response = DeleteChunkResponse {
                    success: true,
                    message: "Chunk deleted successfully".to_string(),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to delete chunk: {}", e);
                Err(Status::internal(format!("Failed to delete chunk: {}", e)))
            }
        }
    }
    
    /// Check if a chunk exists
    async fn chunk_exists(
        &self,
        request: Request<ChunkExistsRequest>,
    ) -> Result<Response<ChunkExistsResponse>, Status> {
        let req = request.into_inner();
        
        match self.server.chunk_exists(req.chunk_id, req.version).await {
            Ok(exists) => {
                let response = ChunkExistsResponse { exists };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to check chunk existence: {}", e);
                Err(Status::internal(format!("Failed to check chunk existence: {}", e)))
            }
        }
    }
    
    /// List all chunks on the server
    async fn list_chunks(
        &self,
        _request: Request<ListChunksRequest>,
    ) -> Result<Response<ListChunksResponse>, Status> {
        match self.server.list_chunks().await {
            Ok(chunks) => {
                let chunk_infos = chunks
                    .into_iter()
                    .map(|(id, version)| ChunkInfo {
                        chunk_id: id,
                        version,
                    })
                    .collect();
                    
                let response = ListChunksResponse {
                    chunks: chunk_infos,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to list chunks: {}", e);
                Err(Status::internal(format!("Failed to list chunks: {}", e)))
            }
        }
    }
    
    /// Get server statistics
    async fn get_stats(
        &self,
        _request: Request<GetStatsRequest>,
    ) -> Result<Response<GetStatsResponse>, Status> {
        match self.server.get_stats().await {
            Ok(stats) => {
                let response = GetStatsResponse {
                    stats: Some(ProtoServerStats {
                        uptime_seconds: stats.uptime_seconds,
                        is_running: stats.is_running,
                        active_operations: stats.active_operations,
                        master_connected: stats.master_connected,
                        total_chunks: stats.storage.total_chunks,
                        total_bytes: stats.storage.total_bytes,
                        free_bytes: stats.storage.free_bytes,
                        cache_hit_ratio: stats.cache_hit_ratio,
                        cache_memory_usage_mb: stats.cache_memory_usage_mb,
                        read_ops: stats.metrics.read_ops(),
                        write_ops: stats.metrics.write_ops(),
                        delete_ops: stats.metrics.delete_ops(),
                        checksum_failures: stats.metrics.checksum_failures(),
                    }),
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get stats: {}", e);
                Err(Status::internal(format!("Failed to get stats: {}", e)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::config::ChunkServerConfig;
    
    async fn create_test_grpc_server() -> (ChunkServerGrpc, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ChunkServerConfig::default();
        config.data_dir = temp_dir.path().to_path_buf();
        
        let server = ChunkServer::new(config).await.unwrap();
        let grpc_server = ChunkServerGrpc::new(server);
        
        (grpc_server, temp_dir)
    }
    
    #[tokio::test]
    async fn test_write_and_read_chunk() {
        let (grpc_server, _temp_dir) = create_test_grpc_server().await;
        
        // Write chunk
        let write_req = WriteChunkRequest {
            chunk_id: 12345,
            version: 1,
            data: b"Hello, World!".to_vec(),
            checksum_type: 0, // Blake3
            storage_class_id: 1,
        };
        
        let write_result = grpc_server.write_chunk(Request::new(write_req)).await;
        assert!(write_result.is_ok());
        assert!(write_result.unwrap().into_inner().success);
        
        // Read chunk
        let read_req = ReadChunkRequest {
            chunk_id: 12345,
            version: 1,
        };
        
        let read_result = grpc_server.read_chunk(Request::new(read_req)).await;
        assert!(read_result.is_ok());
        
        let response = read_result.unwrap().into_inner();
        assert_eq!(response.data, b"Hello, World!");
        assert_eq!(response.checksum_type, 0);
        assert_eq!(response.storage_class_id, 1);
    }
    
    #[tokio::test]
    async fn test_chunk_exists() {
        let (grpc_server, _temp_dir) = create_test_grpc_server().await;
        
        // Check non-existent chunk
        let exists_req = ChunkExistsRequest {
            chunk_id: 99999,
            version: 1,
        };
        
        let result = grpc_server.chunk_exists(Request::new(exists_req)).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().into_inner().exists);
        
        // Write chunk
        let write_req = WriteChunkRequest {
            chunk_id: 12345,
            version: 1,
            data: b"test".to_vec(),
            checksum_type: 0,
            storage_class_id: 1,
        };
        grpc_server.write_chunk(Request::new(write_req)).await.unwrap();
        
        // Check existing chunk
        let exists_req = ChunkExistsRequest {
            chunk_id: 12345,
            version: 1,
        };
        
        let result = grpc_server.chunk_exists(Request::new(exists_req)).await;
        assert!(result.is_ok());
        assert!(result.unwrap().into_inner().exists);
    }
    
    #[tokio::test]
    async fn test_delete_chunk() {
        let (grpc_server, _temp_dir) = create_test_grpc_server().await;
        
        // Write chunk
        let write_req = WriteChunkRequest {
            chunk_id: 12345,
            version: 1,
            data: b"test".to_vec(),
            checksum_type: 0,
            storage_class_id: 1,
        };
        grpc_server.write_chunk(Request::new(write_req)).await.unwrap();
        
        // Delete chunk
        let delete_req = DeleteChunkRequest {
            chunk_id: 12345,
            version: 1,
        };
        
        let result = grpc_server.delete_chunk(Request::new(delete_req)).await;
        assert!(result.is_ok());
        assert!(result.unwrap().into_inner().success);
        
        // Verify chunk is gone
        let exists_req = ChunkExistsRequest {
            chunk_id: 12345,
            version: 1,
        };
        
        let result = grpc_server.chunk_exists(Request::new(exists_req)).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().into_inner().exists);
    }
    
    #[tokio::test]
    async fn test_list_chunks() {
        let (grpc_server, _temp_dir) = create_test_grpc_server().await;
        
        // Write multiple chunks
        for i in 1..=3 {
            let write_req = WriteChunkRequest {
                chunk_id: i,
                version: 1,
                data: format!("chunk{}", i).as_bytes().to_vec(),
                checksum_type: 0,
                storage_class_id: 1,
            };
            grpc_server.write_chunk(Request::new(write_req)).await.unwrap();
        }
        
        // List chunks
        let list_req = ListChunksRequest {};
        let result = grpc_server.list_chunks(Request::new(list_req)).await;
        assert!(result.is_ok());
        
        let chunks = result.unwrap().into_inner().chunks;
        assert_eq!(chunks.len(), 3);
    }
    
    #[tokio::test]
    async fn test_get_stats() {
        let (grpc_server, _temp_dir) = create_test_grpc_server().await;
        
        let stats_req = GetStatsRequest {};
        let result = grpc_server.get_stats(Request::new(stats_req)).await;
        assert!(result.is_ok());
        
        let stats = result.unwrap().into_inner().stats.unwrap();
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.read_ops, 0);
        assert_eq!(stats.write_ops, 0);
    }
}