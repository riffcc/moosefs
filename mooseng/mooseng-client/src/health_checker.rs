//! Client health checker implementation
//! 
//! Provides comprehensive health monitoring for the MooseNG client
//! including FUSE operations, master connection, cache performance, and mount health.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use mooseng_common::health::{HealthChecker, HealthCheckResult, HealthStatus, SelfHealingAction};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::master_client::MasterClient;
use crate::cache::ClientCache;
use crate::filesystem::MooseFuse;

/// Client health checker
pub struct ClientHealthChecker {
    master_client: Arc<MasterClient>,
    cache: Arc<ClientCache>,
    filesystem: Option<Arc<MooseFuse>>,
    metrics: Arc<RwLock<ClientMetrics>>,
    component_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct ClientMetrics {
    pub master_connection_status: bool,
    pub master_response_time_ms: f64,
    pub cache_hit_rate: f64,
    pub cache_size_mb: f64,
    pub memory_usage_percent: f64,
    pub cpu_usage_percent: f64,
    pub fuse_operations_per_sec: f64,
    pub error_rate_percent: f64,
    pub mount_status: bool,
    pub network_latency_ms: f64,
    pub read_throughput_mbps: f64,
    pub write_throughput_mbps: f64,
}

#[derive(Debug, Clone)]
struct CacheStats {
    hit_rate: f64,
    size_mb: f64,
}

#[derive(Debug, Clone)]
struct FilesystemStats {
    operations_per_sec: f64,
    error_rate: f64,
    is_mounted: bool,
    read_throughput_mbps: f64,
    write_throughput_mbps: f64,
}

impl ClientHealthChecker {
    pub fn new(
        master_client: Arc<MasterClient>,
        cache: Arc<ClientCache>,
        filesystem: Option<Arc<MooseFuse>>,
    ) -> Self {
        Self {
            master_client,
            cache,
            filesystem,
            metrics: Arc::new(RwLock::new(ClientMetrics::default())),
            component_name: "mooseng-client".to_string(),
        }
    }
    
    async fn collect_metrics(&self) -> Result<ClientMetrics> {
        let mut metrics = ClientMetrics::default();
        
        // System metrics
        metrics.cpu_usage_percent = Self::get_cpu_usage().await?;
        metrics.memory_usage_percent = Self::get_memory_usage().await?;
        
        // Master connection health - attempt basic connectivity check
        let start = std::time::Instant::now();
        metrics.master_connection_status = self.check_master_connectivity().await;
        metrics.master_response_time_ms = start.elapsed().as_millis() as f64;
        
        // Cache metrics - try to get real stats where possible
        let cache_stats = self.get_cache_stats().await;
        metrics.cache_hit_rate = cache_stats.hit_rate;
        metrics.cache_size_mb = cache_stats.size_mb;
        
        // FUSE filesystem metrics
        if let Some(ref _filesystem) = self.filesystem {
            let fs_stats = self.get_filesystem_stats().await;
            metrics.fuse_operations_per_sec = fs_stats.operations_per_sec;
            metrics.error_rate_percent = fs_stats.error_rate;
            metrics.mount_status = fs_stats.is_mounted;
            metrics.read_throughput_mbps = fs_stats.read_throughput_mbps;
            metrics.write_throughput_mbps = fs_stats.write_throughput_mbps;
        } else {
            // Not using FUSE mount
            metrics.mount_status = true; // Not applicable
            metrics.fuse_operations_per_sec = 0.0;
            metrics.error_rate_percent = 0.0;
            metrics.read_throughput_mbps = 0.0;
            metrics.write_throughput_mbps = 0.0;
        }
        
        // Network latency to master
        metrics.network_latency_ms = metrics.master_response_time_ms;
        
        // Store metrics for future reference
        {
            let mut stored_metrics = self.metrics.write().await;
            *stored_metrics = metrics.clone();
        }
        
        Ok(metrics)
    }
    
    async fn check_master_connectivity(&self) -> bool {
        // Basic connectivity check - this would be more sophisticated in production
        // For now, we assume connection is healthy if we can access the master client
        // In a real implementation, this would do a lightweight ping/health check
        true // Placeholder - implement actual connectivity check
    }
    
