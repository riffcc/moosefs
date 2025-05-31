//! Server load monitoring for MooseNG health checks
//! 
//! Provides comprehensive server load monitoring including:
//! - CPU usage tracking
//! - Memory utilization monitoring  
//! - Network statistics
//! - Process information
//! - Load average monitoring

use std::collections::HashMap;
use std::time::{Duration, SystemTime, Instant};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, warn, error};

/// CPU usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub overall_usage_percent: f64,
    pub per_core_usage: Vec<f64>,
    pub load_average_1min: f64,
    pub load_average_5min: f64,
    pub load_average_15min: f64,
    pub context_switches_per_sec: f64,
    pub interrupts_per_sec: f64,
    pub cpu_cores: usize,
    pub last_updated: SystemTime,
}

/// Memory usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f64,
    pub buffers_bytes: u64,
    pub cached_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_usage_percent: f64,
    pub shared_bytes: u64,
    pub last_updated: SystemTime,
}

/// Network interface statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub bytes_received: u64,
    pub bytes_transmitted: u64,
    pub packets_received: u64,
    pub packets_transmitted: u64,
    pub errors_received: u64,
    pub errors_transmitted: u64,
    pub drops_received: u64,
    pub drops_transmitted: u64,
    pub rx_rate_bytes_per_sec: f64,
    pub tx_rate_bytes_per_sec: f64,
    pub is_up: bool,
}

/// Process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_usage_percent: f64,
    pub threads: u32,
    pub open_files: u32,
    pub command_line: String,
    pub start_time: SystemTime,
    pub status: String,
}

/// Overall server load information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLoadInfo {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub network_interfaces: HashMap<String, NetworkInterface>,
    pub top_processes: Vec<ProcessInfo>,
    pub total_processes: u32,
    pub uptime_seconds: u64,
    pub last_updated: SystemTime,
}

/// Load monitoring configuration
#[derive(Debug, Clone)]
pub struct LoadMonitorConfig {
    /// CPU usage warning threshold (percentage)
    pub cpu_warning_threshold: f64,
    /// CPU usage critical threshold (percentage)
    pub cpu_critical_threshold: f64,
    /// Memory usage warning threshold (percentage)
    pub memory_warning_threshold: f64,
    /// Memory usage critical threshold (percentage)
    pub memory_critical_threshold: f64,
    /// Load average warning threshold (relative to CPU cores)
    pub load_warning_multiplier: f64,
    /// Load average critical threshold (relative to CPU cores)
    pub load_critical_multiplier: f64,
    /// Number of top processes to track
    pub top_processes_count: usize,
    /// Monitoring interval
    pub monitoring_interval: Duration,
    /// Process name filters to monitor
    pub process_filters: Vec<String>,
}

impl Default for LoadMonitorConfig {
    fn default() -> Self {
        Self {
            cpu_warning_threshold: 80.0,
            cpu_critical_threshold: 95.0,
            memory_warning_threshold: 85.0,
            memory_critical_threshold: 95.0,
            load_warning_multiplier: 1.5,
            load_critical_multiplier: 2.0,
            top_processes_count: 10,
            monitoring_interval: Duration::from_secs(30),
            process_filters: vec![
                "mooseng".to_string(),
                "rust".to_string(),
                "postgres".to_string(),
                "redis".to_string(),
            ],
        }
    }
}

/// Server load monitor
pub struct LoadMonitor {
    config: LoadMonitorConfig,
    previous_cpu_stats: Option<CpuStats>,
    previous_network_stats: HashMap<String, NetworkStats>,
    last_reading: Option<ServerLoadInfo>,
}

/// Internal CPU statistics for calculations
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
    timestamp: Instant,
}

/// Internal network statistics for rate calculations
#[derive(Debug, Clone)]
struct NetworkStats {
    rx_bytes: u64,
    tx_bytes: u64,
    timestamp: Instant,
}

impl LoadMonitor {
    /// Create a new load monitor
    pub fn new(config: LoadMonitorConfig) -> Self {
        Self {
            config,
            previous_cpu_stats: None,
            previous_network_stats: HashMap::new(),
            last_reading: None,
        }
    }
    
