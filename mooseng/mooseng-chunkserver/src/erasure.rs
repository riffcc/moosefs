// Reed-Solomon erasure coding implementation for MooseNG
// Provides efficient data encoding and decoding for improved storage efficiency

use anyhow::{anyhow, Result};
use reed_solomon_erasure::{galois_8::ReedSolomon, Error as ReedSolomonError};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn, error};
use bytes::{Bytes, BytesMut};
use mooseng_common::types::{ChunkId, ChunkVersion};
use crate::{Chunk, ChunkMetadata, ChecksumType, ChunkChecksum};

/// Erasure coding configuration
#[derive(Debug, Clone)]
pub struct ErasureConfig {
    /// Number of data shards
    pub data_shards: usize,
    /// Number of parity shards
    pub parity_shards: usize,
    /// Maximum shard size in bytes
    pub max_shard_size: usize,
}

impl ErasureConfig {
    /// Create a new erasure coding configuration
    pub fn new(data_shards: usize, parity_shards: usize) -> Self {
        Self {
            data_shards,
            parity_shards,
            max_shard_size: 64 * 1024 * 1024, // 64MB default
        }
    }
    
    /// Create 4+2 configuration (4 data + 2 parity)
    pub fn config_4_2() -> Self {
        Self::new(4, 2)
    }
    
    /// Create 8+3 configuration (8 data + 3 parity)
    pub fn config_8_3() -> Self {
        Self::new(8, 3)
    }
    
    /// Create 8+4 configuration (8 data + 4 parity)
    pub fn config_8_4() -> Self {
        Self::new(8, 4)
    }
    
    /// Create 16+4 configuration (16 data + 4 parity)
    pub fn config_16_4() -> Self {
        Self::new(16, 4)
    }
    
    /// Get configuration by name
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "4+2" => Ok(Self::config_4_2()),
            "8+3" => Ok(Self::config_8_3()),
            "8+4" => Ok(Self::config_8_4()),
            "16+4" => Ok(Self::config_16_4()),
            _ => Err(anyhow!("Unknown erasure configuration: {}", name)),
        }
    }
    
    /// Total number of shards
    pub fn total_shards(&self) -> usize {
        self.data_shards + self.parity_shards
    }
}

/// Erasure coding encoder/decoder
pub struct ErasureCoder {
    config: ErasureConfig,
    encoder: Arc<Mutex<ReedSolomon>>,
}

impl ErasureCoder {
    /// Create a new erasure coder
    pub fn new(config: ErasureConfig) -> Result<Self> {
        let encoder = ReedSolomon::new(config.data_shards, config.parity_shards)
            .map_err(|e| anyhow!("Failed to create Reed-Solomon encoder: {:?}", e))?;
            
        Ok(Self {
            config,
            encoder: Arc::new(Mutex::new(encoder)),
        })
    }
    
    /// Encode data into shards
    pub async fn encode(&self, data: Bytes) -> Result<Vec<Bytes>> {
        debug!("Encoding {} bytes into {} data + {} parity shards", 
               data.len(), self.config.data_shards, self.config.parity_shards);
        
        // Calculate shard size (pad to make evenly divisible)
        let shard_size = (data.len() + self.config.data_shards - 1) / self.config.data_shards;
        
        if shard_size > self.config.max_shard_size {
            return Err(anyhow!("Data too large: shard size {} exceeds maximum {}", 
                              shard_size, self.config.max_shard_size));
        }
        
        // Create data shards
        let mut shards = Vec::with_capacity(self.config.total_shards());
        for i in 0..self.config.data_shards {
            let start = i * shard_size;
            let end = std::cmp::min(start + shard_size, data.len());
            
            let mut shard = BytesMut::with_capacity(shard_size);
            shard.extend_from_slice(&data[start..end]);
            
            // Pad last shard if necessary
            if shard.len() < shard_size {
                shard.resize(shard_size, 0);
            }
            
            shards.push(shard.freeze());
        }
        
        // Create parity shards (empty, will be filled by encoder)
        for _ in 0..self.config.parity_shards {
            shards.push(Bytes::from(vec![0u8; shard_size]));
        }
        
        // Encode to generate parity shards
        self.encode_shards(&mut shards).await?;
        
        info!("Successfully encoded {} bytes into {} shards", data.len(), shards.len());
        Ok(shards)
    }
    
    /// Decode data from shards
    pub async fn decode(&self, mut shards: Vec<Option<Bytes>>, data_size: usize) -> Result<Bytes> {
        debug!("Decoding from {} shards (data size: {})", shards.len(), data_size);
        
        // Verify we have enough shards
        let available_shards = shards.iter().filter(|s| s.is_some()).count();
        if available_shards < self.config.data_shards {
            return Err(anyhow!("Not enough shards for decoding: {} available, {} required",
                              available_shards, self.config.data_shards));
        }
        
        // Reconstruct missing shards
        self.reconstruct_shards(&mut shards).await?;
        
        // Combine data shards
        let mut result = BytesMut::with_capacity(data_size);
        for i in 0..self.config.data_shards {
            if let Some(shard) = &shards[i] {
                result.extend_from_slice(shard);
            } else {
                return Err(anyhow!("Failed to reconstruct data shard {}", i));
            }
        }
        
        // Trim to original size
        result.truncate(data_size);
        
        info!("Successfully decoded {} bytes from shards", data_size);
        Ok(result.freeze())
    }
    
