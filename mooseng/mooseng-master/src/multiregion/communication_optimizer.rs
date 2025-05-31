/// Inter-region communication optimizer for MooseNG
/// Implements compression, batching, prioritization, and connection pooling
/// for efficient cross-region data transfer

use anyhow::{Result, anyhow};
use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, broadcast, mpsc, Semaphore};
use tokio::time::{interval, timeout, sleep};
use tracing::{debug, info, warn, error, instrument};
use serde::{Serialize, Deserialize};
use flate2::{Compression, write::GzEncoder, read::GzDecoder};
use std::io::{Write, Read};

use crate::multiregion::{
    MultiregionConfig, RegionPeer, HLCTimestamp,
    replication::{ReplicationOperation, ReplicationAck, ReplicationStatus},
};

/// Configuration for communication optimization
#[derive(Debug, Clone)]
pub struct CommunicationConfig {
    /// Enable compression for inter-region traffic
    pub enable_compression: bool,
    
    /// Compression threshold (compress if data > this size)
    pub compression_threshold: usize,
    
    /// Compression level (1-9, higher = better compression, slower)
    pub compression_level: u32,
    
    /// Batching configuration
    pub batch_config: BatchConfig,
    
    /// Connection pooling configuration
    pub connection_pool_config: ConnectionPoolConfig,
    
    /// Priority queue configuration
    pub priority_config: PriorityConfig,
    
    /// Network optimization settings
    pub network_optimization: NetworkOptimization,
}

/// Batching configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum batch size (number of operations)
    pub max_batch_size: usize,
    
    /// Maximum batch size in bytes
    pub max_batch_bytes: usize,
    
    /// Maximum time to wait before sending a partial batch
    pub max_batch_wait: Duration,
    
    /// Minimum operations to trigger a batch
    pub min_batch_size: usize,
    
    /// Enable adaptive batching based on network conditions
    pub adaptive_batching: bool,
}

/// Connection pooling configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum connections per region
    pub max_connections_per_region: usize,
    
    /// Connection idle timeout
    pub connection_idle_timeout: Duration,
    
    /// Connection keep-alive interval
    pub keep_alive_interval: Duration,
    
    /// Maximum connection attempts
    pub max_connection_attempts: u32,
    
    /// Connection retry backoff
    pub connection_retry_backoff: Duration,
}

/// Priority configuration
#[derive(Debug, Clone)]
pub struct PriorityConfig {
    /// Number of priority levels (0 = highest)
    pub priority_levels: u8,
    
    /// Default priority for operations
    pub default_priority: u8,
    
    /// Priority mapping for different operation types
    pub operation_priorities: HashMap<String, u8>,
    
    /// Enable dynamic priority adjustment
    pub dynamic_priority: bool,
}

/// Network optimization settings
#[derive(Debug, Clone)]
pub struct NetworkOptimization {
    /// TCP no-delay (disable Nagle's algorithm)
    pub tcp_nodelay: bool,
    
    /// TCP keep-alive settings
    pub tcp_keepalive: Option<Duration>,
    
    /// Buffer sizes
    pub send_buffer_size: Option<usize>,
    pub recv_buffer_size: Option<usize>,
    
    /// Connection timeout
    pub connection_timeout: Duration,
    
    /// Request timeout
    pub request_timeout: Duration,
    
    /// Enable connection multiplexing
    pub enable_multiplexing: bool,
    
    /// Maximum concurrent requests per connection
    pub max_concurrent_requests: usize,
}

