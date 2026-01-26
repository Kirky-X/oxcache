// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//

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
        #[cfg(feature = "moka")]
        assert_eq!(info, "L1 Cache (Moka): Enabled");

        #[cfg(not(feature = "moka"))]
        assert_eq!(
            info,
            "L1 Cache (Moka): Disabled (enable with 'moka' feature)"
        );
    }

    #[test]
    fn test_l2_feature_info() {
        let info = get_l2_feature_info();
        #[cfg(feature = "redis")]
        assert_eq!(info, "L2 Cache (Redis): Enabled");

        #[cfg(not(feature = "redis"))]
        assert_eq!(
            info,
            "L2 Cache (Redis): Disabled (enable with 'redis' feature)"
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
        #[cfg(feature = "moka")]
        assert!(is_l1_enabled());

        #[cfg(not(feature = "moka"))]
        assert!(!is_l1_enabled());
    }

    #[test]
    fn test_is_l2_enabled() {
        #[cfg(feature = "redis")]
        assert!(is_l2_enabled());

        #[cfg(not(feature = "redis"))]
        assert!(!is_l2_enabled());
    }
}

/// Test Cache initialization with feature flags (new API)
#[cfg(test)]
mod cache_init_tests {
    use oxcache::Cache;

    #[tokio::test]
    async fn test_l1_cache_initialization() {
        #[cfg(feature = "moka")]
        {
            let _cache: Cache<String, Vec<u8>> = Cache::new().await.unwrap();
            // L1 cache should initialize successfully
        }

        #[cfg(not(feature = "moka"))]
        {
            // Without moka feature, Cache::new should still work (in-memory fallback)
            let _cache: Cache<String, Vec<u8>> = Cache::new().await.unwrap();
            // Memory cache should initialize
        }
    }

    #[tokio::test]
    async fn test_redis_cache_initialization() {
        #[cfg(feature = "redis")]
        {
            // TLS connection test (may fail if Redis not running with TLS)
            let _result: Result<Cache<String, Vec<u8>>, _> =
                Cache::redis("rediss://localhost:6379").await;

            // Non-TLS behavior depends on OXCACHE_ALLOW_INSECURE_REDIS environment variable
            let result: Result<Cache<String, Vec<u8>>, _> =
                Cache::redis("redis://localhost:6379").await;

            if std::env::var("OXCACHE_ALLOW_INSECURE_REDIS").is_ok() {
                // With insecure flag, non-TLS connection should succeed (Redis is running)
                // The result depends on whether Redis is available
                println!("OXCACHE_ALLOW_INSECURE_REDIS is set, non-TLS connection attempted");
            } else {
                // Without flag, non-TLS should fail with configuration error
                assert!(
                    result.is_err(),
                    "Non-TLS should fail without OXCACHE_ALLOW_INSECURE_REDIS"
                );
            }
        }

        #[cfg(not(feature = "redis"))]
        {
            // Skip if redis feature is not enabled
        }
    }
}
