use anyhow::{Result, Context};
use bytes::Bytes;
use std::path::PathBuf;
use std::collections::HashMap;
use tracing::{debug, info, warn, error};
use tokio::sync::RwLock;
use std::sync::Arc;

use crate::snapshot::{SnapshotManager, SnapshotMetadata};
use crate::wal::{WalReader};
use crate::replication::MetadataChange;

/// Recovery state information
#[derive(Debug, Clone)]
pub struct RecoveryState {
    pub last_snapshot: Option<SnapshotMetadata>,
    pub last_sequence_id: u64,
    pub entries_recovered: u64,
    pub recovery_time_ms: u64,
}

/// Recovery manager for rebuilding state from snapshots and WAL
pub struct RecoveryManager {
    data_dir: PathBuf,
    snapshot_manager: Arc<SnapshotManager>,
    state: Arc<RwLock<RecoveryState>>,
}

impl RecoveryManager {
    pub fn new(data_dir: PathBuf, snapshot_manager: Arc<SnapshotManager>) -> Self {
        Self {
            data_dir,
            snapshot_manager,
            state: Arc::new(RwLock::new(RecoveryState {
                last_snapshot: None,
                last_sequence_id: 0,
                entries_recovered: 0,
                recovery_time_ms: 0,
            })),
        }
    }

    /// Perform full recovery from snapshots and WAL
    pub async fn recover(&self) -> Result<HashMap<String, Bytes>> {
        let start_time = std::time::Instant::now();
        info!("Starting recovery process");
        
        let mut recovered_data = HashMap::new();
        let mut last_sequence_id = 0u64;
        
        // Step 1: Load latest snapshot if available
        let latest_snapshot = self.snapshot_manager.initialize().await?;
        
        if let Some(snapshot) = &latest_snapshot {
            info!("Loading snapshot: id={}, timestamp={}, entries={}", 
                  snapshot.id, snapshot.timestamp, snapshot.entries_count);
            
            let snapshot_data = self.snapshot_manager.load_snapshot(snapshot).await
                .context("Failed to load snapshot")?;
            
            // Apply snapshot data
            for (key, value) in snapshot_data {
                recovered_data.insert(key, value);
            }
            
            last_sequence_id = snapshot.sequence_id;
            info!("Loaded {} entries from snapshot, last sequence: {}", 
                  recovered_data.len(), last_sequence_id);
        } else {
            info!("No snapshot found, starting fresh recovery");
        }
        
        // Step 2: Apply WAL entries after snapshot
        let wal_dir = self.data_dir.join("wal");
        let wal_reader = WalReader::new(wal_dir);
        
        info!("Reading WAL entries from sequence: {}", last_sequence_id + 1);
        let wal_entries = wal_reader.read_from(last_sequence_id + 1).await
            .context("Failed to read WAL entries")?;
        
        info!("Found {} WAL entries to apply", wal_entries.len());
        
        // Apply WAL entries
        let entries_count = wal_entries.len() as u64;
        for (sequence_id, data) in wal_entries {
            // Parse and apply the change
            if let Ok(change) = self.parse_wal_entry(&data) {
                self.apply_change(&mut recovered_data, &change)?;
                last_sequence_id = sequence_id;
            } else {
                warn!("Failed to parse WAL entry with sequence: {}", sequence_id);
            }
        }
        
        // Update recovery state
        let recovery_time_ms = start_time.elapsed().as_millis() as u64;
        let mut state = self.state.write().await;
        *state = RecoveryState {
            last_snapshot: latest_snapshot,
            last_sequence_id,
            entries_recovered: recovered_data.len() as u64,
            recovery_time_ms,
        };
        
        info!("Recovery completed: {} entries recovered in {}ms, last sequence: {}", 
              recovered_data.len(), recovery_time_ms, last_sequence_id);
        
        Ok(recovered_data)
    }

