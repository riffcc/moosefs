use serde::{Deserialize, Serialize};
use mooseng_common::types::{ChunkId, ChunkVersion, Timestamp, now_micros};
use mooseng_common::compression::{CompressionAlgorithm, CompressedData};
use bytes::Bytes;
use crc32fast::Hasher;

/// Checksum type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChecksumType {
    Blake3,
    Crc32,
    XxHash3,
    /// Hybrid approach: xxHash3 for fast verification + Blake3 for cryptographic integrity
    HybridFast,
    /// Enhanced hybrid: xxHash3 + Blake3 + size verification
    HybridSecure,
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
    
    /// Create a new XxHash3 checksum
    pub fn xxhash3(data: &[u8]) -> Self {
        let hash = xxhash_rust::xxh3::xxh3_64(data);
        Self {
            checksum_type: ChecksumType::XxHash3,
            value: hash.to_le_bytes().to_vec(),
        }
    }
    
    /// Create a hybrid fast checksum (xxHash3 + Blake3)
    pub fn hybrid_fast(data: &[u8]) -> Self {
        let xxhash = xxhash_rust::xxh3::xxh3_64(data);
        let blake3_hash = blake3::hash(data);
        
        let mut combined = Vec::with_capacity(8 + 32);
        combined.extend_from_slice(&xxhash.to_le_bytes());
        combined.extend_from_slice(blake3_hash.as_bytes());
        
        Self {
            checksum_type: ChecksumType::HybridFast,
            value: combined,
        }
    }
    
    /// Create a hybrid secure checksum (xxHash3 + Blake3 + size)
    pub fn hybrid_secure(data: &[u8]) -> Self {
        let xxhash = xxhash_rust::xxh3::xxh3_64(data);
        let blake3_hash = blake3::hash(data);
        let size = data.len() as u64;
        
        let mut combined = Vec::with_capacity(8 + 32 + 8);
        combined.extend_from_slice(&xxhash.to_le_bytes());
        combined.extend_from_slice(blake3_hash.as_bytes());
        combined.extend_from_slice(&size.to_le_bytes());
        
        Self {
            checksum_type: ChecksumType::HybridSecure,
            value: combined,
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
            ChecksumType::XxHash3 => {
                let hash = xxhash_rust::xxh3::xxh3_64(data);
                hash.to_le_bytes().to_vec() == self.value
            }
            ChecksumType::HybridFast => {
                if self.value.len() != 40 { // 8 bytes xxHash3 + 32 bytes Blake3
                    return false;
                }
                
                // Verify xxHash3 first (fast check)
                let expected_xxhash = u64::from_le_bytes(
                    self.value[0..8].try_into().unwrap_or([0; 8])
                );
                let actual_xxhash = xxhash_rust::xxh3::xxh3_64(data);
                if expected_xxhash != actual_xxhash {
                    return false;
                }
                
                // Verify Blake3 (cryptographic integrity)
                let expected_blake3 = &self.value[8..40];
                let actual_blake3 = blake3::hash(data);
                expected_blake3 == actual_blake3.as_bytes()
            }
            ChecksumType::HybridSecure => {
                if self.value.len() != 48 { // 8 bytes xxHash3 + 32 bytes Blake3 + 8 bytes size
                    return false;
                }
                
                // Verify size first (fastest check)
                let expected_size = u64::from_le_bytes(
                    self.value[40..48].try_into().unwrap_or([0; 8])
                );
                if expected_size != data.len() as u64 {
                    return false;
                }
                
                // Verify xxHash3 (fast check)
                let expected_xxhash = u64::from_le_bytes(
                    self.value[0..8].try_into().unwrap_or([0; 8])
                );
                let actual_xxhash = xxhash_rust::xxh3::xxh3_64(data);
                if expected_xxhash != actual_xxhash {
                    return false;
                }
                
                // Verify Blake3 (cryptographic integrity)
                let expected_blake3 = &self.value[8..40];
                let actual_blake3 = blake3::hash(data);
                expected_blake3 == actual_blake3.as_bytes()
            }
            ChecksumType::None => true,
        }
    }
    
    /// Fast verification using only the quick checksum component
    pub fn verify_fast(&self, data: &[u8]) -> bool {
        match self.checksum_type {
            ChecksumType::XxHash3 => self.verify(data),
            ChecksumType::HybridFast | ChecksumType::HybridSecure => {
                if self.value.len() < 8 {
                    return false;
                }
                
                // For hybrid types, check size first if available
                if self.checksum_type == ChecksumType::HybridSecure {
                    if self.value.len() >= 48 {
                        let expected_size = u64::from_le_bytes(
                            self.value[40..48].try_into().unwrap_or([0; 8])
                        );
                        if expected_size != data.len() as u64 {
                            return false;
                        }
                    }
                }
                
                // Quick xxHash3 verification
                let expected_xxhash = u64::from_le_bytes(
                    self.value[0..8].try_into().unwrap_or([0; 8])
                );
                let actual_xxhash = xxhash_rust::xxh3::xxh3_64(data);
                expected_xxhash == actual_xxhash
            }
            _ => self.verify(data), // Fallback to full verification
        }
    }
    
    /// Get hex representation of checksum
    pub fn hex(&self) -> String {
        hex::encode(&self.value)
    }
}

