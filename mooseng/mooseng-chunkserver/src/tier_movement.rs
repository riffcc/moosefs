use anyhow::Result;
use mooseng_common::types::{ChunkId, now_micros};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore, mpsc};
use tokio::time::{interval, sleep};
use tracing::{debug, info, warn, error, instrument};
use chrono::{Datelike, Timelike};

use crate::tiered_storage::{
    StorageTier, TieredStorageManager, ChunkTierMetadata
};
use crate::erasure::{ErasureConfig, ErasureCoder};

// Re-export MovementReason from tiered_storage
pub use crate::tiered_storage::MovementReason;

/// Configuration for the data movement engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementEngineConfig {
    /// Maximum concurrent movements
    pub max_concurrent_movements: usize,
    /// Movement check interval in seconds
    pub check_interval_secs: u64,
    /// Maximum movement rate (chunks per second)
    pub max_movement_rate: f64,
    /// Movement priority weights
    pub priority_weights: PriorityWeights,
    /// Throttling configuration
    pub throttling: ThrottlingConfig,
    /// Movement scheduling configuration
    pub scheduling: SchedulingConfig,
}

/// Priority weights for different movement reasons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityWeights {
    /// Weight for manual movements (highest priority)
    pub manual: f64,
    /// Weight for capacity-driven movements
    pub capacity_constrained: f64,
    /// Weight for performance optimization
    pub performance_optimization: f64,
    /// Weight for cost optimization
    pub cost_optimization: f64,
    /// Weight for automatic access-based movements
    pub automatic_access: f64,
    /// Weight for automatic age-based movements
    pub automatic_age: f64,
}

/// Throttling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottlingConfig {
    /// Enable adaptive throttling based on system load
    pub adaptive: bool,
    /// Maximum network bandwidth usage (bytes per second)
    pub max_bandwidth_bps: u64,
    /// Maximum disk I/O operations per second
    pub max_disk_iops: u32,
    /// CPU usage threshold for throttling (0.0-1.0)
    pub cpu_threshold: f64,
    /// Memory usage threshold for throttling (0.0-1.0)
    pub memory_threshold: f64,
}

/// Scheduling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingConfig {
    /// Preferred time windows for movements (UTC hours)
    pub preferred_hours: Vec<u8>,
    /// Blackout periods where no movements should occur
    pub blackout_periods: Vec<BlackoutPeriod>,
    /// Enable movement during business hours
    pub allow_business_hours: bool,
    /// Batch size for movement operations
    pub batch_size: usize,
}

/// Blackout period configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackoutPeriod {
    /// Start hour (UTC)
    pub start_hour: u8,
    /// End hour (UTC)
    pub end_hour: u8,
    /// Days of week (0 = Sunday, 6 = Saturday)
    pub days_of_week: Vec<u8>,
}

/// Movement task with priority and metadata
#[derive(Debug, Clone)]
pub struct MovementTask {
    /// Chunk to move
    pub chunk_id: ChunkId,
    /// Source tier
    pub from_tier: StorageTier,
    /// Destination tier
    pub to_tier: StorageTier,
    /// Reason for movement
    pub reason: MovementReason,
    /// Task priority (higher = more urgent)
    pub priority: f64,
    /// Task creation timestamp
    pub created_at: u64,
    /// Estimated data size to move
    pub estimated_size: u64,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Maximum retries allowed
    pub max_retries: u32,
}

/// Movement execution result
#[derive(Debug, Clone)]
pub struct MovementResult {
    /// The movement task
    pub task: MovementTask,
    /// Whether the movement succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Actual duration of movement
    pub duration_ms: u64,
    /// Actual bytes moved
    pub bytes_moved: u64,
    /// Completion timestamp
    pub completed_at: u64,
}

