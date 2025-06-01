//! Block allocation management for efficient storage utilization
//! 
//! This module provides different allocation strategies for managing storage blocks
//! within the chunk server, supporting dynamic strategy selection based on system
//! state and workload patterns.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use crate::error::{ChunkServerError, Result};

/// Block size in bytes (64KB by default to match MFS)
pub const BLOCK_SIZE: u64 = 64 * 1024;

/// Maximum number of blocks that can be tracked
pub const MAX_BLOCKS: u64 = 1_000_000;

/// Block allocation result
#[derive(Debug, Clone)]
pub struct AllocatedBlock {
    pub offset: u64,
    pub size: u64,
    pub block_id: u64,
}

/// Free space information
#[derive(Debug, Clone)]
pub struct FreeSpaceInfo {
    pub total_blocks: u64,
    pub free_blocks: u64,
    pub largest_free_block: u64,
    pub fragmentation_ratio: f64,
}

/// Allocation strategy interface
#[async_trait::async_trait]
pub trait AllocationStrategy: Send + Sync {
    /// Allocate a block of the specified size
    async fn allocate(&self, size: u64) -> Result<AllocatedBlock>;
    
    /// Free a previously allocated block
    async fn free(&self, block: &AllocatedBlock) -> Result<()>;
    
    /// Get current free space information
    async fn get_free_space_info(&self) -> Result<FreeSpaceInfo>;
    
    /// Get strategy name for debugging
    fn name(&self) -> &'static str;
    
    /// Check if this strategy is suitable for current conditions
    async fn is_suitable(&self, workload_hint: WorkloadHint) -> Result<bool>;
}

/// Workload hints for dynamic strategy selection
#[derive(Debug, Clone, Copy)]
pub enum WorkloadHint {
    Sequential,    // Large sequential writes
    Random,        // Random access patterns
    SmallFiles,    // Many small files
    LargeFiles,    // Few large files
    Mixed,         // Mixed workload
}

/// First-fit allocation strategy
pub struct FirstFitAllocator {
    free_blocks: Arc<RwLock<BTreeMap<u64, u64>>>, // offset -> size
    allocated_blocks: Arc<RwLock<BTreeMap<u64, u64>>>, // offset -> size
    total_blocks: u64,
    next_block_id: Arc<RwLock<u64>>,
}

impl FirstFitAllocator {
    pub fn new(total_size: u64) -> Self {
        let total_blocks = total_size / BLOCK_SIZE;
        let mut free_blocks = BTreeMap::new();
        free_blocks.insert(0, total_blocks);
        
        Self {
            free_blocks: Arc::new(RwLock::new(free_blocks)),
            allocated_blocks: Arc::new(RwLock::new(BTreeMap::new())),
            total_blocks,
            next_block_id: Arc::new(RwLock::new(1)),
        }
    }
}

#[async_trait::async_trait]
impl AllocationStrategy for FirstFitAllocator {
    async fn allocate(&self, size: u64) -> Result<AllocatedBlock> {
        let blocks_needed = (size + BLOCK_SIZE - 1) / BLOCK_SIZE;
        
        let mut free_blocks = self.free_blocks.write().await;
        let mut allocated_blocks = self.allocated_blocks.write().await;
        
        // Find first free block that can accommodate the request
        for (&offset, &available_blocks) in free_blocks.iter() {
            if available_blocks >= blocks_needed {
                // Remove from free blocks
                free_blocks.remove(&offset);
                
                // If there's leftover space, add it back to free blocks
                if available_blocks > blocks_needed {
                    let remaining_offset = offset + blocks_needed;
                    let remaining_blocks = available_blocks - blocks_needed;
                    free_blocks.insert(remaining_offset, remaining_blocks);
                }
                
                // Add to allocated blocks
                allocated_blocks.insert(offset, blocks_needed);
                
                let mut block_id_guard = self.next_block_id.write().await;
                let block_id = *block_id_guard;
                *block_id_guard += 1;
                
                debug!("Allocated {} blocks at offset {} (block_id: {})", blocks_needed, offset, block_id);
                
                return Ok(AllocatedBlock {
                    offset: offset * BLOCK_SIZE,
                    size: blocks_needed * BLOCK_SIZE,
                    block_id,
                });
            }
        }
        
        Err(ChunkServerError::Storage("No free space available".to_string()))
    }
    
