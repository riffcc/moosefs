//! Configuration management for MooseNG benchmarks

use crate::BenchmarkConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Network interface configuration for benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceConfig {
    pub name: String,
    pub ip_address: String,
    pub bandwidth_mbps: u32,
}

/// Region configuration for multi-region benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionConfig {
    pub name: String,
    pub endpoint: String,
    pub latency_ms: u32,
    pub available_bandwidth_mbps: u32,
}

/// Benchmark environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEnvironmentConfig {
    /// Output directory for benchmark results
    pub output_dir: PathBuf,
    /// Whether to run benchmarks requiring root privileges
    pub enable_privileged_tests: bool,
    /// Network interfaces available for testing
    pub network_interfaces: Vec<NetworkInterfaceConfig>,
    /// Regions available for multi-region testing
    pub regions: Vec<RegionConfig>,
    /// Maximum concurrent benchmark processes
    pub max_concurrency: usize,
    /// Custom environment variables
    pub environment_vars: HashMap<String, String>,
}

impl Default for BenchmarkEnvironmentConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("./benchmark_results"),
            enable_privileged_tests: false,
            network_interfaces: vec![
                NetworkInterfaceConfig {
                    name: "lo".to_string(),
                    ip_address: "127.0.0.1".to_string(),
                    bandwidth_mbps: 1000,
                }
            ],
            regions: vec![
                RegionConfig {
                    name: "local".to_string(),
                    endpoint: "127.0.0.1:8080".to_string(),
                    latency_ms: 1,
                    available_bandwidth_mbps: 1000,
                }
            ],
            max_concurrency: num_cpus::get(),
            environment_vars: HashMap::new(),
        }
    }
}

impl BenchmarkEnvironmentConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::create_dir_all(path.parent().unwrap_or(&PathBuf::from(".")))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the first available network interface
    pub fn primary_interface(&self) -> Option<&NetworkInterfaceConfig> {
        self.network_interfaces.first()
    }

    /// Find a region by name
    pub fn find_region(&self, name: &str) -> Option<&RegionConfig> {
        self.regions.iter().find(|r| r.name == name)
    }
}

/// Configuration source for loading benchmark settings
pub enum ConfigSource {
    File(PathBuf),
    Default,
    Environment,
}

/// Load benchmark configuration from various sources
pub fn load_config(source: ConfigSource) -> Result<BenchmarkConfig, Box<dyn std::error::Error>> {
    match source {
        ConfigSource::File(path) => {
            let content = std::fs::read_to_string(path)?;
            let config: BenchmarkConfig = toml::from_str(&content)?;
            Ok(config)
        }
        ConfigSource::Default => Ok(BenchmarkConfig::default()),
        ConfigSource::Environment => {
            let mut config = BenchmarkConfig::default();
            
            // Override with environment variables if present
            if let Ok(warmup) = std::env::var("MOOSENG_BENCH_WARMUP") {
                config.warmup_iterations = warmup.parse().unwrap_or(config.warmup_iterations);
            }
            
            if let Ok(measurement) = std::env::var("MOOSENG_BENCH_MEASUREMENT") {
                config.measurement_iterations = measurement.parse().unwrap_or(config.measurement_iterations);
            }
            
            if let Ok(detailed) = std::env::var("MOOSENG_BENCH_DETAILED") {
                config.detailed_report = detailed.parse().unwrap_or(config.detailed_report);
            }
            
            Ok(config)
        }
    }
}

/// Save benchmark configuration to file
pub fn save_config(config: &BenchmarkConfig, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = toml::to_string_pretty(config)?;
    std::fs::create_dir_all(path.parent().unwrap_or(&PathBuf::from(".")))?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Database configuration for storing benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:benchmark_results.db".to_string(),
            max_connections: 10,
            timeout_seconds: 30,
        }
    }
}

/// Dashboard configuration for web interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub enabled: bool,
    pub port: u16,
    pub host: String,
    pub realtime_updates: bool,
    pub websocket_port: u16,
}

