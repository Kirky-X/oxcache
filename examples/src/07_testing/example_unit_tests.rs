// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Unit tests example
//
// This example contains unit tests for cache operations.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
struct TestData {
    id: u64,
    value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_operations() {
        let config = OxcacheConfig::builder()
            .with_service(
                "test_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("test_cache").unwrap();

        // Test SET and GET
        let data = TestData {
            id: 1,
            value: "test".to_string(),
        };

        client.set("test_key", &data, None).await.unwrap();
        let retrieved = client.get::<TestData>("test_key").await.unwrap();
        
        assert_eq!(retrieved, Some(data));
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let config = OxcacheConfig::builder()
            .with_service(
                "test_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("test_cache").unwrap();

        // Test cache miss
        let result = client.get::<TestData>("nonexistent_key").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let config = OxcacheConfig::builder()
            .with_service(
                "test_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("test_cache").unwrap();

        let data = TestData {
            id: 1,
            value: "test".to_string(),
        };

        // Set then delete
        client.set("test_key", &data, None).await.unwrap();
        assert!(client.get::<TestData>("test_key").await.unwrap().is_some());
        
        client.delete("test_key").await.unwrap();
        assert!(client.get::<TestData>("test_key").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let config = OxcacheConfig::builder()
            .with_service(
                "test_cache",
                ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100)),
            )
            .build();
        let _ = init(config).await;
        let client = get_client("test_cache").unwrap();

        let data = TestData {
            id: 1,
            value: "test".to_string(),
        };

        // Set with short TTL (1 second)
        client.set("ttl_key", &data, Some(1)).await.unwrap();
        assert!(client.get::<TestData>("ttl_key").await.unwrap().is_some());
        
        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        assert!(client.get::<TestData>("ttl_key").await.unwrap().is_none());
    }
}