use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use tracing::warn;

use crate::erasure::{ErasureConfig, ServerLocation};

/// Advanced chunk placement strategy for erasure-coded data
#[derive(Debug, Clone)]
pub struct ErasurePlacementStrategy {
    config: ErasureConfig,
    topology: ServerTopology,
    placement_rules: PlacementRules,
}

/// Server topology information
#[derive(Debug, Clone)]
pub struct ServerTopology {
    servers: HashMap<String, ServerInfo>,
    regions: HashMap<String, HashSet<String>>,
    racks: HashMap<String, HashSet<String>>,
}

/// Detailed server information
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub id: String,
    pub location: ServerLocation,
    pub capacity: ServerCapacity,
    pub status: ServerStatus,
    pub network_distance: HashMap<String, u32>, // Distance to other servers
}

/// Server capacity information
#[derive(Debug, Clone)]
pub struct ServerCapacity {
    pub total_space: u64,
    pub used_space: u64,
    pub shard_count: u64,
    pub bandwidth_mbps: u32,
    pub iops_limit: u32,
}

/// Server status
#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Active,
    Degraded,
    Maintenance,
    Offline,
}

/// Placement rules and constraints
#[derive(Debug, Clone)]
pub struct PlacementRules {
    pub min_rack_spread: usize,
    pub min_region_spread: usize,
    pub prefer_local_region: bool,
    pub avoid_overloaded_threshold: f64,
    pub network_aware: bool,
}

impl Default for PlacementRules {
    fn default() -> Self {
        Self {
            min_rack_spread: 2,
            min_region_spread: 1,
            prefer_local_region: true,
            avoid_overloaded_threshold: 0.85,
            network_aware: true,
        }
    }
}

/// Placement decision for a set of shards
#[derive(Debug, Clone)]
pub struct PlacementDecision {
    pub shard_assignments: Vec<ShardAssignment>,
    pub score: f64,
    pub meets_constraints: bool,
}

/// Assignment of a shard to servers
#[derive(Debug, Clone)]
pub struct ShardAssignment {
    pub shard_index: usize,
    pub primary_server: String,
    pub replica_servers: Vec<String>,
}

impl ErasurePlacementStrategy {
    pub fn new(config: ErasureConfig) -> Self {
        Self {
            config,
            topology: ServerTopology::new(),
            placement_rules: PlacementRules::default(),
        }
    }
    
    /// Update placement rules
    pub fn set_rules(&mut self, rules: PlacementRules) {
        self.placement_rules = rules;
    }
    
    /// Add or update server information
    pub fn update_server(&mut self, info: ServerInfo) {
        let server_id = info.id.clone();
        let region = info.location.region.clone();
        let rack = info.location.rack.clone();
        
        self.topology.servers.insert(server_id.clone(), info);
        self.topology.regions.entry(region).or_insert_with(HashSet::new).insert(server_id.clone());
        self.topology.racks.entry(rack).or_insert_with(HashSet::new).insert(server_id.clone());
    }
    
    /// Calculate optimal placement for erasure-coded shards
    pub fn calculate_placement(&self, local_region: Option<&str>) -> Result<PlacementDecision> {
        let active_servers = self.get_active_servers();
        
        if active_servers.len() < self.config.total_shards() {
            return Err(anyhow!(
                "Not enough active servers ({}) for {} shards",
                active_servers.len(),
                self.config.total_shards()
            ));
        }
        
        // Generate candidate placements
        let candidates = self.generate_placement_candidates(&active_servers, local_region)?;
        
        // Score and select best placement
        let best_placement = candidates
            .into_iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
            .ok_or_else(|| anyhow!("No valid placement found"))?;
        
        if !best_placement.meets_constraints {
            warn!("Best placement does not meet all constraints");
        }
        
        Ok(best_placement)
    }
    
    /// Get list of active servers
    fn get_active_servers(&self) -> Vec<&ServerInfo> {
        self.topology.servers
            .values()
            .filter(|s| s.status == ServerStatus::Active || s.status == ServerStatus::Degraded)
            .collect()
    }
    
