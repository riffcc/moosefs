//! Resilience enhancements for Raft implementation
//! 
//! This module provides advanced resilience mechanisms including:
//! - Adaptive timeouts based on network conditions
//! - Circuit breaker patterns for peer communication
//! - Health monitoring and recovery mechanisms
//! - Network partition detection and handling

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, info, warn, error};

use crate::raft::{
    state::{NodeId, Term, LogIndex},
    config::RaftConfig,
};

/// Circuit breaker states for peer communication
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,    // Normal operation
    Open,      // Circuit is open, requests fail fast
    HalfOpen,  // Testing if circuit should close
}

/// Circuit breaker for managing peer communication failures
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    last_failure: Option<Instant>,
    last_success: Option<Instant>,
    failure_threshold: u32,
    recovery_timeout: Duration,
    test_requests_allowed: u32,
    test_requests_sent: u32,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            last_failure: None,
            last_success: None,
            failure_threshold,
            recovery_timeout,
            test_requests_allowed: 3,
            test_requests_sent: 0,
        }
    }
    
    /// Check if requests should be allowed through the circuit breaker
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = self.last_failure {
                    if last_failure.elapsed() >= self.recovery_timeout {
                        self.state = CircuitBreakerState::HalfOpen;
                        self.test_requests_sent = 0;
                        debug!("Circuit breaker transitioning to half-open state");
                        return true;
                    }
                }
                false
            }
            CircuitBreakerState::HalfOpen => {
                if self.test_requests_sent < self.test_requests_allowed {
                    self.test_requests_sent += 1;
                    true
                } else {
                    false
                }
            }
        }
    }
    
    /// Record a successful request
    pub fn record_success(&mut self) {
        self.last_success = Some(Instant::now());
        
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count = 0;
            }
            CircuitBreakerState::HalfOpen => {
                // Transition back to closed after successful test
                self.state = CircuitBreakerState::Closed;
                self.failure_count = 0;
                debug!("Circuit breaker closed after successful recovery test");
            }
            _ => {}
        }
    }
    
    /// Record a failed request
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());
        
        match self.state {
            CircuitBreakerState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open;
                    warn!("Circuit breaker opened after {} failures", self.failure_count);
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Go back to open state on failure during testing
                self.state = CircuitBreakerState::Open;
                warn!("Circuit breaker reopened after test failure");
            }
            _ => {}
        }
    }
    
    /// Get current circuit breaker status
    pub fn get_status(&self) -> CircuitBreakerStatus {
        CircuitBreakerStatus {
            state: self.state.clone(),
            failure_count: self.failure_count,
            last_failure: self.last_failure,
            last_success: self.last_success,
        }
    }
}

/// Circuit breaker status for monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    pub state: CircuitBreakerState,
    pub failure_count: u32,
    pub last_failure: Option<Instant>,
    pub last_success: Option<Instant>,
}

/// Adaptive timeout calculator based on network conditions
#[derive(Debug)]
pub struct AdaptiveTimeout {
    base_timeout: Duration,
    min_timeout: Duration,
    max_timeout: Duration,
    rtt_history: Vec<Duration>,
    max_history_size: usize,
    multiplier: f64,
}

impl AdaptiveTimeout {
    pub fn new(base_timeout: Duration, min_timeout: Duration, max_timeout: Duration) -> Self {
        Self {
            base_timeout,
            min_timeout,
            max_timeout,
            rtt_history: Vec::new(),
            max_history_size: 10,
            multiplier: 2.0,
        }
    }
    
    /// Record a new RTT measurement
    pub fn record_rtt(&mut self, rtt: Duration) {
        self.rtt_history.push(rtt);
        if self.rtt_history.len() > self.max_history_size {
            self.rtt_history.remove(0);
        }
    }
    
    /// Calculate adaptive timeout based on recent RTT history
    pub fn get_timeout(&self) -> Duration {
        if self.rtt_history.is_empty() {
            return self.base_timeout;
        }
        
        // Calculate 95th percentile RTT
        let mut sorted_rtts = self.rtt_history.clone();
        sorted_rtts.sort();
        
        let percentile_95_index = (sorted_rtts.len() as f64 * 0.95) as usize;
        let rtt_95 = sorted_rtts.get(percentile_95_index.min(sorted_rtts.len() - 1))
            .unwrap_or(&self.base_timeout);
        
        // Apply multiplier for safety margin
        let adaptive_timeout = Duration::from_nanos(
            (rtt_95.as_nanos() as f64 * self.multiplier) as u64
        );
        
        // Clamp to min/max bounds
        adaptive_timeout.max(self.min_timeout).min(self.max_timeout)
    }
    
    /// Get statistics about the adaptive timeout
    pub fn get_stats(&self) -> AdaptiveTimeoutStats {
        let avg_rtt = if !self.rtt_history.is_empty() {
            let total: Duration = self.rtt_history.iter().sum();
            total / self.rtt_history.len() as u32
        } else {
            Duration::from_millis(0)
        };
        
        AdaptiveTimeoutStats {
            current_timeout: self.get_timeout(),
            avg_rtt,
            sample_count: self.rtt_history.len(),
        }
    }
}

