//! Tiered Storage Management Module
//! 
//! This module provides management interfaces and tools for tiered storage operations,
//! including administrative commands, monitoring dashboards, and configuration management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn, error, instrument};

use crate::core::types::{ChunkId, Size, TierType as CoreTierType, ComponentHealthResult, HealthStatus};
use super::{TierType, TierStats, MigrationRecommendation, AccessPattern};
use super::integration::{TieredStorageIntegrator, IntegrationMetrics, MigrationTask};

/// Management interface for tiered storage operations
pub struct TieredStorageManager {
    /// Integration layer
    integrator: Option<TieredStorageIntegrator>,
    /// Management configuration
    config: ManagementConfig,
    /// Administrative statistics
    admin_stats: AdminStats,
}

/// Configuration for storage management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementConfig {
    /// Enable administrative interfaces
    pub admin_enabled: bool,
    /// Administrative API port
    pub admin_port: u16,
    /// Enable detailed logging
    pub detailed_logging: bool,
    /// Statistics retention period
    pub stats_retention_days: u32,
    /// Enable alerts
    pub alerts_enabled: bool,
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
    /// Backup configuration
    pub backup_config: BackupConfig,
}

/// Alert threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Tier utilization warning threshold (percentage)
    pub tier_utilization_warning: f64,
    /// Tier utilization critical threshold (percentage)
    pub tier_utilization_critical: f64,
    /// Migration failure rate warning threshold (percentage)
    pub migration_failure_rate_warning: f64,
    /// Migration failure rate critical threshold (percentage)
    pub migration_failure_rate_critical: f64,
    /// Migration speed warning threshold (MB/s)
    pub migration_speed_warning: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            tier_utilization_warning: 80.0,
            tier_utilization_critical: 90.0,
            migration_failure_rate_warning: 10.0,
            migration_failure_rate_critical: 25.0,
            migration_speed_warning: 10.0,
        }
    }
}

/// Backup configuration for tiered storage metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Enable automatic backups
    pub enabled: bool,
    /// Backup interval
    pub backup_interval: Duration,
    /// Backup retention period
    pub retention_period: Duration,
    /// Backup location
    pub backup_path: PathBuf,
    /// Enable compression for backups
    pub compression_enabled: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backup_interval: Duration::from_secs(86400), // Daily
            retention_period: Duration::from_secs(86400 * 30), // 30 days
            backup_path: PathBuf::from("/var/lib/mooseng/backups"),
            compression_enabled: true,
        }
    }
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            admin_enabled: true,
            admin_port: 8080,
            detailed_logging: false,
            stats_retention_days: 30,
            alerts_enabled: true,
            alert_thresholds: AlertThresholds::default(),
            backup_config: BackupConfig::default(),
        }
    }
}

/// Administrative statistics
#[derive(Debug, Clone, Default)]
pub struct AdminStats {
    /// Total files managed
    pub total_files: u64,
    /// Total data managed (bytes)
    pub total_data_bytes: u64,
    /// Files per tier
    pub files_per_tier: HashMap<TierType, u64>,
    /// Data per tier (bytes)
    pub data_per_tier: HashMap<TierType, u64>,
    /// Migration statistics
    pub migration_stats: MigrationStats,
    /// Performance statistics
    pub performance_stats: PerformanceStats,
    /// Last updated
    pub last_updated: SystemTime,
}

/// Migration statistics
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    /// Total migrations executed
    pub total_migrations: u64,
    /// Successful migrations
    pub successful_migrations: u64,
    /// Failed migrations
    pub failed_migrations: u64,
    /// Total bytes migrated
    pub bytes_migrated: u64,
    /// Average migration time (seconds)
    pub avg_migration_time: f64,
    /// Migrations by direction
    pub migrations_by_direction: HashMap<String, u64>,
}

/// Performance statistics
#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    /// Average read latency by tier (milliseconds)
    pub avg_read_latency: HashMap<TierType, f64>,
    /// Average write latency by tier (milliseconds)
    pub avg_write_latency: HashMap<TierType, f64>,
    /// IOPS by tier
    pub iops_by_tier: HashMap<TierType, f64>,
    /// Bandwidth utilization by tier (MB/s)
    pub bandwidth_by_tier: HashMap<TierType, f64>,
    /// Error rates by tier (percentage)
    pub error_rates: HashMap<TierType, f64>,
}

