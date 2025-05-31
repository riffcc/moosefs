//! High-performance networking utilities for MooseNG
//! 
//! This module provides:
//! - Connection pooling for gRPC clients
//! - Network optimization utilities
//! - Compression and batching helpers
//! - Connection health monitoring

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::{interval, timeout};
use tonic::transport::{Channel, Endpoint, Certificate, Identity, ClientTlsConfig, ServerTlsConfig};
use tracing::{debug, info, warn};

use std::io::BufReader;
use std::sync::Arc as StdArc;
use rustls::{RootCertStore, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use tokio_rustls::TlsAcceptor;

use crate::error::MooseNGError;

/// TLS configuration for secure connections
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Enable TLS encryption
    pub enabled: bool,
    /// Certificate authority bundle for verifying server certificates
    pub ca_cert_path: Option<String>,
    /// Client certificate for mutual TLS authentication
    pub client_cert_path: Option<String>,
    /// Client private key for mutual TLS authentication
    pub client_key_path: Option<String>,
    /// Server certificate for TLS termination
    pub server_cert_path: Option<String>,
    /// Server private key for TLS termination
    pub server_key_path: Option<String>,
    /// Require client certificates (mutual TLS)
    pub require_client_cert: bool,
    /// Domain name for certificate verification
    pub domain_name: Option<String>,
    /// Skip certificate verification (insecure, for testing only)
    pub insecure_skip_verify: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
            server_cert_path: None,
            server_key_path: None,
            require_client_cert: false,
            domain_name: None,
            insecure_skip_verify: false,
        }
    }
}

impl TlsConfig {
    /// Create a new TLS config for client connections
    pub fn client() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Create a new TLS config for server connections
    pub fn server() -> Self {
        Self {
            enabled: true,
            require_client_cert: true,
            ..Default::default()
        }
    }

    /// Disable TLS encryption (for development only)
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Set client certificate and key for mutual TLS
    pub fn with_client_cert(mut self, cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        self.client_cert_path = Some(cert_path.into());
        self.client_key_path = Some(key_path.into());
        self
    }

    /// Set server certificate and key for TLS termination
    pub fn with_server_cert(mut self, cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        self.server_cert_path = Some(cert_path.into());
        self.server_key_path = Some(key_path.into());
        self
    }

    /// Set CA certificate bundle for verification
    pub fn with_ca_cert(mut self, ca_path: impl Into<String>) -> Self {
        self.ca_cert_path = Some(ca_path.into());
        self
    }

    /// Set domain name for certificate verification
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain_name = Some(domain.into());
        self
    }

    /// Skip certificate verification (insecure!)
    pub fn insecure(mut self) -> Self {
        self.insecure_skip_verify = true;
        self
    }
}

/// Certificate manager for TLS operations
pub struct CertificateManager {
    config: TlsConfig,
    #[allow(dead_code)]
    root_store: RootCertStore,
    client_identity: Option<Identity>,
    server_config: Option<StdArc<ServerConfig>>,
}

impl CertificateManager {
    /// Create a new certificate manager
    pub fn new(config: TlsConfig) -> Result<Self> {
        let mut root_store = RootCertStore::empty();
        
        // Load CA certificates
        if let Some(ca_path) = &config.ca_cert_path {
            Self::load_ca_certificates(&mut root_store, ca_path)?;
        } else {
            // Use system root certificates
            root_store.extend(
                webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| ta.to_owned())
            );
        }

        // Load client identity if configured
        let client_identity = if let (Some(cert_path), Some(key_path)) = 
            (&config.client_cert_path, &config.client_key_path) {
            Some(Self::load_identity(cert_path, key_path)?)
        } else {
            None
        };

        // Load server config if configured
        let server_config = if let (Some(cert_path), Some(key_path)) = 
            (&config.server_cert_path, &config.server_key_path) {
            Some(Self::create_server_config(cert_path, key_path, config.require_client_cert)?)
        } else {
            None
        };

