use mooseng_client::{
    config::ClientConfig, 
    cache::ClientCache,
    error::ClientError,
};
use mooseng_common::types::{FileAttr, FileType, InodeId};
use std::time::Duration;
use bytes::Bytes;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_config_creation() {
        let config = ClientConfig::default();
        assert!(config.cache.metadata_cache_size > 0);
        assert!(config.cache.metadata_cache_ttl > Duration::ZERO);
    }

    #[tokio::test]
    async fn test_client_cache_creation() {
        let cache = ClientCache::new(
            1000,
            Duration::from_secs(60),
            100,
            Duration::from_secs(30),
            1000000,
            Duration::from_secs(10),
            true
        );
        
        // Test cache is empty initially
        assert!(cache.get_attr(1).await.is_none());
    }

    #[tokio::test]
    async fn test_client_config_defaults() {
        let config = ClientConfig::default();
        
        // Test configuration values have sensible defaults
        assert!(config.cache.metadata_cache_size > 0);
        assert!(config.cache.dir_cache_size > 0);
        assert!(config.session_timeout > Duration::ZERO);
        assert!(config.connect_timeout > Duration::ZERO);
    }

    #[tokio::test]
    async fn test_cache_data_operations() {
        let cache = ClientCache::new(
            1000,
            Duration::from_secs(60),
            100,
            Duration::from_secs(30),
            1000000,
            Duration::from_secs(10),
            true
        );
        
        let inode: InodeId = 123;
        let block_offset = 0;
        let test_data = Bytes::from(vec![1, 2, 3, 4, 5]);
        
        // Test data caching
        cache.put_data(inode, block_offset, test_data.clone()).await;
        let cached_data = cache.get_data(inode, block_offset).await;
        assert_eq!(cached_data, Some(test_data));
        
        // Test data invalidation
        cache.invalidate_data(inode).await;
        let cached_data = cache.get_data(inode, block_offset).await;
        assert_eq!(cached_data, None);
    }

    #[tokio::test]
    async fn test_cache_attribute_operations() {
        let cache = ClientCache::new(
            1000,
            Duration::from_secs(60),
            100,
            Duration::from_secs(30),
            1000000,
            Duration::from_secs(10),
            true
        );
        
        let inode: InodeId = 456;
        let attr = FileAttr {
            inode,
            file_type: FileType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            atime: 1234567890,
            mtime: 1234567890,
            ctime: 1234567890,
            nlink: 1,
            length: 1024,
            storage_class: Default::default(),
        };
        
        // Test attribute caching
        cache.put_attr(inode, attr.clone()).await;
        let cached_attr = cache.get_attr(inode).await;
        assert_eq!(cached_attr.unwrap().inode, attr.inode);
        assert_eq!(cached_attr.unwrap().file_type, attr.file_type);
        
        // Test attribute invalidation
        cache.invalidate_attr(inode).await;
        let cached_attr = cache.get_attr(inode).await;
        assert!(cached_attr.is_none());
    }

    #[tokio::test]
    async fn test_cache_negative_caching() {
        let cache = ClientCache::new(
            1000,
            Duration::from_secs(60),
            100,
            Duration::from_secs(30),
            1000000,
            Duration::from_secs(10),
            true
        );
        
        let parent_inode: InodeId = 1;
        let filename = "nonexistent.txt";
        
        // Initially not cached
        assert!(!cache.is_negative_cached(parent_inode, filename).await);
        
        // Add to negative cache
        cache.add_negative(parent_inode, filename.to_string()).await;
        assert!(cache.is_negative_cached(parent_inode, filename).await);
        
        // Remove from negative cache
        cache.remove_negative(parent_inode, filename).await;
        assert!(!cache.is_negative_cached(parent_inode, filename).await);
    }

    #[tokio::test]
    async fn test_error_to_errno_conversion() {
        // Test various error types convert to appropriate errno values
        assert_eq!(ClientError::FileNotFound(123).to_errno(), libc::ENOENT);
        assert_eq!(ClientError::PermissionDenied.to_errno(), libc::EACCES);
        assert_eq!(ClientError::InvalidArgument("test".to_string()).to_errno(), libc::EINVAL);
        assert_eq!(ClientError::NotSupported("test".to_string()).to_errno(), libc::ENOSYS);
        assert_eq!(ClientError::Timeout.to_errno(), libc::ETIMEDOUT);
        assert_eq!(ClientError::Interrupted.to_errno(), libc::EINTR);
        assert_eq!(ClientError::TryAgain.to_errno(), libc::EAGAIN);
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let cache = ClientCache::new(
            1000,
            Duration::from_secs(60),
            100,
            Duration::from_secs(30),
            1000000,
            Duration::from_secs(10),
            true
        );
        
        // Initial stats should show empty caches
        let stats = cache.stats().await;
        assert_eq!(stats.attr_cache_size, 0);
        assert_eq!(stats.dir_cache_size, 0);
        assert_eq!(stats.data_cache_size, 0);
        assert_eq!(stats.negative_cache_size, 0);
        assert_eq!(stats.open_files_count, 0);
    }
}