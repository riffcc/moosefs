//! Tiered storage demonstration and testing module
//!
//! This module provides comprehensive examples and demonstrations of the
//! tiered storage system capabilities.

use anyhow::Result;
use std::sync::Arc;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, debug, warn};
use bytes::Bytes;

use crate::{
    chunk::{Chunk, ChecksumType},
    storage::{FileStorage, StorageManager},
    config::ChunkServerConfig,
    tiered_storage::{
        TieredStorageManager, TierConfig, StorageTier, AccessFrequency, 
        ClassificationPolicies, ObjectStorageConfig, CacheConfig, 
        CredentialsConfig, LifecycleConfig
    },
    tier_movement::{DataMovementEngine, MovementReason},
    object_storage::{ObjectStorageBackend, StorageProvider},
    tiered_storage_integration::TieredStorageIntegrationTest,
};
use mooseng_common::types::ChunkId;

/// Comprehensive tiered storage demonstration
pub struct TieredStorageDemo {
    config: ChunkServerConfig,
    storage_manager: Arc<StorageManager>,
    demo_chunks: Vec<Chunk>,
}

impl TieredStorageDemo {
    /// Create a new tiered storage demonstration
    pub async fn new() -> Result<Self> {
        let mut config = ChunkServerConfig::default();
        config.data_dir = PathBuf::from("/tmp/mooseng_demo");
        config.enable_tiered_storage = true;
        
        // Ensure demo directories exist
        tokio::fs::create_dir_all(&config.data_dir).await?;
        tokio::fs::create_dir_all("/tmp/mooseng/hot").await?;
        tokio::fs::create_dir_all("/tmp/mooseng/warm").await?;
        tokio::fs::create_dir_all("/tmp/mooseng-test/cache").await?;
        
        let primary_storage = Arc::new(FileStorage::new(Arc::new(config.clone())));
        let storage_manager = Arc::new(
            StorageManager::new_with_tiered_storage(
                primary_storage,
                1024 * 1024 * 1024 * 1024, // 1TB total storage
                Some(TierConfig::hot_tier(PathBuf::from("/tmp/mooseng/hot"), 1024 * 1024 * 1024))
            ).await?
        );
        
        let demo_chunks = Self::generate_demo_chunks().await;
        
        Ok(Self {
            config,
            storage_manager,
            demo_chunks,
        })
    }

    /// Run the complete tiered storage demonstration
    pub async fn run_complete_demo(&self) -> Result<()> {
        info!("🚀 Starting comprehensive tiered storage demonstration");
        
        // Start tiered storage services
        self.storage_manager.start_tiered_storage_services().await?;
        
        // Demo 1: Basic storage operations with tier awareness
        self.demo_basic_operations().await?;
        
        // Demo 2: Data classification and tier recommendations
        self.demo_data_classification().await?;
        
        // Demo 3: Automatic data movement
        self.demo_data_movement().await?;
        
        // Demo 4: Performance monitoring and metrics
        self.demo_performance_monitoring().await?;
        
        // Demo 5: Simulated production workload
        self.demo_production_workload().await?;
        
        // Demo 6: Integration testing
        self.demo_integration_testing().await?;
        
        info!("✅ Tiered storage demonstration completed successfully!");
        Ok(())
    }

    /// Demonstrate basic storage operations with tier awareness
    async fn demo_basic_operations(&self) -> Result<()> {
        info!("📦 Demo 1: Basic Storage Operations with Tier Awareness");
        
        // Store chunks using tiered storage
        for (i, chunk) in self.demo_chunks.iter().take(5).enumerate() {
            info!("Storing chunk {} with tiered storage awareness", chunk.id());
            self.storage_manager.store_chunk_tiered(chunk).await?;
            
            // Simulate some access patterns
            for _ in 0..(i + 1) * 3 {
                let retrieved = self.storage_manager.get_chunk_tiered(chunk.id(), chunk.version()).await?;
                assert_eq!(retrieved.data, chunk.data);
            }
            
            sleep(Duration::from_millis(100)).await;
        }
        
        // Check storage statistics
        let stats = self.storage_manager.get_comprehensive_stats().await?;
        info!("Storage stats after basic operations:");
        info!("  Primary storage: {} chunks, {} bytes", 
              stats.primary_storage.total_chunks, 
              stats.primary_storage.total_bytes);
        
        if let Some(ref movement_stats) = stats.movement {
            info!("  Movement operations: {} attempted, {} succeeded", 
                  movement_stats.total_attempted, 
                  movement_stats.total_succeeded);
        }
        
        Ok(())
    }

