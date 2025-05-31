//! System resource monitoring for MooseNG health checks
//! 
//! This module provides comprehensive system monitoring capabilities including:
//! - CPU usage and load averages
//! - Memory utilization and swap usage  
//! - Network I/O and connection statistics
//! - Process counts and resource limits

use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::time::{interval, Instant};
use tracing::{debug, error, warn};

/// System monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMonitorConfig {
    /// Monitoring interval in seconds
    pub interval_seconds: u64,
    /// CPU usage sampling window
    pub cpu_window_seconds: u64,
    /// Memory usage thresholds
    pub memory_warning_percent: f64,
    pub memory_critical_percent: f64,
    /// Load average thresholds
    pub load_warning_ratio: f64,
    pub load_critical_ratio: f64,
    /// Enable detailed process monitoring
    pub enable_process_monitoring: bool,
}

impl Default for SystemMonitorConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 30,
            cpu_window_seconds: 60,
            memory_warning_percent: 85.0,
            memory_critical_percent: 95.0,
            load_warning_ratio: 2.0,  // 2x number of CPU cores
            load_critical_ratio: 4.0, // 4x number of CPU cores
            enable_process_monitoring: true,
        }
    }
}

/// System resource monitor
pub struct SystemMonitor {
    config: SystemMonitorConfig,
    cpu_cores: u32,
    last_cpu_stats: Option<CpuStats>,
    last_measurement_time: Option<Instant>,
}

/// CPU statistics from /proc/stat
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
}

/// Memory statistics from /proc/meminfo
#[derive(Debug, Clone)]
struct MemoryStats {
    total: u64,
    available: u64,
    free: u64,
    buffers: u64,
    cached: u64,
    swap_total: u64,
    swap_free: u64,
}

/// Network interface statistics
#[derive(Debug, Clone)]
struct NetworkStats {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_packets: u64,
    tx_packets: u64,
    rx_errors: u64,
    tx_errors: u64,
}

/// Process information
#[derive(Debug, Clone, Serialize)]
struct ProcessInfo {
    pid: u32,
    name: String,
    cpu_percent: f64,
    memory_kb: u64,
    state: String,
}

impl SystemMonitor {
    /// Create a new system monitor
    pub fn new(config: SystemMonitorConfig) -> Result<Self> {
        let cpu_cores = Self::get_cpu_core_count()?;
        
        Ok(Self {
            config,
            cpu_cores,
            last_cpu_stats: None,
            last_measurement_time: None,
        })
    }

    /// Check system load and resource utilization
    pub async fn check_system_load(&self, _window: Duration) -> Result<HashMap<String, f64>> {
        let mut metrics = HashMap::new();
        
        // Get CPU usage
        if let Ok(cpu_percent) = self.get_cpu_usage().await {
            metrics.insert("cpu_percent".to_string(), cpu_percent);
        }
        
        // Get load averages
        if let Ok(load_averages) = self.get_load_averages().await {
            metrics.insert("load_average_1m".to_string(), load_averages.0);
            metrics.insert("load_average_5m".to_string(), load_averages.1);
            metrics.insert("load_average_15m".to_string(), load_averages.2);
            
            // Calculate load ratios relative to CPU core count
            metrics.insert("load_ratio_1m".to_string(), load_averages.0 / self.cpu_cores as f64);
            metrics.insert("load_ratio_5m".to_string(), load_averages.1 / self.cpu_cores as f64);
            metrics.insert("load_ratio_15m".to_string(), load_averages.2 / self.cpu_cores as f64);
        }
        
        // Get memory usage
        if let Ok(memory_stats) = self.get_memory_stats().await {
            let memory_used = memory_stats.total - memory_stats.available;
            let memory_percent = (memory_used as f64 / memory_stats.total as f64) * 100.0;
            
            metrics.insert("memory_percent".to_string(), memory_percent);
            metrics.insert("memory_total_gb".to_string(), memory_stats.total as f64 / 1024.0 / 1024.0 / 1024.0);
            metrics.insert("memory_used_gb".to_string(), memory_used as f64 / 1024.0 / 1024.0 / 1024.0);
            metrics.insert("memory_available_gb".to_string(), memory_stats.available as f64 / 1024.0 / 1024.0 / 1024.0);
            
            // Swap usage
            if memory_stats.swap_total > 0 {
                let swap_used = memory_stats.swap_total - memory_stats.swap_free;
                let swap_percent = (swap_used as f64 / memory_stats.swap_total as f64) * 100.0;
                metrics.insert("swap_percent".to_string(), swap_percent);
                metrics.insert("swap_used_gb".to_string(), swap_used as f64 / 1024.0 / 1024.0 / 1024.0);
            }
        }
        
        // Get network statistics
        if let Ok(network_stats) = self.get_network_stats().await {
            metrics.insert("network_rx_mb".to_string(), network_stats.rx_bytes as f64 / 1024.0 / 1024.0);
            metrics.insert("network_tx_mb".to_string(), network_stats.tx_bytes as f64 / 1024.0 / 1024.0);
            metrics.insert("network_rx_errors".to_string(), network_stats.rx_errors as f64);
            metrics.insert("network_tx_errors".to_string(), network_stats.tx_errors as f64);
        }
        
        // Get process counts
        if let Ok((process_count, thread_count)) = self.get_process_counts().await {
            metrics.insert("process_count".to_string(), process_count as f64);
            metrics.insert("thread_count".to_string(), thread_count as f64);
        }
        
        // System uptime
        if let Ok(uptime) = self.get_system_uptime().await {
            metrics.insert("uptime_seconds".to_string(), uptime);
        }
        
        // File descriptor usage
        if let Ok(fd_usage) = self.get_file_descriptor_usage().await {
            metrics.insert("file_descriptors_used".to_string(), fd_usage.0 as f64);
            metrics.insert("file_descriptors_max".to_string(), fd_usage.1 as f64);
            metrics.insert("file_descriptors_percent".to_string(), 
                (fd_usage.0 as f64 / fd_usage.1 as f64) * 100.0);
        }
        
        debug!("System load check completed with {} metrics", metrics.len());
        Ok(metrics)
    }