        Ok(Self {
            config,
            root_store,
            client_identity,
            server_config,
        })
    }

    /// Load CA certificates from file
    fn load_ca_certificates(root_store: &mut RootCertStore, ca_path: &str) -> Result<()> {
        let ca_file = std::fs::File::open(ca_path)
            .with_context(|| format!("Failed to open CA certificate file: {}", ca_path))?;
        let mut ca_reader = BufReader::new(ca_file);
        let ca_certs = certs(&mut ca_reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse CA certificates")?;

        for cert in ca_certs {
            root_store.add(cert).context("Failed to add CA certificate to store")?;
        }

        Ok(())
    }

    /// Load client/server identity (certificate + private key)
    fn load_identity(cert_path: &str, key_path: &str) -> Result<Identity> {
        let cert_pem = std::fs::read_to_string(cert_path)
            .with_context(|| format!("Failed to read certificate file: {}", cert_path))?;
        let key_pem = std::fs::read_to_string(key_path)
            .with_context(|| format!("Failed to read private key file: {}", key_path))?;

        Ok(Identity::from_pem(cert_pem, key_pem))
    }

    /// Create server TLS configuration
    fn create_server_config(cert_path: &str, key_path: &str, require_client_cert: bool) -> Result<StdArc<ServerConfig>> {
        let cert_file = std::fs::File::open(cert_path)
            .with_context(|| format!("Failed to open server certificate file: {}", cert_path))?;
        let mut cert_reader = BufReader::new(cert_file);
        let cert_chain = certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse server certificate chain")?;

        let key_file = std::fs::File::open(key_path)
            .with_context(|| format!("Failed to open server private key file: {}", key_path))?;
        let mut key_reader = BufReader::new(key_file);
        let mut keys = pkcs8_private_keys(&mut key_reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse server private key")?;

        if keys.is_empty() {
            return Err(anyhow::anyhow!("No private keys found in key file"));
        }

        let private_key = rustls::pki_types::PrivateKeyDer::Pkcs8(keys.remove(0));

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .context("Failed to create server TLS configuration")?;

        // Enable client certificate verification if required
        if require_client_cert {
            // TODO: Implement client certificate verification
            warn!("Client certificate verification requested but not yet implemented");
        }

        Ok(StdArc::new(config))
    }

    /// Create client TLS configuration for gRPC
    pub fn create_client_tls_config(&self) -> Result<Option<ClientTlsConfig>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let mut tls_config = ClientTlsConfig::new();

        // Set domain name for certificate verification
        if let Some(domain) = &self.config.domain_name {
            tls_config = tls_config.domain_name(domain.clone());
        }

        // Add CA certificates
        if let Some(ca_path) = &self.config.ca_cert_path {
            let ca_pem = std::fs::read_to_string(ca_path)
                .with_context(|| format!("Failed to read CA certificate: {}", ca_path))?;
            let ca_cert = Certificate::from_pem(ca_pem);
            tls_config = tls_config.ca_certificate(ca_cert);
        }

        // Add client identity for mutual TLS
        if let Some(identity) = &self.client_identity {
            tls_config = tls_config.identity(identity.clone());
        }

        Ok(Some(tls_config))
    }

    /// Create server TLS configuration for gRPC
    pub fn create_server_tls_config(&self) -> Result<Option<ServerTlsConfig>> {
        if !self.config.enabled {
            return Ok(None);
        }

        // Load server identity from config paths
        if let (Some(cert_path), Some(key_path)) = 
            (&self.config.server_cert_path, &self.config.server_key_path) {
            let server_identity = Self::load_identity(cert_path, key_path)?;
            let mut tls_config = ServerTlsConfig::new().identity(server_identity);

            // Add client CA certificates for mutual TLS
            if self.config.require_client_cert {
                if let Some(ca_path) = &self.config.ca_cert_path {
                    let ca_pem = std::fs::read_to_string(ca_path)
                        .with_context(|| format!("Failed to read CA certificate: {}", ca_path))?;
                    let ca_cert = Certificate::from_pem(ca_pem);
                    tls_config = tls_config.client_ca_root(ca_cert);
                }
            }

            Ok(Some(tls_config))
        } else {
            Err(anyhow::anyhow!("Server TLS configuration requires server certificate and key"))
        }
    }

    /// Get TLS acceptor for raw TCP connections
    pub fn get_tls_acceptor(&self) -> Option<TlsAcceptor> {
        self.server_config.as_ref().map(|config| TlsAcceptor::from(config.clone()))
    }
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum connections per endpoint
    pub max_connections_per_endpoint: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
    /// Keep-alive timeout
    pub keep_alive_timeout: Duration,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Maximum retries for connection establishment
    pub max_connect_retries: u32,
    /// Enable HTTP/2 adaptive window
    pub enable_adaptive_window: bool,
    /// Initial connection window size
    pub initial_window_size: u32,
    /// TLS configuration for secure connections
    pub tls_config: TlsConfig,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_endpoint: 10,
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            keep_alive_interval: Duration::from_secs(30),
            keep_alive_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300), // 5 minutes
            max_connect_retries: 3,
            enable_adaptive_window: true,
            initial_window_size: 1024 * 1024, // 1MB
            tls_config: TlsConfig::default(),
        }
    }
}