    /// Parse WAL entry back into MetadataChange
    fn parse_wal_entry(&self, data: &Bytes) -> Result<MetadataChange> {
        if data.len() < 21 {
            return Err(anyhow::anyhow!("WAL entry too small"));
        }
        
        let mut cursor = 0;
        
        // Parse sequence_id
        let sequence_id = u64::from_le_bytes(
            data[cursor..cursor+8].try_into()
                .context("Failed to parse sequence_id")?
        );
        cursor += 8;
        
        // Parse timestamp
        let timestamp = i64::from_le_bytes(
            data[cursor..cursor+8].try_into()
                .context("Failed to parse timestamp")?
        );
        cursor += 8;
        
        // Parse operation type
        let operation = match data[cursor] {
            0 => crate::replication::OperationType::Create,
            1 => crate::replication::OperationType::Update,
            2 => crate::replication::OperationType::Delete,
            3 => crate::replication::OperationType::Rename,
            4 => crate::replication::OperationType::SetAttr,
            5 => crate::replication::OperationType::ChunkOperation,
            6 => crate::replication::OperationType::Transaction,
            _ => return Err(anyhow::anyhow!("Unknown operation type")),
        };
        cursor += 1;
        
        // Parse data length
        let data_len = u32::from_le_bytes(
            data[cursor..cursor+4].try_into()
                .context("Failed to parse data length")?
        ) as usize;
        cursor += 4;
        
        // Extract data
        if cursor + data_len > data.len() {
            return Err(anyhow::anyhow!("Invalid data length"));
        }
        
        let change_data = data.slice(cursor..cursor + data_len);
        
        Ok(MetadataChange {
            sequence_id,
            timestamp,
            operation,
            data: change_data,
        })
    }

    /// Apply a metadata change to the recovered state
    fn apply_change(
        &self, 
        state: &mut HashMap<String, Bytes>, 
        change: &MetadataChange
    ) -> Result<()> {
        // This is a simplified version - in reality, we'd need to parse
        // the change data based on the operation type
        match change.operation {
            crate::replication::OperationType::Create |
            crate::replication::OperationType::Update => {
                // Extract key from change data (simplified)
                if let Some(key) = self.extract_key_from_change(&change.data) {
                    state.insert(key, change.data.clone());
                }
            }
            crate::replication::OperationType::Delete => {
                // Extract key and remove
                if let Some(key) = self.extract_key_from_change(&change.data) {
                    state.remove(&key);
                }
            }
            crate::replication::OperationType::Rename => {
                // Would need to extract old and new keys
                debug!("Rename operation not fully implemented");
            }
            _ => {
                debug!("Operation {:?} not implemented", change.operation);
            }
        }
        
        Ok(())
    }

    /// Extract key from change data (simplified implementation)
    fn extract_key_from_change(&self, data: &Bytes) -> Option<String> {
        // In a real implementation, this would parse the actual
        // metadata structure to extract the key
        if data.len() >= 4 {
            let key_len = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
            if data.len() >= 4 + key_len {
                return String::from_utf8(data[4..4+key_len].to_vec()).ok();
            }
        }
        None
    }

    /// Get current recovery state
    pub async fn get_state(&self) -> RecoveryState {
        self.state.read().await.clone()
    }

    /// Create a checkpoint (snapshot) of current state
    pub async fn create_checkpoint(
        &self, 
        data: HashMap<String, Bytes>,
        sequence_id: u64
    ) -> Result<()> {
        info!("Creating checkpoint with {} entries at sequence {}", 
              data.len(), sequence_id);
        
        // Convert HashMap to Vec for snapshot
        let snapshot_data: Vec<(String, Bytes)> = data.into_iter().collect();
        
        // Create snapshot
        let mut metadata = self.snapshot_manager.create_snapshot(snapshot_data).await?;
        metadata.sequence_id = sequence_id;
        
        // Update state
        let mut state = self.state.write().await;
        state.last_snapshot = Some(metadata);
        state.last_sequence_id = sequence_id;
        
        Ok(())
    }

