// Feature-gated initialization tests for CacheManager

#![allow(deprecated)]

use oxcache::{
    get_all_feature_info, get_l1_feature_info, get_l2_feature_info, is_l1_enabled, is_l2_enabled,
};

/// Test feature info functions
#[cfg(test)]
mod feature_info_tests {
    use super::*;

    #[test]
    fn test_l1_feature_info() {
        let info = get_l1_feature_info();
        #[cfg(feature = "l1-moka")]
        assert_eq!(info, "L1 Cache (Moka): Enabled");

        #[cfg(not(feature = "l1-moka"))]
        assert_eq!(
            info,
            "L1 Cache (Moka): Disabled (enable with 'l1-moka' feature)"
        );
    }

    #[test]
    fn test_l2_feature_info() {
        let info = get_l2_feature_info();
        #[cfg(feature = "l2-redis")]
        assert_eq!(info, "L2 Cache (Redis): Enabled");

        #[cfg(not(feature = "l2-redis"))]
        assert_eq!(
            info,
            "L2 Cache (Redis): Disabled (enable with 'l2-redis' feature)"
        );
    }

    #[test]
    fn test_all_feature_info() {
        let infos = get_all_feature_info();
        assert_eq!(infos.len(), 2);
        assert!(infos.iter().any(|s| s.contains("L1")));
        assert!(infos.iter().any(|s| s.contains("L2")));
    }

    #[test]
    fn test_is_l1_enabled() {
        #[cfg(feature = "l1-moka")]
        assert!(is_l1_enabled());

        #[cfg(not(feature = "l1-moka"))]
        assert!(!is_l1_enabled());
    }

    #[test]
    fn test_is_l2_enabled() {
        #[cfg(feature = "l2-redis")]
        assert!(is_l2_enabled());

        #[cfg(not(feature = "l2-redis"))]
        assert!(!is_l2_enabled());
    }
}

/// Test Cache initialization with feature flags (new API)
#[cfg(test)]
mod cache_init_tests {
    use oxcache::Cache;

    #[tokio::test]
    async fn test_l1_cache_initialization() {
        #[cfg(feature = "l1-moka")]
        {
            let _cache: Cache<String, Vec<u8>> = Cache::new().await.unwrap();
            // L1 cache should initialize successfully
        }

        #[cfg(not(feature = "l1-moka"))]
        {
            // Without l1-moka feature, Cache::new should still work (in-memory fallback)
            let _cache: Cache<String, Vec<u8>> = Cache::new().await.unwrap();
            // Memory cache should initialize
        }
    }

    #[tokio::test]
    async fn test_redis_cache_initialization() {
        #[cfg(feature = "l2-redis")]
        {
            // This will fail to connect but should initialize the client
            let _cache: Cache<String, Vec<u8>> =
                Cache::redis("redis://localhost:6379").await.unwrap();
            // We don't assert success here as Redis may not be running
            // The point is that the initialization code paths work
        }

        #[cfg(not(feature = "l2-redis"))]
        {
            // Skip if l2-redis feature is not enabled
        }
    }
}
