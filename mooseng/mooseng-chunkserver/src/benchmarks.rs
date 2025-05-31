use crate::{
    chunk::{Chunk, ChecksumType},
    storage::{ChunkStorage, FileStorage},
    config::ChunkServerConfig,
    integrity::{IntegrityVerifier, IntegrityConfig},
    zero_copy::ZeroCopyTransfer,
    mmap::MmapManager,
};
use mooseng_common::types::{ChunkId, ChunkVersion};
use bytes::Bytes;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
// tempfile only available in tests
use tracing::{info, warn, debug};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for storage benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Number of chunks to use in each benchmark
    pub chunk_count: usize,
    /// Size of each chunk in bytes
    pub chunk_size: usize,
    /// Number of iterations for each benchmark
    pub iterations: usize,
    /// Types of checksums to benchmark
    pub checksum_types: Vec<ChecksumType>,
    /// Whether to include integrity verification benchmarks
    pub include_integrity_tests: bool,
    /// Whether to include zero-copy benchmarks
    pub include_zero_copy_tests: bool,
    /// Concurrent operations to test
    pub concurrency_levels: Vec<usize>,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            chunk_count: 100,
            chunk_size: 1024 * 1024, // 1MB
            iterations: 5,
            checksum_types: vec![
                ChecksumType::Crc32,
                ChecksumType::Blake3,
                ChecksumType::XxHash3,
                ChecksumType::HybridFast,
                ChecksumType::HybridSecure,
            ],
            include_integrity_tests: true,
            include_zero_copy_tests: true,
            concurrency_levels: vec![1, 4, 8, 16],
        }
    }
}

/// Results of a single benchmark operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub operation: String,
    pub checksum_type: String,
    pub chunk_size: usize,
    pub chunk_count: usize,
    pub concurrency: usize,
    pub duration_micros: u64,
    pub throughput_mb_per_sec: f64,
    pub operations_per_sec: f64,
    pub avg_latency_micros: u64,
    pub min_latency_micros: u64,
    pub max_latency_micros: u64,
    pub percentile_95_micros: u64,
    pub success_rate: f64,
}

/// Summary of all benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub config: BenchmarkConfig,
    pub results: Vec<BenchmarkResult>,
    pub total_duration_micros: u64,
    pub system_info: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_count: usize,
    pub memory_gb: u64,
    pub disk_type: String,
    pub rust_version: String,
}

/// Comprehensive storage benchmarking suite
pub struct StorageBenchmarker {
    config: BenchmarkConfig,
    temp_dir: TempDir,
    storage: Arc<dyn ChunkStorage>,
    chunk_server_config: Arc<ChunkServerConfig>,
}

impl StorageBenchmarker {
    /// Create a new benchmarker with the given configuration
    pub fn new(config: BenchmarkConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let mut chunk_server_config = ChunkServerConfig::default();
        chunk_server_config.data_dir = temp_dir.path().to_path_buf();
        
        let storage = Arc::new(FileStorage::new_with_limits(
            Arc::new(chunk_server_config.clone()),
            1000,  // Large cache for benchmarks
            64,    // High concurrency
            64,    // Large batch size
        )) as Arc<dyn ChunkStorage>;

        Ok(Self {
            config,
            temp_dir,
            storage,
            chunk_server_config: Arc::new(chunk_server_config),
        })
    }

    /// Run all benchmarks and return results
    pub async fn run_all_benchmarks(&self) -> Result<BenchmarkSummary, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        info!("Starting comprehensive storage benchmarks");

        let mut all_results = Vec::new();

        // Basic storage operation benchmarks
        all_results.extend(self.benchmark_basic_operations().await?);

        // Checksum algorithm benchmarks
        all_results.extend(self.benchmark_checksum_algorithms().await?);

        // Batch operation benchmarks
        all_results.extend(self.benchmark_batch_operations().await?);

        // Concurrency benchmarks
        all_results.extend(self.benchmark_concurrency().await?);

        // Integrity verification benchmarks
        if self.config.include_integrity_tests {
            all_results.extend(self.benchmark_integrity_verification().await?);
        }

        // Zero-copy benchmarks
        if self.config.include_zero_copy_tests {
            all_results.extend(self.benchmark_zero_copy_operations().await?);
        }

        let total_duration = start_time.elapsed();
        info!("All benchmarks completed in {:?}", total_duration);

