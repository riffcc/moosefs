use mooseng_client::{config::ClientConfig, cache::ClientCache};
use std::time::Duration;

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
}