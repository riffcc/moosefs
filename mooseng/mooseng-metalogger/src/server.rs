use anyhow::{Result, Context};
use bytes::Bytes;
use mooseng_common::ShutdownCoordinator;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, info, error};

use crate::config::MetaloggerConfig;
use crate::replication::{ReplicationClient, MetadataChange};
use crate::wal::WalWriter;
use crate::snapshot::SnapshotManager;
use crate::recovery::RecoveryManager;

/// Metalogger server that coordinates all components
pub struct MetaloggerServer {
    config: MetaloggerConfig,
    shutdown: ShutdownCoordinator,
    
    // Core components
    wal_writer: Arc<WalWriter>,
    snapshot_manager: Arc<SnapshotManager>,
    recovery_manager: Arc<RecoveryManager>,
    replication_client: Arc<RwLock<ReplicationClient>>,
    
    // Current state
    metadata_state: Arc<RwLock<HashMap<String, Bytes>>>,
    server_state: Arc<RwLock<ServerState>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ServerState {
    Initializing,
    Recovering,
    Syncing,
    Running,
    Stopping,
}

impl MetaloggerServer {
    pub async fn new(config: MetaloggerConfig, shutdown: ShutdownCoordinator) -> Result<Self> {
        info!("Initializing Metalogger server");
        
        let data_dir = std::path::PathBuf::from(&config.storage.data_dir);
        
        // Create data directory if it doesn't exist
        tokio::fs::create_dir_all(&data_dir).await
            .context("Failed to create data directory")?;
        
        // Initialize components
        let wal_writer = Arc::new(WalWriter::new(data_dir.clone()).await?);
        
        let snapshot_manager = Arc::new(
            SnapshotManager::new(data_dir.clone(), config.snapshot.clone()).await?
        );
        
        let recovery_manager = Arc::new(
            RecoveryManager::new(data_dir.clone(), snapshot_manager.clone())
        );
        
        let replication_client = Arc::new(RwLock::new(
            ReplicationClient::new(
                config.replication.clone(),
                wal_writer.clone()
            )?
        ));
        
        Ok(Self {
            config,
            shutdown,
            wal_writer,
            snapshot_manager,
            recovery_manager,
            replication_client,
            metadata_state: Arc::new(RwLock::new(HashMap::new())),
            server_state: Arc::new(RwLock::new(ServerState::Initializing)),
        })
    }

    /// Run the metalogger server
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting Metalogger server");
        
        // Step 1: Initialize and recover
        self.set_state(ServerState::Recovering).await;
        let last_sequence = self.initialize_and_recover().await?;
        
        // Step 2: Connect to master and start syncing
        self.set_state(ServerState::Syncing).await;
        self.connect_to_master().await?;
        
        // Step 3: Start all services
        self.set_state(ServerState::Running).await;
        
        // Start replication service
        let replication_handle = self.start_replication_service();
        
        // Start snapshot service
        let snapshot_handle = self.start_snapshot_service();
        
        // Start WAL rotation service
        let wal_rotation_handle = self.start_wal_rotation_service();
        
        // Start gRPC server
        let grpc_handle = self.start_grpc_server();
        
        // Start metrics server
        let metrics_handle = self.start_metrics_server();
        
        // Wait for shutdown signal
        let mut shutdown_signal = self.shutdown.shutdown_signal();
        let _ = shutdown_signal.recv().await;
        
        // Graceful shutdown
        self.set_state(ServerState::Stopping).await;
        info!("Shutting down Metalogger server");
        
        // Cancel all tasks
        replication_handle.abort();
        snapshot_handle.abort();
        wal_rotation_handle.abort();
        grpc_handle.abort();
        metrics_handle.abort();
        
        // Final checkpoint
        self.create_final_checkpoint().await?;
        
        info!("Metalogger server shutdown complete");
        Ok(())
    }

