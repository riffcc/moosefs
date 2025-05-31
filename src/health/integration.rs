//! Health Monitoring Integration Module
//! 
//! This module integrates the MooseNG health monitoring system with
//! the traditional MooseFS components and provides unified health
//! check endpoints and monitoring capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn, error, instrument};

// Re-export from the endpoints module
use super::endpoints::{HealthResponse, HealthCheckDetail, HealthLevel, HealthAlert, AlertLevel};

/// Integration layer between MooseFS and MooseNG health systems
pub struct HealthIntegrationManager {
    /// MooseNG health monitor
    mooseng_monitor: Arc<MooseNGHealthMonitor>,
    /// MooseFS component health checkers
    moosefs_checkers: Arc<RwLock<HashMap<String, Arc<dyn MooseFSHealthChecker>>>>,
    /// Unified health status
    unified_status: Arc<RwLock<UnifiedHealthStatus>>,
    /// Alert broadcaster
    alert_tx: broadcast::Sender<UnifiedHealthAlert>,
    /// Configuration
    config: Arc<HealthIntegrationConfig>,
}

/// Configuration for health integration
#[derive(Debug, Clone)]
pub struct HealthIntegrationConfig {
    /// Health check interval for MooseFS components
    pub moosefs_check_interval: Duration,
    /// Timeout for health checks
    pub check_timeout: Duration,
    /// Enable automatic self-healing
    pub enable_self_healing: bool,
    /// Maximum alerts per hour per component
    pub max_alerts_per_hour: u32,
    /// Health check endpoints configuration
    pub endpoints_config: EndpointsConfig,
}

/// Endpoints configuration
#[derive(Debug, Clone)]
pub struct EndpointsConfig {
    /// Bind address for health endpoints
    pub bind_address: String,
    /// Enable Prometheus metrics
    pub enable_prometheus: bool,
    /// Enable detailed health checks
    pub enable_detailed_checks: bool,
    /// Custom health check paths
    pub custom_paths: HashMap<String, String>,
}

impl Default for HealthIntegrationConfig {
    fn default() -> Self {
        Self {
            moosefs_check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(5),
            enable_self_healing: true,
            max_alerts_per_hour: 10,
            endpoints_config: EndpointsConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                enable_prometheus: true,
                enable_detailed_checks: true,
                custom_paths: HashMap::new(),
            },
        }
    }
}

/// Unified health status combining MooseFS and MooseNG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedHealthStatus {
    /// Overall system health
    pub overall_status: HealthLevel,
    /// MooseFS component statuses
    pub moosefs_components: HashMap<String, ComponentHealth>,
    /// MooseNG component statuses
    pub mooseng_components: HashMap<String, ComponentHealth>,
    /// System-wide metrics
    pub system_metrics: SystemMetrics,
    /// Last update timestamp
    pub last_updated: SystemTime,
}

/// Individual component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Health status
    pub status: HealthLevel,
    /// Status message
    pub message: String,
    /// Performance metrics
    pub metrics: HashMap<String, f64>,
    /// Last check timestamp
    pub last_check: SystemTime,
    /// Component type (MooseFS or MooseNG)
    pub component_type: ComponentType,
}

/// Component type identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentType {
    MooseFS(MooseFSComponent),
    MooseNG(MooseNGComponent),
}

/// MooseFS component types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MooseFSComponent {
    Master,
    ChunkServer,
    MetaLogger,
    Client,
    CGIServer,
}

/// MooseNG component types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MooseNGComponent {
    Master,
    ChunkServer,
    Client,
    MetaLogger,
    TieredStorage,
    RaftConsensus,
    MultiRegion,
}

/// System-wide metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Total memory usage (percentage)
    pub memory_usage_percent: f64,
    /// Total CPU usage (percentage)
    pub cpu_usage_percent: f64,
    /// Total disk usage (percentage)
    pub disk_usage_percent: f64,
    /// Network I/O metrics
    pub network_io_mbps: f64,
    /// Active connections count
    pub active_connections: u64,
    /// Error rate (errors per second)
    pub error_rate: f64,
}

/// Unified health alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedHealthAlert {
    /// Alert ID
    pub id: String,
    /// Component name
    pub component: String,
    /// Alert level
    pub level: AlertLevel,
    /// Alert message
    pub message: String,
    /// Component type
    pub component_type: ComponentType,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Health checker trait for MooseFS components
#[async_trait::async_trait]
pub trait MooseFSHealthChecker: Send + Sync {
    /// Perform health check
    async fn check_health(&self) -> Result<ComponentHealth>;
    