/// Statistics for adaptive timeout monitoring
#[derive(Debug, Clone)]
pub struct AdaptiveTimeoutStats {
    pub current_timeout: Duration,
    pub avg_rtt: Duration,
    pub sample_count: usize,
}

/// Peer health monitor for tracking individual peer status
#[derive(Debug)]
pub struct PeerHealthMonitor {
    circuit_breaker: CircuitBreaker,
    adaptive_timeout: AdaptiveTimeout,
    last_contact: Option<Instant>,
    consecutive_failures: u32,
    total_requests: u64,
    successful_requests: u64,
    is_suspected_partitioned: bool,
}

impl PeerHealthMonitor {
    pub fn new(config: &RaftConfig) -> Self {
        let circuit_breaker = CircuitBreaker::new(
            5, // failure threshold
            Duration::from_secs(30), // recovery timeout
        );
        
        let adaptive_timeout = AdaptiveTimeout::new(
            Duration::from_millis(config.rpc_timeout_ms),
            Duration::from_millis(config.rpc_timeout_ms / 2),
            Duration::from_millis(config.rpc_timeout_ms * 4),
        );
        
        Self {
            circuit_breaker,
            adaptive_timeout,
            last_contact: None,
            consecutive_failures: 0,
            total_requests: 0,
            successful_requests: 0,
            is_suspected_partitioned: false,
        }
    }
    
    /// Check if communication with this peer should be attempted
    pub fn should_attempt_communication(&mut self) -> bool {
        self.circuit_breaker.allow_request()
    }
    
    /// Record a successful communication with the peer
    pub fn record_success(&mut self, rtt: Duration) {
        self.circuit_breaker.record_success();
        self.adaptive_timeout.record_rtt(rtt);
        self.last_contact = Some(Instant::now());
        self.consecutive_failures = 0;
        self.total_requests += 1;
        self.successful_requests += 1;
        self.is_suspected_partitioned = false;
    }
    
    /// Record a failed communication with the peer
    pub fn record_failure(&mut self, is_timeout: bool) {
        self.circuit_breaker.record_failure();
        self.consecutive_failures += 1;
        self.total_requests += 1;
        
        // Detect potential network partition
        if is_timeout && self.consecutive_failures > 10 {
            self.is_suspected_partitioned = true;
        }
    }
    
    /// Get the current adaptive timeout for this peer
    pub fn get_timeout(&self) -> Duration {
        self.adaptive_timeout.get_timeout()
    }
    
    /// Check if peer is suspected to be in a network partition
    pub fn is_suspected_partitioned(&self) -> bool {
        self.is_suspected_partitioned
    }
    
    /// Get comprehensive health status
    pub fn get_health_status(&self) -> PeerHealthStatus {
        let success_rate = if self.total_requests > 0 {
            (self.successful_requests as f64 / self.total_requests as f64) * 100.0
        } else {
            100.0
        };
        
        PeerHealthStatus {
            circuit_breaker: self.circuit_breaker.get_status(),
            adaptive_timeout: self.adaptive_timeout.get_stats(),
            last_contact: self.last_contact,
            consecutive_failures: self.consecutive_failures,
            success_rate,
            is_suspected_partitioned: self.is_suspected_partitioned,
        }
    }
}

/// Comprehensive peer health status
#[derive(Debug, Clone)]
pub struct PeerHealthStatus {
    pub circuit_breaker: CircuitBreakerStatus,
    pub adaptive_timeout: AdaptiveTimeoutStats,
    pub last_contact: Option<Instant>,
    pub consecutive_failures: u32,
    pub success_rate: f64,
    pub is_suspected_partitioned: bool,
}

/// Cluster-wide resilience manager
#[derive(Debug)]
pub struct ResilienceManager {
    peer_monitors: Arc<RwLock<HashMap<NodeId, PeerHealthMonitor>>>,
    config: RaftConfig,
    cluster_health_threshold: f64,
}

impl ResilienceManager {
    pub fn new(config: RaftConfig) -> Self {
        Self {
            peer_monitors: Arc::new(RwLock::new(HashMap::new())),
            config,
            cluster_health_threshold: 0.5, // 50% of peers must be healthy
        }
    }
    
    /// Initialize monitoring for a set of peers
    pub async fn initialize_peers(&self, peer_ids: &[NodeId]) {
        let mut monitors = self.peer_monitors.write().await;
        for peer_id in peer_ids {
            monitors.insert(
                peer_id.clone(),
                PeerHealthMonitor::new(&self.config),
            );
        }
        info!("Initialized resilience monitoring for {} peers", peer_ids.len());
    }
    
