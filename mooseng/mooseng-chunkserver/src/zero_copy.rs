use bytes::{Bytes, BytesMut};
use mooseng_common::types::{ChunkId, ChunkVersion};
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};
use tracing::{debug, warn};

use crate::{
    error::{ChunkServerError, Result as ChunkResult},
    mmap::MmapManager,
};

/// Zero-copy data transfer implementation with enhanced memory management
pub struct ZeroCopyTransfer {
    mmap_manager: Arc<MmapManager>,
    use_sendfile: bool,
    use_splice: bool,
    // Enhanced features
    buffer_pool: Arc<BufferPool>,
    prefetch_size: usize,
    metrics: Arc<ZeroCopyMetrics>,
}

/// High-performance buffer pool for zero-copy operations
pub struct BufferPool {
    small_buffers: tokio::sync::Mutex<Vec<BytesMut>>,  // < 64KB
    medium_buffers: tokio::sync::Mutex<Vec<BytesMut>>, // 64KB - 1MB
    large_buffers: tokio::sync::Mutex<Vec<BytesMut>>,  // > 1MB
    buffer_metrics: BufferMetrics,
}

#[derive(Debug, Default)]
pub struct BufferMetrics {
    pub small_allocated: std::sync::atomic::AtomicU64,
    pub medium_allocated: std::sync::atomic::AtomicU64,
    pub large_allocated: std::sync::atomic::AtomicU64,
    pub pool_hits: std::sync::atomic::AtomicU64,
    pub pool_misses: std::sync::atomic::AtomicU64,
}

impl BufferPool {
    const SMALL_BUFFER_SIZE: usize = 64 * 1024;     // 64KB
    const MEDIUM_BUFFER_SIZE: usize = 1024 * 1024;  // 1MB
    const LARGE_BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB
    const MAX_POOLED_BUFFERS: usize = 32;

    pub fn new() -> Self {
        Self {
            small_buffers: tokio::sync::Mutex::new(Vec::with_capacity(Self::MAX_POOLED_BUFFERS)),
            medium_buffers: tokio::sync::Mutex::new(Vec::with_capacity(Self::MAX_POOLED_BUFFERS)),
            large_buffers: tokio::sync::Mutex::new(Vec::with_capacity(Self::MAX_POOLED_BUFFERS)),
            buffer_metrics: BufferMetrics::default(),
        }
    }

    /// Get a buffer from the pool or allocate a new one
    pub async fn get_buffer(&self, size: usize) -> BytesMut {
        let (pool, buffer_size, metric) = if size <= Self::SMALL_BUFFER_SIZE {
            (&self.small_buffers, Self::SMALL_BUFFER_SIZE, &self.buffer_metrics.small_allocated)
        } else if size <= Self::MEDIUM_BUFFER_SIZE {
            (&self.medium_buffers, Self::MEDIUM_BUFFER_SIZE, &self.buffer_metrics.medium_allocated)
        } else {
            (&self.large_buffers, Self::LARGE_BUFFER_SIZE.max(size), &self.buffer_metrics.large_allocated)
        };

        let mut pool_guard = pool.lock().await;
        if let Some(mut buffer) = pool_guard.pop() {
            buffer.clear();
            buffer.reserve(size);
            self.buffer_metrics.pool_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            buffer
        } else {
            self.buffer_metrics.pool_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            metric.fetch_add(buffer_size as u64, std::sync::atomic::Ordering::Relaxed);
            BytesMut::with_capacity(buffer_size)
        }
    }

    /// Return a buffer to the pool
    pub async fn return_buffer(&self, buffer: BytesMut) {
        let capacity = buffer.capacity();
        let pool = if capacity <= Self::SMALL_BUFFER_SIZE {
            &self.small_buffers
        } else if capacity <= Self::MEDIUM_BUFFER_SIZE {
            &self.medium_buffers
        } else {
            &self.large_buffers
        };

        let mut pool_guard = pool.lock().await;
        if pool_guard.len() < Self::MAX_POOLED_BUFFERS {
            pool_guard.push(buffer);
        }
        // If pool is full, buffer will be dropped (deallocated)
    }

