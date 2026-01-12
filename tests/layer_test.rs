//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 分层缓存测试

use oxcache::config::{CacheType, L1Config, L2Config, ServiceConfig};

mod common;

#[tokio::test]
async fn test_l1_only_mode() {
    common::setup_logging();

    let service_name = common::generate_unique_service_name("l1_only_test");

    // 使用 Builder 模式创建配置
    let config = oxcache::oxcache_config()
        .with_global(oxcache::GlobalConfig::default())
        .with_service(
            &service_name,
            ServiceConfig {
                cache_type: CacheType::L1,
                ttl: Some(60),
                serialization: None,
                #[cfg(feature = "l1-moka")]
                l1: Some(L1Config {
                    max_capacity: 1000,
                    max_key_length: 256,
                    max_value_size: 1024 * 1024,
                    cleanup_interval_secs: 60,
                }),
                #[cfg(not(feature = "l1-moka"))]
                l1: None,
                #[cfg(feature = "l2-redis")]
                l2: None,
                #[cfg(feature = "l2-redis")]
                two_level: None,
            },
        )
        .build();

    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn test_l2_only_mode() {
    common::setup_logging();

    let service_name = common::generate_unique_service_name("l2_only_test");

    let config = oxcache::oxcache_config()
        .with_global(oxcache::GlobalConfig::default())
        .with_service(
            &service_name,
            ServiceConfig {
                cache_type: CacheType::L2,
                ttl: Some(300),
                serialization: None,
                #[cfg(feature = "l1-moka")]
                l1: None,
                #[cfg(feature = "l2-redis")]
                l2: Some(
                    L2Config::new()
                        .with_connection_string("redis://127.0.0.1:6379")
                        .with_default_ttl(300),
                ),
                #[cfg(feature = "l2-redis")]
                two_level: None,
            },
        )
        .build();

    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn test_two_level_mode() {
    common::setup_logging();

    let service_name = common::generate_unique_service_name("two_level_test");

    let config = oxcache::oxcache_config()
        .with_global(oxcache::GlobalConfig::default())
        .with_service(
            &service_name,
            ServiceConfig::two_level()
                .with_ttl(600)
                .with_l2(L2Config::new().with_connection_string("redis://127.0.0.1:6379")),
        )
        .build();

    assert!(config.validate().is_ok());
}
