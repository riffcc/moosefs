//! Unified Benchmark Runner
//!
//! This module provides the core runtime for executing benchmarks in a unified manner.
//! It coordinates benchmark execution, progress reporting, and result collection.

use super::{
    UnifiedBenchmark, UnifiedBenchmarkConfig, UnifiedBenchmarkResult, BenchmarkProgress,
    BenchmarkSession, SessionStatus, BenchmarkCategory, ProgressSender,
    calculate_statistics, BenchmarkStatistics,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Main benchmark runner that coordinates execution
pub struct BenchmarkRunner {
    /// Registered benchmarks
    benchmarks: HashMap<String, Arc<dyn UnifiedBenchmark>>,
    /// Benchmarks organized by category
    categories: HashMap<BenchmarkCategory, Vec<String>>,
    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, BenchmarkSession>>>,
    /// Default configuration
    default_config: UnifiedBenchmarkConfig,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    pub fn new() -> Self {
        Self {
            benchmarks: HashMap::new(),
            categories: HashMap::new(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            default_config: UnifiedBenchmarkConfig::default(),
        }
    }
    
    /// Register a benchmark
    pub fn register_benchmark(&mut self, benchmark: Arc<dyn UnifiedBenchmark>) {
        let name = benchmark.name().to_string();
        let category = benchmark.category();
        
        // Add to benchmarks map
        self.benchmarks.insert(name.clone(), benchmark);
        
        // Add to category map
        self.categories
            .entry(category)
            .or_insert_with(Vec::new)
            .push(name);
        
        info!("Registered benchmark: {} (category: {})", name, category);
    }
    
    /// Register multiple benchmarks
    pub fn register_benchmarks(&mut self, benchmarks: Vec<Arc<dyn UnifiedBenchmark>>) {
        for benchmark in benchmarks {
            self.register_benchmark(benchmark);
        }
    }
    
    /// Get list of available benchmark names
    pub fn list_benchmarks(&self) -> Vec<String> {
        self.benchmarks.keys().cloned().collect()
    }
    
    /// Get benchmarks by category
    pub fn list_benchmarks_by_category(&self, category: BenchmarkCategory) -> Vec<String> {
        self.categories
            .get(&category)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Get all available categories
    pub fn list_categories(&self) -> Vec<BenchmarkCategory> {
        self.categories.keys().copied().collect()
    }
    
    /// Get benchmark information
    pub fn get_benchmark_info(&self, name: &str) -> Option<(String, BenchmarkCategory)> {
        self.benchmarks.get(name).map(|b| {
            (b.description().to_string(), b.category())
        })
    }
    
    /// Create a new benchmark session
    pub async fn create_session(
        &self,
        name: String,
        description: Option<String>,
        config: Option<UnifiedBenchmarkConfig>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let session = BenchmarkSession {
            id: id.clone(),
            name,
            description,
            config: config.unwrap_or_else(|| self.default_config.clone()),
            start_time: chrono::Utc::now(),
            end_time: None,
            status: SessionStatus::Created,
            results: Vec::new(),
        };
        
        let mut sessions = self.sessions.write().await;
        sessions.insert(id.clone(), session);
        
        info!("Created benchmark session: {}", id);
        Ok(id)
    }
    
    /// Get session information
    pub async fn get_session(&self, session_id: &str) -> Option<BenchmarkSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }
    
    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<BenchmarkSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }
    
    /// Run all benchmarks
    pub async fn run_all(
        &self,
        config: Option<UnifiedBenchmarkConfig>,
        progress_sender: Option<ProgressSender>,
    ) -> Result<String> {
        let session_id = self.create_session(
            "Full Benchmark Suite".to_string(),
            Some("Running all available benchmarks".to_string()),
            config,
        ).await?;
        
        let benchmark_names: Vec<String> = self.benchmarks.keys().cloned().collect();
        self.run_benchmarks_internal(session_id.clone(), benchmark_names, progress_sender).await?;
        
        Ok(session_id)
    }
    
    /// Run benchmarks by category
    pub async fn run_category(
        &self,
        category: BenchmarkCategory,
        config: Option<UnifiedBenchmarkConfig>,
        progress_sender: Option<ProgressSender>,
    ) -> Result<String> {
        let session_id = self.create_session(
            format!("{} Benchmarks", category),
            Some(format!("Running benchmarks in {} category", category)),
            config,
        ).await?;
        
        let benchmark_names = self.list_benchmarks_by_category(category);
        if benchmark_names.is_empty() {
            return Err(anyhow::anyhow!("No benchmarks found for category: {}", category));
        }
        
        self.run_benchmarks_internal(session_id.clone(), benchmark_names, progress_sender).await?;
        
        Ok(session_id)
    }
    
    /// Run specific benchmarks by name
    pub async fn run_benchmarks(
        &self,
        benchmark_names: Vec<String>,
        config: Option<UnifiedBenchmarkConfig>,
        progress_sender: Option<ProgressSender>,
    ) -> Result<String> {
        // Validate benchmark names
        for name in &benchmark_names {
            if !self.benchmarks.contains_key(name) {
                return Err(anyhow::anyhow!("Benchmark not found: {}", name));
            }
        }
        
        let session_id = self.create_session(
            format!("Selected Benchmarks: {}", benchmark_names.join(", ")),
            Some("Running selected benchmarks".to_string()),
            config,
        ).await?;
        
        self.run_benchmarks_internal(session_id.clone(), benchmark_names, progress_sender).await?;
        
        Ok(session_id)
    }
    
    /// Internal method to run benchmarks
    async fn run_benchmarks_internal(
        &self,
        session_id: String,
        benchmark_names: Vec<String>,
        progress_sender: Option<ProgressSender>,
    ) -> Result<()> {
        // Update session status to running
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.status = SessionStatus::Running;
            }
        }
        
        let session_config = {
            let sessions = self.sessions.read().await;
            sessions.get(&session_id)
                .map(|s| s.config.clone())
                .unwrap_or_else(|| self.default_config.clone())
        };
        
        let total_benchmarks = benchmark_names.len();
        let mut all_results = Vec::new();
        let start_time = Instant::now();
        
        info!("Starting {} benchmarks in session {}", total_benchmarks, session_id);
        
        for (i, benchmark_name) in benchmark_names.iter().enumerate() {
            let benchmark = match self.benchmarks.get(benchmark_name) {
                Some(b) => b.clone(),
                None => {
                    warn!("Benchmark {} not found, skipping", benchmark_name);
                    continue;
                }
            };
            
            info!("Running benchmark {} ({}/{})", benchmark_name, i + 1, total_benchmarks);
            
            // Send progress update
            if let Some(ref sender) = progress_sender {
                let progress = BenchmarkProgress {
                    benchmark_name: benchmark_name.clone(),
                    current_step: format!("Starting {} benchmark", benchmark_name),
                    progress: i as f64 / total_benchmarks as f64,
                    estimated_remaining: self.estimate_remaining_time(
                        &benchmark_names[i..],
                        &session_config,
                        start_time.elapsed(),
                        i,
                    ),
                    current_result: None,
                };
                let _ = sender.send(progress);
            }
            
            // Validate environment
            if let Err(e) = benchmark.validate_environment().await {
                error!("Environment validation failed for {}: {}", benchmark_name, e);
                continue;
            }
            
            // Run the benchmark
            match benchmark.run(&session_config).await {
                Ok(mut results) => {
                    info!("Completed {} with {} results", benchmark_name, results.len());
                    
                    // Send progress with results
                    if let Some(ref sender) = progress_sender {
                        if let Some(result) = results.first() {
                            let progress = BenchmarkProgress {
                                benchmark_name: benchmark_name.clone(),
                                current_step: format!("Completed {} benchmark", benchmark_name),
                                progress: (i + 1) as f64 / total_benchmarks as f64,
                                estimated_remaining: self.estimate_remaining_time(
                                    &benchmark_names[i + 1..],
                                    &session_config,
                                    start_time.elapsed(),
                                    i + 1,
                                ),
                                current_result: Some(result.clone()),
                            };
                            let _ = sender.send(progress);
                        }
                    }
                    
                    all_results.append(&mut results);
                }
                Err(e) => {
                    error!("Benchmark {} failed: {}", benchmark_name, e);
                    
                    // Update session status to failed if configured to stop on errors
                    if !session_config.detailed_report {
                        let mut sessions = self.sessions.write().await;
                        if let Some(session) = sessions.get_mut(&session_id) {
                            session.status = SessionStatus::Failed;
                            session.end_time = Some(chrono::Utc::now());
                        }
                        return Err(e).context(format!("Benchmark {} failed", benchmark_name));
                    }
                }
            }
        }
        
        // Update session with results and completion status
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.results = all_results;
                session.status = SessionStatus::Completed;
                session.end_time = Some(chrono::Utc::now());
            }
        }
        
        let total_time = start_time.elapsed();
        info!("Completed session {} in {:.2}s with {} results", 
              session_id, total_time.as_secs_f64(), all_results.len());
        
        Ok(())
    }
    
    /// Estimate remaining time for benchmark execution
    fn estimate_remaining_time(
        &self,
        remaining_benchmarks: &[String],
        config: &UnifiedBenchmarkConfig,
        elapsed: Duration,
        completed: usize,
    ) -> Option<Duration> {
        if remaining_benchmarks.is_empty() || completed == 0 {
            return None;
        }
        
        let avg_time_per_benchmark = elapsed.as_secs_f64() / completed as f64;
        let estimated_remaining = avg_time_per_benchmark * remaining_benchmarks.len() as f64;
        
        Some(Duration::from_secs_f64(estimated_remaining))
    }
    
    /// Calculate statistics for a session
    pub async fn calculate_session_statistics(&self, session_id: &str) -> Option<BenchmarkStatistics> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|session| {
            calculate_statistics(&session.results)
        })
    }
    
    /// Export session results in various formats
    pub async fn export_session_results(
        &self,
        session_id: &str,
        format: ExportFormat,
    ) -> Result<String> {
        let session = {
            let sessions = self.sessions.read().await;
            sessions.get(session_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?
        };
        
        match format {
            ExportFormat::Json => {
                serde_json::to_string_pretty(&session.results)
                    .context("Failed to serialize results to JSON")
            }
            ExportFormat::Csv => {
                self.export_to_csv(&session.results)
            }
            ExportFormat::Summary => {
                self.export_to_summary(&session)
            }
        }
    }
    
    /// Export results to CSV format
    fn export_to_csv(&self, results: &[UnifiedBenchmarkResult]) -> Result<String> {
        let mut csv = String::from("id,benchmark_name,operation,category,mean_time_ms,std_dev_ms,min_time_ms,max_time_ms,throughput,samples,timestamp\n");
        
        for result in results {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                result.id,
                result.benchmark_name,
                result.operation,
                result.category,
                result.mean_time.as_secs_f64() * 1000.0,
                result.std_dev.as_secs_f64() * 1000.0,
                result.min_time.as_secs_f64() * 1000.0,
                result.max_time.as_secs_f64() * 1000.0,
                result.throughput.unwrap_or(0.0),
                result.samples,
                result.timestamp.to_rfc3339(),
            ));
        }
        
        Ok(csv)
    }
    
    /// Export session to summary format
    fn export_to_summary(&self, session: &BenchmarkSession) -> Result<String> {
        let stats = calculate_statistics(&session.results);
        
        let mut summary = String::new();
        summary.push_str(&format!("# Benchmark Session Summary\n\n"));
        summary.push_str(&format!("**Session ID:** {}\n", session.id));
        summary.push_str(&format!("**Session Name:** {}\n", session.name));
        if let Some(ref desc) = session.description {
            summary.push_str(&format!("**Description:** {}\n", desc));
        }
        summary.push_str(&format!("**Status:** {}\n", session.status));
        summary.push_str(&format!("**Start Time:** {}\n", session.start_time.to_rfc3339()));
        if let Some(end_time) = session.end_time {
            summary.push_str(&format!("**End Time:** {}\n", end_time.to_rfc3339()));
            let duration = (end_time - session.start_time).num_seconds();
            summary.push_str(&format!("**Duration:** {}s\n", duration));
        }
        
        summary.push_str(&format!("\n## Performance Summary\n\n"));
        summary.push_str(&format!("- **Total Results:** {}\n", stats.total_results));
        summary.push_str(&format!("- **Average Latency:** {:.2}ms\n", stats.average_latency.as_secs_f64() * 1000.0));
        if let Some(throughput) = stats.average_throughput {
            summary.push_str(&format!("- **Average Throughput:** {:.2} ops/s\n", throughput));
        }
        if let Some(peak) = stats.peak_throughput {
            summary.push_str(&format!("- **Peak Throughput:** {:.2} ops/s\n", peak));
        }
        summary.push_str(&format!("- **Performance Grade:** {}\n", stats.performance_grade));
        
        summary.push_str(&format!("\n## Category Breakdown\n\n"));
        for (category, category_stats) in &stats.category_stats {
            summary.push_str(&format!("### {}\n", category));
            summary.push_str(&format!("- **Tests:** {}\n", category_stats.count));
            summary.push_str(&format!("- **Avg Latency:** {:.2}ms\n", category_stats.avg_latency.as_secs_f64() * 1000.0));
            if let Some(throughput) = category_stats.avg_throughput {
                summary.push_str(&format!("- **Avg Throughput:** {:.2} ops/s\n", throughput));
            }
            if let Some(ref fastest) = category_stats.fastest_operation {
                summary.push_str(&format!("- **Fastest:** {}\n", fastest));
            }
            if let Some(ref slowest) = category_stats.slowest_operation {
                summary.push_str(&format!("- **Slowest:** {}\n", slowest));
            }
            summary.push_str("\n");
        }
        
        Ok(summary)
    }
    
    /// Clean up old sessions
    pub async fn cleanup_old_sessions(&self, max_age: Duration) -> usize {
        let cutoff_time = chrono::Utc::now() - chrono::Duration::from_std(max_age).unwrap();
        let mut sessions = self.sessions.write().await;
        
        let old_sessions: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| {
                session.end_time
                    .map(|end| end < cutoff_time)
                    .unwrap_or(false)
            })
            .map(|(id, _)| id.clone())
            .collect();
        
        let count = old_sessions.len();
        for session_id in old_sessions {
            sessions.remove(&session_id);
        }
        
        if count > 0 {
            info!("Cleaned up {} old sessions", count);
        }
        
        count
    }
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Export format options
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
    Csv,
    Summary,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    
    struct TestBenchmark {
        name: String,
        category: BenchmarkCategory,
    }
    
    impl TestBenchmark {
        fn new(name: &str, category: BenchmarkCategory) -> Self {
            Self {
                name: name.to_string(),
                category,
            }
        }
    }
    
    #[async_trait]
    impl UnifiedBenchmark for TestBenchmark {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn description(&self) -> &str {
            "Test benchmark"
        }
        
        fn category(&self) -> BenchmarkCategory {
            self.category
        }
        
        async fn run(&self, _config: &UnifiedBenchmarkConfig) -> Result<Vec<UnifiedBenchmarkResult>> {
            Ok(vec![UnifiedBenchmarkResult {
                id: Uuid::new_v4().to_string(),
                benchmark_name: self.name.clone(),
                operation: "test_op".to_string(),
                category: self.category,
                mean_time: Duration::from_millis(10),
                std_dev: Duration::from_millis(1),
                min_time: Duration::from_millis(9),
                max_time: Duration::from_millis(11),
                throughput: Some(100.0),
                samples: 10,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                percentiles: None,
            }])
        }
    }
    
    #[tokio::test]
    async fn test_benchmark_registration() {
        let mut runner = BenchmarkRunner::new();
        let benchmark = Arc::new(TestBenchmark::new("test1", BenchmarkCategory::FileOperations));
        
        runner.register_benchmark(benchmark);
        
        let benchmarks = runner.list_benchmarks();
        assert!(benchmarks.contains(&"test1".to_string()));
        
        let file_benchmarks = runner.list_benchmarks_by_category(BenchmarkCategory::FileOperations);
        assert!(file_benchmarks.contains(&"test1".to_string()));
    }
    
    #[tokio::test]
    async fn test_session_creation() {
        let runner = BenchmarkRunner::new();
        
        let session_id = runner.create_session(
            "Test Session".to_string(),
            Some("Test description".to_string()),
            None,
        ).await.unwrap();
        
        let session = runner.get_session(&session_id).await.unwrap();
        assert_eq!(session.name, "Test Session");
        assert_eq!(session.status, SessionStatus::Created);
    }
    
    #[tokio::test]
    async fn test_benchmark_execution() {
        let mut runner = BenchmarkRunner::new();
        let benchmark = Arc::new(TestBenchmark::new("test1", BenchmarkCategory::FileOperations));
        runner.register_benchmark(benchmark);
        
        let session_id = runner.run_benchmarks(
            vec!["test1".to_string()],
            None,
            None,
        ).await.unwrap();
        
        let session = runner.get_session(&session_id).await.unwrap();
        assert_eq!(session.status, SessionStatus::Completed);
        assert!(!session.results.is_empty());
    }
}