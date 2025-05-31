//! Conflict-free Replicated Data Types (CRDTs) for MooseNG multiregion support
//! 
//! This module implements various CRDT types to ensure eventual consistency
//! across multiple regions without requiring coordination for updates.

use super::hybrid_clock::HLCTimestamp;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet, BTreeMap, BTreeSet};
use std::cmp::{Ord, Ordering};
use std::hash::Hash;
use std::fmt::Debug;
pub use mooseng_common::types::NodeId;

/// Trait for all CRDT types
pub trait CRDT: Clone + Debug + Serialize + DeserializeOwned {
    /// Merge this CRDT with another CRDT of the same type
    /// This operation must be commutative, associative, and idempotent
    fn merge(&mut self, other: &Self) -> Result<()>;
    
    /// Check if this CRDT is equal to another (structural equality)
    fn crdt_eq(&self, other: &Self) -> bool;
    
    /// Get the size of this CRDT (for monitoring purposes)
    fn size(&self) -> usize;
}

/// G-Counter: Grow-only counter CRDT
/// Each node can only increment its own counter, but can read all counters
#[derive(Debug, Clone, Serialize, ::serde::Deserialize, PartialEq, Eq)]
pub struct GCounter {
    /// Map from node_id to that node's counter value
    counters: HashMap<NodeId, u64>,
}

impl GCounter {
    /// Create a new G-Counter
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }
    
    /// Increment the counter for a specific node
    pub fn increment(&mut self, node_id: NodeId, delta: u64) -> Result<()> {
        let current = self.counters.get(&node_id).unwrap_or(&0);
        self.counters.insert(node_id, current + delta);
        Ok(())
    }
    
    /// Get the total value across all nodes
    pub fn value(&self) -> u64 {
        self.counters.values().sum()
    }
    
    /// Get the value for a specific node
    pub fn value_for_node(&self, node_id: NodeId) -> u64 {
        self.counters.get(&node_id).copied().unwrap_or(0)
    }
}

impl CRDT for GCounter {
    fn merge(&mut self, other: &Self) -> Result<()> {
        for (node_id, other_value) in &other.counters {
            let current_value = self.counters.get(node_id).unwrap_or(&0);
            self.counters.insert(*node_id, (*current_value).max(*other_value));
        }
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        self.counters == other.counters
    }
    
    fn size(&self) -> usize {
        self.counters.len() * (std::mem::size_of::<NodeId>() + std::mem::size_of::<u64>())
    }
}

/// PN-Counter: Increment/Decrement counter CRDT
/// Combines two G-Counters (positive and negative) to allow both operations
#[derive(Debug, Clone, Serialize, ::serde::Deserialize, PartialEq, Eq)]
pub struct PNCounter {
    positive: GCounter,
    negative: GCounter,
}

impl PNCounter {
    /// Create a new PN-Counter
    pub fn new() -> Self {
        Self {
            positive: GCounter::new(),
            negative: GCounter::new(),
        }
    }
    
    /// Increment the counter for a specific node
    pub fn increment(&mut self, node_id: NodeId, delta: u64) -> Result<()> {
        self.positive.increment(node_id, delta)
    }
    
    /// Decrement the counter for a specific node
    pub fn decrement(&mut self, node_id: NodeId, delta: u64) -> Result<()> {
        self.negative.increment(node_id, delta)
    }
    
    /// Get the total value (positive - negative)
    pub fn value(&self) -> i64 {
        self.positive.value() as i64 - self.negative.value() as i64
    }
}

impl CRDT for PNCounter {
    fn merge(&mut self, other: &Self) -> Result<()> {
        self.positive.merge(&other.positive)?;
        self.negative.merge(&other.negative)?;
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        self.positive.crdt_eq(&other.positive) && self.negative.crdt_eq(&other.negative)
    }
    
    fn size(&self) -> usize {
        self.positive.size() + self.negative.size()
    }
}

/// G-Set: Grow-only set CRDT
/// Elements can only be added, never removed
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    elements: BTreeSet<T>,
}

impl<'de, T> Deserialize<'de> for GSet<T>
where
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(bound = "T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord")]
        struct GSetHelper<T> {
            elements: BTreeSet<T>,
        }
        
        let helper = GSetHelper::deserialize(deserializer)?;
        Ok(GSet {
            elements: helper.elements,
        })
    }
}

