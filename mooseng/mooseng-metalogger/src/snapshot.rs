use anyhow::{Result, Context};
use bytes::{Bytes, BytesMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use flate2::Compression;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use std::io::Write;

use crate::config::SnapshotConfig;

const SNAPSHOT_MAGIC: &[u8] = b"MNGSNAP1"; // MooseNG Snapshot version 1
const SNAPSHOT_HEADER_SIZE: usize = 64;

/// Snapshot metadata
#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub sequence_id: u64,
    pub entries_count: u64,
    pub compressed: bool,
    pub size: u64,
    pub checksum: u64,
}

/// Snapshot manager for periodic metadata snapshots
pub struct SnapshotManager {
    config: SnapshotConfig,
    snapshot_dir: PathBuf,
    snapshot_index: Arc<RwLock<u64>>,
    semaphore: Arc<Semaphore>,
}

impl SnapshotManager {
    pub async fn new(data_dir: PathBuf, config: SnapshotConfig) -> Result<Self> {
        let snapshot_dir = data_dir.join("snapshots");
        fs::create_dir_all(&snapshot_dir).await
            .context("Failed to create snapshot directory")?;

        Ok(Self {
            config,
            snapshot_dir,
            snapshot_index: Arc::new(RwLock::new(0)),
            semaphore: Arc::new(Semaphore::new(config.parallel_workers)),
        })
    }

    /// Initialize snapshot manager and find latest snapshot
    pub async fn initialize(&self) -> Result<Option<SnapshotMetadata>> {
        info!("Initializing snapshot manager");
        
        let snapshots = self.list_snapshots().await?;
        if let Some(latest) = snapshots.last() {
            info!("Found latest snapshot: {:?}", latest.timestamp);
            
            // Update snapshot index
            let mut index = self.snapshot_index.write().await;
            *index = latest.id;
            
            return Ok(Some(latest.clone()));
        }
        
        Ok(None)
    }

    /// Start periodic snapshot creation
    pub async fn start_periodic_snapshots<F, Fut>(
        &self,
        snapshot_fn: F,
    ) -> Result<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<Vec<(String, Bytes)>>> + Send,
    {
        let mut interval = interval(Duration::from_secs(self.config.interval));
        
        loop {
            interval.tick().await;
            
            info!("Starting periodic snapshot");
            match snapshot_fn().await {
                Ok(data) => {
                    if let Err(e) = self.create_snapshot(data).await {
                        error!("Failed to create snapshot: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to collect snapshot data: {}", e);
                }
            }
            
            // Clean up old snapshots
            if let Err(e) = self.cleanup_old_snapshots().await {
                warn!("Failed to cleanup old snapshots: {}", e);
            }
        }
    }

    /// Create a new snapshot
    pub async fn create_snapshot(&self, data: Vec<(String, Bytes)>) -> Result<SnapshotMetadata> {
        let mut index = self.snapshot_index.write().await;
        *index += 1;
        let snapshot_id = *index;
        drop(index);

        let timestamp = Utc::now();
        let filename = format!("snapshot_{:010}_{}.snap", snapshot_id, timestamp.timestamp());
        let path = self.snapshot_dir.join(&filename);
        
        info!("Creating snapshot: {:?}", path);
        
        // Calculate total size
        let total_size: usize = data.iter().map(|(_, bytes)| bytes.len()).sum();
        let entries_count = data.len() as u64;
        
        // Create snapshot file
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&path)
            .await?;
        
        let mut writer = BufWriter::new(file);
        
        // Write header (will update later)
        let header_placeholder = vec![0u8; SNAPSHOT_HEADER_SIZE];
        writer.write_all(&header_placeholder).await?;
        
        // Write data with optional compression
        let (written_size, checksum) = if self.config.enable_compression {
            self.write_compressed_data(&mut writer, &data).await?
        } else {
            self.write_uncompressed_data(&mut writer, &data).await?
        };
        
        // Update header
        writer.flush().await?;
        let mut file = writer.into_inner();
        file.seek(std::io::SeekFrom::Start(0)).await?;
        
        let metadata = SnapshotMetadata {
            id: snapshot_id,
            timestamp,
            sequence_id: 0, // Will be set by caller
            entries_count,
            compressed: self.config.enable_compression,
            size: written_size,
            checksum,
        };
        
        self.write_snapshot_header(&mut file, &metadata).await?;
        file.sync_all().await?;
        
        info!("Snapshot created: id={}, entries={}, size={}", 
              snapshot_id, entries_count, written_size);
        
        Ok(metadata)
    }