    /// Check if communication with a peer should be attempted
    pub async fn should_attempt_communication(&self, peer_id: &NodeId) -> bool {
        let mut monitors = self.peer_monitors.write().await;
        if let Some(monitor) = monitors.get_mut(peer_id) {
            monitor.should_attempt_communication()
        } else {
            // Unknown peer - create monitor and allow communication
            monitors.insert(peer_id.clone(), PeerHealthMonitor::new(&self.config));
            true
        }
    }
    
    /// Record successful communication with a peer
    pub async fn record_success(&self, peer_id: &NodeId, rtt: Duration) {
        let mut monitors = self.peer_monitors.write().await;
        if let Some(monitor) = monitors.get_mut(peer_id) {
            monitor.record_success(rtt);
        }
    }
    
    /// Record failed communication with a peer
    pub async fn record_failure(&self, peer_id: &NodeId, is_timeout: bool) {
        let mut monitors = self.peer_monitors.write().await;
        if let Some(monitor) = monitors.get_mut(peer_id) {
            monitor.record_failure(is_timeout);
        }
    }
    
    /// Get adaptive timeout for a specific peer
    pub async fn get_peer_timeout(&self, peer_id: &NodeId) -> Duration {
        let monitors = self.peer_monitors.read().await;
        if let Some(monitor) = monitors.get(peer_id) {
            monitor.get_timeout()
        } else {
            Duration::from_millis(self.config.rpc_timeout_ms)
        }
    }
    
    /// Check overall cluster health
    pub async fn is_cluster_healthy(&self) -> bool {
        let monitors = self.peer_monitors.read().await;
        if monitors.is_empty() {
            return true; // No peers to monitor
        }
        
        let healthy_peers = monitors.values()
            .filter(|monitor| {
                monitor.circuit_breaker.state == CircuitBreakerState::Closed &&
                !monitor.is_suspected_partitioned()
            })
            .count();
        
        let health_ratio = healthy_peers as f64 / monitors.len() as f64;
        health_ratio >= self.cluster_health_threshold
    }
    
    /// Get health status for all peers
    pub async fn get_cluster_health_status(&self) -> HashMap<NodeId, PeerHealthStatus> {
        let monitors = self.peer_monitors.read().await;
        monitors.iter()
            .map(|(id, monitor)| (id.clone(), monitor.get_health_status()))
            .collect()
    }
    
    /// Detect potential network partitions
    pub async fn detect_network_partition(&self) -> Option<Vec<NodeId>> {
        let monitors = self.peer_monitors.read().await;
        let partitioned_peers: Vec<NodeId> = monitors.iter()
            .filter(|(_, monitor)| monitor.is_suspected_partitioned())
            .map(|(id, _)| id.clone())
            .collect();
        
        if partitioned_peers.is_empty() {
            None
        } else {
            Some(partitioned_peers)
        }
    }
    
    /// Get resilience statistics
    pub async fn get_resilience_stats(&self) -> ResilienceStats {
        let monitors = self.peer_monitors.read().await;
        let total_peers = monitors.len();
        
        let healthy_peers = monitors.values()
            .filter(|monitor| monitor.circuit_breaker.state == CircuitBreakerState::Closed)
            .count();
        
        let partitioned_peers = monitors.values()
            .filter(|monitor| monitor.is_suspected_partitioned())
            .count();
        
        let avg_success_rate = if total_peers > 0 {
            monitors.values()
                .map(|monitor| {
                    if monitor.total_requests > 0 {
                        (monitor.successful_requests as f64 / monitor.total_requests as f64) * 100.0
                    } else {
                        100.0
                    }
                })
                .sum::<f64>() / total_peers as f64
        } else {
            100.0
        };
        
        ResilienceStats {
            total_peers,
            healthy_peers,
            partitioned_peers,
            avg_success_rate,
            cluster_health_ratio: if total_peers > 0 {
                healthy_peers as f64 / total_peers as f64
            } else {
                1.0
            },
        }
    }
}

/// Overall resilience statistics
#[derive(Debug, Clone)]
pub struct ResilienceStats {
    pub total_peers: usize,
    pub healthy_peers: usize,
    pub partitioned_peers: usize,
    pub avg_success_rate: f64,
    pub cluster_health_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_circuit_breaker_states() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(10));
        
        // Initially closed
        assert!(cb.allow_request());
        
        // Record failures to open circuit
        cb.record_failure();
        cb.record_failure();
        assert!(cb.allow_request()); // Still closed
        
        cb.record_failure();
        assert_eq!(cb.state, CircuitBreakerState::Open);
        assert!(!cb.allow_request()); // Now open
    }
    
    #[test]
    fn test_adaptive_timeout() {
        let mut timeout = AdaptiveTimeout::new(
            Duration::from_millis(100),
            Duration::from_millis(50),
            Duration::from_millis(500),
        );
        
        // Add some RTT samples
        timeout.record_rtt(Duration::from_millis(20));
        timeout.record_rtt(Duration::from_millis(30));
        timeout.record_rtt(Duration::from_millis(25));
        
        let adaptive = timeout.get_timeout();
        assert!(adaptive >= Duration::from_millis(50));
        assert!(adaptive <= Duration::from_millis(500));
    }
}