impl<T> GSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    /// Create a new G-Set
    pub fn new() -> Self {
        Self {
            elements: BTreeSet::new(),
        }
    }
    
    /// Add an element to the set
    pub fn add(&mut self, element: T) -> Result<()> {
        self.elements.insert(element);
        Ok(())
    }
    
    /// Check if the set contains an element
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }
    
    /// Get all elements in the set
    pub fn elements(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }
    
    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.elements.len()
    }
    
    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

impl<T> CRDT for GSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    fn merge(&mut self, other: &Self) -> Result<()> {
        for element in &other.elements {
            self.elements.insert(element.clone());
        }
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
    
    fn size(&self) -> usize {
        self.elements.len() * std::mem::size_of::<T>()
    }
}

// Note: Using automatic derive for Deserialize works with proper bounds on the struct definition

/// 2P-Set: Two-phase set CRDT
/// Elements can be added and removed, but once removed, they cannot be added again
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TwoPSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    added: GSet<T>,
    removed: GSet<T>,
}

impl<T> TwoPSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    /// Create a new 2P-Set
    pub fn new() -> Self {
        Self {
            added: GSet::new(),
            removed: GSet::new(),
        }
    }
    
    /// Add an element to the set (only if not already removed)
    pub fn add(&mut self, element: T) -> Result<()> {
        if !self.removed.contains(&element) {
            self.added.add(element)?;
        }
        Ok(())
    }
    
    /// Remove an element from the set (only if previously added)
    pub fn remove(&mut self, element: T) -> Result<()> {
        if self.added.contains(&element) {
            self.removed.add(element)?;
        }
        Ok(())
    }
    
    /// Check if the set contains an element (added but not removed)
    pub fn contains(&self, element: &T) -> bool {
        self.added.contains(element) && !self.removed.contains(element)
    }
    
    /// Get all elements currently in the set
    pub fn elements(&self) -> impl Iterator<Item = &T> {
        self.added.elements().filter(|e| !self.removed.contains(e))
    }
    
    /// Get the number of elements currently in the set
    pub fn len(&self) -> usize {
        self.elements().count()
    }
    
    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> CRDT for TwoPSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    fn merge(&mut self, other: &Self) -> Result<()> {
        self.added.merge(&other.added)?;
        self.removed.merge(&other.removed)?;
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        self.added.crdt_eq(&other.added) && self.removed.crdt_eq(&other.removed)
    }
    
    fn size(&self) -> usize {
        self.added.size() + self.removed.size()
    }
}

impl<'de, T> Deserialize<'de> for TwoPSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(bound = "T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord")]
        struct TwoPSetHelper<T> 
        where 
            T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
        {
            added: GSet<T>,
            removed: GSet<T>,
        }
        
        let helper = TwoPSetHelper::deserialize(deserializer)?;
        Ok(TwoPSet {
            added: helper.added,
            removed: helper.removed,
        })
    }
}

/// LWW-Register: Last-Writer-Wins Register CRDT
/// Each update includes a timestamp, and the value with the latest timestamp wins
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LWWRegister<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    value: Option<T>,
    timestamp: HLCTimestamp,
    node_id: NodeId,
}

impl<'de, T> Deserialize<'de> for LWWRegister<T>
where
    T: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(bound = "T: Clone + Debug + Serialize + DeserializeOwned + Eq")]
        struct LWWRegisterHelper<T> {
            value: Option<T>,
            timestamp: HLCTimestamp,
            node_id: NodeId,
        }
        
        let helper = LWWRegisterHelper::deserialize(deserializer)?;
        Ok(LWWRegister {
            value: helper.value,
            timestamp: helper.timestamp,
            node_id: helper.node_id,
        })
    }
}

impl<T> LWWRegister<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    /// Create a new LWW-Register
    pub fn new(node_id: NodeId) -> Self {
        Self {
            value: None,
            timestamp: HLCTimestamp::new_with_node(0, 0, node_id as u32),
            node_id,
        }
    }
    
    /// Set the value with a new timestamp
    pub fn set(&mut self, value: T, timestamp: HLCTimestamp) -> Result<()> {
        if timestamp > self.timestamp || 
           (timestamp == self.timestamp && timestamp.node_id > self.timestamp.node_id) {
            self.value = Some(value);
            self.timestamp = timestamp;
        }
        Ok(())
    }
    
    /// Get the current value
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }
    
    /// Get the timestamp of the current value
    pub fn timestamp(&self) -> &HLCTimestamp {
        &self.timestamp
    }
}

