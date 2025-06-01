//! Background scrubbing and repair system for data integrity
//! 
//! This module provides background processes for data scrubbing, integrity verification,
//! and automatic repair of chunks to maintain data quality and prevent silent corruption.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore, Notify};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn, error, instrument};
use crate::{
    chunk::{Chunk, ChunkMetadata, ChecksumType},
    storage::{ChunkStorage, StorageManager},
    error::{ChunkServerError, Result},
    config::ChunkServerConfig,
};
use mooseng_common::types::{ChunkId, ChunkVersion};

/// Scrubbing priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScrubPriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Scrubbing operation result
#[derive(Debug, Clone)]
pub struct ScrubResult {
    pub chunk_id: ChunkId,
    pub version: ChunkVersion,
    pub success: bool,
    pub corruption_detected: bool,
    pub repair_attempted: bool,
    pub repair_successful: bool,
    pub duration: Duration,
    pub error_message: Option<String>,
}

/// Scrubbing statistics
#[derive(Debug, Clone, Default)]
pub struct ScrubStats {
    pub total_chunks_scrubbed: u64,
    pub corruptions_detected: u64,
    pub repairs_attempted: u64,
    pub repairs_successful: u64,
    pub scrub_errors: u64,
    pub last_full_scan: Option<SystemTime>,
    pub average_scrub_time: Duration,
    pub chunks_per_hour: f64,
}

/// Chunk scrubbing scheduler and manager
pub struct ChunkScrubber {
    storage_manager: Arc<StorageManager>,
    config: Arc<ChunkServerConfig>,
    
    // Scrubbing state
    scrub_queue: Arc<RwLock<VecDeque<(ChunkId, ChunkVersion, ScrubPriority)>>>,
    active_scrubs: Arc<RwLock<HashMap<ChunkId, Instant>>>,
    scrub_semaphore: Arc<Semaphore>,
    shutdown_notify: Arc<Notify>,
    
    // Statistics and tracking
    stats: Arc<RwLock<ScrubStats>>,
    last_scrub_times: Arc<RwLock<HashMap<(ChunkId, ChunkVersion), Instant>>>,
    
    // Performance tracking
    scrub_times: Arc<RwLock<VecDeque<Duration>>>,
    max_scrub_history: usize,
}

impl ChunkScrubber {
    pub fn new(
        storage_manager: Arc<StorageManager>,
        config: Arc<ChunkServerConfig>,
    ) -> Self {
        let max_concurrent_scrubs = config.max_concurrent_scrubs.unwrap_or(2);
        
        Self {
            storage_manager,
            config,
            scrub_queue: Arc::new(RwLock::new(VecDeque::new())),
            active_scrubs: Arc::new(RwLock::new(HashMap::new())),
            scrub_semaphore: Arc::new(Semaphore::new(max_concurrent_scrubs)),
            shutdown_notify: Arc::new(Notify::new()),
            stats: Arc::new(RwLock::new(ScrubStats::default())),
            last_scrub_times: Arc::new(RwLock::new(HashMap::new())),
            scrub_times: Arc::new(RwLock::new(VecDeque::new())),
            max_scrub_history: 1000,
        }
    }
    
    /// Start the background scrubbing service
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting chunk scrubber service");
        
        // Start the main scrubbing loop
        let scrubber = Arc::new(self.clone());
        let scrub_loop = {
            let scrubber = scrubber.clone();
            tokio::spawn(async move {
                scrubber.scrub_loop().await;
            })
        };
        
        // Start the scheduling loop
        let schedule_loop = {
            let scrubber = scrubber.clone();
            tokio::spawn(async move {
                scrubber.schedule_loop().await;
            })
        };
        
        // Start periodic full scans
        let full_scan_loop = {
            let scrubber = scrubber.clone();
            tokio::spawn(async move {
                scrubber.full_scan_loop().await;
            })
        };
        