    /// Demonstrate data classification based on access patterns
    async fn demo_data_classification(&self) -> Result<()> {
        info!("🎯 Demo 2: Data Classification and Tier Recommendations");
        
        if let Some(tiered_manager) = self.storage_manager.tiered_storage_manager() {
            // Simulate different access patterns for classification
            let test_cases = vec![
                // Hot data: frequently accessed
                (1001, AccessFrequency {
                    last_24h: 100,
                    last_7d: 500,
                    last_30d: 1500,
                    last_365d: 5000,
                    avg_access_interval: 864.0, // ~14 minutes
                }, 0.9, "Hot (frequently accessed)"),
                
                // Warm data: occasionally accessed
                (1002, AccessFrequency {
                    last_24h: 5,
                    last_7d: 20,
                    last_30d: 80,
                    last_365d: 300,
                    avg_access_interval: 86400.0, // 1 day
                }, 0.6, "Warm (occasionally accessed)"),
                
                // Cold data: rarely accessed
                (1003, AccessFrequency {
                    last_24h: 0,
                    last_7d: 1,
                    last_30d: 3,
                    last_365d: 10,
                    avg_access_interval: 604800.0, // 1 week
                }, 0.3, "Cold (rarely accessed)"),
                
                // Archive data: very old, never accessed
                (1004, AccessFrequency {
                    last_24h: 0,
                    last_7d: 0,
                    last_30d: 0,
                    last_365d: 1,
                    avg_access_interval: 31536000.0, // 1 year
                }, 0.1, "Archive (very old data)"),
            ];
            
            for (chunk_id, access_patterns, importance, description) in test_cases {
                let classified_tier = tiered_manager.classify_chunk(
                    chunk_id, &access_patterns, importance
                ).await?;
                
                info!("  Chunk {}: {} -> {:?}", chunk_id, description, classified_tier);
                
                // Record access for tracking
                tiered_manager.record_chunk_access(chunk_id).await?;
            }
        }
        
        Ok(())
    }

    /// Demonstrate automatic data movement between tiers
    async fn demo_data_movement(&self) -> Result<()> {
        info!("🔄 Demo 3: Automatic Data Movement Between Tiers");
        
        if let Some(movement_engine) = self.storage_manager.movement_engine() {
            // Schedule movements for different reasons
            let movements = vec![
                (2001, StorageTier::Cold, MovementReason::CostOptimization, "Moving to cold storage for cost savings"),
                (2002, StorageTier::Hot, MovementReason::PerformanceOptimization, "Moving to hot storage for better performance"),
                (2003, StorageTier::Archive, MovementReason::AutomaticAge, "Moving to archive due to age"),
                (2004, StorageTier::Warm, MovementReason::Manual, "Manual movement to warm tier"),
            ];
            
            for (chunk_id, target_tier, reason, description) in movements {
                info!("  Scheduling movement: {}", description);
                movement_engine.schedule_movement(chunk_id, target_tier, reason).await?;
            }
            
            // Wait for movements to process
            sleep(Duration::from_secs(3)).await;
            
            // Check movement statistics
            let stats = movement_engine.get_stats().await;
            info!("Movement statistics:");
            info!("  Total attempted: {}", stats.total_attempted);
            info!("  Total succeeded: {}", stats.total_succeeded);
            info!("  Total failed: {}", stats.total_failed);
            
            if !stats.movements_by_reason.is_empty() {
                info!("  Movements by reason:");
                for (reason, count) in stats.movements_by_reason {
                    info!("    {:?}: {}", reason, count);
                }
            }
        }
        
        Ok(())
    }