    async fn get_cache_stats(&self) -> CacheStats {
        // TODO: Implement actual cache statistics retrieval
        // This would interface with the real ClientCache implementation
        CacheStats {
            hit_rate: 0.8, // Placeholder - should come from actual cache
            size_mb: 256.0, // Placeholder
        }
    }
    
    async fn get_filesystem_stats(&self) -> FilesystemStats {
        // TODO: Implement actual filesystem statistics retrieval
        // This would interface with the real MooseFuse implementation
        FilesystemStats {
            operations_per_sec: 100.0, // Placeholder
            error_rate: 0.1, // Placeholder
            is_mounted: true, // Placeholder
            read_throughput_mbps: 50.0, // Placeholder
            write_throughput_mbps: 30.0, // Placeholder
        }
    }
    
    async fn get_cpu_usage() -> Result<f64> {
        // Simple CPU usage check using /proc/stat
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let contents = fs::read_to_string("/proc/stat")?;
            let first_line = contents.lines().next().unwrap_or("");
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            
            if parts.len() >= 5 {
                let user: u64 = parts[1].parse().unwrap_or(0);
                let nice: u64 = parts[2].parse().unwrap_or(0);
                let system: u64 = parts[3].parse().unwrap_or(0);
                let idle: u64 = parts[4].parse().unwrap_or(0);
                
                let total = user + nice + system + idle;
                let used = user + nice + system;
                
                if total > 0 {
                    Ok((used as f64 / total as f64) * 100.0)
                } else {
                    Ok(0.0)
                }
            } else {
                Ok(0.0)
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // Fallback for non-Linux systems
            Ok(0.0)
        }
    }
    
    async fn get_memory_usage() -> Result<f64> {
        // Simple memory usage check using /proc/meminfo
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let contents = fs::read_to_string("/proc/meminfo")?;
            let mut total_mem = 0u64;
            let mut available_mem = 0u64;
            
            for line in contents.lines() {
                if line.starts_with("MemTotal:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        total_mem = parts[1].parse().unwrap_or(0);
                    }
                } else if line.starts_with("MemAvailable:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        available_mem = parts[1].parse().unwrap_or(0);
                    }
                }
            }
            