/// Infrastructure configuration for multi-region deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureConfig {
    pub regions: Vec<RegionConfig>,
    pub container_registry: String,
    pub deployment_timeout_minutes: u32,
    pub health_check_timeout_seconds: u32,
    pub resource_requirements: ResourceRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: u32,
    pub memory_gb: u32,
    pub disk_gb: u32,
    pub network_bandwidth_mbps: u32,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8080,
            host: "localhost".to_string(),
            realtime_updates: true,
            websocket_port: 8081,
        }
    }
}

impl Default for InfrastructureConfig {
    fn default() -> Self {
        Self {
            regions: vec![
                RegionConfig {
                    name: "us-east-1".to_string(),
                    endpoint: "localhost:8080".to_string(),
                    latency_ms: 10,
                    available_bandwidth_mbps: 1000,
                },
            ],
            container_registry: "localhost:5000".to_string(),
            deployment_timeout_minutes: 30,
            health_check_timeout_seconds: 120,
            resource_requirements: ResourceRequirements::default(),
        }
    }
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            cpu_cores: 4,
            memory_gb: 8,
            disk_gb: 100,
            network_bandwidth_mbps: 1000,
        }
    }
}

/// Complete benchmark configuration including all subsystems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteBenchmarkConfig {
    pub benchmark: BenchmarkConfig,
    pub environment: BenchmarkEnvironmentConfig,
    pub database: DatabaseConfig,
    pub dashboard: DashboardConfig,
}

impl Default for CompleteBenchmarkConfig {
    fn default() -> Self {
        Self {
            benchmark: BenchmarkConfig::default(),
            environment: BenchmarkEnvironmentConfig::default(),
            database: DatabaseConfig::default(),
            dashboard: DashboardConfig::default(),
        }
    }
}

