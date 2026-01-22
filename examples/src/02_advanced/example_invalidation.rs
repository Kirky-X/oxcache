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

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

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
    let config = OxcacheConfig::builder()
        .with_service(
            "order_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    if let Err(e) = init(config).await {
        eprintln!("Init error: {:?}", e);
    }

    let client = get_client("order_cache")?;

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
    let temp_config = OxcacheConfig::builder()
        .with_service(
            "temp_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100)),
        )
        .build();

    if let Err(e) = init(temp_config).await {
        eprintln!("Init error: {:?}", e);
    }
    let temp_client = get_client("temp_cache")?;

    temp_client.set("temp:key", &"value", Some(1)).await?;
    assert!(temp_client.get::<String>("temp:key").await?.is_some());
    println!("   Value exists immediately after set");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(temp_client.get::<String>("temp:key").await?.is_none());
    println!("   Value expired after TTL");

    println!("\nCache invalidation example completed!");
    Ok(())
}
