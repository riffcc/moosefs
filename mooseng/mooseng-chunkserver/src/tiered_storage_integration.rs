//! Integration tests and examples for the tiered storage system
//! 
//! This module provides comprehensive integration testing and example usage
//! of the complete tiered storage system including classification, movement,
//! and object storage integration.

use anyhow::Result;
use std::sync::Arc;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, debug, warn};
use bytes::Bytes;
use rand;

use crate::tiered_storage::{
    TieredStorageManager, TierConfig, StorageTier, AccessFrequency, 
    ClassificationPolicies, ObjectStorageConfig, CacheConfig, 
    CredentialsConfig, LifecycleConfig
};
use crate::tier_movement::{
    DataMovementEngine, MovementEngineConfig, MovementReason
};
use crate::object_storage::{ObjectStorageBackend, StorageProvider};
use mooseng_common::types::ChunkId;

/// Integration test suite for tiered storage
pub struct TieredStorageIntegrationTest {
    storage_manager: Arc<TieredStorageManager>,
    movement_engine: Arc<DataMovementEngine>,
    object_backends: Vec<Arc<ObjectStorageBackend>>,
}

impl TieredStorageIntegrationTest {
    /// Create a new integration test environment
    pub async fn new() -> Result<Self> {
        // Create storage manager
        let storage_manager = Arc::new(TieredStorageManager::new());
        
        // Configure tiers
        Self::configure_test_tiers(&storage_manager).await?;
        
        // Create movement engine
        let movement_engine = Arc::new(DataMovementEngine::new(storage_manager.clone()));
        
        // Configure object storage backends
        let object_backends = Self::create_test_backends().await?;
        
        Ok(Self {
            storage_manager,
            movement_engine,
            object_backends,
        })
    }
    
    /// Configure test storage tiers
    async fn configure_test_tiers(manager: &TieredStorageManager) -> Result<()> {
        // Hot tier (SSD simulation)
        let hot_config = TierConfig::hot_tier(
            PathBuf::from("/tmp/mooseng-test/hot"),
            1024 * 1024 * 1024, // 1GB
        );
        manager.add_tier_config(hot_config).await?;
        
        // Warm tier (HDD simulation)  
        let warm_config = TierConfig::warm_tier(
            PathBuf::from("/tmp/mooseng-test/warm"),
            10 * 1024 * 1024 * 1024, // 10GB
        );
        manager.add_tier_config(warm_config).await?;
        
        // Cold tier (Object storage simulation)
        let cold_config = TierConfig::cold_tier(
            "mooseng-test-cold".to_string(),
            "us-west-2".to_string(),
            100 * 1024 * 1024 * 1024, // 100GB
        );
        manager.add_tier_config(cold_config).await?;
        
        info!("Configured test storage tiers");
        Ok(())
    }
    
    /// Create test object storage backends
    async fn create_test_backends() -> Result<Vec<Arc<ObjectStorageBackend>>> {
        let mut backends = Vec::new();
        
        // Memory backend for testing
        let memory_config = ObjectStorageConfig {
            provider: "memory".to_string(),
            bucket: "test-bucket".to_string(),
            region: "memory".to_string(),
            credentials: CredentialsConfig {
                access_key_id: "test".to_string(),
                secret_access_key: "test".to_string(),
                session_token: None,
                role_arn: None,
            },
            lifecycle: LifecycleConfig {
                transition_days: 30,
                delete_days: 365,
                versioning: false,
            },
            cache: CacheConfig {
                enabled: true,
                max_size: 10 * 1024 * 1024, // 10MB cache
                ttl_seconds: 3600,
                cache_dir: PathBuf::from("/tmp/mooseng-test/cache"),
            },
        };
        
        let memory_backend = Arc::new(
            ObjectStorageBackend::new(StorageProvider::Memory, memory_config).await?
        );
        backends.push(memory_backend);
        
        // Local filesystem backend for testing
        let local_config = ObjectStorageConfig {
            provider: "local".to_string(),
            bucket: "/tmp/mooseng-test/object-storage".to_string(),
            region: "local".to_string(),
            credentials: CredentialsConfig {
                access_key_id: "".to_string(),
                secret_access_key: "".to_string(),
                session_token: None,
                role_arn: None,
            },
            lifecycle: LifecycleConfig {
                transition_days: 7,
                delete_days: 0,
                versioning: false,
            },
            cache: CacheConfig {
                enabled: false,
                max_size: 0,
                ttl_seconds: 0,
                cache_dir: PathBuf::new(),
            },
        };
        
        let local_backend = Arc::new(
            ObjectStorageBackend::new(StorageProvider::Local, local_config).await?
        );
        backends.push(local_backend);
        
        info!("Created {} test object storage backends", backends.len());
        Ok(backends)
    }
    
