use anyhow::Result;
use mooseng_common::config::MasterConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use crate::metadata::MetadataStore;
use crate::cache::MetadataCache;
use crate::filesystem::FileSystem;
use crate::chunk_manager::{ChunkManager, AllocationStrategy};
use crate::session::SessionManager;
use crate::storage_class::StorageClassManager;
use crate::grpc_services::GrpcServer;
use crate::raft::{RaftConsensus, RaftConfig};
use crate::multiregion::raft_multiregion::MultiRegionRaft;

pub struct MasterServer {
    config: Arc<MasterConfig>,
    metadata_store: Arc<MetadataStore>,
    cache: Arc<MetadataCache>,
    filesystem: Arc<FileSystem>,
    chunk_manager: Arc<ChunkManager>,
    session_manager: Arc<SessionManager>,
    storage_class_manager: Arc<StorageClassManager>,
    raft_consensus: Option<Arc<RaftConsensus>>,
    shutdown_tx: broadcast::Sender<()>,
    health_status: Arc<RwLock<ServerHealth>>,
}

#[derive(Debug, Clone)]
pub struct ServerHealth {
    pub is_healthy: bool,
    pub is_leader: bool,
    pub last_health_check: Duration,
    pub active_connections: usize,
    pub metadata_sync_lag: Duration,
}

impl MasterServer {
    pub async fn new(config: MasterConfig) -> Result<Self> {
        info!("Initializing Master Server components");

        // Configure tokio runtime based on configuration
        Self::configure_runtime(&config)?;

        // Create shared configuration
        let config = Arc::new(config);

        // Initialize metadata store
        let metadata_store = Arc::new(MetadataStore::new(&config.data_dir).await?);
        
        // Initialize metadata cache
        let cache = Arc::new(MetadataCache::new(
            Duration::from_secs(300), // 5 minute TTL
            config.metadata_cache_size,
        ));
        
        // Initialize storage class manager
        let storage_class_manager = Arc::new(
            StorageClassManager::new(metadata_store.clone()).await?
        );

        // Initialize chunk manager
        let chunk_manager = Arc::new(
            ChunkManager::new(
                metadata_store.clone(),
                cache.clone(),
                AllocationStrategy::LeastUsed,
            )
        );

        // Initialize filesystem
        let filesystem = Arc::new(
            FileSystem::new(
                (*metadata_store).clone(),
                (*cache).clone(),
            )
        );

        // Initialize session manager
        let session_manager = Arc::new(
            SessionManager::new(
                config.session_timeout_ms,
                config.max_clients,
            )
        );

        // Initialize health status
        let health_status = Arc::new(RwLock::new(ServerHealth {
            is_healthy: true,
            is_leader: true, // Will be updated by HA logic later
            last_health_check: Duration::from_secs(0),
            active_connections: 0,
            metadata_sync_lag: Duration::from_secs(0),
        }));

        let (shutdown_tx, _) = broadcast::channel(1);

        // Initialize Raft consensus if HA is enabled
        let raft_consensus = if config.ha_enabled {
            info!("Initializing Raft consensus for HA");
            let raft_config = RaftConfig::from_master_config(&config);
            match RaftConsensus::new(raft_config).await {
                Ok(consensus) => Some(Arc::new(consensus)),
                Err(e) => {
                    error!("Failed to initialize Raft consensus: {}", e);
                    return Err(e);
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            metadata_store,
            cache,
            filesystem,
            chunk_manager,
            session_manager,
            storage_class_manager,
            raft_consensus,
            shutdown_tx,
            health_status,
        })
    }

    fn configure_runtime(config: &MasterConfig) -> Result<()> {
        // Configure the tokio runtime thread pool
        let worker_threads = if config.worker_threads == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        } else {
            config.worker_threads
        };

        info!("Configuring tokio runtime with {} worker threads", worker_threads);
        
        // Set thread names for better debugging and profiling
        std::env::set_var("TOKIO_WORKER_THREADS", worker_threads.to_string());
        
        Ok(())
    }

