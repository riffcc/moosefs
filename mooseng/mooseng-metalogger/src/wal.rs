use anyhow::{Result, Context};
use bytes::{Bytes, BytesMut, BufMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};
use crc32fast::Hasher;

const WAL_MAGIC: &[u8] = b"MNGWAL01"; // MooseNG WAL version 1
const WAL_HEADER_SIZE: usize = 32;
const MAX_WAL_SIZE: u64 = 1024 * 1024 * 1024; // 1GB default

/// WAL entry header
#[derive(Debug, Clone)]
struct WalEntryHeader {
    sequence_id: u64,
    timestamp: i64,
    data_size: u32,
    crc32: u32,
}

/// Write-Ahead Log writer
pub struct WalWriter {
    wal_dir: PathBuf,
    current_file: Arc<Mutex<Option<WalFile>>>,
    wal_index: Arc<RwLock<u64>>,
    max_file_size: u64,
}

/// Active WAL file handle
struct WalFile {
    file: File,
    path: PathBuf,
    size: u64,
    sequence_start: u64,
    sequence_end: u64,
}

impl WalWriter {
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        let wal_dir = data_dir.join("wal");
        fs::create_dir_all(&wal_dir).await
            .context("Failed to create WAL directory")?;

        Ok(Self {
            wal_dir,
            current_file: Arc::new(Mutex::new(None)),
            wal_index: Arc::new(RwLock::new(0)),
            max_file_size: MAX_WAL_SIZE,
        })
    }

    /// Initialize WAL writer and recover from existing files
    pub async fn initialize(&self) -> Result<u64> {
        info!("Initializing WAL writer");
        
        // Scan existing WAL files
        let mut entries = fs::read_dir(&self.wal_dir).await?;
        let mut wal_files = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                if let Some(index) = Self::parse_wal_filename(&path) {
                    wal_files.push((index, path));
                }
            }
        }
        
        // Sort by index
        wal_files.sort_by_key(|(idx, _)| *idx);
        
        // Find the last sequence ID
        let mut last_sequence = 0u64;
        if let Some((idx, path)) = wal_files.last() {
            info!("Found existing WAL file: {:?}", path);
            last_sequence = self.scan_wal_file(path).await?;
            
            // Update WAL index
            let mut index = self.wal_index.write().await;
            *index = *idx;
        }
        
        info!("WAL initialized with last sequence ID: {}", last_sequence);
        Ok(last_sequence)
    }

    /// Write a single entry to WAL
    pub async fn write_entry(&self, data: Bytes) -> Result<u64> {
        let mut current = self.current_file.lock().await;
        
        // Check if we need a new file
        if current.is_none() || current.as_ref().unwrap().size >= self.max_file_size {
            *current = Some(self.create_new_wal_file().await?);
        }
        
        let wal_file = current.as_mut().unwrap();
        let sequence_id = self.get_next_sequence_id().await;
        
        // Create entry header
        let header = WalEntryHeader {
            sequence_id,
            timestamp: chrono::Utc::now().timestamp(),
            data_size: data.len() as u32,
            crc32: Self::calculate_crc(&data),
        };
        
        // Write header and data
        self.write_entry_to_file(wal_file, &header, &data).await?;
        
        // Update file metadata
        wal_file.sequence_end = sequence_id;
        wal_file.size += WAL_HEADER_SIZE as u64 + data.len() as u64;
        
        Ok(sequence_id)
    }

    /// Write a batch of entries
    pub async fn write_batch(&self, entries: Vec<Bytes>) -> Result<Vec<u64>> {
        let mut sequence_ids = Vec::with_capacity(entries.len());
        
        for data in entries {
            let seq_id = self.write_entry(data).await?;
            sequence_ids.push(seq_id);
        }
        
        // Ensure data is flushed
        if let Some(wal_file) = &mut *self.current_file.lock().await {
            wal_file.file.sync_all().await?;
        }
        
        Ok(sequence_ids)
    }

    /// Create a new WAL file
    async fn create_new_wal_file(&self) -> Result<WalFile> {
        let mut index = self.wal_index.write().await;
        *index += 1;
        
        let filename = format!("wal_{:010}.wal", *index);
        let path = self.wal_dir.join(&filename);
        
        info!("Creating new WAL file: {:?}", path);
        
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&path)
            .await?;
        
        // Write file header
        file.write_all(WAL_MAGIC).await?;
        file.write_all(&index.to_le_bytes()).await?;
        file.write_all(&[0u8; 16]).await?; // Reserved space
        
        Ok(WalFile {
            file,
            path,
            size: WAL_HEADER_SIZE as u64,
            sequence_start: 0,
            sequence_end: 0,
        })
    }

    /// Write entry to file
    async fn write_entry_to_file(
        &self, 
        wal_file: &mut WalFile, 
        header: &WalEntryHeader,
        data: &Bytes
    ) -> Result<()> {
        let mut buffer = BytesMut::with_capacity(WAL_HEADER_SIZE + data.len());
        
        // Write header
        buffer.put_u64_le(header.sequence_id);
        buffer.put_i64_le(header.timestamp);
        buffer.put_u32_le(header.data_size);
        buffer.put_u32_le(header.crc32);
        buffer.put_bytes(0, 8); // Reserved
        
        // Write data
        buffer.extend_from_slice(data);
        
        // Write to file
        wal_file.file.write_all(&buffer).await?;
        
        debug!("Wrote WAL entry: seq={}, size={}", header.sequence_id, data.len());
        Ok(())
    }

    /// Calculate CRC32 checksum
    fn calculate_crc(data: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }

    /// Parse WAL filename to extract index
    fn parse_wal_filename(path: &Path) -> Option<u64> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("wal_"))
            .and_then(|s| s.parse().ok())
    }

    /// Scan WAL file to find last sequence ID
    async fn scan_wal_file(&self, path: &Path) -> Result<u64> {
        let mut file = File::open(path).await?;
        let mut last_sequence = 0u64;
        
        // Skip file header
        file.seek(std::io::SeekFrom::Start(WAL_HEADER_SIZE as u64)).await?;
        
        loop {
            let mut header_buf = [0u8; WAL_HEADER_SIZE];
            match file.read_exact(&mut header_buf).await {
                Ok(_) => {
                    let sequence_id = u64::from_le_bytes(header_buf[0..8].try_into()?);
                    let data_size = u32::from_le_bytes(header_buf[16..20].try_into()?);
                    
                    last_sequence = sequence_id;
                    
                    // Skip data
                    file.seek(std::io::SeekFrom::Current(data_size as i64)).await?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
        }
        
        Ok(last_sequence)
    }

    /// Get next sequence ID
    async fn get_next_sequence_id(&self) -> u64 {
        // This would be coordinated with the replication client
        // For now, use a simple counter
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Rotate WAL files and clean up old ones
    pub async fn rotate(&self, retention_count: u32) -> Result<()> {
        let mut current = self.current_file.lock().await;
        
        // Force creation of new file
        *current = None;
        
        // Clean up old files
        self.cleanup_old_files(retention_count).await?;
        
        Ok(())
    }

    /// Clean up old WAL files
    async fn cleanup_old_files(&self, retention_count: u32) -> Result<()> {
        let mut entries = fs::read_dir(&self.wal_dir).await?;
        let mut wal_files = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                if let Some(index) = Self::parse_wal_filename(&path) {
                    wal_files.push((index, path));
                }
            }
        }
        
        // Sort by index
        wal_files.sort_by_key(|(idx, _)| *idx);
        
        // Remove old files
        if wal_files.len() > retention_count as usize {
            let to_remove = wal_files.len() - retention_count as usize;
            for (_, path) in wal_files.into_iter().take(to_remove) {
                info!("Removing old WAL file: {:?}", path);
                fs::remove_file(path).await?;
            }
        }
        
        Ok(())
    }
}