    /// Write snapshot header
    async fn write_snapshot_header(&self, file: &mut File, metadata: &SnapshotMetadata) -> Result<()> {
        let mut header = BytesMut::with_capacity(SNAPSHOT_HEADER_SIZE);
        
        header.extend_from_slice(SNAPSHOT_MAGIC);
        header.extend_from_slice(&metadata.id.to_le_bytes());
        header.extend_from_slice(&metadata.timestamp.timestamp().to_le_bytes());
        header.extend_from_slice(&metadata.sequence_id.to_le_bytes());
        header.extend_from_slice(&metadata.entries_count.to_le_bytes());
        header.extend_from_slice(&[metadata.compressed as u8]);
        header.extend_from_slice(&[0u8; 7]); // Reserved
        header.extend_from_slice(&metadata.size.to_le_bytes());
        header.extend_from_slice(&metadata.checksum.to_le_bytes());
        
        file.write_all(&header).await?;
        Ok(())
    }

    /// Write compressed data
    async fn write_compressed_data(
        &self,
        writer: &mut BufWriter<File>,
        data: &[(String, Bytes)]
    ) -> Result<(u64, u64)> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut hasher = crc32fast::Hasher::new();
        let mut total_size = 0u64;
        
        // Process data in parallel chunks
        let chunk_size = data.len() / self.config.parallel_workers.max(1);
        let chunks: Vec<_> = data.chunks(chunk_size).collect();
        
        for chunk in chunks {
            for (key, value) in chunk {
                // Write entry: [key_len: 4][key][value_len: 4][value]
                let key_bytes = key.as_bytes();
                encoder.write_all(&(key_bytes.len() as u32).to_le_bytes())?;
                encoder.write_all(key_bytes)?;
                encoder.write_all(&(value.len() as u32).to_le_bytes())?;
                encoder.write_all(value)?;
                
                hasher.update(&(key_bytes.len() as u32).to_le_bytes());
                hasher.update(key_bytes);
                hasher.update(&(value.len() as u32).to_le_bytes());
                hasher.update(value);
                
                total_size += 8 + key_bytes.len() as u64 + value.len() as u64;
            }
        }
        
        let compressed_data = encoder.finish()?;
        writer.write_all(&compressed_data).await?;
        
