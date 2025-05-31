use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use mooseng_common::types::{ChunkId, ChunkVersion, Timestamp, now_micros};
use bytes::Bytes;
use blake3::Hash;
use crc32fast::Hasher;

/// Checksum type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChecksumType {
    Blake3,
    Crc32,
    None,
}

/// Chunk checksum containing both type and value
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkChecksum {
    pub checksum_type: ChecksumType,
    pub value: Vec<u8>,
}

impl ChunkChecksum {
    /// Create a new Blake3 checksum
    pub fn blake3(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Self {
            checksum_type: ChecksumType::Blake3,
            value: hash.as_bytes().to_vec(),
        }
    }
    
    /// Create a new CRC32 checksum
    pub fn crc32(data: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(data);
        let crc = hasher.finalize();
        Self {
            checksum_type: ChecksumType::Crc32,
            value: crc.to_le_bytes().to_vec(),
        }
    }
    
    /// Create a checksum with no verification
    pub fn none() -> Self {
        Self {
            checksum_type: ChecksumType::None,
            value: Vec::new(),
        }
    }
    
    /// Verify data against this checksum
    pub fn verify(&self, data: &[u8]) -> bool {
        match self.checksum_type {
            ChecksumType::Blake3 => {
                let hash = blake3::hash(data);
                hash.as_bytes() == self.value.as_slice()
            }
            ChecksumType::Crc32 => {
                let mut hasher = Hasher::new();
                hasher.update(data);
                let crc = hasher.finalize();
                crc.to_le_bytes().to_vec() == self.value
            }
            ChecksumType::None => true,
        }
    }
    
    /// Get hex representation of checksum
    pub fn hex(&self) -> String {
        hex::encode(&self.value)
    }
}

/// Chunk metadata stored alongside chunk data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_id: ChunkId,
    pub version: ChunkVersion,
    pub size: u64,
    pub checksum: ChunkChecksum,
    pub created_at: Timestamp,
    pub last_accessed: Timestamp,
    pub last_modified: Timestamp,
    pub access_count: u64,
    pub is_corrupted: bool,
    pub storage_class_id: u8,
}

impl ChunkMetadata {
    /// Create new chunk metadata
    pub fn new(
        chunk_id: ChunkId,
        version: ChunkVersion,
        size: u64,
        checksum: ChunkChecksum,
        storage_class_id: u8,
    ) -> Self {
        let now = now_micros();
        Self {
            chunk_id,
            version,
            size,
            checksum,
            created_at: now,
            last_accessed: now,
            last_modified: now,
            access_count: 0,
            is_corrupted: false,
            storage_class_id,
        }
    }
    
    /// Update access time and increment counter
    pub fn touch(&mut self) {
        self.last_accessed = now_micros();
        self.access_count += 1;
    }
    
    /// Mark as modified
    pub fn mark_modified(&mut self) {
        self.last_modified = now_micros();
    }
    
    /// Mark as corrupted
    pub fn mark_corrupted(&mut self) {
        self.is_corrupted = true;
    }
    
    /// Check if chunk is considered hot (frequently accessed)
    pub fn is_hot(&self, threshold_accesses: u64, time_window_micros: u64) -> bool {
        let now = now_micros();
        let time_since_creation = now.saturating_sub(self.created_at);
        
        if time_since_creation < time_window_micros {
            return self.access_count > threshold_accesses;
        }
        
        // Calculate access rate per hour
        let hours = time_since_creation / (1_000_000 * 3600);
        if hours == 0 {
            return self.access_count > threshold_accesses;
        }
        
        self.access_count / hours > threshold_accesses
    }
}

/// In-memory representation of a chunk
#[derive(Debug, Clone)]
pub struct Chunk {
    pub metadata: ChunkMetadata,
    pub data: Bytes,
}