// Async functions for the unified runner
pub async fn load_from_file(path: &str) -> Result<BenchmarkConfig, Box<dyn std::error::Error>> {
    let content = tokio::fs::read_to_string(path).await?;
    
    // Try to load as complete config first, fallback to just benchmark config
    if let Ok(complete_config) = toml::from_str::<CompleteBenchmarkConfig>(&content) {
        Ok(complete_config.benchmark)
    } else {
        let config: BenchmarkConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

pub async fn save_to_file(config: &CompleteBenchmarkConfig, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = toml::to_string_pretty(config)?;
    
    // Create parent directory if it doesn't exist
    if let Some(parent) = std::path::Path::new(path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    
    tokio::fs::write(path, content).await?;
    Ok(())
}

pub async fn validate_file(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = tokio::fs::read_to_string(path).await?;
    
    // Try to parse as complete config or just benchmark config
    if toml::from_str::<CompleteBenchmarkConfig>(&content).is_ok() {
        Ok(())
    } else if toml::from_str::<BenchmarkConfig>(&content).is_ok() {
        Ok(())
    } else {
        Err("Invalid configuration format".into())
    }
}

pub async fn load_with_defaults() -> Result<BenchmarkConfig, Box<dyn std::error::Error>> {
    let mut config = BenchmarkConfig::default();
    
    // Override with environment variables if present
    if let Ok(warmup) = std::env::var("MOOSENG_BENCH_WARMUP") {
        config.warmup_iterations = warmup.parse().unwrap_or(config.warmup_iterations);
    }
    
    if let Ok(measurement) = std::env::var("MOOSENG_BENCH_MEASUREMENT") {
        config.measurement_iterations = measurement.parse().unwrap_or(config.measurement_iterations);
    }
    
    if let Ok(detailed) = std::env::var("MOOSENG_BENCH_DETAILED") {
        config.detailed_report = detailed.parse().unwrap_or(config.detailed_report);
    }
    
    Ok(config)
}

pub fn generate_default_config() -> CompleteBenchmarkConfig {
    CompleteBenchmarkConfig::default()
}

pub fn generate_advanced_config() -> CompleteBenchmarkConfig {
    CompleteBenchmarkConfig {
        benchmark: BenchmarkConfig {
            warmup_iterations: 20,
            measurement_iterations: 200,
            file_sizes: vec![
                1024,         // 1KB
                4096,         // 4KB
                65536,        // 64KB
                1048576,      // 1MB
                10485760,     // 10MB
                104857600,    // 100MB
                1073741824,   // 1GB
            ],
            concurrency_levels: vec![1, 5, 10, 25, 50, 100, 200, 500],
            regions: vec![
                "us-east-1".to_string(),
                "us-west-2".to_string(),
                "eu-west-1".to_string(),
                "ap-southeast-1".to_string(),
                "ap-northeast-1".to_string(),
            ],
            detailed_report: true,
        },
        environment: BenchmarkEnvironmentConfig {
            output_dir: PathBuf::from("./benchmark_results"),
            enable_privileged_tests: true,
            network_interfaces: vec![
                NetworkInterfaceConfig {
                    name: "lo".to_string(),
                    ip_address: "127.0.0.1".to_string(),
                    bandwidth_mbps: 1000,
                },
                NetworkInterfaceConfig {
                    name: "eth0".to_string(),
                    ip_address: "10.0.0.1".to_string(),
                    bandwidth_mbps: 10000,
                },
            ],
            regions: vec![
                RegionConfig {
                    name: "us-east-1".to_string(),
                    endpoint: "us-east-1.mooseng.example.com:8080".to_string(),
                    latency_ms: 50,
                    available_bandwidth_mbps: 1000,
                },
                RegionConfig {
                    name: "eu-west-1".to_string(),
                    endpoint: "eu-west-1.mooseng.example.com:8080".to_string(),
                    latency_ms: 150,
                    available_bandwidth_mbps: 500,
                },
                RegionConfig {
                    name: "ap-southeast-1".to_string(),
                    endpoint: "ap-southeast-1.mooseng.example.com:8080".to_string(),
                    latency_ms: 200,
                    available_bandwidth_mbps: 300,
                },
            ],
            max_concurrency: num_cpus::get() * 2,
            environment_vars: HashMap::new(),
        },
        database: DatabaseConfig {
            url: "postgresql://benchmark:password@localhost/mooseng_benchmarks".to_string(),
            max_connections: 50,
            timeout_seconds: 120,
        },
        dashboard: DashboardConfig {
            enabled: true,
            port: 8080,
            host: "0.0.0.0".to_string(),
            realtime_updates: true,
            websocket_port: 8081,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = BenchmarkEnvironmentConfig::default();
        assert!(!config.enable_privileged_tests);
        assert!(config.network_interfaces.len() > 0);
        assert!(config.regions.len() > 0);
    }

    #[test]
    fn test_config_serialization() {
        let config = BenchmarkEnvironmentConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let _deserialized: BenchmarkEnvironmentConfig = toml::from_str(&serialized).unwrap();
    }

    #[test]
    fn test_config_file_operations() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("test_config.toml");
        
        let config = BenchmarkEnvironmentConfig::default();
        config.to_file(&config_path).unwrap();
        
        let loaded_config = BenchmarkEnvironmentConfig::from_file(&config_path).unwrap();
        assert_eq!(config.enable_privileged_tests, loaded_config.enable_privileged_tests);
    }

    #[test]
    fn test_complete_config_generation() {
        let config = generate_default_config();
        assert!(!config.database.url.is_empty());
        assert!(config.dashboard.port > 0);
        
        let advanced_config = generate_advanced_config();
        assert!(advanced_config.benchmark.measurement_iterations > config.benchmark.measurement_iterations);
    }

    #[tokio::test]
    async fn test_async_config_operations() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("async_test_config.toml");
        
        let config = generate_default_config();
        save_to_file(&config, config_path.to_str().unwrap()).await.unwrap();
        
        let loaded_config = load_from_file(config_path.to_str().unwrap()).await.unwrap();
        assert_eq!(config.benchmark.warmup_iterations, loaded_config.warmup_iterations);
        
        validate_file(config_path.to_str().unwrap()).await.unwrap();
    }
}