    /// Encode shards to generate parity
    async fn encode_shards(&self, shards: &mut Vec<Bytes>) -> Result<()> {
        let encoder = self.encoder.lock().await;
        
        // Convert to mutable byte arrays for encoding
        let mut shard_refs: Vec<Vec<u8>> = shards.iter()
            .map(|s| s.to_vec())
            .collect();
        
        // Encode
        encoder.encode(&mut shard_refs)
            .map_err(|e| anyhow!("Encoding failed: {:?}", e))?;
        
        // Update parity shards
        for i in self.config.data_shards..self.config.total_shards() {
            shards[i] = Bytes::from(shard_refs[i].clone());
        }
        
        Ok(())
    }
    
    /// Reconstruct missing shards
    async fn reconstruct_shards(&self, shards: &mut Vec<Option<Bytes>>) -> Result<()> {
        let encoder = self.encoder.lock().await;
        
        // Convert to format expected by reed-solomon library
        let mut shard_refs: Vec<Option<Vec<u8>>> = shards.iter()
            .map(|s| s.as_ref().map(|b| b.to_vec()))
            .collect();
        
        // Reconstruct
        encoder.reconstruct(&mut shard_refs)
            .map_err(|e| anyhow!("Reconstruction failed: {:?}", e))?;
        
        // Update reconstructed shards
        for (i, shard_ref) in shard_refs.into_iter().enumerate() {
            if shards[i].is_none() {
                if let Some(data) = shard_ref {
                    shards[i] = Some(Bytes::from(data));
                }
            }
        }
        
        Ok(())
    }
    
    /// Verify integrity of shards
    pub async fn verify_shards(&self, shards: &[Bytes]) -> Result<bool> {
        if shards.len() != self.config.total_shards() {
            return Ok(false);
        }
        
        let encoder = self.encoder.lock().await;
        
        // Convert to format for verification
        let shard_refs: Vec<Vec<u8>> = shards.iter()
            .map(|s| s.to_vec())
            .collect();
        
        match encoder.verify(&shard_refs) {
            Ok(valid) => Ok(valid),
            Err(_) => Ok(false),
        }
    }
}

/// Represents an erasure-coded shard
#[derive(Debug, Clone)]
pub struct ErasureShard {
    /// Index of this shard (0 to total_shards-1)
    pub index: usize,
    /// Whether this is a data shard (vs parity)
    pub is_data: bool,
    /// The actual shard data
    pub data: Bytes,
    /// Checksum of this shard
    pub checksum: ChunkChecksum,
}

impl ErasureShard {
    pub fn new(index: usize, is_data: bool, data: Bytes) -> Self {
        let checksum = ChunkChecksum::blake3(&data);
        Self {
            index,
            is_data,
            data,
            checksum,
        }
    }
    
    pub fn verify(&self) -> bool {
        self.checksum.verify(&self.data)
    }
}

/// Erasure-coded chunk representation
#[derive(Debug, Clone)]
pub struct ErasureCodedChunk {
    /// Original chunk metadata
    pub metadata: ChunkMetadata,
    /// Erasure coding configuration used
    pub config: ErasureConfig,
    /// Individual shards
    pub shards: Vec<Option<ErasureShard>>,
    /// Original data size before encoding
    pub original_size: u64,
}

impl ErasureCodedChunk {
    /// Create from existing chunk by encoding it
    pub async fn from_chunk(chunk: Chunk, config: ErasureConfig) -> Result<Self> {
        let coder = ErasureCoder::new(config.clone())?;
        let original_size = chunk.size();
        let shards_data = coder.encode(chunk.data.clone()).await?;
        
        let shards = shards_data.into_iter()
            .enumerate()
            .map(|(index, data)| {
                let is_data = index < config.data_shards;
                Some(ErasureShard::new(index, is_data, data))
            })
            .collect();
        
        Ok(Self {
            metadata: chunk.metadata,
            config,
            shards,
            original_size,
        })
    }
    
    /// Reconstruct the original chunk from shards
    pub async fn reconstruct(&self) -> Result<Chunk> {
        let coder = ErasureCoder::new(self.config.clone())?;
        
        let shard_options: Vec<Option<Bytes>> = self.shards.iter()
            .map(|opt_shard| opt_shard.as_ref().map(|s| s.data.clone()))
            .collect();
        
        let data = coder.decode(shard_options, self.original_size as usize).await?;
        
        Ok(Chunk {
            metadata: self.metadata.clone(),
            data,
        })
    }
    
    /// Get the number of available shards
    pub fn available_shards(&self) -> usize {
        self.shards.iter().filter(|s| s.is_some()).count()
    }
    