impl Default for CommunicationConfig {
    fn default() -> Self {
        let mut operation_priorities = HashMap::new();
        operation_priorities.insert("metadata_update".to_string(), 0); // Highest priority
        operation_priorities.insert("heartbeat".to_string(), 1);
        operation_priorities.insert("data_replication".to_string(), 2);
        operation_priorities.insert("background_sync".to_string(), 3); // Lowest priority
        
        Self {
            enable_compression: true,
            compression_threshold: 1024, // 1KB
            compression_level: 6, // Balanced compression
            batch_config: BatchConfig {
                max_batch_size: 100,
                max_batch_bytes: 1024 * 1024, // 1MB
                max_batch_wait: Duration::from_millis(50),
                min_batch_size: 5,
                adaptive_batching: true,
            },
            connection_pool_config: ConnectionPoolConfig {
                max_connections_per_region: 10,
                connection_idle_timeout: Duration::from_secs(300), // 5 minutes
                keep_alive_interval: Duration::from_secs(30),
                max_connection_attempts: 3,
                connection_retry_backoff: Duration::from_millis(1000),
            },
            priority_config: PriorityConfig {
                priority_levels: 4,
                default_priority: 2,
                operation_priorities,
                dynamic_priority: true,
            },
            network_optimization: NetworkOptimization {
                tcp_nodelay: true,
                tcp_keepalive: Some(Duration::from_secs(60)),
                send_buffer_size: Some(64 * 1024), // 64KB
                recv_buffer_size: Some(64 * 1024), // 64KB
                connection_timeout: Duration::from_secs(10),
                request_timeout: Duration::from_secs(30),
                enable_multiplexing: true,
                max_concurrent_requests: 100,
            },
        }
    }
}

/// Inter-region communication optimizer
pub struct CommunicationOptimizer {
    config: CommunicationConfig,
    multiregion_config: MultiregionConfig,
    
    /// Connection pools for each region
    connection_pools: Arc<RwLock<HashMap<u32, ConnectionPool>>>,
    
    /// Batch managers for each region
    batch_managers: Arc<RwLock<HashMap<u32, BatchManager>>>,
    
    /// Priority queues for each region
    priority_queues: Arc<RwLock<HashMap<u32, PriorityQueue>>>,
    
    /// Compression engine
    compression_engine: Arc<CompressionEngine>,
    
    /// Network statistics collector
    network_stats: Arc<RwLock<NetworkStatistics>>,
    
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

/// Connection pool for a specific region
struct ConnectionPool {
    region_id: u32,
    peer: RegionPeer,
    connections: Vec<Connection>,
    available_connections: VecDeque<usize>,
    connection_semaphore: Arc<Semaphore>,
    stats: ConnectionPoolStats,
}

/// A single connection to a peer region
struct Connection {
    id: usize,
    endpoint: String,
    created_at: Instant,
    last_used: Instant,
    is_healthy: bool,
    active_requests: u32,
    total_requests: u64,
    total_bytes_sent: u64,
    total_bytes_received: u64,
}

/// Batch manager for efficient bulk operations
struct BatchManager {
    region_id: u32,
    current_batch: Vec<ReplicationOperation>,
    current_batch_size: usize,
    batch_start_time: Option<Instant>,
    pending_batches: VecDeque<Batch>,
    stats: BatchStats,
}

/// A completed batch ready for transmission
#[derive(Debug)]
struct Batch {
    operations: Vec<ReplicationOperation>,
    created_at: Instant,
    total_size: usize,
    compressed_data: Option<Vec<u8>>,
    priority: u8,
}

/// Priority queue for operations
struct PriorityQueue {
    region_id: u32,
    queues: Vec<VecDeque<PrioritizedOperation>>,
    stats: PriorityStats,
}

/// Operation with priority information
#[derive(Debug)]
struct PrioritizedOperation {
    operation: ReplicationOperation,
    priority: u8,
    enqueued_at: Instant,
    estimated_size: usize,
}

/// Compression engine
struct CompressionEngine {
    config: CommunicationConfig,
    compression_stats: RwLock<CompressionStats>,
}

/// Network statistics
#[derive(Debug, Default, Clone)]
pub struct NetworkStatistics {
    total_operations_sent: u64,
    total_bytes_sent: u64,
    total_bytes_received: u64,
    total_compression_saved: u64,
    average_latency: Duration,
    operations_by_region: HashMap<u32, RegionStats>,
}

#[derive(Debug, Default, Clone)]
struct RegionStats {
    operations_sent: u64,
    bytes_sent: u64,
    bytes_received: u64,
    average_latency: Duration,
    error_rate: f32,
    last_successful_operation: Option<Instant>,
}

#[derive(Debug, Default)]
struct ConnectionPoolStats {
    total_connections_created: u64,
    active_connections: u32,
    total_requests: u64,
    failed_connections: u64,
    average_connection_age: Duration,
}

#[derive(Debug, Default)]
struct BatchStats {
    total_batches_created: u64,
    total_operations_batched: u64,
    average_batch_size: f32,
    compression_efficiency: f32,
    average_batch_latency: Duration,
}

#[derive(Debug, Default)]
struct PriorityStats {
    operations_by_priority: Vec<u64>,
    average_queue_depth: f32,
    priority_reorderings: u64,
}

#[derive(Debug, Default)]
struct CompressionStats {
    total_operations_compressed: u64,
    total_bytes_before_compression: u64,
    total_bytes_after_compression: u64,
    average_compression_ratio: f32,
    average_compression_time: Duration,
}

impl CommunicationOptimizer {
    pub fn new(
        config: CommunicationConfig,
        multiregion_config: MultiregionConfig,
    ) -> Result<Self> {
        let (shutdown_tx, _) = broadcast::channel(1);
        
        Ok(Self {
            config: config.clone(),
            multiregion_config,
            connection_pools: Arc::new(RwLock::new(HashMap::new())),
            batch_managers: Arc::new(RwLock::new(HashMap::new())),
            priority_queues: Arc::new(RwLock::new(HashMap::new())),
            compression_engine: Arc::new(CompressionEngine::new(config)),
            network_stats: Arc::new(RwLock::new(NetworkStatistics::default())),
            shutdown_tx,
        })
    }
    
