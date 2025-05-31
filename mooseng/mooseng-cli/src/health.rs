//! Health monitoring and management commands for MooseNG CLI
//! Integrates with the health monitoring system and provides real-time health status

use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::grpc_client::{MooseNGClient, load_client_config};
use anyhow::{Context, Result};
use tracing::{info, warn, error};

#[derive(Subcommand)]
pub enum HealthCommands {
    /// Check overall cluster health
    Check {
        /// Include detailed component health information
        #[arg(short, long)]
        detailed: bool,
        /// Output format (table, json, yaml)
        #[arg(short, long, default_value = "table")]
        format: String,
        /// Continuous monitoring mode
        #[arg(short, long)]
        watch: bool,
        /// Watch interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },
    /// Show health alerts
    Alerts {
        /// Alert severity level filter (info, warning, error, critical)
        #[arg(short, long)]
        level: Option<String>,
        /// Component filter
        #[arg(short, long)]
        component: Option<String>,
        /// Number of alerts to show
        #[arg(short, long, default_value = "20")]
        count: u64,
        /// Show only unacknowledged alerts
        #[arg(short, long)]
        unack: bool,
    },
    /// Acknowledge health alerts
    Ack {
        /// Alert ID to acknowledge
        #[arg(short, long)]
        id: Option<String>,
        /// Acknowledge all alerts for a component
        #[arg(short, long)]
        component: Option<String>,
        /// Acknowledge all alerts of a level
        #[arg(short, long)]
        level: Option<String>,
    },
    /// Run health diagnostics
    Diagnose {
        /// Component to diagnose (all, master, chunkserver, client, metalogger)
        #[arg(short, long, default_value = "all")]
        component: String,
        /// Run deep diagnostics (may be slow)
        #[arg(short, long)]
        deep: bool,
        /// Save diagnostic report to file
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Configure health monitoring settings
    Configure {
        /// Enable/disable health monitoring
        #[arg(long)]
        enabled: Option<bool>,
        /// Health check interval in seconds
        #[arg(long)]
        interval: Option<u64>,
        /// Alert threshold configurations
        #[arg(long)]
        thresholds: Option<String>,
        /// Show current configuration
        #[arg(long)]
        show: bool,
    },
    /// Health monitoring metrics
    Metrics {
        /// Component to show metrics for
        #[arg(short, long)]
        component: Option<String>,
        /// Time range (1h, 6h, 24h, 7d)
        #[arg(short, long, default_value = "1h")]
        range: String,
        /// Export metrics to file
        #[arg(short, long)]
        export: Option<String>,
    },
    /// Test health monitoring endpoints
    Test {
        /// Test all endpoints
        #[arg(short, long)]
        all: bool,
        /// Test specific component
        #[arg(short, long)]
        component: Option<String>,
        /// Timeout in seconds
        #[arg(short, long, default_value = "10")]
        timeout: u64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub timestamp: u64,
    pub overall_status: HealthStatus,
    pub components: HashMap<String, ComponentHealthDetail>,
    pub active_alerts: Vec<HealthAlert>,
    pub recommendations: Vec<String>,
    pub system_info: SystemInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
    Maintenance,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentHealthDetail {
    pub name: String,
    pub status: HealthStatus,
    pub uptime_seconds: u64,
    pub last_check: u64,
    pub metrics: HashMap<String, f64>,
    pub dependencies: Vec<String>,
    pub alerts: Vec<HealthAlert>,
    pub self_healing_actions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthAlert {
    pub id: String,
    pub level: AlertLevel,
    pub component: String,
    pub title: String,
    pub message: String,
    pub timestamp: u64,
    pub acknowledged: bool,
    pub acknowledged_by: Option<String>,
    pub acknowledged_at: Option<u64>,
    pub count: u32,
    pub first_seen: u64,
    pub last_seen: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cluster_name: String,
    pub cluster_version: String,
    pub total_nodes: u32,
    pub healthy_nodes: u32,
    pub total_capacity_gb: u64,
    pub used_capacity_gb: u64,
    pub load_average: f64,
    pub memory_usage_percent: f64,
}

pub async fn handle_command(command: HealthCommands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        HealthCommands::Check { detailed, format, watch, interval } => {
            if watch {
                watch_health_status(detailed, &format, interval).await
            } else {
                check_health_status(detailed, &format).await
            }
        }
        HealthCommands::Alerts { level, component, count, unack } => {
            show_health_alerts(level.as_deref(), component.as_deref(), count, unack).await
        }
        HealthCommands::Ack { id, component, level } => {
            acknowledge_alerts(id.as_deref(), component.as_deref(), level.as_deref()).await
        }
        HealthCommands::Diagnose { component, deep, output } => {
            run_health_diagnostics(&component, deep, output.as_deref()).await
        }
        HealthCommands::Configure { enabled, interval, thresholds, show } => {
            configure_health_monitoring(enabled, interval, thresholds.as_deref(), show).await
        }
        HealthCommands::Metrics { component, range, export } => {
            show_health_metrics(component.as_deref(), &range, export.as_deref()).await
        }
        HealthCommands::Test { all, component, timeout } => {
            test_health_endpoints(all, component.as_deref(), timeout).await
        }
    }
}

/// Check cluster health status
async fn check_health_status(detailed: bool, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Checking cluster health status");
    
    // Try to connect to cluster via gRPC
    let grpc_result = fetch_health_via_grpc().await;
    let health = match grpc_result {
        Ok(health) => {
            println!("🟢 Connected to cluster - fetching live health data");
            health
        }
        Err(e) => {
            warn!("Failed to connect via gRPC: {}. Using fallback health check.", e);
            println!("🔴 Unable to connect to cluster - performing local health checks");
            fallback_health_check().await?
        }
    };
    
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&health)?);
        }
        "yaml" => {
            println!("{}", serde_yaml::to_string(&health)?);
        }
        _ => {
            display_health_table(&health, detailed);
        }
    }
    
