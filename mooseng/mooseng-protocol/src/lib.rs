// Include all generated protobuf code in one module since they share the same package
tonic::include_proto!("mooseng.protocol");

// Re-export service types for convenience
pub use master_service_client::MasterServiceClient;
pub use master_service_server::{MasterService, MasterServiceServer};
pub use chunk_server_service_client::ChunkServerServiceClient;
pub use chunk_server_service_server::{ChunkServerService, ChunkServerServiceServer};
pub use client_service_client::ClientServiceClient;
pub use client_service_server::{ClientService, ClientServiceServer};