    /// Run comprehensive integration tests
    pub async fn run_all_tests(&self) -> Result<()> {
        info!("Starting tiered storage integration tests");
        
        // Test 1: Data classification
        self.test_data_classification().await?;
        
        // Test 2: Tier movement
        self.test_tier_movement().await?;
        
        // Test 3: Object storage operations
        self.test_object_storage().await?;
        
        // Test 4: End-to-end workflow
        self.test_end_to_end_workflow().await?;
        
        // Test 5: Performance under load
        self.test_performance_load().await?;
        
        info!("All integration tests completed successfully");
        Ok(())
    }
    
    /// Test data classification functionality
    pub async fn test_data_classification(&self) -> Result<()> {
        info!("Testing data classification");
        
        // Simulate different access patterns
        let test_cases = vec![
            // Hot data: frequently accessed, high importance
            (1, AccessFrequency {
                last_24h: 100,
                last_7d: 500,
                last_30d: 1500,
                last_365d: 5000,
                avg_access_interval: 864.0, // ~14 minutes
            }, 0.9),
            
            // Warm data: occasionally accessed
            (2, AccessFrequency {
                last_24h: 5,
                last_7d: 20,
                last_30d: 80,
                last_365d: 300,
                avg_access_interval: 86400.0, // 1 day
            }, 0.6),
            
            // Cold data: rarely accessed
            (3, AccessFrequency {
                last_24h: 0,
                last_7d: 1,
                last_30d: 3,
                last_365d: 10,
                avg_access_interval: 604800.0, // 1 week
            }, 0.3),
            
            // Archive data: very old, never accessed
            (4, AccessFrequency {
                last_24h: 0,
                last_7d: 0,
                last_30d: 0,
                last_365d: 1,
                avg_access_interval: 31536000.0, // 1 year
            }, 0.1),
        ];
        
        for (chunk_id, access_patterns, importance) in test_cases {
            let tier = self.storage_manager.classify_chunk(
                chunk_id, &access_patterns, importance
            ).await?;
            
            info!("Chunk {} classified to tier {:?}", chunk_id, tier);
            
            // Record access for tracking
            self.storage_manager.record_chunk_access(chunk_id).await?;
        }
        
        Ok(())
    }
    
    /// Test tier movement functionality
    async fn test_tier_movement(&self) -> Result<()> {
        info!("Testing tier movement");
        
        // Start movement engine
        self.movement_engine.clone().start().await?;
        
        // Schedule movements for different reasons
        let movements = vec![
            (101, StorageTier::Cold, MovementReason::CostOptimization),
            (102, StorageTier::Hot, MovementReason::PerformanceOptimization),
            (103, StorageTier::Archive, MovementReason::AutomaticAge),
            (104, StorageTier::Warm, MovementReason::Manual),
        ];
        
        for (chunk_id, target_tier, reason) in movements {
            self.movement_engine.schedule_movement(
                chunk_id, target_tier, reason
            ).await?;
            info!("Scheduled movement for chunk {} to {:?}", chunk_id, target_tier);
        }
        
        // Wait for movements to process
        sleep(Duration::from_secs(2)).await;
        
        // Check movement statistics
        let stats = self.movement_engine.get_stats().await;
        info!("Movement stats: attempted={}, succeeded={}", 
              stats.total_attempted, stats.total_succeeded);
        
        Ok(())
    }
    