    /// Generate placement candidates
    fn generate_placement_candidates(
        &self,
        servers: &[&ServerInfo],
        local_region: Option<&str>,
    ) -> Result<Vec<PlacementDecision>> {
        let mut candidates = Vec::new();
        
        // Strategy 1: Region-aware placement
        if self.placement_rules.min_region_spread > 0 {
            if let Some(placement) = self.generate_region_aware_placement(servers, local_region) {
                candidates.push(placement);
            }
        }
        
        // Strategy 2: Rack-aware placement
        if self.placement_rules.min_rack_spread > 0 {
            if let Some(placement) = self.generate_rack_aware_placement(servers) {
                candidates.push(placement);
            }
        }
        
        // Strategy 3: Capacity-balanced placement
        if let Some(placement) = self.generate_capacity_balanced_placement(servers) {
            candidates.push(placement);
        }
        
        // Strategy 4: Network-optimized placement
        if self.placement_rules.network_aware {
            if let Some(placement) = self.generate_network_optimized_placement(servers, local_region) {
                candidates.push(placement);
            }
        }
        
        if candidates.is_empty() {
            // Fallback to simple round-robin
            candidates.push(self.generate_simple_placement(servers));
        }
        
        Ok(candidates)
    }
    
    /// Generate region-aware placement
    fn generate_region_aware_placement(
        &self,
        servers: &[&ServerInfo],
        local_region: Option<&str>,
    ) -> Option<PlacementDecision> {
        let mut assignments = Vec::new();
        let mut used_servers = HashSet::new();
        let mut region_counts = HashMap::new();
        
        // Group servers by region
        let mut servers_by_region: HashMap<String, Vec<&ServerInfo>> = HashMap::new();
        for server in servers {
            servers_by_region
                .entry(server.location.region.clone())
                .or_insert_with(Vec::new)
                .push(server);
        }
        
        // Prioritize data shards in local region if specified
        let mut shard_idx = 0;
        
        // Place data shards
        if let Some(local) = local_region {
            if let Some(local_servers) = servers_by_region.get(local) {
                for server in local_servers.iter().take(self.config.data_shards) {
                    if !used_servers.contains(&server.id) {
                        assignments.push(ShardAssignment {
                            shard_index: shard_idx,
                            primary_server: server.id.clone(),
                            replica_servers: vec![],
                        });
                        used_servers.insert(server.id.clone());
                        *region_counts.entry(local.to_string()).or_insert(0) += 1;
                        shard_idx += 1;
                        
                        if shard_idx >= self.config.data_shards {
                            break;
                        }
                    }
                }
            }
        }
        
        // Place remaining shards across regions
        let regions: Vec<_> = servers_by_region.keys().cloned().collect();
        let mut region_idx = 0;
        
        while shard_idx < self.config.total_shards() {
            let region = &regions[region_idx % regions.len()];
            if let Some(region_servers) = servers_by_region.get(region) {
                for server in region_servers {
                    if !used_servers.contains(&server.id) {
                        assignments.push(ShardAssignment {
                            shard_index: shard_idx,
                            primary_server: server.id.clone(),
                            replica_servers: vec![],
                        });
                        used_servers.insert(server.id.clone());
                        *region_counts.entry(region.clone()).or_insert(0) += 1;
                        shard_idx += 1;
                        break;
                    }
                }
            }
            region_idx += 1;
        }
        
        if assignments.len() < self.config.total_shards() {
            return None;
        }
        
        let score = self.score_placement(&assignments, &region_counts);
        let meets_constraints = region_counts.len() >= self.placement_rules.min_region_spread;
        
        Some(PlacementDecision {
            shard_assignments: assignments,
            score,
            meets_constraints,
        })
    }
    
    /// Generate rack-aware placement
    fn generate_rack_aware_placement(&self, servers: &[&ServerInfo]) -> Option<PlacementDecision> {
        let mut assignments = Vec::new();
        let mut used_servers = HashSet::new();
        let mut rack_counts = HashMap::new();
        
        // Group servers by rack
        let mut servers_by_rack: HashMap<String, Vec<&ServerInfo>> = HashMap::new();
        for server in servers {
            servers_by_rack
                .entry(server.location.rack.clone())
                .or_insert_with(Vec::new)
                .push(server);
        }
        
        let racks: Vec<_> = servers_by_rack.keys().cloned().collect();
        let mut rack_idx = 0;
        
        for shard_idx in 0..self.config.total_shards() {
            let rack = &racks[rack_idx % racks.len()];
            if let Some(rack_servers) = servers_by_rack.get(rack) {
                for server in rack_servers {
                    if !used_servers.contains(&server.id) {
                        assignments.push(ShardAssignment {
                            shard_index: shard_idx,
                            primary_server: server.id.clone(),
                            replica_servers: vec![],
                        });
                        used_servers.insert(server.id.clone());
                        *rack_counts.entry(rack.clone()).or_insert(0) += 1;
                        break;
                    }
                }
            }
            rack_idx += 1;
        }
        
        if assignments.len() < self.config.total_shards() {
            return None;
        }
        
        let region_counts = self.count_regions(&assignments);
        let score = self.score_placement(&assignments, &region_counts);
        let meets_constraints = rack_counts.len() >= self.placement_rules.min_rack_spread;
        
        Some(PlacementDecision {
            shard_assignments: assignments,
            score,
            meets_constraints,
        })
    }
    