/// A pool entry containing connection info
#[derive(Debug)]
struct PoolEntry {
    channel: Channel,
    #[allow(dead_code)]
    created_at: Instant,
    last_used: Instant,
    request_count: u64,
    error_count: u64,
}

impl PoolEntry {
    fn new(channel: Channel) -> Self {
        let now = Instant::now();
        Self {
            channel,
            created_at: now,
            last_used: now,
            request_count: 0,
            error_count: 0,
        }
    }
    
    fn use_connection(&mut self) {
        self.last_used = Instant::now();
        self.request_count += 1;
    }
    
    fn record_error(&mut self) {
        self.error_count += 1;
    }
    
    fn is_healthy(&self) -> bool {
        // Consider connection unhealthy if error rate > 50%
        if self.request_count > 10 {
            (self.error_count as f64 / self.request_count as f64) < 0.5
        } else {
            true
        }
    }
    
    fn is_expired(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() > idle_timeout
    }
}

/// High-performance connection pool for gRPC clients
pub struct ConnectionPool {
    config: ConnectionPoolConfig,
    pools: Arc<RwLock<HashMap<String, Arc<Mutex<Vec<PoolEntry>>>>>>,
    semaphore: Arc<Semaphore>,
    cert_manager: Option<CertificateManager>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: ConnectionPoolConfig) -> Result<Self> {
        let total_permits = config.max_connections_per_endpoint * 100; // Support up to 100 endpoints
        
        // Initialize certificate manager if TLS is enabled
        let cert_manager = if config.tls_config.enabled {
            Some(CertificateManager::new(config.tls_config.clone())?)
        } else {
            None
        };
        
        Ok(Self {
            semaphore: Arc::new(Semaphore::new(total_permits)),
            pools: Arc::new(RwLock::new(HashMap::new())),
            config,
            cert_manager,
        })
    }
    
    /// Get a connection to the specified endpoint
    pub async fn get_connection(&self, endpoint: &str) -> Result<ManagedConnection> {
        // Acquire semaphore permit
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| MooseNGError::ResourceExhausted("Too many connections".into()))?;
        
        // Get or create pool for endpoint
        let pool = {
            let pools = self.pools.read().await;
            if let Some(pool) = pools.get(endpoint) {
                pool.clone()
            } else {
                drop(pools);
                
                // Create new pool under write lock
                let mut pools = self.pools.write().await;
                if let Some(pool) = pools.get(endpoint) {
                    pool.clone()
                } else {
                    let new_pool = Arc::new(Mutex::new(Vec::new()));
                    pools.insert(endpoint.to_string(), new_pool.clone());
                    new_pool
                }
            }
        };
        
        // Try to get existing connection
        {
            let mut pool_entries = pool.lock().await;
            
            // Find healthy, non-expired connection
            if let Some(pos) = pool_entries.iter().position(|entry| {
                entry.is_healthy() && !entry.is_expired(self.config.idle_timeout)
            }) {
                let mut entry = pool_entries.remove(pos);
                entry.use_connection();
                
                return Ok(ManagedConnection {
                    channel: entry.channel.clone(),
                    pool: pool.clone(),
                    endpoint: endpoint.to_string(),
                    _permit: permit,
                    entry: Some(entry),
                });
            }
        }
        
        // Create new connection
        let channel = self.create_connection(endpoint).await?;
        let entry = PoolEntry::new(channel.clone());
        
        Ok(ManagedConnection {
            channel,
            pool,
            endpoint: endpoint.to_string(),
            _permit: permit,
            entry: Some(entry),
        })
    }
    
    /// Create a new connection with optimized settings
    async fn create_connection(&self, endpoint: &str) -> Result<Channel> {
        let mut endpoint = Endpoint::from_shared(endpoint.to_string())
            .context("Invalid endpoint URL")?
            .connect_timeout(self.config.connect_timeout)
            .timeout(self.config.request_timeout)
            .keep_alive_timeout(self.config.keep_alive_timeout)
            .http2_keep_alive_interval(self.config.keep_alive_interval)
            .initial_stream_window_size(Some(self.config.initial_window_size));
        
        if self.config.enable_adaptive_window {
            endpoint = endpoint.http2_adaptive_window(true);
        }

        // Configure TLS if enabled
        if let Some(cert_manager) = &self.cert_manager {
            if let Some(tls_config) = cert_manager.create_client_tls_config()? {
                endpoint = endpoint.tls_config(tls_config)?;
                info!("TLS encryption enabled for connection to {:?}", endpoint);
            }
        }
        
        // Retry connection establishment
        let mut last_error: Option<anyhow::Error> = None;
        for attempt in 1..=self.config.max_connect_retries {
            match timeout(self.config.connect_timeout, endpoint.connect()).await {
                Ok(Ok(channel)) => {
                    debug!("Connected to {:?} on attempt {}", endpoint, attempt);
                    return Ok(channel);
                }
                Ok(Err(e)) => {
                    warn!("Connection attempt {} to {:?} failed: {}", attempt, endpoint, e);
                    last_error = Some(e.into());
                }
                Err(_) => {
                    warn!("Connection attempt {} to {:?} timed out", attempt, endpoint);
                    last_error = Some(anyhow::anyhow!("Connection timeout"));
                }
            }
            
            if attempt < self.config.max_connect_retries {
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown connection error")))
    }
    
    /// Start background cleanup task
    pub fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let pools = self.pools.clone();
        let idle_timeout = self.config.idle_timeout;
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60));
            
            loop {
                cleanup_interval.tick().await;
                
                let pools_read = pools.read().await;
                for (endpoint, pool) in pools_read.iter() {
                    let mut pool_entries = pool.lock().await;
                    let initial_count = pool_entries.len();
                    
                    // Remove expired or unhealthy connections
                    pool_entries.retain(|entry| {
                        !entry.is_expired(idle_timeout) && entry.is_healthy()
                    });
                    
                    let removed_count = initial_count - pool_entries.len();
                    if removed_count > 0 {
                        debug!("Cleaned up {} expired connections for {}", removed_count, endpoint);
                    }
                }
            }
        })
    }
    
    /// Get pool statistics
    pub async fn get_stats(&self) -> HashMap<String, PoolStats> {
        let mut stats = HashMap::new();
        let pools = self.pools.read().await;
        
        for (endpoint, pool) in pools.iter() {
            let pool_entries = pool.lock().await;
            let mut pool_stats = PoolStats {
                total_connections: pool_entries.len(),
                healthy_connections: 0,
                total_requests: 0,
                total_errors: 0,
                avg_requests_per_connection: 0.0,
                error_rate: 0.0,
            };
            
            for entry in pool_entries.iter() {
                if entry.is_healthy() {
                    pool_stats.healthy_connections += 1;
                }
                pool_stats.total_requests += entry.request_count;
                pool_stats.total_errors += entry.error_count;
            }
            
            if pool_stats.total_connections > 0 {
                pool_stats.avg_requests_per_connection = 
                    pool_stats.total_requests as f64 / pool_stats.total_connections as f64;
            }
            
            if pool_stats.total_requests > 0 {
                pool_stats.error_rate = 
                    pool_stats.total_errors as f64 / pool_stats.total_requests as f64;
            }
            
            stats.insert(endpoint.clone(), pool_stats);
        }
        
        stats
    }
}