    /// Get current CPU usage percentage
    async fn get_cpu_usage(&self) -> Result<f64> {
        let current_stats = self.read_cpu_stats().await?;
        
        // We need previous stats to calculate usage
        if let Some(ref last_stats) = self.last_cpu_stats {
            let total_diff = (current_stats.user + current_stats.nice + current_stats.system + 
                            current_stats.idle + current_stats.iowait + current_stats.irq + 
                            current_stats.softirq + current_stats.steal) -
                           (last_stats.user + last_stats.nice + last_stats.system + 
                            last_stats.idle + last_stats.iowait + last_stats.irq + 
                            last_stats.softirq + last_stats.steal);
            
            let idle_diff = current_stats.idle - last_stats.idle;
            
            if total_diff > 0 {
                let cpu_usage = ((total_diff - idle_diff) as f64 / total_diff as f64) * 100.0;
                return Ok(cpu_usage.max(0.0).min(100.0));
            }
        }
        
        // Fallback: estimate from load average
        let load_averages = self.get_load_averages().await?;
        let cpu_usage = (load_averages.0 / self.cpu_cores as f64) * 100.0;
        Ok(cpu_usage.max(0.0).min(100.0))
    }

    /// Get load averages (1, 5, 15 minutes)
    async fn get_load_averages(&self) -> Result<(f64, f64, f64)> {
        let contents = fs::read_to_string("/proc/loadavg")
            .context("Failed to read /proc/loadavg")?;
        
        let parts: Vec<&str> = contents.split_whitespace().collect();
        if parts.len() < 3 {
            anyhow::bail!("Invalid /proc/loadavg format");
        }
        
        let load_1m = parts[0].parse::<f64>().context("Failed to parse 1m load average")?;
        let load_5m = parts[1].parse::<f64>().context("Failed to parse 5m load average")?;
        let load_15m = parts[2].parse::<f64>().context("Failed to parse 15m load average")?;
        
        Ok((load_1m, load_5m, load_15m))
    }

    /// Read CPU statistics from /proc/stat
    async fn read_cpu_stats(&self) -> Result<CpuStats> {
        let contents = fs::read_to_string("/proc/stat")
            .context("Failed to read /proc/stat")?;
        
        let first_line = contents.lines().next()
            .context("Empty /proc/stat file")?;
        
        if !first_line.starts_with("cpu ") {
            anyhow::bail!("Invalid /proc/stat format");
        }
        
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 8 {
            anyhow::bail!("Insufficient CPU stats in /proc/stat");
        }
        
        Ok(CpuStats {
            user: parts[1].parse()?,
            nice: parts[2].parse()?,
            system: parts[3].parse()?,
            idle: parts[4].parse()?,
            iowait: parts[5].parse()?,
            irq: parts[6].parse()?,
            softirq: parts[7].parse()?,
            steal: if parts.len() > 8 { parts[8].parse()? } else { 0 },
        })
    }