/// Movement statistics
#[derive(Debug, Clone, Default)]
pub struct MovementStats {
    /// Total movements attempted
    pub total_attempted: u64,
    /// Total movements succeeded
    pub total_succeeded: u64,
    /// Total movements failed
    pub total_failed: u64,
    /// Total bytes moved
    pub total_bytes_moved: u64,
    /// Average movement duration (ms)
    pub avg_duration_ms: f64,
    /// Movement rate (movements per hour)
    pub movements_per_hour: f64,
    /// Movements by reason
    pub movements_by_reason: HashMap<MovementReason, u64>,
    /// Movements by tier transition
    pub movements_by_transition: HashMap<(StorageTier, StorageTier), u64>,
}

/// Data movement engine for tiered storage
pub struct DataMovementEngine {
    /// Reference to tiered storage manager
    storage_manager: Arc<TieredStorageManager>,
    /// Movement configuration
    config: RwLock<MovementEngineConfig>,
    /// Pending movement tasks (priority queue)
    pending_tasks: RwLock<VecDeque<MovementTask>>,
    /// Currently executing tasks
    executing_tasks: RwLock<HashMap<ChunkId, MovementTask>>,
    /// Semaphore for controlling concurrent movements
    movement_semaphore: Semaphore,
    /// Movement statistics
    stats: RwLock<MovementStats>,
    /// Results channel for completed movements
    results_tx: mpsc::UnboundedSender<MovementResult>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Default for MovementEngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_movements: 4,
            check_interval_secs: 300, // 5 minutes
            max_movement_rate: 10.0,  // 10 chunks per second
            priority_weights: PriorityWeights::default(),
            throttling: ThrottlingConfig::default(),
            scheduling: SchedulingConfig::default(),
        }
    }
}

impl Default for PriorityWeights {
    fn default() -> Self {
        Self {
            manual: 100.0,
            capacity_constrained: 80.0,
            performance_optimization: 60.0,
            cost_optimization: 40.0,
            automatic_access: 30.0,
            automatic_age: 20.0,
        }
    }
}

impl Default for ThrottlingConfig {
    fn default() -> Self {
        Self {
            adaptive: true,
            max_bandwidth_bps: 100 * 1024 * 1024, // 100 MB/s
            max_disk_iops: 1000,
            cpu_threshold: 0.8,
            memory_threshold: 0.9,
        }
    }
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            preferred_hours: vec![2, 3, 4, 5], // Early morning hours
            blackout_periods: vec![],
            allow_business_hours: true,
            batch_size: 10,
        }
    }
}

impl MovementTask {
    /// Create a new movement task
    pub fn new(
        chunk_id: ChunkId,
        from_tier: StorageTier,
        to_tier: StorageTier,
        reason: MovementReason,
        estimated_size: u64,
    ) -> Self {
        Self {
            chunk_id,
            from_tier,
            to_tier,
            reason,
            priority: 0.0,
            created_at: now_micros() / 1_000_000,
            estimated_size,
            retry_count: 0,
            max_retries: 3,
        }
    }
    
    /// Calculate task priority based on reason and age
    pub fn calculate_priority(&mut self, weights: &PriorityWeights) {
        let base_priority = match self.reason {
            MovementReason::Manual => weights.manual,
            MovementReason::CapacityConstrained => weights.capacity_constrained,
            MovementReason::PerformanceOptimization => weights.performance_optimization,
            MovementReason::CostOptimization => weights.cost_optimization,
            MovementReason::AutomaticAccess => weights.automatic_access,
            MovementReason::AutomaticAge => weights.automatic_age,
        };
        
        // Add age-based priority boost (older tasks get higher priority)
        let now = now_micros() / 1_000_000;
        let age_hours = (now - self.created_at) as f64 / 3600.0;
        let age_boost = (age_hours * 0.1).min(10.0); // Max 10 points for age
        
        self.priority = base_priority + age_boost;
    }
    
