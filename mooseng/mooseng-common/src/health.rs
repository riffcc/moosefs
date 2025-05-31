//! Comprehensive health check and self-healing system for MooseNG
//! 
//! This module provides:
//! - Health check interfaces and implementations
//! - Self-healing mechanisms and recovery strategies
//! - Monitoring and alerting capabilities
//! - Failure detection and automatic remediation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, warn};

use crate::error::MooseNGError;

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub component: String,
    pub status: HealthStatus,
    pub message: String,
    pub timestamp: SystemTime,
    pub metrics: HashMap<String, f64>,
    pub recommendations: Vec<String>,
}

/// Health status levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Degraded,
    Unknown,
}

/// Self-healing action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelfHealingAction {
    RestartComponent { component: String },
    RebalanceData { from: String, to: String },
    ScaleUp { component: String, instances: u32 },
    ScaleDown { component: String, instances: u32 },
    ClearCache { component: String },
    RecoverData { chunk_id: String, source: String },
    NetworkReconnect { endpoint: String },
    CustomAction { name: String, params: HashMap<String, String> },
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub interval: Duration,
    pub timeout: Duration,
    pub failure_threshold: u32,
    pub recovery_threshold: u32,
    pub enable_self_healing: bool,
    pub max_healing_actions_per_hour: u32,
    pub component_checks: HashMap<String, ComponentHealthConfig>,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            failure_threshold: 3,
            recovery_threshold: 2,
            enable_self_healing: true,
            max_healing_actions_per_hour: 10,
            component_checks: HashMap::new(),
        }
    }
}

/// Component-specific health check configuration
#[derive(Debug, Clone)]
pub struct ComponentHealthConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub timeout: Duration,
    pub custom_checks: Vec<String>,
    pub healing_actions: Vec<SelfHealingAction>,
    pub thresholds: HashMap<String, f64>,
}

impl Default for ComponentHealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            custom_checks: Vec::new(),
            healing_actions: Vec::new(),
            thresholds: HashMap::new(),
        }
    }
}

/// Health check trait that components must implement
#[async_trait::async_trait]
pub trait HealthChecker: Send + Sync {
    async fn check_health(&self) -> Result<HealthCheckResult>;
    fn component_name(&self) -> &str;
    async fn perform_self_healing(&self, action: &SelfHealingAction) -> Result<bool>;
}

/// Health monitor manages health checks and self-healing for all components
pub struct HealthMonitor {
    config: Arc<HealthCheckConfig>,
    checkers: Arc<RwLock<HashMap<String, Arc<dyn HealthChecker>>>>,
    health_history: Arc<RwLock<HashMap<String, Vec<HealthCheckResult>>>>,
    healing_history: Arc<RwLock<Vec<HealingAttempt>>>,
    alert_tx: broadcast::Sender<HealthAlert>,
    shutdown_rx: broadcast::Receiver<()>,
}

