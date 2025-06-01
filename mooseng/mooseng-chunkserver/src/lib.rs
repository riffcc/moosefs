pub mod chunk;
pub mod storage;
pub mod cache;
pub mod server;
pub mod config;
pub mod error;
pub mod mmap;
pub mod erasure;
pub mod erasure_storage;
pub mod placement;
pub mod zero_copy;
pub mod health_checker;
pub mod metrics;
pub mod grpc_server;
pub mod compression_service;
pub mod tiered_storage;
pub mod tier_movement;
pub mod block_allocator;
pub mod scrubber;
pub mod object_storage;
pub mod tiered_storage_integration;
pub mod tiered_storage_demo;
pub mod integrity;
pub mod benchmarks;

pub use error::{ChunkServerError, Result};
pub use chunk::{Chunk, ChunkMetrics, ChunkMetadata, ChecksumType, ChunkChecksum};
pub use storage::{ChunkStorage, StorageManager};
pub use cache::ChunkCache;
pub use server::ChunkServer;
pub use config::ChunkServerConfig;
pub use health_checker::ChunkServerHealthChecker;
pub use compression_service::{ChunkCompressionService, CompressionPolicy, CompressionServiceStats};
pub use tiered_storage::{
    StorageTier, TierConfig, TieredStorageManager, ChunkTierMetadata, 
    DataClassification, AccessFrequency, ObjectStorageConfig
};
pub use object_storage::{
    ObjectStorageBackend, StorageProvider, ObjectCache, ChunkObjectMetadata,
    ObjectStorageMetrics
};
pub use tiered_storage_integration::{
    TieredStorageIntegrationTest, demonstrate_tiered_storage
};
pub use tiered_storage_demo::{TieredStorageDemo, run_quick_demo};
pub use tier_movement::{
    DataMovementEngine, MovementEngineConfig, MovementTask, MovementResult,
    MovementStats, MovementReason
};
pub use block_allocator::{
    AllocationStrategy, FirstFitAllocator, BestFitAllocator, DynamicBlockAllocator,
    AllocatedBlock, FreeSpaceInfo, WorkloadHint, BLOCK_SIZE
};
pub use scrubber::{
    ChunkScrubber, ScrubResult, ScrubStats, ScrubPriority
};