    /// Initialize and recover from persistent state
    async fn initialize_and_recover(&self) -> Result<u64> {
        info!("Initializing and recovering metadata state");
        
        // Initialize WAL
        let wal_last_seq = self.wal_writer.initialize().await?;
        info!("WAL initialized with last sequence: {}", wal_last_seq);
        
        // Recover from snapshots and WAL
        let recovered_data = self.recovery_manager.recover().await?;
        
        // Update metadata state
        let mut state = self.metadata_state.write().await;
        *state = recovered_data;
        let entries_count = state.len();
        drop(state);
        
        info!("Recovered {} metadata entries", entries_count);
        
        // Get recovery state
        let recovery_state = self.recovery_manager.get_state().await;
        Ok(recovery_state.last_sequence_id)
    }

    /// Connect to master server
    async fn connect_to_master(&self) -> Result<()> {
        let master_addr = format!("http://{}:{}", 
                                 self.config.network.master_address,
                                 self.config.network.master_port);
        
        let mut client = self.replication_client.write().await;
        client.connect(&master_addr).await?;
        
        Ok(())
    }

    /// Start replication service
    fn start_replication_service(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let server = self.clone();
        
        tokio::spawn(async move {
            info!("Starting replication service");
            
            let mut subscriber = {
                let client = server.replication_client.read().await;
                client.subscribe()
            };
            
            let mut shutdown_signal = server.shutdown.shutdown_signal();
            loop {
                tokio::select! {
                    Ok(change) = subscriber.recv() => {
                        if let Err(e) = server.handle_metadata_change(change).await {
                            error!("Failed to handle metadata change: {}", e);
                        }
                    }
                    _ = shutdown_signal.recv() => {
                        break;
                    }
                }
            }
            
            info!("Replication service stopped");
        })
    }

    /// Handle incoming metadata change
    async fn handle_metadata_change(&self, change: MetadataChange) -> Result<()> {
        debug!("Handling metadata change: seq={}, op={:?}", 
               change.sequence_id, change.operation);
        
        // Apply change to local state
        let mut state = self.metadata_state.write().await;
        
        match change.operation {
            crate::replication::OperationType::Create |
            crate::replication::OperationType::Update => {
                // Simplified - would need proper parsing
                state.insert(
                    format!("key_{}", change.sequence_id),
                    change.data.clone()
                );
            }
            crate::replication::OperationType::Delete => {
                // Would need to extract key from change data
                state.remove(&format!("key_{}", change.sequence_id));
            }
            _ => {
                debug!("Unhandled operation type: {:?}", change.operation);
            }
        }
        
        Ok(())
    }

    /// Start snapshot service
    fn start_snapshot_service(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let server = self.clone();
        
        tokio::spawn(async move {
            info!("Starting snapshot service");
            
            let mut interval = interval(Duration::from_secs(server.config.snapshot.interval));
            
            let mut shutdown_signal = server.shutdown.shutdown_signal();
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = server.create_snapshot().await {
                            error!("Failed to create snapshot: {}", e);
                        }
                    }
                    _ = shutdown_signal.recv() => {
                        break;
                    }
                }
            }
            