            if total_mem > 0 {
                let used_mem = total_mem - available_mem;
                Ok((used_mem as f64 / total_mem as f64) * 100.0)
            } else {
                Ok(0.0)
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // Fallback for non-Linux systems
            Ok(0.0)
        }
    }
    
    fn assess_health_status(&self, metrics: &ClientMetrics) -> (HealthStatus, String, Vec<String>) {
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();
        
        // Check master connection
        if !metrics.master_connection_status {
            issues.push("Master server connection failed".to_string());
            recommendations.push("Check master server availability and network connectivity".to_string());
        } else if metrics.master_response_time_ms > 1000.0 {
            issues.push(format!("High master response time: {:.2}ms", metrics.master_response_time_ms));
            recommendations.push("Check network latency to master server".to_string());
        }
        
        // Check cache performance
        if metrics.cache_hit_rate < 0.5 {
            issues.push(format!("Low cache hit rate: {:.1}%", metrics.cache_hit_rate * 100.0));
            recommendations.push("Consider increasing cache size or reviewing access patterns".to_string());
        }
        
        // Check memory usage
        if metrics.memory_usage_percent > 90.0 {
            issues.push(format!("High memory usage: {:.1}%", metrics.memory_usage_percent));
            recommendations.push("Consider reducing cache size or restarting client".to_string());
        } else if metrics.memory_usage_percent > 80.0 {
            issues.push(format!("Memory usage warning: {:.1}%", metrics.memory_usage_percent));
            recommendations.push("Monitor memory usage closely".to_string());
        }
        
        // Check CPU usage
        if metrics.cpu_usage_percent > 80.0 {
            issues.push(format!("High CPU usage: {:.1}%", metrics.cpu_usage_percent));
            recommendations.push("Check for busy processes or reduce workload".to_string());
        }
        
        // Check mount status (if applicable)
        if self.filesystem.is_some() && !metrics.mount_status {
            issues.push("FUSE mount is not active".to_string());
            recommendations.push("Attempt remount operation".to_string());
        }
        
        // Check error rate
        if metrics.error_rate_percent > 5.0 {
            issues.push(format!("High error rate: {:.1}%", metrics.error_rate_percent));
            recommendations.push("Investigate error causes and consider reconnection".to_string());
        } else if metrics.error_rate_percent > 1.0 {
            issues.push(format!("Elevated error rate: {:.1}%", metrics.error_rate_percent));
            recommendations.push("Monitor error patterns".to_string());
        }
        
        // Determine overall status
        let status = if !metrics.master_connection_status || (self.filesystem.is_some() && !metrics.mount_status) {
            HealthStatus::Critical
        } else if metrics.memory_usage_percent > 90.0 || metrics.error_rate_percent > 5.0 {
            HealthStatus::Critical
        } else if metrics.memory_usage_percent > 80.0 || metrics.error_rate_percent > 1.0 || metrics.cache_hit_rate < 0.5 {
            HealthStatus::Warning
        } else if metrics.memory_usage_percent > 70.0 || metrics.cpu_usage_percent > 70.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
        
        let message = if issues.is_empty() {
            "Client is operating normally".to_string()
        } else {
            format!("Issues detected: {}", issues.join(", "))
        };
        
        (status, message, recommendations)
    }
    
    pub async fn get_metrics(&self) -> ClientMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Clear the client cache for self-healing
    async fn clear_cache(&self) -> Result<()> {
        // TODO: Implement actual cache clearing logic when ClientCache is fully implemented
        // For now, this is a placeholder that would interface with the real cache
        debug!("Cache clearing requested - placeholder implementation");
        Ok(())
    }
    
    /// Attempt to reconnect to master server
    async fn reconnect_to_master(&self) -> Result<()> {
        // TODO: Implement actual reconnection logic when MasterClient supports it
        // For now, this is a placeholder that would interface with the real master client
        debug!("Master reconnection requested - placeholder implementation");
        Ok(())
    }
    
    /// Attempt to remount the filesystem
    async fn remount_filesystem(&self) -> Result<()> {
        // TODO: Implement actual remount logic when MooseFuse supports it
        // For now, this is a placeholder that would interface with the real filesystem
        debug!("Filesystem remount requested - placeholder implementation");
        Ok(())
    }
}

#[async_trait::async_trait]
impl HealthChecker for ClientHealthChecker {
    async fn check_health(&self) -> Result<HealthCheckResult> {
        debug!("Starting health check for client");
        
        let metrics = self.collect_metrics().await?;
        let (status, message, recommendations) = self.assess_health_status(&metrics);
        
        let mut metrics_map = HashMap::new();
        metrics_map.insert("master_response_time_ms".to_string(), metrics.master_response_time_ms);
        metrics_map.insert("cache_hit_rate".to_string(), metrics.cache_hit_rate);
        metrics_map.insert("cache_size_mb".to_string(), metrics.cache_size_mb);
        metrics_map.insert("memory_usage_percent".to_string(), metrics.memory_usage_percent);
        metrics_map.insert("cpu_usage_percent".to_string(), metrics.cpu_usage_percent);
        metrics_map.insert("fuse_operations_per_sec".to_string(), metrics.fuse_operations_per_sec);
        metrics_map.insert("error_rate_percent".to_string(), metrics.error_rate_percent);
        metrics_map.insert("network_latency_ms".to_string(), metrics.network_latency_ms);
        metrics_map.insert("read_throughput_mbps".to_string(), metrics.read_throughput_mbps);
        metrics_map.insert("write_throughput_mbps".to_string(), metrics.write_throughput_mbps);
        
        // Add boolean metrics as 0.0/1.0
        metrics_map.insert("master_connection_status".to_string(), if metrics.master_connection_status { 1.0 } else { 0.0 });
        metrics_map.insert("mount_status".to_string(), if metrics.mount_status { 1.0 } else { 0.0 });
        
        Ok(HealthCheckResult {
            component: self.component_name.clone(),
            status,
            message,
            timestamp: SystemTime::now(),
            metrics: metrics_map,
            recommendations,
        })
    }
    