    /// Get component name
    fn component_name(&self) -> &str;
    
    /// Get component type
    fn component_type(&self) -> MooseFSComponent;
    
    /// Perform self-healing if supported
    async fn perform_self_healing(&self, _issue: &str) -> Result<bool> {
        Ok(false) // Default: no self-healing
    }
}

/// Placeholder for MooseNG health monitor
/// In a real implementation, this would interface with the actual MooseNG health system
pub struct MooseNGHealthMonitor {
    // Placeholder fields
}

impl MooseNGHealthMonitor {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn get_component_health(&self) -> Result<HashMap<String, ComponentHealth>> {
        // Placeholder implementation
        // In reality, this would interface with mooseng-common::health::HealthMonitor
        Ok(HashMap::new())
    }
}

impl HealthIntegrationManager {
    /// Create a new health integration manager
    pub fn new(config: HealthIntegrationConfig) -> Self {
        let (alert_tx, _) = broadcast::channel(1000);
        
        Self {
            mooseng_monitor: Arc::new(MooseNGHealthMonitor::new()),
            moosefs_checkers: Arc::new(RwLock::new(HashMap::new())),
            unified_status: Arc::new(RwLock::new(UnifiedHealthStatus {
                overall_status: HealthLevel::Unknown,
                moosefs_components: HashMap::new(),
                mooseng_components: HashMap::new(),
                system_metrics: SystemMetrics {
                    memory_usage_percent: 0.0,
                    cpu_usage_percent: 0.0,
                    disk_usage_percent: 0.0,
                    network_io_mbps: 0.0,
                    active_connections: 0,
                    error_rate: 0.0,
                },
                last_updated: SystemTime::now(),
            })),
            alert_tx,
            config: Arc::new(config),
        }
    }
    
    /// Register a MooseFS health checker
    #[instrument(skip(self, checker))]
    pub async fn register_moosefs_checker(&self, checker: Arc<dyn MooseFSHealthChecker>) {
        let component_name = checker.component_name().to_string();
        let mut checkers = self.moosefs_checkers.write().await;
        checkers.insert(component_name.clone(), checker);
        info!("Registered MooseFS health checker for: {}", component_name);
    }
    
    /// Start the health integration system
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting health integration system");
        
        // Start periodic health checks
        self.start_health_monitoring().await?;
        
        // Start health endpoints server
        self.start_health_endpoints().await?;
        
