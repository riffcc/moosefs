//! Test runner and performance analysis for Raft implementation
//! 
//! This module provides utilities to run comprehensive tests and analyze
//! the performance of the Raft consensus implementation.

use super::*;
use super::optimization::{OptimizedRaftConsensus, RaftPerformanceMetrics, PerformanceTuningRecommendations};
use anyhow::Result;
use std::time::{Duration, Instant};

/// Helper to create a test-friendly Raft configuration
fn create_test_raft_config(node_id: &str, data_dir: &str, peers: Vec<String>) -> RaftConfig {
    RaftConfig {
        node_id: node_id.to_string(),
        data_dir: data_dir.into(),
        initial_members: peers,
        election_timeout_ms: 200,
        heartbeat_interval_ms: 50,
        rpc_timeout_ms: 100,
        snapshot_interval: 1000,
        raft_port: 9000,
        max_log_entries: 100,
        max_snapshot_size: 64 * 1024,
        batch_size: 32,
        pipeline_enabled: true,
        pre_vote_enabled: true,
        check_quorum: true,
        leader_lease_timeout_ms: 300,
        max_append_entries: Some(100),
        election_timeout_min_ms: 150,
        election_timeout_max_ms: 300,
        read_scaling_enabled: true,
        read_lease_duration_ms: 30000,
    }
}

/// Comprehensive test suite runner
pub struct RaftTestRunner {
    pub test_results: Vec<TestResult>,
    pub performance_metrics: Option<RaftPerformanceMetrics>,
    pub tuning_recommendations: Option<PerformanceTuningRecommendations>,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub test_name: String,
    pub passed: bool,
    pub duration: Duration,
    pub error_message: Option<String>,
}

impl RaftTestRunner {
    pub fn new() -> Self {
        Self {
            test_results: Vec::new(),
            performance_metrics: None,
            tuning_recommendations: None,
        }
    }
    
    /// Run all Raft tests and collect results
    pub async fn run_comprehensive_test_suite(&mut self) -> Result<()> {
        println!("🚀 Starting comprehensive Raft test suite...");
        
        // Basic functionality tests
        self.run_test("basic_creation", Self::test_basic_creation).await;
        self.run_test("start_stop_lifecycle", Self::test_start_stop_lifecycle).await;
        self.run_test("safety_checks", Self::test_safety_checks).await;
        self.run_test("configuration_management", Self::test_configuration_management).await;
        self.run_test("read_scaling", Self::test_read_scaling).await;
        
        // Performance tests
        self.run_test("performance_safety_checks", Self::test_performance_safety_checks).await;
        self.run_test("performance_config_queries", Self::test_performance_config_queries).await;
        self.run_test("concurrent_operations", Self::test_concurrent_operations).await;
        
        // Optimization tests
        self.run_test("optimization_caching", Self::test_optimization_caching).await;
        self.run_test("optimization_metrics", Self::test_optimization_metrics).await;
        
        // Stress tests
        self.run_test("stress_sustained_load", Self::test_stress_sustained_load).await;
        self.run_test("stress_rapid_queries", Self::test_stress_rapid_queries).await;
        
        self.analyze_results().await?;
        
        Ok(())
    }
    
    /// Run a single test and record the result
    async fn run_test<F, Fut>(&mut self, test_name: &str, test_fn: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        println!("  Running test: {}", test_name);
        let start_time = Instant::now();
        
        let result = match test_fn().await {
            Ok(_) => TestResult {
                test_name: test_name.to_string(),
                passed: true,
                duration: start_time.elapsed(),
                error_message: None,
            },
            Err(e) => TestResult {
                test_name: test_name.to_string(),
                passed: false,
                duration: start_time.elapsed(),
                error_message: Some(e.to_string()),
            },
        };
        
        if result.passed {
            println!("    ✅ {} completed in {:?}", test_name, result.duration);
        } else {
            println!("    ❌ {} failed in {:?}: {}", 
                    test_name, result.duration, 
                    result.error_message.as_ref().unwrap_or(&"Unknown error".to_string()));
        }
        
        self.test_results.push(result);
    }
    