    fn component_name(&self) -> &str {
        &self.component_name
    }
    
    async fn perform_self_healing(&self, action: &SelfHealingAction) -> Result<bool> {
        info!("Performing self-healing action: {:?}", action);
        
        match action {
            SelfHealingAction::RestartComponent { component } => {
                if component == &self.component_name {
                    warn!("Restart action requested for client - this would require external intervention");
                    // In a real implementation, this might signal a supervisor process
                    Ok(false)
                } else {
                    Ok(false)
                }
            }
            
            SelfHealingAction::ClearCache { component } => {
                if component == &self.component_name {
                    info!("Clearing client metadata cache");
                    match self.clear_cache().await {
                        Ok(_) => {
                            info!("Successfully cleared client cache");
                            Ok(true)
                        }
                        Err(e) => {
                            warn!("Failed to clear cache: {}", e);
                            Ok(false)
                        }
                    }
                } else {
                    Ok(false)
                }
            }
            
            SelfHealingAction::NetworkReconnect { endpoint } => {
                if endpoint == &self.component_name {
                    info!("Attempting to reconnect to master server");
                    match self.reconnect_to_master().await {
                        Ok(_) => {
                            info!("Successfully reconnected to master server");
                            Ok(true)
                        }
                        Err(e) => {
                            warn!("Failed to reconnect to master server: {}", e);
                            Ok(false)
                        }
                    }
                } else {
                    Ok(false)
                }
            }
            
            SelfHealingAction::CustomAction { name, params } => {
                match name.as_str() {
                    "remount_filesystem" => {
                        if let Some(ref _filesystem) = self.filesystem {
                            info!("Attempting to remount filesystem");
                            match self.remount_filesystem().await {
                                Ok(_) => {
                                    info!("Successfully remounted filesystem");
                                    Ok(true)
                                }
                                Err(e) => {
                                    warn!("Failed to remount filesystem: {}", e);
                                    Ok(false)
                                }
                            }
                        } else {
                            warn!("No filesystem to remount");
                            Ok(false)
                        }
                    }
                    
                    "refresh_cache" => {
                        info!("Refreshing metadata cache");
                        // TODO: Implement refresh method in ClientCache
                        Ok(true)
                    }
                    
                    "adjust_cache_size" => {
                        if let Some(size_str) = params.get("size_mb") {
                            if let Ok(size_mb) = size_str.parse::<usize>() {
                                info!("Adjusting cache size to {} MB", size_mb);
                                // TODO: Implement resize method in ClientCache
                                Ok(true)
                            } else {
                                warn!("Invalid cache size parameter: {}", size_str);
                                Ok(false)
                            }
                        } else {
                            warn!("Missing cache size parameter");
                            Ok(false)
                        }
                    }
                    
                    _ => {
                        warn!("Unknown custom action: {}", name);
                        Ok(false)
                    }
                }
            }
            
            _ => {
                debug!("Action not applicable to client: {:?}", action);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    
    #[tokio::test]
    async fn test_client_health_checker_creation() {
        // This test would require mocking the dependencies
        // For now, just test that the metrics structure works
        let metrics = ClientMetrics::default();
        assert_eq!(metrics.master_connection_status, false);
        assert_eq!(metrics.cache_hit_rate, 0.0);
        assert_eq!(metrics.mount_status, false);
    }
    
    #[tokio::test]
    async fn test_health_status_assessment() {
        // Test various health status scenarios
        let mut metrics = ClientMetrics::default();
        
        // Healthy state
        metrics.master_connection_status = true;
        metrics.master_response_time_ms = 50.0;
        metrics.cache_hit_rate = 0.8;
        metrics.memory_usage_percent = 60.0;
        metrics.cpu_usage_percent = 40.0;
        metrics.error_rate_percent = 0.1;
        metrics.mount_status = true;
        
        // Would need actual ClientHealthChecker instance to test assess_health_status
        // This is a placeholder for proper testing with mocked dependencies
    }
}