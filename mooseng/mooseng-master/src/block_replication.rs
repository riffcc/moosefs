/// Distributed block replication module using Raft consensus
/// Handles replication of data blocks across chunk servers with strong consistency guarantees

use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, broadcast, mpsc};
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};

use crate::raft::{
    RaftConsensus, LogCommand, LogIndex, Term,
    state::NodeId,
};
use crate::multiregion::{
    MultiRegionRaft, ConsistencyLevel, HLCTimestamp,
    RegionPeer, MultiregionConfig,
};
use crate::chunk_manager::{ChunkManager, ChunkServerStatus};
use mooseng_common::types::{
    ChunkId, ChunkLocation, ChunkVersion, InodeId, SessionId,
    StorageClassDef, ChunkMetadata, ChunkServerId,
};

/// Block replication operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockReplicationOp {
    /// Replicate a new block
    ReplicateBlock {
        chunk_id: ChunkId,
        version: ChunkVersion,
        source_server: ChunkServerId,
        target_servers: Vec<ChunkServerId>,
        size: u64,
        checksum: u64,
    },
    
    /// Update block locations after successful replication
    UpdateBlockLocations {
        chunk_id: ChunkId,
        version: ChunkVersion,
        locations: Vec<ChunkLocation>,
        timestamp: HLCTimestamp,
    },
    
    /// Remove block replica from a server
    RemoveReplica {
        chunk_id: ChunkId,
        server_id: ChunkServerId,
    },
    
    /// Initiate block recovery for failed replicas
    RecoverBlock {
        chunk_id: ChunkId,
        failed_servers: Vec<ChunkServerId>,
        target_servers: Vec<ChunkServerId>,
    },
    
    /// Cross-region block replication
    CrossRegionReplicate {
        chunk_id: ChunkId,
        source_region: u32,
        target_regions: Vec<u32>,
        consistency_level: ConsistencyLevel,
    },
}

/// Block replication status
#[derive(Debug, Clone)]
pub struct BlockReplicationStatus {
    pub chunk_id: ChunkId,
    pub version: ChunkVersion,
    pub replicas: HashMap<ChunkServerId, ReplicaStatus>,
    pub target_replica_count: usize,
    pub min_replica_count: usize,
    pub cross_region_replicas: HashMap<u32, Vec<ChunkServerId>>,
    pub last_update: Instant,
}

/// Individual replica status
#[derive(Debug, Clone)]
pub struct ReplicaStatus {
    pub server_id: ChunkServerId,
    pub region_id: u32,
    pub state: ReplicaState,
    pub last_verified: Option<Instant>,
    pub sync_progress: f32,
}

/// Replica state
#[derive(Debug, Clone, PartialEq)]
pub enum ReplicaState {
    Healthy,
    Syncing,
    Stale,
    Failed,
    Recovering,
}

/// Block replication configuration
#[derive(Debug, Clone)]
pub struct BlockReplicationConfig {
    /// Minimum number of replicas per block
    pub min_replicas: usize,
    
    /// Target number of replicas per block
    pub target_replicas: usize,
    
    /// Maximum concurrent replication operations
    pub max_concurrent_replications: usize,
    
    /// Replication timeout
    pub replication_timeout: Duration,
    
    /// Health check interval
    pub health_check_interval: Duration,
    
    /// Cross-region replication enabled
    pub enable_cross_region: bool,
    
    /// Region-aware placement
    pub region_aware_placement: bool,
}

impl Default for BlockReplicationConfig {
    fn default() -> Self {
        Self {
            min_replicas: 2,
            target_replicas: 3,
            max_concurrent_replications: 10,
            replication_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(60),
            enable_cross_region: true,
            region_aware_placement: true,
        }
    }
}

/// Distributed block replication manager
pub struct BlockReplicationManager {
    /// Raft consensus for coordination
    raft: Arc<RaftConsensus>,
    
    /// Multi-region Raft (if enabled)
    multiregion_raft: Option<Arc<MultiRegionRaft>>,
    
