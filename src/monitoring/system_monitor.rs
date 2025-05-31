//! System monitoring for MooseNG health checks
//!
//! Provides comprehensive system resource monitoring including:
//! - CPU usage and load averages
//! - Memory utilization
//! - Network I/O statistics
//! - Process monitoring
//! - System temperature (where available)

use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, warn, error};

/// System resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub load_average_1m: f64,
    pub load_average_5m: f64,
    pub load_average_15m: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub memory_usage_percent: f64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_usage_percent: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub uptime_seconds: u64,
    pub process_count: u32,
    pub thread_count: u32,
    pub last_updated: SystemTime,
}

/// System monitoring configuration
#[derive(Debug, Clone)]
pub struct SystemMonitorConfig {
    /// CPU usage sampling interval
    pub cpu_sample_interval: Duration,
    /// Enable network monitoring
    pub monitor_network: bool,
    /// Enable disk I/O monitoring
    pub monitor_disk_io: bool,
    /// Enable process monitoring
    pub monitor_processes: bool,
    /// Critical CPU threshold
    pub cpu_critical_threshold: f64,
    /// Warning CPU threshold
    pub cpu_warning_threshold: f64,
    /// Critical memory threshold
    pub memory_critical_threshold: f64,
    /// Warning memory threshold
    pub memory_warning_threshold: f64,
    /// Critical load average threshold (relative to CPU count)
    pub load_critical_multiplier: f64,
    /// Warning load average threshold (relative to CPU count)
    pub load_warning_multiplier: f64,
}

impl Default for SystemMonitorConfig {
    fn default() -> Self {
        Self {
            cpu_sample_interval: Duration::from_millis(1000),
            monitor_network: true,
            monitor_disk_io: true,
            monitor_processes: true,
            cpu_critical_threshold: 90.0,
            cpu_warning_threshold: 75.0,
            memory_critical_threshold: 95.0,
            memory_warning_threshold: 85.0,
            load_critical_multiplier: 2.0,
            load_warning_multiplier: 1.5,
        }
    }
}

/// System resource monitor
pub struct SystemMonitor {
    config: SystemMonitorConfig,
    cpu_count: u32,
    last_cpu_stats: Option<CpuStats>,
    last_metrics: Option<SystemMetrics>,
}

/// CPU statistics for usage calculation
#[derive(Debug, Clone)]
struct CpuStats {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    guest: u64,
    guest_nice: u64,
    timestamp: SystemTime,
}

impl CpuStats {
    /// Calculate total CPU time
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + 
        self.irq + self.softirq + self.steal + self.guest + self.guest_nice
    }
    
    /// Calculate idle CPU time
    fn idle_time(&self) -> u64 {
        self.idle + self.iowait
    }
    
    /// Calculate active CPU time
    fn active_time(&self) -> u64 {
        self.total() - self.idle_time()
    }
}

impl SystemMonitor {
    /// Create a new system monitor
    pub async fn new(config: SystemMonitorConfig) -> Result<Self> {
        let cpu_count = Self::get_cpu_count().await?;
        
        Ok(Self {
            config,
            cpu_count,
            last_cpu_stats: None,
            last_metrics: None,
        })
    }
    
    /// Get the number of CPU cores
    async fn get_cpu_count() -> Result<u32> {
        #[cfg(unix)]
        {
            let output = Command::new("nproc").output().await?;
            let count_str = String::from_utf8(output.stdout)?;
            let count = count_str.trim().parse::<u32>().unwrap_or(1);
            Ok(count)
        }
        
        #[cfg(not(unix))]
        {
            // Fallback for non-Unix systems
            Ok(std::thread::available_parallelism()?.get() as u32)
        }
    }
    
    /// Collect all system metrics
    pub async fn collect_metrics(&mut self) -> Result<SystemMetrics> {
        let start_time = SystemTime::now();
        
        // Collect various metrics concurrently
        let (
            cpu_usage,
            load_averages,
            memory_info,
            network_stats,
            disk_io_stats,
            uptime,
            process_info,
        ) = tokio::try_join!(
            self.get_cpu_usage(),
            Self::get_load_averages(),
            Self::get_memory_info(),
            self.get_network_stats(),
            Self::get_disk_io_stats(),
            Self::get_uptime(),
            Self::get_process_info(),
        )?;
        
        let metrics = SystemMetrics {
            cpu_usage_percent: cpu_usage,
            load_average_1m: load_averages.0,
            load_average_5m: load_averages.1,
            load_average_15m: load_averages.2,
            memory_total_bytes: memory_info.total,
            memory_used_bytes: memory_info.used,
            memory_available_bytes: memory_info.available,
            memory_usage_percent: memory_info.usage_percent,
            swap_total_bytes: memory_info.swap_total,
            swap_used_bytes: memory_info.swap_used,
            swap_usage_percent: memory_info.swap_usage_percent,
            network_rx_bytes: network_stats.rx_bytes,
            network_tx_bytes: network_stats.tx_bytes,
            disk_read_bytes: disk_io_stats.read_bytes,
            disk_write_bytes: disk_io_stats.write_bytes,
            uptime_seconds: uptime,
            process_count: process_info.process_count,
            thread_count: process_info.thread_count,
            last_updated: start_time,
        };
        
        self.last_metrics = Some(metrics.clone());
        Ok(metrics)
    }
    
