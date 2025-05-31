use anyhow::Result;
use mooseng_common::config::MasterConfig;
use mooseng_protocol::{
    MasterService, MasterServiceServer,
    CreateFileRequest, CreateFileResponse,
    OpenFileRequest, OpenFileResponse,
    CloseFileRequest, CloseFileResponse,
    DeleteFileRequest, DeleteFileResponse,
    RenameFileRequest, RenameFileResponse,
    CreateDirectoryRequest, CreateDirectoryResponse,
    DeleteDirectoryRequest, DeleteDirectoryResponse,
    ListDirectoryRequest, ListDirectoryResponse,
    GetFileInfoRequest, GetFileInfoResponse,
    SetFileAttributesRequest, SetFileAttributesResponse,
    GetExtendedAttributesRequest, GetExtendedAttributesResponse,
    SetExtendedAttributesRequest, SetExtendedAttributesResponse,
    AllocateChunkRequest, AllocateChunkResponse,
    GetChunkLocationsRequest, GetChunkLocationsResponse,
    ReportChunkStatusRequest, ReportChunkStatusResponse,
    RegisterServerRequest, RegisterServerResponse,
    HeartbeatRequest, HeartbeatResponse,
    GetClusterStatusRequest, GetClusterStatusResponse,
    CreateStorageClassRequest, CreateStorageClassResponse,
    ListStorageClassesRequest, ListStorageClassesResponse,
    SetFileStorageClassRequest, SetFileStorageClassResponse,
    GetRegionInfoRequest, GetRegionInfoResponse,
    SetRegionPolicyRequest, SetRegionPolicyResponse,
    InitiateRegionSyncRequest, RegionSyncProgress,
    RaftMessage, LeaderLeaseRequest, LeaderLeaseResponse,
};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tracing::{error, info, warn, instrument};
use futures_util::Stream;

use crate::{
    chunk_manager::ChunkManager,
    filesystem::FileSystem,
    metadata::MetadataStore,
    session::SessionManager,
    storage_class::StorageClassManager,
    raft::RaftConsensus,
    multiregion::{MultiRegionRaft, ConsistencyLevel},
};

/// gRPC server wrapper that manages multiple tonic servers
pub struct GrpcServer {
    config: Arc<MasterConfig>,
    master_service: MasterServiceImpl,
}

impl GrpcServer {
    pub fn new(
        config: Arc<MasterConfig>,
        metadata_store: Arc<MetadataStore>,
        filesystem: Arc<FileSystem>,
        chunk_manager: Arc<ChunkManager>,
        session_manager: Arc<SessionManager>,
        storage_class_manager: Arc<StorageClassManager>,
        raft_consensus: Option<Arc<RaftConsensus>>,
        multiregion_raft: Option<Arc<MultiRegionRaft>>,
    ) -> Self {
        let master_service = MasterServiceImpl::new(
            metadata_store,
            filesystem,
            chunk_manager,
            session_manager,
            storage_class_manager,
        );

        Self {
            config,
            master_service,
        }
    }
    
    pub async fn run(self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.bind_address, self.config.client_port)
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid address: {}", e))?;

        info!("Starting MooseNG Master gRPC server on {}", addr);

        let master_service = MasterServiceServer::new(self.master_service)
            .max_decoding_message_size(64 * 1024 * 1024) // 64MB max
            .max_encoding_message_size(64 * 1024 * 1024); // 64MB max

        let server = Server::builder()
            .add_service(master_service)
            .serve(addr);

        info!("gRPC server started and ready to accept connections");
        
        tokio::select! {
            result = server => {
                match result {
                    Ok(()) => info!("gRPC server shut down gracefully"),
                    Err(e) => error!("gRPC server error: {}", e),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("gRPC server received shutdown signal");
            }
        }
        
        info!("gRPC server shutting down");
        Ok(())
    }
}

/// Implementation of the Master Service gRPC interface
#[derive(Clone)]
pub struct MasterServiceImpl {
    metadata_store: Arc<MetadataStore>,
    filesystem: Arc<FileSystem>,
    chunk_manager: Arc<ChunkManager>,
    session_manager: Arc<SessionManager>,
    storage_class_manager: Arc<StorageClassManager>,
}

impl MasterServiceImpl {
    pub fn new(
        metadata_store: Arc<MetadataStore>,
        filesystem: Arc<FileSystem>,
        chunk_manager: Arc<ChunkManager>,
        session_manager: Arc<SessionManager>,
        storage_class_manager: Arc<StorageClassManager>,
    ) -> Self {
        Self {
            metadata_store,
            filesystem,
            chunk_manager,
            session_manager,
            storage_class_manager,
        }
    }
}