    Ok(())
}

/// Fetch health status via gRPC
async fn fetch_health_via_grpc() -> Result<HealthCheckResult> {
    let config = load_client_config()?;
    let mut client = MooseNGClient::new(config).await?;
    
    // Get cluster status from master
    let cluster_status = client.get_cluster_status().await
        .context("Failed to get cluster status")?;
    
    // Convert gRPC response to health check result
    let mut components = HashMap::new();
    let mut alerts = Vec::new();
    
    // Process master health
    let master_health = ComponentHealthDetail {
        name: "Master Server".to_string(),
        status: if cluster_status.is_healthy { 
            HealthStatus::Healthy 
        } else { 
            HealthStatus::Warning 
        },
        uptime_seconds: cluster_status.uptime_seconds,
        last_check: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        metrics: {
            let mut metrics = HashMap::new();
            metrics.insert("cpu_usage".to_string(), cluster_status.cpu_usage_percent);
            metrics.insert("memory_usage".to_string(), cluster_status.memory_usage_percent);
            metrics.insert("active_connections".to_string(), cluster_status.active_connections as f64);
            metrics.insert("operations_per_second".to_string(), cluster_status.total_operations as f64);
            metrics
        },
        dependencies: vec!["network".to_string(), "storage".to_string()],
        alerts: Vec::new(),
        self_healing_actions: vec![
            "Automatic cache optimization".to_string(),
            "Connection pool management".to_string(),
        ],
    };
    
    components.insert("master".to_string(), master_health);
    
    // Add alerts for high resource usage
    if cluster_status.cpu_usage_percent > 80.0 {
        alerts.push(HealthAlert {
            id: format!("cpu-high-{}", chrono::Utc::now().timestamp()),
            level: if cluster_status.cpu_usage_percent > 90.0 { 
                AlertLevel::Critical 
            } else { 
                AlertLevel::Warning 
            },
            component: "master".to_string(),
            title: "High CPU Usage".to_string(),
            message: format!("CPU usage is {:.1}%", cluster_status.cpu_usage_percent),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            acknowledged: false,
            acknowledged_by: None,
            acknowledged_at: None,
            count: 1,
            first_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }
    
    if cluster_status.memory_usage_percent > 85.0 {
        alerts.push(HealthAlert {
            id: format!("memory-high-{}", chrono::Utc::now().timestamp()),
            level: if cluster_status.memory_usage_percent > 95.0 { 
                AlertLevel::Critical 
            } else { 
                AlertLevel::Warning 
            },
            component: "master".to_string(),
            title: "High Memory Usage".to_string(),
            message: format!("Memory usage is {:.1}%", cluster_status.memory_usage_percent),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            acknowledged: false,
            acknowledged_by: None,
            acknowledged_at: None,
            count: 1,
            first_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }
    
    // Generate recommendations
    let mut recommendations = Vec::new();
    if cluster_status.cpu_usage_percent > 80.0 {
        recommendations.push("Consider scaling up the cluster or optimizing workloads".to_string());
    }
    if cluster_status.cache_hit_rate < 0.8 {
        recommendations.push(format!(
            "Cache hit rate is {:.1}%. Consider increasing cache size or reviewing access patterns",
            cluster_status.cache_hit_rate * 100.0
        ));
    }
    
    let overall_status = if alerts.iter().any(|a| matches!(a.level, AlertLevel::Critical)) {
        HealthStatus::Critical
    } else if alerts.iter().any(|a| matches!(a.level, AlertLevel::Warning)) {
        HealthStatus::Warning
    } else {
        HealthStatus::Healthy
    };
    
    Ok(HealthCheckResult {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        overall_status,
        components,
        active_alerts: alerts,
        recommendations,
        system_info: SystemInfo {
            cluster_name: cluster_status.cluster_name,
            cluster_version: cluster_status.cluster_version,
            total_nodes: cluster_status.servers.len() as u32,
            healthy_nodes: cluster_status.servers.iter()
                .filter(|s| s.status == "online")
                .count() as u32,
            total_capacity_gb: cluster_status.total_capacity_bytes / (1024 * 1024 * 1024),
            used_capacity_gb: cluster_status.used_capacity_bytes / (1024 * 1024 * 1024),
            load_average: cluster_status.cpu_usage_percent / 100.0,
            memory_usage_percent: cluster_status.memory_usage_percent,
        },
    })
}

/// Fallback health check when gRPC is not available
async fn fallback_health_check() -> Result<HealthCheckResult> {
    let mut components = HashMap::new();
    let mut alerts = Vec::new();
    
    // Basic system checks
    let system_check = check_local_system().await?;
    components.insert("system".to_string(), system_check);
    
    // Network connectivity check
    let network_check = check_network_connectivity().await?;
    components.insert("network".to_string(), network_check);
    
    // Add connectivity alert
    alerts.push(HealthAlert {
        id: "connectivity-lost".to_string(),
        level: AlertLevel::Critical,
        component: "cluster".to_string(),
        title: "Cluster Connectivity Lost".to_string(),
        message: "Unable to connect to master server for health monitoring".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        acknowledged: false,
        acknowledged_by: None,
        acknowledged_at: None,
        count: 1,
        first_seen: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        last_seen: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });
    
    Ok(HealthCheckResult {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        overall_status: HealthStatus::Critical,
        components,
        active_alerts: alerts,
        recommendations: vec![
            "Check network connectivity to master server".to_string(),
            "Verify master server is running and accessible".to_string(),
            "Review firewall and security group settings".to_string(),
        ],
        system_info: SystemInfo {
            cluster_name: "unknown".to_string(),
            cluster_version: "unknown".to_string(),
            total_nodes: 0,
            healthy_nodes: 0,
            total_capacity_gb: 0,
            used_capacity_gb: 0,
            load_average: 0.0,
            memory_usage_percent: 0.0,
        },
    })
}

/// Check local system health
async fn check_local_system() -> Result<ComponentHealthDetail> {
    use std::fs;
    
    let mut metrics = HashMap::new();
    
    // Check available disk space
    if let Ok(metadata) = fs::metadata("/") {
        // This is a simplified check - in production would use proper disk space API
        metrics.insert("disk_available".to_string(), 85.0);
    }
    
    // Check if we can write temp files
    let can_write = std::fs::write("/tmp/mooseng_health_test", "test").is_ok();
    metrics.insert("filesystem_writable".to_string(), if can_write { 1.0 } else { 0.0 });
    
    Ok(ComponentHealthDetail {
        name: "Local System".to_string(),
        status: if can_write { HealthStatus::Healthy } else { HealthStatus::Warning },
        uptime_seconds: 0, // Would get from system APIs
        last_check: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        metrics,
        dependencies: vec!["filesystem".to_string()],
        alerts: Vec::new(),
        self_healing_actions: vec![
            "Cleanup temporary files".to_string(),
        ],
    })
}

/// Check network connectivity
async fn check_network_connectivity() -> Result<ComponentHealthDetail> {
    let mut metrics = HashMap::new();
    
    // Basic DNS resolution test
    let dns_works = tokio::net::lookup_host("github.com:443").await.is_ok();
    metrics.insert("dns_resolution".to_string(), if dns_works { 1.0 } else { 0.0 });
    
    Ok(ComponentHealthDetail {
        name: "Network Connectivity".to_string(),
        status: if dns_works { HealthStatus::Healthy } else { HealthStatus::Critical },
        uptime_seconds: 0,
        last_check: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        metrics,
        dependencies: vec!["dns".to_string(), "internet".to_string()],
        alerts: Vec::new(),
        self_healing_actions: Vec::new(),
    })
}

/// Display health status in table format
fn display_health_table(health: &HealthCheckResult, detailed: bool) {
    println!("Cluster Health Status Report");
    println!("{}", "=".repeat(60));
    println!("Overall Status: {:?}", health.overall_status);
    println!("Timestamp: {}", chrono::DateTime::from_timestamp(health.timestamp as i64, 0)
        .unwrap_or_default()
        .format("%Y-%m-%d %H:%M:%S UTC"));
    println!();
    
    // System overview
    println!("System Overview:");
    println!("  Cluster: {} ({})", health.system_info.cluster_name, health.system_info.cluster_version);
    println!("  Nodes: {}/{} healthy", health.system_info.healthy_nodes, health.system_info.total_nodes);
    println!("  Capacity: {:.1} GB / {:.1} GB ({:.1}%)", 
             health.system_info.used_capacity_gb,
             health.system_info.total_capacity_gb,
             if health.system_info.total_capacity_gb > 0 {
                 (health.system_info.used_capacity_gb as f64 / health.system_info.total_capacity_gb as f64) * 100.0
             } else { 0.0 });
    println!();
    
    // Component status
    println!("Component Status:");
    for (name, component) in &health.components {
        let status_icon = match component.status {
            HealthStatus::Healthy => "✓",
            HealthStatus::Warning => "⚠",
            HealthStatus::Critical => "✗",
            HealthStatus::Unknown => "?",
            HealthStatus::Maintenance => "🔧",
        };
        
        println!("  {} {} - {:?}", status_icon, component.name, component.status);
        
        if detailed {
            for (metric_name, value) in &component.metrics {
                println!("    {}: {:.2}", metric_name.replace('_', " "), value);
            }
            
            if !component.dependencies.is_empty() {
                println!("    Dependencies: {}", component.dependencies.join(", "));
            }
        }
    }
    println!();
    
    // Active alerts
    if !health.active_alerts.is_empty() {
        println!("Active Alerts ({}):", health.active_alerts.len());
        for alert in &health.active_alerts {
            let level_icon = match alert.level {
                AlertLevel::Info => "ℹ",
                AlertLevel::Warning => "⚠",
                AlertLevel::Error => "❌",
                AlertLevel::Critical => "🚨",
            };
            
            let ack_status = if alert.acknowledged { " (ACK)" } else { "" };
            println!("  {} [{}] {}: {}{}", 
                     level_icon, alert.component, alert.title, alert.message, ack_status);
            
            if detailed && alert.count > 1 {
                println!("    Occurrences: {} times", alert.count);
                println!("    First seen: {}", chrono::DateTime::from_timestamp(alert.first_seen as i64, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S"));
            }
        }
        println!();
    }
    
    // Recommendations
    if !health.recommendations.is_empty() {
        println!("Recommendations:");
        for rec in &health.recommendations {
            println!("  • {}", rec);
        }
        println!();
    }
    
    // Summary
    let critical_alerts = health.active_alerts.iter()
        .filter(|a| matches!(a.level, AlertLevel::Critical))
        .count();
    let warning_alerts = health.active_alerts.iter()
        .filter(|a| matches!(a.level, AlertLevel::Warning))
        .count();
    
    if critical_alerts > 0 {
        println!("🚨 {} critical alerts require immediate attention", critical_alerts);
    } else if warning_alerts > 0 {
        println!("⚠️  {} warnings detected", warning_alerts);
    } else {
        println!("✅ All systems operational");
    }
}

/// Watch health status continuously
async fn watch_health_status(detailed: bool, format: &str, interval: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting health monitoring (interval: {}s, press Ctrl+C to stop)", interval);
    
    loop {
        // Clear screen
        print!("\x1B[2J\x1B[1;1H");
        
        match check_health_status(detailed, format).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Health check failed: {}", e);
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
    }
}

/// Show health alerts
async fn show_health_alerts(level: Option<&str>, component: Option<&str>, count: u64, unack_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement when health monitoring system is fully integrated
    println!("Health alerts (filtered by level: {:?}, component: {:?})", level, component);
    println!("Feature coming soon - alerts will be displayed here");
    Ok(())
}

/// Acknowledge health alerts
async fn acknowledge_alerts(id: Option<&str>, component: Option<&str>, level: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement when health monitoring system is fully integrated
    println!("Acknowledging alerts (id: {:?}, component: {:?}, level: {:?})", id, component, level);
    println!("Feature coming soon - alert acknowledgment will be implemented here");
    Ok(())
}

/// Run health diagnostics
async fn run_health_diagnostics(component: &str, deep: bool, output: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Running health diagnostics for: {}", component);
    if deep {
        println!("Performing deep diagnostics (this may take a while)...");
    }
    
    // Perform basic diagnostics
    let diagnostics = perform_diagnostics(component, deep).await?;
    
    if let Some(output_file) = output {
        std::fs::write(output_file, serde_json::to_string_pretty(&diagnostics)?)?;
        println!("Diagnostic report saved to: {}", output_file);
    } else {
        println!("{}", serde_json::to_string_pretty(&diagnostics)?);
    }
    
    Ok(())
}

/// Perform actual diagnostics
async fn perform_diagnostics(component: &str, deep: bool) -> Result<serde_json::Value> {
    let mut diagnostics = serde_json::Map::new();
    
    diagnostics.insert("component".to_string(), serde_json::Value::String(component.to_string()));
    diagnostics.insert("deep_scan".to_string(), serde_json::Value::Bool(deep));
    diagnostics.insert("timestamp".to_string(), serde_json::Value::Number(
        serde_json::Number::from(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs())
    ));
    
    // Add basic system information
    let mut system_info = serde_json::Map::new();
    system_info.insert("os".to_string(), serde_json::Value::String(std::env::consts::OS.to_string()));
    system_info.insert("arch".to_string(), serde_json::Value::String(std::env::consts::ARCH.to_string()));
    
    diagnostics.insert("system".to_string(), serde_json::Value::Object(system_info));
    
    Ok(serde_json::Value::Object(diagnostics))
}

/// Configure health monitoring
async fn configure_health_monitoring(enabled: Option<bool>, interval: Option<u64>, thresholds: Option<&str>, show: bool) -> Result<(), Box<dyn std::error::Error>> {
    if show {
        println!("Current health monitoring configuration:");
        println!("  Enabled: true (placeholder)");
        println!("  Check interval: 30 seconds (placeholder)");
        println!("  CPU threshold: 80% (placeholder)");
        println!("  Memory threshold: 85% (placeholder)");
        return Ok(());
    }
    
    if let Some(enabled) = enabled {
        println!("Setting health monitoring enabled: {}", enabled);
    }
    
    if let Some(interval) = interval {
        println!("Setting health check interval: {} seconds", interval);
    }
    
    if let Some(thresholds) = thresholds {
        println!("Setting alert thresholds: {}", thresholds);
    }
    
    println!("Configuration saved (placeholder implementation)");
    Ok(())
}

/// Show health metrics
async fn show_health_metrics(component: Option<&str>, range: &str, export: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Health metrics for {} over {}", component.unwrap_or("all components"), range);
    println!("Feature coming soon - health metrics will be displayed here");
    
    if let Some(export_file) = export {
        println!("Metrics will be exported to: {}", export_file);
    }
    
    Ok(())
}

/// Test health endpoints
async fn test_health_endpoints(all: bool, component: Option<&str>, timeout: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing health endpoints (timeout: {}s)", timeout);
    
    if all {
        println!("Testing all component endpoints...");
    } else if let Some(comp) = component {
        println!("Testing {} endpoints...", comp);
    }
    
    // Test gRPC connectivity
    match load_client_config() {
        Ok(config) => {
            println!("✓ Client configuration loaded");
            
            match MooseNGClient::new(config).await {
                Ok(mut client) => {
                    println!("✓ gRPC client created");
                    
                    let start = std::time::Instant::now();
                    match client.get_cluster_status().await {
                        Ok(_) => {
                            let duration = start.elapsed();
                            println!("✓ Master server health endpoint responded in {:?}", duration);
                        }
                        Err(e) => {
                            println!("✗ Master server health endpoint failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("✗ Failed to create gRPC client: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to load client configuration: {}", e);
        }
    }
    
    Ok(())
}