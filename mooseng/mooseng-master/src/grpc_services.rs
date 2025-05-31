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
use tonic::codec::CompressionEncoding;
use tracing::{error, info, warn, instrument};
use futures_util::Stream;

use crate::{
    chunk_manager::ChunkManager,
    filesystem::FileSystem,
    metadata::MetadataStore,
    session::SessionManager,
    storage_class::StorageClassManager,
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
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip)
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
        
        // TODO: Implement actual file creation logic
        // For now, return a placeholder response
        let response = CreateFileResponse {
            metadata: None, // TODO: Create actual metadata
            session_id: self.session_manager.create_simple_session().await.unwrap_or(0),
        };
        
        Ok(Response::new(response))
    }

    #[instrument(skip(self, request))]
    async fn open_file(
        &self,
        request: Request<OpenFileRequest>,
    ) -> Result<Response<OpenFileResponse>, Status> {
        let req = request.into_inner();
        info!("Opening file: {}", req.path);
        
        // TODO: Implement actual file opening logic
        let response = OpenFileResponse {
            metadata: None, // TODO: Get actual metadata
            file_handle: 1, // TODO: Generate real file handle
            chunks: vec![], // TODO: Get actual chunk info
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
        
        // TODO: Implement actual file closing logic
        let response = CloseFileResponse { success: true };
        
        Ok(Response::new(response))
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