    /// Verify integrity of recovery data
    pub async fn verify_integrity(&self, data: &HashMap<String, Bytes>) -> Result<()> {
        info!("Verifying integrity of {} recovered entries", data.len());
        
        // Basic integrity checks
        let mut errors = 0;
        
        for (key, value) in data {
            // Check for empty keys
            if key.is_empty() {
                warn!("Found empty key in recovered data");
                errors += 1;
            }
            
            // Check for unreasonably large values
            if value.len() > 10 * 1024 * 1024 { // 10MB
                warn!("Found unusually large value for key: {}", key);
                errors += 1;
            }
            
            // Additional checks could be added here based on
            // the actual metadata structure
        }
        
        if errors > 0 {
            warn!("Found {} integrity issues during verification", errors);
        } else {
            info!("Integrity verification passed");
        }
        
        Ok(())
    }
}

/// Recovery progress tracker
pub struct RecoveryProgress {
    total_entries: u64,
    processed_entries: u64,
    start_time: std::time::Instant,
}

impl RecoveryProgress {
    pub fn new(total_entries: u64) -> Self {
        Self {
            total_entries,
            processed_entries: 0,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, processed: u64) {
        self.processed_entries = processed;
        
        if self.processed_entries % 10000 == 0 {
            let elapsed = self.start_time.elapsed().as_secs();
            let rate = if elapsed > 0 {
                self.processed_entries / elapsed
            } else {
                0
            };
            
            info!("Recovery progress: {}/{} entries ({:.1}%), rate: {} entries/sec",
                  self.processed_entries, self.total_entries,
                  (self.processed_entries as f64 / self.total_entries as f64) * 100.0,
                  rate);
        }
    }

    pub fn finish(&self) {
        let elapsed = self.start_time.elapsed();
        info!("Recovery completed: {} entries in {:.2}s",
              self.processed_entries, elapsed.as_secs_f64());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::config::SnapshotConfig;

    #[tokio::test]
    async fn test_recovery_empty() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = SnapshotConfig {
            interval: 3600,
            retention_count: 10,
            enable_compression: true,
            parallel_workers: 2,
        };
        
        let snapshot_manager = Arc::new(
            SnapshotManager::new(temp_dir.path().to_path_buf(), config).await?
        );
        
        let recovery_manager = RecoveryManager::new(
            temp_dir.path().to_path_buf(),
            snapshot_manager
        );
        
        // Recover from empty state
        let recovered = recovery_manager.recover().await?;
        assert_eq!(recovered.len(), 0);
        
        let state = recovery_manager.get_state().await;
        assert_eq!(state.entries_recovered, 0);
        assert_eq!(state.last_sequence_id, 0);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_recovery_with_checkpoint() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = SnapshotConfig {
            interval: 3600,
            retention_count: 10,
            enable_compression: false,
            parallel_workers: 1,
        };
        
        let snapshot_manager = Arc::new(
            SnapshotManager::new(temp_dir.path().to_path_buf(), config).await?
        );
        
        let recovery_manager = RecoveryManager::new(
            temp_dir.path().to_path_buf(),
            snapshot_manager
        );
        
        // Create some test data
        let mut test_data = HashMap::new();
        test_data.insert("key1".to_string(), Bytes::from("value1"));
        test_data.insert("key2".to_string(), Bytes::from("value2"));
        test_data.insert("key3".to_string(), Bytes::from("value3"));
        
        // Create checkpoint
        recovery_manager.create_checkpoint(test_data.clone(), 100).await?;
        
        // Recover
        let recovered = recovery_manager.recover().await?;
        assert_eq!(recovered.len(), 3);
        assert_eq!(recovered.get("key1"), Some(&Bytes::from("value1")));
        
        let state = recovery_manager.get_state().await;
        assert_eq!(state.entries_recovered, 3);
        assert!(state.last_snapshot.is_some());
        
        Ok(())
    }

    #[test]
    fn test_progress_tracker() {
        let mut progress = RecoveryProgress::new(100);
        progress.update(50);
        assert_eq!(progress.processed_entries, 50);
        progress.finish();
    }
}