impl Chunk {
    /// Create a new chunk
    pub fn new(
        chunk_id: ChunkId,
        version: ChunkVersion,
        data: Bytes,
        checksum_type: ChecksumType,
        storage_class_id: u8,
    ) -> Self {
        let checksum = match checksum_type {
            ChecksumType::Blake3 => ChunkChecksum::blake3(&data),
            ChecksumType::Crc32 => ChunkChecksum::crc32(&data),
            ChecksumType::None => ChunkChecksum::none(),
        };
        
        let metadata = ChunkMetadata::new(
            chunk_id,
            version,
            data.len() as u64,
            checksum,
            storage_class_id,
        );
        
        Self { metadata, data }
    }
    
    /// Verify chunk data integrity
    pub fn verify_integrity(&self) -> bool {
        self.metadata.checksum.verify(&self.data)
    }
    
    /// Get chunk size
    pub fn size(&self) -> u64 {
        self.data.len() as u64
    }
    
    /// Check if chunk is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Get chunk ID
    pub fn id(&self) -> ChunkId {
        self.metadata.chunk_id
    }
    
    /// Get chunk version
    pub fn version(&self) -> ChunkVersion {
        self.metadata.version
    }
    
    /// Access chunk data (immutable)
    pub fn data(&self) -> &Bytes {
        &self.data
    }
    
    /// Take ownership of chunk data
    pub fn into_data(self) -> Bytes {
        self.data
    }
    
    /// Get a slice of chunk data
    pub fn slice(&self, offset: u64, length: u64) -> Result<Bytes, crate::error::ChunkServerError> {
        let start = offset as usize;
        let end = (offset + length) as usize;
        
        if end > self.data.len() {
            return Err(crate::error::ChunkServerError::InvalidOperation(
                format!("Slice bounds out of range: {}-{} for chunk of size {}", 
                        start, end, self.data.len())
            ));
        }
        
        Ok(self.data.slice(start..end))
    }
}

/// Metrics for chunk operations
#[derive(Debug, Clone, Default)]
pub struct ChunkMetrics {
    pub chunks_stored: u64,
    pub chunks_retrieved: u64,
    pub chunks_deleted: u64,
    pub bytes_written: u64,
    pub bytes_read: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub checksum_verifications: u64,
    pub checksum_failures: u64,
    pub corruption_detected: u64,
    pub mmap_operations: u64,
    pub disk_operations: u64,
}

impl ChunkMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record a chunk store operation
    pub fn record_store(&mut self, bytes: u64) {
        self.chunks_stored += 1;
        self.bytes_written += bytes;
    }
    
    /// Record a chunk retrieval operation
    pub fn record_retrieval(&mut self, bytes: u64) {
        self.chunks_retrieved += 1;
        self.bytes_read += bytes;
    }
    
    /// Record a chunk deletion
    pub fn record_deletion(&mut self) {
        self.chunks_deleted += 1;
    }
    
    /// Record cache hit
    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }
    
    /// Record cache miss
    pub fn record_cache_miss(&mut self) {
        self.cache_misses += 1;
    }
    
    /// Record checksum verification
    pub fn record_checksum_verification(&mut self, success: bool) {
        self.checksum_verifications += 1;
        if !success {
            self.checksum_failures += 1;
        }
    }
    
    /// Record corruption detection
    pub fn record_corruption(&mut self) {
        self.corruption_detected += 1;
    }
    
    /// Record memory-mapped operation
    pub fn record_mmap_operation(&mut self) {
        self.mmap_operations += 1;
    }
    
    /// Record disk operation
    pub fn record_disk_operation(&mut self) {
        self.disk_operations += 1;
    }
    
    /// Calculate cache hit ratio
    pub fn cache_hit_ratio(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
    
    /// Calculate checksum failure ratio
    pub fn checksum_failure_ratio(&self) -> f64 {
        if self.checksum_verifications == 0 {
            0.0
        } else {
            self.checksum_failures as f64 / self.checksum_verifications as f64
        }
    }
}