/// Statistics for a connection pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_connections: usize,
    pub healthy_connections: usize,
    pub total_requests: u64,
    pub total_errors: u64,
    pub avg_requests_per_connection: f64,
    pub error_rate: f64,
}

/// A managed connection that returns to the pool when dropped
pub struct ManagedConnection {
    pub channel: Channel,
    pool: Arc<Mutex<Vec<PoolEntry>>>,
    #[allow(dead_code)]
    endpoint: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
    entry: Option<PoolEntry>,
}

impl ManagedConnection {
    /// Record a successful request
    pub fn record_success(&mut self) {
        if let Some(ref mut entry) = self.entry {
            entry.use_connection();
        }
    }
    
    /// Record a failed request
    pub fn record_error(&mut self) {
        if let Some(ref mut entry) = self.entry {
            entry.record_error();
        }
    }
    
    /// Get connection statistics
    pub fn get_stats(&self) -> Option<(u64, u64, f64)> {
        self.entry.as_ref().map(|entry| {
            let error_rate = if entry.request_count > 0 {
                entry.error_count as f64 / entry.request_count as f64
            } else {
                0.0
            };
            (entry.request_count, entry.error_count, error_rate)
        })
    }
}

impl Drop for ManagedConnection {
    fn drop(&mut self) {
        if let Some(entry) = self.entry.take() {
            // Return connection to pool if it's still healthy
            if entry.is_healthy() && !entry.is_expired(Duration::from_secs(300)) {
                let pool = self.pool.clone();
                tokio::spawn(async move {
                    let mut pool_entries = pool.lock().await;
                    pool_entries.push(entry);
                });
            }
        }
    }
}