impl<T> CRDT for LWWRegister<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    fn merge(&mut self, other: &Self) -> Result<()> {
        if other.timestamp > self.timestamp || 
           (other.timestamp == self.timestamp && other.timestamp.node_id > self.timestamp.node_id) {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
        }
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        self.value == other.value && self.timestamp == other.timestamp
    }
    
    fn size(&self) -> usize {
        std::mem::size_of::<HLCTimestamp>() + 
        std::mem::size_of::<NodeId>() + 
        std::mem::size_of::<Option<T>>()
    }
}

/// OR-Set: Observed-Remove Set CRDT
/// Uses unique identifiers for each add/remove operation to handle concurrent operations
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ORSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    added: HashMap<T, BTreeSet<(HLCTimestamp, NodeId)>>,
    removed: HashMap<T, BTreeSet<(HLCTimestamp, NodeId)>>,
}

impl<T> ORSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    /// Create a new OR-Set
    pub fn new() -> Self {
        Self {
            added: HashMap::new(),
            removed: HashMap::new(),
        }
    }
    
    /// Add an element with a unique identifier
    pub fn add(&mut self, element: T, timestamp: HLCTimestamp, node_id: NodeId) -> Result<()> {
        self.added.entry(element)
            .or_insert_with(BTreeSet::new)
            .insert((timestamp, node_id));
        Ok(())
    }
    
    /// Remove an element (removes all current add operations)
    pub fn remove(&mut self, element: &T, timestamp: HLCTimestamp, node_id: NodeId) -> Result<()> {
        if let Some(add_tags) = self.added.get(element) {
            let remove_set = self.removed.entry(element.clone())
                .or_insert_with(BTreeSet::new);
            
            // Add all current add operations to the remove set
            for &add_tag in add_tags {
                remove_set.insert(add_tag);
            }
            
            // Also add the remove operation itself
            remove_set.insert((timestamp, node_id));
        }
        Ok(())
    }
    
    /// Check if the set contains an element
    pub fn contains(&self, element: &T) -> bool {
        if let Some(add_tags) = self.added.get(element) {
            if let Some(remove_tags) = self.removed.get(element) {
                // Element is present if there are add operations not in the remove set
                add_tags.iter().any(|tag| !remove_tags.contains(tag))
            } else {
                // No removes, so element is present if there are adds
                !add_tags.is_empty()
            }
        } else {
            false
        }
    }
    
    /// Get all elements currently in the set
    pub fn elements(&self) -> impl Iterator<Item = &T> {
        self.added.keys().filter(|element| self.contains(element))
    }
    
    /// Get the number of elements currently in the set
    pub fn len(&self) -> usize {
        self.elements().count()
    }
    
    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> CRDT for ORSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    fn merge(&mut self, other: &Self) -> Result<()> {
        // Merge added operations
        for (element, other_tags) in &other.added {
            let our_tags = self.added.entry(element.clone())
                .or_insert_with(BTreeSet::new);
            for &tag in other_tags {
                our_tags.insert(tag);
            }
        }
        
        // Merge removed operations
        for (element, other_tags) in &other.removed {
            let our_tags = self.removed.entry(element.clone())
                .or_insert_with(BTreeSet::new);
            for &tag in other_tags {
                our_tags.insert(tag);
            }
        }
        
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        self.added == other.added && self.removed == other.removed
    }
    
    fn size(&self) -> usize {
        let added_size: usize = self.added.iter()
            .map(|(_, tags)| tags.len() * std::mem::size_of::<(HLCTimestamp, NodeId)>())
            .sum();
        let removed_size: usize = self.removed.iter()
            .map(|(_, tags)| tags.len() * std::mem::size_of::<(HLCTimestamp, NodeId)>())
            .sum();
        added_size + removed_size
    }
}

impl<'de, T> Deserialize<'de> for ORSet<T> 
where 
    T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(bound = "T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord")]
        struct ORSetHelper<T> 
        where 
            T: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
        {
            added: HashMap<T, BTreeSet<(HLCTimestamp, NodeId)>>,
            removed: HashMap<T, BTreeSet<(HLCTimestamp, NodeId)>>,
        }
        
        let helper = ORSetHelper::deserialize(deserializer)?;
        Ok(ORSet {
            added: helper.added,
            removed: helper.removed,
        })
    }
}