    pub async fn run(self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info!("Starting Master Server services");

        // Report startup metrics
        info!("Server configuration: {} max clients, {} MB metadata cache", 
              self.config.max_clients, self.config.metadata_cache_size / (1024 * 1024));
        
        // Start Raft consensus if enabled
        if let Some(ref raft) = self.raft_consensus {
            raft.start().await?;
            info!("Raft consensus started");
        }
        
        // Start background tasks
        self.start_background_tasks();

        // Create and start gRPC server
        let grpc_server = GrpcServer::new(
            self.config.clone(),
            self.metadata_store.clone(),
            self.filesystem.clone(),
            self.chunk_manager.clone(),
            self.session_manager.clone(),
            self.storage_class_manager.clone(),
            self.raft_consensus.clone(),
            None, // multiregion_raft - not initialized yet
        );

        info!("Master Server fully initialized and ready to serve requests");

        // Run the gRPC server with shutdown handling
        tokio::select! {
            result = grpc_server.run() => {
                match result {
                    Ok(_) => info!("gRPC server stopped"),
                    Err(e) => {
                        error!("gRPC server error: {}", e);
                        return Err(e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received");
            }
        }

        // Perform graceful shutdown
        self.shutdown().await?;

        Ok(())
    }

    pub async fn get_health_status(&self) -> ServerHealth {
        self.health_status.read().await.clone()
    }

    /// Check if this node is the Raft leader
    pub async fn is_leader(&self) -> bool {
        match &self.raft_consensus {
            Some(raft) => raft.is_leader().await,
            None => true, // Single node without HA is always leader
        }
    }

    /// Get the current Raft leader ID
    pub async fn get_leader_id(&self) -> Option<String> {
        match &self.raft_consensus {
            Some(raft) => raft.get_leader_id().await,
            None => Some(self.config.node_id.clone()),
        }
    }

    /// Get Raft consensus instance for advanced operations
    pub fn get_raft_consensus(&self) -> Option<Arc<RaftConsensus>> {
        self.raft_consensus.clone()
    }

    /// Add a new node to the Raft cluster
    pub async fn add_cluster_node(&self, node_id: String, voting: bool) -> Result<()> {
        match &self.raft_consensus {
            Some(raft) => {
                use crate::raft::ConfigChangeType;
                let change = if voting {
                    ConfigChangeType::AddVotingNode(node_id)
                } else {
                    ConfigChangeType::AddNonVotingNode(node_id)
                };
                raft.propose_config_change(change).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("Raft consensus not enabled")),
        }
    }

    /// Remove a node from the Raft cluster
    pub async fn remove_cluster_node(&self, node_id: String) -> Result<()> {
        match &self.raft_consensus {
            Some(raft) => {
                use crate::raft::ConfigChangeType;
                let change = ConfigChangeType::RemoveNode(node_id);
                raft.propose_config_change(change).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("Raft consensus not enabled")),
        }
    }

    fn start_background_tasks(&self) {
        let metadata_store = self.metadata_store.clone();
        let chunk_manager = self.chunk_manager.clone();
        let session_manager = self.session_manager.clone();
        let cache = self.cache.clone();
        let health_status = self.health_status.clone();

        // Metadata checksum task
        let metadata_interval = self.config.metadata_checksum_interval_ms;
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(metadata_interval));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        debug!("Running metadata checksum");
                        if let Err(e) = metadata_store.verify_checksum().await {
                            error!("Metadata checksum failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Metadata checksum task shutting down");
                        break;
                    }
                }
            }
        });

        // Chunk health check task
        let chunk_interval = self.config.chunk_test_interval_ms;
        let chunk_manager_clone = chunk_manager.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(chunk_interval));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        debug!("Running chunk health check");
                        chunk_manager_clone.check_chunk_health().await;
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Chunk health check task shutting down");
                        break;
                    }
                }
            }
        });

        // Session cleanup task
        let session_manager_clone = session_manager.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        debug!("Cleaning up expired sessions");
                        session_manager_clone.cleanup_expired().await;
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Session cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        // Cache statistics and cleanup task
        let cache_clone = cache.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let stats = cache_clone.stats();
                        let (hits, misses, inserts, updates, evictions) = stats.get_stats();
                        debug!("Cache stats: {} hits, {} misses, {} inserts, {} updates, {} evictions", 
                               hits, misses, inserts, updates, evictions);
                        
                        // Trigger cache cleanup (basic cleanup, not async)
                        // In a more sophisticated implementation, we could add an async cleanup method
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Cache cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        // Health check task
        let health_status_clone = health_status.clone();
        let session_manager_health = session_manager.clone();
        let raft_consensus_health = self.raft_consensus.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let start_time = std::time::Instant::now();
                        let active_sessions = session_manager_health.get_active_session_count().await;
                        
                        // Check Raft leader status
                        let is_leader = if let Some(ref raft) = raft_consensus_health {
                            raft.is_leader().await
                        } else {
                            true // If HA is disabled, consider this node as leader
                        };
                        
                        let mut health = health_status_clone.write().await;
                        health.last_health_check = start_time.elapsed();
                        health.active_connections = active_sessions;
                        health.is_healthy = health.last_health_check < Duration::from_secs(1);
                        health.is_leader = is_leader;
                        
                        if !health.is_healthy {
                            warn!("Health check took too long: {:?}", health.last_health_check);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Health check task shutting down");
                        break;
                    }
                }
            }
        });

        info!("Background tasks started successfully");
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Master Server");

        // Update health status to indicate shutdown in progress
        {
            let mut health = self.health_status.write().await;
            health.is_healthy = false;
        }

        // Stop Raft consensus if enabled
        if let Some(ref raft) = self.raft_consensus {
            info!("Stopping Raft consensus");
            if let Err(e) = raft.stop().await {
                error!("Error stopping Raft consensus: {}", e);
            }
        }

        // Signal all background tasks to stop
        let _ = self.shutdown_tx.send(());

        // Wait a short time for background tasks to finish
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Save metadata with timeout
        info!("Saving metadata before shutdown");
        match tokio::time::timeout(
            Duration::from_secs(30),
            self.metadata_store.save_metadata()
        ).await {
            Err(_) => warn!("Metadata save timed out during shutdown"),
            Ok(Err(e)) => error!("Failed to save metadata during shutdown: {}", e),
            Ok(Ok(_)) => info!("Metadata saved successfully"),
        }

        // Close all active sessions
        info!("Closing active sessions");
        self.session_manager.close_all_sessions().await;

        // Flush any pending operations with timeout
        info!("Flushing pending chunk operations");
        match tokio::time::timeout(
            Duration::from_secs(10),
            self.chunk_manager.flush_pending()
        ).await {
            Err(_) => warn!("Chunk manager flush timed out during shutdown"),
            Ok(Err(e)) => error!("Failed to flush chunk operations during shutdown: {}", e),
            Ok(Ok(_)) => info!("Chunk operations flushed successfully"),
        }

        // Final cache flush
        info!("Flushing metadata cache");
        // Note: MetadataCache doesn't have flush, just clearing remaining references

        info!("Master Server shutdown complete");
        Ok(())
    }
}