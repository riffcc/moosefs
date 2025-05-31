use anyhow::{Result, Context};
use bytes::Bytes;
use mooseng_common::{retry_with_backoff, RetryConfig};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};
use tonic::transport::Channel;

use crate::config::ReplicationConfig;
use crate::wal::WalWriter;

/// Metadata change event from master server
#[derive(Debug, Clone)]
pub struct MetadataChange {
    pub sequence_id: u64,
    pub timestamp: i64,
    pub operation: OperationType,
    pub data: Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationType {
    Create,
    Update,
    Delete,
    Rename,
    SetAttr,
    ChunkOperation,
    Transaction,
}

/// Real-time replication client that connects to master server
pub struct ReplicationClient {
    config: ReplicationConfig,
    master_channel: Option<Channel>,
    wal_writer: Arc<WalWriter>,
    change_receiver: broadcast::Receiver<MetadataChange>,
    change_sender: broadcast::Sender<MetadataChange>,
    last_sequence_id: Arc<RwLock<u64>>,
    batch_buffer: Vec<MetadataChange>,
}

impl ReplicationClient {
    pub fn new(
        config: ReplicationConfig,
        wal_writer: Arc<WalWriter>,
    ) -> Result<Self> {
        let (change_sender, change_receiver) = broadcast::channel(config.buffer_size);
        let batch_size = config.batch_size;
        
        Ok(Self {
            config,
            master_channel: None,
            wal_writer,
            change_receiver,
            change_sender,
            last_sequence_id: Arc::new(RwLock::new(0)),
            batch_buffer: Vec::with_capacity(batch_size),
        })
    }

    /// Connect to master server
    pub async fn connect(&mut self, master_address: &str) -> Result<()> {
        info!("Connecting to master server at {}", master_address);
        
        let retry_config = RetryConfig {
            max_attempts: self.config.retry_attempts,
            initial_backoff: Duration::from_millis(self.config.retry_delay_ms),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        };

        let channel = retry_with_backoff(&retry_config, || async {
            Channel::from_shared(master_address.to_string())
                .context("Invalid master address")?
                .connect()
                .await
                .context("Failed to connect to master")
        }).await?;

        self.master_channel = Some(channel);
        info!("Successfully connected to master server");
        Ok(())
    }

    /// Start replication from master
    pub async fn start_replication(&mut self) -> Result<()> {
        let channel = self.master_channel
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to master"))?;

        // Get last processed sequence ID
        let last_seq = *self.last_sequence_id.read().await;
        info!("Starting replication from sequence ID: {}", last_seq);

        // TODO: Create gRPC client and start streaming changes
        // For now, we'll simulate receiving changes
        let change_sender = self.change_sender.clone();
        let mut flush_interval = interval(Duration::from_millis(self.config.flush_interval_ms));

        loop {
            tokio::select! {
                // Process incoming changes
                Ok(change) = self.change_receiver.recv() => {
                    self.batch_buffer.push(change);
                    
                    // Process batch if full
                    if self.batch_buffer.len() >= self.config.batch_size {
                        self.process_batch().await?;
                    }
                }
                
                // Periodic flush
                _ = flush_interval.tick() => {
                    if !self.batch_buffer.is_empty() {
                        self.process_batch().await?;
                    }
                }
            }
        }
    }

    /// Process a batch of changes
    async fn process_batch(&mut self) -> Result<()> {
        if self.batch_buffer.is_empty() {
            return Ok(());
        }

        debug!("Processing batch of {} changes", self.batch_buffer.len());
        
        // Write batch to WAL
        let batch = std::mem::replace(
            &mut self.batch_buffer, 
            Vec::with_capacity(self.config.batch_size)
        );
        
        let mut wal_entries = Vec::with_capacity(batch.len());
        for change in &batch {
            wal_entries.push(self.serialize_change(change)?);
        }
        
        self.wal_writer.write_batch(wal_entries).await?;

        // Update last sequence ID
        if let Some(last_change) = batch.last() {
            let mut last_seq = self.last_sequence_id.write().await;
            *last_seq = last_change.sequence_id;
            debug!("Updated last sequence ID to: {}", last_change.sequence_id);
        }

        Ok(())
    }

    /// Serialize metadata change for WAL
    fn serialize_change(&self, change: &MetadataChange) -> Result<Bytes> {
        // Simple serialization format:
        // [sequence_id: 8 bytes][timestamp: 8 bytes][operation: 1 byte][data_len: 4 bytes][data]
        let mut buffer = Vec::new();
        
        buffer.extend_from_slice(&change.sequence_id.to_le_bytes());
        buffer.extend_from_slice(&change.timestamp.to_le_bytes());
        buffer.push(change.operation as u8);
        buffer.extend_from_slice(&(change.data.len() as u32).to_le_bytes());
        buffer.extend_from_slice(&change.data);

        Ok(Bytes::from(buffer))
    }

    /// Get a receiver for metadata changes
    pub fn subscribe(&self) -> broadcast::Receiver<MetadataChange> {
        self.change_sender.subscribe()
    }

    /// Simulate receiving a metadata change (for testing)
    #[cfg(test)]
    pub async fn inject_change(&self, change: MetadataChange) -> Result<()> {
        self.change_sender.send(change)?;
        Ok(())
    }

    /// Get the last processed sequence ID
    pub async fn get_last_sequence_id(&self) -> u64 {
        *self.last_sequence_id.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_replication_client() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = ReplicationConfig {
            buffer_size: 100,
            batch_size: 10,
            flush_interval_ms: 100,
            enable_compression: false,
            retry_attempts: 3,
            retry_delay_ms: 100,
        };
        
        let wal_writer = Arc::new(WalWriter::new(temp_dir.path().to_path_buf()).await?);
        
        let client = ReplicationClient::new(config, wal_writer)?;
        
        // Test change injection
        let change = MetadataChange {
            sequence_id: 1,
            timestamp: 1234567890,
            operation: OperationType::Create,
            data: Bytes::from("test data"),
        };
        
        client.inject_change(change.clone()).await?;
        
        // Verify we can subscribe and receive changes
        let mut subscriber = client.subscribe();
        let received = subscriber.recv().await?;
        assert_eq!(received.sequence_id, change.sequence_id);
        
        Ok(())
    }
}