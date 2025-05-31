// Hybrid Logical Clock implementation for distributed ordering
// Based on Google's TrueTime and HLC papers

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::collections::VecDeque;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::debug;

/// Hybrid Logical Clock for distributed event ordering
/// Combines physical time with logical counters to ensure monotonic ordering
#[derive(Debug)]
pub struct HybridLogicalClock {
    /// Last known physical time in microseconds
    last_physical_time: AtomicU64,
    
    /// Logical counter for events at same physical time
    logical_counter: AtomicU64,
    
    /// Maximum clock drift allowed (in microseconds)
    max_drift: u64,
    
    /// Node ID for this clock instance
    node_id: Option<u32>,
    
    /// Recent timestamps for drift detection
    recent_timestamps: Mutex<VecDeque<(HLCTimestamp, std::time::Instant)>>,
    
    /// Clock synchronization information
    sync_info: Mutex<ClockSyncInfo>,
}

/// HLC timestamp combining physical and logical components
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct HLCTimestamp {
    /// Physical time component (microseconds since epoch)
    pub physical: u64,
    
    /// Logical counter for disambiguation
    pub logical: u32,
    
    /// Node ID that generated this timestamp (for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<u32>,
}

/// Event with associated timestamp for causality tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedEvent {
    pub timestamp: HLCTimestamp,
    pub event_id: u64,
    pub event_type: String,
    pub data: Vec<u8>,
}

/// Clock synchronization info for debugging
#[derive(Debug, Clone)]
pub struct ClockSyncInfo {
    pub local_physical_time: u64,
    pub last_update_time: u64,
    pub drift_estimate: i64,
    pub sync_accuracy: Duration,
}

/// Clock drift statistics
#[derive(Debug, Clone, Default)]
pub struct ClockDriftStats {
    pub mean_drift: i64,
    pub median_drift: i64,
    pub max_drift: i64,
    pub min_drift: i64,
    pub sample_count: usize,
}

impl HybridLogicalClock {
    /// Create a new hybrid logical clock
    pub fn new(max_drift_ms: u64) -> Self {
        let now_micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
            
        Self {
            last_physical_time: AtomicU64::new(0),
            logical_counter: AtomicU64::new(0),
            max_drift: max_drift_ms * 1000, // Convert to microseconds
            node_id: None,
            recent_timestamps: Mutex::new(VecDeque::new()),
            sync_info: Mutex::new(ClockSyncInfo {
                local_physical_time: now_micros,
                last_update_time: now_micros,
                drift_estimate: 0,
                sync_accuracy: Duration::from_millis(max_drift_ms),
            }),
        }
    }
    
    /// Create a new hybrid logical clock with node ID
    pub fn new_with_node_id(max_drift_ms: u64, node_id: u32) -> Self {
        let mut clock = Self::new(max_drift_ms);
        clock.node_id = Some(node_id);
        clock
    }
    
    /// Get current timestamp
    pub fn now(&self) -> Result<HLCTimestamp> {
        let physical_now = Self::physical_time_micros()?;
        
        loop {
            let last_physical = self.last_physical_time.load(Ordering::SeqCst);
            let last_logical = self.logical_counter.load(Ordering::SeqCst);
            
            let (new_physical, new_logical) = if physical_now > last_physical {
                // Physical time has advanced
                (physical_now, 0)
            } else {
                // Same physical time, increment logical counter
                if last_logical >= u32::MAX as u64 {
                    return Err(anyhow!("Logical counter overflow"));
                }
                (last_physical, last_logical + 1)
            };
            
            // Try to update atomically
            if self.last_physical_time.compare_exchange(
                last_physical,
                new_physical,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                self.logical_counter.store(new_logical, Ordering::SeqCst);
                
                return Ok(HLCTimestamp {
                    physical: new_physical,
                    logical: new_logical as u32,
                    node_id: self.node_id,
                });
            }
            // Retry if concurrent update
        }
    }
    
    /// Update clock based on received timestamp
    pub fn update(&self, remote: HLCTimestamp) -> Result<HLCTimestamp> {
        let physical_now = Self::physical_time_micros()?;
        
        // Check for excessive clock drift
        if remote.physical > physical_now + self.max_drift {
            return Err(anyhow!(
                "Clock drift too large: remote {} us ahead",
                remote.physical - physical_now
            ));
        }
        
        loop {
            let last_physical = self.last_physical_time.load(Ordering::SeqCst);
            let last_logical = self.logical_counter.load(Ordering::SeqCst);
            
            let (new_physical, new_logical) = if physical_now > last_physical && 
                                                  physical_now > remote.physical {
                // Local physical time is ahead of both
                (physical_now, 0)
            } else if remote.physical > last_physical {
                // Remote time is ahead
                (remote.physical, remote.logical as u64 + 1)
            } else if remote.physical == last_physical {
                // Same physical time, use max logical + 1
                let max_logical = std::cmp::max(last_logical, remote.logical as u64);
                if max_logical >= u32::MAX as u64 {
                    return Err(anyhow!("Logical counter overflow"));
                }
                (last_physical, max_logical + 1)
            } else {
                // Local time is ahead
                (last_physical, last_logical + 1)
            };
            
            // Try to update atomically
            if self.last_physical_time.compare_exchange(
                last_physical,
                new_physical,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                self.logical_counter.store(new_logical, Ordering::SeqCst);
                
                return Ok(HLCTimestamp {
                    physical: new_physical,
                    logical: new_logical as u32,
                    node_id: self.node_id,
                });
            }
        }
    }
    