/// Network compression utilities
pub mod compression {
    use bytes::Bytes;
    use std::io::{Read, Write};
    
    /// Compression algorithm
    #[derive(Debug, Clone, Copy)]
    pub enum CompressionAlgorithm {
        None,
        Gzip,
        Lz4,
        Zstd,
    }
    
    /// Compress data using the specified algorithm
    pub fn compress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Bytes, Box<dyn std::error::Error>> {
        match algorithm {
            CompressionAlgorithm::None => Ok(Bytes::copy_from_slice(data)),
            CompressionAlgorithm::Gzip => {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data)?;
                let compressed = encoder.finish()?;
                Ok(Bytes::from(compressed))
            }
            CompressionAlgorithm::Lz4 => {
                // TODO: Implement LZ4 compression
                Ok(Bytes::copy_from_slice(data))
            }
            CompressionAlgorithm::Zstd => {
                // TODO: Implement Zstd compression
                Ok(Bytes::copy_from_slice(data))
            }
        }
    }
    
    /// Decompress data using the specified algorithm
    pub fn decompress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Bytes, Box<dyn std::error::Error>> {
        match algorithm {
            CompressionAlgorithm::None => Ok(Bytes::copy_from_slice(data)),
            CompressionAlgorithm::Gzip => {
                use flate2::read::GzDecoder;
                
                let mut decoder = GzDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)?;
                Ok(Bytes::from(decompressed))
            }
            CompressionAlgorithm::Lz4 => {
                // TODO: Implement LZ4 decompression
                Ok(Bytes::copy_from_slice(data))
            }
            CompressionAlgorithm::Zstd => {
                // TODO: Implement Zstd decompression
                Ok(Bytes::copy_from_slice(data))
            }
        }
    }
}

/// Batch request utilities
pub mod batching {
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;
    use tokio::time::interval;
    
    /// Batch configuration
    #[derive(Debug, Clone)]
    pub struct BatchConfig {
        pub max_size: usize,
        pub max_wait: Duration,
        pub max_bytes: usize,
    }
    
    impl Default for BatchConfig {
        fn default() -> Self {
            Self {
                max_size: 100,
                max_wait: Duration::from_millis(10),
                max_bytes: 1024 * 1024, // 1MB
            }
        }
    }
    
    /// A batch of items with metadata
    #[derive(Debug)]
    pub struct Batch<T> {
        pub items: Vec<T>,
        pub total_bytes: usize,
        pub oldest_item_age: Duration,
    }
    
    /// Batch items efficiently
    pub fn create_batcher<T>(
        config: BatchConfig,
    ) -> (mpsc::UnboundedSender<T>, mpsc::Receiver<Batch<T>>)
    where
        T: Send + 'static,
    {
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<T>();
        let (output_tx, output_rx) = mpsc::channel::<Batch<T>>(16);
        
        tokio::spawn(async move {
            let mut current_batch = Vec::new();
            let mut current_bytes = 0;
            let mut batch_start = None;
            let mut flush_interval = interval(config.max_wait);
            
            loop {
                tokio::select! {
                    item = input_rx.recv() => {
                        match item {
                            Some(item) => {
                                if current_batch.is_empty() {
                                    batch_start = Some(Instant::now());
                                }
                                
                                // Rough estimate - assume 64 bytes per item
                                current_bytes += 64;
                                current_batch.push(item);
                                
                                // Flush if batch is full
                                if current_batch.len() >= config.max_size || current_bytes >= config.max_bytes {
                                    let age = batch_start.map(|start| start.elapsed()).unwrap_or_default();
                                    let batch = Batch {
                                        items: std::mem::take(&mut current_batch),
                                        total_bytes: current_bytes,
                                        oldest_item_age: age,
                                    };
                                    
                                    if output_tx.send(batch).await.is_err() {
                                        break;
                                    }
                                    
                                    current_bytes = 0;
                                    batch_start = None;
                                }
                            }
                            None => break,
                        }
                    }
                    _ = flush_interval.tick() => {
                        if !current_batch.is_empty() {
                            let age = batch_start.map(|start| start.elapsed()).unwrap_or_default();
                            let batch = Batch {
                                items: std::mem::take(&mut current_batch),
                                total_bytes: current_bytes,
                                oldest_item_age: age,
                            };
                            
                            if output_tx.send(batch).await.is_err() {
                                break;
                            }
                            
                            current_bytes = 0;
                            batch_start = None;
                        }
                    }
                }
            }
        });
        
        (input_tx, output_rx)
    }
}