    /// Check if we have enough shards to reconstruct
    pub fn can_reconstruct(&self) -> bool {
        self.available_shards() >= self.config.data_shards
    }
}

/// Shard placement strategy for distributing shards across chunk servers
#[derive(Debug, Clone)]
pub struct ShardPlacement {
    config: ErasureConfig,
    /// Map from server ID to its region/rack info
    server_topology: HashMap<String, ServerLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerLocation {
    pub region: String,
    pub rack: String,
    pub server_id: String,
}

impl ShardPlacement {
    pub fn new(config: ErasureConfig) -> Self {
        Self { 
            config,
            server_topology: HashMap::new(),
        }
    }
    
    /// Update server topology information
    pub fn update_topology(&mut self, server_id: String, location: ServerLocation) {
        self.server_topology.insert(server_id, location);
    }
    
    /// Calculate optimal shard placement across available servers
    pub fn calculate_placement(&self, servers: &[String], region_aware: bool) -> Vec<Vec<usize>> {
        if !region_aware || self.server_topology.is_empty() {
            // Simple round-robin for non-region-aware or when topology unknown
            return self.round_robin_placement(servers);
        }
        
        // Region-aware placement
        self.region_aware_placement(servers)
    }
    
    /// Simple round-robin placement
    fn round_robin_placement(&self, servers: &[String]) -> Vec<Vec<usize>> {
        let mut placement = vec![vec![]; self.config.total_shards()];
        
        for (i, _) in placement.iter_mut().enumerate() {
            let server_idx = i % servers.len();
            placement[i] = vec![server_idx];
        }
        
        placement
    }
    
    /// Region-aware placement to maximize fault tolerance
    fn region_aware_placement(&self, servers: &[String]) -> Vec<Vec<usize>> {
        let mut placement = vec![vec![]; self.config.total_shards()];
        
        // Group servers by region and rack
        let mut regions: HashMap<String, HashMap<String, Vec<usize>>> = HashMap::new();
        
        for (idx, server_id) in servers.iter().enumerate() {
            if let Some(location) = self.server_topology.get(server_id) {
                regions
                    .entry(location.region.clone())
                    .or_insert_with(HashMap::new)
                    .entry(location.rack.clone())
                    .or_insert_with(Vec::new)
                    .push(idx);
            }
        }
        
        // Distribute shards across regions and racks
        let mut shard_idx = 0;
        let region_keys: Vec<_> = regions.keys().cloned().collect();
        
        'outer: for region_round in 0.. {
            for region in &region_keys {
                if let Some(racks) = regions.get(region) {
                    let rack_keys: Vec<_> = racks.keys().cloned().collect();
                    let rack_idx = region_round % rack_keys.len();
                    
                    if let Some(rack) = rack_keys.get(rack_idx) {
                        if let Some(servers_in_rack) = racks.get(rack) {
                            let server_idx = region_round % servers_in_rack.len();
                            
                            if let Some(&server) = servers_in_rack.get(server_idx) {
                                placement[shard_idx] = vec![server];
                                shard_idx += 1;
                                
                                if shard_idx >= self.config.total_shards() {
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Fill any remaining with round-robin
        while shard_idx < self.config.total_shards() {
            placement[shard_idx] = vec![shard_idx % servers.len()];
            shard_idx += 1;
        }
        
        placement
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_encode_decode_4_2() {
        let coder = ErasureCoder::new(ErasureConfig::config_4_2()).unwrap();
        
        let original = Bytes::from(vec![1u8; 1024 * 1024]); // 1MB
        let data_size = original.len();
        
        // Encode
        let shards = coder.encode(original.clone()).await.unwrap();
        assert_eq!(shards.len(), 6); // 4 data + 2 parity
        
        // Decode with all shards
        let mut decode_shards: Vec<Option<Bytes>> = shards.iter()
            .map(|s| Some(s.clone()))
            .collect();
        let decoded = coder.decode(decode_shards, data_size).await.unwrap();
        assert_eq!(decoded, original);
        
        // Decode with 2 missing shards
        let mut partial_shards: Vec<Option<Bytes>> = shards.iter()
            .enumerate()
            .map(|(i, s)| if i < 2 { None } else { Some(s.clone()) })
            .collect();
        let decoded_partial = coder.decode(partial_shards, data_size).await.unwrap();
        assert_eq!(decoded_partial, original);
    }
    
    #[tokio::test]
    async fn test_verify_shards() {
        let coder = ErasureCoder::new(ErasureConfig::config_8_3()).unwrap();
        
        let original = Bytes::from(vec![42u8; 8192]);
        let shards = coder.encode(original).await.unwrap();
        
        // Verify valid shards
        assert!(coder.verify_shards(&shards).await.unwrap());
        
        // Corrupt a shard
        let mut corrupted = shards.clone();
        corrupted[0] = Bytes::from(vec![0u8; corrupted[0].len()]);
        assert!(!coder.verify_shards(&corrupted).await.unwrap());
    }
}