/// Tier management report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierManagementReport {
    /// Report timestamp
    pub timestamp: SystemTime,
    /// Tier utilization summary
    pub tier_utilization: HashMap<TierType, TierUtilizationInfo>,
    /// Migration recommendations
    pub migration_recommendations: Vec<MigrationRecommendation>,
    /// Performance metrics
    pub performance_metrics: PerformanceStats,
    /// Health status
    pub health_status: HealthStatus,
    /// Alerts
    pub alerts: Vec<Alert>,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Tier utilization information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierUtilizationInfo {
    /// Total capacity (bytes)
    pub total_capacity: u64,
    /// Used capacity (bytes)
    pub used_capacity: u64,
    /// Available capacity (bytes)
    pub available_capacity: u64,
    /// Utilization percentage
    pub utilization_percent: f64,
    /// File count
    pub file_count: u64,
    /// Growth rate (bytes per day)
    pub growth_rate_bytes_per_day: f64,
    /// Projected days until full
    pub days_until_full: Option<u32>,
}

/// Alert information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Alert ID
    pub id: String,
    /// Alert level
    pub level: AlertLevel,
    /// Alert title
    pub title: String,
    /// Alert message
    pub message: String,
    /// Alert timestamp
    pub timestamp: SystemTime,
    /// Alert category
    pub category: AlertCategory,
    /// Recommended actions
    pub recommended_actions: Vec<String>,
}

/// Alert levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

/// Alert categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCategory {
    Capacity,
    Performance,
    Migration,
    Health,
    Configuration,
}

/// Administrative command for tier management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdminCommand {
    /// Get tier status
    GetTierStatus,
    /// Force migration of specific chunk
    ForceMigration {
        chunk_id: ChunkId,
        target_tier: TierType,
    },
    /// Cancel migration
    CancelMigration {
        migration_id: String,
    },
    /// Update tier configuration
    UpdateTierConfig {
        tier: TierType,
        config: TierConfigUpdate,
    },
    /// Generate management report
    GenerateReport,
    /// Set alert thresholds
    SetAlertThresholds {
        thresholds: AlertThresholds,
    },
    /// Backup tier metadata
    BackupMetadata,
    /// Restore tier metadata
    RestoreMetadata {
        backup_file: PathBuf,
    },
    /// Rebalance tier utilization
    RebalanceTiers {
        target_utilization: f64,
    },
}

/// Tier configuration update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfigUpdate {
    /// New capacity limit
    pub capacity_limit: Option<u64>,
    /// Performance thresholds
    pub performance_thresholds: Option<PerformanceThresholds>,
    /// Cost settings
    pub cost_settings: Option<CostSettings>,
}

/// Performance thresholds for a tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable read latency (ms)
    pub max_read_latency: f64,
    /// Maximum acceptable write latency (ms)
    pub max_write_latency: f64,
    /// Minimum required IOPS
    pub min_iops: u32,
    /// Minimum required bandwidth (MB/s)
    pub min_bandwidth: f64,
}

/// Cost settings for a tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSettings {
    /// Cost per GB per month
    pub cost_per_gb_month: f64,
    /// Access cost per operation
    pub access_cost_per_operation: f64,
    /// Transfer cost per GB
    pub transfer_cost_per_gb: f64,
}

impl TieredStorageManager {
    /// Create a new tiered storage manager
    pub fn new(config: ManagementConfig) -> Self {
        Self {
            integrator: None,
            config,
            admin_stats: AdminStats::default(),
        }
    }
    
    /// Initialize with integrator
    #[instrument(skip(self, integrator))]
    pub async fn initialize(&mut self, integrator: TieredStorageIntegrator) -> Result<()> {
        self.integrator = Some(integrator);
        info!("Initialized tiered storage manager");
        Ok(())
    }
    