        Ok(BenchmarkSummary {
            config: self.config.clone(),
            results: all_results,
            total_duration_micros: total_duration.as_micros() as u64,
            system_info: self.get_system_info(),
        })
    }

    /// Benchmark basic storage operations (store, retrieve, delete)
    async fn benchmark_basic_operations(&self) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        info!("Running basic storage operation benchmarks");
        let mut results = Vec::new();

        for &checksum_type in &self.config.checksum_types {
            // Generate test chunks
            let chunks = self.generate_test_chunks(self.config.chunk_count, checksum_type).await;

            // Benchmark store operations
            let store_result = self.benchmark_store_operations(&chunks, checksum_type).await?;
            results.push(store_result);

            // Benchmark retrieve operations
            let retrieve_result = self.benchmark_retrieve_operations(&chunks, checksum_type).await?;
            results.push(retrieve_result);

            // Benchmark delete operations
            let delete_result = self.benchmark_delete_operations(&chunks, checksum_type).await?;
            results.push(delete_result);
        }

        Ok(results)
    }

    /// Benchmark different checksum algorithms
    async fn benchmark_checksum_algorithms(&self) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        info!("Running checksum algorithm benchmarks");
        let mut results = Vec::new();

        for &checksum_type in &self.config.checksum_types {
            let result = self.benchmark_checksum_computation(checksum_type).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Benchmark batch operations
    async fn benchmark_batch_operations(&self) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        info!("Running batch operation benchmarks");
        let mut results = Vec::new();

        for &checksum_type in &self.config.checksum_types {
            let chunks = self.generate_test_chunks(self.config.chunk_count, checksum_type).await;

            // Benchmark batch store
            let batch_store_result = self.benchmark_batch_store(&chunks, checksum_type).await?;
            results.push(batch_store_result);

            // Benchmark batch retrieve
            let batch_retrieve_result = self.benchmark_batch_retrieve(&chunks, checksum_type).await?;
            results.push(batch_retrieve_result);
        }

        Ok(results)
    }

    /// Benchmark concurrent operations
    async fn benchmark_concurrency(&self) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        info!("Running concurrency benchmarks");
        let mut results = Vec::new();

        for &concurrency in &self.config.concurrency_levels {
            for &checksum_type in &self.config.checksum_types {
                let result = self.benchmark_concurrent_operations(concurrency, checksum_type).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Benchmark integrity verification
    async fn benchmark_integrity_verification(&self) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        info!("Running integrity verification benchmarks");
        let mut results = Vec::new();

        let verifier = IntegrityVerifier::new(self.storage.clone(), IntegrityConfig::default());

        for &checksum_type in &self.config.checksum_types {
            let chunks = self.generate_test_chunks(self.config.chunk_count, checksum_type).await;
            
            // Store chunks first
            for chunk in &chunks {
                self.storage.store_chunk(chunk).await?;
            }

            let result = self.benchmark_integrity_scan(&verifier, checksum_type).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Benchmark zero-copy operations
    async fn benchmark_zero_copy_operations(&self) -> Result<Vec<BenchmarkResult>, Box<dyn std::error::Error>> {
        info!("Running zero-copy operation benchmarks");
        let mut results = Vec::new();

        let mmap_manager = Arc::new(MmapManager::new(Default::default()));
        let zero_copy = ZeroCopyTransfer::new(mmap_manager);

        for &checksum_type in &self.config.checksum_types {
            let result = self.benchmark_zero_copy_reads(&zero_copy, checksum_type).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Benchmark store operations
    async fn benchmark_store_operations(&self, chunks: &[Chunk], checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        for iteration in 0..self.config.iterations {
            for (i, chunk) in chunks.iter().enumerate() {
                let chunk_start = Instant::now();
                
                match self.storage.store_chunk(chunk).await {
                    Ok(_) => {
                        success_count += 1;
                        latencies.push(chunk_start.elapsed().as_micros() as u64);
                    }
                    Err(e) => {
                        warn!("Store operation failed: {}", e);
                    }
                }

                // Clean up for next iteration
                if iteration < self.config.iterations - 1 {
                    let _ = self.storage.delete_chunk(chunk.id(), chunk.version()).await;
                }
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "store".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            chunks.len() * self.config.iterations,
        )
    }

    /// Benchmark retrieve operations
    async fn benchmark_retrieve_operations(&self, chunks: &[Chunk], checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        // First store all chunks
        for chunk in chunks {
            self.storage.store_chunk(chunk).await?;
        }

        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        for _iteration in 0..self.config.iterations {
            for chunk in chunks {
                let chunk_start = Instant::now();
                
                match self.storage.get_chunk(chunk.id(), chunk.version()).await {
                    Ok(_) => {
                        success_count += 1;
                        latencies.push(chunk_start.elapsed().as_micros() as u64);
                    }
                    Err(e) => {
                        warn!("Retrieve operation failed: {}", e);
                    }
                }
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "retrieve".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            chunks.len() * self.config.iterations,
        )
    }

    /// Benchmark delete operations
    async fn benchmark_delete_operations(&self, chunks: &[Chunk], checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        for iteration in 0..self.config.iterations {
            // Store chunks for deletion
            for chunk in chunks {
                self.storage.store_chunk(chunk).await?;
            }

            for chunk in chunks {
                let chunk_start = Instant::now();
                
                match self.storage.delete_chunk(chunk.id(), chunk.version()).await {
                    Ok(_) => {
                        success_count += 1;
                        latencies.push(chunk_start.elapsed().as_micros() as u64);
                    }
                    Err(e) => {
                        warn!("Delete operation failed: {}", e);
                    }
                }
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "delete".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            chunks.len() * self.config.iterations,
        )
    }

    /// Benchmark checksum computation
    async fn benchmark_checksum_computation(&self, checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let test_data = self.generate_random_data(self.config.chunk_size);
        let mut latencies = Vec::new();

        let start_time = Instant::now();

        for _iteration in 0..self.config.iterations {
            for _i in 0..self.config.chunk_count {
                let chunk_start = Instant::now();
                
                let _chunk = Chunk::new(
                    1000 + _i as u64,
                    1,
                    test_data.clone(),
                    checksum_type,
                    1,
                );
                
                latencies.push(chunk_start.elapsed().as_micros() as u64);
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "checksum_computation".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            latencies.len(),
            latencies.len(),
        )
    }

    /// Benchmark batch store operations
    async fn benchmark_batch_store(&self, chunks: &[Chunk], checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        for _iteration in 0..self.config.iterations {
            let chunk_refs: Vec<&Chunk> = chunks.iter().collect();
            let batch_start = Instant::now();
            
            match self.storage.store_chunks_batch(&chunk_refs).await {
                Ok(results) => {
                    success_count += results.iter().filter(|r| r.is_ok()).count();
                    latencies.push(batch_start.elapsed().as_micros() as u64);
                }
                Err(e) => {
                    warn!("Batch store operation failed: {}", e);
                }
            }

            // Clean up for next iteration
            for chunk in chunks {
                let _ = self.storage.delete_chunk(chunk.id(), chunk.version()).await;
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "batch_store".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            chunks.len() * self.config.iterations,
        )
    }

    /// Benchmark batch retrieve operations
    async fn benchmark_batch_retrieve(&self, chunks: &[Chunk], checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        // Store chunks first
        for chunk in chunks {
            self.storage.store_chunk(chunk).await?;
        }

        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        for _iteration in 0..self.config.iterations {
            let chunk_ids: Vec<(ChunkId, ChunkVersion)> = chunks.iter()
                .map(|c| (c.id(), c.version()))
                .collect();
            
            let batch_start = Instant::now();
            
            match self.storage.get_chunks_batch(&chunk_ids).await {
                Ok(results) => {
                    success_count += results.iter().filter(|r| r.is_ok()).count();
                    latencies.push(batch_start.elapsed().as_micros() as u64);
                }
                Err(e) => {
                    warn!("Batch retrieve operation failed: {}", e);
                }
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "batch_retrieve".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            chunks.len() * self.config.iterations,
        )
    }

    /// Benchmark concurrent operations
    async fn benchmark_concurrent_operations(&self, concurrency: usize, checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let chunks = self.generate_test_chunks(self.config.chunk_count, checksum_type).await;
        let chunks_per_worker = chunks.len() / concurrency;
        
        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        let mut tasks = Vec::new();
        for worker_id in 0..concurrency {
            let worker_chunks = chunks.iter()
                .skip(worker_id * chunks_per_worker)
                .take(chunks_per_worker)
                .cloned()
                .collect::<Vec<_>>();
            
            let storage = self.storage.clone();
            let task = tokio::spawn(async move {
                let mut worker_latencies = Vec::new();
                let mut worker_success = 0;

                for chunk in worker_chunks {
                    let op_start = Instant::now();
                    
                    if storage.store_chunk(&chunk).await.is_ok() {
                        worker_success += 1;
                        worker_latencies.push(op_start.elapsed().as_micros() as u64);
                    }
                }

                (worker_latencies, worker_success)
            });
            
            tasks.push(task);
        }

        for task in tasks {
            let (worker_latencies, worker_success) = task.await?;
            latencies.extend(worker_latencies);
            success_count += worker_success;
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            format!("concurrent_store_{}", concurrency),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            chunks.len(),
        )
    }

    /// Benchmark integrity scanning
    async fn benchmark_integrity_scan(&self, verifier: &IntegrityVerifier, checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        for _iteration in 0..self.config.iterations {
            let scan_start = Instant::now();
            
            match verifier.run_full_scan().await {
                Ok(summary) => {
                    success_count += 1;
                    latencies.push(scan_start.elapsed().as_micros() as u64);
                    debug!("Integrity scan: {} chunks verified", summary.verified_chunks);
                }
                Err(e) => {
                    warn!("Integrity scan failed: {}", e);
                }
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "integrity_scan".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            self.config.iterations,
        )
    }

    /// Benchmark zero-copy read operations
    async fn benchmark_zero_copy_reads(&self, zero_copy: &ZeroCopyTransfer, checksum_type: ChecksumType) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        // Create test files
        let mut test_files = Vec::new();
        for i in 0..self.config.chunk_count {
            let file_path = self.temp_dir.path().join(format!("test_file_{}.dat", i));
            let test_data = self.generate_random_data(self.config.chunk_size);
            fs::write(&file_path, &test_data).await?;
            test_files.push(file_path);
        }

        let mut latencies = Vec::new();
        let mut success_count = 0;

        let start_time = Instant::now();

        for _iteration in 0..self.config.iterations {
            for file_path in &test_files {
                let read_start = Instant::now();
                
                match zero_copy.read_chunk(file_path, 0, self.config.chunk_size as u64).await {
                    Ok(_) => {
                        success_count += 1;
                        latencies.push(read_start.elapsed().as_micros() as u64);
                    }
                    Err(e) => {
                        warn!("Zero-copy read failed: {}", e);
                    }
                }
            }
        }

        let total_duration = start_time.elapsed();
        self.create_benchmark_result(
            "zero_copy_read".to_string(),
            checksum_type,
            total_duration,
            &latencies,
            success_count,
            test_files.len() * self.config.iterations,
        )
    }

    /// Generate test chunks with specified parameters
    async fn generate_test_chunks(&self, count: usize, checksum_type: ChecksumType) -> Vec<Chunk> {
        let mut chunks = Vec::with_capacity(count);
        
        for i in 0..count {
            let data = self.generate_random_data(self.config.chunk_size);
            let chunk = Chunk::new(
                i as u64,
                1,
                data,
                checksum_type,
                1,
            );
            chunks.push(chunk);
        }
        
        chunks
    }

    /// Generate random test data
    fn generate_random_data(&self, size: usize) -> Bytes {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let data: Vec<u8> = (0..size).map(|_| rng.gen()).collect();
        Bytes::from(data)
    }

    /// Create a benchmark result from collected data
    fn create_benchmark_result(
        &self,
        operation: String,
        checksum_type: ChecksumType,
        total_duration: Duration,
        latencies: &[u64],
        success_count: usize,
        total_operations: usize,
    ) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
        let total_bytes = success_count * self.config.chunk_size;
        let throughput_mb_per_sec = if total_duration.as_secs_f64() > 0.0 {
            (total_bytes as f64 / (1024.0 * 1024.0)) / total_duration.as_secs_f64()
        } else {
            0.0
        };

        let operations_per_sec = if total_duration.as_secs_f64() > 0.0 {
            success_count as f64 / total_duration.as_secs_f64()
        } else {
            0.0
        };

        let success_rate = if total_operations > 0 {
            (success_count as f64 / total_operations as f64) * 100.0
        } else {
            0.0
        };

        let (avg_latency, min_latency, max_latency, percentile_95) = if !latencies.is_empty() {
            let mut sorted_latencies = latencies.to_vec();
            sorted_latencies.sort_unstable();
            
            let avg = sorted_latencies.iter().sum::<u64>() / sorted_latencies.len() as u64;
            let min = *sorted_latencies.first().unwrap();
            let max = *sorted_latencies.last().unwrap();
            let p95_index = (sorted_latencies.len() as f64 * 0.95) as usize;
            let p95 = sorted_latencies.get(p95_index).copied().unwrap_or(max);
            
            (avg, min, max, p95)
        } else {
            (0, 0, 0, 0)
        };

        Ok(BenchmarkResult {
            operation,
            checksum_type: format!("{:?}", checksum_type),
            chunk_size: self.config.chunk_size,
            chunk_count: self.config.chunk_count,
            concurrency: 1, // Default, will be updated for concurrent benchmarks
            duration_micros: total_duration.as_micros() as u64,
            throughput_mb_per_sec,
            operations_per_sec,
            avg_latency_micros: avg_latency,
            min_latency_micros: min_latency,
            max_latency_micros: max_latency,
            percentile_95_micros: percentile_95,
            success_rate,
        })
    }

    /// Get system information
    fn get_system_info(&self) -> SystemInfo {
        SystemInfo {
            cpu_count: num_cpus::get(),
            memory_gb: 8, // Simplified - would need more complex detection
            disk_type: "SSD".to_string(), // Simplified
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
        }
    }
}

/// Utility functions for benchmark result analysis
impl BenchmarkSummary {
    /// Print a formatted report of all benchmark results
    pub fn print_report(&self) {
        println!("\n🚀 Storage Performance Benchmark Report");
        println!("========================================");
        println!("System Info: {} CPUs, {}GB RAM, {} disk", 
                 self.system_info.cpu_count, 
                 self.system_info.memory_gb, 
                 self.system_info.disk_type);
        println!("Configuration: {} chunks × {} bytes, {} iterations",
                 self.config.chunk_count,
                 self.config.chunk_size,
                 self.config.iterations);
        println!("Total benchmark time: {:?}\n", self.total_duration);

        // Group results by operation type
        let mut operations: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();
        for result in &self.results {
            operations.entry(result.operation.clone()).or_default().push(result);
        }

        for (operation, results) in operations {
            println!("📊 {} Operations:", operation.to_uppercase());
            println!("┌─────────────────┬──────────────┬──────────────┬─────────────┬──────────────┐");
            println!("│ Checksum        │ Throughput   │ Ops/sec      │ Avg Latency │ Success Rate │");
            println!("├─────────────────┼──────────────┼──────────────┼─────────────┼──────────────┤");
            
            for result in results {
                println!("│ {:15} │ {:10.2} MB/s │ {:10.2} ops │ {:9} μs │ {:10.1}% │",
                         result.checksum_type,
                         result.throughput_mb_per_sec,
                         result.operations_per_sec,
                         result.avg_latency_micros,
                         result.success_rate);
            }
            println!("└─────────────────┴──────────────┴──────────────┴─────────────┴──────────────┘\n");
        }
    }

    /// Export results to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get the best performing checksum algorithm for each operation
    pub fn get_best_performers(&self) -> HashMap<String, String> {
        let mut best_performers = HashMap::new();
        
        let mut operations: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();
        for result in &self.results {
            operations.entry(result.operation.clone()).or_default().push(result);
        }

        for (operation, results) in operations {
            if let Some(best) = results.iter()
                .max_by(|a, b| a.throughput_mb_per_sec.partial_cmp(&b.throughput_mb_per_sec).unwrap()) {
                best_performers.insert(operation, best.checksum_type.clone());
            }
        }

        best_performers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_benchmark_creation() {
        let config = BenchmarkConfig {
            chunk_count: 10,
            chunk_size: 1024,
            iterations: 2,
            checksum_types: vec![ChecksumType::Blake3],
            include_integrity_tests: false,
            include_zero_copy_tests: false,
            concurrency_levels: vec![1],
        };

        let benchmarker = StorageBenchmarker::new(config).unwrap();
        assert_eq!(benchmarker.config.chunk_count, 10);
    }

    #[tokio::test]
    async fn test_chunk_generation() {
        let config = BenchmarkConfig::default();
        let benchmarker = StorageBenchmarker::new(config).unwrap();
        
        let chunks = benchmarker.generate_test_chunks(5, ChecksumType::Blake3).await;
        assert_eq!(chunks.len(), 5);
        
        for chunk in chunks {
            assert!(chunk.verify_integrity());
        }
    }
}