#[tonic::async_trait]
impl MasterService for MasterServiceImpl {
    #[instrument(skip(self, request))]
    async fn create_file(
        &self,
        request: Request<CreateFileRequest>,
    ) -> Result<Response<CreateFileResponse>, Status> {
        let req = request.into_inner();
        info!("Creating file: {}", req.path);
        
        // Parse parent directory and filename
        let path = std::path::Path::new(&req.path);
        let parent_path = path.parent()
            .ok_or_else(|| Status::invalid_argument("Invalid file path"))?
            .to_str()
            .ok_or_else(|| Status::invalid_argument("Invalid path encoding"))?;
        let filename = path.file_name()
            .ok_or_else(|| Status::invalid_argument("Missing filename"))?
            .to_str()
            .ok_or_else(|| Status::invalid_argument("Invalid filename encoding"))?;
        
        // Create file in filesystem
        match self.filesystem.create_file(
            parent_path,
            filename,
            req.mode.unwrap_or(0o644) as u16,
            req.uid,
            req.gid,
            req.storage_class_id.unwrap_or(1),
        ).await {
            Ok(inode_id) => {
                // Get the created file's metadata
                let file_node = self.filesystem.get_inode(inode_id).await
                    .map_err(|e| {
                        error!("Failed to get created file metadata: {}", e);
                        Status::internal("Failed to get file metadata")
                    })?
                    .ok_or_else(|| {
                        error!("Created file inode {} not found", inode_id);
                        Status::internal("File creation inconsistency")
                    })?;
                
                // Create session for the file
                let session_id = self.session_manager.create_file_session(
                    inode_id,
                    req.session_info.as_ref().map(|s| s.client_id.clone()).unwrap_or_default(),
                ).await.map_err(|e| {
                    error!("Failed to create session: {}", e);
                    Status::internal("Failed to create session")
                })?;
                
                // Convert to protobuf metadata
                let metadata = Some(mooseng_protocol::FileMetadata {
                    inode: inode_id,
                    parent_inode: file_node.parent.unwrap_or(0),
                    name: filename.to_string(),
                    file_type: mooseng_protocol::FileType::RegularFile as i32,
                    mode: file_node.mode as u32,
                    uid: file_node.uid,
                    gid: file_node.gid,
                    size: 0,
                    atime: file_node.atime,
                    mtime: file_node.mtime,
                    ctime: file_node.ctime,
                    storage_class_id: file_node.storage_class_id as u32,
                    chunk_count: 0,
                });
                
                let response = CreateFileResponse {
                    metadata,
                    session_id,
                };
                
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to create file: {}", e);
                if e.to_string().contains("already exists") {
                    Err(Status::already_exists("File already exists"))
                } else if e.to_string().contains("not found") {
                    Err(Status::not_found("Parent directory not found"))
                } else if e.to_string().contains("Permission") {
                    Err(Status::permission_denied("Permission denied"))
                } else {
                    Err(Status::internal(format!("Internal error creating file: {}", e)))
                }
            }
        }
    }