    /// Get memory statistics
    async fn get_memory_stats(&self) -> Result<MemoryStats> {
        let contents = fs::read_to_string("/proc/meminfo")
            .context("Failed to read /proc/meminfo")?;
        
        let mut stats = MemoryStats {
            total: 0,
            available: 0,
            free: 0,
            buffers: 0,
            cached: 0,
            swap_total: 0,
            swap_free: 0,
        };
        
        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            
            let value = parts[1].parse::<u64>().unwrap_or(0) * 1024; // Convert KB to bytes
            
            match parts[0] {
                "MemTotal:" => stats.total = value,
                "MemAvailable:" => stats.available = value,
                "MemFree:" => stats.free = value,
                "Buffers:" => stats.buffers = value,
                "Cached:" => stats.cached = value,
                "SwapTotal:" => stats.swap_total = value,
                "SwapFree:" => stats.swap_free = value,
                _ => {}
            }
        }
        
        // If MemAvailable is not present, estimate it
        if stats.available == 0 {
            stats.available = stats.free + stats.buffers + stats.cached;
        }
        
        Ok(stats)
    }

    /// Get network interface statistics
    async fn get_network_stats(&self) -> Result<NetworkStats> {
        let contents = fs::read_to_string("/proc/net/dev")
            .context("Failed to read /proc/net/dev")?;
        
        let mut total_stats = NetworkStats {
            rx_bytes: 0,
            tx_bytes: 0,
            rx_packets: 0,
            tx_packets: 0,
            rx_errors: 0,
            tx_errors: 0,
        };
        
        for line in contents.lines().skip(2) { // Skip header lines
            let line = line.trim();
            if line.is_empty() || line.starts_with("lo:") {
                continue; // Skip loopback interface
            }
            
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 11 {
                continue;
            }
            
            // Network stats format: interface: rx_bytes rx_packets rx_errs ... tx_bytes tx_packets tx_errs ...
            total_stats.rx_bytes += parts[1].parse::<u64>().unwrap_or(0);
            total_stats.rx_packets += parts[2].parse::<u64>().unwrap_or(0);
            total_stats.rx_errors += parts[3].parse::<u64>().unwrap_or(0);
            
            total_stats.tx_bytes += parts[9].parse::<u64>().unwrap_or(0);
            total_stats.tx_packets += parts[10].parse::<u64>().unwrap_or(0);
            total_stats.tx_errors += parts[11].parse::<u64>().unwrap_or(0);
        }
        
        Ok(total_stats)
    }

    /// Get process and thread counts
    async fn get_process_counts(&self) -> Result<(u32, u32)> {
        let contents = fs::read_to_string("/proc/stat")
            .context("Failed to read /proc/stat")?;
        
        let mut processes = 0;
        let mut threads = 0;
        
        for line in contents.lines() {
            if line.starts_with("processes ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    processes = parts[1].parse::<u32>().unwrap_or(0);
                }
            } else if line.starts_with("procs_running ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    threads = parts[1].parse::<u32>().unwrap_or(0);
                }
            }
        }
        
        // If we can't get exact counts, estimate from /proc
        if processes == 0 || threads == 0 {
            let proc_entries = fs::read_dir("/proc")
                .context("Failed to read /proc directory")?;
            
            let mut proc_count = 0;
            for entry in proc_entries {
                if let Ok(entry) = entry {
                    if let Ok(name) = entry.file_name().into_string() {
                        if name.chars().all(|c| c.is_ascii_digit()) {
                            proc_count += 1;
                        }
                    }
                }
            }
            
            if processes == 0 {
                processes = proc_count;
            }
            if threads == 0 {
                threads = proc_count; // Approximate
            }
        }
        
        Ok((processes, threads))
    }

    /// Get system uptime in seconds
    async fn get_system_uptime(&self) -> Result<f64> {
        let contents = fs::read_to_string("/proc/uptime")
            .context("Failed to read /proc/uptime")?;
        
        let parts: Vec<&str> = contents.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Invalid /proc/uptime format");
        }
        
        parts[0].parse::<f64>().context("Failed to parse uptime")
    }

    /// Get file descriptor usage
    async fn get_file_descriptor_usage(&self) -> Result<(u32, u32)> {
        // Get current file descriptor count
        let fd_entries = fs::read_dir("/proc/self/fd")
            .context("Failed to read /proc/self/fd")?;
        let current_fds = fd_entries.count() as u32;
        
        // Get file descriptor limit
        let limits_content = fs::read_to_string("/proc/self/limits")
            .context("Failed to read /proc/self/limits")?;
        
        let mut max_fds = 1024; // Default fallback
        
        for line in limits_content.lines() {
            if line.contains("Max open files") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let Ok(limit) = parts[3].parse::<u32>() {
                        max_fds = limit;
                        break;
                    }
                }
            }
        }
        
        Ok((current_fds, max_fds))
    }

    /// Get number of CPU cores
    fn get_cpu_core_count() -> Result<u32> {
        let contents = fs::read_to_string("/proc/cpuinfo")
            .context("Failed to read /proc/cpuinfo")?;
        
        let core_count = contents.lines()
            .filter(|line| line.starts_with("processor"))
            .count() as u32;
        
        if core_count == 0 {
            anyhow::bail!("Could not determine CPU core count");
        }
        
        Ok(core_count)
    }

    /// Get top processes by CPU usage
    pub async fn get_top_processes(&self, limit: usize) -> Result<Vec<ProcessInfo>> {
        if !self.config.enable_process_monitoring {
            return Ok(Vec::new());
        }
        
        let mut processes = Vec::new();
        
        let proc_entries = fs::read_dir("/proc")
            .context("Failed to read /proc directory")?;
        
        for entry in proc_entries {
            if let Ok(entry) = entry {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(pid) = name.parse::<u32>() {
                            if let Ok(process_info) = self.get_process_info(pid).await {
                                processes.push(process_info);
                            }
                        }
                    }
                }
            }
        }
        
        // Sort by CPU usage
        processes.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));
        processes.truncate(limit);
        
        Ok(processes)
    }

    /// Get information about a specific process
    async fn get_process_info(&self, pid: u32) -> Result<ProcessInfo> {
        let stat_path = format!("/proc/{}/stat", pid);
        let status_path = format!("/proc/{}/status", pid);
        
        let stat_content = fs::read_to_string(&stat_path)
            .context("Failed to read process stat file")?;
        let status_content = fs::read_to_string(&status_path)
            .context("Failed to read process status file")?;
        
        // Parse process name and state from stat
        let stat_parts: Vec<&str> = stat_content.split_whitespace().collect();
        let name = if stat_parts.len() > 1 {
            stat_parts[1].trim_matches(|c| c == '(' || c == ')').to_string()
        } else {
            "unknown".to_string()
        };
        
        let state = if stat_parts.len() > 2 {
            stat_parts[2].to_string()
        } else {
            "unknown".to_string()
        };
        
        // Parse memory usage from status
        let mut memory_kb = 0;
        for line in status_content.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    memory_kb = parts[1].parse::<u64>().unwrap_or(0);
                    break;
                }
            }
        }
        
        // CPU usage calculation would require tracking over time
        // For now, we'll use a placeholder
        let cpu_percent = 0.0;
        
        Ok(ProcessInfo {
            pid,
            name,
            cpu_percent,
            memory_kb,
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_monitor_creation() {
        let config = SystemMonitorConfig::default();
        let monitor = SystemMonitor::new(config).unwrap();
        assert!(monitor.cpu_cores > 0);
    }

    #[tokio::test]
    async fn test_load_averages() {
        let config = SystemMonitorConfig::default();
        let monitor = SystemMonitor::new(config).unwrap();
        
        match monitor.get_load_averages().await {
            Ok((load_1m, load_5m, load_15m)) => {
                assert!(load_1m >= 0.0);
                assert!(load_5m >= 0.0);
                assert!(load_15m >= 0.0);
            }
            Err(_) => {
                // May fail in test environments without /proc
            }
        }
    }

    #[tokio::test]
    async fn test_system_load_check() {
        let config = SystemMonitorConfig::default();
        let monitor = SystemMonitor::new(config).unwrap();
        
        match monitor.check_system_load(Duration::from_secs(300)).await {
            Ok(metrics) => {
                assert!(!metrics.is_empty());
                // Should have at least load average metrics
                assert!(metrics.contains_key("load_average_1m") || 
                       metrics.contains_key("cpu_percent"));
            }
            Err(_) => {
                // May fail in test environments without /proc
            }
        }
    }
}