    /// Chunk manager
    chunk_manager: Arc<ChunkManager>,
    
    /// Configuration
    config: BlockReplicationConfig,
    
    /// Multiregion configuration
    multiregion_config: Option<MultiregionConfig>,
    
    /// Block replication status
    replication_status: Arc<RwLock<HashMap<ChunkId, BlockReplicationStatus>>>,
    
    /// Pending replication operations
    pending_operations: Arc<Mutex<HashMap<ChunkId, BlockReplicationOp>>>,
    
    /// Replication operation channel
    op_tx: mpsc::Sender<BlockReplicationOp>,
    op_rx: Arc<Mutex<mpsc::Receiver<BlockReplicationOp>>>,
    
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl BlockReplicationManager {
    pub async fn new(
        raft: Arc<RaftConsensus>,
        chunk_manager: Arc<ChunkManager>,
        config: BlockReplicationConfig,
        multiregion_config: Option<MultiregionConfig>,
    ) -> Result<Self> {
        let (op_tx, op_rx) = mpsc::channel(1000);
        let (shutdown_tx, _) = broadcast::channel(1);
        
        // Create multi-region Raft if configured
        let multiregion_raft = if let Some(mr_config) = &multiregion_config {
            Some(Arc::new(MultiRegionRaft::new(raft.clone(), mr_config.clone()).await?))
        } else {
            None
        };
        
        Ok(Self {
            raft,
            multiregion_raft,
            chunk_manager,
            config,
            multiregion_config,
            replication_status: Arc::new(RwLock::new(HashMap::new())),
            pending_operations: Arc::new(Mutex::new(HashMap::new())),
            op_tx,
            op_rx: Arc::new(Mutex::new(op_rx)),
            shutdown_tx,
        })
    }
    
    /// Start the block replication manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting block replication manager");
        
        // Start multi-region Raft if enabled
        if let Some(mr_raft) = &self.multiregion_raft {
            mr_raft.start().await?;
        }
        
        // Start replication worker
        self.start_replication_worker().await?;
        
        // Start health checker
        self.start_health_checker().await?;
        
        Ok(())
    }
    
    /// Stop the block replication manager
    pub async fn stop(&self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        
        if let Some(mr_raft) = &self.multiregion_raft {
            mr_raft.stop().await?;
        }
        
        Ok(())
    }
    
    /// Initiate block replication
    pub async fn replicate_block(
        &self,
        chunk_id: ChunkId,
        source_server: ChunkServerId,
        storage_class: &StorageClassDef,
    ) -> Result<LogIndex> {
        // Get current chunk metadata
        let chunk_meta = self.chunk_manager.get_chunk_metadata(chunk_id).await?
            .ok_or_else(|| anyhow!("Chunk {} not found", chunk_id))?;
        
        // Determine target servers based on storage class
        let target_servers = self.select_replication_targets(
            &chunk_meta,
            storage_class,
            source_server,
        ).await?;
        
        if target_servers.is_empty() {
            return Err(anyhow!("No suitable replication targets found"));
        }
        
        // Create replication operation
        let op = BlockReplicationOp::ReplicateBlock {
            chunk_id,
            version: chunk_meta.version,
            source_server,
            target_servers: target_servers.clone(),
            size: 0, // TODO: Get actual size
            checksum: 0, // TODO: Calculate checksum
        };
        
        // Submit to Raft for consensus
        let index = self.submit_operation(op.clone()).await?;
        
        // Track pending operation
        {
            let mut pending = self.pending_operations.lock().await;
            pending.insert(chunk_id, op);
        }
        
        Ok(index)
    }
    
