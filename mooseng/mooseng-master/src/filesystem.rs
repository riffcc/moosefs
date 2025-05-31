use anyhow::{anyhow, bail, Context, Result};
use mooseng_common::types::{
    DirStats, FileType, FsEdge, FsNode, FsNodeType, InodeId, MFS_NAME_MAX, MFS_PATH_MAX,
    MFS_ROOT_ID, Status, now_micros,
};
use std::collections::VecDeque;
use std::path::{Component, Path};
use tracing::{debug, trace, warn};

use crate::cache::MetadataCache;
use crate::metadata::MetadataStore;

/// Filesystem operations handler
pub struct FileSystem {
    metadata: MetadataStore,
    cache: MetadataCache,
}

impl FileSystem {
    pub fn new(metadata: MetadataStore, cache: MetadataCache) -> Self {
        Self { metadata, cache }
    }
    
    /// Initialize the filesystem with root directory
    pub async fn initialize(&self) -> Result<()> {
        // Check if root already exists
        if self.get_inode(MFS_ROOT_ID).await?.is_some() {
            debug!("Root directory already exists");
            return Ok(());
        }
        
        // Create root directory
        let root = FsNode {
            inode: MFS_ROOT_ID,
            parent: None,
            ctime: now_micros(),
            mtime: now_micros(),
            atime: now_micros(),
            uid: 0,
            gid: 0,
            mode: 0o755,
            flags: 0,
            winattr: 0,
            storage_class_id: 1,
            trash_retention: 7,
            node_type: FsNodeType::Directory {
                children: vec![],
                stats: DirStats::default(),
                quota: None,
            },
        };
        
        self.metadata.save_inode(&root)?;
        self.cache.insert_inode(root);
        
        debug!("Initialized filesystem with root directory");
        Ok(())
    }
    
    /// Resolve a path to an inode ID
    pub async fn resolve_path(&self, path: &str) -> Result<Option<InodeId>> {
        // Normalize the path
        let normalized = self.normalize_path(path)?;
        
        // Check cache first
        if let Some(inode) = self.cache.get_path_inode(&normalized) {
            return Ok(Some(inode));
        }
        
        // Parse path components
        let components = self.parse_path(&normalized)?;
        if components.is_empty() {
            // Root directory
            return Ok(Some(MFS_ROOT_ID));
        }
        
        // Traverse from root
        let mut current_inode = MFS_ROOT_ID;
        let mut current_path = String::from("/");
        
        for component in components {
            // Get current node
            let node = match self.get_inode(current_inode).await? {
                Some(n) => n,
                None => return Ok(None),
            };
            
            // Verify it's a directory
            match &node.node_type {
                FsNodeType::Directory { .. } => {},
                _ => return Ok(None), // Not a directory
            }
            
            // Find child
            match self.metadata.find_child(current_inode, &component)? {
                Some(child_inode) => {
                    current_inode = child_inode;
                    if current_path != "/" {
                        current_path.push('/');
                    }
                    current_path.push_str(&component);
                    
                    // Cache intermediate paths
                    self.cache.insert_path_inode(current_path.clone(), current_inode);
                }
                None => return Ok(None), // Child not found
            }
        }
        
        // Cache the full path
        self.cache.insert_path_inode(normalized, current_inode);
        Ok(Some(current_inode))
    }
    
    /// Create a new file
    pub async fn create_file(
        &self,
        parent_path: &str,
        name: &str,
        mode: u16,
        uid: u32,
        gid: u32,
        storage_class_id: u8,
    ) -> Result<InodeId> {
        self.validate_name(name)?;
        
        // Resolve parent directory
        let parent_inode = self.resolve_path(parent_path).await?
            .ok_or_else(|| anyhow!("Parent directory not found"))?;
        
        // Get parent node
        let mut parent = self.get_inode(parent_inode).await?
            .ok_or_else(|| anyhow!("Parent inode not found"))?;
        
        // Verify parent is a directory
        let (mut children, mut stats) = match &mut parent.node_type {
            FsNodeType::Directory { children, stats, .. } => {
                (children.clone(), stats.clone())
            }
            _ => bail!("Parent is not a directory"),
        };
        
        // Check if file already exists
        if self.metadata.find_child(parent_inode, name)?.is_some() {
            bail!("File already exists");
        }
        
        // Create new file node
        let inode_id = self.metadata.next_inode_id()?;
        let now = now_micros();
        let file_node = FsNode {
            inode: inode_id,
            parent: Some(parent_inode),
            ctime: now,
            mtime: now,
            atime: now,
            uid,
            gid,
            mode: mode & 0o777,
            flags: 0,
            winattr: 0,
            storage_class_id,
            trash_retention: parent.trash_retention,
            node_type: FsNodeType::File {
                length: 0,
                chunk_ids: vec![],
                session_id: None,
            },
        };
        
        // Create edge
        let edge = FsEdge {
            edge_id: self.metadata.next_edge_id()?,
            parent_inode,
            child_inode: inode_id,
            name: name.to_string(),
        };
        
        // Update parent stats
        stats.total_files += 1;
        children.push(edge.clone());
        
        // Save everything
        self.metadata.save_inode(&file_node)?;
        self.metadata.save_edge(&edge)?;
        
        // Update parent
        parent.mtime = now;
        parent.node_type = FsNodeType::Directory {
            children,
            stats,
            quota: match &parent.node_type {
                FsNodeType::Directory { quota, .. } => quota.clone(),
                _ => None,
            },
        };
        self.metadata.save_inode(&parent)?;
        
        // Update caches
        self.cache.insert_inode(file_node);
        self.cache.insert_inode(parent);
        
        debug!("Created file {} in directory {} (inode {})", name, parent_path, inode_id);
        Ok(inode_id)
    }
    