        info!("Chunk scrubber service started successfully");
        Ok(())
    }
    
    /// Stop the scrubbing service
    pub async fn stop(&self) {
        info!("Stopping chunk scrubber service");
        self.shutdown_notify.notify_waiters();
    }
    
    /// Add a chunk to the scrub queue with specified priority
    pub async fn schedule_scrub(&self, chunk_id: ChunkId, version: ChunkVersion, priority: ScrubPriority) {
        let mut queue = self.scrub_queue.write().await;
        
        // Check if already in queue
        if queue.iter().any(|(id, ver, _)| *id == chunk_id && *ver == version) {
            return;
        }
        
        // Insert based on priority (higher priority first)
        let position = queue.iter().position(|(_, _, p)| *p < priority).unwrap_or(queue.len());
        queue.insert(position, (chunk_id, version, priority));
        
        debug!("Scheduled chunk {}:{} for scrubbing with priority {:?}", chunk_id, version, priority);
    }
    
    /// Force immediate scrub of a specific chunk
    pub async fn scrub_chunk_immediate(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<ScrubResult> {
        info!("Performing immediate scrub of chunk {}:{}", chunk_id, version);
        self.scrub_single_chunk(chunk_id, version).await
    }
    
    /// Get current scrubbing statistics
    pub async fn get_stats(&self) -> ScrubStats {
        self.stats.read().await.clone()
    }
    
    /// Get the current scrub queue length
    pub async fn get_queue_length(&self) -> usize {
        self.scrub_queue.read().await.len()
    }
    
    /// Main scrubbing loop
    #[instrument(skip(self))]
    async fn scrub_loop(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    info!("Scrub loop shutting down");
                    break;
                }
                _ = self.process_scrub_queue() => {
                    // Continue processing
                }
            }
            
            // Small delay to prevent busy waiting
            sleep(Duration::from_millis(100)).await;
        }
    }
    
    /// Process items from the scrub queue
    async fn process_scrub_queue(&self) {
        let (chunk_id, version, priority) = {
            let mut queue = self.scrub_queue.write().await;
            match queue.pop_front() {
                Some(item) => item,
                None => {
                    drop(queue);
                    sleep(Duration::from_secs(1)).await;
                    return;
                }
            }
        };
        
        // Acquire semaphore to limit concurrent scrubs
        let _permit = match self.scrub_semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                // Put item back in queue and wait
                {
                    let mut queue = self.scrub_queue.write().await;
                    queue.push_front((chunk_id, version, priority));
                }
                sleep(Duration::from_millis(500)).await;
                return;
            }
        };
        
        // Check if already being scrubbed
        {
            let active = self.active_scrubs.read().await;
            if active.contains_key(&chunk_id) {
                debug!("Chunk {} already being scrubbed, skipping", chunk_id);
                return;
            }
        }
        
        // Mark as active
        {
            let mut active = self.active_scrubs.write().await;
            active.insert(chunk_id, Instant::now());
        }
        
        // Perform the scrub
        let result = self.scrub_single_chunk(chunk_id, version).await;
        
        // Remove from active scrubs
        {
            let mut active = self.active_scrubs.write().await;
            active.remove(&chunk_id);
        }
        
        // Update statistics
        self.update_stats(&result).await;
        
        match result {
            Ok(scrub_result) => {
                if scrub_result.corruption_detected {
                    warn!("Corruption detected in chunk {}:{}, repair attempted: {}, successful: {}", 
                          chunk_id, version, scrub_result.repair_attempted, scrub_result.repair_successful);
                } else {
                    debug!("Chunk {}:{} scrubbed successfully in {:?}", 
                           chunk_id, version, scrub_result.duration);
                }
            }
            Err(e) => {
                error!("Failed to scrub chunk {}:{}: {}", chunk_id, version, e);
            }
        }
    }
    
    /// Scrub a single chunk
    #[instrument(skip(self))]
    async fn scrub_single_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<ScrubResult> {
        let start_time = Instant::now();
        let mut result = ScrubResult {
            chunk_id,
            version,
            success: false,
            corruption_detected: false,
            repair_attempted: false,
            repair_successful: false,
            duration: Duration::default(),
            error_message: None,
        };
        
        // Perform the scrub operation with timeout
        let scrub_timeout = Duration::from_secs(30);
        let scrub_future = self.perform_scrub_operation(chunk_id, version);
        
        match timeout(scrub_timeout, scrub_future).await {
            Ok(Ok((corruption_detected, repair_result))) => {
                result.success = true;
                result.corruption_detected = corruption_detected;
                if corruption_detected {
                    result.repair_attempted = true;
                    result.repair_successful = repair_result.unwrap_or(false);
                }
            }
            Ok(Err(e)) => {
                result.error_message = Some(e.to_string());
            }
            Err(_) => {
                result.error_message = Some("Scrub operation timed out".to_string());
            }
        }
        
        result.duration = start_time.elapsed();
        
        // Update last scrub time
        {
            let mut last_scrubs = self.last_scrub_times.write().await;
            last_scrubs.insert((chunk_id, version), Instant::now());
        }
        
        Ok(result)
    }
    
    /// Perform the actual scrub operation
    async fn perform_scrub_operation(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<(bool, Option<bool>)> {
        // Verify chunk integrity
        let verification_result = self.storage_manager.primary().verify_chunk(chunk_id, version).await;
        
        match verification_result {
            Ok(true) => {
                // Chunk is valid
                Ok((false, None))
            }
            Ok(false) => {
                // Corruption detected, attempt repair
                warn!("Corruption detected in chunk {}:{}, attempting repair", chunk_id, version);
                let repair_success = self.attempt_chunk_repair(chunk_id, version).await?;
                Ok((true, Some(repair_success)))
            }
            Err(e) => {
                // Error during verification
                Err(e)
            }
        }
    }
    
    /// Attempt to repair a corrupted chunk
    #[instrument(skip(self))]
    async fn attempt_chunk_repair(&self, chunk_id: ChunkId, version: ChunkVersion) -> Result<bool> {
        // Implementation depends on the repair strategy:
        // 1. Try to reconstruct from erasure coding if available
        // 2. Request chunk from other chunk servers
        // 3. Mark chunk as lost if repair is not possible
        
        // For now, implement basic repair by requesting from peers
        // This would require master server communication to get peer locations
        
        info!("Attempting repair of chunk {}:{}", chunk_id, version);
        
        // TODO: Implement actual repair logic based on:
        // - Erasure coding reconstruction
        // - Peer chunk server requests
        // - Master server coordination
        
        // For now, return false (repair not implemented)
        warn!("Chunk repair not yet implemented for {}:{}", chunk_id, version);
        Ok(false)
    }
    
    /// Update scrubbing statistics
    async fn update_stats(&self, result: &Result<ScrubResult>) {
        let mut stats = self.stats.write().await;
        
        match result {
            Ok(scrub_result) => {
                stats.total_chunks_scrubbed += 1;
                if scrub_result.corruption_detected {
                    stats.corruptions_detected += 1;
                }
                if scrub_result.repair_attempted {
                    stats.repairs_attempted += 1;
                }
                if scrub_result.repair_successful {
                    stats.repairs_successful += 1;
                }
                
                // Update performance metrics
                let mut times = self.scrub_times.write().await;
                times.push_back(scrub_result.duration);
                if times.len() > self.max_scrub_history {
                    times.pop_front();
                }
                
                // Calculate average scrub time
                if !times.is_empty() {
                    let total_time: Duration = times.iter().sum();
                    stats.average_scrub_time = total_time / times.len() as u32;
                    
                    // Calculate chunks per hour
                    if stats.average_scrub_time.as_secs() > 0 {
                        stats.chunks_per_hour = 3600.0 / stats.average_scrub_time.as_secs_f64();
                    }
                }
            }
            Err(_) => {
                stats.scrub_errors += 1;
            }
        }
    }
    
    /// Scheduling loop for periodic scrubs
    #[instrument(skip(self))]
    async fn schedule_loop(&self) {
        let mut last_schedule = Instant::now();
        
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    info!("Schedule loop shutting down");
                    break;
                }
                _ = sleep(Duration::from_secs(60)) => {
                    // Run scheduling every minute
                }
            }
            
            let now = Instant::now();
            if now.duration_since(last_schedule) >= Duration::from_secs(60) {
                self.schedule_periodic_scrubs().await;
                last_schedule = now;
            }
        }
    }
    
    /// Schedule periodic scrubs based on age and access patterns
    async fn schedule_periodic_scrubs(&self) {
        // Get list of all chunks
        let chunks = match self.storage_manager.primary().list_chunks().await {
            Ok(chunks) => chunks,
            Err(e) => {
                warn!("Failed to list chunks for scheduling: {}", e);
                return;
            }
        };
        
        let now = Instant::now();
        let last_scrubs = self.last_scrub_times.read().await;
        
        for (chunk_id, version) in chunks {
            let last_scrub = last_scrubs.get(&(chunk_id, version));
            let time_since_scrub = last_scrub.map(|t| now.duration_since(*t));
            
            let should_schedule = match time_since_scrub {
                None => true, // Never scrubbed
                Some(duration) => {
                    // Schedule based on age: daily scrub for chunks older than 1 day
                    duration >= Duration::from_secs(24 * 60 * 60)
                }
            };
            
            if should_schedule {
                let priority = if last_scrub.is_none() {
                    ScrubPriority::Medium // Never scrubbed
                } else {
                    ScrubPriority::Low // Regular periodic scrub
                };
                
                self.schedule_scrub(chunk_id, version, priority).await;
            }
        }
        
        debug!("Scheduled periodic scrubs, queue length: {}", self.get_queue_length().await);
    }
    
    /// Full scan loop for comprehensive integrity checking
    #[instrument(skip(self))]
    async fn full_scan_loop(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    info!("Full scan loop shutting down");
                    break;
                }
                _ = sleep(Duration::from_secs(24 * 60 * 60)) => {
                    // Run full scan daily
                }
            }
            
            info!("Starting full integrity scan");
            self.perform_full_scan().await;
        }
    }
    
    /// Perform a full integrity scan of all chunks
    async fn perform_full_scan(&self) {
        let start_time = Instant::now();
        
        // Get all chunks
        let chunks = match self.storage_manager.primary().list_chunks().await {
            Ok(chunks) => chunks,
            Err(e) => {
                error!("Failed to list chunks for full scan: {}", e);
                return;
            }
        };
        
        info!("Full scan starting: {} chunks to check", chunks.len());
        
        // Schedule all chunks for high-priority scrubbing
        for (chunk_id, version) in chunks {
            self.schedule_scrub(chunk_id, version, ScrubPriority::High).await;
        }
        
        // Update last full scan time
        {
            let mut stats = self.stats.write().await;
            stats.last_full_scan = Some(SystemTime::now());
        }
        
        let duration = start_time.elapsed();
        info!("Full scan scheduled in {:?}", duration);
    }
}