/// WAN optimization features for cross-region communication
pub mod wan_optimization {
    use anyhow::Result;
    use bytes::Bytes;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;
    use quinn::{Endpoint, ClientConfig, Connection, VarInt};
    use std::collections::HashMap;

    /// WAN optimization configuration
    #[derive(Debug, Clone)]
    pub struct WanConfig {
        /// Enable QUIC protocol for unreliable networks
        pub enable_quic: bool,
        /// Congestion control algorithm
        pub congestion_control: CongestionControl,
        /// Compression level (0-9, 0 = disabled)
        pub compression_level: u8,
        /// Connection idle timeout
        pub idle_timeout: Duration,
        /// Maximum concurrent streams per connection
        pub max_concurrent_streams: u32,
        /// Enable adaptive compression
        pub adaptive_compression: bool,
        /// Regional latency hints (ms)
        pub regional_latencies: HashMap<String, u64>,
    }

    impl Default for WanConfig {
        fn default() -> Self {
            Self {
                enable_quic: true,
                congestion_control: CongestionControl::Bbr,
                compression_level: 6,
                idle_timeout: Duration::from_secs(60),
                max_concurrent_streams: 100,
                adaptive_compression: true,
                regional_latencies: HashMap::new(),
            }
        }
    }

    /// Congestion control algorithms optimized for WAN
    #[derive(Debug, Clone, Copy)]
    pub enum CongestionControl {
        /// BBR (Bottleneck Bandwidth and Round-trip propagation time)
        Bbr,
        /// CUBIC (default TCP algorithm)
        Cubic,
        /// Reno (conservative)
        Reno,
    }

    /// Advanced compression for WAN traffic
    #[derive(Debug, Clone, Copy)]
    pub enum WanCompression {
        None,
        Lz4,
        Zstd(i32), // compression level
        Adaptive,  // Choose based on content
    }

    /// WAN-optimized connection manager
    pub struct WanConnectionManager {
        config: WanConfig,
        quic_endpoint: Option<Endpoint>,
        connections: Arc<RwLock<HashMap<SocketAddr, Arc<Connection>>>>,
        stats: Arc<RwLock<WanStats>>,
    }

    /// WAN connection statistics
    #[derive(Debug, Default)]
    pub struct WanStats {
        pub total_connections: u64,
        pub active_connections: u64,
        pub bytes_sent: u64,
        pub bytes_received: u64,
        pub compression_ratio: f64,
        pub avg_latency_ms: f64,
        pub packet_loss_rate: f64,
    }

    impl WanConnectionManager {
        /// Create a new WAN connection manager
        pub fn new(config: WanConfig) -> Result<Self> {
            let mut manager = Self {
                config,
                quic_endpoint: None,
                connections: Arc::new(RwLock::new(HashMap::new())),
                stats: Arc::new(RwLock::new(WanStats::default())),
            };

            if manager.config.enable_quic {
                manager.init_quic_endpoint()?;
            }

            Ok(manager)
        }

        /// Initialize QUIC endpoint for WAN optimization
        fn init_quic_endpoint(&mut self) -> Result<()> {
            // Create client configuration
            let mut client_config = ClientConfig::with_platform_verifier();
            
            // Configure congestion control
            match self.config.congestion_control {
                CongestionControl::Bbr => {
                    // BBR configuration would go here
                    // Note: Quinn uses a different congestion control interface
                }
                CongestionControl::Cubic => {
                    // CUBIC is often the default
                }
                CongestionControl::Reno => {
                    // Conservative configuration
                }
            }

            // Configure transport parameters
            let mut transport_config = quinn::TransportConfig::default();
            transport_config.max_idle_timeout(Some(VarInt::from_u32(self.config.idle_timeout.as_secs() as u32).into()));
            transport_config.max_concurrent_uni_streams(VarInt::from_u32(self.config.max_concurrent_streams));
            transport_config.max_concurrent_bidi_streams(VarInt::from_u32(self.config.max_concurrent_streams));
            
            client_config.transport_config(Arc::new(transport_config));

            // Create endpoint
            let mut endpoint = Endpoint::client("[::]:0".parse()?)?;
            endpoint.set_default_client_config(client_config);

            self.quic_endpoint = Some(endpoint);
            Ok(())
        }