    /// Create a new directory
    pub async fn create_directory(
        &self,
        parent_path: &str,
        name: &str,
        mode: u16,
        uid: u32,
        gid: u32,
        storage_class_id: u8,
    ) -> Result<InodeId> {
        self.validate_name(name)?;
        
        // Resolve parent directory
        let parent_inode = self.resolve_path(parent_path).await?
            .ok_or_else(|| anyhow!("Parent directory not found"))?;
        
        // Get parent node
        let mut parent = self.get_inode(parent_inode).await?
            .ok_or_else(|| anyhow!("Parent inode not found"))?;
        
        // Verify parent is a directory
        let (mut children, mut stats) = match &mut parent.node_type {
            FsNodeType::Directory { children, stats, .. } => {
                (children.clone(), stats.clone())
            }
            _ => bail!("Parent is not a directory"),
        };
        
        // Check if directory already exists
        if self.metadata.find_child(parent_inode, name)?.is_some() {
            bail!("Directory already exists");
        }
        
        // Create new directory node
        let inode_id = self.metadata.next_inode_id()?;
        let now = now_micros();
        let dir_node = FsNode {
            inode: inode_id,
            parent: Some(parent_inode),
            ctime: now,
            mtime: now,
            atime: now,
            uid,
            gid,
            mode: (mode & 0o777) | 0o40000, // Add directory bit
            flags: 0,
            winattr: 0,
            storage_class_id,
            trash_retention: parent.trash_retention,
            node_type: FsNodeType::Directory {
                children: vec![],
                stats: DirStats::default(),
                quota: None,
            },
        };
        
        // Create edge
        let edge = FsEdge {
            edge_id: self.metadata.next_edge_id()?,
            parent_inode,
            child_inode: inode_id,
            name: name.to_string(),
        };
        
        // Update parent stats
        stats.total_dirs += 1;
        children.push(edge.clone());
        
        // Save everything
        self.metadata.save_inode(&dir_node)?;
        self.metadata.save_edge(&edge)?;
        
        // Update parent
        parent.mtime = now;
        parent.node_type = FsNodeType::Directory {
            children,
            stats,
            quota: match &parent.node_type {
                FsNodeType::Directory { quota, .. } => quota.clone(),
                _ => None,
            },
        };
        self.metadata.save_inode(&parent)?;
        
        // Update caches
        self.cache.insert_inode(dir_node);
        self.cache.insert_inode(parent);
        
        debug!("Created directory {} in {} (inode {})", name, parent_path, inode_id);
        Ok(inode_id)
    }
    
    /// List directory contents
    pub async fn list_directory(&self, path: &str) -> Result<Vec<(String, FsNode)>> {
        // Resolve directory
        let dir_inode = self.resolve_path(path).await?
            .ok_or_else(|| anyhow!("Directory not found"))?;
        
        // Get directory node
        let dir_node = self.get_inode(dir_inode).await?
            .ok_or_else(|| anyhow!("Directory inode not found"))?;
        
        // Verify it's a directory
        match &dir_node.node_type {
            FsNodeType::Directory { .. } => {},
            _ => bail!("Not a directory"),
        }
        
        // Get children edges
        let edges = self.metadata.list_children(dir_inode)?;
        
        // Get child nodes
        let mut entries = Vec::new();
        for edge in edges {
            if let Some(child) = self.get_inode(edge.child_inode).await? {
                entries.push((edge.name, child));
            }
        }
        
        // Sort by name
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        
        debug!("Listed {} entries in directory {}", entries.len(), path);
        Ok(entries)
    }
    