    /// Collect current server load information
    pub async fn collect_load_info(&mut self) -> Result<ServerLoadInfo> {
        let start_time = Instant::now();
        
        // Collect CPU information
        let cpu_info = self.collect_cpu_info().await?;
        
        // Collect memory information
        let memory_info = self.collect_memory_info().await?;
        
        // Collect network information
        let network_interfaces = self.collect_network_info().await?;
        
        // Collect process information
        let (top_processes, total_processes) = self.collect_process_info().await?;
        
        // Get system uptime
        let uptime_seconds = self.get_system_uptime().await?;
        
        let load_info = ServerLoadInfo {
            cpu: cpu_info,
            memory: memory_info,
            network_interfaces,
            top_processes,
            total_processes,
            uptime_seconds,
            last_updated: SystemTime::now(),
        };
        
        debug!("Load info collection took {:?}", start_time.elapsed());
        self.last_reading = Some(load_info.clone());
        
        Ok(load_info)
    }
    
    /// Collect CPU information
    async fn collect_cpu_info(&mut self) -> Result<CpuInfo> {
        #[cfg(unix)]
        {
            self.collect_unix_cpu_info().await
        }
        
        #[cfg(not(unix))]
        {
            self.collect_fallback_cpu_info().await
        }
    }
    
    /// Collect CPU information on Unix-like systems
    #[cfg(unix)]
    async fn collect_unix_cpu_info(&mut self) -> Result<CpuInfo> {
        // Read /proc/stat for CPU information
        let stat_content = fs::read_to_string("/proc/stat").await?;
        let loadavg_content = fs::read_to_string("/proc/loadavg").await?;
        
        let mut lines = stat_content.lines();
        let cpu_line = lines.next()
            .ok_or_else(|| anyhow::anyhow!("Failed to read CPU stats from /proc/stat"))?;
        
        // Parse overall CPU stats
        let cpu_parts: Vec<&str> = cpu_line.split_whitespace().collect();
        if cpu_parts.len() < 11 {
            return Err(anyhow::anyhow!("Invalid CPU stats format"));
        }
        
        let current_stats = CpuStats {
            user: cpu_parts[1].parse()?,
            nice: cpu_parts[2].parse()?,
            system: cpu_parts[3].parse()?,
            idle: cpu_parts[4].parse()?,
            iowait: cpu_parts[5].parse()?,
            irq: cpu_parts[6].parse()?,
            softirq: cpu_parts[7].parse()?,
            steal: cpu_parts[8].parse()?,
            guest: cpu_parts[9].parse()?,
            guest_nice: cpu_parts[10].parse()?,
            timestamp: Instant::now(),
        };
        
        // Calculate CPU usage percentage
        let overall_usage_percent = if let Some(ref prev_stats) = self.previous_cpu_stats {
            self.calculate_cpu_usage(&current_stats, prev_stats)
        } else {
            0.0 // First reading, no previous data
        };
        
        // Collect per-core usage
        let mut per_core_usage = Vec::new();
        for line in lines {
            if line.starts_with("cpu") && line.chars().nth(3).map_or(false, |c| c.is_ascii_digit()) {
                // This is a per-CPU line
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 11 {
                    // Calculate per-core usage (simplified)
                    let user: u64 = parts[1].parse().unwrap_or(0);
                    let system: u64 = parts[3].parse().unwrap_or(0);
                    let idle: u64 = parts[4].parse().unwrap_or(0);
                    let total = user + system + idle;
                    let usage = if total > 0 {
                        ((user + system) as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };
                    per_core_usage.push(usage);
                }
            }
        }
        
        // Parse load averages
        let loadavg_parts: Vec<&str> = loadavg_content.split_whitespace().collect();
        let load_average_1min: f64 = loadavg_parts.get(0).unwrap_or(&"0.0").parse().unwrap_or(0.0);
        let load_average_5min: f64 = loadavg_parts.get(1).unwrap_or(&"0.0").parse().unwrap_or(0.0);
        let load_average_15min: f64 = loadavg_parts.get(2).unwrap_or(&"0.0").parse().unwrap_or(0.0);
        
        // Get number of CPU cores
        let cpu_cores = num_cpus::get();
        
        // Get context switches and interrupts (simplified)
        let context_switches_per_sec = 0.0; // Would need /proc/stat history
        let interrupts_per_sec = 0.0; // Would need /proc/interrupts history
        
        self.previous_cpu_stats = Some(current_stats);
        
        Ok(CpuInfo {
            overall_usage_percent,
            per_core_usage,
            load_average_1min,
            load_average_5min,
            load_average_15min,
            context_switches_per_sec,
            interrupts_per_sec,
            cpu_cores,
            last_updated: SystemTime::now(),
        })
    }
    
    /// Calculate CPU usage between two measurements
    fn calculate_cpu_usage(&self, current: &CpuStats, previous: &CpuStats) -> f64 {
        let total_current = current.user + current.nice + current.system + current.idle + 
                           current.iowait + current.irq + current.softirq + current.steal;
        let total_previous = previous.user + previous.nice + previous.system + previous.idle + 
                            previous.iowait + previous.irq + previous.softirq + previous.steal;
        
        let total_diff = total_current.saturating_sub(total_previous);
        let idle_diff = current.idle.saturating_sub(previous.idle);
        
        if total_diff == 0 {
            return 0.0;
        }
        
        let used_diff = total_diff.saturating_sub(idle_diff);
        (used_diff as f64 / total_diff as f64) * 100.0
    }
    
    /// Fallback CPU information for non-Unix systems
    #[cfg(not(unix))]
    async fn collect_fallback_cpu_info(&mut self) -> Result<CpuInfo> {
        Ok(CpuInfo {
            overall_usage_percent: 50.0, // Placeholder
            per_core_usage: vec![50.0; num_cpus::get()],
            load_average_1min: 1.0,
            load_average_5min: 1.0,
            load_average_15min: 1.0,
            context_switches_per_sec: 0.0,
            interrupts_per_sec: 0.0,
            cpu_cores: num_cpus::get(),
            last_updated: SystemTime::now(),
        })
    }
    
    /// Collect memory information
    async fn collect_memory_info(&self) -> Result<MemoryInfo> {
        #[cfg(unix)]
        {
            self.collect_unix_memory_info().await
        }
        
        #[cfg(not(unix))]
        {
            self.collect_fallback_memory_info().await
        }
    }
    
    /// Collect memory information on Unix-like systems
    #[cfg(unix)]
    async fn collect_unix_memory_info(&self) -> Result<MemoryInfo> {
        let meminfo_content = fs::read_to_string("/proc/meminfo").await?;
        
        let mut memory_values = HashMap::new();
        for line in meminfo_content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0].trim_end_matches(':');
                let value: u64 = parts[1].parse().unwrap_or(0) * 1024; // Convert KB to bytes
                memory_values.insert(key, value);
            }
        }
        
        let total_bytes = memory_values.get("MemTotal").copied().unwrap_or(0);
        let available_bytes = memory_values.get("MemAvailable")
            .or_else(|| memory_values.get("MemFree"))
            .copied()
            .unwrap_or(0);
        let buffers_bytes = memory_values.get("Buffers").copied().unwrap_or(0);
        let cached_bytes = memory_values.get("Cached").copied().unwrap_or(0);
        let shared_bytes = memory_values.get("Shmem").copied().unwrap_or(0);
        
        let used_bytes = total_bytes.saturating_sub(available_bytes);
        let usage_percent = if total_bytes > 0 {
            (used_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };
        
        let swap_total_bytes = memory_values.get("SwapTotal").copied().unwrap_or(0);
        let swap_free_bytes = memory_values.get("SwapFree").copied().unwrap_or(0);
        let swap_used_bytes = swap_total_bytes.saturating_sub(swap_free_bytes);
        let swap_usage_percent = if swap_total_bytes > 0 {
            (swap_used_bytes as f64 / swap_total_bytes as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(MemoryInfo {
            total_bytes,
            available_bytes,
            used_bytes,
            usage_percent,
            buffers_bytes,
            cached_bytes,
            swap_total_bytes,
            swap_used_bytes,
            swap_usage_percent,
            shared_bytes,
            last_updated: SystemTime::now(),
        })
    }
    
    /// Fallback memory information for non-Unix systems
    #[cfg(not(unix))]
    async fn collect_fallback_memory_info(&self) -> Result<MemoryInfo> {
        Ok(MemoryInfo {
            total_bytes: 8_000_000_000, // 8GB placeholder
            available_bytes: 4_000_000_000, // 4GB placeholder
            used_bytes: 4_000_000_000, // 4GB placeholder
            usage_percent: 50.0,
            buffers_bytes: 0,
            cached_bytes: 0,
            swap_total_bytes: 0,
            swap_used_bytes: 0,
            swap_usage_percent: 0.0,
            shared_bytes: 0,
            last_updated: SystemTime::now(),
        })
    }
    
    /// Collect network interface information
    async fn collect_network_info(&mut self) -> Result<HashMap<String, NetworkInterface>> {
        #[cfg(unix)]
        {
            self.collect_unix_network_info().await
        }
        
        #[cfg(not(unix))]
        {
            self.collect_fallback_network_info().await
        }
    }
    
    /// Collect network information on Unix-like systems
    #[cfg(unix)]
    async fn collect_unix_network_info(&mut self) -> Result<HashMap<String, NetworkInterface>> {
        let netdev_content = fs::read_to_string("/proc/net/dev").await?;
        let mut interfaces = HashMap::new();
        let current_time = Instant::now();
        
        for line in netdev_content.lines().skip(2) { // Skip header lines
            let line = line.trim();
            if let Some(colon_pos) = line.find(':') {
                let interface_name = line[..colon_pos].trim().to_string();
                let stats = &line[colon_pos + 1..];
                let parts: Vec<&str> = stats.split_whitespace().collect();
                
                if parts.len() >= 16 {
                    let bytes_received: u64 = parts[0].parse().unwrap_or(0);
                    let packets_received: u64 = parts[1].parse().unwrap_or(0);
                    let errors_received: u64 = parts[2].parse().unwrap_or(0);
                    let drops_received: u64 = parts[3].parse().unwrap_or(0);
                    
                    let bytes_transmitted: u64 = parts[8].parse().unwrap_or(0);
                    let packets_transmitted: u64 = parts[9].parse().unwrap_or(0);
                    let errors_transmitted: u64 = parts[10].parse().unwrap_or(0);
                    let drops_transmitted: u64 = parts[11].parse().unwrap_or(0);
                    
                    // Calculate rates if we have previous data
                    let (rx_rate, tx_rate) = if let Some(prev_stats) = self.previous_network_stats.get(&interface_name) {
                        let time_diff = current_time.duration_since(prev_stats.timestamp).as_secs_f64();
                        if time_diff > 0.0 {
                            let rx_diff = bytes_received.saturating_sub(prev_stats.rx_bytes) as f64;
                            let tx_diff = bytes_transmitted.saturating_sub(prev_stats.tx_bytes) as f64;
                            (rx_diff / time_diff, tx_diff / time_diff)
                        } else {
                            (0.0, 0.0)
                        }
                    } else {
                        (0.0, 0.0)
                    };
                    
                    // Store current stats for next iteration
                    self.previous_network_stats.insert(interface_name.clone(), NetworkStats {
                        rx_bytes: bytes_received,
                        tx_bytes: bytes_transmitted,
                        timestamp: current_time,
                    });
                    
                    // Check if interface is up (simplified)
                    let is_up = !interface_name.starts_with("lo") && bytes_received > 0;
                    
                    interfaces.insert(interface_name.clone(), NetworkInterface {
                        name: interface_name,
                        bytes_received,
                        bytes_transmitted,
                        packets_received,
                        packets_transmitted,
                        errors_received,
                        errors_transmitted,
                        drops_received,
                        drops_transmitted,
                        rx_rate_bytes_per_sec: rx_rate,
                        tx_rate_bytes_per_sec: tx_rate,
                        is_up,
                    });
                }
            }
        }
        
        Ok(interfaces)
    }
    
    /// Fallback network information for non-Unix systems
    #[cfg(not(unix))]
    async fn collect_fallback_network_info(&mut self) -> Result<HashMap<String, NetworkInterface>> {
        let mut interfaces = HashMap::new();
        
        interfaces.insert("eth0".to_string(), NetworkInterface {
            name: "eth0".to_string(),
            bytes_received: 1_000_000,
            bytes_transmitted: 500_000,
            packets_received: 1000,
            packets_transmitted: 800,
            errors_received: 0,
            errors_transmitted: 0,
            drops_received: 0,
            drops_transmitted: 0,
            rx_rate_bytes_per_sec: 1000.0,
            tx_rate_bytes_per_sec: 500.0,
            is_up: true,
        });
        
        Ok(interfaces)
    }
    
    /// Collect process information
    async fn collect_process_info(&self) -> Result<(Vec<ProcessInfo>, u32)> {
        #[cfg(unix)]
        {
            self.collect_unix_process_info().await
        }
        
        #[cfg(not(unix))]
        {
            self.collect_fallback_process_info().await
        }
    }
    
    /// Collect process information on Unix-like systems
    #[cfg(unix)]
    async fn collect_unix_process_info(&self) -> Result<(Vec<ProcessInfo>, u32)> {
        // This is a simplified implementation
        // In production, you would use a more comprehensive process monitoring library
        
        let mut processes = Vec::new();
        let mut total_processes = 0u32;
        
        // Read /proc directory for process information
        let mut proc_dir = fs::read_dir("/proc").await?;
        
        while let Some(entry) = proc_dir.next_entry().await? {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            
            // Check if this is a process directory (numeric)
            if let Ok(pid) = name_str.parse::<u32>() {
                total_processes += 1;
                
                // Try to read process information
                if let Ok(process_info) = self.read_process_info(pid).await {
                    // Filter processes if needed
                    if self.config.process_filters.is_empty() || 
                       self.config.process_filters.iter().any(|filter| process_info.name.contains(filter)) {
                        processes.push(process_info);
                    }
                }
            }
        }
        
        // Sort by CPU usage and take top N processes
        processes.sort_by(|a, b| b.cpu_usage_percent.partial_cmp(&a.cpu_usage_percent).unwrap_or(std::cmp::Ordering::Equal));
        processes.truncate(self.config.top_processes_count);
        
        Ok((processes, total_processes))
    }
    
    /// Read information for a specific process
    #[cfg(unix)]
    async fn read_process_info(&self, pid: u32) -> Result<ProcessInfo> {
        let stat_path = format!("/proc/{}/stat", pid);
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        let status_path = format!("/proc/{}/status", pid);
        
        let stat_content = fs::read_to_string(&stat_path).await?;
        let cmdline_content = fs::read_to_string(&cmdline_path).await.unwrap_or_default();
        let status_content = fs::read_to_string(&status_path).await.unwrap_or_default();
        
        let stat_parts: Vec<&str> = stat_content.split_whitespace().collect();
        if stat_parts.len() < 23 {
            return Err(anyhow::anyhow!("Invalid stat format for PID {}", pid));
        }
        
        // Extract process name (remove parentheses)
        let name = stat_parts[1].trim_start_matches('(').trim_end_matches(')').to_string();
        
        // Parse command line
        let command_line = cmdline_content.replace('\0', " ").trim().to_string();
        let command_line = if command_line.is_empty() {
            format!("[{}]", name)
        } else {
            command_line
        };
        
        // Extract CPU time (simplified)
        let utime: u64 = stat_parts[13].parse().unwrap_or(0);
        let stime: u64 = stat_parts[14].parse().unwrap_or(0);
        let cpu_usage_percent = 0.0; // Would need historical data for accurate calculation
        
        // Extract memory information
        let memory_usage_bytes = 0u64; // Would parse from /proc/pid/status
        let memory_usage_percent = 0.0;
        
        // Extract thread count
        let threads: u32 = stat_parts[19].parse().unwrap_or(1);
        
        // Extract start time
        let start_time = SystemTime::now(); // Would calculate from boot time + start time
        
        let status = "Running".to_string(); // Would parse from status file
        
        Ok(ProcessInfo {
            pid,
            name,
            cpu_usage_percent,
            memory_usage_bytes,
            memory_usage_percent,
            threads,
            open_files: 0, // Would count files in /proc/pid/fd
            command_line,
            start_time,
            status,
        })
    }
    
    /// Fallback process information for non-Unix systems
    #[cfg(not(unix))]
    async fn collect_fallback_process_info(&self) -> Result<(Vec<ProcessInfo>, u32)> {
        let process = ProcessInfo {
            pid: 1234,
            name: "mooseng-master".to_string(),
            cpu_usage_percent: 25.0,
            memory_usage_bytes: 100_000_000,
            memory_usage_percent: 1.25,
            threads: 4,
            open_files: 50,
            command_line: "/usr/bin/mooseng-master --config /etc/mooseng/master.toml".to_string(),
            start_time: SystemTime::now() - Duration::from_secs(3600),
            status: "Running".to_string(),
        };
        
        Ok((vec![process], 1))
    }
    
    /// Get system uptime
    async fn get_system_uptime(&self) -> Result<u64> {
        #[cfg(unix)]
        {
            let uptime_content = fs::read_to_string("/proc/uptime").await?;
            let uptime_str = uptime_content.split_whitespace().next()
                .ok_or_else(|| anyhow::anyhow!("Invalid uptime format"))?;
            let uptime_seconds: f64 = uptime_str.parse()?;
            Ok(uptime_seconds as u64)
        }
        
        #[cfg(not(unix))]
        {
            Ok(3600) // 1 hour placeholder
        }
    }
    
    /// Get the last reading
    pub fn get_last_reading(&self) -> Option<&ServerLoadInfo> {
        self.last_reading.as_ref()
    }
    
    /// Check if server load is critical
    pub fn is_critical(&self, load_info: &ServerLoadInfo) -> bool {
        load_info.cpu.overall_usage_percent >= self.config.cpu_critical_threshold ||
        load_info.memory.usage_percent >= self.config.memory_critical_threshold ||
        load_info.cpu.load_average_1min >= (load_info.cpu.cpu_cores as f64 * self.config.load_critical_multiplier)
    }
    
    /// Check if server load is in warning state
    pub fn is_warning(&self, load_info: &ServerLoadInfo) -> bool {
        load_info.cpu.overall_usage_percent >= self.config.cpu_warning_threshold ||
        load_info.memory.usage_percent >= self.config.memory_warning_threshold ||
        load_info.cpu.load_average_1min >= (load_info.cpu.cpu_cores as f64 * self.config.load_warning_multiplier)
    }
    
    /// Get load health summary
    pub fn get_health_summary(&self, load_info: &ServerLoadInfo) -> String {
        if self.is_critical(load_info) {
            format!(
                "CRITICAL: CPU {:.1}%, Memory {:.1}%, Load {:.2}",
                load_info.cpu.overall_usage_percent,
                load_info.memory.usage_percent,
                load_info.cpu.load_average_1min
            )
        } else if self.is_warning(load_info) {
            format!(
                "WARNING: CPU {:.1}%, Memory {:.1}%, Load {:.2}",
                load_info.cpu.overall_usage_percent,
                load_info.memory.usage_percent,
                load_info.cpu.load_average_1min
            )
        } else {
            format!(
                "HEALTHY: CPU {:.1}%, Memory {:.1}%, Load {:.2}",
                load_info.cpu.overall_usage_percent,
                load_info.memory.usage_percent,
                load_info.cpu.load_average_1min
            )
        }
    }
    
    /// Get recommendations for load management
    pub fn get_recommendations(&self, load_info: &ServerLoadInfo) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if load_info.cpu.overall_usage_percent >= self.config.cpu_critical_threshold {
            recommendations.push("URGENT: CPU usage is critically high - consider scaling or load balancing".to_string());
        } else if load_info.cpu.overall_usage_percent >= self.config.cpu_warning_threshold {
            recommendations.push("Monitor CPU usage closely - may need optimization".to_string());
        }
        
        if load_info.memory.usage_percent >= self.config.memory_critical_threshold {
            recommendations.push("URGENT: Memory usage is critically high - add more RAM or optimize memory usage".to_string());
        } else if load_info.memory.usage_percent >= self.config.memory_warning_threshold {
            recommendations.push("Monitor memory usage - consider optimizing or adding RAM".to_string());
        }
        
        let load_critical_threshold = load_info.cpu.cpu_cores as f64 * self.config.load_critical_multiplier;
        let load_warning_threshold = load_info.cpu.cpu_cores as f64 * self.config.load_warning_multiplier;
        
        if load_info.cpu.load_average_1min >= load_critical_threshold {
            recommendations.push("URGENT: Load average is critically high - system is overloaded".to_string());
        } else if load_info.cpu.load_average_1min >= load_warning_threshold {
            recommendations.push("Load average is elevated - monitor system performance".to_string());
        }
        
        if load_info.memory.swap_usage_percent > 50.0 {
            recommendations.push("High swap usage detected - consider adding more RAM".to_string());
        }
        
        if recommendations.is_empty() {
            recommendations.push("Server load is within normal parameters".to_string());
        }
        
        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_load_monitor_creation() {
        let config = LoadMonitorConfig::default();
        let monitor = LoadMonitor::new(config);
        
        assert!(monitor.last_reading.is_none());
        assert!(monitor.previous_cpu_stats.is_none());
    }
    
    #[test]
    fn test_health_evaluation() {
        let config = LoadMonitorConfig::default();
        let monitor = LoadMonitor::new(config);
        
        let load_info = ServerLoadInfo {
            cpu: CpuInfo {
                overall_usage_percent: 90.0,
                per_core_usage: vec![85.0, 95.0],
                load_average_1min: 2.5,
                load_average_5min: 2.0,
                load_average_15min: 1.5,
                context_switches_per_sec: 1000.0,
                interrupts_per_sec: 500.0,
                cpu_cores: 2,
                last_updated: SystemTime::now(),
            },
            memory: MemoryInfo {
                total_bytes: 8_000_000_000,
                available_bytes: 1_000_000_000,
                used_bytes: 7_000_000_000,
                usage_percent: 87.5,
                buffers_bytes: 100_000_000,
                cached_bytes: 500_000_000,
                swap_total_bytes: 1_000_000_000,
                swap_used_bytes: 100_000_000,
                swap_usage_percent: 10.0,
                shared_bytes: 50_000_000,
                last_updated: SystemTime::now(),
            },
            network_interfaces: HashMap::new(),
            top_processes: Vec::new(),
            total_processes: 150,
            uptime_seconds: 86400,
            last_updated: SystemTime::now(),
        };
        
        assert!(monitor.is_critical(&load_info));
        let summary = monitor.get_health_summary(&load_info);
        assert!(summary.contains("CRITICAL"));
        
        let recommendations = monitor.get_recommendations(&load_info);
        assert!(!recommendations.is_empty());
        assert!(recommendations.iter().any(|r| r.contains("CPU")));
        assert!(recommendations.iter().any(|r| r.contains("Memory")));
    }
    
    #[test]
    fn test_cpu_usage_calculation() {
        let monitor = LoadMonitor::new(LoadMonitorConfig::default());
        
        let prev_stats = CpuStats {
            user: 1000,
            nice: 0,
            system: 500,
            idle: 8500,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
            guest: 0,
            guest_nice: 0,
            timestamp: Instant::now(),
        };
        
        let current_stats = CpuStats {
            user: 1100,
            nice: 0,
            system: 550,
            idle: 8850,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
            guest: 0,
            guest_nice: 0,
            timestamp: Instant::now(),
        };
        
        let usage = monitor.calculate_cpu_usage(&current_stats, &prev_stats);
        assert!(usage >= 0.0 && usage <= 100.0);
    }
}