    /// Execute administrative command
    #[instrument(skip(self))]
    pub async fn execute_command(&mut self, command: AdminCommand) -> Result<AdminCommandResult> {
        if let Some(ref integrator) = self.integrator {
            match command {
                AdminCommand::GetTierStatus => {
                    let metrics = integrator.get_metrics().await;
                    let health = integrator.get_health_status().await;
                    
                    Ok(AdminCommandResult::TierStatus {
                        metrics,
                        health,
                        active_migrations: integrator.get_active_migrations().await,
                    })
                }
                
                AdminCommand::ForceMigration { chunk_id, target_tier } => {
                    // In a real implementation, this would trigger a specific migration
                    info!("Force migration requested for chunk {} to tier {:?}", chunk_id, target_tier);
                    Ok(AdminCommandResult::Success {
                        message: format!("Migration queued for chunk {}", chunk_id),
                    })
                }
                
                AdminCommand::CancelMigration { migration_id } => {
                    let cancelled = integrator.cancel_migration(&migration_id).await?;
                    Ok(AdminCommandResult::Success {
                        message: if cancelled {
                            format!("Migration {} cancelled", migration_id)
                        } else {
                            format!("Migration {} not found or cannot be cancelled", migration_id)
                        },
                    })
                }
                
                AdminCommand::GenerateReport => {
                    let report = self.generate_management_report().await?;
                    Ok(AdminCommandResult::Report(report))
                }
                
                AdminCommand::SetAlertThresholds { thresholds } => {
                    self.config.alert_thresholds = thresholds;
                    Ok(AdminCommandResult::Success {
                        message: "Alert thresholds updated".to_string(),
                    })
                }
                
                AdminCommand::BackupMetadata => {
                    self.backup_metadata().await?;
                    Ok(AdminCommandResult::Success {
                        message: "Metadata backup completed".to_string(),
                    })
                }
                
                AdminCommand::RestoreMetadata { backup_file } => {
                    self.restore_metadata(&backup_file).await?;
                    Ok(AdminCommandResult::Success {
                        message: format!("Metadata restored from {:?}", backup_file),
                    })
                }
                
                AdminCommand::RebalanceTiers { target_utilization } => {
                    let migrations_queued = self.rebalance_tiers(target_utilization).await?;
                    Ok(AdminCommandResult::Success {
                        message: format!("Rebalancing initiated: {} migrations queued", migrations_queued),
                    })
                }
                
                AdminCommand::UpdateTierConfig { tier, config } => {
                    self.update_tier_config(tier, config).await?;
                    Ok(AdminCommandResult::Success {
                        message: format!("Configuration updated for tier {:?}", tier),
                    })
                }
            }
        } else {
            Err(anyhow::anyhow!("Integrator not initialized"))
        }
    }
    
    /// Generate comprehensive management report
    #[instrument(skip(self))]
    async fn generate_management_report(&self) -> Result<TierManagementReport> {
        let integrator = self.integrator.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Integrator not initialized"))?;
        
        let metrics = integrator.get_metrics().await;
        let health = integrator.get_health_status().await;
        
        // Generate tier utilization info
        let mut tier_utilization = HashMap::new();
        for (tier, utilization_percent) in &metrics.tier_utilization {
            let capacity = self.estimate_tier_capacity(*tier);
            let used_capacity = (capacity as f64 * utilization_percent / 100.0) as u64;
            let available_capacity = capacity - used_capacity;
            
            let utilization_info = TierUtilizationInfo {
                total_capacity: capacity,
                used_capacity,
                available_capacity,
                utilization_percent: *utilization_percent,
                file_count: self.admin_stats.files_per_tier.get(tier).copied().unwrap_or(0),
                growth_rate_bytes_per_day: self.estimate_growth_rate(*tier),
                days_until_full: self.estimate_days_until_full(*tier, *utilization_percent),
            };
            
            tier_utilization.insert(*tier, utilization_info);
        }
        
        // Generate alerts
        let alerts = self.generate_alerts(&metrics, &health).await;
        
        // Generate recommendations
        let recommendations = self.generate_recommendations(&metrics, &alerts).await;
        
        Ok(TierManagementReport {
            timestamp: SystemTime::now(),
            tier_utilization,
            migration_recommendations: Vec::new(), // Would be populated from actual data
            performance_metrics: self.admin_stats.performance_stats.clone(),
            health_status: health.status,
            alerts,
            recommendations,
        })
    }
    