            info!("Snapshot service stopped");
        })
    }

    /// Create a snapshot
    async fn create_snapshot(&self) -> Result<()> {
        info!("Creating periodic snapshot");
        
        // Get current state
        let state = self.metadata_state.read().await;
        let snapshot_data: Vec<(String, Bytes)> = state
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        drop(state);
        
        // Get current sequence ID
        let sequence_id = self.replication_client.read().await
            .get_last_sequence_id().await;
        
        // Create snapshot
        let mut metadata = self.snapshot_manager
            .create_snapshot(snapshot_data).await?;
        metadata.sequence_id = sequence_id;
        
        info!("Snapshot created successfully: id={}, seq={}", 
              metadata.id, metadata.sequence_id);
        
        Ok(())
    }

    /// Start WAL rotation service
    fn start_wal_rotation_service(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let server = self.clone();
        
        tokio::spawn(async move {
            info!("Starting WAL rotation service");
            
            let mut interval = interval(Duration::from_secs(3600)); // Hourly
            
            let mut shutdown_signal = server.shutdown.shutdown_signal();
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = server.rotate_wal().await {
                            error!("Failed to rotate WAL: {}", e);
                        }
                    }
                    _ = shutdown_signal.recv() => {
                        break;
                    }
                }
            }
            
            info!("WAL rotation service stopped");
        })
    }

    /// Rotate WAL files
    async fn rotate_wal(&self) -> Result<()> {
        info!("Rotating WAL files");
        self.wal_writer.rotate(self.config.storage.wal_retention_count).await?;
        Ok(())
    }

    /// Start gRPC server
    fn start_grpc_server(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let server = self.clone();
        
        tokio::spawn(async move {
            info!("Starting gRPC server on {}:{}", 
                  server.config.network.listen_address,
                  server.config.network.listen_port);
            
            let addr = format!("{}:{}",
                             server.config.network.listen_address,
                             server.config.network.listen_port)
                .parse()
                .expect("Invalid address");
            
            // In a real implementation, we'd implement the gRPC service here
            // For now, just keep the task alive
            let mut shutdown_signal = server.shutdown.shutdown_signal();
            let _ = shutdown_signal.recv().await;
            
            info!("gRPC server stopped");
        })
    }

    /// Start metrics server
    fn start_metrics_server(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let server = self.clone();
        
        tokio::spawn(async move {
            info!("Starting metrics server");
            
            // In a real implementation, we'd expose Prometheus metrics here
            let mut shutdown_signal = server.shutdown.shutdown_signal();
            let _ = shutdown_signal.recv().await;
            
            info!("Metrics server stopped");
        })
    }

    /// Create final checkpoint before shutdown
    async fn create_final_checkpoint(&self) -> Result<()> {
        info!("Creating final checkpoint before shutdown");
        
        let state = self.metadata_state.read().await;
        let data = state.clone();
        drop(state);
        
        let sequence_id = self.replication_client.read().await
            .get_last_sequence_id().await;
        
        self.recovery_manager.create_checkpoint(data, sequence_id).await?;
        
        Ok(())
    }

    /// Set server state
    async fn set_state(&self, new_state: ServerState) {
        let mut state = self.server_state.write().await;
        info!("Server state transition: {:?} -> {:?}", *state, new_state);
        *state = new_state;
    }

    /// Get server state
    pub async fn get_state(&self) -> ServerState {
        *self.server_state.read().await
    }

    /// Get metadata statistics
    pub async fn get_stats(&self) -> MetaloggerStats {
        let metadata_count = self.metadata_state.read().await.len();
        let recovery_state = self.recovery_manager.get_state().await;
        let last_sequence = self.replication_client.read().await
            .get_last_sequence_id().await;
        
        MetaloggerStats {
            metadata_entries: metadata_count as u64,
            last_sequence_id: last_sequence,
            last_snapshot_id: recovery_state.last_snapshot.map(|s| s.id),
            uptime_seconds: 0, // Would track actual uptime
        }
    }
}

/// Metalogger statistics
#[derive(Debug, Clone)]
pub struct MetaloggerStats {
    pub metadata_entries: u64,
    pub last_sequence_id: u64,
    pub last_snapshot_id: Option<u64>,
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_lifecycle() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = MetaloggerConfig::default();
        config.storage.data_dir = temp_dir.path().to_str().unwrap().to_string();
        
        let runtime = AsyncRuntime::new(Default::default())?;
        let server = Arc::new(MetaloggerServer::new(config, runtime.clone()).await?);
        
        // Check initial state
        assert_eq!(server.get_state().await, ServerState::Initializing);
        
        // Check stats
        let stats = server.get_stats().await;
        assert_eq!(stats.metadata_entries, 0);
        
        Ok(())
    }
}