    /// Generate capacity-balanced placement
    fn generate_capacity_balanced_placement(&self, servers: &[&ServerInfo]) -> Option<PlacementDecision> {
        let mut assignments = Vec::new();
        
        // Sort servers by available capacity
        let mut sorted_servers: Vec<_> = servers.to_vec();
        sorted_servers.sort_by_key(|s| {
            let used_ratio = s.capacity.used_space as f64 / s.capacity.total_space as f64;
            (used_ratio * 1000.0) as i64
        });
        
        // Avoid overloaded servers
        let available_servers: Vec<_> = sorted_servers
            .into_iter()
            .filter(|s| {
                let used_ratio = s.capacity.used_space as f64 / s.capacity.total_space as f64;
                used_ratio < self.placement_rules.avoid_overloaded_threshold
            })
            .collect();
        
        if available_servers.len() < self.config.total_shards() {
            return None;
        }
        
        for (shard_idx, server) in available_servers.iter().take(self.config.total_shards()).enumerate() {
            assignments.push(ShardAssignment {
                shard_index: shard_idx,
                primary_server: server.id.clone(),
                replica_servers: vec![],
            });
        }
        
        let region_counts = self.count_regions(&assignments);
        let score = self.score_placement(&assignments, &region_counts);
        let rack_spread = self.count_racks(&assignments);
        let meets_constraints = rack_spread >= self.placement_rules.min_rack_spread &&
                               region_counts.len() >= self.placement_rules.min_region_spread;
        
        Some(PlacementDecision {
            shard_assignments: assignments,
            score,
            meets_constraints,
        })
    }
    
    /// Generate network-optimized placement
    fn generate_network_optimized_placement(
        &self,
        servers: &[&ServerInfo],
        local_region: Option<&str>,
    ) -> Option<PlacementDecision> {
        let mut assignments = Vec::new();
        
        // If local region specified, find servers with lowest network distance
        let reference_servers: Vec<_> = if let Some(local) = local_region {
            servers.iter()
                .filter(|s| s.location.region == local)
                .cloned()
                .collect()
        } else {
            vec![servers[0]] // Use first server as reference
        };
        
        if reference_servers.is_empty() {
            return None;
        }
        
        // Calculate average network distance from reference
        let mut server_distances: Vec<(&&ServerInfo, u32)> = servers.iter()
            .map(|server| {
                let avg_distance = reference_servers.iter()
                    .filter_map(|ref_server| {
                        ref_server.network_distance.get(&server.id)
                    })
                    .sum::<u32>() / reference_servers.len() as u32;
                (server, avg_distance)
            })
            .collect();
        
        // Sort by network distance
        server_distances.sort_by_key(|(_, dist)| *dist);
        
        // Place shards starting with closest servers
        for (shard_idx, (server, _)) in server_distances.iter().take(self.config.total_shards()).enumerate() {
            assignments.push(ShardAssignment {
                shard_index: shard_idx,
                primary_server: server.id.clone(),
                replica_servers: vec![],
            });
        }
        
        let region_counts = self.count_regions(&assignments);
        let score = self.score_placement(&assignments, &region_counts);
        let rack_spread = self.count_racks(&assignments);
        let meets_constraints = rack_spread >= self.placement_rules.min_rack_spread &&
                               region_counts.len() >= self.placement_rules.min_region_spread;
        
        Some(PlacementDecision {
            shard_assignments: assignments,
            score,
            meets_constraints,
        })
    }
    
    /// Generate simple round-robin placement
    fn generate_simple_placement(&self, servers: &[&ServerInfo]) -> PlacementDecision {
        let assignments: Vec<_> = (0..self.config.total_shards())
            .map(|shard_idx| {
                let server_idx = shard_idx % servers.len();
                ShardAssignment {
                    shard_index: shard_idx,
                    primary_server: servers[server_idx].id.clone(),
                    replica_servers: vec![],
                }
            })
            .collect();
        
        let region_counts = self.count_regions(&assignments);
        let score = self.score_placement(&assignments, &region_counts);
        let rack_spread = self.count_racks(&assignments);
        let meets_constraints = rack_spread >= self.placement_rules.min_rack_spread &&
                               region_counts.len() >= self.placement_rules.min_region_spread;
        
        PlacementDecision {
            shard_assignments: assignments,
            score,
            meets_constraints,
        }
    }
    