    /// Get CPU usage percentage
    async fn get_cpu_usage(&mut self) -> Result<f64> {
        let current_stats = self.parse_cpu_stats().await?;
        
        let usage_percent = if let Some(ref last_stats) = self.last_cpu_stats {
            let current_active = current_stats.active_time();
            let current_total = current_stats.total();
            let last_active = last_stats.active_time();
            let last_total = last_stats.total();
            
            let active_diff = current_active.saturating_sub(last_active);
            let total_diff = current_total.saturating_sub(last_total);
            
            if total_diff > 0 {
                (active_diff as f64 / total_diff as f64) * 100.0
            } else {
                0.0
            }
        } else {
            // First reading, can't calculate usage yet
            0.0
        };
        
        self.last_cpu_stats = Some(current_stats);
        Ok(usage_percent)
    }
    
    /// Parse CPU statistics from /proc/stat
    async fn parse_cpu_stats(&self) -> Result<CpuStats> {
        let stat_content = fs::read_to_string("/proc/stat").await?;
        let cpu_line = stat_content
            .lines()
            .find(|line| line.starts_with("cpu "))
            .ok_or_else(|| anyhow::anyhow!("CPU line not found in /proc/stat"))?;
        
        let fields: Vec<&str> = cpu_line.split_whitespace().collect();
        if fields.len() < 8 {
            return Err(anyhow::anyhow!("Invalid CPU line format"));
        }
        
        Ok(CpuStats {
            user: fields[1].parse()?,
            nice: fields[2].parse()?,
            system: fields[3].parse()?,
            idle: fields[4].parse()?,
            iowait: fields[5].parse()?,
            irq: fields[6].parse()?,
            softirq: fields[7].parse()?,
            steal: fields.get(8).unwrap_or(&"0").parse().unwrap_or(0),
            guest: fields.get(9).unwrap_or(&"0").parse().unwrap_or(0),
            guest_nice: fields.get(10).unwrap_or(&"0").parse().unwrap_or(0),
            timestamp: SystemTime::now(),
        })
    }
    
    /// Get system load averages
    async fn get_load_averages() -> Result<(f64, f64, f64)> {
        let loadavg_content = fs::read_to_string("/proc/loadavg").await?;
        let fields: Vec<&str> = loadavg_content.split_whitespace().collect();
        
        if fields.len() < 3 {
            return Err(anyhow::anyhow!("Invalid loadavg format"));
        }
        
        Ok((
            fields[0].parse()?,
            fields[1].parse()?,
            fields[2].parse()?,
        ))
    }
    