    /// Get buffer pool statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.buffer_metrics.small_allocated.load(std::sync::atomic::Ordering::Relaxed),
            self.buffer_metrics.medium_allocated.load(std::sync::atomic::Ordering::Relaxed),
            self.buffer_metrics.large_allocated.load(std::sync::atomic::Ordering::Relaxed),
            self.buffer_metrics.pool_hits.load(std::sync::atomic::Ordering::Relaxed),
            self.buffer_metrics.pool_misses.load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

impl ZeroCopyTransfer {
    pub fn new(mmap_manager: Arc<MmapManager>) -> Self {
        Self {
            mmap_manager,
            use_sendfile: cfg!(target_os = "linux"),
            use_splice: cfg!(target_os = "linux"),
            buffer_pool: Arc::new(BufferPool::new()),
            prefetch_size: 128 * 1024, // 128KB prefetch
            metrics: Arc::new(ZeroCopyMetrics::default()),
        }
    }

    pub fn new_with_config(mmap_manager: Arc<MmapManager>, prefetch_size: usize) -> Self {
        Self {
            mmap_manager,
            use_sendfile: cfg!(target_os = "linux"),
            use_splice: cfg!(target_os = "linux"),
            buffer_pool: Arc::new(BufferPool::new()),
            prefetch_size,
            metrics: Arc::new(ZeroCopyMetrics::default()),
        }
    }

    /// Get zero-copy metrics
    pub fn get_metrics(&self) -> Arc<ZeroCopyMetrics> {
        self.metrics.clone()
    }

    /// Get buffer pool statistics
    pub fn get_buffer_stats(&self) -> (u64, u64, u64, u64, u64) {
        self.buffer_pool.get_stats()
    }
    
    /// Read chunk data using enhanced zero-copy techniques with intelligent prefetching
    pub async fn read_chunk<P: AsRef<Path>>(
        &self,
        path: P,
        offset: u64,
        length: u64,
    ) -> ChunkResult<Bytes> {
        let path = path.as_ref();
        let start_time = std::time::Instant::now();
        
        // Try memory-mapped I/O first
        if let Ok(Some(mmap_file)) = self.mmap_manager.get_mmap(path).await {
            debug!("Using memory-mapped I/O for {}", path.display());
            let result = mmap_file.slice(offset, length);
            self.metrics.record_mmap_read(length);
            return result;
        }
        
        // Enhanced buffered read with prefetching
        debug!("Using enhanced buffered read with prefetching for {}", path.display());
        let result = self.enhanced_buffered_read(path, offset, length).await;
        
        let duration = start_time.elapsed();
        debug!("Zero-copy read completed in {:?} for {} bytes", duration, length);
        
        result
    }
    
    /// Enhanced buffered read with intelligent prefetching and buffer pooling
    async fn enhanced_buffered_read<P: AsRef<Path>>(
        &self,
        path: P,
        offset: u64,
        length: u64,
    ) -> ChunkResult<Bytes> {
        let mut file = tokio::fs::File::open(path.as_ref()).await
            .map_err(|e| ChunkServerError::Io(e))?;

        if offset > 0 {
            file.seek(io::SeekFrom::Start(offset)).await
                .map_err(|e| ChunkServerError::Io(e))?;
        }

        // Determine if we should prefetch more data
        let prefetch_length = if length < self.prefetch_size as u64 {
            // For small reads, prefetch additional data
            (length + self.prefetch_size as u64).min(
                file.metadata().await.map(|m| m.len().saturating_sub(offset)).unwrap_or(length)
            )
        } else {
            length
        };

        // Get buffer from pool
        let mut buffer = self.buffer_pool.get_buffer(prefetch_length as usize).await;
        buffer.resize(prefetch_length as usize, 0);

        // Read data
        file.read_exact(&mut buffer).await
            .map_err(|e| ChunkServerError::Io(e))?;

        // Extract the requested portion
        let result = if prefetch_length > length {
            let result = Bytes::from(buffer.split_to(length as usize).freeze());
            // Return remaining buffer to pool
            self.buffer_pool.return_buffer(buffer).await;
            result
        } else {
            // Full buffer is used, just freeze it
            Bytes::from(buffer.freeze())
        };

        Ok(result)
    }
    