    /// Score a placement decision
    fn score_placement(&self, assignments: &[ShardAssignment], region_counts: &HashMap<String, usize>) -> f64 {
        let mut score = 0.0;
        
        // Factor 1: Region distribution (higher is better)
        let region_spread = region_counts.len() as f64 / self.topology.regions.len() as f64;
        score += region_spread * 30.0;
        
        // Factor 2: Rack distribution
        let rack_spread = self.count_racks(assignments) as f64 / self.topology.racks.len() as f64;
        score += rack_spread * 20.0;
        
        // Factor 3: Capacity balance
        let capacity_score = self.calculate_capacity_balance(assignments);
        score += capacity_score * 25.0;
        
        // Factor 4: Network efficiency
        let network_score = self.calculate_network_efficiency(assignments);
        score += network_score * 25.0;
        
        score
    }
    
    /// Count regions used in assignments
    fn count_regions(&self, assignments: &[ShardAssignment]) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        
        for assignment in assignments {
            if let Some(server) = self.topology.servers.get(&assignment.primary_server) {
                *counts.entry(server.location.region.clone()).or_insert(0) += 1;
            }
        }
        
        counts
    }
    
    /// Count racks used in assignments
    fn count_racks(&self, assignments: &[ShardAssignment]) -> usize {
        let mut racks = HashSet::new();
        
        for assignment in assignments {
            if let Some(server) = self.topology.servers.get(&assignment.primary_server) {
                racks.insert(server.location.rack.clone());
            }
        }
        
        racks.len()
    }
    
    /// Calculate capacity balance score (returns higher score for better balance)
    fn calculate_capacity_balance(&self, assignments: &[ShardAssignment]) -> f64 {
        let mut server_loads = HashMap::new();
        
        for assignment in assignments {
            *server_loads.entry(&assignment.primary_server).or_insert(0) += 1;
        }
        
        if server_loads.is_empty() {
            return 0.0;
        }
        
        let avg_load = assignments.len() as f64 / server_loads.len() as f64;
        let variance: f64 = server_loads.values()
            .map(|&load| {
                let diff = load as f64 - avg_load;
                diff * diff
            })
            .sum::<f64>() / server_loads.len() as f64;
        
        // Lower variance is better
        1.0 / (1.0 + variance.sqrt())
    }
    
    /// Calculate network efficiency score (returns higher score for lower latency)
    fn calculate_network_efficiency(&self, assignments: &[ShardAssignment]) -> f64 {
        // Simple implementation: prefer assignments with lower total network distance
        let mut total_distance = 0u32;
        let mut pair_count = 0;
        
        for i in 0..assignments.len() {
            for j in i+1..assignments.len() {
                let server1 = &assignments[i].primary_server;
                let server2 = &assignments[j].primary_server;
                
                if let Some(info1) = self.topology.servers.get(server1) {
                    if let Some(&distance) = info1.network_distance.get(server2) {
                        total_distance += distance;
                        pair_count += 1;
                    }
                }
            }
        }
        
        if pair_count == 0 {
            return 0.5; // Default score
        }
        
        let avg_distance = total_distance as f64 / pair_count as f64;
        // Normalize: assume max distance is 1000
        1.0 - (avg_distance / 1000.0).min(1.0)
    }
}

impl ServerTopology {
    fn new() -> Self {
        Self {
            servers: HashMap::new(),
            regions: HashMap::new(),
            racks: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_server(id: &str, region: &str, rack: &str) -> ServerInfo {
        ServerInfo {
            id: id.to_string(),
            location: ServerLocation {
                region: region.to_string(),
                rack: rack.to_string(),
                server_id: id.to_string(),
            },
            capacity: ServerCapacity {
                total_space: 1000,
                used_space: 100,
                shard_count: 10,
                bandwidth_mbps: 1000,
                iops_limit: 10000,
            },
            status: ServerStatus::Active,
            network_distance: HashMap::new(),
        }
    }
    
    #[test]
    fn test_placement_strategy() {
        let mut strategy = ErasurePlacementStrategy::new(ErasureConfig::config_4_2());
        
        // Add servers across regions and racks
        strategy.update_server(create_test_server("s1", "us-east", "rack1"));
        strategy.update_server(create_test_server("s2", "us-east", "rack2"));
        strategy.update_server(create_test_server("s3", "us-west", "rack1"));
        strategy.update_server(create_test_server("s4", "us-west", "rack2"));
        strategy.update_server(create_test_server("s5", "eu-west", "rack1"));
        strategy.update_server(create_test_server("s6", "eu-west", "rack2"));
        
        let placement = strategy.calculate_placement(Some("us-east")).unwrap();
        
        assert_eq!(placement.shard_assignments.len(), 6); // 4 data + 2 parity
        assert!(placement.meets_constraints);
        assert!(placement.score > 0.0);
    }
}