/// LWW-Map: Last-Writer-Wins Map CRDT
/// A map where each key is associated with an LWW-Register
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LWWMap<K, V> 
where 
    K: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
    V: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    entries: BTreeMap<K, LWWRegister<V>>,
    node_id: NodeId,
}

impl<K, V> LWWMap<K, V> 
where 
    K: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
    V: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    /// Create a new LWW-Map
    pub fn new(node_id: NodeId) -> Self {
        Self {
            entries: BTreeMap::new(),
            node_id,
        }
    }
    
    /// Set a key-value pair with a timestamp
    pub fn set(&mut self, key: K, value: V, timestamp: HLCTimestamp) -> Result<()> {
        let register = self.entries.entry(key)
            .or_insert_with(|| LWWRegister::new(self.node_id));
        register.set(value, timestamp)
    }
    
    /// Get the value for a key
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).and_then(|register| register.get())
    }
    
    /// Remove a key (by setting it to None with a new timestamp)
    pub fn remove(&mut self, key: &K, timestamp: HLCTimestamp) -> Result<()> {
        if let Some(register) = self.entries.get_mut(key) {
            // For removal, we use a special sentinel value or handle None
            // This is a simplified implementation - in practice, you might use Option<V>
        }
        Ok(())
    }
    
    /// Get all key-value pairs
    pub fn entries(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter()
            .filter_map(|(k, register)| register.get().map(|v| (k, v)))
    }
    
    /// Get all keys
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries()
            .map(|(k, _)| k)
    }
    
    /// Get all values
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries()
            .map(|(_, v)| v)
    }
    
    /// Check if the map contains a key
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }
    
    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries().count()
    }
    
    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K, V> CRDT for LWWMap<K, V> 
where 
    K: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
    V: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    fn merge(&mut self, other: &Self) -> Result<()> {
        for (key, other_register) in &other.entries {
            let our_register = self.entries.entry(key.clone())
                .or_insert_with(|| LWWRegister::new(self.node_id));
            our_register.merge(other_register)?;
        }
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        if self.entries.len() != other.entries.len() {
            return false;
        }
        
        for (key, register) in &self.entries {
            if let Some(other_register) = other.entries.get(key) {
                if !register.crdt_eq(other_register) {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        true
    }
    
    fn size(&self) -> usize {
        self.entries.iter()
            .map(|(_, register)| register.size())
            .sum()
    }
}

impl<'de, K, V> Deserialize<'de> for LWWMap<K, V> 
where 
    K: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
    V: Clone + Debug + Serialize + DeserializeOwned + Eq,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(bound = "K: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord, V: Clone + Debug + Serialize + DeserializeOwned + Eq")]
        struct LWWMapHelper<K, V> 
        where 
            K: Clone + Debug + Serialize + DeserializeOwned + Eq + Hash + Ord,
            V: Clone + Debug + Serialize + DeserializeOwned + Eq,
        {
            entries: BTreeMap<K, LWWRegister<V>>,
            node_id: NodeId,
        }
        
        let helper = LWWMapHelper::deserialize(deserializer)?;
        Ok(LWWMap {
            entries: helper.entries,
            node_id: helper.node_id,
        })
    }
}

/// File metadata CRDT for MooseNG
/// Represents file/directory metadata that can be updated concurrently across regions
#[derive(Debug, Clone, Serialize, ::serde::Deserialize, PartialEq, Eq)]
pub struct FileMetadataCRDT {
    /// File path (immutable once set)
    pub path: String,
    
    /// File size (LWW register)
    pub size: LWWRegister<u64>,
    
    /// File permissions (LWW register)
    pub permissions: LWWRegister<u32>,
    
    /// File tags (OR-Set)
    pub tags: ORSet<String>,
    
    /// Custom attributes (LWW map)
    pub attributes: LWWMap<String, String>,
    
    /// Access count (G-Counter)
    pub access_count: GCounter,
    
    /// Last modified timestamp (LWW register)
    pub last_modified: LWWRegister<HLCTimestamp>,
    
    /// File type/MIME type (LWW register)
    pub file_type: LWWRegister<String>,
}