    /// Test object storage operations
    pub async fn test_object_storage(&self) -> Result<()> {
        info!("Testing object storage operations");
        
        for (i, backend) in self.object_backends.iter().enumerate() {
            info!("Testing backend {}: {:?}", i, backend);
            
            // Test upload
            let chunk_data = Bytes::from(format!("test_chunk_data_{}", i).into_bytes());
            let chunk_id = 1000 + i as ChunkId;
            
            let metadata = backend.upload_chunk(chunk_id, chunk_data.clone()).await?;
            info!("Uploaded chunk {} to object storage", chunk_id);
            
            // Test download
            let downloaded = backend.download_chunk(&metadata).await?;
            assert_eq!(downloaded, chunk_data);
            info!("Downloaded and verified chunk {}", chunk_id);
            
            // Test list objects
            let objects = backend.list_objects(Some("chunks")).await?;
            info!("Listed {} objects", objects.len());
            
            // Test metrics
            let metrics = backend.get_metrics().await;
            info!("Backend metrics: uploads={}, downloads={}", 
                  metrics.uploads_success, metrics.downloads_success);
            
            // Test health check
            backend.health_check().await?;
            info!("Health check passed for backend {}", i);
            
            // Test delete
            backend.delete_chunk(&metadata).await?;
            info!("Deleted chunk {} from object storage", chunk_id);
        }
        
        Ok(())
    }
    
    /// Test end-to-end workflow
    pub async fn test_end_to_end_workflow(&self) -> Result<()> {
        info!("Testing end-to-end workflow");
        
        // Simulate a typical data lifecycle
        let chunk_id = 2000;
        
        // 1. Create chunk with initial high access
        let initial_access = AccessFrequency {
            last_24h: 50,
            last_7d: 200,
            last_30d: 600,
            last_365d: 2000,
            avg_access_interval: 1800.0,
        };
        
        let initial_tier = self.storage_manager.classify_chunk(
            chunk_id, &initial_access, 0.8
        ).await?;
        info!("Initial classification: {:?}", initial_tier);
        
        // 2. Simulate aging - access patterns decrease
        for day in 1..=30 {
            // Simulate decreasing access over time
            let access_rate = (50.0 * (-day as f64 / 10.0).exp()) as u64;
            
            for _ in 0..access_rate {
                self.storage_manager.record_chunk_access(chunk_id).await?;
            }
            
            // Check if tier should change
            if let Some(new_tier) = self.storage_manager.should_move_chunk(chunk_id).await? {
                info!("Day {}: Should move chunk {} to {:?}", day, chunk_id, new_tier);
                
                // Schedule movement
                self.movement_engine.schedule_movement(
                    chunk_id, new_tier, MovementReason::AutomaticAccess
                ).await?;
            }
            
            // Simulate daily interval
            sleep(Duration::from_millis(10)).await;
        }
        
        // 3. Check final tier and performance metrics
        let final_tier = self.storage_manager.get_optimal_tier(chunk_id).await?;
        let performance_metrics = self.storage_manager.get_performance_metrics().await;
        
        info!("Final tier: {:?}", final_tier);
        info!("Performance metrics: {:?}", performance_metrics);
        
        Ok(())
    }
    
