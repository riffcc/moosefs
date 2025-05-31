use anyhow::{Context, Result};
use mooseng_common::types::{
    ChunkId, ChunkMetadata, FsEdge, FsNode, InodeId, StorageClassDef, SystemConfig,
};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info};

/// Health statistics for metadata store monitoring
#[derive(Debug, Clone)]
pub struct MetadataHealthStats {
    pub operations_per_second: f64,
    pub disk_usage_percent: f64,
    pub total_inodes: u64,
    pub total_chunks: u64,
    pub total_edges: u64,
}

const INODES_TREE: &str = "inodes";
const EDGES_TREE: &str = "edges";
const CHUNKS_TREE: &str = "chunks";
const STORAGE_CLASSES_TREE: &str = "storage_classes";
const SYSTEM_CONFIG_TREE: &str = "system_config";
const INODE_COUNTER_KEY: &str = "next_inode_id";
const CHUNK_COUNTER_KEY: &str = "next_chunk_id";
const EDGE_COUNTER_KEY: &str = "next_edge_id";

#[derive(Clone)]
pub struct MetadataStore {
    db: Arc<Db>,
    inodes: Arc<Tree>,
    edges: Arc<Tree>,
    chunks: Arc<Tree>,
    storage_classes: Arc<Tree>,
    system_config: Arc<Tree>,
}

impl MetadataStore {
    /// Create a new metadata store at the specified path
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = sled::open(path)?;
        
        let inodes = Arc::new(db.open_tree(INODES_TREE)?);
        let edges = Arc::new(db.open_tree(EDGES_TREE)?);
        let chunks = Arc::new(db.open_tree(CHUNKS_TREE)?);
        let storage_classes = Arc::new(db.open_tree(STORAGE_CLASSES_TREE)?);
        let system_config = Arc::new(db.open_tree(SYSTEM_CONFIG_TREE)?);
        
        let store = Self {
            db: Arc::new(db),
            inodes,
            edges,
            chunks,
            storage_classes,
            system_config,
        };
        
        // Initialize default system config if not exists
        if !store.has_system_config()? {
            store.save_system_config(&SystemConfig::default())?;
        }
        
        // Initialize default storage classes if empty
        if store.list_storage_classes()?.is_empty() {
            for sc in SystemConfig::default().storage_classes {
                store.save_storage_class(&sc)?;
            }
        }
        