    /// Initialize the communication optimizer
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing communication optimizer");
        
        // Initialize connection pools for each peer region
        for peer in &self.multiregion_config.peer_regions {
            self.initialize_region_resources(peer).await?;
        }
        
        // Start background tasks
        self.start_batch_processor().await?;
        self.start_connection_manager().await?;
        self.start_stats_collector().await?;
        
        Ok(())
    }
    
    /// Initialize resources for a specific region
    async fn initialize_region_resources(&self, peer: &RegionPeer) -> Result<()> {
        let region_id = peer.region_id;
        
        // Initialize connection pool
        {
            let mut pools = self.connection_pools.write().await;
            let pool = ConnectionPool::new(region_id, peer.clone(), &self.config)?;
            pools.insert(region_id, pool);
        }
        
        // Initialize batch manager
        {
            let mut managers = self.batch_managers.write().await;
            let manager = BatchManager::new(region_id);
            managers.insert(region_id, manager);
        }
        
        // Initialize priority queue
        {
            let mut queues = self.priority_queues.write().await;
            let queue = PriorityQueue::new(region_id, self.config.priority_config.priority_levels);
            queues.insert(region_id, queue);
        }
        
        info!("Initialized resources for region {}", region_id);
        Ok(())
    }
    
    /// Send a replication operation with optimization
    #[instrument(skip(self, operation))]
    pub async fn send_operation(
        &self,
        target_region: u32,
        operation: ReplicationOperation,
    ) -> Result<()> {
        let start_time = Instant::now();
        
        // Determine priority for the operation
        let priority = self.determine_operation_priority(&operation)?;
        
        // Add to priority queue
        self.enqueue_operation(target_region, operation, priority).await?;
        
        // Update statistics
        self.update_send_stats(target_region, start_time.elapsed()).await;
        
        Ok(())
    }
    