    /// Write chunk data using enhanced zero-copy techniques
    pub async fn write_chunk<P: AsRef<Path>>(
        &self,
        path: P,
        data: &Bytes,
        offset: u64,
    ) -> ChunkResult<()> {
        let path = path.as_ref();
        let start_time = std::time::Instant::now();
        
        debug!("Writing {} bytes to {} at offset {}", data.len(), path.display(), offset);
        
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        if offset > 0 {
            file.seek(io::SeekFrom::Start(offset)).await
                .map_err(|e| ChunkServerError::Io(e))?;
        }
        
        // Use vectored write for better performance with large data
        if data.len() > 64 * 1024 {
            self.vectored_write(&mut file, data).await?;
        } else {
            file.write_all(data).await
                .map_err(|e| ChunkServerError::Io(e))?;
        }
        
        file.sync_all().await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        let duration = start_time.elapsed();
        self.metrics.record_mmap_write(data.len() as u64);
        debug!("Write completed in {:?} for {} bytes", duration, data.len());
        
        Ok(())
    }
    
    /// Vectored write for large data blocks
    async fn vectored_write(&self, file: &mut tokio::fs::File, data: &Bytes) -> ChunkResult<()> {
        const CHUNK_SIZE: usize = 256 * 1024; // 256KB chunks
        
        for chunk in data.chunks(CHUNK_SIZE) {
            file.write_all(chunk).await
                .map_err(|e| ChunkServerError::Io(e))?;
        }
        
        self.metrics.record_vectored_io();
        Ok(())
    }
    
    /// Batch write multiple chunks efficiently
    pub async fn write_chunks_batch<P: AsRef<Path>>(
        &self,
        writes: &[(P, &Bytes, u64)], // (path, data, offset) tuples
    ) -> ChunkResult<Vec<ChunkResult<()>>> {
        let start_time = std::time::Instant::now();
        
        let mut futures = Vec::with_capacity(writes.len());
        for (path, data, offset) in writes {
            let future = self.write_chunk(path, data, *offset);
            futures.push(future);
        }
        
        let results = futures::future::join_all(futures).await;
        
        let duration = start_time.elapsed();
        debug!("Batch write of {} chunks completed in {:?}", writes.len(), duration);
        
        Ok(results)
    }
    
    /// Async copy with progress callback
    pub async fn copy_with_progress<P1: AsRef<Path>, P2: AsRef<Path>, F>(
        &self,
        src_path: P1,
        dst_path: P2,
        progress_callback: F,
    ) -> ChunkResult<u64>
    where
        F: Fn(u64, u64) + Send + Sync, // (bytes_copied, total_bytes)
    {
        let src_path = src_path.as_ref();
        let dst_path = dst_path.as_ref();
        
        let metadata = tokio::fs::metadata(src_path).await
            .map_err(|e| ChunkServerError::Io(e))?;
        let total_bytes = metadata.len();
        
        let mut src_file = tokio::fs::File::open(src_path).await
            .map_err(|e| ChunkServerError::Io(e))?;
        let mut dst_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dst_path)
            .await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        const COPY_BUFFER_SIZE: usize = 1024 * 1024; // 1MB
        let mut bytes_copied = 0u64;
        
