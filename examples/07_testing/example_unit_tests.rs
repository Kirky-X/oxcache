// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Unit tests example
//
// This example contains unit tests for cache operations.

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
struct TestData {
    id: u64,
    value: String,
}

#[tokio::test]
async fn test_basic_operations() {
    let mut services = HashMap::new();
    services.insert(
        "test_cache".to_string(),
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
    let client = oxcache::get_client("test_cache").unwrap();

    // Test SET and GET
    let data = TestData {
        id: 1,
        value: "test".to_string(),
    };
    client.set("test:1", &data, Some(60)).await.unwrap();
    let result = client.get::<TestData>("test:1").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().value, "test");
}

#[tokio::test]
async fn test_delete() {
    let client = oxcache::get_client("test_cache").unwrap();

    // Delete existing key
    client.delete("test:1").await.unwrap();
    let result = client.get::<TestData>("test:1").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_nonexistent_key() {
    let client = oxcache::get_client("test_cache").unwrap();
    let result = client.get::<TestData>("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_update() {
    let client = oxcache::get_client("test_cache").unwrap();

    let data1 = TestData {
        id: 2,
        value: "original".to_string(),
    };
    client.set("test:2", &data1, Some(60)).await.unwrap();

    let data2 = TestData {
        id: 2,
        value: "updated".to_string(),
    };
    client.set("test:2", &data2, Some(60)).await.unwrap();

    let result = client.get::<TestData>("test:2").await.unwrap();
    assert_eq!(result.unwrap().value, "updated");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running unit tests...");
    println!("\nUnit tests are in #[cfg(test)] modules");
    println!("Use: cargo test --example example_unit_tests");
    println!("\n✓ Unit tests example completed!");
    Ok(())
}
