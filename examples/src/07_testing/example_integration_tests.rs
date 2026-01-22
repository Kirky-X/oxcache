// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Integration tests example
//
// This example contains integration tests for cache behavior
// with real Redis and database connections.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct IntegrationTestData {
    id: u64,
    name: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

fn create_test_config() -> OxcacheConfig {
    OxcacheConfig::builder()
        .with_service(
            "integration_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100)),
        )
        .build()
}

/// Check if Redis is available
async fn is_redis_available() -> bool {
    use std::net::SocketAddr;
    use tokio::net::TcpSocket;
    
    match "127.0.0.1:6379".parse::<SocketAddr>() {
        Ok(addr) => {
            match TcpSocket::connect(addr).await {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cache_with_redis() {
        // Skip test if Redis is not available
        if !is_redis_available().await {
            println!("Skipping Redis integration test - Redis not available");
            return;
        }

        let config = OxcacheConfig::builder()
            .with_service(
                "integration_cache",
                ServiceConfig::two_level()
                    .with_l1(L1Config::new().with_max_capacity(100)),
            )
            .build();
        
        let _ = init(config).await;
        let client = get_client("integration_cache").unwrap();

        let data = IntegrationTestData {
            id: 1,
            name: "Integration Test".to_string(),
            timestamp: chrono::Utc::now(),
        };

        // Test basic operations
        client.set("integration_key", &data, None).await.unwrap();
        let retrieved = client.get::<IntegrationTestData>("integration_key").await.unwrap();
        
        assert_eq!(retrieved, Some(data));
    }

    #[tokio::test]
    async fn test_cache_consistency() {
        let config = create_test_config();
        let _ = init(config).await;
        let client = get_client("integration_cache").unwrap();

        let data = IntegrationTestData {
            id: 1,
            name: "Consistency Test".to_string(),
            timestamp: chrono::Utc::now(),
        };

        // Write data
        client.set("consistency_key", &data, None).await.unwrap();
        
        // Read multiple times
        for _ in 0..10 {
            let retrieved = client.get::<IntegrationTestData>("consistency_key").await.unwrap();
            assert_eq!(retrieved, Some(data.clone()));
        }
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let config = create_test_config();
        let _ = init(config).await;
        let client = get_client("integration_cache").unwrap();

        let mut handles = Vec::new();
        
        // Spawn concurrent tasks
        for i in 0..10 {
            let client_clone = client.clone();
            let handle = tokio::spawn(async move {
                let data = IntegrationTestData {
                    id: i,
                    name: format!("Concurrent Test {}", i),
                    timestamp: chrono::Utc::now(),
                };
                
                let key = format!("concurrent_key_{}", i);
                client_clone.set(&key, &data, None).await.unwrap();
                
                let retrieved = client_clone.get::<IntegrationTestData>(&key).await.unwrap();
                assert_eq!(retrieved, Some(data));
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }
}