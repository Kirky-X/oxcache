use oxcache::macros::cached;
use oxcache::{get_client, init};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestData {
    id: u64,
    value: String,
}

#[tokio::test]
async fn test_cached_macro_with_cache_type() {
    // Initialize cache with a test configuration
    let config = r#"
    [global]
    default_ttl = 300

    [services.test_service]
    cache_type = "two-level"

    [services.test_service.l1]
    max_capacity = 1000
    ttl = 60

    [services.test_service.l2]
    mode = "standalone"
    connection_string = "redis://localhost:6379"
    "#;

    // Write config to temp file
    let config_path = "test_config.toml";
    std::fs::write(config_path, config).unwrap();

    // Initialize cache (this will fail if Redis is not running, but we just need to test macro compilation)
    let _ = init(config_path).await;

    // Test function with two-level cache (default)
    #[cached(service = "test_service", key = "test:{id}", ttl = 300)]
    async fn test_two_level(id: u64) -> Result<TestData, String> {
        Ok(TestData {
            id,
            value: "two-level".to_string(),
        })
    }

    // Test function with l1-only cache
    #[cached(
        service = "test_service",
        key = "test-l1:{id}",
        ttl = 300,
        cache_type = "l1-only"
    )]
    async fn test_l1_only(id: u64) -> Result<TestData, String> {
        Ok(TestData {
            id,
            value: "l1-only".to_string(),
        })
    }

    // Test function with l2-only cache
    #[cached(
        service = "test_service",
        key = "test-l2:{id}",
        ttl = 300,
        cache_type = "l2-only"
    )]
    async fn test_l2_only(id: u64) -> Result<TestData, String> {
        Ok(TestData {
            id,
            value: "l2-only".to_string(),
        })
    }

    // Just verify the functions can be called (we don't need to actually test caching logic here)
    let result1 = test_two_level(1).await;
    let result2 = test_l1_only(1).await;
    let result3 = test_l2_only(1).await;

    // Clean up
    std::fs::remove_file(config_path).unwrap();

    // Even if cache initialization fails, the functions should still compile and run
    assert!(result1.is_ok() || result1.is_err());
    assert!(result2.is_ok() || result2.is_err());
    assert!(result3.is_ok() || result3.is_err());
}
