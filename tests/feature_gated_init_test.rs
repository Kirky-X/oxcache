//! Feature-gated initialization tests for CacheManager

use crate::manager::{
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

/// Test CacheManager initialization with feature flags
#[cfg(test)]
mod cache_manager_init_tests {
    use crate::manager::CacheManager;
    use crate::CacheType;

    // Minimal test config
    fn create_test_config() -> crate::OxcacheConfig {
        crate::OxcacheConfig::builder()
            .with_global(crate::GlobalConfig::default())
            .with_service(
                "test_service",
                crate::ServiceConfig::builder(CacheType::L1)
                    .with_l1_config(crate::L1Config::new(1000))
                    .build(),
            )
            .build()
    }

    #[tokio::test]
    async fn test_init_requires_l1_feature_for_l1_cache() {
        #[cfg(feature = "l1-moka")]
        {
            let config = create_test_config();
            let result = CacheManager::init(config).await;
            assert!(result.is_ok(), "L1 cache should initialize successfully");
        }

        #[cfg(not(feature = "l1-moka"))]
        {
            let config = create_test_config();
            let result = CacheManager::init(config).await;
            assert!(
                result.is_err(),
                "L1 cache should fail without l1-moka feature"
            );
        }
    }
}