    /// Delete a file or empty directory
    pub async fn delete(&self, path: &str) -> Result<()> {
        // Resolve path
        let inode = self.resolve_path(path).await?
            .ok_or_else(|| anyhow!("Path not found"))?;
        
        // Cannot delete root
        if inode == MFS_ROOT_ID {
            bail!("Cannot delete root directory");
        }
        
        // Get node
        let node = self.get_inode(inode).await?
            .ok_or_else(|| anyhow!("Inode not found"))?;
        
        // Check if directory is empty
        if let FsNodeType::Directory { children, .. } = &node.node_type {
            if !children.is_empty() {
                bail!("Directory not empty");
            }
        }
        
        // Get parent
        let parent_inode = node.parent
            .ok_or_else(|| anyhow!("No parent found"))?;
        
        let mut parent = self.get_inode(parent_inode).await?
            .ok_or_else(|| anyhow!("Parent inode not found"))?;
        
        // Find and remove edge
        let edges = self.metadata.list_children(parent_inode)?;
        for edge in edges {
            if edge.child_inode == inode {
                self.metadata.delete_edge(&edge)?;
                
                // Update parent stats
                match &mut parent.node_type {
                    FsNodeType::Directory { stats, children, .. } => {
                        match node.node_type {
                            FsNodeType::File { .. } => stats.total_files -= 1,
                            FsNodeType::Directory { .. } => stats.total_dirs -= 1,
                            _ => {},
                        }
                        children.retain(|e| e.edge_id != edge.edge_id);
                    }
                    _ => {},
                }
                
                break;
            }
        }
        
        // Delete the inode
        self.metadata.delete_inode(inode)?;
        
        // Update parent
        parent.mtime = now_micros();
        self.metadata.save_inode(&parent)?;
        
        // Update caches
        self.cache.remove_inode(inode);
        self.cache.insert_inode(parent);
        self.cache.remove_path(path);
        
        debug!("Deleted {} (inode {})", path, inode);
        Ok(())
    }
    
    /// Check access permissions
    pub async fn check_access(
        &self,
        path: &str,
        uid: u32,
        gid: u32,
        mode: u8,
    ) -> Result<bool> {
        // Resolve path
        let inode = self.resolve_path(path).await?
            .ok_or_else(|| anyhow!("Path not found"))?;
        
        // Get node
        let node = self.get_inode(inode).await?
            .ok_or_else(|| anyhow!("Inode not found"))?;
        
        // Root has all permissions
        if uid == 0 {
            return Ok(true);
        }
        
        // Check owner permissions
        if uid == node.uid {
            let owner_perms = (node.mode >> 6) & 0o7;
            return Ok(self.has_permission(owner_perms, mode));
        }
        
        // Check group permissions
        if gid == node.gid {
            let group_perms = (node.mode >> 3) & 0o7;
            return Ok(self.has_permission(group_perms, mode));
        }
        
        // Check other permissions
        let other_perms = node.mode & 0o7;
        Ok(self.has_permission(other_perms, mode))
    }
    
    /// Get inode information
    pub async fn get_inode(&self, inode: InodeId) -> Result<Option<FsNode>> {
        // Check cache first
        if let Some(node) = self.cache.get_inode(inode) {
            return Ok(Some(node));
        }
        
        // Get from metadata store
        if let Some(node) = self.metadata.get_inode(inode)? {
            // Update cache
            self.cache.insert_inode(node.clone());
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }
    
    /// Normalize a path
    fn normalize_path(&self, path: &str) -> Result<String> {
        if path.len() > MFS_PATH_MAX {
            bail!("Path too long");
        }
        
        let path = Path::new(path);
        let mut components = Vec::new();
        
        for component in path.components() {
            match component {
                Component::RootDir => {},
                Component::Normal(name) => {
                    if let Some(name_str) = name.to_str() {
                        components.push(name_str);
                    } else {
                        bail!("Invalid UTF-8 in path");
                    }
                }
                Component::ParentDir => {
                    components.pop();
                }
                Component::CurDir => {},
                Component::Prefix(_) => bail!("Windows paths not supported"),
            }
        }
        
        if components.is_empty() {
            Ok("/".to_string())
        } else {
            Ok(format!("/{}", components.join("/")))
        }
    }
    
    /// Parse path into components
    fn parse_path(&self, path: &str) -> Result<Vec<String>> {
        if path == "/" {
            return Ok(vec![]);
        }
        
        let trimmed = path.trim_start_matches('/').trim_end_matches('/');
        if trimmed.is_empty() {
            return Ok(vec![]);
        }
        
        Ok(trimmed.split('/').map(|s| s.to_string()).collect())
    }
    
    /// Validate a file/directory name
    fn validate_name(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            bail!("Name cannot be empty");
        }
        
        if name.len() > MFS_NAME_MAX {
            bail!("Name too long");
        }
        
        if name == "." || name == ".." {
            bail!("Invalid name");
        }
        
        if name.contains('/') {
            bail!("Name cannot contain slash");
        }
        
        if name.contains('\0') {
            bail!("Name cannot contain null character");
        }
        
        Ok(())
    }
    