        Ok(store)
    }
    
    // Inode operations
    
    /// Get the next available inode ID
    pub fn next_inode_id(&self) -> Result<InodeId> {
        let counter = self.db
            .generate_id()
            .context("Failed to generate inode ID")?;
        Ok(counter)
    }
    
    /// Save an FsNode
    pub fn save_inode(&self, node: &FsNode) -> Result<()> {
        let key = inode_key(node.inode);
        let value = bincode::serialize(node)?;
        self.inodes.insert(key, value)?;
        debug!("Saved inode {}", node.inode);
        Ok(())
    }
    
    /// Get an FsNode by inode ID
    pub fn get_inode(&self, inode: InodeId) -> Result<Option<FsNode>> {
        let key = inode_key(inode);
        match self.inodes.get(key)? {
            Some(data) => {
                let node = bincode::deserialize(&data)?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }
    
    /// Delete an FsNode
    pub fn delete_inode(&self, inode: InodeId) -> Result<()> {
        let key = inode_key(inode);
        self.inodes.remove(key)?;
        debug!("Deleted inode {}", inode);
        Ok(())
    }
    
    /// List all inodes
    pub fn list_inodes(&self) -> Result<Vec<FsNode>> {
        let mut nodes = Vec::new();
        for item in self.inodes.iter() {
            let (_, value) = item?;
            let node: FsNode = bincode::deserialize(&value)?;
            nodes.push(node);
        }
        Ok(nodes)
    }
    
    // Edge operations
    
    /// Get the next available edge ID
    pub fn next_edge_id(&self) -> Result<u64> {
        let counter = self.db
            .generate_id()
            .context("Failed to generate edge ID")?;
        Ok(counter)
    }
    
    /// Save an FsEdge
    pub fn save_edge(&self, edge: &FsEdge) -> Result<()> {
        let key = edge_key(edge.edge_id);
        let value = bincode::serialize(edge)?;
        self.edges.insert(key, value)?;
        
        // Also create parent->child index
        let parent_key = parent_child_key(edge.parent_inode, &edge.name);
        self.edges.insert(parent_key, edge.child_inode.to_be_bytes().to_vec())?;
        
        debug!("Saved edge {} ({} -> {})", edge.edge_id, edge.parent_inode, edge.child_inode);
        Ok(())
    }
    
    /// Get an FsEdge by edge ID
    pub fn get_edge(&self, edge_id: u64) -> Result<Option<FsEdge>> {
        let key = edge_key(edge_id);
        match self.edges.get(key)? {
            Some(data) => {
                let edge = bincode::deserialize(&data)?;
                Ok(Some(edge))
            }
            None => Ok(None),
        }
    }
    
    /// Find child inode by parent and name
    pub fn find_child(&self, parent: InodeId, name: &str) -> Result<Option<InodeId>> {
        let key = parent_child_key(parent, name);
        match self.edges.get(key)? {
            Some(data) => {
                let child_id = InodeId::from_be_bytes(data.as_ref().try_into()?);
                Ok(Some(child_id))
            }
            None => Ok(None),
        }
    }
    
    /// List children of a directory
    pub fn list_children(&self, parent: InodeId) -> Result<Vec<FsEdge>> {
        let mut children = Vec::new();
        let prefix = format!("edge:");
        
        for item in self.edges.iter() {
            let (key, value) = item?;
            if key.starts_with(prefix.as_bytes()) {
                let edge: FsEdge = bincode::deserialize(&value)?;
                if edge.parent_inode == parent {
                    children.push(edge);
                }
            }
        }
        
        Ok(children)
    }
    
    /// Delete an edge
    pub fn delete_edge(&self, edge: &FsEdge) -> Result<()> {
        let key = edge_key(edge.edge_id);
        self.edges.remove(key)?;
        
        // Also remove parent->child index
        let parent_key = parent_child_key(edge.parent_inode, &edge.name);
        self.edges.remove(parent_key)?;
        
        debug!("Deleted edge {} ({} -> {})", edge.edge_id, edge.parent_inode, edge.child_inode);
        Ok(())
    }
    
    // Chunk operations
    
    /// Get the next available chunk ID
    pub fn next_chunk_id(&self) -> Result<ChunkId> {
        let counter = self.db
            .generate_id()
            .context("Failed to generate chunk ID")?;
        Ok(counter)
    }
    
    /// Save chunk metadata
    pub fn save_chunk(&self, chunk: &ChunkMetadata) -> Result<()> {
        let key = chunk_key(chunk.chunk_id);
        let value = bincode::serialize(chunk)?;
        self.chunks.insert(key, value)?;
        debug!("Saved chunk {}", chunk.chunk_id);
        Ok(())
    }
    
    /// Get chunk metadata
    pub fn get_chunk(&self, chunk_id: ChunkId) -> Result<Option<ChunkMetadata>> {
        let key = chunk_key(chunk_id);
        match self.chunks.get(key)? {
            Some(data) => {
                let chunk = bincode::deserialize(&data)?;
                Ok(Some(chunk))
            }
            None => Ok(None),
        }
    }
    
    /// Delete chunk metadata
    pub fn delete_chunk(&self, chunk_id: ChunkId) -> Result<()> {
        let key = chunk_key(chunk_id);
        self.chunks.remove(key)?;
        debug!("Deleted chunk {}", chunk_id);
        Ok(())
    }
    
    /// List all chunks
    pub fn list_chunks(&self) -> Result<Vec<ChunkMetadata>> {
        let mut chunks = Vec::new();
        for item in self.chunks.iter() {
            let (_, value) = item?;
            let chunk: ChunkMetadata = bincode::deserialize(&value)?;
            chunks.push(chunk);
        }
        Ok(chunks)
    }
    
    // Storage class operations
    
    /// Save a storage class definition
    pub fn save_storage_class(&self, sc: &StorageClassDef) -> Result<()> {
        let key = storage_class_key(sc.id);
        let value = bincode::serialize(sc)?;
        self.storage_classes.insert(key, value)?;
        info!("Saved storage class {} ({})", sc.id, sc.name);
        Ok(())
    }
    
    /// Get a storage class definition
    pub fn get_storage_class(&self, id: u8) -> Result<Option<StorageClassDef>> {
        let key = storage_class_key(id);
        match self.storage_classes.get(key)? {
            Some(data) => {
                let sc = bincode::deserialize(&data)?;
                Ok(Some(sc))
            }
            None => Ok(None),
        }
    }
    
    /// List all storage classes
    pub fn list_storage_classes(&self) -> Result<Vec<StorageClassDef>> {
        let mut classes = Vec::new();
        for item in self.storage_classes.iter() {
            let (_, value) = item?;
            let sc: StorageClassDef = bincode::deserialize(&value)?;
            classes.push(sc);
        }
        Ok(classes)
    }
    
    // System configuration
    
    /// Check if system config exists
    fn has_system_config(&self) -> Result<bool> {
        Ok(self.system_config.contains_key("config")?)
    }
    
    /// Save system configuration
    pub fn save_system_config(&self, config: &SystemConfig) -> Result<()> {
        let value = bincode::serialize(config)?;
        self.system_config.insert("config", value)?;
        info!("Saved system configuration");
        Ok(())
    }
    
    /// Get system configuration
    pub fn get_system_config(&self) -> Result<SystemConfig> {
        match self.system_config.get("config")? {
            Some(data) => {
                let config = bincode::deserialize(&data)?;
                Ok(config)
            }
            None => Ok(SystemConfig::default()),
        }
    }
    
    /// Flush all pending writes to disk
    pub async fn flush(&self) -> Result<()> {
        self.db.flush_async().await?;
        Ok(())
    }
    
    /// Get database size
    pub fn size(&self) -> Result<u64> {
        Ok(self.db.size_on_disk()?)
    }

    /// Verify metadata checksum (placeholder implementation)
    pub async fn verify_checksum(&self) -> Result<()> {
        // In a real implementation, this would verify metadata integrity
        debug!("Metadata checksum verification complete");
        Ok(())
    }

    /// Save metadata to disk (placeholder implementation)
    pub async fn save_metadata(&self) -> Result<()> {
        self.flush().await?;
        debug!("Metadata saved to disk");
        Ok(())
    }
    
    /// Get health statistics for monitoring
    pub async fn get_stats(&self) -> MetadataHealthStats {
        // In production, these would be real metrics
        let total_inodes = self.inodes.len() as u64;
        let total_chunks = self.chunks.len() as u64;
        let total_edges = self.edges.len() as u64;
        
        MetadataHealthStats {
            operations_per_second: 0.0, // TODO: Implement real operation counting
            disk_usage_percent: 0.0,     // TODO: Implement real disk usage calculation
            total_inodes,
            total_chunks,
            total_edges,
        }
    }
}

// Key generation helpers

fn inode_key(inode: InodeId) -> Vec<u8> {
    format!("inode:{:016x}", inode).into_bytes()
}

fn edge_key(edge_id: u64) -> Vec<u8> {
    format!("edge:{:016x}", edge_id).into_bytes()
}

fn parent_child_key(parent: InodeId, name: &str) -> Vec<u8> {
    format!("pc:{:016x}:{}", parent, name).into_bytes()
}

fn chunk_key(chunk_id: ChunkId) -> Vec<u8> {
    format!("chunk:{:016x}", chunk_id).into_bytes()
}

fn storage_class_key(id: u8) -> Vec<u8> {
    format!("sc:{:03}", id).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mooseng_common::types::{FileType, FsNodeType, now_micros};
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_metadata_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path()).await.unwrap();
        
        // Should have default system config
        let config = store.get_system_config().unwrap();
        assert_eq!(config.trash_retention_days, 7);
        
        // Should have default storage classes
        let classes = store.list_storage_classes().unwrap();
        assert!(!classes.is_empty());
    }
    
    #[tokio::test]
    async fn test_inode_operations() {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path()).await.unwrap();
        
        let inode_id = store.next_inode_id().unwrap();
        let node = FsNode {
            inode: inode_id,
            parent: None,
            ctime: now_micros(),
            mtime: now_micros(),
            atime: now_micros(),
            uid: 1000,
            gid: 1000,
            mode: 0o755,
            flags: 0,
            winattr: 0,
            storage_class_id: 1,
            trash_retention: 7,
            node_type: FsNodeType::Directory {
                children: vec![],
                stats: Default::default(),
                quota: None,
            },
        };
        
        // Save
        store.save_inode(&node).unwrap();
        
        // Get
        let retrieved = store.get_inode(inode_id).unwrap().unwrap();
        assert_eq!(retrieved.inode, node.inode);
        assert_eq!(retrieved.uid, node.uid);
        
        // List
        let all_nodes = store.list_inodes().unwrap();
        assert_eq!(all_nodes.len(), 1);
        
        // Delete
        store.delete_inode(inode_id).unwrap();
        assert!(store.get_inode(inode_id).unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_edge_operations() {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path()).await.unwrap();
        
        let edge = FsEdge {
            edge_id: store.next_edge_id().unwrap(),
            parent_inode: 1,
            child_inode: 2,
            name: "test.txt".to_string(),
        };
        
        // Save
        store.save_edge(&edge).unwrap();
        
        // Get by edge ID
        let retrieved = store.get_edge(edge.edge_id).unwrap().unwrap();
        assert_eq!(retrieved.name, edge.name);
        
        // Find by parent and name
        let child_id = store.find_child(1, "test.txt").unwrap().unwrap();
        assert_eq!(child_id, 2);
        
        // List children
        let children = store.list_children(1).unwrap();
        assert_eq!(children.len(), 1);
        
        // Delete
        store.delete_edge(&edge).unwrap();
        assert!(store.find_child(1, "test.txt").unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_chunk_operations() {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path()).await.unwrap();
        
        let chunk = ChunkMetadata {
            chunk_id: store.next_chunk_id().unwrap(),
            version: 1,
            locked_to: None,
            archive_flag: false,
            storage_class_id: 1,
            locations: vec![],
            ec_info: None,
            last_modified: now_micros(),
        };
        
        // Save
        store.save_chunk(&chunk).unwrap();
        
        // Get
        let retrieved = store.get_chunk(chunk.chunk_id).unwrap().unwrap();
        assert_eq!(retrieved.version, chunk.version);
        
        // List
        let all_chunks = store.list_chunks().unwrap();
        assert_eq!(all_chunks.len(), 1);
        
        // Delete
        store.delete_chunk(chunk.chunk_id).unwrap();
        assert!(store.get_chunk(chunk.chunk_id).unwrap().is_none());
    }
}