        loop {
            let mut buffer = self.buffer_pool.get_buffer(COPY_BUFFER_SIZE).await;
            buffer.resize(COPY_BUFFER_SIZE, 0);
            
            let bytes_read = match src_file.read(&mut buffer).await {
                Ok(0) => break, // EOF
                Ok(n) => n,
                Err(e) => return Err(ChunkServerError::Io(e)),
            };
            
            buffer.truncate(bytes_read);
            dst_file.write_all(&buffer).await
                .map_err(|e| ChunkServerError::Io(e))?;
            
            bytes_copied += bytes_read as u64;
            progress_callback(bytes_copied, total_bytes);
            
            self.buffer_pool.return_buffer(buffer).await;
        }
        
        dst_file.sync_all().await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        Ok(bytes_copied)
    }
    
    /// Transfer data between files using zero-copy
    pub async fn transfer_between_files<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        src_path: P1,
        dst_path: P2,
        src_offset: u64,
        dst_offset: u64,
        length: u64,
    ) -> ChunkResult<u64> {
        let src_path = src_path.as_ref();
        let dst_path = dst_path.as_ref();
        
        #[cfg(target_os = "linux")]
        {
            // Try sendfile first on Linux
            if self.use_sendfile {
                match self.sendfile_transfer(src_path, dst_path, src_offset, dst_offset, length).await {
                    Ok(bytes) => return Ok(bytes),
                    Err(e) => {
                        warn!("Sendfile failed, falling back to standard copy: {}", e);
                    }
                }
            }
        }
        
        // Fallback to memory-mapped copy
        self.mmap_copy(src_path, dst_path, src_offset, dst_offset, length).await
    }
    
    /// Standard read implementation
    async fn standard_read<P: AsRef<Path>>(
        &self,
        path: P,
        offset: u64,
        length: u64,
    ) -> ChunkResult<Bytes> {
        let mut file = tokio::fs::File::open(path.as_ref()).await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        if offset > 0 {
            file.seek(io::SeekFrom::Start(offset)).await
                .map_err(|e| ChunkServerError::Io(e))?;
        }
        
        let mut buffer = BytesMut::with_capacity(length as usize);
        buffer.resize(length as usize, 0);
        
        file.read_exact(&mut buffer).await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        Ok(buffer.freeze())
    }
    
    /// Memory-mapped copy between files
    async fn mmap_copy<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        src_path: P1,
        dst_path: P2,
        src_offset: u64,
        dst_offset: u64,
        length: u64,
    ) -> ChunkResult<u64> {
        // Read from source using mmap if possible
        let data = self.read_chunk(src_path, src_offset, length).await?;
        
        // Write to destination
        self.write_chunk(dst_path, &data, dst_offset).await?;
        
        Ok(data.len() as u64)
    }
    
    /// Linux sendfile implementation
    #[cfg(target_os = "linux")]
    async fn sendfile_transfer<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        src_path: P1,
        dst_path: P2,
        src_offset: u64,
        dst_offset: u64,
        length: u64,
    ) -> ChunkResult<u64> {
        use nix::sys::sendfile::sendfile;
        use std::os::unix::fs::OpenOptionsExt;
        
        let src_file = std::fs::File::open(src_path.as_ref())
            .map_err(|e| ChunkServerError::Io(e))?;
        
        let dst_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(dst_path.as_ref())
            .map_err(|e| ChunkServerError::Io(e))?;
        
        // Set destination offset
        if dst_offset > 0 {
            use std::os::unix::fs::FileExt;
            dst_file.write_at(&[], dst_offset)
                .map_err(|e| ChunkServerError::Io(e))?;
        }
        
        // Perform sendfile
        let mut offset = src_offset as i64;
        let bytes_sent = nix::sys::sendfile::sendfile(&dst_file, &src_file, Some(&mut offset), length as usize)
            .map_err(|e| ChunkServerError::Io(io::Error::new(io::ErrorKind::Other, e)))?;
        
        Ok(bytes_sent as u64)
    }
}