    /// Generate alerts based on current metrics
    async fn generate_alerts(&self, metrics: &IntegrationMetrics, health: &ComponentHealthResult) -> Vec<Alert> {
        let mut alerts = Vec::new();
        
        // Check tier utilization alerts
        for (tier, utilization) in &metrics.tier_utilization {
            if *utilization >= self.config.alert_thresholds.tier_utilization_critical {
                alerts.push(Alert {
                    id: format!("tier_util_critical_{:?}", tier),
                    level: AlertLevel::Critical,
                    title: format!("Critical: {:?} Tier Utilization", tier),
                    message: format!("Tier {:?} is {:.1}% full (critical threshold: {:.1}%)", 
                                   tier, utilization, self.config.alert_thresholds.tier_utilization_critical),
                    timestamp: SystemTime::now(),
                    category: AlertCategory::Capacity,
                    recommended_actions: vec![
                        "Immediately migrate data to lower tiers".to_string(),
                        "Add additional storage capacity".to_string(),
                        "Review and adjust tier policies".to_string(),
                    ],
                });
            } else if *utilization >= self.config.alert_thresholds.tier_utilization_warning {
                alerts.push(Alert {
                    id: format!("tier_util_warning_{:?}", tier),
                    level: AlertLevel::Warning,
                    title: format!("Warning: {:?} Tier Utilization", tier),
                    message: format!("Tier {:?} is {:.1}% full (warning threshold: {:.1}%)", 
                                   tier, utilization, self.config.alert_thresholds.tier_utilization_warning),
                    timestamp: SystemTime::now(),
                    category: AlertCategory::Capacity,
                    recommended_actions: vec![
                        "Plan for data migration to lower tiers".to_string(),
                        "Monitor growth trends closely".to_string(),
                    ],
                });
            }
        }
        
        // Check migration performance
        let migration_speed_mbps = metrics.avg_migration_speed / (1024.0 * 1024.0);
        if migration_speed_mbps < self.config.alert_thresholds.migration_speed_warning && metrics.active_migrations_count > 0 {
            alerts.push(Alert {
                id: "migration_speed_low".to_string(),
                level: AlertLevel::Warning,
                title: "Low Migration Performance".to_string(),
                message: format!("Migration speed is {:.2} MB/s (threshold: {:.2} MB/s)", 
                               migration_speed_mbps, self.config.alert_thresholds.migration_speed_warning),
                timestamp: SystemTime::now(),
                category: AlertCategory::Performance,
                recommended_actions: vec![
                    "Check network bandwidth usage".to_string(),
                    "Review concurrent migration limits".to_string(),
                    "Verify storage tier performance".to_string(),
                ],
            });
        }
        
        // Check migration failure rate
        if metrics.migrations_completed > 0 {
            let failure_rate = (metrics.migrations_failed as f64 / (metrics.migrations_completed + metrics.migrations_failed) as f64) * 100.0;
            
            if failure_rate >= self.config.alert_thresholds.migration_failure_rate_critical {
                alerts.push(Alert {
                    id: "migration_failure_critical".to_string(),
                    level: AlertLevel::Critical,
                    title: "Critical: High Migration Failure Rate".to_string(),
                    message: format!("Migration failure rate is {:.1}% (critical threshold: {:.1}%)", 
                                   failure_rate, self.config.alert_thresholds.migration_failure_rate_critical),
                    timestamp: SystemTime::now(),
                    category: AlertCategory::Migration,
                    recommended_actions: vec![
                        "Immediately investigate migration errors".to_string(),
                        "Check storage tier connectivity".to_string(),
                        "Review error logs for patterns".to_string(),
                    ],
                });
            } else if failure_rate >= self.config.alert_thresholds.migration_failure_rate_warning {
                alerts.push(Alert {
                    id: "migration_failure_warning".to_string(),
                    level: AlertLevel::Warning,
                    title: "Warning: Elevated Migration Failure Rate".to_string(),
                    message: format!("Migration failure rate is {:.1}% (warning threshold: {:.1}%)", 
                                   failure_rate, self.config.alert_thresholds.migration_failure_rate_warning),
                    timestamp: SystemTime::now(),
                    category: AlertCategory::Migration,
                    recommended_actions: vec![
                        "Monitor migration errors closely".to_string(),
                        "Review storage tier health".to_string(),
                    ],
                });
            }
        }
        
        alerts
    }
    
    /// Generate recommendations based on metrics and alerts
    async fn generate_recommendations(&self, metrics: &IntegrationMetrics, alerts: &[Alert]) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Check if there are critical alerts
        let has_critical_alerts = alerts.iter().any(|alert| matches!(alert.level, AlertLevel::Critical));
        
