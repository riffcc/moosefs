/// Comprehensive failure handling for multi-region operations
/// Handles node failures, network partitions, and region outages

use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, broadcast, mpsc};
use tokio::time::{interval, timeout, sleep};
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};

use crate::multiregion::{
    MultiregionConfig, RegionPeer, ConsistencyLevel,
    NetworkPartition, PartitionType, ConsistencyManager,
};

/// Types of failures that can occur in the multi-region system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureType {
    /// Single node failure within a region
    NodeFailure {
        region_id: u32,
        node_id: String,
        #[serde(skip, default = "Instant::now")]
        failure_time: Instant,
    },
    
    /// Network partition between regions
    NetworkPartition {
        affected_regions: Vec<u32>,
        partition_type: PartitionType,
        #[serde(skip, default = "Instant::now")]
        detected_at: Instant,
        estimated_duration: Option<Duration>,
    },
    
    /// Complete region outage
    RegionOutage {
        region_id: u32,
        #[serde(skip, default = "Instant::now")]
        outage_start: Instant,
        #[serde(skip)]
        estimated_recovery: Option<Instant>,
        severity: OutageSeverity,
    },
    
    /// Intermittent connectivity issues
    ConnectivityIssue {
        source_region: u32,
        target_region: u32,
        latency_spike: Duration,
        packet_loss_rate: f32,
    },
    
    /// Data corruption detected
    DataCorruption {
        region_id: u32,
        affected_keys: Vec<String>,
        corruption_type: CorruptionType,
    },
}

/// Severity levels for region outages
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OutageSeverity {
    /// Minor outage, some services still available
    Minor,
    /// Major outage, most services unavailable
    Major,
    /// Complete outage, all services unavailable
    Complete,
}

/// Types of data corruption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorruptionType {
    Checksum,
    Structural,
    Consistency,
}

/// Recovery strategies for different failure types
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Wait for automatic recovery
    WaitForRecovery(Duration),
    /// Failover to backup region
    FailoverToBackup,
    /// Split-brain resolution
    SplitBrainResolution,
    /// Data reconstruction from other regions
    DataReconstruction,
    /// Manual intervention required
    ManualIntervention,
}

/// State machine for handling failures
#[derive(Debug, Clone)]
pub enum FailureState {
    /// Normal operation, no failures detected
    Normal,
    /// Failure detected, assessment in progress
    AssessingFailure,
    /// Recovery in progress
    Recovering,
    /// Failed over to backup systems
    FailedOver,
    /// Manual intervention required
    RequiresIntervention,
}

/// Failure detector and handler
pub struct FailureHandler {
    config: MultiregionConfig,
    consistency_manager: Arc<ConsistencyManager>,
    
    /// Current failure state
    state: Arc<RwLock<FailureState>>,
    
    /// Active failures being tracked
    active_failures: Arc<RwLock<HashMap<String, FailureInstance>>>,
    
    /// Failure history for pattern analysis
    failure_history: Arc<RwLock<VecDeque<FailureInstance>>>,
    
    /// Recovery strategies for different scenarios
    recovery_strategies: Arc<RwLock<HashMap<String, RecoveryStrategy>>>,
    
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
    
    /// Failure notification channel
    failure_tx: mpsc::UnboundedSender<FailureEvent>,
    failure_rx: Arc<Mutex<mpsc::UnboundedReceiver<FailureEvent>>>,
}

/// A specific failure instance with tracking information
#[derive(Debug, Clone)]
pub struct FailureInstance {
    pub id: String,
    pub failure_type: FailureType,
    pub detected_at: Instant,
    pub last_updated: Instant,
    pub recovery_strategy: Option<RecoveryStrategy>,
    pub recovery_progress: RecoveryProgress,
    pub impact_assessment: ImpactAssessment,
}

/// Progress of recovery operations
#[derive(Debug, Clone)]
pub struct RecoveryProgress {
    pub started_at: Option<Instant>,
    pub estimated_completion: Option<Instant>,
    pub steps_completed: u32,
    pub total_steps: u32,
    pub current_step: String,
}