    /// Submit operation to Raft consensus
    async fn submit_operation(&self, op: BlockReplicationOp) -> Result<LogIndex> {
        // Serialize operation
        let op_bytes = bincode::serialize(&op)?;
        
        // Create log command
        let command = LogCommand::SetMetadata {
            key: format!("block_replication:{}", match &op {
                BlockReplicationOp::ReplicateBlock { chunk_id, .. } => chunk_id.to_string(),
                BlockReplicationOp::UpdateBlockLocations { chunk_id, .. } => chunk_id.to_string(),
                BlockReplicationOp::RemoveReplica { chunk_id, .. } => chunk_id.to_string(),
                BlockReplicationOp::RecoverBlock { chunk_id, .. } => chunk_id.to_string(),
                BlockReplicationOp::CrossRegionReplicate { chunk_id, .. } => chunk_id.to_string(),
            }),
            value: op_bytes,
        };
        
        // Use multi-region Raft if available, otherwise use regular Raft
        if let Some(mr_raft) = &self.multiregion_raft {
            let (index, _timestamp) = mr_raft.append_entry_with_timestamp(command).await?;
            Ok(index)
        } else {
            self.raft.append_entry(command).await
        }
    }
    
    /// Select replication targets based on storage class and placement policy
    async fn select_replication_targets(
        &self,
        chunk_meta: &ChunkMetadata,
        storage_class: &StorageClassDef,
        exclude_server: ChunkServerId,
    ) -> Result<Vec<ChunkServerId>> {
        let servers = self.chunk_manager.get_active_servers().await?;
        let mut candidates: Vec<ChunkServerStatus> = servers.into_iter()
            .filter(|s| s.id != exclude_server && s.is_active)
            .collect();
        
        // Sort by available space and load
        candidates.sort_by_key(|s| {
            let usage_ratio = s.used_space as f64 / s.total_space as f64;
            (usage_ratio * 1000.0) as u64
        });
        
        let mut selected = Vec::new();
        let target_count = storage_class.copies.max(self.config.target_replicas) as usize;
        
        // Region-aware selection if enabled
        if self.config.region_aware_placement && self.multiregion_config.is_some() {
            let mut regions_used = HashSet::new();
            
            // First, select servers from different regions
            for server in &candidates {
                if !regions_used.contains(&server.region_id) {
                    selected.push(server.id);
                    regions_used.insert(server.region_id);
                    
                    if selected.len() >= target_count {
                        break;
                    }
                }
            }
            
            // If we need more replicas, select from same regions
            if selected.len() < target_count {
                for server in &candidates {
                    if !selected.contains(&server.id) {
                        selected.push(server.id);
                        
                        if selected.len() >= target_count {
                            break;
                        }
                    }
                }
            }
        } else {
            // Simple selection without region awareness
            selected = candidates.iter()
                .take(target_count)
                .map(|s| s.id)
                .collect();
        }
        
        Ok(selected)
    }
    