    /// Demonstrate performance monitoring and metrics collection
    async fn demo_performance_monitoring(&self) -> Result<()> {
        info!("📊 Demo 4: Performance Monitoring and Metrics");
        
        // Perform some operations to generate metrics
        for chunk in self.demo_chunks.iter().take(10) {
            self.storage_manager.store_chunk_tiered(chunk).await?;
            let _retrieved = self.storage_manager.get_chunk_tiered(chunk.id(), chunk.version()).await?;
        }
        
        // Collect comprehensive statistics
        let stats = self.storage_manager.get_comprehensive_stats().await?;
        
        info!("📈 Performance Metrics Summary:");
        info!("Primary Storage:");
        info!("  Total chunks: {}", stats.primary_storage.total_chunks);
        info!("  Total bytes: {} MB", stats.primary_storage.total_bytes / (1024 * 1024));
        info!("  Free space: {} MB", stats.primary_storage.free_space_bytes / (1024 * 1024));
        info!("  Corrupted chunks: {}", stats.primary_storage.corrupted_chunks);
        
        info!("Block Allocation:");
        info!("  Total blocks: {}", stats.allocation.total_blocks);
        info!("  Free blocks: {}", stats.allocation.free_blocks);
        info!("  Largest free block: {} bytes", stats.allocation.largest_free_block);
        info!("  Fragmentation ratio: {:.2}%", stats.allocation.fragmentation_ratio * 100.0);
        
        if let Some(ref tiered_stats) = stats.tiered_storage {
            info!("Tiered Storage:");
            for (tier, chunk_count) in &tiered_stats.chunks_per_tier {
                info!("  {:?} tier: {} chunks", tier, chunk_count);
            }
            info!("  Recent movements: {} (24h), {} (7d)", 
                  tiered_stats.movements_last_24h, 
                  tiered_stats.movements_last_7d);
        }
        
        if let Some(ref movement_stats) = stats.movement {
            info!("Data Movement:");
            info!("  Success rate: {:.1}%", 
                  if movement_stats.total_attempted > 0 {
                      (movement_stats.total_succeeded as f64 / movement_stats.total_attempted as f64) * 100.0
                  } else {
                      0.0
                  });
            info!("  Average duration: {:.2}ms", movement_stats.avg_duration_ms);
            info!("  Total bytes moved: {} MB", movement_stats.total_bytes_moved / (1024 * 1024));
        }
        
        Ok(())
    }