    async fn free(&self, block: &AllocatedBlock) -> Result<()> {
        let block_offset = block.offset / BLOCK_SIZE;
        let block_count = block.size / BLOCK_SIZE;
        
        let mut free_blocks = self.free_blocks.write().await;
        let mut allocated_blocks = self.allocated_blocks.write().await;
        
        // Remove from allocated blocks
        if allocated_blocks.remove(&block_offset).is_none() {
            warn!("Attempted to free unallocated block at offset {}", block_offset);
            return Ok(());
        }
        
        // Add back to free blocks and try to coalesce
        self.coalesce_free_block(&mut free_blocks, block_offset, block_count).await?;
        
        debug!("Freed {} blocks at offset {}", block_count, block_offset);
        Ok(())
    }
    
    async fn get_free_space_info(&self) -> Result<FreeSpaceInfo> {
        let free_blocks = self.free_blocks.read().await;
        
        let free_blocks_count: u64 = free_blocks.values().sum();
        let largest_free_block = free_blocks.values().max().copied().unwrap_or(0);
        
        let fragmentation_ratio = if free_blocks_count > 0 {
            1.0 - (largest_free_block as f64 / free_blocks_count as f64)
        } else {
            0.0
        };
        
        Ok(FreeSpaceInfo {
            total_blocks: self.total_blocks,
            free_blocks: free_blocks_count,
            largest_free_block,
            fragmentation_ratio,
        })
    }
    
    fn name(&self) -> &'static str {
        "FirstFit"
    }
    
    async fn is_suitable(&self, workload_hint: WorkloadHint) -> Result<bool> {
        let info = self.get_free_space_info().await?;
        
        match workload_hint {
            WorkloadHint::Sequential => Ok(info.fragmentation_ratio < 0.5),
            WorkloadHint::Random => Ok(true), // First-fit is reasonable for random access
            WorkloadHint::SmallFiles => Ok(info.fragmentation_ratio < 0.7),
            WorkloadHint::LargeFiles => Ok(info.largest_free_block * BLOCK_SIZE > 1024 * 1024), // 1MB threshold
            WorkloadHint::Mixed => Ok(true),
        }
    }
}

impl FirstFitAllocator {
    async fn coalesce_free_block(
        &self,
        free_blocks: &mut BTreeMap<u64, u64>,
        offset: u64,
        size: u64,
    ) -> Result<()> {
        let mut new_offset = offset;
        let mut new_size = size;
        
        // Check if we can merge with previous block
        if let Some((&prev_offset, &prev_size)) = free_blocks.range(..offset).next_back() {
            if prev_offset + prev_size == offset {
                free_blocks.remove(&prev_offset);
                new_offset = prev_offset;
                new_size += prev_size;
            }
        }
        
        // Check if we can merge with next block
        let next_offset = new_offset + new_size;
        if let Some(&next_size) = free_blocks.get(&next_offset) {
            free_blocks.remove(&next_offset);
            new_size += next_size;
        }
        
        free_blocks.insert(new_offset, new_size);
        Ok(())
    }
}

/// Best-fit allocation strategy
pub struct BestFitAllocator {
    free_blocks: Arc<RwLock<BTreeMap<u64, BTreeSet<u64>>>>, // size -> set of offsets
    offset_to_size: Arc<RwLock<BTreeMap<u64, u64>>>, // offset -> size mapping
    allocated_blocks: Arc<RwLock<BTreeMap<u64, u64>>>, // offset -> size
    total_blocks: u64,
    next_block_id: Arc<RwLock<u64>>,
}

