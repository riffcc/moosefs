use anyhow::{anyhow, Result};
use dashmap::DashMap;
use mooseng_common::types::{StorageClass, StorageClassId, ChunkId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::metadata::MetadataStore;

/// Storage class definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageClassDef {
    pub id: StorageClassId,
    pub name: String,
    pub description: String,
    pub copies: u8,
    pub ec_enabled: bool,
    pub ec_data_chunks: u8,
    pub ec_parity_chunks: u8,
    pub allowed_labels: Vec<String>,
    pub preferred_labels: Vec<String>,
    pub create_labels: Vec<String>,
    pub keep_labels: Vec<String>,
    pub arch_mode: ArchiveMode,
    pub default_class: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchiveMode {
    None,
    Time(u64), // seconds
    Size(u64), // bytes
}

/// Manages storage classes and their assignments
pub struct StorageClassManager {
    metadata_store: Arc<MetadataStore>,
    classes: Arc<DashMap<StorageClassId, StorageClassDef>>,
    default_class_id: Arc<RwLock<StorageClassId>>,
}

impl StorageClassManager {
    pub async fn new(metadata_store: Arc<MetadataStore>) -> Result<Self> {
        let manager = Self {
            metadata_store,
            classes: Arc::new(DashMap::new()),
            default_class_id: Arc::new(RwLock::new(1)),
        };
        
        // Load existing storage classes
        manager.load_storage_classes().await?;
        
        // Create default storage classes if none exist
        if manager.classes.is_empty() {
            manager.create_default_classes().await?;
        }
        
        Ok(manager)
    }
    
    /// Load storage classes from metadata store
    async fn load_storage_classes(&self) -> Result<()> {
        debug!("Loading storage classes from metadata store");
        
        // This would load from sled/metadata store
        // For now, we'll implement a placeholder
        Ok(())
    }
    
    /// Create default storage classes
    async fn create_default_classes(&self) -> Result<()> {
        info!("Creating default storage classes");
        
        // Standard replication class
        let std_class = StorageClassDef {
            id: 1,
            name: "standard".to_string(),
            description: "Standard replication with 3 copies".to_string(),
            copies: 3,
            ec_enabled: false,
            ec_data_chunks: 0,
            ec_parity_chunks: 0,
            allowed_labels: vec![],
            preferred_labels: vec![],
            create_labels: vec![],
            keep_labels: vec![],
            arch_mode: ArchiveMode::None,
            default_class: true,
        };
        
        // EC 4+2 class
        let ec_class = StorageClassDef {
            id: 2,
            name: "ec_4_2".to_string(),
            description: "Erasure coding 4+2 configuration".to_string(),
            copies: 0,
            ec_enabled: true,
            ec_data_chunks: 4,
            ec_parity_chunks: 2,
            allowed_labels: vec![],
            preferred_labels: vec![],
            create_labels: vec![],
            keep_labels: vec![],
            arch_mode: ArchiveMode::None,
            default_class: false,
        };
        
        // EC 8+3 class
        let ec_large_class = StorageClassDef {
            id: 3,
            name: "ec_8_3".to_string(),
            description: "Erasure coding 8+3 configuration for large files".to_string(),
            copies: 0,
            ec_enabled: true,
            ec_data_chunks: 8,
            ec_parity_chunks: 3,
            allowed_labels: vec![],
            preferred_labels: vec![],
            create_labels: vec![],
            keep_labels: vec![],
            arch_mode: ArchiveMode::Size(64 * 1024 * 1024), // 64MB threshold
            default_class: false,
        };
        
        self.classes.insert(1, std_class);
        self.classes.insert(2, ec_class);
        self.classes.insert(3, ec_large_class);
        
        // Save to metadata store
        self.save_storage_classes().await?;
        
        info!("Created {} default storage classes", self.classes.len());
        Ok(())
    }
    
    /// Save storage classes to metadata store
    async fn save_storage_classes(&self) -> Result<()> {
        debug!("Saving storage classes to metadata store");
        
        // This would save to sled/metadata store
        // For now, we'll implement a placeholder
        Ok(())
    }
    
    /// Get storage class by ID
    pub async fn get_class(&self, id: StorageClassId) -> Result<StorageClassDef> {
        self.classes.get(&id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| anyhow!("Storage class {} not found", id))
    }
    
    /// Get storage class by name
    pub async fn get_class_by_name(&self, name: &str) -> Result<StorageClassDef> {
        for entry in self.classes.iter() {
            if entry.value().name == name {
                return Ok(entry.value().clone());
            }
        }
        Err(anyhow!("Storage class '{}' not found", name))
    }
    
    /// Get default storage class
    pub async fn get_default_class(&self) -> Result<StorageClassDef> {
        let default_id = *self.default_class_id.read().await;
        self.get_class(default_id).await
    }
    
    /// List all storage classes
    pub async fn list_classes(&self) -> Vec<StorageClassDef> {
        self.classes.iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
    
    /// Create a new storage class
    pub async fn create_class(&self, mut class: StorageClassDef) -> Result<StorageClassId> {
        // Generate new ID
        let new_id = self.classes.len() as StorageClassId + 1;
        class.id = new_id;
        
        // Validate the class definition
        self.validate_class(&class)?;
        
        // Insert the class
        self.classes.insert(new_id, class.clone());
        
        // Save to metadata store
        self.save_storage_classes().await?;
        
        info!("Created storage class '{}' with ID {}", class.name, new_id);
        Ok(new_id)
    }
    
    /// Update an existing storage class
    pub async fn update_class(&self, id: StorageClassId, updated_class: StorageClassDef) -> Result<()> {
        if !self.classes.contains_key(&id) {
            return Err(anyhow!("Storage class {} not found", id));
        }
        
        // Validate the updated class
        self.validate_class(&updated_class)?;
        
        // Update the class
        self.classes.insert(id, updated_class);
        
        // Save to metadata store
        self.save_storage_classes().await?;
        
        info!("Updated storage class {}", id);
        Ok(())
    }
    
    /// Delete a storage class
    pub async fn delete_class(&self, id: StorageClassId) -> Result<()> {
        // Cannot delete default class
        let default_id = *self.default_class_id.read().await;
        if id == default_id {
            return Err(anyhow!("Cannot delete default storage class"));
        }
        
        // Check if class is in use
        if self.is_class_in_use(id).await? {
            return Err(anyhow!("Storage class {} is in use", id));
        }
        
        // Remove the class
        if let Some((_, class)) = self.classes.remove(&id) {
            // Save to metadata store
            self.save_storage_classes().await?;
            
            info!("Deleted storage class '{}' (ID: {})", class.name, id);
            Ok(())
        } else {
            Err(anyhow!("Storage class {} not found", id))
        }
    }
    
    /// Set default storage class
    pub async fn set_default_class(&self, id: StorageClassId) -> Result<()> {
        if !self.classes.contains_key(&id) {
            return Err(anyhow!("Storage class {} not found", id));
        }
        
        *self.default_class_id.write().await = id;
        
        // Update class definitions to reflect default status
        for mut entry in self.classes.iter_mut() {
            entry.value_mut().default_class = entry.key() == &id;
        }
        
        // Save to metadata store
        self.save_storage_classes().await?;
        
        info!("Set storage class {} as default", id);
        Ok(())
    }
    
    /// Validate storage class definition
    fn validate_class(&self, class: &StorageClassDef) -> Result<()> {
        if class.name.is_empty() {
            return Err(anyhow!("Storage class name cannot be empty"));
        }
        
        if class.ec_enabled {
            if class.ec_data_chunks == 0 || class.ec_parity_chunks == 0 {
                return Err(anyhow!("EC enabled but data/parity chunks not configured"));
            }
            if class.copies != 0 {
                return Err(anyhow!("Cannot have both replication and erasure coding"));
            }
        } else {
            if class.copies == 0 {
                return Err(anyhow!("Must specify either replication copies or enable EC"));
            }
        }
        
        Ok(())
    }
    
    /// Check if storage class is in use
    async fn is_class_in_use(&self, _id: StorageClassId) -> Result<bool> {
        // This would check if any files are using this storage class
        // For now, return false as a placeholder
        Ok(false)
    }
    
    /// Get recommended storage class for a file
    pub async fn recommend_class(&self, file_size: u64, _path: &str) -> Result<StorageClassId> {
        // Simple recommendation logic
        if file_size > 64 * 1024 * 1024 {
            // Large files use EC 8+3
            if self.classes.contains_key(&3) {
                return Ok(3);
            }
        } else if file_size > 1024 * 1024 {
            // Medium files use EC 4+2
            if self.classes.contains_key(&2) {
                return Ok(2);
            }
        }
        
        // Default to standard replication
        Ok(*self.default_class_id.read().await)
    }
    
    /// Calculate storage efficiency for a storage class
    pub async fn calculate_efficiency(&self, id: StorageClassId) -> Result<f64> {
        let class = self.get_class(id).await?;
        
        if class.ec_enabled {
            let total_chunks = class.ec_data_chunks + class.ec_parity_chunks;
            Ok(class.ec_data_chunks as f64 / total_chunks as f64)
        } else {
            Ok(1.0 / class.copies as f64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_manager() -> (StorageClassManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let metadata_store = Arc::new(MetadataStore::new(temp_dir.path()).await.unwrap());
        let manager = StorageClassManager::new(metadata_store).await.unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_default_classes() {
        let (manager, _temp) = create_test_manager().await;
        
        let classes = manager.list_classes().await;
        assert_eq!(classes.len(), 3);
        
        let default_class = manager.get_default_class().await.unwrap();
        assert_eq!(default_class.name, "standard");
        assert_eq!(default_class.copies, 3);
    }
    
    #[tokio::test]
    async fn test_class_by_name() {
        let (manager, _temp) = create_test_manager().await;
        
        let ec_class = manager.get_class_by_name("ec_4_2").await.unwrap();
        assert!(ec_class.ec_enabled);
        assert_eq!(ec_class.ec_data_chunks, 4);
        assert_eq!(ec_class.ec_parity_chunks, 2);
    }
    
    #[tokio::test]
    async fn test_efficiency_calculation() {
        let (manager, _temp) = create_test_manager().await;
        
        // Standard replication (3 copies) = 1/3 efficiency
        let std_efficiency = manager.calculate_efficiency(1).await.unwrap();
        assert!((std_efficiency - (1.0/3.0)).abs() < 0.01);
        
        // EC 4+2 = 4/6 efficiency
        let ec_efficiency = manager.calculate_efficiency(2).await.unwrap();
        assert!((ec_efficiency - (4.0/6.0)).abs() < 0.01);
    }
    
    #[tokio::test]
    async fn test_recommendation() {
        let (manager, _temp) = create_test_manager().await;
        
        // Small file -> standard replication
        let small_rec = manager.recommend_class(1024, "/test").await.unwrap();
        assert_eq!(small_rec, 1);
        
        // Medium file -> EC 4+2
        let medium_rec = manager.recommend_class(10 * 1024 * 1024, "/test").await.unwrap();
        assert_eq!(medium_rec, 2);
        
        // Large file -> EC 8+3
        let large_rec = manager.recommend_class(100 * 1024 * 1024, "/test").await.unwrap();
        assert_eq!(large_rec, 3);
    }
}