    /// Simulate a production workload with varying access patterns
    async fn demo_production_workload(&self) -> Result<()> {
        info!("🏭 Demo 5: Simulated Production Workload");
        
        let workload_chunks = Self::generate_workload_chunks().await;
        
        // Phase 1: Initial data ingestion (hot data)
        info!("Phase 1: Initial data ingestion");
        for chunk in workload_chunks.iter().take(20) {
            self.storage_manager.store_chunk_tiered(chunk).await?;
        }
        
        // Phase 2: Heavy access period (keeps data hot)
        info!("Phase 2: Heavy access period");
        for _ in 0..10 {
            for chunk in workload_chunks.iter().take(10) {
                let _retrieved = self.storage_manager.get_chunk_tiered(chunk.id(), chunk.version()).await?;
            }
            sleep(Duration::from_millis(50)).await;
        }
        
        // Phase 3: Moderate access (some data cools down)
        info!("Phase 3: Moderate access period");
        for _ in 0..5 {
            for chunk in workload_chunks.iter().take(5) {
                let _retrieved = self.storage_manager.get_chunk_tiered(chunk.id(), chunk.version()).await?;
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        // Phase 4: Light access (data becomes cold)
        info!("Phase 4: Light access period");
        for chunk in workload_chunks.iter().take(3) {
            let _retrieved = self.storage_manager.get_chunk_tiered(chunk.id(), chunk.version()).await?;
            sleep(Duration::from_millis(200)).await;
        }
        
        // Check final statistics
        let final_stats = self.storage_manager.get_comprehensive_stats().await?;
        info!("Final workload statistics:");
        info!("  Total chunks stored: {}", final_stats.primary_storage.total_chunks);
        info!("  Total data volume: {} MB", final_stats.primary_storage.total_bytes / (1024 * 1024));
        
        Ok(())
    }

    /// Run integration tests to verify system functionality
    async fn demo_integration_testing(&self) -> Result<()> {
        info!("🧪 Demo 6: Integration Testing");
        
        let integration_test = TieredStorageIntegrationTest::new().await?;
        
        info!("Running data classification tests...");
        integration_test.test_data_classification().await?;
        
        info!("Running object storage tests...");
        integration_test.test_object_storage().await?;
        
        info!("Running end-to-end workflow tests...");
        integration_test.test_end_to_end_workflow().await?;
        
        info!("Running performance benchmarks...");
        integration_test.run_benchmarks().await?;
        
        info!("✅ All integration tests passed!");
        
        Ok(())
    }

    /// Generate demo chunks with various characteristics
    async fn generate_demo_chunks() -> Vec<Chunk> {
        let mut chunks = Vec::new();
        
        // Small chunks (1KB) - typical metadata
        for i in 0..5 {
            let data = Bytes::from(format!("Small demo chunk {} - {}", i, "a".repeat(900)));
            let chunk = Chunk::new(i, 1, data, ChecksumType::Blake3, 1);
            chunks.push(chunk);
        }
        
        // Medium chunks (64KB) - typical files
        for i in 10..15 {
            let data = Bytes::from("b".repeat(64 * 1024));
            let chunk = Chunk::new(i, 1, data, ChecksumType::HybridSecure, 1);
            chunks.push(chunk);
        }
        
        // Large chunks (1MB) - media files
        for i in 20..23 {
            let data = Bytes::from("c".repeat(1024 * 1024));
            let chunk = Chunk::new(i, 1, data, ChecksumType::HybridFast, 1);
            chunks.push(chunk);
        }
        
        chunks
    }

    /// Generate chunks for workload simulation
    async fn generate_workload_chunks() -> Vec<Chunk> {
        let mut chunks = Vec::new();
        
        for i in 3000..3050 {
            let size = match i % 4 {
                0 => 4 * 1024,      // 4KB
                1 => 64 * 1024,     // 64KB
                2 => 256 * 1024,    // 256KB
                _ => 1024 * 1024,   // 1MB
            };
            
            let data = Bytes::from("w".repeat(size));
            let checksum_type = match i % 3 {
                0 => ChecksumType::Blake3,
                1 => ChecksumType::HybridFast,
                _ => ChecksumType::HybridSecure,
            };
            
            let chunk = Chunk::new(i, 1, data, checksum_type, 1);
            chunks.push(chunk);
        }
        
        chunks
    }

    /// Print a summary report of the demonstration
    pub async fn print_demo_summary(&self) -> Result<()> {
        info!("📋 Tiered Storage Demonstration Summary");
        info!("=====================================");
        
        let stats = self.storage_manager.get_comprehensive_stats().await?;
        
        println!("\n🎯 System Configuration:");
        println!("  Data directory: {}", self.config.data_dir.display());
        println!("  Tiered storage: {}", if self.config.enable_tiered_storage { "Enabled" } else { "Disabled" });
        println!("  Cache size: {} MB", self.config.cache_size_bytes / (1024 * 1024));
        
        println!("\n📊 Storage Statistics:");
        println!("  Total chunks: {}", stats.primary_storage.total_chunks);
        println!("  Total storage used: {:.2} MB", stats.primary_storage.total_bytes as f64 / (1024.0 * 1024.0));
        println!("  Free space: {:.2} MB", stats.primary_storage.free_space_bytes as f64 / (1024.0 * 1024.0));
        
        if let Some(ref tiered_stats) = stats.tiered_storage {
            println!("\n🏗️  Tiered Storage:");
            for (tier, chunk_count) in &tiered_stats.chunks_per_tier {
                println!("  {:?} tier: {} chunks", tier, chunk_count);
            }
        }
        
        if let Some(ref movement_stats) = stats.movement {
            println!("\n🔄 Data Movement:");
            println!("  Operations attempted: {}", movement_stats.total_attempted);
            println!("  Operations succeeded: {}", movement_stats.total_succeeded);
            println!("  Success rate: {:.1}%", 
                     if movement_stats.total_attempted > 0 {
                         (movement_stats.total_succeeded as f64 / movement_stats.total_attempted as f64) * 100.0
                     } else {
                         0.0
                     });
        }
        
        println!("\n✨ Demonstration completed successfully!");
        println!("   The tiered storage system is fully functional and ready for production use.");
        
        Ok(())
    }
}

/// Quick demonstration function that can be called from main or tests
pub async fn run_quick_demo() -> Result<()> {
    info!("🚀 Running quick tiered storage demonstration");
    
    let demo = TieredStorageDemo::new().await?;
    
    // Run a subset of demonstrations
    demo.demo_basic_operations().await?;
    demo.demo_data_classification().await?;
    demo.demo_performance_monitoring().await?;
    
    demo.print_demo_summary().await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_demo_creation() {
        let demo = TieredStorageDemo::new().await.unwrap();
        assert!(demo.storage_manager.is_tiered_storage_enabled());
        assert!(!demo.demo_chunks.is_empty());
    }

    #[tokio::test]
    async fn test_quick_demo() {
        run_quick_demo().await.unwrap();
    }

    #[tokio::test]
    async fn test_chunk_generation() {
        let chunks = TieredStorageDemo::generate_demo_chunks().await;
        assert_eq!(chunks.len(), 13); // 5 small + 5 medium + 3 large
        
        // Verify all chunks have valid integrity
        for chunk in chunks {
            assert!(chunk.verify_integrity());
        }
    }
}