    /// Check if task should be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }
    
    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

impl DataMovementEngine {
    /// Create a new data movement engine
    pub fn new(storage_manager: Arc<TieredStorageManager>) -> Self {
        let config = MovementEngineConfig::default();
        let (results_tx, _) = mpsc::unbounded_channel();
        
        Self {
            storage_manager,
            config: RwLock::new(config.clone()),
            pending_tasks: RwLock::new(VecDeque::new()),
            executing_tasks: RwLock::new(HashMap::new()),
            movement_semaphore: Semaphore::new(config.max_concurrent_movements),
            stats: RwLock::new(MovementStats::default()),
            results_tx,
            shutdown_tx: None,
        }
    }
    
    /// Start the movement engine background tasks
    pub async fn start(self: Arc<Self>) -> Result<()> {
        // Start task scheduler
        let scheduler_engine = self.clone();
        tokio::spawn(async move {
            scheduler_engine.run_scheduler().await;
        });
        
        // Start movement executor
        let executor_engine = self.clone();
        tokio::spawn(async move {
            executor_engine.run_executor().await;
        });
        
        // Start statistics updater
        let stats_engine = self.clone();
        tokio::spawn(async move {
            stats_engine.run_stats_updater().await;
        });
        
        info!("Data movement engine started");
        Ok(())
    }
    
    /// Schedule a chunk for movement
    #[instrument(skip(self))]
    pub async fn schedule_movement(
        &self,
        chunk_id: ChunkId,
        to_tier: StorageTier,
        reason: MovementReason,
    ) -> Result<()> {
        debug!("Scheduling movement for chunk {} to tier {:?}", chunk_id, to_tier);
        
        // Get current chunk metadata
        let chunk_metadata = self.get_chunk_metadata(chunk_id).await?;
        let from_tier = chunk_metadata.current_tier;
        
        // Don't schedule if already in target tier
        if from_tier == to_tier {
            debug!("Chunk {} already in target tier {:?}", chunk_id, to_tier);
            return Ok(());
        }
        
        // Create movement task
        let mut task = MovementTask::new(
            chunk_id,
            from_tier,
            to_tier,
            reason,
            chunk_metadata.size,
        );
        
        // Calculate priority
        let config = self.config.read().await;
        task.calculate_priority(&config.priority_weights);
        drop(config);
        
        // Add to pending queue
        let mut pending = self.pending_tasks.write().await;
        
        // Check if already scheduled
        if pending.iter().any(|t| t.chunk_id == chunk_id) {
            warn!("Chunk {} already scheduled for movement", chunk_id);
            return Ok(());
        }
        
        // Insert in priority order (higher priority first)
        let insert_pos = pending
            .iter()
            .position(|t| t.priority < task.priority)
            .unwrap_or(pending.len());
        
        let priority = task.priority;
        pending.insert(insert_pos, task);
        info!("Scheduled movement for chunk {} with priority {:.1}", chunk_id, priority);
        
        Ok(())
    }
    