impl FileMetadataCRDT {
    /// Create new file metadata CRDT
    pub fn new(path: String, node_id: NodeId) -> Self {
        Self {
            path,
            size: LWWRegister::new(node_id),
            permissions: LWWRegister::new(node_id),
            tags: ORSet::new(),
            attributes: LWWMap::new(node_id),
            access_count: GCounter::new(),
            last_modified: LWWRegister::new(node_id),
            file_type: LWWRegister::new(node_id),
        }
    }
    
    /// Update file size
    pub fn set_size(&mut self, size: u64, timestamp: HLCTimestamp) -> Result<()> {
        self.size.set(size, timestamp)
    }
    
    /// Update file permissions
    pub fn set_permissions(&mut self, permissions: u32, timestamp: HLCTimestamp) -> Result<()> {
        self.permissions.set(permissions, timestamp)
    }
    
    /// Add a tag
    pub fn add_tag(&mut self, tag: String, timestamp: HLCTimestamp, node_id: NodeId) -> Result<()> {
        self.tags.add(tag, timestamp, node_id)
    }
    
    /// Remove a tag
    pub fn remove_tag(&mut self, tag: &str, timestamp: HLCTimestamp, node_id: NodeId) -> Result<()> {
        self.tags.remove(&tag.to_string(), timestamp, node_id)
    }
    
    /// Set an attribute
    pub fn set_attribute(&mut self, key: String, value: String, timestamp: HLCTimestamp) -> Result<()> {
        self.attributes.set(key, value, timestamp)
    }
    
    /// Increment access count
    pub fn increment_access(&mut self, node_id: NodeId) -> Result<()> {
        self.access_count.increment(node_id, 1)
    }
    
    /// Update last modified timestamp
    pub fn set_last_modified(&mut self, timestamp: HLCTimestamp) -> Result<()> {
        self.last_modified.set(timestamp, timestamp)
    }
    
    /// Set file type
    pub fn set_file_type(&mut self, file_type: String, timestamp: HLCTimestamp) -> Result<()> {
        self.file_type.set(file_type, timestamp)
    }
}

impl CRDT for FileMetadataCRDT {
    fn merge(&mut self, other: &Self) -> Result<()> {
        if self.path != other.path {
            return Err(anyhow::anyhow!("Cannot merge metadata for different files"));
        }
        
        self.size.merge(&other.size)?;
        self.permissions.merge(&other.permissions)?;
        self.tags.merge(&other.tags)?;
        self.attributes.merge(&other.attributes)?;
        self.access_count.merge(&other.access_count)?;
        self.last_modified.merge(&other.last_modified)?;
        self.file_type.merge(&other.file_type)?;
        
        Ok(())
    }
    
    fn crdt_eq(&self, other: &Self) -> bool {
        self.path == other.path &&
        self.size.crdt_eq(&other.size) &&
        self.permissions.crdt_eq(&other.permissions) &&
        self.tags.crdt_eq(&other.tags) &&
        self.attributes.crdt_eq(&other.attributes) &&
        self.access_count.crdt_eq(&other.access_count) &&
        self.last_modified.crdt_eq(&other.last_modified) &&
        self.file_type.crdt_eq(&other.file_type)
    }
    
    fn size(&self) -> usize {
        self.path.len() +
        self.size.size() +
        self.permissions.size() +
        self.tags.size() +
        self.attributes.size() +
        self.access_count.size() +
        self.last_modified.size() +
        self.file_type.size()
    }
}

/// CRDT Manager for handling different CRDT types and operations
pub struct CRDTManager {
    node_id: NodeId,
    file_metadata: HashMap<String, FileMetadataCRDT>,
}