    /// Determine the priority of an operation
    fn determine_operation_priority(&self, operation: &ReplicationOperation) -> Result<u8> {
        // Extract operation type from log entry
        let operation_type = match &operation.log_entry.command {
            crate::raft::log::LogCommand::SetMetadata { .. } => "metadata_update",
            crate::raft::log::LogCommand::DeleteMetadata { .. } => "metadata_update",
            _ => "data_replication",
        };
        
        let base_priority = self.config.priority_config
            .operation_priorities
            .get(operation_type)
            .copied()
            .unwrap_or(self.config.priority_config.default_priority);
        
        // Dynamic priority adjustment based on consistency level
        let adjusted_priority = match operation.consistency_level {
            crate::multiregion::ConsistencyLevel::Strong => {
                std::cmp::max(0, base_priority.saturating_sub(1)) // Higher priority
            }
            crate::multiregion::ConsistencyLevel::Eventual => {
                std::cmp::min(
                    self.config.priority_config.priority_levels - 1,
                    base_priority + 1
                ) // Lower priority
            }
            _ => base_priority,
        };
        
        Ok(adjusted_priority)
    }
    
    /// Add operation to priority queue
    async fn enqueue_operation(
        &self,
        target_region: u32,
        operation: ReplicationOperation,
        priority: u8,
    ) -> Result<()> {
        let mut queues = self.priority_queues.write().await;
        
        if let Some(queue) = queues.get_mut(&target_region) {
            let prioritized_op = PrioritizedOperation {
                estimated_size: self.estimate_operation_size(&operation),
                operation,
                priority,
                enqueued_at: Instant::now(),
            };
            
            queue.enqueue(prioritized_op).await?;
        } else {
            return Err(anyhow!("No priority queue found for region {}", target_region));
        }
        
        Ok(())
    }
    
    /// Estimate the size of an operation for batching decisions
    fn estimate_operation_size(&self, operation: &ReplicationOperation) -> usize {
        // Simple estimation based on serialized size
        // In practice, this could be more sophisticated
        bincode::serialized_size(operation).unwrap_or(512) as usize
    }
    
