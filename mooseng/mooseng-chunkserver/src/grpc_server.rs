//! gRPC server implementation for ChunkServer
//! 
//! This module provides the gRPC interface for external communication
//! with the chunk server, implementing the ChunkServer service.

use std::sync::Arc;
use std::net::SocketAddr;
use std::pin::Pin;
use tokio::sync::{broadcast, mpsc};
use futures::stream::{Stream, StreamExt};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tracing::{error, info, debug};

use mooseng_protocol::{
    chunk_server_service_server::{ChunkServerService, ChunkServerServiceServer},
    ReadChunkRequest, ReadChunkResponse,
    WriteChunkRequest, WriteChunkResponse,
    DeleteChunkRequest, DeleteChunkResponse,
    ListChunksRequest, ListChunksResponse, ChunkDetail,
    ServerStatusResponse, ServerInfo, DiskInfo, DiskStatus, ServerCapabilities,
    VerifyChunkRequest, VerifyChunkResponse,
    GetChunkInfoRequest, GetChunkInfoResponse,
    ReplicateChunkRequest, ReplicateChunkResponse,
    EncodeChunkRequest, EncodeChunkResponse,
    DecodeChunkRequest, DecodeChunkResponse,
    StoreEcShardsRequest, StoreEcShardsResponse,
    GetEcShardsRequest, GetEcShardsResponse,
    InitiateScrubRequest, ScrubProgress,
    XRegionReplicationRequest, XRegionReplicationProgress,
    XRegionDataTransfer, XRegionDataResponse,
    write_chunk_request,
};
use prost_types;

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

type ResponseStream<T> = Pin<Box<dyn futures::Stream<Item = Result<T, Status>> + Send>>;

#[tonic::async_trait]
impl ChunkServerService for ChunkServerGrpc {
    type ReadChunkStream = ResponseStream<ReadChunkResponse>;
    type DecodeChunkStream = ResponseStream<DecodeChunkResponse>;
    type GetECShardsStream = ResponseStream<GetEcShardsResponse>;
    type InitiateScrubStream = ResponseStream<ScrubProgress>;
    type InitiateXRegionReplicationStream = ResponseStream<XRegionReplicationProgress>;
    