/// WAL reader for recovery
pub struct WalReader {
    wal_dir: PathBuf,
}

impl WalReader {
    pub fn new(wal_dir: PathBuf) -> Self {
        Self { wal_dir }
    }

    /// Read all WAL entries starting from a sequence ID
    pub async fn read_from(&self, start_sequence: u64) -> Result<Vec<(u64, Bytes)>> {
        let mut entries = Vec::new();
        let wal_files = self.list_wal_files().await?;
        
        for path in wal_files {
            let file_entries = self.read_wal_file(&path, start_sequence).await?;
            entries.extend(file_entries);
        }
        
        Ok(entries)
    }

    /// List all WAL files in order
    async fn list_wal_files(&self) -> Result<Vec<PathBuf>> {
        let mut entries = fs::read_dir(&self.wal_dir).await?;
        let mut wal_files = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                if let Some(index) = WalWriter::parse_wal_filename(&path) {
                    wal_files.push((index, path));
                }
            }
        }
        
        wal_files.sort_by_key(|(idx, _)| *idx);
        Ok(wal_files.into_iter().map(|(_, path)| path).collect())
    }

    /// Read entries from a single WAL file
    async fn read_wal_file(&self, path: &Path, start_sequence: u64) -> Result<Vec<(u64, Bytes)>> {
        let mut file = File::open(path).await?;
        let mut entries = Vec::new();
        
        // Skip file header
        file.seek(std::io::SeekFrom::Start(WAL_HEADER_SIZE as u64)).await?;
        
        loop {
            let mut header_buf = [0u8; WAL_HEADER_SIZE];
            match file.read_exact(&mut header_buf).await {
                Ok(_) => {
                    let sequence_id = u64::from_le_bytes(header_buf[0..8].try_into()?);
                    let _timestamp = i64::from_le_bytes(header_buf[8..16].try_into()?);
                    let data_size = u32::from_le_bytes(header_buf[16..20].try_into()?);
                    let crc32 = u32::from_le_bytes(header_buf[20..24].try_into()?);
                    
                    let mut data = vec![0u8; data_size as usize];
                    file.read_exact(&mut data).await?;
                    
                    // Verify CRC
                    if WalWriter::calculate_crc(&data) != crc32 {
                        warn!("CRC mismatch for sequence {}", sequence_id);
                        continue;
                    }
                    
                    if sequence_id >= start_sequence {
                        entries.push((sequence_id, Bytes::from(data)));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
        }
        
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_wal_write_read() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let wal_writer = WalWriter::new(temp_dir.path().to_path_buf()).await?;
        
        // Initialize
        let last_seq = wal_writer.initialize().await?;
        assert_eq!(last_seq, 0);
        
        // Write some entries
        let data1 = Bytes::from("test entry 1");
        let data2 = Bytes::from("test entry 2");
        let data3 = Bytes::from("test entry 3");
        
        let seq1 = wal_writer.write_entry(data1.clone()).await?;
        let seq2 = wal_writer.write_entry(data2.clone()).await?;
        let seq3 = wal_writer.write_entry(data3.clone()).await?;
        
        // Read entries
        let wal_reader = WalReader::new(temp_dir.path().join("wal"));
        let entries = wal_reader.read_from(seq1).await?;
        
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].1, data1);
        assert_eq!(entries[1].1, data2);
        assert_eq!(entries[2].1, data3);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_rotation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut wal_writer = WalWriter::new(temp_dir.path().to_path_buf()).await?;
        wal_writer.max_file_size = 100; // Small size to trigger rotation
        
        // Write enough entries to trigger rotation
        for i in 0..10 {
            let data = Bytes::from(format!("test entry {}", i));
            wal_writer.write_entry(data).await?;
        }
        
        // Check that multiple files were created
        let mut entries = fs::read_dir(temp_dir.path().join("wal")).await?;
        let mut file_count = 0;
        while let Some(_) = entries.next_entry().await? {
            file_count += 1;
        }
        
        assert!(file_count > 1);
        
        Ok(())
    }
}