/// Health alert for monitoring systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub component: String,
    pub status: HealthStatus,
    pub message: String,
    pub timestamp: SystemTime,
    pub alert_id: String,
    pub severity: AlertSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Record of self-healing attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingAttempt {
    pub component: String,
    pub action: SelfHealingAction,
    pub timestamp: SystemTime,
    pub success: bool,
    pub message: String,
    pub duration: Duration,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(config: HealthCheckConfig) -> Self {
        let (alert_tx, _) = broadcast::channel(1000);
        let (_, shutdown_rx) = broadcast::channel(1);
        
        Self {
            config: Arc::new(config),
            checkers: Arc::new(RwLock::new(HashMap::new())),
            health_history: Arc::new(RwLock::new(HashMap::new())),
            healing_history: Arc::new(RwLock::new(Vec::new())),
            alert_tx,
            shutdown_rx,
        }
    }
    
    /// Register a health checker for a component
    pub async fn register_checker(&self, checker: Arc<dyn HealthChecker>) {
        let component_name = checker.component_name().to_string();
        let mut checkers = self.checkers.write().await;
        checkers.insert(component_name.clone(), checker);
        
        info!("Registered health checker for component: {}", component_name);
    }
    
    /// Start the health monitoring system
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting health monitoring system");
        
        let checkers = self.checkers.clone();
        let config = self.config.clone();
        let health_history = self.health_history.clone();
        let healing_history = self.healing_history.clone();
        let alert_tx = self.alert_tx.clone();
        let mut shutdown_rx = self.shutdown_rx.resubscribe();
        
        tokio::spawn(async move {
            let mut health_interval = interval(config.interval);
            let mut cleanup_interval = interval(Duration::from_secs(3600)); // Cleanup every hour
            
            loop {
                tokio::select! {
                    _ = health_interval.tick() => {
                        Self::perform_health_checks(
                            &checkers,
                            &config,
                            &health_history,
                            &healing_history,
                            &alert_tx,
                        ).await;
                    }
                    _ = cleanup_interval.tick() => {
                        Self::cleanup_old_records(&health_history, &healing_history).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Health monitor shutting down");
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Perform health checks for all registered components
    async fn perform_health_checks(
        checkers: &Arc<RwLock<HashMap<String, Arc<dyn HealthChecker>>>>,
        config: &HealthCheckConfig,
        health_history: &Arc<RwLock<HashMap<String, Vec<HealthCheckResult>>>>,
        healing_history: &Arc<RwLock<Vec<HealingAttempt>>>,
        alert_tx: &broadcast::Sender<HealthAlert>,
    ) {
        let checkers_guard = checkers.read().await;
        
        for (component_name, checker) in checkers_guard.iter() {
            let checker = checker.clone();
            let config = config.clone();
            let health_history = health_history.clone();
            let healing_history = healing_history.clone();
            let alert_tx = alert_tx.clone();
            let component_name = component_name.clone();
            
            tokio::spawn(async move {
                Self::check_component_health(
                    &component_name,
                    checker,
                    &config,
                    &health_history,
                    &healing_history,
                    &alert_tx,
                ).await;
            });
        }
    }
    
    /// Check health for a specific component
    async fn check_component_health(
        component_name: &str,
        checker: Arc<dyn HealthChecker>,
        config: &HealthCheckConfig,
        health_history: &Arc<RwLock<HashMap<String, Vec<HealthCheckResult>>>>,
        healing_history: &Arc<RwLock<Vec<HealingAttempt>>>,
        alert_tx: &broadcast::Sender<HealthAlert>,
    ) {
        let start_time = SystemTime::now();
        
        // Perform health check with timeout
        let health_result = match timeout(config.timeout, checker.check_health()).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                warn!("Health check failed for {}: {}", component_name, e);
                HealthCheckResult {
                    component: component_name.to_string(),
                    status: HealthStatus::Critical,
                    message: format!("Health check error: {}", e),
                    timestamp: start_time,
                    metrics: HashMap::new(),
                    recommendations: vec!["Investigate component failure".to_string()],
                }
            }
            Err(_) => {
                warn!("Health check timed out for {}", component_name);
                HealthCheckResult {
                    component: component_name.to_string(),
                    status: HealthStatus::Critical,
                    message: "Health check timeout".to_string(),
                    timestamp: start_time,
                    metrics: HashMap::new(),
                    recommendations: vec!["Check component responsiveness".to_string()],
                }
            }
        };
        
        debug!("Health check for {} completed in {:?}: {:?}", 
               component_name, start_time.elapsed().unwrap_or_default(), health_result.status);
        
        // Store health result
        {
            let mut history = health_history.write().await;
            let component_history = history.entry(component_name.to_string()).or_insert_with(Vec::new);
            component_history.push(health_result.clone());
            
            // Keep only recent history (last 24 hours)
            let cutoff = SystemTime::now() - Duration::from_secs(86400);
            component_history.retain(|result| result.timestamp > cutoff);
        }
        
        // Check if self-healing is needed
        if config.enable_self_healing && Self::should_trigger_healing(&health_result, health_history, component_name).await {
            Self::internal_trigger_self_healing(
                component_name,
                checker,
                &health_result,
                config,
                healing_history,
                alert_tx,
            ).await;
        }
        
        // Send alert if status is concerning
        if matches!(health_result.status, HealthStatus::Warning | HealthStatus::Critical | HealthStatus::Degraded) {
            let alert = HealthAlert {
                component: component_name.to_string(),
                status: health_result.status.clone(),
                message: health_result.message.clone(),
                timestamp: health_result.timestamp,
                alert_id: format!("{}-{}", component_name, 
                    health_result.timestamp.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()),
                severity: match health_result.status {
                    HealthStatus::Warning => AlertSeverity::Medium,
                    HealthStatus::Degraded => AlertSeverity::High,
                    HealthStatus::Critical => AlertSeverity::Critical,
                    _ => AlertSeverity::Low,
                },
            };
            
            if alert_tx.send(alert).is_err() {
                warn!("Failed to send health alert for {}", component_name);
            }
        }
    }
    
    /// Check if self-healing should be triggered
    async fn should_trigger_healing(
        current_result: &HealthCheckResult,
        health_history: &Arc<RwLock<HashMap<String, Vec<HealthCheckResult>>>>,
        component_name: &str,
    ) -> bool {
        if !matches!(current_result.status, HealthStatus::Critical | HealthStatus::Degraded) {
            return false;
        }
        
        let history = health_history.read().await;
        if let Some(component_history) = history.get(component_name) {
            // Check if we've had consecutive failures
            let recent_failures = component_history
                .iter()
                .rev()
                .take(3)
                .filter(|result| matches!(result.status, HealthStatus::Critical | HealthStatus::Degraded))
                .count();
            
            recent_failures >= 2
        } else {
            false
        }
    }
    
    /// Internal trigger self-healing for a component
    async fn internal_trigger_self_healing(
        component_name: &str,
        checker: Arc<dyn HealthChecker>,
        health_result: &HealthCheckResult,
        config: &HealthCheckConfig,
        healing_history: &Arc<RwLock<Vec<HealingAttempt>>>,
        alert_tx: &broadcast::Sender<HealthAlert>,
    ) {
        // Check rate limiting
        {
            let history = healing_history.read().await;
            let recent_actions = history
                .iter()
                .filter(|attempt| attempt.component == component_name)
                .filter(|attempt| attempt.timestamp.elapsed().unwrap_or_default() < Duration::from_secs(3600))
                .count();
                
            if recent_actions >= config.max_healing_actions_per_hour as usize {
                warn!("Rate limit exceeded for self-healing on {}", component_name);
                return;
            }
        }
        
        // Determine appropriate healing action
        let healing_action = Self::choose_healing_action(health_result);
        
        info!("Triggering self-healing for {}: {:?}", component_name, healing_action);
        
        let start_time = SystemTime::now();
        let success = match checker.perform_self_healing(&healing_action).await {
            Ok(success) => {
                if success {
                    info!("Self-healing successful for {}: {:?}", component_name, healing_action);
                } else {
                    warn!("Self-healing failed for {}: {:?}", component_name, healing_action);
                }
                success
            }
            Err(e) => {
                error!("Self-healing error for {}: {}", component_name, e);
                false
            }
        };
        
        let attempt = HealingAttempt {
            component: component_name.to_string(),
            action: healing_action,
            timestamp: start_time,
            success,
            message: if success { "Healing successful".to_string() } else { "Healing failed".to_string() },
            duration: start_time.elapsed().unwrap_or_default(),
        };
        
        // Record healing attempt
        {
            let mut history = healing_history.write().await;
            history.push(attempt);
            
            // Keep only recent history
            let cutoff = SystemTime::now() - Duration::from_secs(86400 * 7); // 7 days
            history.retain(|attempt| attempt.timestamp > cutoff);
        }
        
        // Send healing alert
        let alert = HealthAlert {
            component: component_name.to_string(),
            status: if success { HealthStatus::Warning } else { HealthStatus::Critical },
            message: format!("Self-healing attempt: {}", if success { "successful" } else { "failed" }),
            timestamp: start_time,
            alert_id: format!("{}-healing-{}", component_name, start_time.elapsed().unwrap_or_default().as_millis()),
            severity: if success { AlertSeverity::Medium } else { AlertSeverity::High },
        };
        
        if alert_tx.send(alert).is_err() {
            warn!("Failed to send healing alert for {}", component_name);
        }
    }
    
    /// Choose appropriate healing action based on health result
    fn choose_healing_action(health_result: &HealthCheckResult) -> SelfHealingAction {
        // Simple heuristic - in production this would be more sophisticated
        if health_result.message.contains("timeout") || health_result.message.contains("connection") {
            SelfHealingAction::NetworkReconnect {
                endpoint: health_result.component.clone(),
            }
        } else if health_result.message.contains("memory") || health_result.message.contains("cache") {
            SelfHealingAction::ClearCache {
                component: health_result.component.clone(),
            }
        } else if health_result.message.contains("disk") || health_result.message.contains("storage") {
            SelfHealingAction::RebalanceData {
                from: health_result.component.clone(),
                to: "available_node".to_string(),
            }
        } else {
            SelfHealingAction::RestartComponent {
                component: health_result.component.clone(),
            }
        }
    }
    
    /// Clean up old health and healing records
    async fn cleanup_old_records(
        health_history: &Arc<RwLock<HashMap<String, Vec<HealthCheckResult>>>>,
        healing_history: &Arc<RwLock<Vec<HealingAttempt>>>,
    ) {
        let health_cutoff = SystemTime::now() - Duration::from_secs(86400); // 24 hours
        let healing_cutoff = SystemTime::now() - Duration::from_secs(86400 * 7); // 7 days
        
        // Clean health history
        {
            let mut history = health_history.write().await;
            for component_history in history.values_mut() {
                component_history.retain(|result| result.timestamp > health_cutoff);
            }
            // Remove empty component histories
            history.retain(|_, component_history| !component_history.is_empty());
        }
        
        // Clean healing history
        {
            let mut history = healing_history.write().await;
            history.retain(|attempt| attempt.timestamp > healing_cutoff);
        }
        
        debug!("Cleaned up old health and healing records");
    }
    
    /// Get current health status for all components
    pub async fn get_health_status(&self) -> HashMap<String, HealthCheckResult> {
        let history = self.health_history.read().await;
        let mut current_status = HashMap::new();
        
        for (component, component_history) in history.iter() {
            if let Some(latest) = component_history.last() {
                current_status.insert(component.clone(), latest.clone());
            }
        }
        
        current_status
    }
    
    /// Get healing history for a component
    pub async fn get_healing_history(&self, component: Option<&str>) -> Vec<HealingAttempt> {
        let history = self.healing_history.read().await;
        if let Some(component_name) = component {
            history.iter()
                .filter(|attempt| attempt.component == component_name)
                .cloned()
                .collect()
        } else {
            history.clone()
        }
    }
    
    /// Subscribe to health alerts
    pub fn subscribe_to_alerts(&self) -> broadcast::Receiver<HealthAlert> {
        self.alert_tx.subscribe()
    }
    
    /// Manually trigger health check for a component
    pub async fn trigger_health_check(&self, component: &str) -> Result<HealthCheckResult> {
        let checkers = self.checkers.read().await;
        if let Some(checker) = checkers.get(component) {
            checker.check_health().await
        } else {
            Err(MooseNGError::ComponentNotFound(component.to_string()).into())
        }
    }
    
    /// Manually trigger self-healing for a component
    pub async fn trigger_self_healing(&self, component: &str, action: SelfHealingAction) -> Result<bool> {
        let checkers = self.checkers.read().await;
        if let Some(checker) = checkers.get(component) {
            checker.perform_self_healing(&action).await
        } else {
            Err(MooseNGError::ComponentNotFound(component.to_string()).into())
        }
    }
}

/// Basic health checker implementation for generic components
pub struct BasicHealthChecker {
    component_name: String,
    health_check_fn: Arc<dyn Fn() -> Result<HealthCheckResult> + Send + Sync>,
    healing_actions: HashMap<String, Arc<dyn Fn(&SelfHealingAction) -> Result<bool> + Send + Sync>>,
}

impl BasicHealthChecker {
    pub fn new<F>(component_name: String, health_check_fn: F) -> Self 
    where
        F: Fn() -> Result<HealthCheckResult> + Send + Sync + 'static,
    {
        Self {
            component_name,
            health_check_fn: Arc::new(health_check_fn),
            healing_actions: HashMap::new(),
        }
    }
    
    pub fn with_healing_action<F>(mut self, action_type: String, handler: F) -> Self
    where
        F: Fn(&SelfHealingAction) -> Result<bool> + Send + Sync + 'static,
    {
        self.healing_actions.insert(action_type, Arc::new(handler));
        self
    }
}

#[async_trait::async_trait]
impl HealthChecker for BasicHealthChecker {
    async fn check_health(&self) -> Result<HealthCheckResult> {
        (self.health_check_fn)()
    }
    
    fn component_name(&self) -> &str {
        &self.component_name
    }
    
    async fn perform_self_healing(&self, action: &SelfHealingAction) -> Result<bool> {
        let action_type = match action {
            SelfHealingAction::RestartComponent { .. } => "restart",
            SelfHealingAction::RebalanceData { .. } => "rebalance",
            SelfHealingAction::ScaleUp { .. } => "scale_up",
            SelfHealingAction::ScaleDown { .. } => "scale_down",
            SelfHealingAction::ClearCache { .. } => "clear_cache",
            SelfHealingAction::RecoverData { .. } => "recover_data",
            SelfHealingAction::NetworkReconnect { .. } => "network_reconnect",
            SelfHealingAction::CustomAction { name, .. } => name,
        };
        
        if let Some(handler) = self.healing_actions.get(action_type) {
            handler(action)
        } else {
            warn!("No healing handler for action type: {}", action_type);
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::atomic::AtomicUsize;
    
    #[tokio::test]
    async fn test_health_monitor_creation() {
        let config = HealthCheckConfig::default();
        let monitor = HealthMonitor::new(config);
        
        let status = monitor.get_health_status().await;
        assert!(status.is_empty());
    }
    
    #[tokio::test]
    async fn test_basic_health_checker() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        
        let checker = BasicHealthChecker::new(
            "test_component".to_string(),
            move || {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok(HealthCheckResult {
                    component: "test_component".to_string(),
                    status: if count < 2 { HealthStatus::Healthy } else { HealthStatus::Warning },
                    message: format!("Check #{}", count + 1),
                    timestamp: SystemTime::now(),
                    metrics: HashMap::new(),
                    recommendations: Vec::new(),
                })
            }
        );
        
        // First check should be healthy
        let result1 = checker.check_health().await.unwrap();
        assert_eq!(result1.status, HealthStatus::Healthy);
        assert_eq!(result1.component, "test_component");
        
        // Second check should be healthy
        let result2 = checker.check_health().await.unwrap();
        assert_eq!(result2.status, HealthStatus::Healthy);
        
        // Third check should be warning
        let result3 = checker.check_health().await.unwrap();
        assert_eq!(result3.status, HealthStatus::Warning);
    }
    
    #[tokio::test]
    async fn test_self_healing_action() {
        let healed = Arc::new(AtomicBool::new(false));
        let healed_clone = healed.clone();
        
        let checker = BasicHealthChecker::new(
            "test_component".to_string(),
            || Ok(HealthCheckResult {
                component: "test_component".to_string(),
                status: HealthStatus::Critical,
                message: "Component failed".to_string(),
                timestamp: SystemTime::now(),
                metrics: HashMap::new(),
                recommendations: Vec::new(),
            })
        ).with_healing_action("restart".to_string(), move |_action| {
            healed_clone.store(true, Ordering::SeqCst);
            Ok(true)
        });
        
        let action = SelfHealingAction::RestartComponent {
            component: "test_component".to_string(),
        };
        
        let success = checker.perform_self_healing(&action).await.unwrap();
        assert!(success);
        assert!(healed.load(Ordering::SeqCst));
    }
    
    #[tokio::test]
    async fn test_health_monitor_with_checker() {
        let config = HealthCheckConfig {
            interval: Duration::from_millis(100),
            ..Default::default()
        };
        
        let mut monitor = HealthMonitor::new(config);
        
        let checker = Arc::new(BasicHealthChecker::new(
            "test_component".to_string(),
            || Ok(HealthCheckResult {
                component: "test_component".to_string(),
                status: HealthStatus::Healthy,
                message: "All good".to_string(),
                timestamp: SystemTime::now(),
                metrics: HashMap::new(),
                recommendations: Vec::new(),
            })
        ));
        
        monitor.register_checker(checker).await;
        
        // Trigger a manual health check
        let result = monitor.trigger_health_check("test_component").await.unwrap();
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.component, "test_component");
    }
}