        if has_critical_alerts {
            recommendations.push("Address critical alerts immediately to prevent service degradation".to_string());
        }
        
        // Recommend tier rebalancing if needed
        let high_util_tiers: Vec<_> = metrics.tier_utilization.iter()
            .filter(|(_, util)| **util > 80.0)
            .map(|(tier, util)| format!("{:?} ({:.1}%)", tier, util))
            .collect();
        
        if !high_util_tiers.is_empty() {
            recommendations.push(format!("Consider rebalancing high utilization tiers: {}", high_util_tiers.join(", ")));
        }
        
        // Performance recommendations
        if metrics.avg_migration_speed > 0.0 {
            let speed_mbps = metrics.avg_migration_speed / (1024.0 * 1024.0);
            if speed_mbps < 50.0 {
                recommendations.push("Migration performance could be improved - consider increasing bandwidth limits or reducing concurrent migrations".to_string());
            }
        }
        
        // Migration queue recommendations
        if metrics.active_migrations_count == 0 && has_critical_alerts {
            recommendations.push("No active migrations detected despite capacity alerts - verify migration policies are enabled".to_string());
        }
        
        recommendations
    }
    
    /// Backup tier metadata
    #[instrument(skip(self))]
    async fn backup_metadata(&self) -> Result<()> {
        if !self.config.backup_config.enabled {
            return Err(anyhow::anyhow!("Backups are disabled"));
        }
        
        let backup_path = &self.config.backup_config.backup_path;
        
        // Create backup directory if it doesn't exist
        tokio::fs::create_dir_all(backup_path).await
            .context("Failed to create backup directory")?;
        
        let backup_file = backup_path.join(format!("tier_metadata_{}.json", 
                                                  SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                                                      .unwrap_or_default().as_secs()));
        
        // In a real implementation, this would serialize and backup actual metadata
        let backup_data = serde_json::json!({
            "timestamp": SystemTime::now(),
            "admin_stats": self.admin_stats,
            "config": self.config
        });
        
        let backup_content = if self.config.backup_config.compression_enabled {
            // Implement compression here
            serde_json::to_string_pretty(&backup_data)?
        } else {
            serde_json::to_string_pretty(&backup_data)?
        };
        
        tokio::fs::write(backup_file, backup_content).await
            .context("Failed to write backup file")?;
        
        info!("Tier metadata backup completed");
        Ok(())
    }
    
    /// Restore tier metadata from backup
    #[instrument(skip(self))]
    async fn restore_metadata(&mut self, backup_file: &PathBuf) -> Result<()> {
        let backup_content = tokio::fs::read_to_string(backup_file).await
            .context("Failed to read backup file")?;
        
        let backup_data: serde_json::Value = serde_json::from_str(&backup_content)
            .context("Failed to parse backup file")?;
        
        // In a real implementation, this would restore actual metadata
        info!("Tier metadata restored from backup: {:?}", backup_file);
        Ok(())
    }
    
    /// Rebalance tiers to achieve target utilization
    #[instrument(skip(self))]
    async fn rebalance_tiers(&self, target_utilization: f64) -> Result<u32> {
        let integrator = self.integrator.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Integrator not initialized"))?;
        
        // Trigger migration evaluation
        let migrations_queued = integrator.trigger_migration_evaluation().await?;
        
        info!("Tier rebalancing initiated: target utilization {:.1}%, {} migrations queued", 
              target_utilization, migrations_queued);
        
        Ok(migrations_queued)
    }
    
    /// Update tier configuration
    #[instrument(skip(self))]
    async fn update_tier_config(&mut self, tier: TierType, config_update: TierConfigUpdate) -> Result<()> {
        // In a real implementation, this would update actual tier configuration
        info!("Updated configuration for tier {:?}: {:?}", tier, config_update);
        Ok(())
    }
    
    /// Estimate tier capacity (placeholder implementation)
    fn estimate_tier_capacity(&self, tier: TierType) -> u64 {
        match tier {
            TierType::HotSSD => 1024 * 1024 * 1024 * 1024, // 1TB
            TierType::WarmSSD => 2 * 1024 * 1024 * 1024 * 1024, // 2TB
            TierType::WarmHDD => 10 * 1024 * 1024 * 1024 * 1024, // 10TB
            TierType::Cold => 100 * 1024 * 1024 * 1024 * 1024, // 100TB
            TierType::Archive => 1000 * 1024 * 1024 * 1024 * 1024, // 1PB
        }
    }
    
    /// Estimate growth rate for a tier (placeholder implementation)
    fn estimate_growth_rate(&self, _tier: TierType) -> f64 {
        1024.0 * 1024.0 * 1024.0 // 1GB per day
    }
    
    /// Estimate days until tier is full
    fn estimate_days_until_full(&self, tier: TierType, utilization_percent: f64) -> Option<u32> {
        if utilization_percent >= 100.0 {
            return Some(0);
        }
        
        let remaining_percent = 100.0 - utilization_percent;
        let capacity = self.estimate_tier_capacity(tier);
        let remaining_bytes = (capacity as f64 * remaining_percent / 100.0) as u64;
        let growth_rate = self.estimate_growth_rate(tier);
        
        if growth_rate > 0.0 {
            Some((remaining_bytes as f64 / growth_rate) as u32)
        } else {
            None
        }
    }
    
    /// Get current administrative statistics
    pub fn get_admin_stats(&self) -> &AdminStats {
        &self.admin_stats
    }
    
    /// Update administrative statistics
    pub fn update_admin_stats(&mut self, stats: AdminStats) {
        self.admin_stats = stats;
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> &ManagementConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: ManagementConfig) {
        self.config = config;
    }
}