        Ok(())
    }
    
    /// Start periodic health monitoring
    async fn start_health_monitoring(&self) -> Result<()> {
        let checkers = self.moosefs_checkers.clone();
        let mooseng_monitor = self.mooseng_monitor.clone();
        let unified_status = self.unified_status.clone();
        let alert_tx = self.alert_tx.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.moosefs_check_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::perform_unified_health_check(
                    &checkers,
                    &mooseng_monitor,
                    &unified_status,
                    &alert_tx,
                    &config,
                ).await {
                    error!("Unified health check failed: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Start health endpoints server
    async fn start_health_endpoints(&self) -> Result<()> {
        use super::endpoints::{start_health_server, HealthServerState};
        
        // Create server state from our integration manager
        let server_state = self.create_health_server_state().await;
        
        let bind_addr = self.config.endpoints_config.bind_address.clone();
        tokio::spawn(async move {
            if let Err(e) = start_health_server(&bind_addr, server_state).await {
                error!("Health endpoints server failed: {}", e);
            }
        });
        
        info!("Started health endpoints server on {}", self.config.endpoints_config.bind_address);
        Ok(())
    }
    
    /// Create health server state for the endpoints
    async fn create_health_server_state(&self) -> super::endpoints::HealthServerState {
        // This would create the proper state structure needed by the endpoints
        // For now, we'll create a placeholder
        super::endpoints::HealthServerState {
            // Fields would be populated based on actual endpoints implementation
        }
    }
    
    /// Perform unified health check across all systems
    #[instrument(skip(checkers, mooseng_monitor, unified_status, alert_tx, config))]
    async fn perform_unified_health_check(
        checkers: &Arc<RwLock<HashMap<String, Arc<dyn MooseFSHealthChecker>>>>,
        mooseng_monitor: &Arc<MooseNGHealthMonitor>,
        unified_status: &Arc<RwLock<UnifiedHealthStatus>>,
        alert_tx: &broadcast::Sender<UnifiedHealthAlert>,
        config: &HealthIntegrationConfig,
    ) -> Result<()> {
        let start_time = SystemTime::now();
        
        // Check MooseFS components
        let moosefs_health = Self::check_moosefs_components(checkers, config).await?;
        
        // Check MooseNG components
        let mooseng_health = mooseng_monitor.get_component_health().await?;
        
        // Collect system metrics
        let system_metrics = Self::collect_system_metrics().await?;
        
        // Determine overall health status
        let overall_status = Self::determine_overall_health(&moosefs_health, &mooseng_health);
        
        // Update unified status
        {
            let mut status = unified_status.write().await;
            status.moosefs_components = moosefs_health;
            status.mooseng_components = mooseng_health;
            status.system_metrics = system_metrics;
            status.overall_status = overall_status;
            status.last_updated = start_time;
        }
        
        // Send alerts for any unhealthy components
        Self::send_alerts_if_needed(unified_status, alert_tx).await?;
        
        debug!("Unified health check completed in {:?}", start_time.elapsed().unwrap_or_default());
        Ok(())
    }
    
    /// Check health of MooseFS components
    async fn check_moosefs_components(
        checkers: &Arc<RwLock<HashMap<String, Arc<dyn MooseFSHealthChecker>>>>,
        config: &HealthIntegrationConfig,
    ) -> Result<HashMap<String, ComponentHealth>> {
        let checkers_guard = checkers.read().await;
        let mut health_results = HashMap::new();
        
        // Use futures to check all components concurrently
        let mut tasks = Vec::new();
        for (name, checker) in checkers_guard.iter() {
            let checker = checker.clone();
            let timeout = config.check_timeout;
            let name = name.clone();
            
            let task = tokio::spawn(async move {
                let result = tokio::time::timeout(timeout, checker.check_health()).await;
                match result {
                    Ok(Ok(health)) => (name, Ok(health)),
                    Ok(Err(e)) => (name, Err(e)),
                    Err(_) => (name, Err(anyhow::anyhow!("Health check timeout"))),
                }
            });
            tasks.push(task);
        }
        
        // Collect results
        for task in tasks {
            if let Ok((name, result)) = task.await {
                match result {
                    Ok(health) => {
                        health_results.insert(name.clone(), health);
                    }
                    Err(e) => {
                        warn!("Health check failed for {}: {}", name, e);
                        health_results.insert(name.clone(), ComponentHealth {
                            name: name.clone(),
                            status: HealthLevel::Critical,
                            message: format!("Health check failed: {}", e),
                            metrics: HashMap::new(),
                            last_check: SystemTime::now(),
                            component_type: ComponentType::MooseFS(MooseFSComponent::Master), // Default
                        });
                    }
                }
            }
        }
        
        Ok(health_results)
    }
    
    /// Collect system-wide metrics
    async fn collect_system_metrics() -> Result<SystemMetrics> {
        // In a real implementation, this would collect actual system metrics
        // using system monitoring libraries or external tools
        Ok(SystemMetrics {
            memory_usage_percent: 45.0,  // Placeholder
            cpu_usage_percent: 25.0,     // Placeholder
            disk_usage_percent: 70.0,    // Placeholder
            network_io_mbps: 150.0,      // Placeholder
            active_connections: 1500,    // Placeholder
            error_rate: 0.01,            // Placeholder
        })
    }
    
    /// Determine overall health status
    fn determine_overall_health(
        moosefs_health: &HashMap<String, ComponentHealth>,
        mooseng_health: &HashMap<String, ComponentHealth>,
    ) -> HealthLevel {
        let all_components: Vec<&ComponentHealth> = moosefs_health
            .values()
            .chain(mooseng_health.values())
            .collect();
        
        if all_components.is_empty() {
            return HealthLevel::Unknown;
        }
        
        // Determine overall status based on component statuses
        if all_components.iter().any(|c| c.status == HealthLevel::Critical) {
            HealthLevel::Critical
        } else if all_components.iter().any(|c| c.status == HealthLevel::Warning) {
            HealthLevel::Warning
        } else if all_components.iter().all(|c| c.status == HealthLevel::Healthy) {
            HealthLevel::Healthy
        } else {
            HealthLevel::Unknown
        }
    }
    
    /// Send alerts for unhealthy components
    async fn send_alerts_if_needed(
        unified_status: &Arc<RwLock<UnifiedHealthStatus>>,
        alert_tx: &broadcast::Sender<UnifiedHealthAlert>,
    ) -> Result<()> {
        let status = unified_status.read().await;
        
        // Check all components and send alerts for unhealthy ones
        for (name, component) in status.moosefs_components.iter()
            .chain(status.mooseng_components.iter()) {
            
            if matches!(component.status, HealthLevel::Warning | HealthLevel::Critical) {
                let alert = UnifiedHealthAlert {
                    id: format!("{}-{}", name, SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()),
                    component: name.clone(),
                    level: match component.status {
                        HealthLevel::Warning => AlertLevel::Warning,
                        HealthLevel::Critical => AlertLevel::Critical,
                        _ => AlertLevel::Info,
                    },
                    message: component.message.clone(),
                    component_type: component.component_type.clone(),
                    timestamp: component.last_check,
                    context: HashMap::new(),
                };
                
                if let Err(e) = alert_tx.send(alert) {
                    warn!("Failed to send alert for {}: {}", name, e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get current unified health status
    pub async fn get_health_status(&self) -> UnifiedHealthStatus {
        self.unified_status.read().await.clone()
    }
    
    /// Subscribe to health alerts
    pub fn subscribe_to_alerts(&self) -> broadcast::Receiver<UnifiedHealthAlert> {
        self.alert_tx.subscribe()
    }
    
    /// Trigger manual health check for a specific component
    #[instrument(skip(self))]
    pub async fn trigger_component_check(&self, component: &str) -> Result<ComponentHealth> {
        let checkers = self.moosefs_checkers.read().await;
        if let Some(checker) = checkers.get(component) {
            checker.check_health().await
        } else {
            // Try MooseNG components
            if let Ok(mooseng_health) = self.mooseng_monitor.get_component_health().await {
                if let Some(health) = mooseng_health.get(component) {
                    Ok(health.clone())
                } else {
                    Err(anyhow::anyhow!("Component not found: {}", component))
                }
            } else {
                Err(anyhow::anyhow!("Component not found: {}", component))
            }
        }
    }
}

/// Example MooseFS Master health checker implementation
pub struct MooseFSMasterHealthChecker {
    master_host: String,
    master_port: u16,
}

impl MooseFSMasterHealthChecker {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            master_host: host,
            master_port: port,
        }
    }
}

#[async_trait::async_trait]
impl MooseFSHealthChecker for MooseFSMasterHealthChecker {
    async fn check_health(&self) -> Result<ComponentHealth> {
        // In a real implementation, this would:
        // 1. Connect to MooseFS master server
        // 2. Check various health metrics
        // 3. Return detailed health status
        
        // Placeholder implementation
        Ok(ComponentHealth {
            name: "moosefs-master".to_string(),
            status: HealthLevel::Healthy,
            message: "Master server is running".to_string(),
            metrics: {
                let mut metrics = HashMap::new();
                metrics.insert("chunks_count".to_string(), 1000.0);
                metrics.insert("free_space_gb".to_string(), 500.0);
                metrics.insert("connected_chunkservers".to_string(), 5.0);
                metrics
            },
            last_check: SystemTime::now(),
            component_type: ComponentType::MooseFS(MooseFSComponent::Master),
        })
    }
    
    fn component_name(&self) -> &str {
        "moosefs-master"
    }
    
    fn component_type(&self) -> MooseFSComponent {
        MooseFSComponent::Master
    }
    
    async fn perform_self_healing(&self, issue: &str) -> Result<bool> {
        info!("Attempting self-healing for MooseFS master: {}", issue);
        // In a real implementation, this might:
        // 1. Restart master service
        // 2. Clear temporary files
        // 3. Reconnect to chunk servers
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    
    #[tokio::test]
    async fn test_health_integration_creation() {
        let config = HealthIntegrationConfig::default();
        let manager = HealthIntegrationManager::new(config);
        
        let status = manager.get_health_status().await;
        assert_eq!(status.overall_status, HealthLevel::Unknown);
    }
    
    #[tokio::test]
    async fn test_moosefs_checker_registration() {
        let config = HealthIntegrationConfig::default();
        let manager = HealthIntegrationManager::new(config);
        
        let checker = Arc::new(MooseFSMasterHealthChecker::new(
            "localhost".to_string(),
            9421,
        ));
        
        manager.register_moosefs_checker(checker).await;
        
        // Verify checker was registered
        let health = manager.trigger_component_check("moosefs-master").await.unwrap();
        assert_eq!(health.name, "moosefs-master");
    }
    
    #[tokio::test]
    async fn test_unified_health_status() {
        let config = HealthIntegrationConfig::default();
        let manager = HealthIntegrationManager::new(config);
        
        let checker = Arc::new(MooseFSMasterHealthChecker::new(
            "localhost".to_string(),
            9421,
        ));
        
        manager.register_moosefs_checker(checker).await;
        
        let status = manager.get_health_status().await;
        assert!(status.last_updated <= SystemTime::now());
    }
}