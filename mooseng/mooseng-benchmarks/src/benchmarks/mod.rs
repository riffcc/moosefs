//! Benchmark modules for different aspects of MooseNG

pub mod file_operations;
pub mod metadata_operations;
pub mod multiregion;
pub mod multiregion_performance;
pub mod enhanced_multiregion;
pub mod comparison;
pub mod network_simulation;
pub mod network_file_operations;
pub mod real_network;
pub mod integration;
pub mod infrastructure;
pub mod multiregion_infrastructure;
pub mod performance_analysis;

use crate::{Benchmark, BenchmarkConfig, BenchmarkResult};
use std::sync::Arc;

/// Collection of all available benchmarks
pub struct BenchmarkSuite {
    benchmarks: Vec<Arc<dyn Benchmark + Send + Sync>>,
    categories: std::collections::HashMap<String, Vec<String>>,
}

impl BenchmarkSuite {
    /// Create a new benchmark suite with all available benchmarks
    pub fn new() -> Self {
        let benchmarks: Vec<Arc<dyn Benchmark + Send + Sync>> = vec![
            Arc::new(file_operations::SmallFileBenchmark::new()),
            Arc::new(file_operations::LargeFileBenchmark::new()),
            Arc::new(file_operations::RandomAccessBenchmark::new()),
            Arc::new(metadata_operations::CreateDeleteBenchmark::new()),
            Arc::new(metadata_operations::ListingBenchmark::new()),
            Arc::new(metadata_operations::AttributesBenchmark::new()),
            Arc::new(multiregion::CrossRegionReplicationBenchmark::new()),
            Arc::new(multiregion::ConsistencyBenchmark::new()),
            Arc::new(multiregion_performance::MultiRegionReplicationBenchmark::new()),
            Arc::new(multiregion_performance::GeographicLatencyBenchmark::new()),
            Arc::new(enhanced_multiregion::EnhancedMultiRegionBenchmark::new()),
            Arc::new(comparison::MooseFSComparisonBenchmark::new()),
            Arc::new(network_simulation::NetworkConditionBenchmark::new()),
            Arc::new(network_simulation::RealNetworkBenchmark::new()),
            Arc::new(network_file_operations::NetworkFileBenchmark::new()),
            Arc::new(network_file_operations::MultiRegionLatencyBenchmark::new()),
            Arc::new(real_network::RealNetworkBenchmark::new("lo")),
            Arc::new(multiregion_infrastructure::MultiRegionBenchmark::new()),
            Arc::new(performance_analysis::PerformanceAnalysisBenchmark::new()),
        ];
        
        // Define categories
        let mut categories = std::collections::HashMap::new();
        categories.insert("file_operations".to_string(), vec![
            "small_file_benchmark".to_string(),
            "large_file_benchmark".to_string(),
            "random_access_benchmark".to_string(),
        ]);
        categories.insert("metadata".to_string(), vec![
            "create_delete_benchmark".to_string(),
            "listing_benchmark".to_string(),
            "attributes_benchmark".to_string(),
        ]);
        categories.insert("multiregion".to_string(), vec![
            "cross_region_replication_benchmark".to_string(),
            "consistency_benchmark".to_string(),
            "multiregion_replication_benchmark".to_string(),
            "geographic_latency_benchmark".to_string(),
            "enhanced_multi_region".to_string(),
        ]);
        categories.insert("network".to_string(), vec![
            "network_condition_benchmark".to_string(),
            "real_network_benchmark".to_string(),
            "network_file_benchmark".to_string(),
            "multiregion_latency_benchmark".to_string(),
        ]);
        categories.insert("comparison".to_string(), vec![
            "moosefs_comparison_benchmark".to_string(),
        ]);
        
        Self { benchmarks, categories }
    }
    
    /// Run all benchmarks
    pub fn run_all(&self, config: &BenchmarkConfig) -> Vec<(String, Vec<BenchmarkResult>)> {
        self.benchmarks
            .iter()
            .map(|bench| (bench.name().to_string(), bench.run(config)))
            .collect()
    }
    
    /// Run specific benchmarks by name
    pub fn run_selected(&self, names: &[&str], config: &BenchmarkConfig) -> Vec<(String, Vec<BenchmarkResult>)> {
        self.benchmarks
            .iter()
            .filter(|bench| names.contains(&bench.name()))
            .map(|bench| (bench.name().to_string(), bench.run(config)))
            .collect()
    }
    
    /// Run benchmarks by category
    pub fn run_category(&self, category: &str, config: &BenchmarkConfig) -> Vec<(String, Vec<BenchmarkResult>)> {
        if let Some(benchmark_names) = self.categories.get(category) {
            let names: Vec<&str> = benchmark_names.iter().map(|s| s.as_str()).collect();
            self.run_selected(&names, config)
        } else {
            Vec::new()
        }
    }
    
    /// List all available benchmarks with descriptions
    pub fn list_benchmarks(&self) -> Vec<(String, String)> {
        self.benchmarks
            .iter()
            .map(|bench| (bench.name().to_string(), bench.description().to_string()))
            .collect()
    }
    
    /// List all available categories
    pub fn list_categories(&self) -> Vec<String> {
        self.categories.keys().cloned().collect()
    }
    
    /// Get benchmark count by category
    pub fn category_stats(&self) -> std::collections::HashMap<String, usize> {
        self.categories
            .iter()
            .map(|(category, benchmarks)| (category.clone(), benchmarks.len()))
            .collect()
    }
}