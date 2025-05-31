use anyhow::{anyhow, Result};
use dashmap::DashMap;
use mooseng_common::types::{
    ChunkId, ChunkLocation, ChunkMetadata, ChunkServerId, ChunkVersion, InodeId, SessionId,
    StorageClassDef, now_micros,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::cache::MetadataCache;
use crate::metadata::MetadataStore;

/// Chunk allocation strategy
#[derive(Debug, Clone, Copy)]
pub enum AllocationStrategy {
    RoundRobin,
    LeastUsed,
    RandomSpread,
    LocalityAware,
}

/// Chunk server status
#[derive(Debug, Clone)]
pub struct ChunkServerStatus {
    pub id: ChunkServerId,
    pub ip: std::net::IpAddr,
    pub port: u16,
    pub region_id: u8,
    pub rack_id: u16,
    pub total_space: u64,
    pub used_space: u64,
    pub chunk_count: u64,
    pub last_heartbeat: u64,
    pub is_active: bool,
}

/// Manages chunks and their distribution across chunk servers
pub struct ChunkManager {
    metadata: Arc<MetadataStore>,
    cache: Arc<MetadataCache>,
    chunk_servers: Arc<RwLock<HashMap<ChunkServerId, ChunkServerStatus>>>,
    chunk_locations: Arc<DashMap<ChunkId, Vec<ChunkLocation>>>,
    file_chunks: Arc<DashMap<InodeId, Vec<ChunkId>>>,
    allocation_strategy: AllocationStrategy,
}

impl ChunkManager {
    pub fn new(
        metadata: Arc<MetadataStore>,
        cache: Arc<MetadataCache>,
        allocation_strategy: AllocationStrategy,
    ) -> Self {
        Self {
            metadata,
            cache,
            chunk_servers: Arc::new(RwLock::new(HashMap::new())),
            chunk_locations: Arc::new(DashMap::new()),
            file_chunks: Arc::new(DashMap::new()),
            allocation_strategy,
        }
    }
    
    /// Register a chunk server
    pub async fn register_chunk_server(&self, status: ChunkServerStatus) -> Result<()> {
        let mut servers = self.chunk_servers.write().await;
        servers.insert(status.id, status.clone());
        info!("Registered chunk server {} at {}:{}", status.id, status.ip, status.port);
        Ok(())
    }
    
    /// Update chunk server status
    pub async fn update_chunk_server_status(&self, id: ChunkServerId, status: ChunkServerStatus) -> Result<()> {
        let mut servers = self.chunk_servers.write().await;
        servers.insert(id, status);
        debug!("Updated chunk server {} status", id);
        Ok(())
    }
    
    /// Mark chunk server as inactive
    pub async fn deactivate_chunk_server(&self, id: ChunkServerId) -> Result<()> {
        let mut servers = self.chunk_servers.write().await;
        if let Some(server) = servers.get_mut(&id) {
            server.is_active = false;
            warn!("Deactivated chunk server {}", id);
        }
        Ok(())
    }
    
    /// Create a new chunk for a file
    pub async fn create_chunk(
        &self,
        inode: InodeId,
        chunk_index: u32,
        storage_class: &StorageClassDef,
        session_id: SessionId,
    ) -> Result<ChunkId> {
        // Generate new chunk ID
        let chunk_id = self.metadata.next_chunk_id()?;
        
        // Select chunk servers based on storage class
        let servers = self.select_chunk_servers(storage_class).await?;
        if servers.is_empty() {
            return Err(anyhow!("No available chunk servers"));
        }
        
        // Create chunk metadata
        let chunk = ChunkMetadata {
            chunk_id,
            version: 1,
            locked_to: Some(session_id),
            archive_flag: false,
            storage_class_id: storage_class.id,
            locations: servers.clone(),
            ec_info: None,
            last_modified: now_micros(),
        };
        
        // Save to metadata store
        self.metadata.save_chunk(&chunk)?;
        
        // Update cache
        self.cache.insert_chunk(chunk.clone());
        
        // Update chunk locations
        self.chunk_locations.insert(chunk_id, servers);
        
        // Update file chunks mapping
        self.file_chunks.entry(inode).or_default().push(chunk_id);
        
        debug!("Created chunk {} for inode {} at index {}", chunk_id, inode, chunk_index);
        Ok(chunk_id)
    }
    
    /// Get chunk locations
    pub async fn get_chunk_locations(&self, chunk_id: ChunkId) -> Result<Vec<ChunkLocation>> {
        // Check in-memory map first
        if let Some(locations) = self.chunk_locations.get(&chunk_id) {
            return Ok(locations.clone());
        }
        
        // Check cache
        if let Some(chunk) = self.cache.get_chunk(chunk_id) {
            self.chunk_locations.insert(chunk_id, chunk.locations.clone());
            return Ok(chunk.locations);
        }
        
        // Get from metadata store
        if let Some(chunk) = self.metadata.get_chunk(chunk_id)? {
            self.cache.insert_chunk(chunk.clone());
            self.chunk_locations.insert(chunk_id, chunk.locations.clone());
            return Ok(chunk.locations);
        }
        
        Err(anyhow!("Chunk {} not found", chunk_id))
    }
    
    /// Update chunk version
    pub async fn update_chunk_version(
        &self,
        chunk_id: ChunkId,
        new_version: ChunkVersion,
    ) -> Result<()> {
        // Get chunk metadata
        let mut chunk = self.metadata.get_chunk(chunk_id)?
            .ok_or_else(|| anyhow!("Chunk {} not found", chunk_id))?;
        
        // Update version
        chunk.version = new_version;
        chunk.last_modified = now_micros();
        
        // Save updated metadata
        self.metadata.save_chunk(&chunk)?;
        self.cache.insert_chunk(chunk);
        
        debug!("Updated chunk {} to version {}", chunk_id, new_version);
        Ok(())
    }
    
    /// Lock chunk for writing
    pub async fn lock_chunk(&self, chunk_id: ChunkId, session_id: SessionId) -> Result<()> {
        // Get chunk metadata
        let mut chunk = self.metadata.get_chunk(chunk_id)?
            .ok_or_else(|| anyhow!("Chunk {} not found", chunk_id))?;
        
        // Check if already locked
        if let Some(locked_to) = chunk.locked_to {
            if locked_to != session_id {
                return Err(anyhow!("Chunk {} is locked by session {}", chunk_id, locked_to));
            }
        }
        
        // Lock the chunk
        chunk.locked_to = Some(session_id);
        chunk.last_modified = now_micros();
        
        // Save updated metadata
        self.metadata.save_chunk(&chunk)?;
        self.cache.insert_chunk(chunk);
        
        debug!("Locked chunk {} for session {}", chunk_id, session_id);
        Ok(())
    }
    
    /// Unlock chunk
    pub async fn unlock_chunk(&self, chunk_id: ChunkId, session_id: SessionId) -> Result<()> {
        // Get chunk metadata
        let mut chunk = self.metadata.get_chunk(chunk_id)?
            .ok_or_else(|| anyhow!("Chunk {} not found", chunk_id))?;
        
        // Verify lock ownership
        if let Some(locked_to) = chunk.locked_to {
            if locked_to != session_id {
                return Err(anyhow!("Chunk {} is not locked by session {}", chunk_id, session_id));
            }
        }
        
        // Unlock the chunk
        chunk.locked_to = None;
        chunk.last_modified = now_micros();
        
        // Save updated metadata
        self.metadata.save_chunk(&chunk)?;
        self.cache.insert_chunk(chunk);
        
        debug!("Unlocked chunk {} by session {}", chunk_id, session_id);
        Ok(())
    }
    
    /// Get chunks for a file
    pub async fn get_file_chunks(&self, inode: InodeId) -> Result<Vec<ChunkId>> {
        if let Some(chunks) = self.file_chunks.get(&inode) {
            return Ok(chunks.clone());
        }
        
        // TODO: Load from metadata if not in memory
        Ok(vec![])
    }
    
    /// Delete a chunk
    pub async fn delete_chunk(&self, chunk_id: ChunkId) -> Result<()> {
        // Remove from metadata
        self.metadata.delete_chunk(chunk_id)?;
        
        // Remove from cache
        self.cache.remove_chunk(chunk_id);
        
        // Remove from locations
        self.chunk_locations.remove(&chunk_id);
        
        // Remove from file mappings
        for mut entry in self.file_chunks.iter_mut() {
            entry.value_mut().retain(|&id| id != chunk_id);
        }
        
        debug!("Deleted chunk {}", chunk_id);
        Ok(())
    }
    
    /// Select chunk servers based on storage class
    async fn select_chunk_servers(&self, storage_class: &StorageClassDef) -> Result<Vec<ChunkLocation>> {
        let servers = self.chunk_servers.read().await;
        let active_servers: Vec<&ChunkServerStatus> = servers
            .values()
            .filter(|s| s.is_active)
            .collect();
        
        if active_servers.is_empty() {
            return Err(anyhow!("No active chunk servers available"));
        }
        
        // Determine number of replicas needed
        let replica_count = match &storage_class.create_replication {
            mooseng_common::types::ReplicationPolicy::Copies { count } => *count as usize,
            mooseng_common::types::ReplicationPolicy::ErasureCoding { data, parity } => {
                (*data + *parity) as usize
            }
            mooseng_common::types::ReplicationPolicy::XRegion { min_copies, .. } => {
                *min_copies as usize
            }
        };
        
        // Select servers based on strategy
        let selected = match self.allocation_strategy {
            AllocationStrategy::RoundRobin => {
                self.select_round_robin(&active_servers, replica_count)
            }
            AllocationStrategy::LeastUsed => {
                self.select_least_used(&active_servers, replica_count)
            }
            AllocationStrategy::RandomSpread => {
                self.select_random_spread(&active_servers, replica_count)
            }
            AllocationStrategy::LocalityAware => {
                self.select_locality_aware(&active_servers, replica_count, storage_class)
            }
        };
        
        Ok(selected.into_iter().map(|s| ChunkLocation {
            chunk_server_id: s.id,
            ip: match s.ip {
                std::net::IpAddr::V4(ip) => ip,
                std::net::IpAddr::V6(_) => std::net::Ipv4Addr::new(127, 0, 0, 1), // Fallback
            },
            port: s.port,
            region_id: s.region_id,
            rack_id: s.rack_id,
        }).collect())
    }
    
    /// Round-robin server selection
    fn select_round_robin<'a>(&self, servers: &[&'a ChunkServerStatus], count: usize) -> Vec<&'a ChunkServerStatus> {
        servers.iter()
            .cycle()
            .take(count.min(servers.len()))
            .cloned()
            .collect()
    }
    
    /// Select least used servers
    fn select_least_used<'a>(&self, servers: &[&'a ChunkServerStatus], count: usize) -> Vec<&'a ChunkServerStatus> {
        let mut sorted = servers.to_vec();
        sorted.sort_by_key(|s| s.used_space * 100 / s.total_space.max(1));
        sorted.into_iter().take(count).collect()
    }
    
    /// Random server selection with spread
    fn select_random_spread<'a>(&self, servers: &[&'a ChunkServerStatus], count: usize) -> Vec<&'a ChunkServerStatus> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let mut shuffled = servers.to_vec();
        shuffled.shuffle(&mut rng);
        shuffled.into_iter().take(count).collect()
    }
    
    /// Locality-aware server selection
    fn select_locality_aware<'a>(
        &self,
        servers: &[&'a ChunkServerStatus],
        count: usize,
        _storage_class: &StorageClassDef,
    ) -> Vec<&'a ChunkServerStatus> {
        // Group servers by region and rack
        let mut by_region: HashMap<u8, Vec<&ChunkServerStatus>> = HashMap::new();
        for server in servers {
            by_region.entry(server.region_id).or_default().push(server);
        }
        
        // Try to spread across regions if possible
        let mut selected = Vec::new();
        let regions: Vec<_> = by_region.keys().cloned().collect();
        
        for (_i, region_id) in regions.iter().cycle().enumerate() {
            if selected.len() >= count {
                break;
            }
            
            if let Some(region_servers) = by_region.get_mut(region_id) {
                if let Some(server) = region_servers.pop() {
                    selected.push(server);
                }
            }
        }
        
        // Fill remaining from any region
        while selected.len() < count {
            for region_servers in by_region.values_mut() {
                if let Some(server) = region_servers.pop() {
                    selected.push(server);
                    if selected.len() >= count {
                        break;
                    }
                }
            }
            if by_region.values().all(|v| v.is_empty()) {
                break;
            }
        }
        
        selected
    }
    
    /// Perform chunk garbage collection
    pub async fn garbage_collect(&self) -> Result<u64> {
        let mut collected = 0;
        let all_chunks = self.metadata.list_chunks()?;
        
        // Build set of referenced chunks
        let mut referenced = HashSet::new();
        for entry in self.file_chunks.iter() {
            for chunk_id in entry.value() {
                referenced.insert(*chunk_id);
            }
        }
        
        // Delete unreferenced chunks
        for chunk in all_chunks {
            if !referenced.contains(&chunk.chunk_id) {
                self.delete_chunk(chunk.chunk_id).await?;
                collected += 1;
            }
        }
        
        if collected > 0 {
            info!("Garbage collected {} orphaned chunks", collected);
        }
        
        Ok(collected)
    }

    /// Check chunk health across all servers (placeholder implementation)
    pub async fn check_chunk_health(&self) {
        debug!("Running chunk health check across all servers");
        let servers = self.chunk_servers.read().await;
        for (id, server) in servers.iter() {
            if server.is_active {
                debug!("Health check for chunk server {}: OK", id);
            } else {
                warn!("Chunk server {} is inactive", id);
            }
        }
    }

    /// Flush any pending chunk operations
    pub async fn flush_pending(&self) -> Result<()> {
        debug!("Flushing pending chunk operations");
        // In a real implementation, this would flush any pending chunk operations
        // to chunk servers and ensure all operations are committed
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mooseng_common::types::ReplicationPolicy;
    use std::time::Duration;
    use tempfile::TempDir;
    
    async fn create_test_manager() -> (ChunkManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let metadata = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
        let cache = Arc::new(MetadataCache::new(Duration::from_secs(60), 1000));
        let manager = ChunkManager::new(metadata, cache, AllocationStrategy::RoundRobin);
        
        // Register some test chunk servers
        for i in 0..3 {
            let status = ChunkServerStatus {
                id: i,
                ip: std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, i as u8 + 1)),
                port: 9522,
                region_id: i % 2,
                rack_id: i,
                total_space: 1024 * 1024 * 1024 * 100, // 100GB
                used_space: 0,
                chunk_count: 0,
                last_heartbeat: now_micros(),
                is_active: true,
            };
            manager.register_chunk_server(status).await.unwrap();
        }
        
        (manager, temp_dir)
    }
    
    #[tokio::test]
    async fn test_chunk_creation() {
        let (manager, _temp) = create_test_manager().await;
        
        let storage_class = StorageClassDef {
            id: 1,
            name: "default".to_string(),
            admin_only: false,
            create_labels: vec![],
            keep_labels: vec![],
            archive_labels: vec![],
            trash_labels: vec![],
            create_replication: ReplicationPolicy::Copies { count: 2 },
            keep_replication: ReplicationPolicy::Copies { count: 2 },
            archive_replication: ReplicationPolicy::Copies { count: 1 },
            trash_replication: ReplicationPolicy::Copies { count: 1 },
        };
        
        let chunk_id = manager.create_chunk(1, 0, &storage_class, 100).await.unwrap();
        
        // Verify chunk was created
        let locations = manager.get_chunk_locations(chunk_id).await.unwrap();
        assert_eq!(locations.len(), 2);
        
        // Verify file mapping
        let file_chunks = manager.get_file_chunks(1).await.unwrap();
        assert_eq!(file_chunks.len(), 1);
        assert_eq!(file_chunks[0], chunk_id);
    }
    
    #[tokio::test]
    async fn test_chunk_locking() {
        let (manager, _temp) = create_test_manager().await;
        
        let storage_class = StorageClassDef {
            id: 1,
            name: "default".to_string(),
            admin_only: false,
            create_labels: vec![],
            keep_labels: vec![],
            archive_labels: vec![],
            trash_labels: vec![],
            create_replication: ReplicationPolicy::Copies { count: 1 },
            keep_replication: ReplicationPolicy::Copies { count: 1 },
            archive_replication: ReplicationPolicy::Copies { count: 1 },
            trash_replication: ReplicationPolicy::Copies { count: 1 },
        };
        
        let chunk_id = manager.create_chunk(1, 0, &storage_class, 100).await.unwrap();
        
        // Should be locked by creating session
        assert!(manager.lock_chunk(chunk_id, 100).await.is_ok());
        
        // Different session should fail
        assert!(manager.lock_chunk(chunk_id, 200).await.is_err());
        
        // Unlock
        manager.unlock_chunk(chunk_id, 100).await.unwrap();
        
        // Now different session can lock
        assert!(manager.lock_chunk(chunk_id, 200).await.is_ok());
    }
}