    /// Get physical time in microseconds since epoch
    fn physical_time_micros() -> Result<u64> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .map_err(|e| anyhow!("System time error: {}", e))
    }
    
    /// Compare if timestamp a happened-before timestamp b
    pub fn happened_before(a: HLCTimestamp, b: HLCTimestamp) -> bool {
        a < b
    }
    
    /// Check if two timestamps are concurrent (neither happened-before the other)
    pub fn are_concurrent(a: HLCTimestamp, b: HLCTimestamp) -> bool {
        a == b
    }
    
    /// Get clock synchronization information
    pub async fn get_sync_info(&self) -> ClockSyncInfo {
        let sync_info = self.sync_info.lock().await;
        sync_info.clone()
    }
    
    /// Update clock synchronization information based on external time source
    pub async fn sync_with_external(&self, external_time: u64, accuracy: Duration) -> Result<()> {
        let mut sync_info = self.sync_info.lock().await;
        let local_time = Self::physical_time_micros()?;
        
        sync_info.drift_estimate = external_time as i64 - local_time as i64;
        sync_info.sync_accuracy = accuracy;
        sync_info.last_update_time = local_time;
        
        debug!("Clock sync updated: drift={} us, accuracy={:?}", 
               sync_info.drift_estimate, accuracy);
        Ok(())
    }
    
    /// Create a timestamped event
    pub async fn create_event(&self, event_id: u64, event_type: String, data: Vec<u8>) -> Result<TimestampedEvent> {
        let timestamp = self.now()?;
        Ok(TimestampedEvent {
            timestamp,
            event_id,
            event_type,
            data,
        })
    }
    
    /// Calculate approximate network delay between two timestamps
    pub fn estimate_network_delay(&self, sent: HLCTimestamp, received: HLCTimestamp) -> Duration {
        if received.physical > sent.physical {
            Duration::from_micros(received.physical - sent.physical)
        } else {
            Duration::ZERO
        }
    }
    
    /// Get clock drift statistics from recent timestamps
    pub async fn get_drift_stats(&self) -> ClockDriftStats {
        let timestamps = self.recent_timestamps.lock().await;
        
        if timestamps.len() < 2 {
            return ClockDriftStats::default();
        }
        
        let mut drifts = Vec::new();
        let mut prev_timestamp = timestamps[0].0;
        let mut prev_real_time = timestamps[0].1;
        
        for (timestamp, real_time) in timestamps.iter().skip(1) {
            let hlc_elapsed = timestamp.physical - prev_timestamp.physical;
            let real_elapsed = real_time.duration_since(prev_real_time).as_micros() as u64;
            
            if real_elapsed > 0 {
                let drift = hlc_elapsed as i64 - real_elapsed as i64;
                drifts.push(drift);
            }
            
            prev_timestamp = *timestamp;
            prev_real_time = *real_time;
        }
        
        if drifts.is_empty() {
            return ClockDriftStats::default();
        }
        
        drifts.sort_unstable();
        let len = drifts.len();
        
        ClockDriftStats {
            mean_drift: drifts.iter().sum::<i64>() / len as i64,
            median_drift: drifts[len / 2],
            max_drift: *drifts.iter().max().unwrap(),
            min_drift: *drifts.iter().min().unwrap(),
            sample_count: len,
        }
    }
}

impl HLCTimestamp {
    /// Create a new timestamp
    pub fn new(physical: u64, logical: u32) -> Self {
        Self { physical, logical, node_id: None }
    }
    
    /// Create a new timestamp with node ID
    pub fn new_with_node(physical: u64, logical: u32, node_id: u32) -> Self {
        Self { physical, logical, node_id: Some(node_id) }
    }
    
    /// Convert to bytes for network transmission
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut bytes = [0u8; 12];
        bytes[0..8].copy_from_slice(&self.physical.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.logical.to_be_bytes());
        bytes
    }
    
    /// Create from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 12 {
            return Err(anyhow!("Invalid timestamp bytes length"));
        }
        
        let physical = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let logical = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
        
        Ok(Self { physical, logical, node_id: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hlc_monotonic() {
        let clock = HybridLogicalClock::new(100);
        
        let t1 = clock.now().unwrap();
        let t2 = clock.now().unwrap();
        let t3 = clock.now().unwrap();
        
        assert!(t1 < t2);
        assert!(t2 < t3);
    }
    
    #[test]
    fn test_hlc_update() {
        let clock = HybridLogicalClock::new(100);
        
        let t1 = clock.now().unwrap();
        
        // Simulate receiving a future timestamp
        let remote = HLCTimestamp::new(t1.physical + 1000, 5);
        let t2 = clock.update(remote).unwrap();
        
        assert!(t2.physical >= remote.physical);
        assert!(t2 > remote);
    }
}