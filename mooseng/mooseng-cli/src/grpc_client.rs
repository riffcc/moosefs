use anyhow::{Context, Result};
use mooseng_protocol::{
    master_service_client::MasterServiceClient,
    GetClusterStatusRequest, GetClusterStatusResponse,
    GetRegionInfoRequest, GetRegionInfoResponse,
    RegisterServerRequest, RegisterServerResponse,
    HeartbeatRequest, HeartbeatResponse,
    ListStorageClassesRequest, ListStorageClassesResponse,
    CreateStorageClassRequest, CreateStorageClassResponse,
    ServerInfo, ServerType, ServerStatus, ServerCapabilities, StorageClass,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, error, info, warn};

/// Configuration for gRPC client connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcClientConfig {
    /// Master server addresses
    pub master_addresses: Vec<String>,
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
    /// Enable TLS
    pub enable_tls: bool,
    /// TLS certificate path (if using custom certs)
    pub tls_cert_path: Option<String>,
    /// Retry attempts for failed requests
    pub retry_attempts: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for GrpcClientConfig {
    fn default() -> Self {
        Self {
            master_addresses: vec!["http://localhost:9421".to_string()],
            connect_timeout_secs: 10,
            request_timeout_secs: 30,
            enable_tls: false,
            tls_cert_path: None,
            retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// MooseNG CLI gRPC client
pub struct MooseNGClient {
    config: GrpcClientConfig,
    master_clients: Vec<MasterServiceClient<Channel>>,
    current_master_index: usize,
}

impl MooseNGClient {
    /// Create a new MooseNG client
    pub async fn new(config: GrpcClientConfig) -> Result<Self> {
        let mut master_clients = Vec::new();
        
        for address in &config.master_addresses {
            match Self::create_master_client(address, &config).await {
                Ok(client) => {
                    master_clients.push(client);
                    info!("Connected to master at {}", address);
                }
                Err(e) => {
                    warn!("Failed to connect to master at {}: {}", address, e);
                }
            }
        }
        
        if master_clients.is_empty() {
            return Err(anyhow::anyhow!("No master servers available"));
        }
        
        Ok(Self {
            config,
            master_clients,
            current_master_index: 0,
        })
    }
    
    /// Create a master service client for a given address
    async fn create_master_client(
        address: &str,
        config: &GrpcClientConfig,
    ) -> Result<MasterServiceClient<Channel>> {
        let endpoint = Endpoint::from_shared(address.to_string())?
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .timeout(Duration::from_secs(config.request_timeout_secs));
        
        let channel = endpoint.connect().await
            .context("Failed to connect to master server")?;
        
        Ok(MasterServiceClient::new(channel))
    }
    
    /// Get the current master client with failover support
    fn get_master_client(&mut self) -> Result<&mut MasterServiceClient<Channel>> {
        if self.master_clients.is_empty() {
            return Err(anyhow::anyhow!("No master clients available"));
        }
        
        Ok(&mut self.master_clients[self.current_master_index])
    }
    
    /// Execute a request with automatic failover and retry
    async fn execute_with_failover<T, F, Fut>(&mut self, mut operation: F) -> Result<T>
    where
        F: FnMut(&mut MasterServiceClient<Channel>) -> Fut,
        Fut: std::future::Future<Output = Result<T, tonic::Status>>,
    {
        let mut last_error = None;
        let mut retry_count = 0;
        let max_retries = self.config.retry_attempts as usize;
        
        // Try each master server with retries
        for attempt in 0..self.master_clients.len() {
            let client = &mut self.master_clients[self.current_master_index];
            
            // Retry logic for current master
            for retry in 0..=max_retries {
                match operation(client).await {
                    Ok(response) => {
                        if retry > 0 {
                            info!("Request succeeded after {} retries on master {}", 
                                  retry, self.config.master_addresses[self.current_master_index]);
                        }
                        return Ok(response);
                    }
                    Err(e) => {
                        let is_retryable = match e.code() {
                            tonic::Code::Unavailable | tonic::Code::DeadlineExceeded | 
                            tonic::Code::ResourceExhausted | tonic::Code::Aborted => true,
                            tonic::Code::NotFound | tonic::Code::InvalidArgument | 
                            tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => false,
                            _ => true, // Default to retryable for unknown errors
                        };
                        
                        if retry < max_retries && is_retryable {
                            warn!("Request failed on master {} (attempt {}/{}): {} - retrying in {}ms", 
                                  self.config.master_addresses[self.current_master_index], 
                                  retry + 1, max_retries + 1, e, self.config.retry_delay_ms);
                            tokio::time::sleep(std::time::Duration::from_millis(self.config.retry_delay_ms)).await;
                            retry_count += 1;
                            continue;
                        } else {
                            warn!("Request failed on master {} (final attempt): {}", 
                                  self.config.master_addresses[self.current_master_index], e);
                            last_error = Some(e);
                            break;
                        }
                    }
                }
            }
            
            // Move to next master if current one failed
            self.current_master_index = (self.current_master_index + 1) % self.master_clients.len();
        }
        
        Err(anyhow::anyhow!(
            "All {} master servers failed after {} total retries. Last error: {:?}", 
            self.master_clients.len(), retry_count, last_error
        ))
    }
    
    /// Get cluster status
    pub async fn get_cluster_status(&mut self) -> Result<GetClusterStatusResponse> {
        debug!("Getting cluster status");
        
        self.execute_with_failover(|client| async move {
            let mut request = tonic::Request::new(GetClusterStatusRequest {
                include_chunk_distribution: true,
                include_metrics: true,
            });
            request.set_timeout(Duration::from_secs(30)); // Longer timeout for status requests
            client.get_cluster_status(request).await.map(|r| r.into_inner())
        }).await
    }
    
    /// Get region information
    pub async fn get_region_info(&mut self, region_id: Option<u32>) -> Result<GetRegionInfoResponse> {
        debug!("Getting region info for region: {:?}", region_id);
        
        self.execute_with_failover(|client| async move {
            let request = tonic::Request::new(GetRegionInfoRequest {
                region_id: region_id.unwrap_or(0),
            });
            client.get_region_info(request).await.map(|r| r.into_inner())
        }).await
    }
    
    /// Register a new server with the cluster
    pub async fn register_server(
        &mut self,
        server_id: String,
        server_type: String,
        address: String,
        port: u32,
    ) -> Result<RegisterServerResponse> {
        info!("Registering {} server {} at {}:{}", server_type, server_id, address, port);
        
        self.execute_with_failover(|client| async move {
            let server_info = ServerInfo {
                server_id: server_id.parse().unwrap_or(0),
                r#type: ServerType::ServerTypeChunk as i32,
                hostname: address.clone(),
                ip_address: address.clone(),
                port,
                region: "default".to_string(),
                rack_id: 0,
                status: ServerStatus::Online as i32,
                capabilities: Some(ServerCapabilities::default()),
                last_heartbeat: None,
            };
            
            let request = tonic::Request::new(RegisterServerRequest {
                info: Some(server_info),
                auth_token: "".to_string(),
            });
            client.register_server(request).await.map(|r| r.into_inner())
        }).await
    }
    
    /// Send heartbeat to maintain server registration
    pub async fn send_heartbeat(
        &mut self,
        server_id: u64,
        capabilities: ServerCapabilities,
    ) -> Result<HeartbeatResponse> {
        debug!("Sending heartbeat for server {}", server_id);
        
        self.execute_with_failover(|client| async move {
            let request = tonic::Request::new(HeartbeatRequest {
                server_id,
                capabilities: Some(capabilities),
                chunk_reports: vec![],
                metrics: std::collections::HashMap::new(),
            });
            client.heartbeat(request).await.map(|r| r.into_inner())
        }).await
    }
    
    /// List storage classes
    pub async fn list_storage_classes(&mut self) -> Result<ListStorageClassesResponse> {
        debug!("Listing storage classes");
        
        self.execute_with_failover(|client| async move {
            let request = tonic::Request::new(ListStorageClassesRequest {});
            client.list_storage_classes(request).await.map(|r| r.into_inner())
        }).await
    }
    
    /// Create a new storage class
    pub async fn create_storage_class(
        &mut self,
        name: String,
        replication_factor: u32,
        erasure_coding: Option<(u32, u32)>, // (data_shards, parity_shards)
    ) -> Result<CreateStorageClassResponse> {
        info!("Creating storage class: {}", name);
        
        self.execute_with_failover(|client| async move {
            let storage_class = StorageClass {
                id: 0, // Will be assigned by server
                name: name.clone(),
                replication_factor,
                ec_scheme: if erasure_coding.is_some() { 1 } else { 0 }, // TODO: map proper scheme
                policy: None,
                allowed_regions: vec![],
                min_regions: 1,
            };
            
            let request = tonic::Request::new(CreateStorageClassRequest {
                storage_class: Some(storage_class),
            });
            client.create_storage_class(request).await.map(|r| r.into_inner())
        }).await
    }
    
    /// Test connectivity to all configured masters
    pub async fn test_connectivity(&mut self) -> Vec<(String, Result<Duration>)> {
        let mut results = Vec::new();
        
        for (i, address) in self.config.master_addresses.iter().enumerate() {
            let start = std::time::Instant::now();
            
            match self.master_clients.get_mut(i) {
                Some(client) => {
                    let mut request = tonic::Request::new(GetClusterStatusRequest {
                        include_chunk_distribution: false,
                        include_metrics: false,
                    });
                    
                    // Set timeout for connectivity test
                    request.set_timeout(Duration::from_secs(self.config.connect_timeout_secs));
                    
                    match client.get_cluster_status(request).await {
                        Ok(_) => {
                            let duration = start.elapsed();
                            info!("Successfully connected to master {} in {:?}", address, duration);
                            results.push((address.clone(), Ok(duration)));
                        }
                        Err(e) => {
                            let error_msg = format!("gRPC error ({}): {}", e.code(), e.message());
                            warn!("Failed to connect to master {}: {}", address, error_msg);
                            results.push((address.clone(), Err(anyhow::anyhow!(error_msg))));
                        }
                    }
                }
                None => {
                    let error_msg = "No client available";
                    warn!("{} for master {}", error_msg, address);
                    results.push((address.clone(), Err(anyhow::anyhow!(error_msg))));
                }
            }
        }
        
        results
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> &GrpcClientConfig {
        &self.config
    }
    
    /// Update client configuration and reconnect if needed
    pub async fn update_config(&mut self, new_config: GrpcClientConfig) -> Result<()> {
        // Check if master addresses changed
        let addresses_changed = self.config.master_addresses != new_config.master_addresses;
        
        self.config = new_config;
        
        if addresses_changed {
            // Reconnect to new master addresses
            self.master_clients.clear();
            
            for address in &self.config.master_addresses {
                match Self::create_master_client(address, &self.config).await {
                    Ok(client) => {
                        self.master_clients.push(client);
                        info!("Reconnected to master at {}", address);
                    }
                    Err(e) => {
                        warn!("Failed to reconnect to master at {}: {}", address, e);
                    }
                }
            }
            
            if self.master_clients.is_empty() {
                return Err(anyhow::anyhow!("No master servers available after config update"));
            }
            
            self.current_master_index = 0;
        }
        
        Ok(())
    }
}

/// Load gRPC client configuration from file or environment
pub fn load_client_config() -> Result<GrpcClientConfig> {
    // Try to load from config file first
    if let Ok(config_path) = std::env::var("MOOSENG_CLI_CONFIG") {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            return toml::from_str(&content)
                .context("Failed to parse config file");
        }
    }
    
    // Try standard locations
    let config_paths = [
        "~/.mooseng/cli.toml",
        "/etc/mooseng/cli.toml",
        "./mooseng-cli.toml",
    ];
    
    for path in &config_paths {
        let expanded_path = shellexpand::tilde(path);
        if let Ok(content) = std::fs::read_to_string(expanded_path.as_ref()) {
            return toml::from_str(&content)
                .context("Failed to parse config file");
        }
    }
    
    // Use environment variables if available
    let mut config = GrpcClientConfig::default();
    
    if let Ok(masters) = std::env::var("MOOSENG_MASTERS") {
        config.master_addresses = masters.split(',').map(|s| s.trim().to_string()).collect();
    }
    
    if let Ok(timeout) = std::env::var("MOOSENG_CONNECT_TIMEOUT") {
        if let Ok(timeout_secs) = timeout.parse() {
            config.connect_timeout_secs = timeout_secs;
        }
    }
    
    if let Ok(enable_tls) = std::env::var("MOOSENG_ENABLE_TLS") {
        config.enable_tls = enable_tls.to_lowercase() == "true";
    }
    
    Ok(config)
}

/// Save gRPC client configuration to file
pub fn save_client_config(config: &GrpcClientConfig) -> Result<()> {
    let config_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?
        .join(".mooseng");
    
    std::fs::create_dir_all(&config_dir)
        .context("Failed to create config directory")?;
    
    let config_path = config_dir.join("cli.toml");
    let content = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;
    
    std::fs::write(&config_path, content)
        .context("Failed to write config file")?;
    
    info!("Saved CLI configuration to {}", config_path.display());
    Ok(())
}