    /// Get memory information
    async fn get_memory_info() -> Result<MemoryInfo> {
        let meminfo_content = fs::read_to_string("/proc/meminfo").await?;
        let mut memory_info = MemoryInfo::default();
        
        for line in meminfo_content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value_str = value.trim().replace(" kB", "");
                if let Ok(value_kb) = value_str.parse::<u64>() {
                    let value_bytes = value_kb * 1024;
                    
                    match key {
                        "MemTotal" => memory_info.total = value_bytes,
                        "MemAvailable" => memory_info.available = value_bytes,
                        "SwapTotal" => memory_info.swap_total = value_bytes,
                        "SwapFree" => memory_info.swap_free = value_bytes,
                        _ => {}
                    }
                }
            }
        }
        
        memory_info.used = memory_info.total.saturating_sub(memory_info.available);
        memory_info.usage_percent = if memory_info.total > 0 {
            (memory_info.used as f64 / memory_info.total as f64) * 100.0
        } else {
            0.0
        };
        
        memory_info.swap_used = memory_info.swap_total.saturating_sub(memory_info.swap_free);
        memory_info.swap_usage_percent = if memory_info.swap_total > 0 {
            (memory_info.swap_used as f64 / memory_info.swap_total as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(memory_info)
    }
    
    /// Get network statistics
    async fn get_network_stats(&self) -> Result<NetworkStats> {
        if !self.config.monitor_network {
            return Ok(NetworkStats::default());
        }
        
        let netdev_content = fs::read_to_string("/proc/net/dev").await?;
        let mut total_rx = 0u64;
        let mut total_tx = 0u64;
        
        for line in netdev_content.lines().skip(2) { // Skip header lines
            if let Some((interface, stats)) = line.split_once(':') {
                let interface = interface.trim();
                
                // Skip loopback interface
                if interface == "lo" {
                    continue;
                }
                
                let fields: Vec<&str> = stats.split_whitespace().collect();
                if fields.len() >= 9 {
                    if let (Ok(rx_bytes), Ok(tx_bytes)) = (fields[0].parse::<u64>(), fields[8].parse::<u64>()) {
                        total_rx += rx_bytes;
                        total_tx += tx_bytes;
                    }
                }
            }
        }
        
        Ok(NetworkStats {
            rx_bytes: total_rx,
            tx_bytes: total_tx,
        })
    }
    
    /// Get disk I/O statistics
    async fn get_disk_io_stats() -> Result<DiskIoStats> {
        let diskstats_content = fs::read_to_string("/proc/diskstats").await?;
        let mut total_read_bytes = 0u64;
        let mut total_write_bytes = 0u64;
        
        for line in diskstats_content.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 14 {
                let device_name = fields[2];
                
                // Only consider physical devices (not partitions)
                if device_name.chars().last().unwrap_or('0').is_alphabetic() {
                    if let (Ok(read_sectors), Ok(write_sectors)) = (fields[5].parse::<u64>(), fields[9].parse::<u64>()) {
                        // Convert sectors to bytes (assuming 512 bytes per sector)
                        total_read_bytes += read_sectors * 512;
                        total_write_bytes += write_sectors * 512;
                    }
                }
            }
        }
        
        Ok(DiskIoStats {
            read_bytes: total_read_bytes,
            write_bytes: total_write_bytes,
        })
    }
    
    /// Get system uptime
    async fn get_uptime() -> Result<u64> {
        let uptime_content = fs::read_to_string("/proc/uptime").await?;
        let uptime_str = uptime_content
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Invalid uptime format"))?;
        
        let uptime_seconds = uptime_str.parse::<f64>()? as u64;
        Ok(uptime_seconds)
    }
    
    /// Get process information
    async fn get_process_info() -> Result<ProcessInfo> {
        let stat_content = fs::read_to_string("/proc/stat").await?;
        let mut process_count = 0u32;
        let mut thread_count = 0u32;
        
        for line in stat_content.lines() {
            if line.starts_with("processes ") {
                if let Some(count_str) = line.split_whitespace().nth(1) {
                    process_count = count_str.parse().unwrap_or(0);
                }
            } else if line.starts_with("procs_running ") {
                if let Some(count_str) = line.split_whitespace().nth(1) {
                    thread_count = count_str.parse().unwrap_or(0);
                }
            }
        }
        
        Ok(ProcessInfo {
            process_count,
            thread_count,
        })
    }
    
    /// Check if system is in critical state
    pub fn is_critical(&self) -> bool {
        if let Some(ref metrics) = self.last_metrics {
            metrics.cpu_usage_percent > self.config.cpu_critical_threshold ||
            metrics.memory_usage_percent > self.config.memory_critical_threshold ||
            metrics.load_average_1m > (self.cpu_count as f64 * self.config.load_critical_multiplier)
        } else {
            false
        }
    }
    
    /// Check if system is in warning state
    pub fn is_warning(&self) -> bool {
        if let Some(ref metrics) = self.last_metrics {
            metrics.cpu_usage_percent > self.config.cpu_warning_threshold ||
            metrics.memory_usage_percent > self.config.memory_warning_threshold ||
            metrics.load_average_1m > (self.cpu_count as f64 * self.config.load_warning_multiplier)
        } else {
            false
        }
    }
    
    /// Get health summary
    pub fn get_health_summary(&self) -> String {
        if let Some(ref metrics) = self.last_metrics {
            if self.is_critical() {
                "CRITICAL: System resources under severe load".to_string()
            } else if self.is_warning() {
                "WARNING: System resources under moderate load".to_string()
            } else {
                "HEALTHY: System resources operating normally".to_string()
            }
        } else {
            "UNKNOWN: No system metrics available".to_string()
        }
    }
    
    /// Get the latest metrics
    pub fn get_latest_metrics(&self) -> Option<&SystemMetrics> {
        self.last_metrics.as_ref()
    }
    
    /// Convert metrics to Prometheus format
    pub fn to_prometheus_metrics(&self) -> String {
        if let Some(ref metrics) = self.last_metrics {
            format!(
                "# HELP mooseng_cpu_usage_percent Current CPU usage percentage\n\
                # TYPE mooseng_cpu_usage_percent gauge\n\
                mooseng_cpu_usage_percent {}\n\
                # HELP mooseng_load_average_1m Load average over 1 minute\n\
                # TYPE mooseng_load_average_1m gauge\n\
                mooseng_load_average_1m {}\n\
                # HELP mooseng_memory_usage_percent Current memory usage percentage\n\
                # TYPE mooseng_memory_usage_percent gauge\n\
                mooseng_memory_usage_percent {}\n\
                # HELP mooseng_memory_total_bytes Total system memory in bytes\n\
                # TYPE mooseng_memory_total_bytes gauge\n\
                mooseng_memory_total_bytes {}\n\
                # HELP mooseng_uptime_seconds System uptime in seconds\n\
                # TYPE mooseng_uptime_seconds counter\n\
                mooseng_uptime_seconds {}\n",
                metrics.cpu_usage_percent,
                metrics.load_average_1m,
                metrics.memory_usage_percent,
                metrics.memory_total_bytes,
                metrics.uptime_seconds
            )
        } else {
            String::new()
        }
    }
}