    /// Read a chunk from the server
    async fn read_chunk(
        &self,
        request: Request<ReadChunkRequest>,
    ) -> Result<Response<Self::ReadChunkStream>, Status> {
        let req = request.into_inner();
        debug!("Reading chunk {} v{}", req.chunk_id, req.version);
        
        let chunk_id = req.chunk_id;
        let version = req.version;
        let offset = req.offset;
        let length = req.length;
        
        let server = self.server.clone();
        
        let (tx, rx) = mpsc::channel(10);
        
        tokio::spawn(async move {
            match server.get_chunk(chunk_id, version).await {
                Ok(chunk) => {
                    let data = chunk.data();
                    let total_size = data.len() as u64;
                    
                    // Calculate actual read range
                    let start = offset.min(total_size) as usize;
                    let end = if length == 0 {
                        total_size as usize
                    } else {
                        ((offset + length).min(total_size)) as usize
                    };
                    
                    // Send data in chunks
                    const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks
                    let mut current_offset = start;
                    
                    while current_offset < end {
                        let chunk_end = (current_offset + CHUNK_SIZE).min(end);
                        let is_last = chunk_end == end;
                        
                        let response = ReadChunkResponse {
                            data: data[current_offset..chunk_end].to_vec().into(),
                            offset: current_offset as u64,
                            is_last,
                            chunk_checksum: if is_last && req.verify_checksum {
                                chunk.checksum().value.clone().into()
                            } else {
                                bytes::Bytes::new()
                            },
                        };
                        
                        if tx.send(Ok(response)).await.is_err() {
                            break;
                        }
                        
                        current_offset = chunk_end;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(Status::internal(format!("Failed to read chunk: {}", e)))).await;
                }
            }
        });
        
        let stream = futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        });
        Ok(Response::new(Box::pin(stream) as Self::ReadChunkStream))
    }
    
    /// Write a chunk to the server
    async fn write_chunk(
        &self,
        request: Request<Streaming<WriteChunkRequest>>,
    ) -> Result<Response<WriteChunkResponse>, Status> {
        let mut stream = request.into_inner();
        
        // First message should be header
        let header = match stream.next().await {
            Some(Ok(req)) => match req.request {
                Some(write_chunk_request::Request::Header(h)) => h,
                _ => return Err(Status::invalid_argument("First message must be header")),
            },
            Some(Err(e)) => return Err(e),
            None => return Err(Status::invalid_argument("Empty request stream")),
        };
        
        debug!("Writing chunk {} v{} ({} bytes)", header.chunk_id, header.version, header.total_size);
        
        // Collect all data
        let mut data = Vec::with_capacity(header.total_size as usize);
        
        while let Some(msg) = stream.next().await {
            match msg? {
                WriteChunkRequest { 
                    request: Some(write_chunk_request::Request::Data(chunk_data)) 
                } => {
                    data.extend_from_slice(&chunk_data.data);
                    if chunk_data.is_last {
                        break;
                    }
                }
                _ => return Err(Status::invalid_argument("Expected data message")),
            }
        }
        
        // Validate data size
        if data.len() != header.total_size as usize {
            return Err(Status::invalid_argument(format!(
                "Data size mismatch: expected {}, got {}",
                header.total_size,
                data.len()
            )));
        }

        // Store chunk
        match self.server.store_chunk(
            header.chunk_id,
            header.version,
            Bytes::from(data),
            ChecksumType::Blake3, // Default for now
            header.storage_class_id as u8,
        ).await {
            Ok(chunk) => {
                let response = WriteChunkResponse {
                    success: true,
                    version: header.version,
                    final_checksum: chunk.checksum().value.clone().into(),
                    chain_status: vec![],
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to write chunk {}: {}", header.chunk_id, e);
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
                    freed_bytes: 0, // TODO: Track freed bytes
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to delete chunk: {}", e);
                Err(Status::internal(format!("Failed to delete chunk: {}", e)))
            }
        }
    }
    
    /// Replicate a chunk from another server
    async fn replicate_chunk(
        &self,
        _request: Request<Streaming<ReplicateChunkRequest>>,
    ) -> Result<Response<ReplicateChunkResponse>, Status> {
        // TODO: Implement chunk replication
        Err(Status::unimplemented("Chunk replication not yet implemented"))
    }
    
    /// Verify chunk integrity
    async fn verify_chunk(
        &self,
        request: Request<VerifyChunkRequest>,
    ) -> Result<Response<VerifyChunkResponse>, Status> {
        let req = request.into_inner();
        debug!("Verifying chunk {} v{}", req.chunk_id, req.version);
        
        match self.server.verify_chunk(req.chunk_id, req.version).await {
            Ok(valid) => {
                // Try to get chunk details for additional info
                let (size, checksum) = if valid {
                    match self.server.get_chunk(req.chunk_id, req.version).await {
                        Ok(chunk) => (
                            chunk.data().len() as u64,
                            chunk.checksum().value.clone().into()
                        ),
                        Err(_) => (0, bytes::Bytes::new()),
                    }
                } else {
                    (0, bytes::Bytes::new())
                };

                let response = VerifyChunkResponse {
                    valid,
                    version: req.version,
                    size,
                    checksum,
                    corruptions: vec![], // TODO: Add detailed corruption info when available
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to verify chunk {} v{}: {}", req.chunk_id, req.version, e);
                Err(Status::internal(format!("Failed to verify chunk: {}", e)))
            }
        }
    }
    
    /// Get chunk information
    async fn get_chunk_info(
        &self,
        request: Request<GetChunkInfoRequest>,
    ) -> Result<Response<GetChunkInfoResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting chunk info for chunk_ids: {:?}", req.chunk_ids);
        
        let mut chunks = Vec::new();
        
        for chunk_id in req.chunk_ids {
            match self.server.get_chunk(chunk_id, 1).await { // Use default version 1
                Ok(chunk) => {
                    let metadata = &chunk.metadata;
                    let chunk_detail = ChunkDetail {
                        chunk_id,
                        version: chunk.version(),
                        size: chunk.data().len() as u64,
                        checksum: chunk.checksum().value.clone().into(),
                        created: Some(prost_types::Timestamp {
                            seconds: (metadata.created_at / 1_000_000) as i64,
                            nanos: ((metadata.created_at % 1_000_000) * 1000) as i32,
                        }),
                        modified: Some(prost_types::Timestamp {
                            seconds: (metadata.last_modified / 1_000_000) as i64,
                            nanos: ((metadata.last_modified % 1_000_000) * 1000) as i32,
                        }),
                        accessed: Some(prost_types::Timestamp {
                            seconds: (metadata.last_accessed / 1_000_000) as i64,
                            nanos: ((metadata.last_accessed % 1_000_000) * 1000) as i32,
                        }),
                        access_count: metadata.access_count as u32,
                        status: 1, // ChunkStatus::CHUNK_STATUS_HEALTHY
                    };
                    chunks.push(chunk_detail);
                }
                Err(e) => {
                    error!("Failed to get chunk info for {}: {}", chunk_id, e);
                    // Continue with other chunks instead of failing entirely
                }
            }
        }
        
        let response = GetChunkInfoResponse { chunks };
        Ok(Response::new(response))
    }
    
    /// Encode chunk for erasure coding
    async fn encode_chunk(
        &self,
        _request: Request<EncodeChunkRequest>,
    ) -> Result<Response<EncodeChunkResponse>, Status> {
        // TODO: Implement erasure coding
        Err(Status::unimplemented("Erasure coding not yet implemented"))
    }
    
    /// Decode chunk from erasure coded shards
    async fn decode_chunk(
        &self,
        _request: Request<DecodeChunkRequest>,
    ) -> Result<Response<Pin<Box<dyn Stream<Item = Result<DecodeChunkResponse, Status>> + Send>>>, Status> {
        // TODO: Implement erasure decoding
        Err(Status::unimplemented("Erasure decoding not yet implemented"))
    }
    
    /// Store erasure coded shards
    async fn store_ec_shards(
        &self,
        _request: Request<Streaming<StoreEcShardsRequest>>,
    ) -> Result<Response<StoreEcShardsResponse>, Status> {
        // TODO: Implement EC shard storage
        Err(Status::unimplemented("EC shard storage not yet implemented"))
    }
    
    /// Get erasure coded shards
    async fn get_ec_shards(
        &self,
        _request: Request<GetEcShardsRequest>,
    ) -> Result<Response<Self::GetECShardsStream>, Status> {
        // TODO: Implement EC shard retrieval
        Err(Status::unimplemented("EC shard retrieval not yet implemented"))
    }
    
    /// Get server status
    async fn get_server_status(
        &self,
        _request: Request<()>,
    ) -> Result<Response<ServerStatusResponse>, Status> {
        match self.server.get_stats().await {
            Ok(stats) => {
                let response = ServerStatusResponse {
                    info: Some(ServerInfo {
                        server_id: self.server.server_id() as u64,
                        r#type: mooseng_protocol::ServerType::Chunk as i32,
                        hostname: "localhost".to_string(), // TODO: Get actual hostname
                        ip_address: self.server.config().bind_address.clone(),
                        port: self.server.config().port as u32,
                        region: "default".to_string(), // TODO: Get from config
                        rack_id: self.server.config().rack_id as u32,
                        status: mooseng_protocol::ServerStatus::Online as i32,
                        last_heartbeat: None, // TODO: Track last heartbeat
                        capabilities: Some(ServerCapabilities {
                            total_space: stats.storage.free_space_bytes + stats.storage.total_bytes,
                            used_space: stats.storage.total_bytes,
                            max_chunks: 1000000, // TODO: Get from config
                            current_chunks: stats.storage.total_chunks as u32,
                            supports_erasure_coding: true,
                            supported_ec_schemes: vec![
                                mooseng_protocol::ErasureCodingScheme::EcSchemeRs42 as i32,
                                mooseng_protocol::ErasureCodingScheme::EcSchemeRs84 as i32,
                            ],
                            network_bandwidth_mbps: 1000, // TODO: Get from config
                            disk_iops: 10000, // TODO: Measure actual IOPS
                        }),
                    }),
                    capabilities: Some(ServerCapabilities {
                        total_space: stats.storage.free_space_bytes + stats.storage.total_bytes,
                        used_space: stats.storage.total_bytes,
                        max_chunks: 1000000, // TODO: Get from config
                        current_chunks: stats.storage.total_chunks as u32,
                        supports_erasure_coding: true,
                        supported_ec_schemes: vec![
                            mooseng_protocol::ErasureCodingScheme::EcSchemeRs42 as i32,
                            mooseng_protocol::ErasureCodingScheme::EcSchemeRs84 as i32,
                        ],
                        network_bandwidth_mbps: 1000, // TODO: Get from config
                        disk_iops: 10000, // TODO: Measure actual IOPS
                    }),
                    disks: vec![DiskInfo {
                        path: self.server.config().data_dir.to_string_lossy().to_string(),
                        total_space: stats.storage.free_space_bytes + stats.storage.total_bytes,
                        used_space: stats.storage.total_bytes,
                        chunk_count: stats.storage.total_chunks,
                        status: DiskStatus::Healthy as i32,
                        io_stats: Default::default(),
                    }],
                    metrics: Default::default(), // TODO: Add metrics
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get server status: {}", e);
                Err(Status::internal(format!("Failed to get server status: {}", e)))
            }
        }
    }
    
    /// List chunks on the server
    async fn list_chunks(
        &self,
        request: Request<ListChunksRequest>,
    ) -> Result<Response<ListChunksResponse>, Status> {
        let req = request.into_inner();
        
        match self.server.list_chunks().await {
            Ok(chunks) => {
                // Apply pagination
                let offset = req.offset as usize;
                let limit = if req.limit == 0 { 1000 } else { req.limit } as usize;
                
                let total_count = chunks.len();
                let end = (offset + limit).min(total_count);
                
                let mut chunk_details = Vec::new();
                for (id, version) in &chunks[offset..end] {
                    // Try to get chunk details, fallback to basic info if not available
                    let detail = match self.server.get_chunk(*id, *version).await {
                        Ok(chunk) => {
                            let metadata = &chunk.metadata;
                            ChunkDetail {
                                chunk_id: *id,
                                version: *version,
                                size: chunk.data().len() as u64,
                                checksum: chunk.checksum().value.clone().into(),
                                created: Some(prost_types::Timestamp {
                                    seconds: (metadata.created_at / 1_000_000) as i64,
                                    nanos: ((metadata.created_at % 1_000_000) * 1000) as i32,
                                }),
                                modified: Some(prost_types::Timestamp {
                                    seconds: (metadata.last_modified / 1_000_000) as i64,
                                    nanos: ((metadata.last_modified % 1_000_000) * 1000) as i32,
                                }),
                                accessed: Some(prost_types::Timestamp {
                                    seconds: (metadata.last_accessed / 1_000_000) as i64,
                                    nanos: ((metadata.last_accessed % 1_000_000) * 1000) as i32,
                                }),
                                access_count: metadata.access_count as u32,
                                status: 0, // TODO: Define status codes
                            }
                        }
                        Err(_) => {
                            // Fallback for chunks that can't be loaded
                            ChunkDetail {
                                chunk_id: *id,
                                version: *version,
                                size: 0,
                                checksum: bytes::Bytes::new(),
                                created: None,
                                modified: None,
                                accessed: None,
                                access_count: 0,
                                status: 1, // Mark as potentially problematic
                            }
                        }
                    };
                    chunk_details.push(detail);
                }
                
                let response = ListChunksResponse {
                    chunks: chunk_details,
                    has_more: end < total_count,
                    next_offset: end as u64,
                    total_count: total_count as u64,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to list chunks: {}", e);
                Err(Status::internal(format!("Failed to list chunks: {}", e)))
            }
        }
    }
    
    /// Initiate chunk scrubbing
    async fn initiate_scrub(
        &self,
        _request: Request<InitiateScrubRequest>,
    ) -> Result<Response<Pin<Box<dyn Stream<Item = Result<ScrubProgress, Status>> + Send>>>, Status> {
        // TODO: Implement chunk scrubbing
        Err(Status::unimplemented("Chunk scrubbing not yet implemented"))
    }
    
    /// Initiate cross-region replication
    async fn initiate_x_region_replication(
        &self,
        _request: Request<XRegionReplicationRequest>,
    ) -> Result<Response<Pin<Box<dyn Stream<Item = Result<XRegionReplicationProgress, Status>> + Send>>>, Status> {
        // TODO: Implement cross-region replication
        Err(Status::unimplemented("Cross-region replication not yet implemented"))
    }
    
    /// Receive cross-region data transfer
    async fn receive_x_region_data(
        &self,
        _request: Request<Streaming<XRegionDataTransfer>>,
    ) -> Result<Response<XRegionDataResponse>, Status> {
        // TODO: Implement cross-region data reception
        Err(Status::unimplemented("Cross-region data reception not yet implemented"))
    }
}