    #[instrument(skip(self, request))]
    async fn open_file(
        &self,
        request: Request<OpenFileRequest>,
    ) -> Result<Response<OpenFileResponse>, Status> {
        let req = request.into_inner();
        info!("Opening file: {}", req.path);
        
        // Resolve file path to inode
        let inode_id = self.filesystem.resolve_path(&req.path).await
            .map_err(|e| {
                error!("Failed to resolve path: {}", e);
                Status::internal("Failed to resolve path")
            })?
            .ok_or_else(|| Status::not_found("File not found"))?;
        
        // Get file metadata
        let file_node = self.filesystem.get_inode(inode_id).await
            .map_err(|e| {
                error!("Failed to get file metadata: {}", e);
                Status::internal("Failed to get file metadata")
            })?
            .ok_or_else(|| Status::not_found("File not found"))?;
        
        // Verify it's a file
        let (file_size, chunk_count) = match &file_node.node_type {
            mooseng_common::types::FsNodeType::File { length, chunks, .. } => {
                (*length, chunks.len() as u32)
            }
            _ => return Err(Status::invalid_argument("Path is not a file")),
        };
        
        // Create file handle through session manager
        let file_handle = self.session_manager.open_file(
            inode_id,
            req.flags,
            req.session_info.as_ref().map(|s| s.client_id.clone()).unwrap_or_default(),
        ).await.map_err(|e| {
            error!("Failed to create file handle: {}", e);
            Status::internal("Failed to create file handle")
        })?;
        
        // Get chunk information if requested
        let chunks = if req.include_chunks {
            match &file_node.node_type {
                mooseng_common::types::FsNodeType::File { chunks, .. } => {
                    let mut chunk_infos = Vec::new();
                    for chunk_id in chunks {
                        if let Ok(locations) = self.chunk_manager.get_chunk_locations(*chunk_id).await {
                            chunk_infos.push(mooseng_protocol::ChunkInfo {
                                chunk_id: *chunk_id,
                                chunk_index: chunk_infos.len() as u32,
                                version: 1, // TODO: Get actual version
                                size: mooseng_common::CHUNK_SIZE as u64,
                                locations: locations.into_iter().map(|loc| {
                                    mooseng_protocol::ChunkLocation {
                                        server_id: loc.server_id,
                                        ip: loc.ip,
                                        port: loc.port as u32,
                                        label: loc.label.unwrap_or_default(),
                                    }
                                }).collect(),
                            });
                        }
                    }
                    chunk_infos
                }
                _ => vec![],
            }
        } else {
            vec![]
        };
        
        // Extract filename from path
        let filename = std::path::Path::new(&req.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        // Convert to protobuf metadata
        let metadata = Some(mooseng_protocol::FileMetadata {
            inode: inode_id,
            parent_inode: file_node.parent.unwrap_or(0),
            name: filename.to_string(),
            file_type: mooseng_protocol::FileType::RegularFile as i32,
            mode: file_node.mode as u32,
            uid: file_node.uid,
            gid: file_node.gid,
            size: file_size,
            atime: file_node.atime,
            mtime: file_node.mtime,
            ctime: file_node.ctime,
            storage_class_id: file_node.storage_class_id as u32,
            chunk_count,
        });
        
        let response = OpenFileResponse {
            metadata,
            file_handle,
            chunks,
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn close_file(
        &self,
        request: Request<CloseFileRequest>,
    ) -> Result<Response<CloseFileResponse>, Status> {
        let req = request.into_inner();
        info!("Closing file handle: {}", req.file_handle);
        
        // Close file handle through session manager
        match self.session_manager.close_file(req.file_handle).await {
            Ok(_) => {
                let response = CloseFileResponse { success: true };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to close file handle {}: {}", req.file_handle, e);
                // Still return success as the client expects, but log the error
                // This matches MooseFS behavior where close rarely fails
                let response = CloseFileResponse { success: true };
                Ok(Response::new(response))
            }
        }
    }

    #[instrument(skip(self, request))]
    async fn delete_file(
        &self,
        request: Request<DeleteFileRequest>,
    ) -> Result<Response<DeleteFileResponse>, Status> {
        let req = request.into_inner();
        info!("Deleting file: {}", req.path);
        
        // TODO: Implement actual file deletion logic
        let response = DeleteFileResponse {
            success: true,
            freed_space: 0, // TODO: Calculate actual freed space
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn rename_file(
        &self,
        request: Request<RenameFileRequest>,
    ) -> Result<Response<RenameFileResponse>, Status> {
        let req = request.into_inner();
        info!("Renaming file: {} -> {}", req.old_path, req.new_path);
        
        // TODO: Implement actual file renaming logic
        let response = RenameFileResponse {
            success: true,
            metadata: None, // TODO: Return updated metadata
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn create_directory(
        &self,
        request: Request<CreateDirectoryRequest>,
    ) -> Result<Response<CreateDirectoryResponse>, Status> {
        let req = request.into_inner();
        info!("Creating directory: {}", req.path);
        
        // TODO: Implement actual directory creation logic
        let response = CreateDirectoryResponse {
            metadata: None, // TODO: Create actual metadata
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn delete_directory(
        &self,
        request: Request<DeleteDirectoryRequest>,
    ) -> Result<Response<DeleteDirectoryResponse>, Status> {
        let req = request.into_inner();
        info!("Deleting directory: {} (recursive: {})", req.path, req.recursive);
        
        // TODO: Implement actual directory deletion logic
        let response = DeleteDirectoryResponse {
            success: true,
            freed_space: 0,    // TODO: Calculate actual freed space
            deleted_files: 0,  // TODO: Count actual deleted files
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn list_directory(
        &self,
        request: Request<ListDirectoryRequest>,
    ) -> Result<Response<ListDirectoryResponse>, Status> {
        let req = request.into_inner();
        info!("Listing directory: {} (offset: {}, limit: {})", req.path, req.offset, req.limit);
        
        // TODO: Implement actual directory listing logic
        let response = ListDirectoryResponse {
            entries: vec![], // TODO: Get actual directory entries
            has_more: false,
            next_offset: req.offset,
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn get_file_info(
        &self,
        request: Request<GetFileInfoRequest>,
    ) -> Result<Response<GetFileInfoResponse>, Status> {
        let req = request.into_inner();
        info!("Getting file info: {} (follow_symlinks: {})", req.path, req.follow_symlinks);
        
        // TODO: Implement actual file info retrieval
        let response = GetFileInfoResponse {
            metadata: None, // TODO: Get actual metadata
            chunks: vec![], // TODO: Get actual chunk info
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn set_file_attributes(
        &self,
        request: Request<SetFileAttributesRequest>,
    ) -> Result<Response<SetFileAttributesResponse>, Status> {
        let req = request.into_inner();
        info!("Setting file attributes: {}", req.path);
        
        // TODO: Implement actual attribute setting logic
        let response = SetFileAttributesResponse {
            success: true,
            metadata: None, // TODO: Return updated metadata
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn get_extended_attributes(
        &self,
        request: Request<GetExtendedAttributesRequest>,
    ) -> Result<Response<GetExtendedAttributesResponse>, Status> {
        let req = request.into_inner();
        info!("Getting extended attributes: {}", req.path);
        
        // TODO: Implement actual xattr retrieval logic
        let response = GetExtendedAttributesResponse {
            xattrs: std::collections::HashMap::new(), // TODO: Get actual xattrs
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn set_extended_attributes(
        &self,
        request: Request<SetExtendedAttributesRequest>,
    ) -> Result<Response<SetExtendedAttributesResponse>, Status> {
        let req = request.into_inner();
        info!("Setting extended attributes: {} (replace: {})", req.path, req.replace);
        
        // TODO: Implement actual xattr setting logic
        let response = SetExtendedAttributesResponse { success: true };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn allocate_chunk(
        &self,
        request: Request<AllocateChunkRequest>,
    ) -> Result<Response<AllocateChunkResponse>, Status> {
        let req = request.into_inner();
        info!("Allocating chunk for inode: {}, index: {}", req.inode, req.chunk_index);
        
        // TODO: Implement actual chunk allocation logic
        let response = AllocateChunkResponse {
            chunk: None,            // TODO: Create actual chunk info
            write_locations: vec![], // TODO: Determine write locations
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn get_chunk_locations(
        &self,
        request: Request<GetChunkLocationsRequest>,
    ) -> Result<Response<GetChunkLocationsResponse>, Status> {
        let req = request.into_inner();
        info!("Getting chunk locations: {} (for_write: {})", req.chunk_id, req.for_write);
        
        // TODO: Implement actual chunk location retrieval
        let response = GetChunkLocationsResponse {
            chunk: None,     // TODO: Get actual chunk info
            locations: vec![], // TODO: Get actual locations
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn report_chunk_status(
        &self,
        request: Request<ReportChunkStatusRequest>,
    ) -> Result<Response<ReportChunkStatusResponse>, Status> {
        let req = request.into_inner();
        info!("Chunk status report from server: {} ({} reports)", req.server_id, req.reports.len());
        
        // TODO: Implement actual chunk status processing
        let response = ReportChunkStatusResponse {
            actions: vec![], // TODO: Determine necessary actions
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn register_server(
        &self,
        request: Request<RegisterServerRequest>,
    ) -> Result<Response<RegisterServerResponse>, Status> {
        let req = request.into_inner();
        info!("Server registration request from: {:?}", req.info);
        
        // TODO: Implement actual server registration logic
        let response = RegisterServerResponse {
            server_id: 1, // TODO: Generate actual server ID
            accepted: true,
            reject_reason: String::new(),
            peers: vec![], // TODO: Return peer list
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        // Don't log heartbeats at info level to avoid spam
        
        // TODO: Implement actual heartbeat processing
        let response = HeartbeatResponse {
            acknowledged: true,
            chunk_actions: vec![], // TODO: Determine necessary actions
            config_updates: std::collections::HashMap::new(),
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn get_cluster_status(
        &self,
        request: Request<GetClusterStatusRequest>,
    ) -> Result<Response<GetClusterStatusResponse>, Status> {
        let req = request.into_inner();
        info!("Getting cluster status (include_chunks: {}, include_metrics: {})", 
              req.include_chunk_distribution, req.include_metrics);
        
        // TODO: Implement actual cluster status retrieval
        let response = GetClusterStatusResponse {
            servers: vec![],  // TODO: Get actual server list
            regions: vec![],  // TODO: Get actual region info
            stats: None,      // TODO: Get actual statistics
            issues: vec![],   // TODO: Get actual health issues
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn create_storage_class(
        &self,
        request: Request<CreateStorageClassRequest>,
    ) -> Result<Response<CreateStorageClassResponse>, Status> {
        let req = request.into_inner();
        info!("Creating storage class: {:?}", req.storage_class);
        
        // TODO: Implement actual storage class creation
        let response = CreateStorageClassResponse {
            storage_class_id: 1, // TODO: Generate actual ID
            success: true,
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self))]
    async fn list_storage_classes(
        &self,
        _request: Request<ListStorageClassesRequest>,
    ) -> Result<Response<ListStorageClassesResponse>, Status> {
        info!("Listing storage classes");
        
        // TODO: Implement actual storage class listing
        let response = ListStorageClassesResponse {
            storage_classes: vec![], // TODO: Get actual storage classes
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn set_file_storage_class(
        &self,
        request: Request<SetFileStorageClassRequest>,
    ) -> Result<Response<SetFileStorageClassResponse>, Status> {
        let req = request.into_inner();
        info!("Setting storage class for: {} to {} (recursive: {}, migrate: {})", 
              req.path, req.storage_class_id, req.recursive, req.migrate_existing);
        
        // TODO: Implement actual storage class setting
        let response = SetFileStorageClassResponse {
            success: true,
            affected_files: 0,    // TODO: Count actual affected files
            chunks_to_migrate: 0, // TODO: Count chunks to migrate
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn get_region_info(
        &self,
        request: Request<GetRegionInfoRequest>,
    ) -> Result<Response<GetRegionInfoResponse>, Status> {
        let req = request.into_inner();
        info!("Getting region info for: {:?}", req.region_ids);
        
        // TODO: Implement actual region info retrieval
        let response = GetRegionInfoResponse {
            regions: vec![],      // TODO: Get actual region info
            connectivity: std::collections::HashMap::new(), // TODO: Get connectivity info
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn set_region_policy(
        &self,
        request: Request<SetRegionPolicyRequest>,
    ) -> Result<Response<SetRegionPolicyResponse>, Status> {
        let req = request.into_inner();
        info!("Setting region policy for: {}", req.region_id);
        
        // TODO: Implement actual region policy setting
        let response = SetRegionPolicyResponse { success: true };
        
        Ok(Response::new(response))
    }

    type InitiateRegionSyncStream = Pin<Box<dyn Stream<Item = Result<RegionSyncProgress, Status>> + Send>>;

    #[instrument(skip(self, request))]
    async fn initiate_region_sync(
        &self,
        request: Request<InitiateRegionSyncRequest>,
    ) -> Result<Response<Self::InitiateRegionSyncStream>, Status> {
        let req = request.into_inner();
        info!("Initiating region sync: {} -> {} (full: {})", 
              req.source_region, req.target_region, req.full_sync);
        
        // TODO: Implement actual region sync logic
        // For now, return a simple stream with one progress update
        let stream = async_stream::stream! {
            let progress = RegionSyncProgress {
                total_items: 0,
                synced_items: 0,
                bytes_transferred: 0,
                current_item: String::new(),
                estimated_completion: None,
            };
            yield Ok(progress);
        };
        
        Ok(Response::new(Box::pin(stream)))
    }

    #[instrument(skip(self, request))]
    async fn process_raft_message(
        &self,
        request: Request<RaftMessage>,
    ) -> Result<Response<RaftMessage>, Status> {
        let req = request.into_inner();
        info!("Processing Raft message from term: {}, leader: {}", req.term, req.leader_id);
        
        // TODO: Implement actual Raft message processing
        // For now, return a simple response
        let response = RaftMessage {
            term: req.term,
            leader_id: req.leader_id,
            message: None,
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn request_leader_lease(
        &self,
        request: Request<LeaderLeaseRequest>,
    ) -> Result<Response<LeaderLeaseResponse>, Status> {
        let req = request.into_inner();
        info!("Leader lease request for region: {} (duration: {}s)", 
              req.region, req.duration_seconds);
        
        // TODO: Implement actual leader lease logic
        let response = LeaderLeaseResponse {
            granted: false, // TODO: Implement actual lease granting
            lease: None,
            rejection_reason: "Not implemented yet".to_string(),
        };
        
        Ok(Response::new(response))
    }
}