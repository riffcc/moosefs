/// Multi-region Raft consensus implementation
/// Extends the base Raft consensus to work across multiple regions

use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, broadcast};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn, error};

use crate::raft::{
    RaftConsensus, RaftConfig, Term, LogIndex,
    log::{LogEntry, LogCommand},
    state::{NodeState, NodeId},
};
use crate::multiregion::{
    MultiregionConfig, RegionPeer, HybridLogicalClock, HLCTimestamp,
    consistency::ConsistencyLevel,
};

/// Multi-region Raft extension
pub struct MultiRegionRaft {
    /// Base Raft consensus
    local_raft: Arc<RaftConsensus>,
    
    /// Multi-region configuration
    config: MultiregionConfig,
    
    /// Hybrid logical clock for cross-region ordering
    hlc: Arc<RwLock<HybridLogicalClock>>,
    
    /// Cross-region peer connections
    region_peers: Arc<RwLock<HashMap<u32, RegionPeerState>>>,
    
    /// Leader lease manager
    leader_lease: Arc<Mutex<LeaderLease>>,
    
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

/// State for each peer region
#[derive(Debug, Clone)]
struct RegionPeerState {
    peer: RegionPeer,
    last_heartbeat: Instant,
    leader_id: Option<NodeId>,
    leader_term: Term,
    replication_lag: Duration,
    is_available: bool,
}

/// Leader lease management for fast reads
#[derive(Debug)]
struct LeaderLease {
    /// Current lease holder (if any)
    lease_holder: Option<NodeId>,
    
    /// When the current lease expires
    lease_expires: Option<Instant>,
    
    /// Lease duration
    lease_duration: Duration,
    
    /// Last successful heartbeat round
    last_heartbeat_success: Instant,
}

impl LeaderLease {
    fn new(lease_duration: Duration) -> Self {
        Self {
            lease_holder: None,
            lease_expires: None,
            lease_duration,
            last_heartbeat_success: Instant::now(),
        }
    }
    
