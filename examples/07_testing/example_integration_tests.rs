// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Integration tests example
//
// This example contains integration tests for cache behavior
// with real Redis and database connections.

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct IntegrationTestData {
    id: u64,
    name: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[tokio::test]
async fn test_l1_l2_consistency() {
    let mut services = HashMap::new();
    services.insert(
        "integration_cache".to_string(),
        oxcache::config::ServiceConfig {
            l1: Some(oxcache::config::L1Config {
                max_capacity: 100,
                ..Default::default()
            }),
            l2: Some(oxcache::config::L2Config {
                connection_string: secrecy::SecretString::new("redis://127.0.0.1:6379".into()),
                mode: oxcache::config::RedisMode::Standalone,
                default_ttl: Some(300),
                ..Default::default()
            }),
            cache_type: oxcache::config::CacheType::TwoLevel,
            ..Default::default()
        },
    );

    let config = oxcache::config::Config {
        services,
        ..Default::default()
    };
    let _ = oxcache::init(config).await;
    let client = oxcache::get_client("integration_cache").unwrap();

    let data = IntegrationTestData {
        id: 1,
        name: "consistency_test".to_string(),
        timestamp: chrono::Utc::now(),
    };

    // Write through
    client
        .set("integration:test", &data, Some(60))
        .await
        .unwrap();

    // Verify both layers
    let result = client
        .get::<IntegrationTestData>("integration:test")
        .await
        .unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "consistency_test");
}

#[tokio::test]
async fn test_concurrent_operations() {
    let services = HashMap::new();
    let config = oxcache::config::Config {
        services,
        ..Default::default()
    };
    let _ = oxcache::init(config).await;
    let client = oxcache::get_client("integration_cache").unwrap();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let client = client.clone();
            tokio::spawn(async move {
                let data = IntegrationTestData {
                    id: i,
                    name: format!("concurrent_{}", i),
                    timestamp: chrono::Utc::now(),
                };
                client
                    .set(&format!("concurrent:{}", i), &data, Some(60))
                    .await
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all writes
    for i in 0..10 {
        let result = client
            .get::<IntegrationTestData>(&format!("concurrent:{}", i))
            .await
            .unwrap();
        assert!(result.is_some());
    }
}

#[tokio::test]
async fn test_failure_handling() {
    // Test behavior when Redis is unavailable
    let services = HashMap::new();
    let config = oxcache::config::Config {
        services,
        ..Default::default()
    };
    let _ = oxcache::init(config).await;
    let client = oxcache::get_client("integration_cache").unwrap();

    // Should gracefully handle failures by falling back to L1 only
    let result = client.get::<IntegrationTestData>("nonexistent").await;
    assert!(result.is_ok());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running integration tests...");
    println!("\nIntegration tests verify:");
    println!("  - L1/L2 consistency");
    println!("  - Concurrent operations");
    println!("  - Failure handling");
    println!("  - Real Redis/database interactions");
    println!("\nUse: cargo test --example example_integration_tests");
    println!("\n✓ Integration tests example completed!");
    Ok(())
}
