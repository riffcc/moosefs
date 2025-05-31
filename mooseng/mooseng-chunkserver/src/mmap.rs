use crate::error::{ChunkServerError, Result};
use memmap2::{Mmap, MmapOptions};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use bytes::Bytes;
use tracing::{debug, error, warn};

/// Memory-mapped file handle with metadata
#[derive(Debug)]
pub struct MmapFile {
    path: PathBuf,
    mmap: Mmap,
    size: u64,
    access_count: std::sync::atomic::AtomicU64,
    last_accessed: std::sync::atomic::AtomicU64,
}

impl MmapFile {
    /// Create a new memory-mapped file
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        
        let file = File::open(&path).map_err(|e| {
            ChunkServerError::Io(e)
        })?;
        
        let metadata = file.metadata().map_err(|e| {
            ChunkServerError::Io(e)
        })?;
        
        let size = metadata.len();
        
        if size == 0 {
            return Err(ChunkServerError::MemoryMapping(
                "Cannot memory-map empty file".to_string()
            ));
        }
        
        // Create memory mapping
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|e| {
                    ChunkServerError::MemoryMapping(format!("Failed to create memory mapping: {}", e))
                })?
        };
        
        debug!("Created memory mapping for {} ({} bytes)", path.display(), size);
        
        Ok(Self {
            path,
            mmap,
            size,
            access_count: std::sync::atomic::AtomicU64::new(0),
            last_accessed: std::sync::atomic::AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
        })
    }
    
    /// Get the full content as bytes
    pub fn data(&self) -> Bytes {
        self.touch();
        Bytes::copy_from_slice(&self.mmap[..])
    }
    
    /// Get a slice of the memory-mapped data
    pub fn slice(&self, offset: u64, length: u64) -> Result<Bytes> {
        self.touch();
        
        let start = offset as usize;
        let end = (offset + length) as usize;
        
        if end > self.mmap.len() {
            return Err(ChunkServerError::InvalidOperation(
                format!("Slice bounds out of range: {}-{} for file of size {}", 
                        start, end, self.mmap.len())
            ));
        }
        
        Ok(Bytes::copy_from_slice(&self.mmap[start..end]))
    }
    
    /// Get file size
    pub fn size(&self) -> u64 {
        self.size
    }
    
    /// Get file path
    pub fn path(&self) -> &Path {
        &self.path
    }
    
    /// Get access count
    pub fn access_count(&self) -> u64 {
        self.access_count.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    /// Get last access time (unix timestamp)
    pub fn last_accessed(&self) -> u64 {
        self.last_accessed.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    /// Record an access to this file
    fn touch(&self) {
        self.access_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.last_accessed.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            std::sync::atomic::Ordering::Relaxed
        );
    }
    
    /// Check if file is considered hot (frequently accessed)
    pub fn is_hot(&self, threshold_accesses: u64, time_window_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let last_access = self.last_accessed();
        let access_count = self.access_count();
        
        // If accessed recently and frequently
        if now.saturating_sub(last_access) < time_window_secs {
            return access_count > threshold_accesses;
        }
        
        false
    }
}

/// Configuration for memory-mapped file manager
#[derive(Debug, Clone)]
pub struct MmapConfig {
    /// Maximum number of memory-mapped files to keep open
    pub max_files: usize,
    /// Minimum file size to consider for memory mapping (bytes)
    pub min_file_size: u64,
    /// Maximum file size to consider for memory mapping (bytes)
    pub max_file_size: u64,
    /// Access count threshold to consider a file "hot"
    pub hot_access_threshold: u64,
    /// Time window for hot file detection (seconds)
    pub hot_time_window_secs: u64,
    /// Cleanup interval for evicting cold files (seconds)
    pub cleanup_interval_secs: u64,
}

impl Default for MmapConfig {
    fn default() -> Self {
        Self {
            max_files: 1024,
            min_file_size: 4096,        // 4KB
            max_file_size: 128 * 1024 * 1024, // 128MB
            hot_access_threshold: 10,
            hot_time_window_secs: 300,  // 5 minutes
            cleanup_interval_secs: 60,  // 1 minute
        }
    }
}