/// Memory information structure
#[derive(Debug, Default)]
struct MemoryInfo {
    total: u64,
    used: u64,
    available: u64,
    usage_percent: f64,
    swap_total: u64,
    swap_used: u64,
    swap_free: u64,
    swap_usage_percent: f64,
}

/// Network statistics structure
#[derive(Debug, Default)]
struct NetworkStats {
    rx_bytes: u64,
    tx_bytes: u64,
}

/// Disk I/O statistics structure
#[derive(Debug, Default)]
struct DiskIoStats {
    read_bytes: u64,
    write_bytes: u64,
}

/// Process information structure
#[derive(Debug, Default)]
struct ProcessInfo {
    process_count: u32,
    thread_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_system_monitor_creation() {
        let config = SystemMonitorConfig::default();
        let result = SystemMonitor::new(config).await;
        assert!(result.is_ok());
        
        let monitor = result.unwrap();
        assert!(monitor.cpu_count > 0);
    }
    
    #[test]
    fn test_cpu_stats_calculations() {
        let stats = CpuStats {
            user: 1000,
            nice: 100,
            system: 500,
            idle: 8000,
            iowait: 200,
            irq: 50,
            softirq: 50,
            steal: 0,
            guest: 0,
            guest_nice: 0,
            timestamp: SystemTime::now(),
        };
        
        assert_eq!(stats.total(), 9900);
        assert_eq!(stats.idle_time(), 8200);
        assert_eq!(stats.active_time(), 1700);
    }
    
    #[tokio::test]
    async fn test_load_averages_parsing() {
        // This test would need a mock /proc/loadavg file
        // In a real environment, this would test the actual parsing
    }
    
    #[test]
    fn test_system_metrics_health_checks() {
        let config = SystemMonitorConfig::default();
        let mut monitor = SystemMonitor {
            config: config.clone(),
            cpu_count: 4,
            last_cpu_stats: None,
            last_metrics: Some(SystemMetrics {
                cpu_usage_percent: 80.0,  // Above warning threshold
                memory_usage_percent: 70.0, // Below warning threshold
                load_average_1m: 2.0,     // Normal load
                load_average_5m: 1.8,
                load_average_15m: 1.5,
                memory_total_bytes: 8_000_000_000,
                memory_used_bytes: 5_600_000_000,
                memory_available_bytes: 2_400_000_000,
                swap_total_bytes: 2_000_000_000,
                swap_used_bytes: 0,
                swap_usage_percent: 0.0,
                network_rx_bytes: 1_000_000,
                network_tx_bytes: 500_000,
                disk_read_bytes: 10_000_000,
                disk_write_bytes: 5_000_000,
                uptime_seconds: 3600,
                process_count: 200,
                thread_count: 10,
                last_updated: SystemTime::now(),
            }),
        };
        
        assert!(monitor.is_warning());
        assert!(!monitor.is_critical());
        
        // Test critical state
        if let Some(ref mut metrics) = monitor.last_metrics {
            metrics.cpu_usage_percent = 95.0; // Above critical threshold
        }
        
        assert!(monitor.is_critical());
    }
}