    /// Test performance under load
    async fn test_performance_load(&self) -> Result<()> {
        info!("Testing performance under load");
        
        let num_chunks = 100;
        let mut tasks = Vec::new();
        
        // Create concurrent operations
        for chunk_id in 3000..3000 + num_chunks {
            let storage_manager = self.storage_manager.clone();
            let movement_engine = self.movement_engine.clone();
            
            let task = tokio::spawn(async move {
                // Simulate random access pattern
                let access_count = rand::random::<u64>() % 100;
                for _ in 0..access_count {
                    let _ = storage_manager.record_chunk_access(chunk_id).await;
                }
                
                // Classify chunk
                let optimal_tier = storage_manager.get_optimal_tier(chunk_id).await?;
                
                // Schedule movement if needed
                if rand::random::<f64>() < 0.3 { // 30% chance of movement
                    let _ = movement_engine.schedule_movement(
                        chunk_id,
                        optimal_tier,
                        MovementReason::PerformanceOptimization,
                    ).await;
                }
                
                Ok::<(), anyhow::Error>(())
            });
            
            tasks.push(task);
        }
        
        // Wait for all tasks to complete
        let start_time = std::time::Instant::now();
        for task in tasks {
            task.await??;
        }
        let duration = start_time.elapsed();
        
        info!("Processed {} chunks in {:?} ({:.2} chunks/sec)", 
              num_chunks, duration, num_chunks as f64 / duration.as_secs_f64());
        
        // Check system performance metrics
        let movement_stats = self.movement_engine.get_stats().await;
        let storage_metrics = self.storage_manager.get_performance_metrics().await;
        
        info!("Load test results:");
        info!("  Movement operations: {}", movement_stats.total_attempted);
        info!("  Storage operations: {}", storage_metrics.chunks_per_tier.values().sum::<u64>());
        
        Ok(())
    }
    
    /// Run specific benchmark scenarios
    pub async fn run_benchmarks(&self) -> Result<()> {
        info!("Running tiered storage benchmarks");
        
        // Benchmark 1: Classification performance
        let start = std::time::Instant::now();
        for i in 0..1000 {
            let access_patterns = AccessFrequency {
                last_24h: i % 100,
                last_7d: i % 500,
                last_30d: i % 1500,
                last_365d: i % 5000,
                avg_access_interval: (i as f64) * 10.0,
            };
            
            let _ = self.storage_manager.classify_chunk(i, &access_patterns, 0.5).await?;
        }
        let classification_time = start.elapsed();
        info!("Classification benchmark: 1000 operations in {:?} ({:.2} ops/sec)",
              classification_time, 1000.0 / classification_time.as_secs_f64());
        
        // Benchmark 2: Object storage throughput
        if let Some(backend) = self.object_backends.first() {
            let start = std::time::Instant::now();
            let mut total_bytes = 0u64;
            
            for i in 0..100 {
                let data_size = 1024 * (1 + i % 1024); // 1KB to 1MB
                let chunk_data = Bytes::from(vec![i as u8; data_size]);
                total_bytes += data_size as u64;
                
                let metadata = backend.upload_chunk(10000 + i as ChunkId, chunk_data.clone()).await?;
                let _downloaded = backend.download_chunk(&metadata).await?;
                backend.delete_chunk(&metadata).await?;
            }
            
            let object_time = start.elapsed();
            let throughput = total_bytes as f64 / object_time.as_secs_f64() / 1024.0 / 1024.0;
            info!("Object storage benchmark: {:.2} MB/s throughput", throughput);
        }
        
        Ok(())
    }
}

/// Example usage and demonstration
pub async fn demonstrate_tiered_storage() -> Result<()> {
    info!("Demonstrating tiered storage capabilities");
    
    let test_env = TieredStorageIntegrationTest::new().await?;
    
    // Run demonstration scenarios
    info!("=== Data Classification Demo ===");
    test_env.test_data_classification().await?;
    
    info!("=== Object Storage Demo ===");
    test_env.test_object_storage().await?;
    
    info!("=== End-to-End Workflow Demo ===");
    test_env.test_end_to_end_workflow().await?;
    
    info!("=== Performance Benchmarks ===");
    test_env.run_benchmarks().await?;
    
    info!("Tiered storage demonstration completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_integration_environment() {
        let test_env = TieredStorageIntegrationTest::new().await.unwrap();
        
        // Verify components are properly initialized
        assert!(!test_env.object_backends.is_empty());
        
        // Test basic functionality
        test_env.test_data_classification().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_full_integration() {
        let test_env = TieredStorageIntegrationTest::new().await.unwrap();
        test_env.run_all_tests().await.unwrap();
    }
}