/// Assessment of failure impact
#[derive(Debug, Clone)]
pub struct ImpactAssessment {
    pub affected_regions: Vec<u32>,
    pub affected_operations: Vec<String>,
    pub estimated_data_loss: Option<u64>, // bytes
    pub rpo_violation: bool,
    pub rto_violation: bool,
}

/// Failure events for notification
#[derive(Debug, Clone)]
pub struct FailureEvent {
    pub event_type: FailureEventType,
    pub failure_id: String,
    pub timestamp: Instant,
    pub details: String,
}

#[derive(Debug, Clone)]
pub enum FailureEventType {
    FailureDetected,
    RecoveryStarted,
    RecoveryCompleted,
    RecoveryFailed,
    ManualInterventionRequired,
}

impl FailureHandler {
    pub fn new(
        config: MultiregionConfig,
        consistency_manager: Arc<ConsistencyManager>,
    ) -> Result<Self> {
        let (shutdown_tx, _) = broadcast::channel(1);
        let (failure_tx, failure_rx) = mpsc::unbounded_channel();
        
        Ok(Self {
            config,
            consistency_manager,
            state: Arc::new(RwLock::new(FailureState::Normal)),
            active_failures: Arc::new(RwLock::new(HashMap::new())),
            failure_history: Arc::new(RwLock::new(VecDeque::new())),
            recovery_strategies: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            failure_tx,
            failure_rx: Arc::new(Mutex::new(failure_rx)),
        })
    }
    
    /// Start the failure handler
    pub async fn start(&self) -> Result<()> {
        info!("Starting failure handler for multi-region system");
        
        // Start failure detection loop
        self.start_failure_detection().await?;
        
        // Start recovery manager
        self.start_recovery_manager().await?;
        
        // Start failure notification handler
        self.start_notification_handler().await?;
        
        Ok(())
    }
    