        /// Establish WAN-optimized connection
        pub async fn connect(&self, addr: SocketAddr, server_name: &str) -> Result<Arc<Connection>> {
            if let Some(endpoint) = &self.quic_endpoint {
                // Check if connection already exists
                {
                    let connections = self.connections.read().await;
                    if let Some(conn) = connections.get(&addr) {
                        if !conn.close_reason().is_some() {
                            return Ok(conn.clone());
                        }
                    }
                }

                // Create new QUIC connection
                let connection = endpoint.connect(addr, server_name)?
                    .await
                    .map_err(|e| anyhow::anyhow!("QUIC connection failed: {}", e))?;

                let connection = Arc::new(connection);

                // Store connection
                {
                    let mut connections = self.connections.write().await;
                    connections.insert(addr, connection.clone());
                }

                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.total_connections += 1;
                    stats.active_connections += 1;
                }

                Ok(connection)
            } else {
                Err(anyhow::anyhow!("QUIC not enabled"))
            }
        }

        /// Send data with WAN optimization
        pub async fn send_optimized(&self, connection: &Connection, data: &[u8]) -> Result<()> {
            // Apply compression if enabled
            let compressed_data = if self.config.compression_level > 0 {
                self.compress_data(data)?
            } else {
                Bytes::copy_from_slice(data)
            };

            // Open a unidirectional stream for sending
            let mut send_stream = connection.open_uni().await
                .map_err(|e| anyhow::anyhow!("Failed to open stream: {}", e))?;

            // Send compressed data
            send_stream.write_all(&compressed_data).await
                .map_err(|e| anyhow::anyhow!("Failed to send data: {}", e))?;

            send_stream.finish()
                .map_err(|e| anyhow::anyhow!("Failed to finish stream: {}", e))?;

            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.bytes_sent += data.len() as u64;
                if self.config.compression_level > 0 {
                    stats.compression_ratio = compressed_data.len() as f64 / data.len() as f64;
                }
            }