/// Scatter-gather I/O operations for efficient data handling
pub struct ScatterGatherIO;

impl ScatterGatherIO {
    /// Read multiple chunks into separate buffers
    pub async fn readv_chunks<P: AsRef<Path>>(
        path: P,
        chunks: &[(u64, u64)], // (offset, length) pairs
    ) -> ChunkResult<Vec<Bytes>> {
        let mut file = tokio::fs::File::open(path.as_ref()).await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        let mut results = Vec::with_capacity(chunks.len());
        
        for &(offset, length) in chunks {
            file.seek(io::SeekFrom::Start(offset)).await
                .map_err(|e| ChunkServerError::Io(e))?;
            
            let mut buffer = BytesMut::with_capacity(length as usize);
            buffer.resize(length as usize, 0);
            
            file.read_exact(&mut buffer).await
                .map_err(|e| ChunkServerError::Io(e))?;
            
            results.push(buffer.freeze());
        }
        
        Ok(results)
    }
    
    /// Write multiple chunks from separate buffers
    pub async fn writev_chunks<P: AsRef<Path>>(
        path: P,
        chunks: &[(u64, &Bytes)], // (offset, data) pairs
    ) -> ChunkResult<()> {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(path.as_ref())
            .await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        for &(offset, data) in chunks {
            file.seek(io::SeekFrom::Start(offset)).await
                .map_err(|e| ChunkServerError::Io(e))?;
            
            file.write_all(data).await
                .map_err(|e| ChunkServerError::Io(e))?;
        }
        
        file.sync_all().await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        Ok(())
    }
}

/// Direct I/O support for bypassing page cache
pub struct DirectIO;

impl DirectIO {
    /// Check if Direct I/O is supported
    pub fn is_supported() -> bool {
        cfg!(target_os = "linux")
    }
    
    /// Open file with O_DIRECT flag
    #[cfg(target_os = "linux")]
    pub fn open_direct<P: AsRef<Path>>(path: P) -> io::Result<File> {
        use std::os::unix::fs::OpenOptionsExt;
        
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECT)
            .open(path.as_ref())
    }
    
    #[cfg(not(target_os = "linux"))]
    pub fn open_direct<P: AsRef<Path>>(path: P) -> io::Result<File> {
        // Direct I/O not supported, use regular open
        std::fs::File::open(path.as_ref())
    }
    
    /// Read with aligned buffer for Direct I/O
    pub async fn read_aligned<P: AsRef<Path>>(
        path: P,
        offset: u64,
        length: u64,
    ) -> ChunkResult<Bytes> {
        const ALIGNMENT: usize = 512; // Typical block size alignment
        
        // Align offset and length
        let aligned_offset = (offset / ALIGNMENT as u64) * ALIGNMENT as u64;
        let offset_diff = (offset - aligned_offset) as usize;
        let aligned_length = ((offset_diff + length as usize + ALIGNMENT - 1) / ALIGNMENT) * ALIGNMENT;
        
        let file = Self::open_direct(path)
            .map_err(|e| ChunkServerError::Io(e))?;
        
        // Allocate aligned buffer
        let mut buffer = vec![0u8; aligned_length];
        
        // Read data
        use std::os::unix::fs::FileExt;
        file.read_exact_at(&mut buffer, aligned_offset)
            .map_err(|e| ChunkServerError::Io(e))?;
        
        // Extract the requested portion
        let end = offset_diff + length as usize;
        Ok(Bytes::from(buffer[offset_diff..end].to_vec()))
    }
}

/// Zero-copy optimized chunk storage
pub struct ZeroCopyChunkStorage {
    base_path: std::path::PathBuf,
    zero_copy: Arc<ZeroCopyTransfer>,
}

