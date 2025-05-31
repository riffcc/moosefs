//! Compression support for MooseNG
//! 
//! This module provides transparent compression and decompression capabilities
//! using multiple algorithms optimized for different data types and use cases.

use anyhow::{Result, anyhow};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::Instant;
use tracing::{debug, warn};

/// Supported compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// LZ4 - Fast compression with good ratio
    Lz4,
    /// Zstandard - Excellent compression ratio
    Zstd,
    /// Gzip - Standard compression with good compatibility
    Gzip,
    /// Automatic selection based on data characteristics
    Auto,
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        Self::Auto
    }
}

/// Compression levels for algorithms that support them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionLevel {
    /// Fastest compression
    Fastest,
    /// Balanced compression and speed
    Balanced,
    /// Maximum compression ratio
    Maximum,
    /// Custom level (algorithm-specific)
    Custom(i32),
}

impl Default for CompressionLevel {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Default algorithm to use
    pub algorithm: CompressionAlgorithm,
    /// Compression level
    pub level: CompressionLevel,
    /// Minimum size to compress (bytes)
    pub min_size: usize,
    /// Maximum size to attempt compression (bytes)
    pub max_size: usize,
    /// Sample size for auto-detection (bytes)
    pub sample_size: usize,
    /// Enable compression ratio tracking
    pub track_ratio: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::Auto,
            level: CompressionLevel::Balanced,
            min_size: 1024,        // 1KB minimum
            max_size: 100 << 20,   // 100MB maximum
            sample_size: 4096,     // 4KB sample for auto-detection
            track_ratio: true,
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub bytes_compressed: u64,
    pub bytes_decompressed: u64,
    pub compression_time_us: u64,
    pub decompression_time_us: u64,
    pub total_ratio: f64,
    pub algorithm_usage: std::collections::HashMap<CompressionAlgorithm, u64>,
}

/// Compressed data container
#[derive(Debug, Clone)]
pub struct CompressedData {
    /// Compressed payload
    pub data: Bytes,
    /// Algorithm used for compression
    pub algorithm: CompressionAlgorithm,
    /// Original size before compression
    pub original_size: usize,
    /// Compression ratio achieved
    pub ratio: f64,
}

/// Main compression engine
pub struct CompressionEngine {
    config: CompressionConfig,
    stats: std::sync::Arc<std::sync::Mutex<CompressionStats>>,
}