    /// Check if permission bits allow the requested mode
    fn has_permission(&self, perms: u16, mode: u8) -> bool {
        if mode & 4 != 0 && perms & 4 == 0 {
            return false; // Read requested but not allowed
        }
        if mode & 2 != 0 && perms & 2 == 0 {
            return false; // Write requested but not allowed
        }
        if mode & 1 != 0 && perms & 1 == 0 {
            return false; // Execute requested but not allowed
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;
    
    async fn create_test_fs() -> (FileSystem, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let metadata = MetadataStore::new(temp_dir.path()).unwrap();
        let cache = MetadataCache::new(Duration::from_secs(60), 1000);
        let fs = FileSystem::new(metadata, cache);
        fs.initialize().await.unwrap();
        (fs, temp_dir)
    }
    
    #[tokio::test]
    async fn test_filesystem_initialization() {
        let (fs, _temp) = create_test_fs().await;
        
        // Root should exist
        let root_inode = fs.resolve_path("/").await.unwrap().unwrap();
        assert_eq!(root_inode, MFS_ROOT_ID);
        
        let root = fs.get_inode(MFS_ROOT_ID).await.unwrap().unwrap();
        match root.node_type {
            FsNodeType::Directory { .. } => {},
            _ => panic!("Root should be a directory"),
        }
    }
    
    #[tokio::test]
    async fn test_create_file() {
        let (fs, _temp) = create_test_fs().await;
        
        // Create a file
        let file_inode = fs.create_file("/", "test.txt", 0o644, 1000, 1000, 1).await.unwrap();
        
        // Verify file exists
        let resolved = fs.resolve_path("/test.txt").await.unwrap().unwrap();
        assert_eq!(resolved, file_inode);
        
        // Verify file attributes
        let file = fs.get_inode(file_inode).await.unwrap().unwrap();
        assert_eq!(file.uid, 1000);
        assert_eq!(file.gid, 1000);
        assert_eq!(file.mode & 0o777, 0o644);
        
        match file.node_type {
            FsNodeType::File { length, .. } => assert_eq!(length, 0),
            _ => panic!("Should be a file"),
        }
    }
    
    #[tokio::test]
    async fn test_create_directory() {
        let (fs, _temp) = create_test_fs().await;
        
        // Create a directory
        let dir_inode = fs.create_directory("/", "testdir", 0o755, 1000, 1000, 1).await.unwrap();
        
        // Create a file in the directory
        let file_inode = fs.create_file("/testdir", "file.txt", 0o644, 1000, 1000, 1).await.unwrap();
        
        // Verify paths resolve correctly
        assert_eq!(fs.resolve_path("/testdir").await.unwrap().unwrap(), dir_inode);
        assert_eq!(fs.resolve_path("/testdir/file.txt").await.unwrap().unwrap(), file_inode);
    }
    
    #[tokio::test]
    async fn test_list_directory() {
        let (fs, _temp) = create_test_fs().await;
        
        // Create some files and directories
        fs.create_file("/", "file1.txt", 0o644, 1000, 1000, 1).await.unwrap();
        fs.create_file("/", "file2.txt", 0o644, 1000, 1000, 1).await.unwrap();
        fs.create_directory("/", "dir1", 0o755, 1000, 1000, 1).await.unwrap();
        fs.create_directory("/", "dir2", 0o755, 1000, 1000, 1).await.unwrap();
        
        // List root directory
        let entries = fs.list_directory("/").await.unwrap();
        assert_eq!(entries.len(), 4);
        
        // Verify entries are sorted
        let names: Vec<String> = entries.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(names, vec!["dir1", "dir2", "file1.txt", "file2.txt"]);
    }
    
    #[tokio::test]
    async fn test_delete_file() {
        let (fs, _temp) = create_test_fs().await;
        
        // Create and delete a file
        fs.create_file("/", "temp.txt", 0o644, 1000, 1000, 1).await.unwrap();
        fs.delete("/temp.txt").await.unwrap();
        
        // Verify file is gone
        assert!(fs.resolve_path("/temp.txt").await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_permission_checks() {
        let (fs, _temp) = create_test_fs().await;
        
        // Create a file with specific permissions
        fs.create_file("/", "private.txt", 0o600, 1000, 1000, 1).await.unwrap();
        
        // Owner should have read/write access
        assert!(fs.check_access("/private.txt", 1000, 1000, 6).await.unwrap());
        
        // Others should not have access
        assert!(!fs.check_access("/private.txt", 2000, 2000, 4).await.unwrap());
        
        // Root should always have access
        assert!(fs.check_access("/private.txt", 0, 0, 7).await.unwrap());
    }
}