/// Chunk metadata stored alongside chunk data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Compression algorithm used for this chunk
    pub compression: CompressionAlgorithm,
    /// Original size before compression (0 if not compressed)
    pub original_size: u64,
    /// Compression ratio achieved (1.0 if not compressed)
    pub compression_ratio: f64,
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
            compression: CompressionAlgorithm::None,
            original_size: 0,
            compression_ratio: 1.0,
        }
    }

    /// Create new chunk metadata with compression info
    pub fn new_with_compression(
        chunk_id: ChunkId,
        version: ChunkVersion,
        size: u64,
        checksum: ChunkChecksum,
        storage_class_id: u8,
        compression: CompressionAlgorithm,
        original_size: u64,
        compression_ratio: f64,
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
            compression,
            original_size,
            compression_ratio,
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
            ChecksumType::XxHash3 => ChunkChecksum::xxhash3(&data),
            ChecksumType::HybridFast => ChunkChecksum::hybrid_fast(&data),
            ChecksumType::HybridSecure => ChunkChecksum::hybrid_secure(&data),
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

    /// Create a new compressed chunk
    pub fn new_compressed(
        chunk_id: ChunkId,
        version: ChunkVersion,
        compressed_data: CompressedData,
        checksum_type: ChecksumType,
        storage_class_id: u8,
    ) -> Self {
        let checksum = match checksum_type {
            ChecksumType::Blake3 => ChunkChecksum::blake3(&compressed_data.data),
            ChecksumType::Crc32 => ChunkChecksum::crc32(&compressed_data.data),
            ChecksumType::XxHash3 => ChunkChecksum::xxhash3(&compressed_data.data),
            ChecksumType::HybridFast => ChunkChecksum::hybrid_fast(&compressed_data.data),
            ChecksumType::HybridSecure => ChunkChecksum::hybrid_secure(&compressed_data.data),
            ChecksumType::None => ChunkChecksum::none(),
        };
        
        let metadata = ChunkMetadata::new_with_compression(
            chunk_id,
            version,
            compressed_data.data.len() as u64,
            checksum,
            storage_class_id,
            compressed_data.algorithm,
            compressed_data.original_size as u64,
            compressed_data.ratio,
        );
        
        Self { 
            metadata, 
            data: compressed_data.data 
        }
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
    
    /// Check if chunk is compressed
    pub fn is_compressed(&self) -> bool {
        self.metadata.compression != CompressionAlgorithm::None
    }
    
    /// Get original size (before compression)
    pub fn original_size(&self) -> u64 {
        if self.is_compressed() {
            self.metadata.original_size
        } else {
            self.data.len() as u64
        }
    }
    
    /// Get compression ratio
    pub fn compression_ratio(&self) -> f64 {
        self.metadata.compression_ratio
    }
    
    /// Get space saved by compression in bytes
    pub fn space_saved(&self) -> u64 {
        if self.is_compressed() {
            self.metadata.original_size.saturating_sub(self.data.len() as u64)
        } else {
            0
        }
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
    
    /// Get chunk checksum
    pub fn checksum(&self) -> &ChunkChecksum {
        &self.metadata.checksum
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
    // Compression metrics
    pub chunks_compressed: u64,
    pub chunks_decompressed: u64,
    pub bytes_saved_by_compression: u64,
    pub compression_ratio_avg: f64,
    pub compression_time_total_us: u64,
    pub decompression_time_total_us: u64,
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
    
    /// Record write operation with timing
    pub fn record_write(&mut self, _duration_micros: u64) {
        self.chunks_stored += 1;
        // TODO: Track timing metrics
    }
    
    /// Record read operation with timing
    pub fn record_read(&mut self, _duration_micros: u64) {
        self.chunks_retrieved += 1;
        // TODO: Track timing metrics
    }
    
    /// Record delete operation
    pub fn record_delete(&mut self) {
        self.chunks_deleted += 1;
    }
    
    /// Record checksum failure
    pub fn record_checksum_failure(&mut self) {
        self.checksum_failures += 1;
    }
    
    /// Get read operation count
    pub fn read_ops(&self) -> u64 {
        self.chunks_retrieved
    }
    
    /// Get write operation count
    pub fn write_ops(&self) -> u64 {
        self.chunks_stored
    }
    
    /// Get delete operation count
    pub fn delete_ops(&self) -> u64 {
        self.chunks_deleted
    }
    
    /// Get checksum failure count
    pub fn checksum_failures(&self) -> u64 {
        self.checksum_failures
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
    
    /// Get total read operations
    pub fn total_reads(&self) -> u64 {
        self.chunks_retrieved
    }
    
    /// Get total write operations
    pub fn total_writes(&self) -> u64 {
        self.chunks_stored
    }
    
    /// Get total delete operations
    pub fn total_deletes(&self) -> u64 {
        self.chunks_deleted
    }
}