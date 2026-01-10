// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Cache invalidation example
//
// This example demonstrates cache invalidation strategies:
// - Manual deletion
// - TTL-based expiration
// - Pattern-based invalidation

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Order {
    id: u64,
    user_id: u64,
    total: f64,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only configuration for simplicity (no Redis required)
    let mut services = HashMap::new();

    services.insert(
        "order_cache".to_string(),
        oxcache::config::ServiceConfig {
            l1: Some(oxcache::config::L1Config {
                max_capacity: 10000,
                ..Default::default()
            }),
            cache_type: oxcache::config::CacheType::L1,
            ..Default::default()
        },
    );

    let config = oxcache::config::Config {
        services,
        ..Default::default()
    };
    if let Err(e) = oxcache::init(config).await {
        eprintln!("Init error: {:?}", e);
    }

    let client = oxcache::get_client("order_cache")?;

    // Create test orders
    let orders = vec![
        Order {
            id: 1,
            user_id: 1,
            total: 99.99,
            status: "pending".to_string(),
        },
        Order {
            id: 2,
            user_id: 1,
            total: 149.99,
            status: "processing".to_string(),
        },
        Order {
            id: 3,
            user_id: 2,
            total: 49.99,
            status: "shipped".to_string(),
        },
    ];

    // Cache orders
    println!("Caching orders...");
    for order in &orders {
        client
            .set(&format!("order:{}", order.id), order, None)
            .await?;
    }

    // Manual invalidation - delete single entry
    println!("\n1. Manual deletion:");
    client.delete("order:1").await?;
    assert!(client.get::<Order>("order:1").await?.is_none());
    println!("   Deleted order:1");

    // TTL-based expiration (simulated with short TTL)
    println!("\n2. TTL-based expiration:");
    let temp_client = {
        let mut services = HashMap::new();
        services.insert(
            "temp_cache".to_string(),
            oxcache::config::ServiceConfig {
                l1: Some(oxcache::config::L1Config {
                    max_capacity: 100,
                    ..Default::default()
                }),
                cache_type: oxcache::config::CacheType::L1,
                ..Default::default()
            },
        );
        let config = oxcache::config::Config {
            services,
            ..Default::default()
        };
        if let Err(e) = oxcache::init(config).await {
            eprintln!("Init error: {:?}", e);
        }
        oxcache::get_client("temp_cache")?
    };

    temp_client.set("temp:key", &"value", Some(1)).await?;
    assert!(temp_client.get::<String>("temp:key").await?.is_some());
    println!("   Value exists immediately after set");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(temp_client.get::<String>("temp:key").await?.is_none());
    println!("   Value expired after TTL");

    println!("\nCache invalidation example completed!");
    Ok(())
}