// Clone implementation for use in tokio::spawn
impl Clone for ChunkScrubber {
    fn clone(&self) -> Self {
        Self {
            storage_manager: self.storage_manager.clone(),
            config: self.config.clone(),
            scrub_queue: self.scrub_queue.clone(),
            active_scrubs: self.active_scrubs.clone(),
            scrub_semaphore: self.scrub_semaphore.clone(),
            shutdown_notify: self.shutdown_notify.clone(),
            stats: self.stats.clone(),
            last_scrub_times: self.last_scrub_times.clone(),
            scrub_times: self.scrub_times.clone(),
            max_scrub_history: self.max_scrub_history,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalChunkStorage;
    use tempfile::TempDir;
    
    async fn create_test_scrubber() -> (ChunkScrubber, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Arc::new(ChunkServerConfig::default());
        let storage = Arc::new(LocalChunkStorage::new(config.clone()).unwrap());
        let storage_manager = Arc::new(StorageManager::new(storage, 1024 * 1024 * 1024)); // 1GB
        
        let scrubber = ChunkScrubber::new(storage_manager, config);
        (scrubber, temp_dir)
    }
    
    #[tokio::test]
    async fn test_scrub_scheduling() {
        let (scrubber, _temp) = create_test_scrubber().await;
        
        // Schedule some chunks
        scrubber.schedule_scrub(1, 1, ScrubPriority::High).await;
        scrubber.schedule_scrub(2, 1, ScrubPriority::Low).await;
        scrubber.schedule_scrub(3, 1, ScrubPriority::Medium).await;
        
        assert_eq!(scrubber.get_queue_length().await, 3);
        
        // Check priority ordering
        let queue = scrubber.scrub_queue.read().await;
        assert_eq!(queue[0].2, ScrubPriority::High);
        assert_eq!(queue[1].2, ScrubPriority::Medium);
        assert_eq!(queue[2].2, ScrubPriority::Low);
    }
    
    #[tokio::test]
    async fn test_stats_tracking() {
        let (scrubber, _temp) = create_test_scrubber().await;
        
        let initial_stats = scrubber.get_stats().await;
        assert_eq!(initial_stats.total_chunks_scrubbed, 0);
        
        // Simulate a scrub result
        let result = Ok(ScrubResult {
            chunk_id: 1,
            version: 1,
            success: true,
            corruption_detected: false,
            repair_attempted: false,
            repair_successful: false,
            duration: Duration::from_millis(100),
            error_message: None,
        });
        
        scrubber.update_stats(&result).await;
        
        let updated_stats = scrubber.get_stats().await;
        assert_eq!(updated_stats.total_chunks_scrubbed, 1);
    }
}