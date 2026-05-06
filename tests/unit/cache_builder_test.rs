//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache builder tests extracted from cache_builder.rs

use oxcache::backend::MokaMemoryBackend as MemoryBackend;
use oxcache::cache::builder::CacheBuilder;
use oxcache::cache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestValue {
    id: u64,
    name: String,
}

#[tokio::test]
async fn test_cache_builder_default() {
    let cache: Cache<String, TestValue> = CacheBuilder::default().build().await.unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
async fn test_cache_builder_with_capacity() {
    let cache: Cache<String, TestValue> = CacheBuilder::default().capacity(1000).build().await.unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
async fn test_cache_builder_with_ttl() {
    let cache: Cache<String, TestValue> = CacheBuilder::default()
        .ttl(Duration::from_secs(3600))
        .build()
        .await
        .unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
async fn test_cache_builder_with_backend() {
    let backend = MemoryBackend::builder().capacity(5000).build();
    let cache: Cache<String, TestValue> = CacheBuilder::default().with_backend(backend).build().await.unwrap();
    cache.health_check().await.unwrap();
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_cache_builder_from_unified_config_memory() {
    use oxcache::core::confers_config::UnifiedConfigBuilder;

    let config = UnifiedConfigBuilder::memory_only()
        .with_ttl(3600)
        .with_l1_capacity(5000)
        .build()
        .unwrap();

    let builder = CacheBuilder::from_unified_config(&config).unwrap();
    let cache: Cache<String, TestValue> = builder.build().await.unwrap();

    cache.health_check().await.unwrap();
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_cache_builder_from_unified_config_tiered() {
    use oxcache::core::confers_config::UnifiedConfigBuilder;

    let config = UnifiedConfigBuilder::tiered()
        .with_ttl(7200)
        .with_l1_capacity(10000)
        .with_redis_url("redis://localhost:6379")
        .build()
        .unwrap();

    let builder_result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
    assert!(builder_result.is_ok(), "Config should be valid");
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_from_unified_config_valid_memory() {
    use oxcache::core::confers_config::UnifiedConfigBuilder;

    let config = UnifiedConfigBuilder::memory_only()
        .with_ttl(3600)
        .with_l1_capacity(10000)
        .build()
        .unwrap();

    let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
    assert!(result.is_ok(), "Valid memory config should succeed");
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_from_unified_config_valid_tiered() {
    use oxcache::core::confers_config::UnifiedConfigBuilder;

    let config = UnifiedConfigBuilder::tiered()
        .with_ttl(7200)
        .with_l1_capacity(10000)
        .with_redis_url("redis://localhost:6379")
        .build()
        .unwrap();

    let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
    assert!(result.is_ok(), "Valid tiered config should succeed");
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_from_unified_config_redis_missing_connection_string() {
    use oxcache::core::confers_config::{BackendConfig, UnifiedConfig};

    let config = UnifiedConfig {
        backend: BackendConfig {
            backend_type: "Redis".to_string(),
            l2_options_json: serde_json::json!({"mode": "standalone"}).to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
    assert!(result.is_ok(), "Missing connection string should be allowed");
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_from_unified_config_redis_empty_connection_string() {
    use oxcache::core::confers_config::{BackendConfig, UnifiedConfig};

    let config = UnifiedConfig {
        backend: BackendConfig {
            backend_type: "Redis".to_string(),
            l2_options_json: serde_json::json!({"connection_string": "", "mode": "standalone"}).to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
    assert!(result.is_ok(), "Empty connection string should be allowed");
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_from_unified_config_zero_capacity() {
    use oxcache::core::confers_config::{BackendConfig, UnifiedConfig};

    let config = UnifiedConfig {
        backend: BackendConfig {
            backend_type: "Memory".to_string(),
            l1_options_json: serde_json::json!({"max_capacity": 0}).to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = CacheBuilder::<String, TestValue>::from_unified_config(&config);
    assert!(result.is_ok(), "Zero capacity should use default");
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_from_unified_config_with_service_valid() {
    use oxcache::core::confers_config::{CacheType, UnifiedConfigBuilder};

    let config = UnifiedConfigBuilder::memory_only()
        .with_ttl(3600)
        .with_l1_capacity(10000)
        .with_service("user_cache", CacheType::L1, 600)
        .build()
        .unwrap();

    let result = CacheBuilder::<String, TestValue>::from_unified_config_with_service(&config, "user_cache");
    assert!(result.is_ok(), "Valid service config should succeed");
}

#[tokio::test]
#[cfg(feature = "confers")]
async fn test_from_unified_config_with_service_not_found() {
    use oxcache::core::confers_config::UnifiedConfigBuilder;

    let config = UnifiedConfigBuilder::memory_only()
        .with_ttl(3600)
        .with_l1_capacity(10000)
        .build()
        .unwrap();

    let result = CacheBuilder::<String, TestValue>::from_unified_config_with_service(&config, "nonexistent_service");
    match result {
        Err(oxcache::error::CacheError::ServiceNotFound(msg)) => {
            assert!(msg.contains("nonexistent_service"), "Error should mention service name");
        }
        _ => panic!("Expected ServiceNotFound error"),
    }
}

#[test]
#[cfg(feature = "confers")]
fn test_config_format_from_path() {
    use oxcache::core::confers_config::ConfigFormat;

    assert_eq!(ConfigFormat::from_path("config.toml"), Some(ConfigFormat::Toml));
    assert_eq!(ConfigFormat::from_path("config.json"), Some(ConfigFormat::Json));
    assert_eq!(ConfigFormat::from_path("config.yaml"), None);
}

#[test]
#[cfg(feature = "confers")]
fn test_config_format_extension() {
    use oxcache::core::confers_config::ConfigFormat;

    assert_eq!(ConfigFormat::Toml.extension(), "toml");
    assert_eq!(ConfigFormat::Json.extension(), "json");
}