/// Manager for memory-mapped files with LRU eviction
pub struct MmapManager {
    config: MmapConfig,
    files: Arc<DashMap<PathBuf, Arc<RwLock<Option<Arc<MmapFile>>>>>>,
    eviction_queue: Arc<RwLock<Vec<PathBuf>>>, // Simple LRU queue
}

impl MmapManager {
    /// Create a new memory-mapped file manager
    pub fn new(config: MmapConfig) -> Self {
        Self {
            config,
            files: Arc::new(DashMap::new()),
            eviction_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Check if a file should be memory-mapped based on configuration
    pub fn should_mmap<P: AsRef<Path>>(&self, path: P) -> Result<bool> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Ok(false);
        }
        
        let metadata = std::fs::metadata(path).map_err(|e| {
            ChunkServerError::Io(e)
        })?;
        
        let size = metadata.len();
        
        Ok(size >= self.config.min_file_size && size <= self.config.max_file_size)
    }
    
    /// Get or create a memory-mapped file
    pub async fn get_mmap<P: AsRef<Path>>(&self, path: P) -> Result<Option<Arc<MmapFile>>> {
        let path = path.as_ref().to_path_buf();
        
        // Check if we should memory-map this file
        if !self.should_mmap(&path)? {
            return Ok(None);
        }
        
        // Check if we already have this file mapped
        if let Some(entry) = self.files.get(&path) {
            let lock = entry.read().await;
            if let Some(mmap_file) = lock.as_ref() {
                return Ok(Some(mmap_file.clone()));
            }
        }
        
        // Check if we need to evict files before adding a new one
        self.maybe_evict_files().await?;
        
        // Create new memory mapping
        let mmap_file = match MmapFile::new(&path) {
            Ok(file) => Arc::new(file),
            Err(e) => {
                warn!("Failed to create memory mapping for {}: {}", path.display(), e);
                return Ok(None);
            }
        };
        
        // Insert into our tracking structures
        let entry = self.files.entry(path.clone()).or_insert_with(|| {
            Arc::new(RwLock::new(None))
        });
        
        {
            let mut lock = entry.write().await;
            *lock = Some(mmap_file.clone());
        }
        
        // Update eviction queue
        {
            let mut queue = self.eviction_queue.write().await;
            // Remove path if it already exists (move to end)
            queue.retain(|p| p != &path);
            queue.push(path);
        }
        
        debug!("Created memory mapping for {} ({} bytes)", 
               mmap_file.path().display(), mmap_file.size());
        
        Ok(Some(mmap_file))
    }
    
    /// Remove a memory-mapped file
    pub async fn remove_mmap<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        
        if let Some((_, entry)) = self.files.remove(&path) {
            let mut lock = entry.write().await;
            if let Some(mmap_file) = lock.take() {
                debug!("Removed memory mapping for {}", mmap_file.path().display());
            }
        }
        
        // Remove from eviction queue
        {
            let mut queue = self.eviction_queue.write().await;
            queue.retain(|p| p != &path);
        }
        