    /// Start failure detection background task
    async fn start_failure_detection(&self) -> Result<()> {
        let config = self.config.clone();
        let active_failures = Arc::clone(&self.active_failures);
        let failure_tx = self.failure_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut detection_interval = interval(Duration::from_secs(5));
            detection_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = detection_interval.tick() => {
                        if let Err(e) = Self::detect_failures(
                            &config,
                            &active_failures,
                            &failure_tx,
                        ).await {
                            error!("Failure detection error: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Failure detection shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Detect various types of failures
    async fn detect_failures(
        config: &MultiregionConfig,
        active_failures: &Arc<RwLock<HashMap<String, FailureInstance>>>,
        failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        // Detect network partitions
        Self::detect_network_partitions(config, active_failures, failure_tx).await?;
        
        // Detect region outages
        Self::detect_region_outages(config, active_failures, failure_tx).await?;
        
        // Detect connectivity issues
        Self::detect_connectivity_issues(config, active_failures, failure_tx).await?;
        
        // Detect data corruption
        Self::detect_data_corruption(config, active_failures, failure_tx).await?;
        
        Ok(())
    }
    
    /// Detect network partitions between regions
    async fn detect_network_partitions(
        config: &MultiregionConfig,
        active_failures: &Arc<RwLock<HashMap<String, FailureInstance>>>,
        failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        let mut detected_partitions = Vec::new();
        
        // Check connectivity to each peer region
        for peer in &config.peer_regions {
            let connectivity_result = Self::check_region_connectivity(peer).await;
            
            match connectivity_result {
                Ok(false) => {
                    // Potential partition detected
                    let partition_id = format!("partition_{}_to_{}", config.region_id, peer.region_id);
                    
                    let failures = active_failures.read().await;
                    if !failures.contains_key(&partition_id) {
                        drop(failures);
                        
                        let failure = FailureType::NetworkPartition {
                            affected_regions: vec![config.region_id, peer.region_id],
                            partition_type: PartitionType::Complete,
                            detected_at: Instant::now(),
                            estimated_duration: Some(Duration::from_secs(300)), // 5 minutes default
                        };
                        
                        detected_partitions.push((partition_id, failure));
                    }
                }
                Ok(true) => {
                    // Connectivity is good, remove any existing partition
                    let partition_id = format!("partition_{}_to_{}", config.region_id, peer.region_id);
                    let mut failures = active_failures.write().await;
                    if failures.remove(&partition_id).is_some() {
                        info!("Network partition to region {} resolved", peer.region_id);
                    }
                }
                Err(e) => {
                    warn!("Error checking connectivity to region {}: {}", peer.region_id, e);
                }
            }
        }
        
        // Register newly detected partitions
        for (partition_id, failure) in detected_partitions {
            Self::register_failure(
                partition_id.clone(),
                failure,
                active_failures,
                failure_tx,
            ).await?;
        }
        
        Ok(())
    }
    
    /// Check connectivity to a peer region
    async fn check_region_connectivity(peer: &RegionPeer) -> Result<bool> {
        // Try to connect to any endpoint in the peer region
        for endpoint in &peer.endpoints {
            let connect_result = timeout(
                Duration::from_secs(10),
                tokio::net::TcpStream::connect(endpoint),
            ).await;
            
            match connect_result {
                Ok(Ok(_)) => return Ok(true),
                Ok(Err(_)) => continue,
                Err(_) => continue, // Timeout
            }
        }
        
        Ok(false)
    }
    
    /// Detect region outages
    async fn detect_region_outages(
        config: &MultiregionConfig,
        active_failures: &Arc<RwLock<HashMap<String, FailureInstance>>>,
        failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        for peer in &config.peer_regions {
            let health_status = Self::check_region_health(peer).await?;
            let outage_id = format!("outage_{}", peer.region_id);
            
            match health_status {
                RegionHealthStatus::Healthy => {
                    // Remove any existing outage
                    let mut failures = active_failures.write().await;
                    if failures.remove(&outage_id).is_some() {
                        info!("Region {} outage resolved", peer.region_id);
                    }
                }
                RegionHealthStatus::Degraded => {
                    // Check if we already have this as a minor outage
                    let failures = active_failures.read().await;
                    if !failures.contains_key(&outage_id) {
                        drop(failures);
                        
                        let failure = FailureType::RegionOutage {
                            region_id: peer.region_id,
                            outage_start: Instant::now(),
                            estimated_recovery: Some(Instant::now() + Duration::from_mins(30)),
                            severity: OutageSeverity::Minor,
                        };
                        
                        Self::register_failure(
                            outage_id,
                            failure,
                            active_failures,
                            failure_tx,
                        ).await?;
                    }
                }
                RegionHealthStatus::Unavailable => {
                    let failure = FailureType::RegionOutage {
                        region_id: peer.region_id,
                        outage_start: Instant::now(),
                        estimated_recovery: None, // Unknown recovery time
                        severity: OutageSeverity::Complete,
                    };
                    
                    Self::register_failure(
                        outage_id,
                        failure,
                        active_failures,
                        failure_tx,
                    ).await?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Check the health status of a region
    async fn check_region_health(peer: &RegionPeer) -> Result<RegionHealthStatus> {
        let mut successful_endpoints = 0;
        let total_endpoints = peer.endpoints.len();
        
        for endpoint in &peer.endpoints {
            let health_check = timeout(
                Duration::from_secs(5),
                Self::perform_health_check(endpoint),
            ).await;
            
            if health_check.is_ok() && health_check.unwrap().is_ok() {
                successful_endpoints += 1;
            }
        }
        
        let health_ratio = successful_endpoints as f32 / total_endpoints as f32;
        
        if health_ratio >= 0.8 {
            Ok(RegionHealthStatus::Healthy)
        } else if health_ratio >= 0.3 {
            Ok(RegionHealthStatus::Degraded)
        } else {
            Ok(RegionHealthStatus::Unavailable)
        }
    }
    
    /// Perform a health check on a specific endpoint
    async fn perform_health_check(endpoint: &str) -> Result<()> {
        // TODO: Implement actual health check protocol
        // This could be a gRPC health check or HTTP ping
        let _stream = tokio::net::TcpStream::connect(endpoint).await?;
        Ok(())
    }
    
    /// Detect connectivity issues like latency spikes
    async fn detect_connectivity_issues(
        config: &MultiregionConfig,
        active_failures: &Arc<RwLock<HashMap<String, FailureInstance>>>,
        failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        for peer in &config.peer_regions {
            let latency = Self::measure_latency(peer).await?;
            let expected_latency = Duration::from_millis(peer.latency_ms as u64);
            
            if latency > expected_latency * 3 {
                let issue_id = format!("connectivity_{}_{}", config.region_id, peer.region_id);
                
                let failure = FailureType::ConnectivityIssue {
                    source_region: config.region_id,
                    target_region: peer.region_id,
                    latency_spike: latency,
                    packet_loss_rate: 0.0, // TODO: Measure actual packet loss
                };
                
                Self::register_failure(
                    issue_id,
                    failure,
                    active_failures,
                    failure_tx,
                ).await?;
            }
        }
        
        Ok(())
    }
    
    /// Measure latency to a peer region
    async fn measure_latency(peer: &RegionPeer) -> Result<Duration> {
        if let Some(endpoint) = peer.endpoints.first() {
            let start = Instant::now();
            let _stream = timeout(
                Duration::from_secs(10),
                tokio::net::TcpStream::connect(endpoint),
            ).await??;
            Ok(start.elapsed())
        } else {
            Err(anyhow!("No endpoints available for region {}", peer.region_id))
        }
    }
    
    /// Detect data corruption
    async fn detect_data_corruption(
        _config: &MultiregionConfig,
        _active_failures: &Arc<RwLock<HashMap<String, FailureInstance>>>,
        _failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        // TODO: Implement data corruption detection
        // This would involve checking checksums, consistency checks, etc.
        Ok(())
    }
    
    /// Register a new failure
    async fn register_failure(
        failure_id: String,
        failure_type: FailureType,
        active_failures: &Arc<RwLock<HashMap<String, FailureInstance>>>,
        failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        let now = Instant::now();
        
        let failure_instance = FailureInstance {
            id: failure_id.clone(),
            failure_type: failure_type.clone(),
            detected_at: now,
            last_updated: now,
            recovery_strategy: None,
            recovery_progress: RecoveryProgress {
                started_at: None,
                estimated_completion: None,
                steps_completed: 0,
                total_steps: 0,
                current_step: "Assessing failure".to_string(),
            },
            impact_assessment: Self::assess_impact(&failure_type),
        };
        
        {
            let mut failures = active_failures.write().await;
            failures.insert(failure_id.clone(), failure_instance);
        }
        
        // Notify about the new failure
        let event = FailureEvent {
            event_type: FailureEventType::FailureDetected,
            failure_id: failure_id.clone(),
            timestamp: now,
            details: format!("Detected failure: {:?}", failure_type),
        };
        
        if let Err(e) = failure_tx.send(event) {
            warn!("Failed to send failure notification: {}", e);
        }
        
        error!("Registered new failure: {} - {:?}", failure_id, failure_type);
        
        Ok(())
    }
    
    /// Assess the impact of a failure
    fn assess_impact(failure_type: &FailureType) -> ImpactAssessment {
        match failure_type {
            FailureType::NodeFailure { region_id, .. } => {
                ImpactAssessment {
                    affected_regions: vec![*region_id],
                    affected_operations: vec!["metadata_operations".to_string()],
                    estimated_data_loss: Some(0),
                    rpo_violation: false,
                    rto_violation: false,
                }
            }
            FailureType::NetworkPartition { affected_regions, .. } => {
                ImpactAssessment {
                    affected_regions: affected_regions.clone(),
                    affected_operations: vec!["cross_region_replication".to_string()],
                    estimated_data_loss: None,
                    rpo_violation: true,
                    rto_violation: false,
                }
            }
            FailureType::RegionOutage { region_id, severity, .. } => {
                let (data_loss, rpo_violation, rto_violation) = match severity {
                    OutageSeverity::Minor => (Some(0), false, false),
                    OutageSeverity::Major => (Some(1024 * 1024), true, false), // 1MB potential loss
                    OutageSeverity::Complete => (Some(10 * 1024 * 1024), true, true), // 10MB potential loss
                };
                
                ImpactAssessment {
                    affected_regions: vec![*region_id],
                    affected_operations: vec!["all_operations".to_string()],
                    estimated_data_loss: data_loss,
                    rpo_violation,
                    rto_violation,
                }
            }
            FailureType::ConnectivityIssue { source_region, target_region, .. } => {
                ImpactAssessment {
                    affected_regions: vec![*source_region, *target_region],
                    affected_operations: vec!["cross_region_operations".to_string()],
                    estimated_data_loss: Some(0),
                    rpo_violation: false,
                    rto_violation: false,
                }
            }
            FailureType::DataCorruption { region_id, .. } => {
                ImpactAssessment {
                    affected_regions: vec![*region_id],
                    affected_operations: vec!["data_operations".to_string()],
                    estimated_data_loss: Some(1024 * 1024), // 1MB default
                    rpo_violation: true,
                    rto_violation: false,
                }
            }
        }
    }
    
    /// Start recovery manager
    async fn start_recovery_manager(&self) -> Result<()> {
        let active_failures = Arc::clone(&self.active_failures);
        let recovery_strategies = Arc::clone(&self.recovery_strategies);
        let failure_tx = self.failure_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut recovery_interval = interval(Duration::from_secs(10));
            recovery_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = recovery_interval.tick() => {
                        if let Err(e) = Self::manage_recovery(
                            &active_failures,
                            &recovery_strategies,
                            &failure_tx,
                        ).await {
                            error!("Recovery management error: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Recovery manager shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Manage recovery operations
    async fn manage_recovery(
        active_failures: &Arc<RwLock<HashMap<String, FailureInstance>>>,
        recovery_strategies: &Arc<RwLock<HashMap<String, RecoveryStrategy>>>,
        failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        let mut failures = active_failures.write().await;
        let strategies = recovery_strategies.read().await;
        
        for (failure_id, failure) in failures.iter_mut() {
            if failure.recovery_strategy.is_none() {
                // Determine recovery strategy
                let strategy = Self::determine_recovery_strategy(&failure.failure_type);
                failure.recovery_strategy = Some(strategy.clone());
                
                // Start recovery
                Self::start_recovery(failure_id.clone(), strategy, failure_tx).await?;
                failure.recovery_progress.started_at = Some(Instant::now());
            }
        }
        
        Ok(())
    }
    
    /// Determine the appropriate recovery strategy for a failure
    fn determine_recovery_strategy(failure_type: &FailureType) -> RecoveryStrategy {
        match failure_type {
            FailureType::NodeFailure { .. } => {
                RecoveryStrategy::WaitForRecovery(Duration::from_secs(300))
            }
            FailureType::NetworkPartition { .. } => {
                RecoveryStrategy::WaitForRecovery(Duration::from_secs(600))
            }
            FailureType::RegionOutage { severity, .. } => {
                match severity {
                    OutageSeverity::Minor => RecoveryStrategy::WaitForRecovery(Duration::from_secs(1800)),
                    OutageSeverity::Major => RecoveryStrategy::FailoverToBackup,
                    OutageSeverity::Complete => RecoveryStrategy::FailoverToBackup,
                }
            }
            FailureType::ConnectivityIssue { .. } => {
                RecoveryStrategy::WaitForRecovery(Duration::from_secs(300))
            }
            FailureType::DataCorruption { .. } => {
                RecoveryStrategy::DataReconstruction
            }
        }
    }
    
    /// Start recovery process
    async fn start_recovery(
        failure_id: String,
        strategy: RecoveryStrategy,
        failure_tx: &mpsc::UnboundedSender<FailureEvent>,
    ) -> Result<()> {
        let event = FailureEvent {
            event_type: FailureEventType::RecoveryStarted,
            failure_id: failure_id.clone(),
            timestamp: Instant::now(),
            details: format!("Started recovery with strategy: {:?}", strategy),
        };
        
        failure_tx.send(event)?;
        
        info!("Started recovery for failure {} with strategy {:?}", failure_id, strategy);
        
        Ok(())
    }
    
    /// Start notification handler
    async fn start_notification_handler(&self) -> Result<()> {
        let failure_rx = Arc::clone(&self.failure_rx);
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        tokio::spawn(async move {
            let mut rx = failure_rx.lock().await;
            
            loop {
                tokio::select! {
                    event = rx.recv() => {
                        match event {
                            Some(event) => {
                                if let Err(e) = Self::handle_failure_event(event).await {
                                    error!("Error handling failure event: {}", e);
                                }
                            }
                            None => {
                                warn!("Failure event channel closed");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Failure notification handler shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Handle a failure event
    async fn handle_failure_event(event: FailureEvent) -> Result<()> {
        match event.event_type {
            FailureEventType::FailureDetected => {
                warn!("FAILURE DETECTED: {} - {}", event.failure_id, event.details);
                // TODO: Send alerts to monitoring systems
            }
            FailureEventType::RecoveryStarted => {
                info!("RECOVERY STARTED: {} - {}", event.failure_id, event.details);
            }
            FailureEventType::RecoveryCompleted => {
                info!("RECOVERY COMPLETED: {} - {}", event.failure_id, event.details);
            }
            FailureEventType::RecoveryFailed => {
                error!("RECOVERY FAILED: {} - {}", event.failure_id, event.details);
            }
            FailureEventType::ManualInterventionRequired => {
                error!("MANUAL INTERVENTION REQUIRED: {} - {}", event.failure_id, event.details);
                // TODO: Send high-priority alerts
            }
        }
        
        Ok(())
    }
    
    /// Get current failure state
    pub async fn get_failure_state(&self) -> FailureState {
        self.state.read().await.clone()
    }
    
    /// Get all active failures
    pub async fn get_active_failures(&self) -> HashMap<String, FailureInstance> {
        self.active_failures.read().await.clone()
    }
    
    /// Manual recovery trigger
    pub async fn trigger_manual_recovery(&self, failure_id: &str) -> Result<()> {
        let mut failures = self.active_failures.write().await;
        
        if let Some(failure) = failures.get_mut(failure_id) {
            failure.recovery_strategy = Some(RecoveryStrategy::ManualIntervention);
            failure.last_updated = Instant::now();
            
            let event = FailureEvent {
                event_type: FailureEventType::ManualInterventionRequired,
                failure_id: failure_id.to_string(),
                timestamp: Instant::now(),
                details: "Manual recovery triggered".to_string(),
            };
            
            self.failure_tx.send(event)?;
            
            Ok(())
        } else {
            Err(anyhow!("Failure {} not found", failure_id))
        }
    }
    
    /// Shutdown the failure handler
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down failure handler");
        let _ = self.shutdown_tx.send(());
        Ok(())
    }
}

/// Health status of a region
#[derive(Debug, Clone, Copy)]
enum RegionHealthStatus {
    Healthy,
    Degraded,
    Unavailable,
}

// Helper trait extensions
trait DurationExt {
    fn from_mins(mins: u64) -> Duration;
}

impl DurationExt for Duration {
    fn from_mins(mins: u64) -> Duration {
        Duration::from_secs(mins * 60)
    }
}