    /// Start replication worker
    async fn start_replication_worker(&self) -> Result<()> {
        let op_rx = self.op_rx.clone();
        let status = self.replication_status.clone();
        let chunk_manager = self.chunk_manager.clone();
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(op) = async {
                        let mut rx = op_rx.lock().await;
                        rx.recv().await
                    } => {
                        if let Err(e) = Self::process_replication_op(
                            op,
                            &status,
                            &chunk_manager,
                            &config,
                        ).await {
                            error!("Failed to process replication operation: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Replication worker shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Process a replication operation
    async fn process_replication_op(
        op: BlockReplicationOp,
        status: &Arc<RwLock<HashMap<ChunkId, BlockReplicationStatus>>>,
        chunk_manager: &Arc<ChunkManager>,
        config: &BlockReplicationConfig,
    ) -> Result<()> {
        match op {
            BlockReplicationOp::ReplicateBlock { 
                chunk_id, 
                version, 
                source_server, 
                target_servers,
                ..
            } => {
                info!("Replicating block {} v{} from {} to {:?}", 
                    chunk_id, version, source_server, target_servers);
                
                // TODO: Implement actual block replication
                // This would involve:
                // 1. Connecting to source server
                // 2. Streaming block data to target servers
                // 3. Verifying checksums
                // 4. Updating metadata
                
                // For now, simulate successful replication
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                // Update status
                let mut status_map = status.write().await;
                let block_status = status_map.entry(chunk_id).or_insert_with(|| {
                    BlockReplicationStatus {
                        chunk_id,
                        version,
                        replicas: HashMap::new(),
                        target_replica_count: config.target_replicas,
                        min_replica_count: config.min_replicas,
                        cross_region_replicas: HashMap::new(),
                        last_update: Instant::now(),
                    }
                });
                
                // Add new replicas
                for server_id in target_servers {
                    block_status.replicas.insert(server_id, ReplicaStatus {
                        server_id,
                        region_id: 0, // TODO: Get actual region
                        state: ReplicaState::Healthy,
                        last_verified: Some(Instant::now()),
                        sync_progress: 1.0,
                    });
                }
                
                block_status.last_update = Instant::now();
            }
            
            BlockReplicationOp::UpdateBlockLocations { 
                chunk_id, 
                locations,
                ..
            } => {
                // Update chunk locations in chunk manager
                chunk_manager.update_chunk_locations(chunk_id, locations).await?;
            }
            
            BlockReplicationOp::RemoveReplica { 
                chunk_id, 
                server_id 
            } => {
                // Remove replica from tracking
                let mut status_map = status.write().await;
                if let Some(block_status) = status_map.get_mut(&chunk_id) {
                    block_status.replicas.remove(&server_id);
                    block_status.last_update = Instant::now();
                }
            }
            
            BlockReplicationOp::RecoverBlock { 
                chunk_id,
                failed_servers,
                target_servers,
            } => {
                info!("Recovering block {} from failed servers {:?} to {:?}", 
                    chunk_id, failed_servers, target_servers);
                
                // TODO: Implement block recovery
                // This would involve finding healthy replicas and copying to new targets
            }
            
            BlockReplicationOp::CrossRegionReplicate {
                chunk_id,
                source_region,
                target_regions,
                consistency_level,
            } => {
                info!("Cross-region replication of block {} from region {} to {:?} with {:?} consistency",
                    chunk_id, source_region, target_regions, consistency_level);
                
                // TODO: Implement cross-region replication
                // This would use the multi-region Raft for coordination
            }
        }
        
        Ok(())
    }
    
    /// Start health checker
    async fn start_health_checker(&self) -> Result<()> {
        let status = self.replication_status.clone();
        let chunk_manager = self.chunk_manager.clone();
        let config = self.config.clone();
        let op_tx = self.op_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut check_interval = tokio::time::interval(config.health_check_interval);
            check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = check_interval.tick() => {
                        if let Err(e) = Self::check_replica_health(
                            &status,
                            &chunk_manager,
                            &config,
                            &op_tx,
                        ).await {
                            warn!("Health check failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Health checker shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Check health of all replicas
    async fn check_replica_health(
        status: &Arc<RwLock<HashMap<ChunkId, BlockReplicationStatus>>>,
        chunk_manager: &Arc<ChunkManager>,
        config: &BlockReplicationConfig,
        op_tx: &mpsc::Sender<BlockReplicationOp>,
    ) -> Result<()> {
        let status_snapshot = {
            let status_map = status.read().await;
            status_map.clone()
        };
        
        for (chunk_id, block_status) in status_snapshot {
            let healthy_replicas: Vec<_> = block_status.replicas.values()
                .filter(|r| r.state == ReplicaState::Healthy)
                .collect();
            
            let healthy_count = healthy_replicas.len();
            
            // Check if we have enough healthy replicas
            if healthy_count < block_status.min_replica_count {
                warn!("Block {} has only {} healthy replicas, minimum is {}", 
                    chunk_id, healthy_count, block_status.min_replica_count);
                
                // Find failed servers
                let failed_servers: Vec<_> = block_status.replicas.values()
                    .filter(|r| r.state == ReplicaState::Failed)
                    .map(|r| r.server_id)
                    .collect();
                
                if !failed_servers.is_empty() {
                    // Initiate recovery
                    let recovery_op = BlockReplicationOp::RecoverBlock {
                        chunk_id,
                        failed_servers,
                        target_servers: vec![], // Will be selected during recovery
                    };
                    
                    if let Err(e) = op_tx.send(recovery_op).await {
                        error!("Failed to queue recovery operation: {}", e);
                    }
                }
            }
            
            // Check if we need more replicas to reach target
            if healthy_count < block_status.target_replica_count {
                debug!("Block {} has {} healthy replicas, target is {}", 
                    chunk_id, healthy_count, block_status.target_replica_count);
                
                // TODO: Queue replication to reach target count
            }
        }
        
        Ok(())
    }
    
    /// Get replication status for a block
    pub async fn get_block_status(&self, chunk_id: ChunkId) -> Option<BlockReplicationStatus> {
        let status_map = self.replication_status.read().await;
        status_map.get(&chunk_id).cloned()
    }
    
    /// Get overall replication health metrics
    pub async fn get_replication_metrics(&self) -> ReplicationMetrics {
        let status_map = self.replication_status.read().await;
        
        let mut total_blocks = 0;
        let mut under_replicated_blocks = 0;
        let mut critical_blocks = 0;
        let mut total_replicas = 0;
        let mut healthy_replicas = 0;
        
        for block_status in status_map.values() {
            total_blocks += 1;
            
            let healthy_count = block_status.replicas.values()
                .filter(|r| r.state == ReplicaState::Healthy)
                .count();
            
            total_replicas += block_status.replicas.len();
            healthy_replicas += healthy_count;
            
            if healthy_count < block_status.target_replica_count {
                under_replicated_blocks += 1;
            }
            
            if healthy_count < block_status.min_replica_count {
                critical_blocks += 1;
            }
        }
        
        ReplicationMetrics {
            total_blocks,
            under_replicated_blocks,
            critical_blocks,
            total_replicas,
            healthy_replicas,
            replication_factor: if total_blocks > 0 { 
                total_replicas as f32 / total_blocks as f32 
            } else { 
                0.0 
            },
        }
    }
}

/// Replication health metrics
#[derive(Debug, Clone)]
pub struct ReplicationMetrics {
    pub total_blocks: usize,
    pub under_replicated_blocks: usize,
    pub critical_blocks: usize,
    pub total_replicas: usize,
    pub healthy_replicas: usize,
    pub replication_factor: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::raft::RaftConfig;
    
    async fn create_test_replication_manager() -> Result<BlockReplicationManager> {
        let dir = tempdir()?;
        let raft_config = RaftConfig {
            node_id: "test_node".to_string(),
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let raft = Arc::new(RaftConsensus::new(raft_config).await?);
        let metadata_store = Arc::new(crate::metadata::MetadataStore::new(dir.path().to_path_buf())?);
        let cache = Arc::new(crate::cache::MetadataCache::new(Default::default()));
        let chunk_manager = Arc::new(ChunkManager::new(
            metadata_store,
            cache,
            crate::chunk_manager::AllocationStrategy::RoundRobin,
        ));
        
        let config = BlockReplicationConfig::default();
        
        BlockReplicationManager::new(raft, chunk_manager, config, None).await
    }
    
    #[tokio::test]
    async fn test_block_replication_manager_creation() -> Result<()> {
        let manager = create_test_replication_manager().await?;
        assert_eq!(manager.config.target_replicas, 3);
        assert_eq!(manager.config.min_replicas, 2);
        Ok(())
    }
    
    #[tokio::test]
    async fn test_replication_metrics() -> Result<()> {
        let manager = create_test_replication_manager().await?;
        let metrics = manager.get_replication_metrics().await;
        
        assert_eq!(metrics.total_blocks, 0);
        assert_eq!(metrics.under_replicated_blocks, 0);
        assert_eq!(metrics.critical_blocks, 0);
        Ok(())
    }
}