impl BestFitAllocator {
    pub fn new(total_size: u64) -> Self {
        let total_blocks = total_size / BLOCK_SIZE;
        let mut free_blocks = BTreeMap::new();
        let mut offset_set = BTreeSet::new();
        offset_set.insert(0);
        free_blocks.insert(total_blocks, offset_set);
        
        let mut offset_to_size = BTreeMap::new();
        offset_to_size.insert(0, total_blocks);
        
        Self {
            free_blocks: Arc::new(RwLock::new(free_blocks)),
            offset_to_size: Arc::new(RwLock::new(offset_to_size)),
            allocated_blocks: Arc::new(RwLock::new(BTreeMap::new())),
            total_blocks,
            next_block_id: Arc::new(RwLock::new(1)),
        }
    }
}

#[async_trait::async_trait]
impl AllocationStrategy for BestFitAllocator {
    async fn allocate(&self, size: u64) -> Result<AllocatedBlock> {
        let blocks_needed = (size + BLOCK_SIZE - 1) / BLOCK_SIZE;
        
        let mut free_blocks = self.free_blocks.write().await;
        let mut offset_to_size = self.offset_to_size.write().await;
        let mut allocated_blocks = self.allocated_blocks.write().await;
        
        // Find smallest free block that can accommodate the request
        for (&available_blocks, offsets) in free_blocks.range(blocks_needed..) {
            if let Some(&offset) = offsets.iter().next() {
                // Remove from free block structures
                let mut offsets = offsets.clone();
                offsets.remove(&offset);
                if offsets.is_empty() {
                    free_blocks.remove(&available_blocks);
                } else {
                    free_blocks.insert(available_blocks, offsets);
                }
                offset_to_size.remove(&offset);
                
                // If there's leftover space, add it back
                if available_blocks > blocks_needed {
                    let remaining_offset = offset + blocks_needed;
                    let remaining_blocks = available_blocks - blocks_needed;
                    
                    offset_to_size.insert(remaining_offset, remaining_blocks);
                    free_blocks.entry(remaining_blocks)
                        .or_insert_with(BTreeSet::new)
                        .insert(remaining_offset);
                }
                
                // Add to allocated blocks
                allocated_blocks.insert(offset, blocks_needed);
                
                let mut block_id_guard = self.next_block_id.write().await;
                let block_id = *block_id_guard;
                *block_id_guard += 1;
                
                debug!("BestFit allocated {} blocks at offset {} (block_id: {})", blocks_needed, offset, block_id);
                
                return Ok(AllocatedBlock {
                    offset: offset * BLOCK_SIZE,
                    size: blocks_needed * BLOCK_SIZE,
                    block_id,
                });
            }
        }
        
        Err(ChunkServerError::Storage("No free space available".to_string()))
    }
    
    async fn free(&self, block: &AllocatedBlock) -> Result<()> {
        let block_offset = block.offset / BLOCK_SIZE;
        let block_count = block.size / BLOCK_SIZE;
        
        let mut free_blocks = self.free_blocks.write().await;
        let mut offset_to_size = self.offset_to_size.write().await;
        let mut allocated_blocks = self.allocated_blocks.write().await;
        
        // Remove from allocated blocks
        if allocated_blocks.remove(&block_offset).is_none() {
            warn!("Attempted to free unallocated block at offset {}", block_offset);
            return Ok(());
        }
        
        // Add back to free blocks with coalescing
        self.coalesce_and_add_free_block(
            &mut free_blocks,
            &mut offset_to_size,
            block_offset,
            block_count,
        ).await?;
        
        debug!("BestFit freed {} blocks at offset {}", block_count, block_offset);
        Ok(())
    }
    
    async fn get_free_space_info(&self) -> Result<FreeSpaceInfo> {
        let free_blocks = self.free_blocks.read().await;
        
        let free_blocks_count: u64 = free_blocks.iter()
            .map(|(&size, offsets)| size * offsets.len() as u64)
            .sum();
        let largest_free_block = free_blocks.keys().last().copied().unwrap_or(0);
        
        let fragmentation_ratio = if free_blocks_count > 0 {
            1.0 - (largest_free_block as f64 / free_blocks_count as f64)
        } else {
            0.0
        };
        
        Ok(FreeSpaceInfo {
            total_blocks: self.total_blocks,
            free_blocks: free_blocks_count,
            largest_free_block,
            fragmentation_ratio,
        })
    }
    