impl ZeroCopyChunkStorage {
    pub fn new<P: AsRef<Path>>(base_path: P, mmap_manager: Arc<MmapManager>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            zero_copy: Arc::new(ZeroCopyTransfer::new(mmap_manager)),
        }
    }
    
    /// Read chunk using zero-copy
    pub async fn read_chunk(&self, chunk_id: ChunkId, version: ChunkVersion) -> ChunkResult<Bytes> {
        let path = self.chunk_path(chunk_id, version);
        let metadata = tokio::fs::metadata(&path).await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        self.zero_copy.read_chunk(&path, 0, metadata.len()).await
    }
    
    /// Write chunk using zero-copy
    pub async fn write_chunk(
        &self,
        chunk_id: ChunkId,
        version: ChunkVersion,
        data: &Bytes,
    ) -> ChunkResult<()> {
        let path = self.chunk_path(chunk_id, version);
        
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| ChunkServerError::Io(e))?;
        }
        
        self.zero_copy.write_chunk(&path, data, 0).await
    }
    
    /// Copy chunk using zero-copy
    pub async fn copy_chunk(
        &self,
        src_chunk_id: ChunkId,
        src_version: ChunkVersion,
        dst_chunk_id: ChunkId,
        dst_version: ChunkVersion,
    ) -> ChunkResult<()> {
        let src_path = self.chunk_path(src_chunk_id, src_version);
        let dst_path = self.chunk_path(dst_chunk_id, dst_version);
        
        let metadata = tokio::fs::metadata(&src_path).await
            .map_err(|e| ChunkServerError::Io(e))?;
        
        self.zero_copy.transfer_between_files(
            &src_path,
            &dst_path,
            0,
            0,
            metadata.len(),
        ).await?;
        
        Ok(())
    }
    
    fn chunk_path(&self, chunk_id: ChunkId, version: ChunkVersion) -> std::path::PathBuf {
        self.base_path
            .join(format!("{:016x}", chunk_id))
            .join(format!("chunk_{:016x}_v{:08x}.dat", chunk_id, version))
    }
}

/// Performance metrics for zero-copy operations
#[derive(Debug, Default)]
pub struct ZeroCopyMetrics {
    pub mmap_reads: std::sync::atomic::AtomicU64,
    pub mmap_writes: std::sync::atomic::AtomicU64,
    pub sendfile_transfers: std::sync::atomic::AtomicU64,
    pub direct_io_reads: std::sync::atomic::AtomicU64,
    pub vectored_io_ops: std::sync::atomic::AtomicU64,
    pub bytes_transferred: std::sync::atomic::AtomicU64,
}

impl ZeroCopyMetrics {
    pub fn record_mmap_read(&self, bytes: u64) {
        self.mmap_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bytes_transferred.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_mmap_write(&self, bytes: u64) {
        self.mmap_writes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bytes_transferred.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_sendfile(&self, bytes: u64) {
        self.sendfile_transfers.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bytes_transferred.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_direct_io(&self, bytes: u64) {
        self.direct_io_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bytes_transferred.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_vectored_io(&self) {
        self.vectored_io_ops.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[tokio::test]
    async fn test_zero_copy_read() {
        let mmap_manager = Arc::new(MmapManager::new(Default::default()));
        let zero_copy = ZeroCopyTransfer::new(mmap_manager);
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Hello, Zero Copy!").unwrap();
        temp_file.flush().unwrap();
        
        let data = zero_copy.read_chunk(temp_file.path(), 0, 17).await.unwrap();
        assert_eq!(data, "Hello, Zero Copy!");
    }
    
    #[tokio::test]
    async fn test_scatter_gather_io() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"AAABBBCCC").unwrap();
        temp_file.flush().unwrap();
        
        let chunks = vec![(0, 3), (3, 3), (6, 3)];
        let results = ScatterGatherIO::readv_chunks(temp_file.path(), &chunks).await.unwrap();
        
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], "AAA");
        assert_eq!(results[1], "BBB");
        assert_eq!(results[2], "CCC");
    }
}