    /// Start batch processing background task
    async fn start_batch_processor(&self) -> Result<()> {
        let batch_managers = Arc::clone(&self.batch_managers);
        let priority_queues = Arc::clone(&self.priority_queues);
        let connection_pools = Arc::clone(&self.connection_pools);
        let compression_engine = Arc::clone(&self.compression_engine);
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut batch_interval = interval(config.batch_config.max_batch_wait / 4);
            batch_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = batch_interval.tick() => {
                        if let Err(e) = Self::process_batches(
                            &batch_managers,
                            &priority_queues,
                            &connection_pools,
                            &compression_engine,
                            &config,
                        ).await {
                            error!("Batch processing error: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Batch processor shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Process batches for all regions
    async fn process_batches(
        batch_managers: &Arc<RwLock<HashMap<u32, BatchManager>>>,
        priority_queues: &Arc<RwLock<HashMap<u32, PriorityQueue>>>,
        connection_pools: &Arc<RwLock<HashMap<u32, ConnectionPool>>>,
        compression_engine: &Arc<CompressionEngine>,
        config: &CommunicationConfig,
    ) -> Result<()> {
        let region_ids: Vec<u32> = {
            let managers = batch_managers.read().await;
            managers.keys().copied().collect()
        };
        
        for region_id in region_ids {
            // Fill batches from priority queues
            Self::fill_batch_from_queue(
                region_id,
                batch_managers,
                priority_queues,
                config,
            ).await?;
            
            // Send ready batches
            Self::send_ready_batches(
                region_id,
                batch_managers,
                connection_pools,
                compression_engine,
                config,
            ).await?;
        }
        
        Ok(())
    }
    
    /// Fill batch from priority queue
    async fn fill_batch_from_queue(
        region_id: u32,
        batch_managers: &Arc<RwLock<HashMap<u32, BatchManager>>>,
        priority_queues: &Arc<RwLock<HashMap<u32, PriorityQueue>>>,
        config: &CommunicationConfig,
    ) -> Result<()> {
        let mut managers = batch_managers.write().await;
        let mut queues = priority_queues.write().await;
        
        if let (Some(manager), Some(queue)) = (managers.get_mut(&region_id), queues.get_mut(&region_id)) {
            // Try to fill current batch
            while manager.current_batch.len() < config.batch_config.max_batch_size &&
                  manager.current_batch_size < config.batch_config.max_batch_bytes {
                
                if let Some(prioritized_op) = queue.dequeue().await? {
                    manager.current_batch.push(prioritized_op.operation);
                    manager.current_batch_size += prioritized_op.estimated_size;
                    
                    if manager.batch_start_time.is_none() {
                        manager.batch_start_time = Some(Instant::now());
                    }
                } else {
                    break; // No more operations available
                }
            }
            
            // Check if batch should be sent
            let should_send = manager.should_send_batch(config);
            
            if should_send && !manager.current_batch.is_empty() {
                let batch = manager.finalize_current_batch();
                manager.pending_batches.push_back(batch);
            }
        }
        
        Ok(())
    }
    
    /// Send ready batches
    async fn send_ready_batches(
        region_id: u32,
        batch_managers: &Arc<RwLock<HashMap<u32, BatchManager>>>,
        connection_pools: &Arc<RwLock<HashMap<u32, ConnectionPool>>>,
        compression_engine: &Arc<CompressionEngine>,
        _config: &CommunicationConfig,
    ) -> Result<()> {
        let batches_to_send = {
            let mut managers = batch_managers.write().await;
            if let Some(manager) = managers.get_mut(&region_id) {
                std::mem::take(&mut manager.pending_batches)
            } else {
                VecDeque::new()
            }
        };
        
        for mut batch in batches_to_send {
            // Compress batch if beneficial
            if let Err(e) = compression_engine.compress_batch(&mut batch).await {
                warn!("Failed to compress batch for region {}: {}", region_id, e);
            }
            
            // Send batch
            if let Err(e) = Self::transmit_batch(region_id, batch, connection_pools).await {
                error!("Failed to send batch to region {}: {}", region_id, e);
                // TODO: Implement retry logic
            }
        }
        
        Ok(())
    }
    
    /// Transmit a batch to the target region
    async fn transmit_batch(
        region_id: u32,
        batch: Batch,
        connection_pools: &Arc<RwLock<HashMap<u32, ConnectionPool>>>,
    ) -> Result<()> {
        let _pools = connection_pools.read().await;
        
        // TODO: Implement actual gRPC/HTTP transmission
        // This would involve:
        // 1. Getting a connection from the pool
        // 2. Serializing the batch
        // 3. Sending via the appropriate protocol
        // 4. Handling responses and errors
        // 5. Returning connection to pool
        
        debug!("Transmitted batch of {} operations to region {} (compressed: {})",
               batch.operations.len(),
               region_id,
               batch.compressed_data.is_some());
        
        Ok(())
    }
    
    /// Start connection manager background task
    async fn start_connection_manager(&self) -> Result<()> {
        let connection_pools = Arc::clone(&self.connection_pools);
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut maintenance_interval = interval(config.connection_pool_config.keep_alive_interval);
            maintenance_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = maintenance_interval.tick() => {
                        if let Err(e) = Self::maintain_connections(&connection_pools, &config).await {
                            error!("Connection maintenance error: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Connection manager shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Maintain connection pools
    async fn maintain_connections(
        connection_pools: &Arc<RwLock<HashMap<u32, ConnectionPool>>>,
        config: &CommunicationConfig,
    ) -> Result<()> {
        let mut pools = connection_pools.write().await;
        
        for pool in pools.values_mut() {
            pool.maintain_connections(config).await?;
        }
        
        Ok(())
    }
    
    /// Start statistics collection background task
    async fn start_stats_collector(&self) -> Result<()> {
        let network_stats = Arc::clone(&self.network_stats);
        let connection_pools = Arc::clone(&self.connection_pools);
        let batch_managers = Arc::clone(&self.batch_managers);
        let compression_engine = Arc::clone(&self.compression_engine);
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut stats_interval = interval(Duration::from_secs(60)); // Collect stats every minute
            stats_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = stats_interval.tick() => {
                        if let Err(e) = Self::collect_statistics(
                            &network_stats,
                            &connection_pools,
                            &batch_managers,
                            &compression_engine,
                        ).await {
                            error!("Statistics collection error: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Statistics collector shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Collect and update statistics
    async fn collect_statistics(
        network_stats: &Arc<RwLock<NetworkStatistics>>,
        connection_pools: &Arc<RwLock<HashMap<u32, ConnectionPool>>>,
        batch_managers: &Arc<RwLock<HashMap<u32, BatchManager>>>,
        compression_engine: &Arc<CompressionEngine>,
    ) -> Result<()> {
        let mut stats = network_stats.write().await;
        let pools = connection_pools.read().await;
        let managers = batch_managers.read().await;
        let compression_stats = compression_engine.compression_stats.read().await;
        
        // Update network statistics
        stats.total_compression_saved = 
            compression_stats.total_bytes_before_compression
            .saturating_sub(compression_stats.total_bytes_after_compression);
        
        // Update per-region statistics
        for (region_id, pool) in pools.iter() {
            let region_stats = stats.operations_by_region.entry(*region_id).or_default();
            
            // Aggregate connection pool stats
            let mut total_requests = 0;
            let mut total_bytes_sent = 0;
            let mut total_bytes_received = 0;
            
            for conn in &pool.connections {
                total_requests += conn.total_requests;
                total_bytes_sent += conn.total_bytes_sent;
                total_bytes_received += conn.total_bytes_received;
            }
            
            region_stats.operations_sent = total_requests;
            region_stats.bytes_sent = total_bytes_sent;
            region_stats.bytes_received = total_bytes_received;
        }
        
        debug!("Updated network statistics: {} total operations, {} bytes saved through compression",
               stats.total_operations_sent, stats.total_compression_saved);
        
        Ok(())
    }
    
    /// Update send statistics
    async fn update_send_stats(&self, region_id: u32, latency: Duration) {
        let mut stats = self.network_stats.write().await;
        stats.total_operations_sent += 1;
        
        let region_stats = stats.operations_by_region.entry(region_id).or_default();
        region_stats.operations_sent += 1;
        region_stats.last_successful_operation = Some(Instant::now());
        
        // Update running average of latency
        region_stats.average_latency = Duration::from_nanos(
            (region_stats.average_latency.as_nanos() as u64 + latency.as_nanos() as u64) / 2
        );
    }
    
    /// Get current network statistics
    pub async fn get_network_statistics(&self) -> NetworkStatistics {
        self.network_stats.read().await.clone()
    }
    
    /// Shutdown the communication optimizer
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down communication optimizer");
        let _ = self.shutdown_tx.send(());
        Ok(())
    }
}

impl ConnectionPool {
    fn new(region_id: u32, peer: RegionPeer, config: &CommunicationConfig) -> Result<Self> {
        let max_connections = config.connection_pool_config.max_connections_per_region;
        
        Ok(Self {
            region_id,
            peer,
            connections: Vec::with_capacity(max_connections),
            available_connections: VecDeque::new(),
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            stats: ConnectionPoolStats::default(),
        })
    }
    
    async fn maintain_connections(&mut self, config: &CommunicationConfig) -> Result<()> {
        let now = Instant::now();
        let idle_timeout = config.connection_pool_config.connection_idle_timeout;
        
        // Remove idle connections
        self.connections.retain(|conn| {
            now.duration_since(conn.last_used) < idle_timeout
        });
        
        // Update available connections list
        self.available_connections.clear();
        for (idx, conn) in self.connections.iter().enumerate() {
            if conn.is_healthy && conn.active_requests == 0 {
                self.available_connections.push_back(idx);
            }
        }
        
        debug!("Maintained connection pool for region {}: {} active connections",
               self.region_id, self.connections.len());
        
        Ok(())
    }
}

impl BatchManager {
    fn new(region_id: u32) -> Self {
        Self {
            region_id,
            current_batch: Vec::new(),
            current_batch_size: 0,
            batch_start_time: None,
            pending_batches: VecDeque::new(),
            stats: BatchStats::default(),
        }
    }
    
    fn should_send_batch(&self, config: &CommunicationConfig) -> bool {
        // Check size limits
        if self.current_batch.len() >= config.batch_config.max_batch_size ||
           self.current_batch_size >= config.batch_config.max_batch_bytes {
            return true;
        }
        
        // Check time limit
        if let Some(start_time) = self.batch_start_time {
            if start_time.elapsed() >= config.batch_config.max_batch_wait {
                return true;
            }
        }
        
        // Check minimum batch size with time consideration
        if self.current_batch.len() >= config.batch_config.min_batch_size {
            if let Some(start_time) = self.batch_start_time {
                if start_time.elapsed() >= config.batch_config.max_batch_wait / 2 {
                    return true;
                }
            }
        }
        
        false
    }
    
    fn finalize_current_batch(&mut self) -> Batch {
        let operations = std::mem::take(&mut self.current_batch);
        let size = self.current_batch_size;
        let created_at = self.batch_start_time.unwrap_or_else(Instant::now);
        
        self.current_batch_size = 0;
        self.batch_start_time = None;
        self.stats.total_batches_created += 1;
        self.stats.total_operations_batched += operations.len() as u64;
        
        Batch {
            operations,
            created_at,
            total_size: size,
            compressed_data: None,
            priority: 2, // Default priority for batches
        }
    }
}

impl PriorityQueue {
    fn new(region_id: u32, priority_levels: u8) -> Self {
        let mut queues = Vec::new();
        for _ in 0..priority_levels {
            queues.push(VecDeque::new());
        }
        
        Self {
            region_id,
            queues,
            stats: PriorityStats {
                operations_by_priority: vec![0; priority_levels as usize],
                average_queue_depth: 0.0,
                priority_reorderings: 0,
            },
        }
    }
    
    async fn enqueue(&mut self, operation: PrioritizedOperation) -> Result<()> {
        let priority = operation.priority as usize;
        
        if priority >= self.queues.len() {
            return Err(anyhow!("Invalid priority level: {}", priority));
        }
        
        self.queues[priority].push_back(operation);
        self.stats.operations_by_priority[priority] += 1;
        
        Ok(())
    }
    
    async fn dequeue(&mut self) -> Result<Option<PrioritizedOperation>> {
        // Start from highest priority (lowest index)
        for queue in &mut self.queues {
            if let Some(operation) = queue.pop_front() {
                return Ok(Some(operation));
            }
        }
        
        Ok(None)
    }
}

impl CompressionEngine {
    fn new(config: CommunicationConfig) -> Self {
        Self {
            config,
            compression_stats: RwLock::new(CompressionStats::default()),
        }
    }
    
    async fn compress_batch(&self, batch: &mut Batch) -> Result<()> {
        if !self.config.enable_compression || 
           batch.total_size < self.config.compression_threshold {
            return Ok(());
        }
        
        let start_time = Instant::now();
        
        // Serialize operations
        let serialized = bincode::serialize(&batch.operations)?;
        let original_size = serialized.len();
        
        // Compress data
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
        encoder.write_all(&serialized)?;
        let compressed = encoder.finish()?;
        
        let compressed_size = compressed.len();
        let compression_time = start_time.elapsed();
        
        // Only use compression if it provides significant savings
        if compressed_size < original_size * 90 / 100 { // At least 10% savings
            batch.compressed_data = Some(compressed);
            
            // Update statistics
            let mut stats = self.compression_stats.write().await;
            stats.total_operations_compressed += batch.operations.len() as u64;
            stats.total_bytes_before_compression += original_size as u64;
            stats.total_bytes_after_compression += compressed_size as u64;
            stats.average_compression_time = Duration::from_nanos(
                (stats.average_compression_time.as_nanos() as u64 + compression_time.as_nanos() as u64) / 2
            );
            
            debug!("Compressed batch: {} -> {} bytes ({:.1}% savings)",
                   original_size, compressed_size,
                   (1.0 - compressed_size as f32 / original_size as f32) * 100.0);
        }
        
        Ok(())
    }
}