    fn is_valid(&self) -> bool {
        if let Some(expires) = self.lease_expires {
            expires > Instant::now()
        } else {
            false
        }
    }
}

impl MultiRegionRaft {
    pub async fn new(
        local_raft: Arc<RaftConsensus>,
        config: MultiregionConfig,
    ) -> Result<Self> {
        let hlc = Arc::new(RwLock::new(HybridLogicalClock::new(100))); // 100ms max drift
        let region_peers = Arc::new(RwLock::new(HashMap::new()));
        let (shutdown_tx, _) = broadcast::channel(1);
        
        let leader_lease = Arc::new(Mutex::new(LeaderLease::new(config.leader_lease_duration)));
        
        // Initialize peer states
        {
            let mut peers = region_peers.write().await;
            for peer in &config.peer_regions {
                peers.insert(peer.region_id, RegionPeerState {
                    peer: peer.clone(),
                    last_heartbeat: Instant::now(),
                    leader_id: None,
                    leader_term: 0,
                    replication_lag: Duration::ZERO,
                    is_available: false,
                });
            }
        }
        
        Ok(Self {
            local_raft,
            config,
            hlc,
            region_peers,
            leader_lease,
            shutdown_tx,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        info!("Starting multi-region Raft for region {}", self.config.region_id);
        
        // Start the base Raft
        self.local_raft.start().await?;
        
        // Start cross-region monitoring
        self.start_region_monitoring().await?;
        
        // Start leader lease management
        if self.config.enable_leader_leases {
            self.start_leader_lease_management().await?;
        }
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        self.local_raft.stop().await?;
        Ok(())
    }
    
    /// Start monitoring peer regions
    async fn start_region_monitoring(&self) -> Result<()> {
        let peers = self.region_peers.clone();
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut monitor_interval = interval(Duration::from_secs(5));
            monitor_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = monitor_interval.tick() => {
                        if let Err(e) = Self::monitor_peer_regions(&peers, &config).await {
                            warn!("Region monitoring failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Region monitoring shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Monitor peer regions for availability and leadership
    async fn monitor_peer_regions(
        peers: &Arc<RwLock<HashMap<u32, RegionPeerState>>>,
        config: &MultiregionConfig,
    ) -> Result<()> {
        let mut peers_guard = peers.write().await;
        
        for (region_id, peer_state) in peers_guard.iter_mut() {
            // Check if peer is responsive
            let elapsed = peer_state.last_heartbeat.elapsed();
            let was_available = peer_state.is_available;
            
            peer_state.is_available = elapsed < Duration::from_secs(30);
            
            if was_available && !peer_state.is_available {
                warn!("Region {} became unavailable", region_id);
            } else if !was_available && peer_state.is_available {
                info!("Region {} became available", region_id);
            }
            
            // Update replication lag
            peer_state.replication_lag = elapsed;
            
            // TODO: Actually query peer regions for status
            // This would involve making RPC calls to peer regions
        }
        
        Ok(())
    }
    
    /// Start leader lease management
    async fn start_leader_lease_management(&self) -> Result<()> {
        let local_raft = self.local_raft.clone();
        let leader_lease = self.leader_lease.clone();
        let region_peers = self.region_peers.clone();
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut lease_interval = interval(Duration::from_millis(100));
            lease_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = lease_interval.tick() => {
                        if let Err(e) = Self::manage_leader_lease(
                            &local_raft,
                            &leader_lease,
                            &region_peers,
                            &config,
                        ).await {
                            warn!("Leader lease management failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Leader lease management shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Manage leader lease for fast reads
    async fn manage_leader_lease(
        local_raft: &Arc<RaftConsensus>,
        leader_lease: &Arc<Mutex<LeaderLease>>,
        _region_peers: &Arc<RwLock<HashMap<u32, RegionPeerState>>>,
        config: &MultiregionConfig,
    ) -> Result<()> {
        let is_leader = local_raft.is_leader().await;
        let mut lease = leader_lease.lock().await;
        
        let now = Instant::now();
        
        if is_leader {
            // If we're leader, try to acquire/renew lease
            let should_renew = match lease.lease_expires {
                Some(expires) => now + (config.leader_lease_duration / 2) > expires,
                None => true,
            };
            
            if should_renew {
                // TODO: Send heartbeats to majority of regions
                // For now, just grant ourselves the lease
                let node_id = "local_leader".to_string(); // TODO: Get actual node ID
                lease.lease_holder = Some(node_id);
                lease.lease_expires = Some(now + config.leader_lease_duration);
                lease.last_heartbeat_success = now;
                debug!("Renewed leader lease until {:?}", lease.lease_expires);
            }
        } else {
            // If we're not leader, clear any lease we might have
            if lease.lease_holder.is_some() {
                lease.lease_holder = None;
                lease.lease_expires = None;
                debug!("Cleared leader lease (no longer leader)");
            }
        }
        
        // Check if current lease has expired
        if let Some(expires) = lease.lease_expires {
            if now > expires {
                lease.lease_holder = None;
                lease.lease_expires = None;
                debug!("Leader lease expired");
            }
        }
        
        Ok(())
    }
    
    /// Check if we can serve fast reads (have valid leader lease)
    pub async fn can_serve_fast_reads(&self) -> bool {
        if !self.config.enable_leader_leases {
            return false;
        }
        
        let lease = self.leader_lease.lock().await;
        let now = Instant::now();
        
        match (lease.lease_holder.as_ref(), lease.lease_expires) {
            (Some(_), Some(expires)) => now < expires,
            _ => false,
        }
    }
    
    /// Append entry with cross-region timestamp
    pub async fn append_entry_with_timestamp(&self, command: LogCommand) -> Result<(LogIndex, HLCTimestamp)> {
        // Generate HLC timestamp
        let timestamp = {
            let mut hlc = self.hlc.write().await;
            hlc.now()?
        };
        
        // Create timestamped command
        let timestamped_command = match command {
            LogCommand::SetMetadata { key, value } => LogCommand::SetMetadata { 
                key: format!("{}@{}-{}", key, timestamp.physical, timestamp.logical),
                value 
            },
            other => other,
        };
        
        // Append to local Raft
        let index = self.local_raft.append_entry(timestamped_command).await?;
        
        Ok((index, timestamp))
    }
    
    /// Read with consistency level
    pub async fn read_with_consistency(
        &self,
        key: &str,
        consistency: Option<ConsistencyLevel>,
    ) -> Result<Option<Vec<u8>>> {
        let consistency = consistency.unwrap_or(self.config.default_consistency.clone());
        
        match consistency {
            ConsistencyLevel::Strong => {
                // Strong consistency requires being the leader
                if !self.local_raft.is_leader().await {
                    return Err(anyhow!("Strong consistency read requires leadership"));
                }
                
                // Also check leader lease if enabled
                if self.config.enable_leader_leases && !self.can_serve_fast_reads().await {
                    return Err(anyhow!("No valid leader lease for strong consistency read"));
                }
                
                // TODO: Implement actual read from state machine
                Ok(None)
            }
            ConsistencyLevel::BoundedStaleness(max_staleness) => {
                // Check if our data is fresh enough
                let lease = self.leader_lease.lock().await;
                let staleness = lease.last_heartbeat_success.elapsed();
                
                if staleness > max_staleness {
                    return Err(anyhow!("Data too stale for bounded staleness read"));
                }
                
                // TODO: Implement actual read from state machine
                Ok(None)
            }
            ConsistencyLevel::Eventual => {
                // Eventual consistency allows reading from any node
                // TODO: Implement actual read from state machine
                Ok(None)
            }
            ConsistencyLevel::Session => {
                // Session consistency requires tracking session state
                // For now, treat similar to bounded staleness with 1 second max
                let max_staleness = Duration::from_secs(1);
                let lease = self.leader_lease.lock().await;
                
                if lease.is_valid() {
                    // We have a valid lease
                    if lease.lease_holder.is_some() {
                        // We have a valid lease
                        // TODO: Check session state and implement actual read
                        Ok(None)
                    } else {
                        // Lease expired, need to contact leader
                        if let Some(leader_id) = self.local_raft.get_leader_id().await {
                            // TODO: Forward read to leader
                            Ok(None)
                        } else {
                            Err(anyhow!("No leader available for session consistency read"))
                        }
                    }
                } else {
                    // No lease, forward to leader
                    if let Some(leader_id) = self.local_raft.get_leader_id().await {
                        // TODO: Forward read to leader
                        Ok(None)
                    } else {
                        Err(anyhow!("No leader available for session consistency read"))
                    }
                }
            }
        }
    }
    
    /// Get current HLC timestamp
    pub async fn current_timestamp(&self) -> Result<HLCTimestamp> {
        let mut hlc = self.hlc.write().await;
        hlc.now()
    }
    
    /// Update HLC from remote timestamp
    pub async fn update_timestamp(&self, remote_timestamp: HLCTimestamp) -> Result<HLCTimestamp> {
        let mut hlc = self.hlc.write().await;
        hlc.update(remote_timestamp)
    }
    
    /// Get region status
    pub async fn get_region_status(&self) -> RegionStatus {
        let is_leader = self.local_raft.is_leader().await;
        let can_fast_read = self.can_serve_fast_reads().await;
        let peers = self.region_peers.read().await;
        
        let peer_status: Vec<_> = peers.values().map(|peer| PeerRegionStatus {
            region_id: peer.peer.region_id,
            region_name: peer.peer.region_name.clone(),
            is_available: peer.is_available,
            replication_lag: peer.replication_lag,
            leader_id: peer.leader_id.clone(),
        }).collect();
        
        RegionStatus {
            region_id: self.config.region_id,
            region_name: self.config.region_name.clone(),
            is_leader,
            can_serve_fast_reads: can_fast_read,
            peer_regions: peer_status,
        }
    }
    
    /// Check if the cluster has sufficient regions available
    pub async fn has_region_quorum(&self) -> bool {
        let peers = self.region_peers.read().await;
        let available_regions = 1 + peers.values().filter(|p| p.is_available).count(); // 1 for ourselves
        let total_regions = 1 + peers.len();
        
        available_regions > total_regions / 2
    }
}

/// Status information for the entire multi-region cluster
#[derive(Debug, Clone)]
pub struct RegionStatus {
    pub region_id: u32,
    pub region_name: String,
    pub is_leader: bool,
    pub can_serve_fast_reads: bool,
    pub peer_regions: Vec<PeerRegionStatus>,
}

/// Status for a peer region
#[derive(Debug, Clone)]
pub struct PeerRegionStatus {
    pub region_id: u32,
    pub region_name: String,
    pub is_available: bool,
    pub replication_lag: Duration,
    pub leader_id: Option<NodeId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    async fn create_test_multiregion_raft() -> Result<MultiRegionRaft> {
        let dir = tempdir()?;
        let raft_config = RaftConfig {
            node_id: "test_node".to_string(),
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let local_raft = Arc::new(RaftConsensus::new(raft_config).await?);
        
        let multiregion_config = MultiregionConfig {
            region_id: 1,
            region_name: "us-east-1".to_string(),
            peer_regions: vec![
                RegionPeer {
                    region_id: 2,
                    region_name: "us-west-1".to_string(),
                    endpoints: vec!["127.0.0.1:9426".to_string()],
                    priority: 1,
                    latency_ms: 50,
                }
            ],
            ..Default::default()
        };
        
        MultiRegionRaft::new(local_raft, multiregion_config).await
    }
    
    #[tokio::test]
    async fn test_multiregion_raft_creation() -> Result<()> {
        let mr_raft = create_test_multiregion_raft().await?;
        assert_eq!(mr_raft.config.region_id, 1);
        assert_eq!(mr_raft.config.region_name, "us-east-1");
        Ok(())
    }
    
    #[tokio::test]
    async fn test_region_quorum_check() -> Result<()> {
        let mr_raft = create_test_multiregion_raft().await?;
        
        // With 2 total regions (us + 1 peer), we need 2 for quorum
        // Initially peer is unavailable, so no quorum
        assert!(!mr_raft.has_region_quorum().await);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_hlc_timestamp_generation() -> Result<()> {
        let mr_raft = create_test_multiregion_raft().await?;
        
        let ts1 = mr_raft.current_timestamp().await?;
        let ts2 = mr_raft.current_timestamp().await?;
        
        assert!(ts2 > ts1);
        
        Ok(())
    }
}