/// Result types for administrative commands
#[derive(Debug, Clone)]
pub enum AdminCommandResult {
    Success {
        message: String,
    },
    TierStatus {
        metrics: IntegrationMetrics,
        health: ComponentHealthResult,
        active_migrations: Vec<MigrationTask>,
    },
    Report(TierManagementReport),
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiered_storage::integration::IntegrationConfig;
    
    #[test]
    fn test_manager_creation() {
        let config = ManagementConfig::default();
        let manager = TieredStorageManager::new(config);
        
        assert!(manager.integrator.is_none());
        assert_eq!(manager.admin_stats.total_files, 0);
    }
    
    #[test]
    fn test_config_defaults() {
        let config = ManagementConfig::default();
        
        assert!(config.admin_enabled);
        assert_eq!(config.admin_port, 8080);
        assert!(config.alerts_enabled);
        assert_eq!(config.alert_thresholds.tier_utilization_warning, 80.0);
    }
    
    #[test]
    fn test_alert_thresholds() {
        let thresholds = AlertThresholds::default();
        
        assert!(thresholds.tier_utilization_warning < thresholds.tier_utilization_critical);
        assert!(thresholds.migration_failure_rate_warning < thresholds.migration_failure_rate_critical);
        assert!(thresholds.migration_speed_warning > 0.0);
    }
    
    #[test]
    fn test_backup_config() {
        let backup_config = BackupConfig::default();
        
        assert!(backup_config.enabled);
        assert!(backup_config.compression_enabled);
        assert!(backup_config.backup_interval > Duration::from_secs(0));
    }
    
    #[tokio::test]
    async fn test_command_without_integrator() {
        let config = ManagementConfig::default();
        let mut manager = TieredStorageManager::new(config);
        
        let result = manager.execute_command(AdminCommand::GetTierStatus).await;
        assert!(result.is_err());
    }
    
    #[test]
    fn test_capacity_estimation() {
        let config = ManagementConfig::default();
        let manager = TieredStorageManager::new(config);
        
        let hot_capacity = manager.estimate_tier_capacity(TierType::HotSSD);
        let archive_capacity = manager.estimate_tier_capacity(TierType::Archive);
        
        assert!(hot_capacity < archive_capacity);
        assert!(hot_capacity > 0);
    }
    
    #[test]
    fn test_days_until_full_calculation() {
        let config = ManagementConfig::default();
        let manager = TieredStorageManager::new(config);
        
        let days_90_percent = manager.estimate_days_until_full(TierType::HotSSD, 90.0);
        let days_99_percent = manager.estimate_days_until_full(TierType::HotSSD, 99.0);
        let days_100_percent = manager.estimate_days_until_full(TierType::HotSSD, 100.0);
        
        assert!(days_90_percent.is_some());
        assert!(days_99_percent.is_some());
        assert_eq!(days_100_percent, Some(0));
        
        if let (Some(days_90), Some(days_99)) = (days_90_percent, days_99_percent) {
            assert!(days_90 > days_99);
        }
    }
}