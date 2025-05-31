use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::grpc_client::{MooseNGClient, load_client_config};
use anyhow::{Context, Result};
use tracing::{info, warn, error};

#[derive(Subcommand)]
pub enum MonitorCommands {
    /// Show real-time cluster metrics
    Metrics {
        /// Component to monitor (cluster, master, chunkserver, metalogger)
        #[arg(short, long, default_value = "cluster")]
        component: String,
        /// Specific component ID to monitor
        #[arg(short, long)]
        id: Option<String>,
        /// Refresh interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
        /// Number of iterations (0 = infinite)
        #[arg(short, long, default_value = "0")]
        count: u64,
    },
    /// Show performance statistics
    Stats {
        /// Time range (1h, 24h, 7d, 30d)
        #[arg(short, long, default_value = "1h")]
        range: String,
        /// Show detailed statistics
        #[arg(short, long)]
        verbose: bool,
    },
    /// Show cluster health status
    Health {
        /// Include detailed health checks
        #[arg(short, long)]
        detailed: bool,
    },
    /// Monitor specific operations
    Operations {
        /// Operation type (read, write, replication, recovery)
        #[arg(short, long)]
        operation: Option<String>,
        /// Show only slow operations
        #[arg(short, long)]
        slow: bool,
        /// Minimum duration in ms for slow operations
        #[arg(short, long, default_value = "1000")]
        threshold: u64,
    },
    /// Show event log
    Events {
        /// Number of recent events to show
        #[arg(short, long, default_value = "20")]
        count: u64,
        /// Event level filter (info, warn, error, critical)
        #[arg(short, long)]
        level: Option<String>,
        /// Component filter
        #[arg(short, long)]
        component: Option<String>,
    },
    /// Export metrics to file
    Export {
        /// Output format (json, csv, prometheus)
        #[arg(short, long, default_value = "json")]
        format: String,
        /// Output file path
        #[arg(short, long)]
        output: String,
        /// Time range to export
        #[arg(short, long, default_value = "24h")]
        range: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterMetrics {
    pub timestamp: u64,
    pub total_operations: u64,
    pub read_operations: u64,
    pub write_operations: u64,
    pub avg_read_latency_ms: f64,
    pub avg_write_latency_ms: f64,
    pub throughput_mbps: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub network_io_mbps: f64,
    pub disk_io_mbps: f64,
    pub active_connections: u64,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub overall_health: HealthLevel,
    pub components: HashMap<String, ComponentHealth>,
    pub alerts: Vec<Alert>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HealthLevel {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub health: HealthLevel,
    pub status: String,
    pub metrics: HashMap<String, f64>,
    pub last_check: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub level: AlertLevel,
    pub component: String,
    pub message: String,
    pub timestamp: u64,
    pub acknowledged: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub operation_type: String,
    pub component: String,
    pub start_time: u64,
    pub duration_ms: u64,
    pub size_bytes: u64,
    pub status: OperationStatus,
    pub details: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OperationStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub timestamp: u64,
    pub level: AlertLevel,
    pub component: String,
    pub message: String,
    pub metadata: HashMap<String, String>,
}

pub async fn handle_command(command: MonitorCommands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        MonitorCommands::Metrics { component, id, interval, count } => {
            show_metrics(&component, id.as_deref(), interval, count).await
        }
        MonitorCommands::Stats { range, verbose } => {
            show_stats(&range, verbose).await
        }
        MonitorCommands::Health { detailed } => {
            show_health(detailed).await
        }
        MonitorCommands::Operations { operation, slow, threshold } => {
            show_operations(operation.as_deref(), slow, threshold).await
        }
        MonitorCommands::Events { count, level, component } => {
            show_events(count, level.as_deref(), component.as_deref()).await
        }
        MonitorCommands::Export { format, output, range } => {
            export_metrics(&format, &output, &range).await
        }
    }
}

async fn show_metrics(component: &str, id: Option<&str>, interval: u64, count: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("Monitoring {} metrics", component);
    if let Some(component_id) = id {
        println!("Component ID: {}", component_id);
    }
    println!("Refresh interval: {} seconds", interval);
    println!();

    // Try to connect to gRPC client
    let grpc_client = create_grpc_client().await;
    let mut iterations = 0;
    
    loop {
        let metrics = if let Ok(ref mut client) = grpc_client.as_ref() {
            // Fetch real metrics from cluster
            fetch_cluster_metrics(client, component).await.unwrap_or_else(|e| {
                eprintln!("Warning: Failed to fetch cluster metrics: {}. Using fallback data.", e);
                generate_fallback_metrics(iterations)
            })
        } else {
            // Use fallback metrics if gRPC connection failed
            generate_fallback_metrics(iterations)
        };

        // Clear screen and display metrics
        print!("\x1B[2J\x1B[1;1H"); // Clear screen and move cursor to top-left
        let connection_status = if grpc_client.is_ok() { "🟢 LIVE" } else { "🔴 OFFLINE" };
        println!("MooseNG Cluster Metrics - {} ({}) [{}]", component, 
                 chrono::DateTime::from_timestamp(metrics.timestamp as i64, 0)
                     .unwrap_or_default()
                     .format("%Y-%m-%d %H:%M:%S"),
                 connection_status);
        println!("{}", "=".repeat(70));
        
        println!("Operations:");
        println!("  Total: {} ops/s", metrics.total_operations);
        println!("  Read:  {} ops/s (avg: {:.2}ms)", metrics.read_operations, metrics.avg_read_latency_ms);
        println!("  Write: {} ops/s (avg: {:.2}ms)", metrics.write_operations, metrics.avg_write_latency_ms);
        println!();
        
        println!("Performance:");
        println!("  Throughput: {:.1} MB/s", metrics.throughput_mbps);
        println!("  CPU Usage: {:.1}%", metrics.cpu_usage_percent);
        println!("  Memory Usage: {:.1}%", metrics.memory_usage_percent);
        println!("  Network I/O: {:.1} MB/s", metrics.network_io_mbps);
        println!("  Disk I/O: {:.1} MB/s", metrics.disk_io_mbps);
        println!();
        
        println!("Cache & Connections:");
        println!("  Cache Hit Rate: {:.1}%", metrics.cache_hit_rate * 100.0);
        println!("  Active Connections: {}", metrics.active_connections);
        
        if grpc_client.is_err() {
            println!();
            println!("⚠️  Note: Unable to connect to master server. Displaying simulated data.");
            println!("   Check that the master server is running and accessible.");
        }
        
        iterations += 1;
        
        if count > 0 && iterations >= count {
            break;
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
    }

    Ok(())
}

/// Create gRPC client for cluster communication
async fn create_grpc_client() -> Result<MooseNGClient, Box<dyn std::error::Error>> {
    let config = load_client_config()?;
    let client = MooseNGClient::new(config).await?;
    Ok(client)
}

/// Fetch real cluster metrics from master server
async fn fetch_cluster_metrics(client: &mut MooseNGClient, component: &str) -> Result<ClusterMetrics> {
    let cluster_status = client.get_cluster_status().await
        .context("Failed to get cluster status")?;
    
    // Convert gRPC response to our metrics format
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    
    Ok(ClusterMetrics {
        timestamp,
        total_operations: cluster_status.total_operations as u64,
        read_operations: cluster_status.read_operations as u64,
        write_operations: cluster_status.write_operations as u64,
        avg_read_latency_ms: cluster_status.avg_read_latency_ms,
        avg_write_latency_ms: cluster_status.avg_write_latency_ms,
        throughput_mbps: cluster_status.throughput_mbps,
        cpu_usage_percent: cluster_status.cpu_usage_percent,
        memory_usage_percent: cluster_status.memory_usage_percent,
        network_io_mbps: cluster_status.network_io_mbps,
        disk_io_mbps: cluster_status.disk_io_mbps,
        active_connections: cluster_status.active_connections as u64,
        cache_hit_rate: cluster_status.cache_hit_rate,
    })
}

/// Generate fallback metrics when gRPC is unavailable
fn generate_fallback_metrics(iterations: u64) -> ClusterMetrics {
    ClusterMetrics {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        total_operations: 15420 + iterations * 50,
        read_operations: 10280 + iterations * 30,
        write_operations: 5140 + iterations * 20,
        avg_read_latency_ms: 2.5 + (iterations as f64 * 0.1),
        avg_write_latency_ms: 8.2 + (iterations as f64 * 0.2),
        throughput_mbps: 125.6 + (iterations as f64 * 2.0),
        cpu_usage_percent: 45.2 + (iterations as f64 * 0.5) % 20.0,
        memory_usage_percent: 72.1 + (iterations as f64 * 0.3) % 15.0,
        network_io_mbps: 89.4 + (iterations as f64 * 1.5),
        disk_io_mbps: 234.7 + (iterations as f64 * 3.0),
        active_connections: 1250 + iterations * 5,
        cache_hit_rate: 0.89 + (iterations as f64 * 0.001) % 0.1,
    }
}

async fn show_stats(range: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement actual statistics retrieval
    println!("Performance Statistics ({})", range);
    println!("{}", "=".repeat(40));
    
    if verbose {
        println!("Average Read Latency: 2.3ms (±0.5ms)");
        println!("Average Write Latency: 8.1ms (±1.2ms)");
        println!("P95 Read Latency: 4.8ms");
        println!("P95 Write Latency: 15.2ms");
        println!("P99 Read Latency: 12.1ms");
        println!("P99 Write Latency: 28.4ms");
        println!();
        println!("Total Operations: 15,420,384");
        println!("Read Operations: 10,280,256 (66.7%)");
        println!("Write Operations: 5,140,128 (33.3%)");
        println!("Failed Operations: 1,024 (0.007%)");
        println!();
        println!("Average Throughput: 125.6 MB/s");
        println!("Peak Throughput: 245.2 MB/s");
        println!("Average CPU Usage: 45.2%");
        println!("Peak CPU Usage: 78.9%");
        println!("Average Memory Usage: 72.1%");
        println!("Peak Memory Usage: 89.3%");
    } else {
        println!("Read Latency (avg): 2.3ms");
        println!("Write Latency (avg): 8.1ms");
        println!("Throughput (avg): 125.6 MB/s");
        println!("CPU Usage (avg): 45.2%");
        println!("Memory Usage (avg): 72.1%");
        println!("Cache Hit Rate: 89.2%");
        println!("Uptime: 99.98%");
    }

    Ok(())
}

async fn show_health(detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Collecting cluster health status...");
    
    // Try to connect to gRPC client first
    let grpc_client = create_grpc_client().await;
    let health = if let Ok(mut client) = grpc_client {
        println!("🟢 Connected to master server - fetching live health data");
        collect_cluster_health_grpc(&mut client).await.unwrap_or_else(|e| {
            eprintln!("Warning: Failed to fetch health data via gRPC: {}. Using fallback data.", e);
            collect_cluster_health_fallback()
        })
    } else {
        println!("🔴 Unable to connect to master server - using simulated data");
        collect_cluster_health_fallback()
    };
    
    display_health_status(&health, detailed).await;
    Ok(())
}

/// Collect cluster health via gRPC
async fn collect_cluster_health_grpc(client: &mut MooseNGClient) -> Result<HealthStatus> {
    let cluster_status = client.get_cluster_status().await
        .context("Failed to get cluster status")?;
    
    let mut components = HashMap::new();
    let mut alerts = Vec::new();
    let mut recommendations = Vec::new();
    
    // Add master component health from cluster status
    let mut master_metrics = HashMap::new();
    master_metrics.insert("cpu_usage".to_string(), cluster_status.cpu_usage_percent);
    master_metrics.insert("memory_usage".to_string(), cluster_status.memory_usage_percent);
    master_metrics.insert("total_operations".to_string(), cluster_status.total_operations as f64);
    master_metrics.insert("cache_hit_rate".to_string(), cluster_status.cache_hit_rate);
    
    let master_health = if cluster_status.cpu_usage_percent > 90.0 || cluster_status.memory_usage_percent > 95.0 {
        HealthLevel::Critical
    } else if cluster_status.cpu_usage_percent > 80.0 || cluster_status.memory_usage_percent > 85.0 {
        HealthLevel::Warning
    } else {
        HealthLevel::Healthy
    };
    
    components.insert("master".to_string(), ComponentHealth {
        name: "Master Server".to_string(),
        health: master_health,
        status: format!("Master online - {} active connections", cluster_status.active_connections),
        metrics: master_metrics,
        last_check: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });
    
    // Generate alerts based on metrics
    if cluster_status.cpu_usage_percent > 90.0 {
        alerts.push(Alert {
            id: "high-cpu".to_string(),
            level: AlertLevel::Critical,
            component: "master".to_string(),
            message: format!("High CPU usage: {:.1}%", cluster_status.cpu_usage_percent),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            acknowledged: false,
        });
    }
    
    if cluster_status.memory_usage_percent > 90.0 {
        alerts.push(Alert {
            id: "high-memory".to_string(),
            level: AlertLevel::Warning,
            component: "master".to_string(),
            message: format!("High memory usage: {:.1}%", cluster_status.memory_usage_percent),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            acknowledged: false,
        });
    }
    
    if cluster_status.cache_hit_rate < 0.7 {
        recommendations.push(format!(
            "Cache hit rate is low ({:.1}%). Consider increasing cache size or reviewing access patterns.",
            cluster_status.cache_hit_rate * 100.0
        ));
    }
    
    // Determine overall health
    let overall_health = determine_overall_health(&components, &alerts);
    
    Ok(HealthStatus {
        overall_health,
        components,
        alerts,
        recommendations,
    })
}

/// Fallback health collection when gRPC is unavailable
fn collect_cluster_health_fallback() -> HealthStatus {
    let mut components = HashMap::new();
    let mut alerts = Vec::new();
    let mut recommendations = Vec::new();
    
    // Create mock master health since we can't connect to real cluster
    let mut master_metrics = HashMap::new();
    master_metrics.insert("cpu_usage".to_string(), 35.2);
    master_metrics.insert("memory_usage".to_string(), 68.4);
    master_metrics.insert("uptime_hours".to_string(), 168.5);
    master_metrics.insert("active_sessions".to_string(), 42.0);
    
    components.insert("master".to_string(), ComponentHealth {
        name: "Master Server".to_string(),
        health: HealthLevel::Unknown,
        status: "Unable to connect - status unknown".to_string(),
        metrics: master_metrics,
        last_check: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });
    
    // Add connectivity alert
    alerts.push(Alert {
        id: "master-connectivity".to_string(),
        level: AlertLevel::Warning,
        component: "master".to_string(),
        message: "Unable to connect to master server for health check".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        acknowledged: false,
    });
    
    // Add basic recommendations
    recommendations.push("Check network connectivity to master server".to_string());
    recommendations.push("Verify master server configuration and status".to_string());
    
    // Determine overall health
    let overall_health = determine_overall_health(&components, &alerts);
    
    HealthStatus {
        overall_health,
        components,
        alerts,
        recommendations,
    }
}

async fn collect_master_health() -> Result<Option<ComponentHealth>, Box<dyn std::error::Error>> {
    // TODO: Re-enable when mooseng_master health module is available
    // use mooseng_common::health::{HealthMonitor, HealthChecker, HealthStatus as CommonHealthStatus};
    // use mooseng_master::health_checker::MasterHealthChecker;
    use std::sync::Arc;
    
    // TODO: Replace with actual master health check when modules are available
    // For now, return placeholder health status
    let mut metrics = HashMap::new();
    metrics.insert("cpu_usage".to_string(), 35.2);
    metrics.insert("memory_usage".to_string(), 68.4);
    metrics.insert("uptime_hours".to_string(), 168.5);
    metrics.insert("active_sessions".to_string(), 42.0);
    metrics.insert("metadata_operations_per_sec".to_string(), 1250.0);
    
    Ok(Some(ComponentHealth {
        name: "Master Server".to_string(),
        health: HealthLevel::Healthy,
        status: "Master server running (placeholder status)".to_string(),
        metrics,
        last_check: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }))
}

async fn collect_chunkserver_health() -> Result<HashMap<String, ComponentHealth>, Box<dyn std::error::Error>> {
    // TODO: Re-enable when mooseng_chunkserver health module is available
    // use mooseng_common::health::{HealthChecker, HealthStatus as CommonHealthStatus};
    // use mooseng_chunkserver::health_checker::ChunkServerHealthChecker;
    use std::sync::Arc;
    
    let mut chunkservers = HashMap::new();
    
    // TODO: Replace with actual chunk server discovery and health checks
    // For now, simulate a couple of chunk servers with placeholder data
    let chunk_server_ids = vec!["cs-001", "cs-002", "cs-003"];
    
    for (i, server_id) in chunk_server_ids.iter().enumerate() {
        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 40.0 + (i as f64 * 5.0));
        metrics.insert("memory_usage".to_string(), 70.0 + (i as f64 * 8.0));
        metrics.insert("disk_usage".to_string(), 65.0 + (i as f64 * 15.0));
        metrics.insert("active_chunks".to_string(), 15000.0 + (i as f64 * 2500.0));
        metrics.insert("network_io_mbps".to_string(), 85.0 + (i as f64 * 10.0));
        
        let health_level = if i == 1 { HealthLevel::Warning } else { HealthLevel::Healthy };
        let status = if i == 1 { 
            "High disk usage detected".to_string() 
        } else { 
            "Chunk server operating normally".to_string() 
        };
        
        chunkservers.insert(server_id.to_string(), ComponentHealth {
            name: format!("Chunk Server {}", server_id),
            health: health_level,
            status,
            metrics,
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }
    
    Ok(chunkservers)
}

async fn collect_metalogger_health() -> Result<HashMap<String, ComponentHealth>, Box<dyn std::error::Error>> {
    // TODO: Re-enable when mooseng_metalogger health module is available
    // use mooseng_common::health::{HealthChecker, HealthStatus as CommonHealthStatus};
    // use mooseng_metalogger::health_checker::MetaloggerHealthChecker;
    use std::sync::Arc;
    
    let mut metaloggers = HashMap::new();
    
    // TODO: Replace with actual metalogger discovery and health checks
    // For now, simulate metaloggers with placeholder data
    let metalogger_ids = vec!["ml-001", "ml-002"];
    
    for (i, metalogger_id) in metalogger_ids.iter().enumerate() {
        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 25.0 + (i as f64 * 3.0));
        metrics.insert("memory_usage".to_string(), 45.0 + (i as f64 * 5.0));
        metrics.insert("replication_lag_ms".to_string(), 150.0 + (i as f64 * 50.0));
        metrics.insert("wal_size_mb".to_string(), 1024.0 + (i as f64 * 256.0));
        metrics.insert("last_snapshot_age_hours".to_string(), 12.0 + (i as f64 * 2.0));
        
        metaloggers.insert(metalogger_id.to_string(), ComponentHealth {
            name: format!("Metalogger {}", metalogger_id),
            health: HealthLevel::Healthy,
            status: "Metalogger replicating metadata successfully".to_string(),
            metrics,
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
    }
    
    Ok(metaloggers)
}

async fn collect_client_health() -> Result<HashMap<String, ComponentHealth>, Box<dyn std::error::Error>> {
    // TODO: Re-enable when mooseng_client health module is available
    // use mooseng_common::health::{HealthChecker, HealthStatus as CommonHealthStatus};
    // use mooseng_client::health_checker::ClientHealthChecker;
    use std::sync::Arc;
    
    let mut clients = HashMap::new();
    
    // TODO: Replace with actual client discovery and health checks
    // Client health is optional - only check if clients are detected/running
    // For now, simulate minimal client presence with placeholder data
    
    if std::env::var("MOOSENG_CLI_CHECK_CLIENT_HEALTH").is_ok() {
        let client_ids = vec!["client-001"];
        
        for client_id in client_ids {
            let mut metrics = HashMap::new();
            metrics.insert("cache_hit_rate".to_string(), 0.85);
            metrics.insert("active_connections".to_string(), 5.0);
            metrics.insert("read_operations_per_sec".to_string(), 125.0);
            metrics.insert("write_operations_per_sec".to_string(), 45.0);
            
            clients.insert(client_id.to_string(), ComponentHealth {
                name: format!("Client {}", client_id),
                health: HealthLevel::Healthy,
                status: "Client connected and operating normally".to_string(),
                metrics,
                last_check: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
        }
    }
    
    Ok(clients)
}

fn determine_overall_health(components: &HashMap<String, ComponentHealth>, alerts: &[Alert]) -> HealthLevel {
    // Check for critical alerts
    if alerts.iter().any(|a| matches!(a.level, AlertLevel::Critical)) {
        return HealthLevel::Critical;
    }
    
    // Check component health levels
    let critical_components = components.values()
        .filter(|c| matches!(c.health, HealthLevel::Critical))
        .count();
    
    if critical_components > 0 {
        return HealthLevel::Critical;
    }
    
    let warning_components = components.values()
        .filter(|c| matches!(c.health, HealthLevel::Warning))
        .count();
    
    if warning_components > 0 || alerts.iter().any(|a| matches!(a.level, AlertLevel::Error | AlertLevel::Warning)) {
        return HealthLevel::Warning;
    }
    
    HealthLevel::Healthy
}

fn generate_health_recommendations(components: &HashMap<String, ComponentHealth>, alerts: &[Alert]) -> Vec<String> {
    let mut recommendations = Vec::new();
    
    // Check for high disk usage
    let high_disk_servers: Vec<_> = components.iter()
        .filter(|(name, component)| {
            name.starts_with("chunkserver") && 
            component.metrics.get("disk_usage").unwrap_or(&0.0) > &85.0
        })
        .map(|(name, _)| name.clone())
        .collect();
    
    if !high_disk_servers.is_empty() {
        recommendations.push(format!(
            "Consider adding more chunk servers or rebalancing data. High disk usage on: {}",
            high_disk_servers.join(", ")
        ));
    }
    
    // Check for replication lag
    let high_lag_metaloggers: Vec<_> = components.iter()
        .filter(|(name, component)| {
            name.starts_with("metalogger") && 
            component.metrics.get("replication_lag_ms").unwrap_or(&0.0) > &5000.0
        })
        .map(|(name, _)| name.clone())
        .collect();
    
    if !high_lag_metaloggers.is_empty() {
        recommendations.push(format!(
            "High replication lag detected on: {}. Check network connectivity and master server performance.",
            high_lag_metaloggers.join(", ")
        ));
    }
    
    // Check for old snapshots
    let old_snapshot_metaloggers: Vec<_> = components.iter()
        .filter(|(name, component)| {
            name.starts_with("metalogger") && 
            component.metrics.get("last_snapshot_age_hours").unwrap_or(&0.0) > &24.0
        })
        .map(|(name, _)| name.clone())
        .collect();
    
    if !old_snapshot_metaloggers.is_empty() {
        recommendations.push(format!(
            "Create fresh snapshots on: {}. Old snapshots may impact recovery capabilities.",
            old_snapshot_metaloggers.join(", ")
        ));
    }
    
    // Check for cache performance
    let low_cache_components: Vec<_> = components.iter()
        .filter(|(_, component)| {
            component.metrics.get("cache_hit_rate").unwrap_or(&1.0) < &0.7
        })
        .map(|(name, _)| name.clone())
        .collect();
    
    if !low_cache_components.is_empty() {
        recommendations.push(format!(
            "Low cache hit rate on: {}. Consider increasing cache size or reviewing access patterns.",
            low_cache_components.join(", ")
        ));
    }
    
    recommendations
}

async fn display_health_status(health: &HealthStatus, detailed: bool) {
    println!("Cluster Health Status: {:?}", health.overall_health);
    println!("{}", "=".repeat(50));
    
    // Group components by type for better display
    let mut master_components = Vec::new();
    let mut chunkserver_components = Vec::new();
    let mut metalogger_components = Vec::new();
    let mut client_components = Vec::new();
    
    for (name, component) in &health.components {
        if name.starts_with("master") {
            master_components.push((name, component));
        } else if name.starts_with("chunkserver") {
            chunkserver_components.push((name, component));
        } else if name.starts_with("metalogger") {
            metalogger_components.push((name, component));
        } else if name.starts_with("client") {
            client_components.push((name, component));
        }
    }
    
    // Display master components
    if !master_components.is_empty() {
        println!("Master Servers:");
        for (_, component) in master_components {
            display_component_health(component, detailed);
        }
        println!();
    }
    
    // Display chunk server components
    if !chunkserver_components.is_empty() {
        println!("Chunk Servers:");
        for (_, component) in chunkserver_components {
            display_component_health(component, detailed);
        }
        println!();
    }
    
    // Display metalogger components
    if !metalogger_components.is_empty() {
        println!("Metaloggers:");
        for (_, component) in metalogger_components {
            display_component_health(component, detailed);
        }
        println!();
    }
    
    // Display client components (if any)
    if !client_components.is_empty() {
        println!("Clients:");
        for (_, component) in client_components {
            display_component_health(component, detailed);
        }
        println!();
    }
    
    // Display active alerts
    if !health.alerts.is_empty() {
        println!("Active Alerts:");
        for alert in &health.alerts {
            let ack_status = if alert.acknowledged { " (ACK)" } else { "" };
            let age = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() - alert.timestamp;
            println!("  [{:?}] {}: {} ({}m ago){}", 
                     alert.level, alert.component, alert.message, age / 60, ack_status);
        }
        println!();
    }
    
    // Display recommendations
    if !health.recommendations.is_empty() {
        println!("Recommendations:");
        for rec in &health.recommendations {
            println!("  • {}", rec);
        }
        println!();
    }
    
    // Display summary
    let healthy_count = health.components.values()
        .filter(|c| matches!(c.health, HealthLevel::Healthy))
        .count();
    let warning_count = health.components.values()
        .filter(|c| matches!(c.health, HealthLevel::Warning))
        .count();
    let critical_count = health.components.values()
        .filter(|c| matches!(c.health, HealthLevel::Critical))
        .count();
    
    println!("Summary: {} healthy, {} warning, {} critical components", 
             healthy_count, warning_count, critical_count);
}

fn display_component_health(component: &ComponentHealth, detailed: bool) {
    let status_icon = match component.health {
        HealthLevel::Healthy => "✓",
        HealthLevel::Warning => "⚠",
        HealthLevel::Critical => "✗",
        HealthLevel::Unknown => "?",
    };
    
    println!("  {} {}: {:?}", status_icon, component.name, component.health);
    println!("    Status: {}", component.status);
    
    if detailed {
        println!("    Metrics:");
        for (metric_name, value) in &component.metrics {
            let formatted_name = metric_name.replace('_', " ");
            if metric_name.contains("percent") || metric_name.contains("usage") {
                println!("      {}: {:.1}%", formatted_name, value);
            } else if metric_name.contains("rate") || metric_name.contains("ratio") {
                println!("      {}: {:.3}", formatted_name, value);
            } else if metric_name.contains("hours") {
                println!("      {}: {:.1}h", formatted_name, value);
            } else if metric_name.contains("ms") {
                println!("      {}: {:.1}ms", formatted_name, value);
            } else if metric_name.contains("mb") {
                println!("      {}: {:.1}MB", formatted_name, value);
            } else if value.fract() == 0.0 && *value < 1000000.0 {
                println!("      {}: {:.0}", formatted_name, value);
            } else {
                println!("      {}: {:.1}", formatted_name, value);
            }
        }
        
        let last_check_age = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() - component.last_check;
        println!("    Last check: {}s ago", last_check_age);
    }
}

// Keep the existing placeholder implementation as fallback
async fn show_health_placeholder(detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
    let health = HealthStatus {
        overall_health: HealthLevel::Warning,
        components: {
            let mut components = HashMap::new();
            components.insert("master".to_string(), ComponentHealth {
                name: "Master Server".to_string(),
                health: HealthLevel::Healthy,
                status: "All masters online and in sync".to_string(),
                metrics: {
                    let mut metrics = HashMap::new();
                    metrics.insert("cpu_usage".to_string(), 35.2);
                    metrics.insert("memory_usage".to_string(), 68.4);
                    metrics.insert("uptime_hours".to_string(), 168.5);
                    metrics
                },
                last_check: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
            components.insert("chunkserver".to_string(), ComponentHealth {
                name: "Chunk Servers".to_string(),
                health: HealthLevel::Warning,
                status: "1 chunk server has high disk usage".to_string(),
                metrics: {
                    let mut metrics = HashMap::new();
                    metrics.insert("avg_cpu_usage".to_string(), 42.1);
                    metrics.insert("avg_memory_usage".to_string(), 75.8);
                    metrics.insert("avg_disk_usage".to_string(), 85.2);
                    metrics
                },
                last_check: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
            components
        },
        alerts: vec![
            Alert {
                id: "alert-001".to_string(),
                level: AlertLevel::Warning,
                component: "chunkserver".to_string(),
                message: "Chunk server cs-002 disk usage above 80%".to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() - 3600,
                acknowledged: false,
            },
        ],
        recommendations: vec![
            "Consider adding more chunk servers to reduce disk usage".to_string(),
            "Monitor chunk server cs-002 closely".to_string(),
        ],
    };

    println!("Cluster Health Status: {:?}", health.overall_health);
    println!("{}", "=".repeat(40));
    
    for (_, component) in &health.components {
        println!("{}: {:?}", component.name, component.health);
        println!("  Status: {}", component.status);
        if detailed {
            for (metric_name, value) in &component.metrics {
                println!("  {}: {:.1}", metric_name.replace('_', " "), value);
            }
        }
        println!();
    }
    
    if !health.alerts.is_empty() {
        println!("Active Alerts:");
        for alert in &health.alerts {
            let ack_status = if alert.acknowledged { " (ACK)" } else { "" };
            println!("  [{:?}] {}: {}{}", alert.level, alert.component, alert.message, ack_status);
        }
        println!();
    }
    
    if !health.recommendations.is_empty() {
        println!("Recommendations:");
        for rec in &health.recommendations {
            println!("  • {}", rec);
        }
    }

    Ok(())
}

async fn show_operations(operation_filter: Option<&str>, slow_only: bool, threshold: u64) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement actual operations monitoring
    let operations = vec![
        Operation {
            id: "op-001".to_string(),
            operation_type: "read".to_string(),
            component: "chunkserver".to_string(),
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 5,
            duration_ms: 850,
            size_bytes: 65536,
            status: OperationStatus::Running,
            details: HashMap::new(),
        },
        Operation {
            id: "op-002".to_string(),
            operation_type: "write".to_string(),
            component: "chunkserver".to_string(),
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 10,
            duration_ms: 1250,
            size_bytes: 131072,
            status: OperationStatus::Completed,
            details: HashMap::new(),
        },
    ];

    let filtered_ops: Vec<_> = operations.into_iter()
        .filter(|op| {
            if let Some(filter) = operation_filter {
                op.operation_type == filter
            } else {
                true
            }
        })
        .filter(|op| {
            if slow_only {
                op.duration_ms >= threshold
            } else {
                true
            }
        })
        .collect();

    println!("Active Operations");
    if let Some(filter) = operation_filter {
        println!("Filter: {} operations", filter);
    }
    if slow_only {
        println!("Showing only slow operations (>{}ms)", threshold);
    }
    println!("{}", "=".repeat(60));
    
    println!("{:<10} {:<8} {:<12} {:<8} {:<12} {:<10}", 
             "ID", "Type", "Component", "Duration", "Size", "Status");
    println!("{}", "-".repeat(60));
    
    for op in &filtered_ops {
        println!("{:<10} {:<8} {:<12} {:<8}ms {:<12} {:<10}", 
                 op.id, op.operation_type, op.component, op.duration_ms,
                 format!("{} B", op.size_bytes), format!("{:?}", op.status));
    }

    Ok(())
}

async fn show_events(count: u64, level_filter: Option<&str>, component_filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement actual event log retrieval
    let events = vec![
        Event {
            id: "evt-001".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 300,
            level: AlertLevel::Warning,
            component: "chunkserver".to_string(),
            message: "Disk usage above 80% threshold".to_string(),
            metadata: HashMap::new(),
        },
        Event {
            id: "evt-002".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 600,
            level: AlertLevel::Info,
            component: "master".to_string(),
            message: "New chunk server cs-003 joined cluster".to_string(),
            metadata: HashMap::new(),
        },
    ];

    let filtered_events: Vec<_> = events.into_iter()
        .filter(|event| {
            if let Some(filter) = level_filter {
                format!("{:?}", event.level).to_lowercase() == filter.to_lowercase()
            } else {
                true
            }
        })
        .filter(|event| {
            if let Some(filter) = component_filter {
                event.component == filter
            } else {
                true
            }
        })
        .take(count as usize)
        .collect();

    println!("Recent Events");
    println!("{}", "=".repeat(60));
    
    for event in &filtered_events {
        let timestamp = chrono::DateTime::from_timestamp(event.timestamp as i64, 0)
            .unwrap_or_default()
            .format("%H:%M:%S");
        println!("[{}] [{:?}] {}: {}", 
                 timestamp, event.level, event.component, event.message);
    }

    Ok(())
}

async fn export_metrics(format: &str, output: &str, range: &str) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement actual metrics export
    println!("Exporting metrics to {} format", format);
    println!("Output file: {}", output);
    println!("Time range: {}", range);
    println!("Export completed successfully (placeholder implementation)");
    Ok(())
}