        Ok((compressed_data.len() as u64, hasher.finalize() as u64))
    }

    /// Write uncompressed data
    async fn write_uncompressed_data(
        &self,
        writer: &mut BufWriter<File>,
        data: &[(String, Bytes)]
    ) -> Result<(u64, u64)> {
        let mut hasher = crc32fast::Hasher::new();
        let mut total_size = 0u64;
        
        for (key, value) in data {
            let key_bytes = key.as_bytes();
            
            // Write entry: [key_len: 4][key][value_len: 4][value]
            writer.write_all(&(key_bytes.len() as u32).to_le_bytes()).await?;
            writer.write_all(key_bytes).await?;
            writer.write_all(&(value.len() as u32).to_le_bytes()).await?;
            writer.write_all(value).await?;
            
            hasher.update(&(key_bytes.len() as u32).to_le_bytes());
            hasher.update(key_bytes);
            hasher.update(&(value.len() as u32).to_le_bytes());
            hasher.update(value);
            
            total_size += 8 + key_bytes.len() as u64 + value.len() as u64;
        }
        
        Ok((total_size, hasher.finalize() as u64))
    }

    /// List all snapshots in order
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>> {
        let mut entries = fs::read_dir(&self.snapshot_dir).await?;
        let mut snapshots = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("snap") {
                if let Ok(metadata) = self.read_snapshot_metadata(&path).await {
                    snapshots.push(metadata);
                }
            }
        }
        
        snapshots.sort_by_key(|s| s.id);
        Ok(snapshots)
    }

    /// Read snapshot metadata
    async fn read_snapshot_metadata(&self, path: &Path) -> Result<SnapshotMetadata> {
        let mut file = File::open(path).await?;
        let mut header = vec![0u8; SNAPSHOT_HEADER_SIZE];
        file.read_exact(&mut header).await?;
        
        // Verify magic
        if &header[0..8] != SNAPSHOT_MAGIC {
            return Err(anyhow::anyhow!("Invalid snapshot file"));
        }
        
        Ok(SnapshotMetadata {
            id: u64::from_le_bytes(header[8..16].try_into()?),
            timestamp: DateTime::from_timestamp(
                i64::from_le_bytes(header[16..24].try_into()?), 0
            ).unwrap_or_else(Utc::now),
            sequence_id: u64::from_le_bytes(header[24..32].try_into()?),
            entries_count: u64::from_le_bytes(header[32..40].try_into()?),
            compressed: header[40] != 0,
            size: u64::from_le_bytes(header[48..56].try_into()?),
            checksum: u64::from_le_bytes(header[56..64].try_into()?),
        })
    }

    /// Load snapshot data
    pub async fn load_snapshot(&self, metadata: &SnapshotMetadata) -> Result<Vec<(String, Bytes)>> {
        let filename = format!("snapshot_{:010}_{}.snap", metadata.id, metadata.timestamp.timestamp());
        let path = self.snapshot_dir.join(&filename);
        
        info!("Loading snapshot: {:?}", path);
        
        let file = File::open(&path).await?;
        let mut reader = BufReader::new(file);
        
        // Skip header
        let mut header = vec![0u8; SNAPSHOT_HEADER_SIZE];
        reader.read_exact(&mut header).await?;
        
        if metadata.compressed {
            self.read_compressed_data(reader).await
        } else {
            self.read_uncompressed_data(reader).await
        }
    }

    /// Read compressed data
    async fn read_compressed_data(&self, mut reader: BufReader<File>) -> Result<Vec<(String, Bytes)>> {
        let mut compressed_data = Vec::new();
        reader.read_to_end(&mut compressed_data).await?;
        
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed)?;
        
        self.parse_snapshot_entries(&decompressed)
    }

    /// Read uncompressed data
    async fn read_uncompressed_data(&self, mut reader: BufReader<File>) -> Result<Vec<(String, Bytes)>> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data).await?;
        self.parse_snapshot_entries(&data)
    }

    /// Parse snapshot entries
    fn parse_snapshot_entries(&self, data: &[u8]) -> Result<Vec<(String, Bytes)>> {
        let mut entries = Vec::new();
        let mut cursor = 0;
        
        while cursor < data.len() {
            // Read key length
            if cursor + 4 > data.len() {
                break;
            }
            let key_len = u32::from_le_bytes(data[cursor..cursor+4].try_into()?) as usize;
            cursor += 4;
            
            // Read key
            if cursor + key_len > data.len() {
                return Err(anyhow::anyhow!("Corrupted snapshot data"));
            }
            let key = String::from_utf8(data[cursor..cursor+key_len].to_vec())?;
            cursor += key_len;
            
            // Read value length
            if cursor + 4 > data.len() {
                return Err(anyhow::anyhow!("Corrupted snapshot data"));
            }
            let value_len = u32::from_le_bytes(data[cursor..cursor+4].try_into()?) as usize;
            cursor += 4;
            
            // Read value
            if cursor + value_len > data.len() {
                return Err(anyhow::anyhow!("Corrupted snapshot data"));
            }
            let value = Bytes::from(data[cursor..cursor+value_len].to_vec());
            cursor += value_len;
            
            entries.push((key, value));
        }
        
        Ok(entries)
    }

    /// Clean up old snapshots
    async fn cleanup_old_snapshots(&self) -> Result<()> {
        let snapshots = self.list_snapshots().await?;
        
        if snapshots.len() > self.config.retention_count as usize {
            let to_remove = snapshots.len() - self.config.retention_count as usize;
            
            for snapshot in snapshots.into_iter().take(to_remove) {
                let filename = format!("snapshot_{:010}_{}.snap", 
                                     snapshot.id, snapshot.timestamp.timestamp());
                let path = self.snapshot_dir.join(&filename);
                
                info!("Removing old snapshot: {:?}", path);
                fs::remove_file(path).await?;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_snapshot_create_load() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = SnapshotConfig {
            interval: 3600,
            retention_count: 10,
            enable_compression: true,
            parallel_workers: 2,
        };
        
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf(), config).await?;
        
        // Create test data
        let data = vec![
            ("key1".to_string(), Bytes::from("value1")),
            ("key2".to_string(), Bytes::from("value2")),
            ("key3".to_string(), Bytes::from("value3")),
        ];
        
        // Create snapshot
        let metadata = manager.create_snapshot(data.clone()).await?;
        assert_eq!(metadata.entries_count, 3);
        assert!(metadata.compressed);
        
        // Load snapshot
        let loaded = manager.load_snapshot(&metadata).await?;
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].0, "key1");
        assert_eq!(loaded[0].1, Bytes::from("value1"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_cleanup() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = SnapshotConfig {
            interval: 3600,
            retention_count: 2,
            enable_compression: false,
            parallel_workers: 1,
        };
        
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf(), config).await?;
        
        // Create multiple snapshots
        for i in 0..5 {
            let data = vec![(format!("key{}", i), Bytes::from(format!("value{}", i)))];
            manager.create_snapshot(data).await?;
        }
        
        // Clean up old snapshots
        manager.cleanup_old_snapshots().await?;
        
        // Check remaining snapshots
        let snapshots = manager.list_snapshots().await?;
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].id, 4);
        assert_eq!(snapshots[1].id, 5);
        
        Ok(())
    }
}