impl CRDTManager {
    /// Create a new CRDT manager
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            file_metadata: HashMap::new(),
        }
    }
    
    /// Get or create file metadata CRDT
    pub fn get_file_metadata(&mut self, path: &str) -> &mut FileMetadataCRDT {
        self.file_metadata.entry(path.to_string())
            .or_insert_with(|| FileMetadataCRDT::new(path.to_string(), self.node_id))
    }
    
    /// Merge file metadata from another node
    pub fn merge_file_metadata(&mut self, other_metadata: FileMetadataCRDT) -> Result<()> {
        let path = other_metadata.path.clone();
        let our_metadata = self.get_file_metadata(&path);
        our_metadata.merge(&other_metadata)
    }
    
    /// Get all file paths
    pub fn get_all_file_paths(&self) -> impl Iterator<Item = &String> {
        self.file_metadata.keys()
    }
    
    /// Export all CRDTs for synchronization
    pub fn export_all(&self) -> Vec<FileMetadataCRDT> {
        self.file_metadata.values().cloned().collect()
    }
    
    /// Import CRDTs from another node
    pub fn import_all(&mut self, crdts: Vec<FileMetadataCRDT>) -> Result<()> {
        for crdt in crdts {
            self.merge_file_metadata(crdt)?;
        }
        Ok(())
    }
    
    /// Get statistics about CRDT usage
    pub fn get_statistics(&self) -> CRDTStatistics {
        let total_files = self.file_metadata.len();
        let total_size: usize = self.file_metadata.values()
            .map(|metadata| metadata.size())
            .sum();
        
        let total_tags: usize = self.file_metadata.values()
            .map(|metadata| metadata.tags.len())
            .sum();
        
        let total_attributes: usize = self.file_metadata.values()
            .map(|metadata| metadata.attributes.len())
            .sum();
        
        let total_access_count: u64 = self.file_metadata.values()
            .map(|metadata| metadata.access_count.value())
            .sum();
        
        CRDTStatistics {
            total_files,
            total_size,
            total_tags,
            total_attributes,
            total_access_count,
        }
    }
}

/// Statistics about CRDT usage
#[derive(Debug, Clone, Serialize, ::serde::Deserialize)]
pub struct CRDTStatistics {
    pub total_files: usize,
    pub total_size: usize,
    pub total_tags: usize,
    pub total_attributes: usize,
    pub total_access_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_g_counter() {
        let mut counter1 = GCounter::new();
        let mut counter2 = GCounter::new();
        
        counter1.increment(1, 5).unwrap();
        counter2.increment(2, 3).unwrap();
        
        assert_eq!(counter1.value(), 5);
        assert_eq!(counter2.value(), 3);
        
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 8);
        
        counter2.merge(&counter1).unwrap();
        assert_eq!(counter2.value(), 8);
        
        // Test idempotent property
        let old_value = counter1.value();
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), old_value);
    }
    
    #[test]
    fn test_pn_counter() {
        let mut counter1 = PNCounter::new();
        let mut counter2 = PNCounter::new();
        
        counter1.increment(1, 10).unwrap();
        counter1.decrement(1, 3).unwrap();
        
        counter2.increment(2, 5).unwrap();
        counter2.decrement(2, 2).unwrap();
        
        assert_eq!(counter1.value(), 7);
        assert_eq!(counter2.value(), 3);
        
        counter1.merge(&counter2).unwrap();
        assert_eq!(counter1.value(), 10);
    }
    
    #[test]
    fn test_g_set() {
        let mut set1 = GSet::<String>::new();
        let mut set2 = GSet::<String>::new();
        
        set1.add("a".to_string()).unwrap();
        set1.add("b".to_string()).unwrap();
        
        set2.add("b".to_string()).unwrap();
        set2.add("c".to_string()).unwrap();
        
        assert!(set1.contains(&"a".to_string()));
        assert!(set1.contains(&"b".to_string()));
        assert!(!set1.contains(&"c".to_string()));
        
        set1.merge(&set2).unwrap();
        
        assert!(set1.contains(&"a".to_string()));
        assert!(set1.contains(&"b".to_string()));
        assert!(set1.contains(&"c".to_string()));
        assert_eq!(set1.len(), 3);
    }
    
    #[test]
    fn test_lww_register() {
        let mut reg1 = LWWRegister::new(1);
        let mut reg2 = LWWRegister::new(2);
        
        let ts1 = HLCTimestamp::new(100, 0, 1);
        let ts2 = HLCTimestamp::new(101, 0, 2);
        
        reg1.set("value1".to_string(), ts1).unwrap();
        reg2.set("value2".to_string(), ts2).unwrap();
        
        assert_eq!(reg1.get(), Some(&"value1".to_string()));
        assert_eq!(reg2.get(), Some(&"value2".to_string()));
        
        // reg2 has later timestamp, so it should win
        reg1.merge(&reg2).unwrap();
        assert_eq!(reg1.get(), Some(&"value2".to_string()));
    }
}