impl CompressionEngine {
    /// Create a new compression engine
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config,
            stats: std::sync::Arc::new(std::sync::Mutex::new(CompressionStats::default())),
        }
    }

    /// Compress data using the configured algorithm
    pub fn compress(&self, data: &[u8]) -> Result<CompressedData> {
        if data.len() < self.config.min_size {
            return Ok(CompressedData {
                data: Bytes::copy_from_slice(data),
                algorithm: CompressionAlgorithm::None,
                original_size: data.len(),
                ratio: 1.0,
            });
        }

        if data.len() > self.config.max_size {
            warn!("Data size {} exceeds maximum compression size {}", 
                  data.len(), self.config.max_size);
            return Ok(CompressedData {
                data: Bytes::copy_from_slice(data),
                algorithm: CompressionAlgorithm::None,
                original_size: data.len(),
                ratio: 1.0,
            });
        }

        let algorithm = match self.config.algorithm {
            CompressionAlgorithm::Auto => self.select_algorithm(data)?,
            algo => algo,
        };

        let start = Instant::now();
        let compressed = self.compress_with_algorithm(data, algorithm)?;
        let duration = start.elapsed();

        let ratio = data.len() as f64 / compressed.len() as f64;

        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            stats.bytes_compressed += data.len() as u64;
            stats.compression_time_us += duration.as_micros() as u64;
            stats.total_ratio = (stats.total_ratio + ratio) / 2.0; // Simple moving average
            *stats.algorithm_usage.entry(algorithm).or_insert(0) += 1;
        }

        debug!("Compressed {} bytes to {} bytes using {:?} (ratio: {:.2}x) in {:?}",
               data.len(), compressed.len(), algorithm, ratio, duration);

        Ok(CompressedData {
            data: compressed,
            algorithm,
            original_size: data.len(),
            ratio,
        })
    }

    /// Decompress data
    pub fn decompress(&self, compressed: &CompressedData) -> Result<Bytes> {
        if compressed.algorithm == CompressionAlgorithm::None {
            return Ok(compressed.data.clone());
        }

        let start = Instant::now();
        let decompressed = self.decompress_with_algorithm(&compressed.data, compressed.algorithm)?;
        let duration = start.elapsed();

        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            stats.bytes_decompressed += decompressed.len() as u64;
            stats.decompression_time_us += duration.as_micros() as u64;
        }

        debug!("Decompressed {} bytes to {} bytes using {:?} in {:?}",
               compressed.data.len(), decompressed.len(), compressed.algorithm, duration);

        Ok(decompressed)
    }

    /// Get compression statistics
    pub fn stats(&self) -> CompressionStats {
        self.stats.lock().unwrap().clone()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            *stats = CompressionStats::default();
        }
    }

    /// Select the best algorithm for the given data
    fn select_algorithm(&self, data: &[u8]) -> Result<CompressionAlgorithm> {
        let sample_size = std::cmp::min(self.config.sample_size, data.len());
        let sample = &data[..sample_size];

        // Calculate entropy as a heuristic for compressibility
        let entropy = self.calculate_entropy(sample);
        
        // High entropy (> 7.5) suggests data is already compressed or random
        if entropy > 7.5 {
            return Ok(CompressionAlgorithm::None);
        }
        
        // Medium-high entropy (6.0-7.5) - use fast compression
        if entropy > 6.0 {
            return Ok(CompressionAlgorithm::Lz4);
        }
        
        // Low-medium entropy (< 6.0) - use high compression
        Ok(CompressionAlgorithm::Zstd)
    }

    /// Calculate Shannon entropy of data
    fn calculate_entropy(&self, data: &[u8]) -> f64 {
        let mut counts = [0u64; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }

        let len = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &counts {
            if count > 0 {
                let p = count as f64 / len;
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    /// Compress data with a specific algorithm
    fn compress_with_algorithm(&self, data: &[u8], algorithm: CompressionAlgorithm) -> Result<Bytes> {
        match algorithm {
            CompressionAlgorithm::None => Ok(Bytes::copy_from_slice(data)),
            CompressionAlgorithm::Lz4 => self.compress_lz4(data),
            CompressionAlgorithm::Zstd => self.compress_zstd(data),
            CompressionAlgorithm::Gzip => self.compress_gzip(data),
            CompressionAlgorithm::Auto => unreachable!("Auto should be resolved before this point"),
        }
    }

    /// Decompress data with a specific algorithm
    fn decompress_with_algorithm(&self, data: &[u8], algorithm: CompressionAlgorithm) -> Result<Bytes> {
        match algorithm {
            CompressionAlgorithm::None => Ok(Bytes::copy_from_slice(data)),
            CompressionAlgorithm::Lz4 => self.decompress_lz4(data),
            CompressionAlgorithm::Zstd => self.decompress_zstd(data),
            CompressionAlgorithm::Gzip => self.decompress_gzip(data),
            CompressionAlgorithm::Auto => unreachable!("Auto should be resolved before this point"),
        }
    }

    /// LZ4 compression
    fn compress_lz4(&self, data: &[u8]) -> Result<Bytes> {
        let compressed = lz4_flex::compress_prepend_size(data);
        Ok(Bytes::from(compressed))
    }

    /// LZ4 decompression
    fn decompress_lz4(&self, data: &[u8]) -> Result<Bytes> {
        let decompressed = lz4_flex::decompress_size_prepended(data)
            .map_err(|e| anyhow!("LZ4 decompression failed: {}", e))?;
        Ok(Bytes::from(decompressed))
    }

    /// Zstandard compression
    fn compress_zstd(&self, data: &[u8]) -> Result<Bytes> {
        let level = match self.config.level {
            CompressionLevel::Fastest => 1,
            CompressionLevel::Balanced => 3,
            CompressionLevel::Maximum => 19,
            CompressionLevel::Custom(level) => level,
        };

        let compressed = zstd::bulk::compress(data, level)
            .map_err(|e| anyhow!("Zstd compression failed: {}", e))?;
        Ok(Bytes::from(compressed))
    }

    /// Zstandard decompression
    fn decompress_zstd(&self, data: &[u8]) -> Result<Bytes> {
        let decompressed = zstd::bulk::decompress(data, 16 << 20) // 16MB limit
            .map_err(|e| anyhow!("Zstd decompression failed: {}", e))?;
        Ok(Bytes::from(decompressed))
    }

    /// Gzip compression
    fn compress_gzip(&self, data: &[u8]) -> Result<Bytes> {
        use flate2::Compression;
        use flate2::write::GzEncoder;

        let level = match self.config.level {
            CompressionLevel::Fastest => Compression::fast(),
            CompressionLevel::Balanced => Compression::default(),
            CompressionLevel::Maximum => Compression::best(),
            CompressionLevel::Custom(level) => Compression::new(level as u32),
        };

        let mut encoder = GzEncoder::new(Vec::new(), level);
        encoder.write_all(data)
            .map_err(|e| anyhow!("Gzip write failed: {}", e))?;
        let compressed = encoder.finish()
            .map_err(|e| anyhow!("Gzip compression failed: {}", e))?;
        Ok(Bytes::from(compressed))
    }

    /// Gzip decompression
    fn decompress_gzip(&self, data: &[u8]) -> Result<Bytes> {
        use flate2::read::GzDecoder;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| anyhow!("Gzip decompression failed: {}", e))?;
        Ok(Bytes::from(decompressed))
    }
}

/// Convenience functions for quick compression/decompression
pub mod quick {
    use super::*;

    /// Quick compression with default settings
    pub fn compress(data: &[u8]) -> Result<CompressedData> {
        let engine = CompressionEngine::new(CompressionConfig::default());
        engine.compress(data)
    }

    /// Quick decompression
    pub fn decompress(compressed: &CompressedData) -> Result<Bytes> {
        let engine = CompressionEngine::new(CompressionConfig::default());
        engine.decompress(compressed)
    }

    /// Compress with specific algorithm
    pub fn compress_with(data: &[u8], algorithm: CompressionAlgorithm) -> Result<CompressedData> {
        let config = CompressionConfig {
            algorithm,
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);
        engine.compress(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_compression_small_data() {
        let engine = CompressionEngine::new(CompressionConfig::default());
        let data = b"small";
        let compressed = engine.compress(data).unwrap();
        
        assert_eq!(compressed.algorithm, CompressionAlgorithm::None);
        assert_eq!(compressed.ratio, 1.0);
    }

    #[test]
    fn test_lz4_compression() {
        let config = CompressionConfig {
            algorithm: CompressionAlgorithm::Lz4,
            min_size: 0,
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);
        
        let data = b"hello world hello world hello world hello world";
        let compressed = engine.compress(data).unwrap();
        let decompressed = engine.decompress(&compressed).unwrap();
        
        assert_eq!(compressed.algorithm, CompressionAlgorithm::Lz4);
        assert_eq!(decompressed.as_ref(), data);
        assert!(compressed.ratio > 1.0);
    }

    #[test]
    fn test_zstd_compression() {
        let config = CompressionConfig {
            algorithm: CompressionAlgorithm::Zstd,
            min_size: 0,
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);
        
        let data = b"hello world hello world hello world hello world";
        let compressed = engine.compress(data).unwrap();
        let decompressed = engine.decompress(&compressed).unwrap();
        
        assert_eq!(compressed.algorithm, CompressionAlgorithm::Zstd);
        assert_eq!(decompressed.as_ref(), data);
        assert!(compressed.ratio > 1.0);
    }

    #[test]
    fn test_gzip_compression() {
        let config = CompressionConfig {
            algorithm: CompressionAlgorithm::Gzip,
            min_size: 0,
            ..Default::default()
        };
        let engine = CompressionEngine::new(config);
        
        let data = b"hello world hello world hello world hello world";
        let compressed = engine.compress(data).unwrap();
        let decompressed = engine.decompress(&compressed).unwrap();
        
        assert_eq!(compressed.algorithm, CompressionAlgorithm::Gzip);
        assert_eq!(decompressed.as_ref(), data);
        assert!(compressed.ratio > 1.0);
    }

    #[test]
    fn test_auto_selection_random_data() {
        let engine = CompressionEngine::new(CompressionConfig::default());
        
        // Random data should not be compressed
        let mut random_data = vec![0u8; 4096];
        for i in 0..random_data.len() {
            random_data[i] = (i % 256) as u8;
        }
        
        let compressed = engine.compress(&random_data).unwrap();
        // High entropy data should either not be compressed or use fast compression
        assert!(compressed.algorithm == CompressionAlgorithm::None || 
                compressed.algorithm == CompressionAlgorithm::Lz4);
    }

    #[test]
    fn test_auto_selection_repetitive_data() {
        let engine = CompressionEngine::new(CompressionConfig::default());
        
        // Repetitive data should be compressed well
        let repetitive_data = vec![b'A'; 4096];
        
        let compressed = engine.compress(&repetitive_data).unwrap();
        // Low entropy data should use good compression
        assert!(compressed.algorithm == CompressionAlgorithm::Zstd);
        assert!(compressed.ratio > 10.0); // Should compress very well
    }

    #[test]
    fn test_entropy_calculation() {
        let engine = CompressionEngine::new(CompressionConfig::default());
        
        // All same byte - entropy should be 0
        let uniform_data = vec![b'A'; 1000];
        let entropy = engine.calculate_entropy(&uniform_data);
        assert_eq!(entropy, 0.0);
        
        // Two different bytes - entropy should be 1
        let mut binary_data = vec![b'A'; 500];
        binary_data.extend(vec![b'B'; 500]);
        let entropy = engine.calculate_entropy(&binary_data);
        assert!((entropy - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_quick_api() {
        let data = b"hello world hello world hello world hello world";
        
        let compressed = quick::compress(data).unwrap();
        let decompressed = quick::decompress(&compressed).unwrap();
        
        assert_eq!(decompressed.as_ref(), data);
    }

    #[test]
    fn test_statistics() {
        let engine = CompressionEngine::new(CompressionConfig::default());
        let data = vec![b'A'; 10000];
        
        engine.compress(&data).unwrap();
        
        let stats = engine.stats();
        assert_eq!(stats.bytes_compressed, 10000);
        assert!(stats.compression_time_us > 0);
        assert!(stats.total_ratio > 1.0);
    }
}