        Ok(())
    }
    
    /// Check if we need to evict files and do so if necessary
    async fn maybe_evict_files(&self) -> Result<()> {
        let current_count = self.files.len();
        
        if current_count >= self.config.max_files {
            let evict_count = (current_count - self.config.max_files + 1).max(1);
            self.evict_oldest_files(evict_count).await?;
        }
        
        Ok(())
    }
    
    /// Evict the oldest (least recently used) files
    async fn evict_oldest_files(&self, count: usize) -> Result<()> {
        let paths_to_evict = {
            let mut queue = self.eviction_queue.write().await;
            let evict_count = count.min(queue.len());
            queue.drain(0..evict_count).collect::<Vec<_>>()
        };
        
        for path in paths_to_evict {
            if let Some((_, entry)) = self.files.remove(&path) {
                let mut lock = entry.write().await;
                if let Some(mmap_file) = lock.take() {
                    debug!("Evicted memory mapping for {} ({} accesses)", 
                           mmap_file.path().display(), mmap_file.access_count());
                }
            }
        }
        
        debug!("Evicted {} memory-mapped files", count);
        Ok(())
    }
    
    /// Cleanup cold (infrequently accessed) files
    pub async fn cleanup_cold_files(&self) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut cold_paths = Vec::new();
        
        // Find cold files
        for entry in self.files.iter() {
            let path = entry.key().clone();
            let lock = entry.value().read().await;
            
            if let Some(mmap_file) = lock.as_ref() {
                let last_access = mmap_file.last_accessed();
                let time_since_access = now.saturating_sub(last_access);
                
                // Consider file cold if not accessed in the time window
                if time_since_access > self.config.hot_time_window_secs &&
                   !mmap_file.is_hot(self.config.hot_access_threshold, self.config.hot_time_window_secs) {
                    cold_paths.push(path);
                }
            }
        }
        
        // Remove cold files
        for path in cold_paths {
            self.remove_mmap(&path).await?;
        }
        
        Ok(())
    }
    
    /// Get statistics about memory-mapped files
    pub async fn get_stats(&self) -> MmapStats {
        let total_files = self.files.len();
        let mut total_size = 0u64;
        let mut hot_files = 0;
        let mut total_accesses = 0u64;
        
        for entry in self.files.iter() {
            let lock = entry.value().read().await;
            if let Some(mmap_file) = lock.as_ref() {
                total_size += mmap_file.size();
                total_accesses += mmap_file.access_count();
                
                if mmap_file.is_hot(self.config.hot_access_threshold, self.config.hot_time_window_secs) {
                    hot_files += 1;
                }
            }
        }
        
        MmapStats {
            total_files,
            total_size_bytes: total_size,
            hot_files,
            total_accesses,
            max_files: self.config.max_files,
        }
    }
    
    /// Start a background task for periodic cleanup
    pub fn start_cleanup_task(&self, shutdown_rx: tokio::sync::broadcast::Receiver<()>) {
        let manager = MmapManager {
            config: self.config.clone(),
            files: self.files.clone(),
            eviction_queue: self.eviction_queue.clone(),
        };
        
        tokio::spawn(async move {
            manager.cleanup_loop(shutdown_rx).await;
        });
    }
    
    /// Background cleanup loop
    async fn cleanup_loop(&self, mut shutdown_rx: tokio::sync::broadcast::Receiver<()>) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.cleanup_interval_secs)
        );
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    debug!("Running memory-mapped file cleanup");
                    if let Err(e) = self.cleanup_cold_files().await {
                        error!("Error during mmap cleanup: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    debug!("Memory-mapped file cleanup task shutting down");
                    break;
                }
            }
        }
    }
}

/// Statistics for memory-mapped files
#[derive(Debug, Clone)]
pub struct MmapStats {
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub hot_files: usize,
    pub total_accesses: u64,
    pub max_files: usize,
}

impl MmapStats {
    /// Calculate hit ratio (hot files / total files)
    pub fn hot_ratio(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            self.hot_files as f64 / self.total_files as f64
        }
    }
    
    /// Calculate utilization (total files / max files)
    pub fn utilization(&self) -> f64 {
        if self.max_files == 0 {
            0.0
        } else {
            self.total_files as f64 / self.max_files as f64
        }
    }
    
    /// Calculate average accesses per file
    pub fn avg_accesses_per_file(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            self.total_accesses as f64 / self.total_files as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[tokio::test]
    async fn test_mmap_file_creation() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Hello, World!").unwrap();
        temp_file.flush().unwrap();
        
        let mmap_file = MmapFile::new(temp_file.path()).unwrap();
        assert_eq!(mmap_file.size(), 13);
        assert_eq!(mmap_file.data(), "Hello, World!");
    }
    
    #[tokio::test]
    async fn test_mmap_file_slice() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Hello, World!").unwrap();
        temp_file.flush().unwrap();
        
        let mmap_file = MmapFile::new(temp_file.path()).unwrap();
        let slice = mmap_file.slice(0, 5).unwrap();
        assert_eq!(slice, "Hello");
        
        let slice = mmap_file.slice(7, 5).unwrap();
        assert_eq!(slice, "World");
    }
    
    #[tokio::test]
    async fn test_mmap_manager() {
        let config = MmapConfig {
            max_files: 2,
            min_file_size: 1,
            max_file_size: 1024,
            ..Default::default()
        };
        
        let manager = MmapManager::new(config);
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Hello, World!").unwrap();
        temp_file.flush().unwrap();
        
        // Get memory mapping
        let mmap = manager.get_mmap(temp_file.path()).await.unwrap();
        assert!(mmap.is_some());
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_size_bytes, 13);
    }
}