    fn name(&self) -> &'static str {
        "BestFit"
    }
    
    async fn is_suitable(&self, workload_hint: WorkloadHint) -> Result<bool> {
        let info = self.get_free_space_info().await?;
        
        match workload_hint {
            WorkloadHint::Sequential => Ok(info.fragmentation_ratio < 0.3),
            WorkloadHint::Random => Ok(true),
            WorkloadHint::SmallFiles => Ok(true), // Best-fit is good for small files
            WorkloadHint::LargeFiles => Ok(info.largest_free_block * BLOCK_SIZE > 10 * 1024 * 1024), // 10MB threshold
            WorkloadHint::Mixed => Ok(info.fragmentation_ratio < 0.6),
        }
    }
}

impl BestFitAllocator {
    async fn coalesce_and_add_free_block(
        &self,
        free_blocks: &mut BTreeMap<u64, BTreeSet<u64>>,
        offset_to_size: &mut BTreeMap<u64, u64>,
        offset: u64,
        size: u64,
    ) -> Result<()> {
        let mut new_offset = offset;
        let mut new_size = size;
        
        // Check if we can merge with previous block
        if let Some((&prev_offset, &prev_size)) = offset_to_size.range(..offset).next_back() {
            if prev_offset + prev_size == offset {
                // Remove previous block from free structures
                offset_to_size.remove(&prev_offset);
                let mut prev_offsets = free_blocks.get(&prev_size).unwrap().clone();
                prev_offsets.remove(&prev_offset);
                if prev_offsets.is_empty() {
                    free_blocks.remove(&prev_size);
                } else {
                    free_blocks.insert(prev_size, prev_offsets);
                }
                
                new_offset = prev_offset;
                new_size += prev_size;
            }
        }
        
        // Check if we can merge with next block
        let next_offset = new_offset + new_size;
        if let Some(&next_size) = offset_to_size.get(&next_offset) {
            // Remove next block from free structures
            offset_to_size.remove(&next_offset);
            let mut next_offsets = free_blocks.get(&next_size).unwrap().clone();
            next_offsets.remove(&next_offset);
            if next_offsets.is_empty() {
                free_blocks.remove(&next_size);
            } else {
                free_blocks.insert(next_size, next_offsets);
            }
            
            new_size += next_size;
        }
        
        // Add the coalesced block
        offset_to_size.insert(new_offset, new_size);
        free_blocks.entry(new_size)
            .or_insert_with(BTreeSet::new)
            .insert(new_offset);
        
        Ok(())
    }
}

/// Dynamic block allocator that selects the best strategy based on workload
pub struct DynamicBlockAllocator {
    strategies: Vec<Arc<dyn AllocationStrategy>>,
    current_strategy: Arc<RwLock<usize>>,
    workload_history: Arc<RwLock<Vec<WorkloadHint>>>,
    max_history: usize,
}

impl DynamicBlockAllocator {
    pub fn new(total_size: u64) -> Self {
        let strategies: Vec<Arc<dyn AllocationStrategy>> = vec![
            Arc::new(FirstFitAllocator::new(total_size)),
            Arc::new(BestFitAllocator::new(total_size)),
        ];
        
        Self {
            strategies,
            current_strategy: Arc::new(RwLock::new(0)), // Start with FirstFit
            workload_history: Arc::new(RwLock::new(Vec::new())),
            max_history: 100,
        }
    }
    
    pub async fn allocate_with_hint(&self, size: u64, hint: WorkloadHint) -> Result<AllocatedBlock> {
        // Update workload history
        let mut history = self.workload_history.write().await;
        history.push(hint);
        if history.len() > self.max_history {
            history.remove(0);
        }
        drop(history);
        
        // Check if we should switch strategies
        self.evaluate_and_switch_strategy(hint).await?;
        
        // Allocate using current strategy
        let current_idx = *self.current_strategy.read().await;
        self.strategies[current_idx].allocate(size).await
    }
    