    /// Basic creation test
    async fn test_basic_creation() -> Result<()> {
        let config = create_test_raft_config(
            "test_node",
            "/tmp/raft_test_basic",
            vec!["peer1".to_string(), "peer2".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        assert!(!raft.is_leader().await);
        
        Ok(())
    }
    
    /// Start/stop lifecycle test
    async fn test_start_stop_lifecycle() -> Result<()> {
        let config = create_test_raft_config(
            "test_lifecycle",
            "/tmp/raft_test_lifecycle",
            vec!["peer1".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        raft.start().await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        raft.stop().await?;
        
        Ok(())
    }
    
    /// Safety checks test
    async fn test_safety_checks() -> Result<()> {
        let config = create_test_raft_config(
            "test_safety",
            "/tmp/raft_test_safety",
            vec!["peer1".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        
        // Run multiple safety checks
        for _ in 0..10 {
            raft.check_safety().await?;
        }
        
        assert!(raft.can_start_election().await?);
        
        Ok(())
    }
    
    /// Configuration management test
    async fn test_configuration_management() -> Result<()> {
        let config = create_test_raft_config(
            "test_config",
            "/tmp/raft_test_config",
            vec!["peer1".to_string(), "peer2".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        
        let cluster_config = raft.get_cluster_config().await;
        assert!(!cluster_config.voting_members.is_empty());
        
        let targets = raft.get_replication_targets().await;
        assert!(!targets.is_empty());
        
        let majority = raft.get_majority_size().await;
        assert!(majority > 0);
        
        Ok(())
    }
    
    /// Read scaling test
    async fn test_read_scaling() -> Result<()> {
        let config = create_test_raft_config(
            "test_reads",
            "/tmp/raft_test_reads",
            vec!["peer1".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        
        let request = ReadRequest {
            consistency: ReadConsistency::Sequential,
            min_commit_index: None,
        };
        
        let _can_serve = raft.can_serve_read(&request).await?;
        let _needs_renewal = raft.needs_lease_renewal().await;
        let _is_valid = raft.validate_read_lease().await;
        
        Ok(())
    }
    
    /// Performance test for safety checks
    async fn test_performance_safety_checks() -> Result<()> {
        let config = create_test_raft_config(
            "test_perf_safety",
            "/tmp/raft_test_perf_safety",
            vec!["peer1".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        
        let start_time = Instant::now();
        let iterations = 1000;
        
        for _ in 0..iterations {
            raft.check_safety().await?;
        }
        
        let total_duration = start_time.elapsed();
        let avg_duration = total_duration / iterations;
        
        // Safety checks should be reasonably fast (< 1ms each on average)
        if avg_duration > Duration::from_millis(1) {
            return Err(anyhow::anyhow!(
                "Safety checks too slow: {:?} average (expected < 1ms)", avg_duration
            ));
        }
        
        println!("    Performance: {} safety checks in {:?} (avg: {:?})", 
                iterations, total_duration, avg_duration);
        
        Ok(())
    }
    
    /// Performance test for configuration queries
    async fn test_performance_config_queries() -> Result<()> {
        let config = create_test_raft_config(
            "test_perf_config",
            "/tmp/raft_test_perf_config",
            vec!["peer1".to_string(), "peer2".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        
        let start_time = Instant::now();
        let iterations = 500;
        
        for _ in 0..iterations {
            let _ = raft.get_cluster_config().await;
            let _ = raft.get_replication_targets().await;
            let _ = raft.get_majority_size().await;
        }
        
        let total_duration = start_time.elapsed();
        let avg_duration = total_duration / (iterations * 3);
        
        println!("    Performance: {} config queries in {:?} (avg: {:?})", 
                iterations * 3, total_duration, avg_duration);
        
        Ok(())
    }
    
    /// Concurrent operations test
    async fn test_concurrent_operations() -> Result<()> {
        let config = create_test_raft_config(
            "test_concurrent",
            "/tmp/raft_test_concurrent",
            vec!["peer1".to_string()]
        );
        
        let raft = std::sync::Arc::new(RaftConsensus::new(config).await?);
        
        use tokio::task::JoinSet;
        let mut tasks = JoinSet::new();
        
        // Run concurrent operations
        for i in 0..10 {
            let raft_clone = raft.clone();
            tasks.spawn(async move {
                for _j in 0..20 {
                    let _ = raft_clone.check_safety().await;
                    let _ = raft_clone.is_leader().await;
                    let _ = raft_clone.get_cluster_config().await;
                    tokio::task::yield_now().await;
                }
            });
        }
        
        // Wait for all tasks to complete
        while let Some(_) = tasks.join_next().await {}
        
        Ok(())
    }
    
    /// Optimization caching test
    async fn test_optimization_caching() -> Result<()> {
        let config = create_test_raft_config(
            "test_opt_cache",
            "/tmp/raft_test_opt_cache",
            vec!["peer1".to_string()]
        );
        
        let optimized_raft = OptimizedRaftConsensus::new(config).await?;
        
        // Test caching by making repeated calls
        for _ in 0..10 {
            let _ = optimized_raft.get_cluster_config().await;
            let _ = optimized_raft.get_replication_targets().await;
            let _ = optimized_raft.get_majority_size().await;
        }
        
        // Check cache hit ratio
        let hit_ratio = optimized_raft.get_config_cache_hit_ratio().await;
        println!("    Cache hit ratio: {:.1}%", hit_ratio * 100.0);
        
        // Should have some cache hits
        assert!(hit_ratio > 0.0);
        
        Ok(())
    }
    
    /// Optimization metrics test
    async fn test_optimization_metrics() -> Result<()> {
        let config = create_test_raft_config(
            "test_opt_metrics",
            "/tmp/raft_test_opt_metrics",
            vec!["peer1".to_string()]
        );
        
        let optimized_raft = OptimizedRaftConsensus::new(config).await?;
        
        // Perform operations to generate metrics
        for _ in 0..20 {
            let _ = optimized_raft.check_safety().await;
            let _ = optimized_raft.is_leader().await;
            let _ = optimized_raft.get_cluster_config().await;
        }
        
        let metrics = optimized_raft.get_performance_metrics().await;
        
        // Should have collected metrics
        assert!(metrics.safety_checks > 0);
        assert!(metrics.leadership_queries > 0);
        
        println!("    Metrics: {} safety checks, {} leadership queries", 
                metrics.safety_checks, metrics.leadership_queries);
        
        Ok(())
    }
    
    /// Stress test with sustained load
    async fn test_stress_sustained_load() -> Result<()> {
        let config = create_test_raft_config(
            "test_stress_load",
            "/tmp/raft_test_stress_load",
            vec!["peer1".to_string()]
        );
        
        let raft = std::sync::Arc::new(RaftConsensus::new(config).await?);
        
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::task::JoinSet;
        
        let success_counter = std::sync::Arc::new(AtomicUsize::new(0));
        let error_counter = std::sync::Arc::new(AtomicUsize::new(0));
        
        let mut tasks = JoinSet::new();
        
        // Run sustained load for 1 second
        for _i in 0..5 {
            let raft_clone = raft.clone();
            let success_clone = success_counter.clone();
            let error_clone = error_counter.clone();
            
            tasks.spawn(async move {
                let start_time = Instant::now();
                let duration = Duration::from_millis(1000);
                
                while start_time.elapsed() < duration {
                    match raft_clone.check_safety().await {
                        Ok(_) => success_clone.fetch_add(1, Ordering::Relaxed),
                        Err(_) => error_clone.fetch_add(1, Ordering::Relaxed),
                    };
                    
                    let _ = raft_clone.is_leader().await;
                    success_clone.fetch_add(1, Ordering::Relaxed);
                    
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }
            });
        }
        
        // Wait for all tasks to complete
        while let Some(_) = tasks.join_next().await {}
        
        let successes = success_counter.load(Ordering::Relaxed);
        let errors = error_counter.load(Ordering::Relaxed);
        
        println!("    Stress test: {} successes, {} errors", successes, errors);
        
        // Should handle sustained load well
        assert!(successes > errors);
        
        Ok(())
    }
    
    /// Stress test with rapid queries
    async fn test_stress_rapid_queries() -> Result<()> {
        let config = create_test_raft_config(
            "test_stress_rapid",
            "/tmp/raft_test_stress_rapid",
            vec!["peer1".to_string(), "peer2".to_string()]
        );
        
        let raft = RaftConsensus::new(config).await?;
        
        let start_time = Instant::now();
        let iterations = 2000;
        
        // Rapid fire queries
        for _i in 0..iterations {
            let _ = raft.get_cluster_config().await;
            let _ = raft.is_leader().await;
            let _ = raft.has_pending_config_change().await;
        }
        
        let total_duration = start_time.elapsed();
        let avg_duration = total_duration / iterations;
        
        println!("    Rapid queries: {} operations in {:?} (avg: {:?})", 
                iterations, total_duration, avg_duration);
        
        Ok(())
    }
    
    /// Analyze test results and generate recommendations
    async fn analyze_results(&mut self) -> Result<()> {
        println!("\n📊 Test Results Analysis");
        println!("========================");
        
        let total_tests = self.test_results.len();
        let passed_tests = self.test_results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;
        
        println!("Total tests: {}", total_tests);
        println!("Passed: {} ({}%)", passed_tests, (passed_tests * 100) / total_tests);
        println!("Failed: {} ({}%)", failed_tests, (failed_tests * 100) / total_tests);
        
        if failed_tests > 0 {
            println!("\n❌ Failed Tests:");
            for result in &self.test_results {
                if !result.passed {
                    println!("  - {}: {}", result.test_name, 
                            result.error_message.as_ref().unwrap_or(&"Unknown error".to_string()));
                }
            }
        }
        
        // Calculate average test duration
        let total_duration: Duration = self.test_results.iter().map(|r| r.duration).sum();
        let avg_duration = total_duration / total_tests as u32;
        
        println!("\n⏱️  Performance Summary:");
        println!("Total test time: {:?}", total_duration);
        println!("Average test time: {:?}", avg_duration);
        
        // Find slowest tests
        let mut sorted_results = self.test_results.clone();
        sorted_results.sort_by(|a, b| b.duration.cmp(&a.duration));
        
        println!("\n🐌 Slowest Tests:");
        for (i, result) in sorted_results.iter().take(3).enumerate() {
            println!("  {}. {} - {:?}", i + 1, result.test_name, result.duration);
        }
        
        Ok(())
    }
    
    /// Generate a comprehensive performance report
    pub async fn generate_performance_report(&self) -> Result<String> {
        let mut report = String::new();
        
        report.push_str("# Raft Implementation Test and Performance Report\n\n");
        report.push_str(&format!("Generated at: {}\n\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        
        // Test results summary
        let total_tests = self.test_results.len();
        let passed_tests = self.test_results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;
        
        report.push_str("## Test Results Summary\n\n");
        report.push_str(&format!("- Total tests: {}\n", total_tests));
        report.push_str(&format!("- Passed: {} ({:.1}%)\n", passed_tests, (passed_tests as f64 * 100.0) / total_tests as f64));
        report.push_str(&format!("- Failed: {} ({:.1}%)\n\n", failed_tests, (failed_tests as f64 * 100.0) / total_tests as f64));
        
        // Performance metrics
        if let Some(metrics) = &self.performance_metrics {
            report.push_str("## Performance Metrics\n\n");
            report.push_str(&format!("- Safety checks: {} (avg: {}μs)\n", metrics.safety_checks, metrics.avg_safety_check_time));
            report.push_str(&format!("- Configuration queries: {} (avg: {}μs)\n", metrics.configuration_queries, metrics.avg_config_query_time));
            report.push_str(&format!("- Read operations: {} (avg: {}μs)\n", metrics.read_operations, metrics.avg_read_operation_time));
            report.push_str(&format!("- Leadership queries: {}\n", metrics.leadership_queries));
            
            if metrics.config_cache_hits + metrics.config_cache_misses > 0 {
                let hit_rate = (metrics.config_cache_hits as f64 / (metrics.config_cache_hits + metrics.config_cache_misses) as f64) * 100.0;
                report.push_str(&format!("- Config cache hit rate: {:.1}%\n", hit_rate));
            }
            
            report.push_str("\n");
        }
        
        // Tuning recommendations
        if let Some(recommendations) = &self.tuning_recommendations {
            if !recommendations.critical_issues.is_empty() {
                report.push_str("## Critical Issues\n\n");
                for issue in &recommendations.critical_issues {
                    report.push_str(&format!("- ⚠️  {}\n", issue));
                }
                report.push_str("\n");
            }
            
            if !recommendations.recommendations.is_empty() {
                report.push_str("## Recommendations\n\n");
                for rec in &recommendations.recommendations {
                    report.push_str(&format!("- 💡 {}\n", rec));
                }
                report.push_str("\n");
            }
            
            if !recommendations.optimization_opportunities.is_empty() {
                report.push_str("## Optimization Opportunities\n\n");
                for opt in &recommendations.optimization_opportunities {
                    report.push_str(&format!("- 🚀 {}\n", opt));
                }
                report.push_str("\n");
            }
        }
        
        // Detailed test results
        report.push_str("## Detailed Test Results\n\n");
        for result in &self.test_results {
            let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("- {} **{}** ({:?})", status, result.test_name, result.duration));
            if let Some(error) = &result.error_message {
                report.push_str(&format!(" - Error: {}", error));
            }
            report.push_str("\n");
        }
        
        Ok(report)
    }
    
    /// Save performance report to file
    pub async fn save_report_to_file(&self, file_path: &str) -> Result<()> {
        let report = self.generate_performance_report().await?;
        tokio::fs::write(file_path, report).await?;
        Ok(())
    }
}

/// Main function to run all tests and optimizations
pub async fn run_raft_testing_and_optimization() -> Result<()> {
    println!("🔬 Starting Raft Testing and Optimization Suite");
    println!("================================================");
    
    let mut test_runner = RaftTestRunner::new();
    test_runner.run_comprehensive_test_suite().await?;
    
    // Generate and save report
    let report_path = "/tmp/raft_performance_report.md";
    test_runner.save_report_to_file(report_path).await?;
    println!("\n📄 Performance report saved to: {}", report_path);
    
    println!("\n🎉 Raft testing and optimization completed successfully!");
    
    Ok(())
}