            Ok(())
        }

        /// Compress data using WAN-optimized algorithms
        pub fn compress_data(&self, data: &[u8]) -> Result<Bytes> {
            let compression = if self.config.adaptive_compression {
                self.choose_compression_algorithm(data)
            } else {
                WanCompression::Zstd(self.config.compression_level as i32)
            };

            match compression {
                WanCompression::None => Ok(Bytes::copy_from_slice(data)),
                WanCompression::Lz4 => {
                    let compressed = lz4_flex::compress_prepend_size(data);
                    Ok(Bytes::from(compressed))
                }
                WanCompression::Zstd(level) => {
                    let compressed = zstd::bulk::compress(data, level)
                        .map_err(|e| anyhow::anyhow!("ZSTD compression failed: {}", e))?;
                    Ok(Bytes::from(compressed))
                }
                WanCompression::Adaptive => {
                    // Use LZ4 for speed, ZSTD for better compression
                    if data.len() < 1024 {
                        let compressed = lz4_flex::compress_prepend_size(data);
                        Ok(Bytes::from(compressed))
                    } else {
                        let compressed = zstd::bulk::compress(data, 3)
                            .map_err(|e| anyhow::anyhow!("ZSTD compression failed: {}", e))?;
                        Ok(Bytes::from(compressed))
                    }
                }
            }
        }

        /// Choose optimal compression algorithm based on data characteristics
        fn choose_compression_algorithm(&self, data: &[u8]) -> WanCompression {
            // Simple heuristic: use LZ4 for small data, ZSTD for larger data
            if data.len() < 512 {
                WanCompression::Lz4
            } else {
                // Sample data to estimate compressibility
                let sample_size = std::cmp::min(data.len(), 1024);
                let sample = &data[..sample_size];
                
                // Count unique bytes as a simple entropy measure
                let mut byte_counts = [0u8; 256];
                for &byte in sample {
                    byte_counts[byte as usize] = byte_counts[byte as usize].saturating_add(1);
                }
                
                let unique_bytes = byte_counts.iter().filter(|&&count| count > 0).count();
                
                // High entropy data (many unique bytes) - use faster compression
                if unique_bytes > 200 {
                    WanCompression::Lz4
                } else {
                    // Low entropy data - use better compression
                    WanCompression::Zstd(6)
                }
            }
        }

        /// Get WAN connection statistics
        pub async fn get_stats(&self) -> WanStats {
            let stats = self.stats.read().await;
            WanStats {
                total_connections: stats.total_connections,
                active_connections: stats.active_connections,
                bytes_sent: stats.bytes_sent,
                bytes_received: stats.bytes_received,
                compression_ratio: stats.compression_ratio,
                avg_latency_ms: stats.avg_latency_ms,
                packet_loss_rate: stats.packet_loss_rate,
            }
        }

        /// Close connection and cleanup
        pub async fn close_connection(&self, addr: SocketAddr) -> Result<()> {
            let mut connections = self.connections.write().await;
            if let Some(connection) = connections.remove(&addr) {
                connection.close(VarInt::from_u32(0), b"Closing connection");
                
                // Update stats
                let mut stats = self.stats.write().await;
                stats.active_connections = stats.active_connections.saturating_sub(1);
            }
            Ok(())
        }
    }

    /// Regional latency optimizer
    pub struct RegionalLatencyOptimizer {
        region_latencies: HashMap<String, Duration>,
        connection_preferences: HashMap<String, Vec<String>>, // region -> preferred regions
    }

    impl RegionalLatencyOptimizer {
        pub fn new() -> Self {
            Self {
                region_latencies: HashMap::new(),
                connection_preferences: HashMap::new(),
            }
        }

        /// Update latency measurement for a region
        pub fn update_latency(&mut self, region: &str, latency: Duration) {
            self.region_latencies.insert(region.to_string(), latency);
            self.recompute_preferences();
        }

        /// Get preferred connection order for a source region
        pub fn get_preferred_regions(&self, source_region: &str) -> Vec<String> {
            self.connection_preferences
                .get(source_region)
                .cloned()
                .unwrap_or_default()
        }

        /// Recompute region preferences based on latency
        fn recompute_preferences(&mut self) {
            for (source_region, _) in &self.region_latencies.clone() {
                let mut regions: Vec<_> = self.region_latencies
                    .iter()
                    .filter(|(region, _)| *region != source_region)
                    .collect();

                // Sort by latency (ascending)
                regions.sort_by(|(_, latency_a), (_, latency_b)| latency_a.cmp(latency_b));

                let preferred: Vec<String> = regions.into_iter()
                    .map(|(region, _)| region.clone())
                    .collect();

                self.connection_preferences.insert(source_region.clone(), preferred);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_pool_creation() {
        let mut config = ConnectionPoolConfig::default();
        config.tls_config = TlsConfig::disabled(); // Disable TLS for testing
        let pool = ConnectionPool::new(config).expect("Failed to create connection pool");
        
        let stats = pool.get_stats().await;
        assert!(stats.is_empty());
    }
    
    #[test]
    fn test_compression() {
        use compression::*;
        
        let data = b"Hello, world! This is a test string for compression.";
        
        // Test gzip compression
        let compressed = compress(data, CompressionAlgorithm::Gzip).unwrap();
        assert!(compressed.len() < data.len());
        
        let decompressed = decompress(&compressed, CompressionAlgorithm::Gzip).unwrap();
        assert_eq!(decompressed.as_ref(), data);
    }
    
    #[tokio::test]
    async fn test_batching() {
        use batching::*;
        
        let config = BatchConfig {
            max_size: 3,
            max_wait: Duration::from_millis(50),
            max_bytes: 1000,
        };
        
        let (tx, mut rx) = create_batcher::<i32>(config);
        
        // Send items
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap(); // Should trigger batch
        
        let batch = rx.recv().await.unwrap();
        assert_eq!(batch.items, vec![1, 2, 3]);
        
        // Send more items and wait for timeout
        tx.send(4).unwrap();
        tx.send(5).unwrap();
        
        let batch = rx.recv().await.unwrap();
        assert_eq!(batch.items, vec![4, 5]);
    }

    #[tokio::test]
    async fn test_wan_optimization() {
        use wan_optimization::*;
        
        // Test WAN configuration
        let config = WanConfig {
            enable_quic: false, // Disable QUIC for testing without server
            compression_level: 6,
            adaptive_compression: true,
            ..Default::default()
        };
        
        // Create WAN connection manager
        let manager = WanConnectionManager::new(config).unwrap();
        
        // Test compression selection
        let small_data = vec![0u8; 100];
        let large_data = vec![1u8; 2048];
        
        // Should work without panicking
        let _ = manager.compress_data(&small_data);
        let _ = manager.compress_data(&large_data);
        
        // Test regional latency optimizer
        let mut optimizer = RegionalLatencyOptimizer::new();
        optimizer.update_latency("us-east-1", Duration::from_millis(20));
        optimizer.update_latency("us-west-2", Duration::from_millis(80));
        optimizer.update_latency("eu-west-1", Duration::from_millis(120));
        
        let preferences = optimizer.get_preferred_regions("us-east-1");
        // Should prefer closer regions first
        assert!(!preferences.is_empty());
    }
}