    async fn evaluate_and_switch_strategy(&self, current_hint: WorkloadHint) -> Result<()> {
        let current_idx = *self.current_strategy.read().await;
        let current_strategy = &self.strategies[current_idx];
        
        // Check if current strategy is still suitable
        if current_strategy.is_suitable(current_hint).await? {
            return Ok(());
        }
        
        // Find a better strategy
        for (idx, strategy) in self.strategies.iter().enumerate() {
            if idx != current_idx && strategy.is_suitable(current_hint).await? {
                info!("Switching allocation strategy from {} to {}", 
                      current_strategy.name(), strategy.name());
                *self.current_strategy.write().await = idx;
                break;
            }
        }
        
        Ok(())
    }
    
    pub async fn get_current_strategy_name(&self) -> String {
        let idx = *self.current_strategy.read().await;
        self.strategies[idx].name().to_string()
    }
}

#[async_trait::async_trait]
impl AllocationStrategy for DynamicBlockAllocator {
    async fn allocate(&self, size: u64) -> Result<AllocatedBlock> {
        self.allocate_with_hint(size, WorkloadHint::Mixed).await
    }
    
    async fn free(&self, block: &AllocatedBlock) -> Result<()> {
        let current_idx = *self.current_strategy.read().await;
        self.strategies[current_idx].free(block).await
    }
    
    async fn get_free_space_info(&self) -> Result<FreeSpaceInfo> {
        let current_idx = *self.current_strategy.read().await;
        self.strategies[current_idx].get_free_space_info().await
    }
    
    fn name(&self) -> &'static str {
        "Dynamic"
    }
    
    async fn is_suitable(&self, workload_hint: WorkloadHint) -> Result<bool> {
        // Dynamic allocator is always suitable as it adapts
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_first_fit_allocation() {
        let allocator = FirstFitAllocator::new(1024 * 1024); // 1MB
        
        // Allocate some blocks
        let block1 = allocator.allocate(BLOCK_SIZE).await.unwrap();
        let block2 = allocator.allocate(BLOCK_SIZE * 2).await.unwrap();
        
        assert_eq!(block1.size, BLOCK_SIZE);
        assert_eq!(block2.size, BLOCK_SIZE * 2);
        assert_ne!(block1.offset, block2.offset);
        
        // Free and reallocate
        allocator.free(&block1).await.unwrap();
        let block3 = allocator.allocate(BLOCK_SIZE).await.unwrap();
        assert_eq!(block3.offset, block1.offset); // Should reuse the space
    }
    
    #[tokio::test]
    async fn test_best_fit_allocation() {
        let allocator = BestFitAllocator::new(1024 * 1024); // 1MB
        
        // Allocate and free to create fragmentation
        let block1 = allocator.allocate(BLOCK_SIZE).await.unwrap();
        let block2 = allocator.allocate(BLOCK_SIZE * 3).await.unwrap();
        let block3 = allocator.allocate(BLOCK_SIZE * 2).await.unwrap();
        
        allocator.free(&block2).await.unwrap(); // Creates a 3-block hole
        
        // This should fit in the 3-block hole (best fit)
        let block4 = allocator.allocate(BLOCK_SIZE * 2).await.unwrap();
        assert_eq!(block4.offset, block2.offset);
    }
    
    #[tokio::test]
    async fn test_dynamic_allocator() {
        let allocator = DynamicBlockAllocator::new(1024 * 1024);
        
        // Test allocation with different hints
        let _block1 = allocator.allocate_with_hint(BLOCK_SIZE, WorkloadHint::SmallFiles).await.unwrap();
        let _block2 = allocator.allocate_with_hint(BLOCK_SIZE * 10, WorkloadHint::LargeFiles).await.unwrap();
        
        let strategy_name = allocator.get_current_strategy_name().await;
        assert!(!strategy_name.is_empty());
    }
}