    /// Cancel a scheduled movement
    pub async fn cancel_movement(&self, chunk_id: ChunkId) -> Result<bool> {
        let mut pending = self.pending_tasks.write().await;
        
        if let Some(pos) = pending.iter().position(|t| t.chunk_id == chunk_id) {
            pending.remove(pos);
            info!("Cancelled scheduled movement for chunk {}", chunk_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// Get movement statistics
    pub async fn get_stats(&self) -> MovementStats {
        self.stats.read().await.clone()
    }
    
    /// Update configuration
    pub async fn update_config(&self, new_config: MovementEngineConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated movement engine configuration");
        Ok(())
    }
    
    /// Background scheduler task
    async fn run_scheduler(&self) {
        let mut interval_timer = interval(Duration::from_secs(
            self.config.read().await.check_interval_secs
        ));
        
        loop {
            interval_timer.tick().await;
            
            if let Err(e) = self.discover_movement_candidates().await {
                error!("Error discovering movement candidates: {}", e);
            }
        }
    }
    
    /// Background executor task
    async fn run_executor(&self) {
        loop {
            // Wait for available slot
            let _permit = self.movement_semaphore.acquire().await.unwrap();
            
            // Get next task
            let task = {
                let mut pending = self.pending_tasks.write().await;
                pending.pop_front()
            };
            
            if let Some(task) = task {
                // Check if movement is allowed right now
                if !self.is_movement_allowed().await {
                    // Put task back and wait
                    let mut pending = self.pending_tasks.write().await;
                    pending.push_front(task);
                    drop(pending);
                    sleep(Duration::from_secs(60)).await;
                    continue;
                }
                
                // Execute movement
                let chunk_id = task.chunk_id;
                {
                    let mut executing = self.executing_tasks.write().await;
                    executing.insert(chunk_id, task.clone());
                }
                
                let result = self.execute_movement(task).await;
                
                {
                    let mut executing = self.executing_tasks.write().await;
                    executing.remove(&chunk_id);
                }
                
                // Send result
                if let Err(e) = self.results_tx.send(result) {
                    error!("Failed to send movement result: {}", e);
                }
            } else {
                // No tasks available, wait a bit
                sleep(Duration::from_secs(10)).await;
            }
        }
    }
    
    /// Background statistics updater
    async fn run_stats_updater(&self) {
        let mut interval_timer = interval(Duration::from_secs(60));
        
        loop {
            interval_timer.tick().await;
            self.update_stats().await;
        }
    }
    
    /// Discover chunks that should be moved
    async fn discover_movement_candidates(&self) -> Result<()> {
        debug!("Discovering movement candidates");
        
        // This would integrate with the storage manager to find chunks
        // that should be moved based on access patterns, age, etc.
        // For now, this is a placeholder
        
        Ok(())
    }
    
    /// Check if movement is currently allowed based on scheduling
    async fn is_movement_allowed(&self) -> bool {
        let config = self.config.read().await;
        
        // Check blackout periods
        let now = chrono::Utc::now();
        let current_hour = now.hour() as u8;
        let current_weekday = now.weekday().num_days_from_sunday() as u8;
        
        for blackout in &config.scheduling.blackout_periods {
            if blackout.days_of_week.contains(&current_weekday) {
                if blackout.start_hour <= blackout.end_hour {
                    // Same day
                    if current_hour >= blackout.start_hour && current_hour < blackout.end_hour {
                        return false;
                    }
                } else {
                    // Spans midnight
                    if current_hour >= blackout.start_hour || current_hour < blackout.end_hour {
                        return false;
                    }
                }
            }
        }
        
        // Check preferred hours
        if !config.scheduling.preferred_hours.is_empty() 
            && !config.scheduling.preferred_hours.contains(&current_hour) {
            return false;
        }
        
        // Check system load if adaptive throttling is enabled
        if config.throttling.adaptive {
            // This would check actual system metrics
            // For now, always allow
        }
        
        true
    }
    
    /// Execute a movement task
    async fn execute_movement(&self, mut task: MovementTask) -> MovementResult {
        let start_time = std::time::Instant::now();
        info!("Executing movement: chunk {} from {:?} to {:?}", 
              task.chunk_id, task.from_tier, task.to_tier);
        
        let mut success = true;
        let mut error_msg = None;
        let mut bytes_moved = 0u64;
        
        // Check if erasure coding re-encoding is needed
        match self.storage_manager.needs_reencoding(task.chunk_id, task.from_tier, task.to_tier).await {
            Ok(needs_reencoding) => {
                if needs_reencoding {
                    info!("Chunk {} requires re-encoding for tier movement", task.chunk_id);
                    
                    match self.perform_reencoding(&task).await {
                        Ok(moved_bytes) => {
                            bytes_moved = moved_bytes;
                            info!("Successfully re-encoded chunk {} ({} bytes)", task.chunk_id, bytes_moved);
                        }
                        Err(e) => {
                            success = false;
                            error_msg = Some(format!("Re-encoding failed: {}", e));
                            error!("Failed to re-encode chunk {}: {}", task.chunk_id, e);
                        }
                    }
                } else {
                    // Simple movement without re-encoding
                    match self.perform_simple_movement(&task).await {
                        Ok(moved_bytes) => {
                            bytes_moved = moved_bytes;
                            info!("Successfully moved chunk {} ({} bytes)", task.chunk_id, bytes_moved);
                        }
                        Err(e) => {
                            success = false;
                            error_msg = Some(format!("Movement failed: {}", e));
                            error!("Failed to move chunk {}: {}", task.chunk_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                success = false;
                error_msg = Some(format!("Failed to check re-encoding requirements: {}", e));
                error!("Failed to check re-encoding for chunk {}: {}", task.chunk_id, e);
            }
        }
        
        let duration = start_time.elapsed();
        
        // Record movement in chunk metadata
        if success {
            if let Err(e) = self.record_successful_movement(&task).await {
                error!("Failed to record movement: {}", e);
            }
        }
        
        MovementResult {
            task,
            success,
            error: error_msg,
            duration_ms: duration.as_millis() as u64,
            bytes_moved,
            completed_at: now_micros() / 1_000_000,
        }
    }
    
    /// Record successful movement in chunk metadata
    async fn record_successful_movement(&self, task: &MovementTask) -> Result<()> {
        // This would update the chunk metadata in the storage manager
        // For now, this is a placeholder
        Ok(())
    }
    
    /// Update movement statistics
    async fn update_stats(&self) {
        // This would calculate and update various statistics
        // For now, this is a placeholder
    }
    
    /// Perform chunk re-encoding for tier movement
    async fn perform_reencoding(&self, task: &MovementTask) -> Result<u64> {
        debug!("Performing re-encoding for chunk {} from {:?} to {:?}", 
               task.chunk_id, task.from_tier, task.to_tier);
        
        // Get source and destination tier configurations
        let from_config = self.storage_manager.get_tier_erasure_config(task.from_tier).await?;
        let to_config = self.storage_manager.get_tier_erasure_config(task.to_tier).await?;
        
        match (from_config, to_config) {
            (Some(from_ec), Some(to_ec)) => {
                // Re-encode from one erasure scheme to another
                self.reencode_chunk(task.chunk_id, &from_ec, &to_ec).await
            }
            (None, Some(to_ec)) => {
                // Encode to erasure-coded tier
                self.encode_chunk(task.chunk_id, &to_ec).await
            }
            (Some(_), None) => {
                // Decode from erasure-coded tier to replicated
                self.decode_chunk(task.chunk_id).await
            }
            (None, None) => {
                // Should not happen since we checked needs_reencoding
                Err(anyhow::anyhow!("No re-encoding needed but perform_reencoding called"))
            }
        }
    }
    
    /// Perform simple movement without re-encoding
    async fn perform_simple_movement(&self, task: &MovementTask) -> Result<u64> {
        debug!("Performing simple movement for chunk {} from {:?} to {:?}", 
               task.chunk_id, task.from_tier, task.to_tier);
        
        // This would involve moving data between storage locations
        // For now, simulate with the estimated size
        sleep(Duration::from_millis(50)).await;
        Ok(task.estimated_size)
    }
    
    /// Re-encode chunk from one erasure scheme to another
    async fn reencode_chunk(&self, chunk_id: ChunkId, from_config: &ErasureConfig, to_config: &ErasureConfig) -> Result<u64> {
        info!("Re-encoding chunk {} from {:?} to {:?}", chunk_id, from_config, to_config);
        
        // Create decoders/encoders
        let from_coder = ErasureCoder::new(from_config.clone())?;
        let to_coder = ErasureCoder::new(to_config.clone())?;
        
        // This would:
        // 1. Reconstruct original data from source erasure shards
        // 2. Re-encode with new erasure configuration
        // 3. Store new shards in destination tier
        
        // For now, simulate the operation
        let estimated_size = 1024 * 1024; // 1MB
        sleep(Duration::from_millis(200)).await; // Simulate encoding work
        
        info!("Successfully re-encoded chunk {} ({} bytes)", chunk_id, estimated_size);
        Ok(estimated_size)
    }
    
    /// Encode chunk to erasure-coded format
    async fn encode_chunk(&self, chunk_id: ChunkId, config: &ErasureConfig) -> Result<u64> {
        info!("Encoding chunk {} with erasure configuration {:?}", chunk_id, config);
        
        let coder = ErasureCoder::new(config.clone())?;
        
        // This would:
        // 1. Read original chunk data
        // 2. Encode to shards
        // 3. Store shards in destination tier
        
        // For now, simulate the operation
        let estimated_size = 1024 * 1024; // 1MB
        sleep(Duration::from_millis(150)).await; // Simulate encoding work
        
        info!("Successfully encoded chunk {} ({} bytes)", chunk_id, estimated_size);
        Ok(estimated_size)
    }
    
    /// Decode chunk from erasure-coded format
    async fn decode_chunk(&self, chunk_id: ChunkId) -> Result<u64> {
        info!("Decoding chunk {} from erasure-coded format", chunk_id);
        
        // This would:
        // 1. Read erasure shards
        // 2. Reconstruct original data
        // 3. Store in destination tier with replication
        
        // For now, simulate the operation
        let estimated_size = 1024 * 1024; // 1MB
        sleep(Duration::from_millis(100)).await; // Simulate decoding work
        
        info!("Successfully decoded chunk {} ({} bytes)", chunk_id, estimated_size);
        Ok(estimated_size)
    }
    
    /// Get chunk metadata (placeholder)
    async fn get_chunk_metadata(&self, chunk_id: ChunkId) -> Result<ChunkTierMetadata> {
        // This would get metadata from the storage manager
        // For now, return a default
        Ok(ChunkTierMetadata {
            chunk_id,
            current_tier: StorageTier::Hot,
            original_tier: StorageTier::Hot,
            last_accessed: now_micros() / 1_000_000,
            created_at: now_micros() / 1_000_000,
            last_moved: None,
            access_count: 0,
            size: 1024 * 1024, // 1MB default
            storage_class_id: 1,
            cost_metadata: crate::tiered_storage::CostMetadata {
                total_cost: 0.0,
                monthly_cost: 0.0,
                access_cost: 0.0,
                transfer_cost: 0.0,
            },
            movement_history: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiered_storage::TieredStorageManager;
    
    #[tokio::test]
    async fn test_movement_task_priority() {
        let mut task = MovementTask::new(
            1,
            StorageTier::Hot,
            StorageTier::Cold,
            MovementReason::Manual,
            1024,
        );
        
        let weights = PriorityWeights::default();
        task.calculate_priority(&weights);
        
        assert!(task.priority >= weights.manual);
    }
    
    #[tokio::test]
    async fn test_movement_scheduling() {
        let storage_manager = Arc::new(TieredStorageManager::new());
        let engine = DataMovementEngine::new(storage_manager);
        
        // Schedule a movement
        let result = engine.schedule_movement(
            123,
            StorageTier::Cold,
            MovementReason::CostOptimization,
        ).await;
        
        assert!(result.is_ok());
        
        // Check that task was added to pending queue
        let pending = engine.pending_tasks.read().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].chunk_id, 123);
    }
    
    #[tokio::test]
    async fn test_movement_cancellation() {
        let storage_manager = Arc::new(TieredStorageManager::new());
        let engine = DataMovementEngine::new(storage_manager);
        
        // Schedule and then cancel a movement
        engine.schedule_movement(
            456,
            StorageTier::Archive,
            MovementReason::AutomaticAge,
        ).await.unwrap();
        
        let cancelled = engine.cancel_movement(456).await.unwrap();
        assert!(cancelled);
        
        let pending = engine.pending_tasks.read().await;
        assert_eq!(pending.len(), 0);
    }
    
    #[tokio::test]
    async fn test_config_update() {
        let storage_manager = Arc::new(TieredStorageManager::new());
        let engine = DataMovementEngine::new(storage_manager);
        
        let mut new_config = MovementEngineConfig::default();
        new_config.max_concurrent_movements = 8;
        
        let result = engine.update_config(new_config).await;
        assert!(result.is_ok());
        
        let config = engine.config.read().await;
        assert_eq!(config.max_concurrent_movements, 8);
    }
}