// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Redis Sentinel mode example
//
// This example demonstrates using Redis with Sentinel for
// high availability and automatic failover.
//
// Note: This example uses L1-only mode for demonstration.
// To use with Redis Sentinel, configure with:
// - cache_type: TwoLevel
// - l2.mode: Sentinel
// - l2.sentinel.master_name: "mymaster"
// - l2.sentinel.nodes: [...]

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct SentinelData {
    id: u64,
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for demo (no Redis required)
    // For real Sentinel usage, configure with TwoLevel + Sentinel
    let mut services = HashMap::new();

    services.insert(
        "sentinel_cache".to_string(),
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
    let _ = oxcache::init(config).await;

    let client = oxcache::get_client("sentinel_cache")?;

    println!("Redis Sentinel Mode Example");
    println!("===========================\n");
    println!("Note: Using L1-only mode for demo");
    println!("For real Sentinel, configure:");
    println!("  - cache_type: TwoLevel");
    println!("  - l2.mode: Sentinel");
    println!("  - l2.sentinel.master_name: mymaster");
    println!("  - l2.sentinel.nodes: [host1:26379, host2:26379, ...]\n");

    // Test basic operations
    let data = SentinelData {
        id: 1,
        content: "High availability data".to_string(),
    };

    println!("Writing data...");
    client.set("sentinel:test", &data, None).await?;
    println!("  Wrote: {}", data.content);

    println!("\nReading data...");
    if let Some(cached) = client.get::<SentinelData>("sentinel:test").await? {
        println!("  Read: {}", cached.content);
    }

    println!("\nSentinel benefits:");
    println!("  - Automatic failover");
    println!("  - High availability");
    println!("  - Master replica synchronization");

    println!("\n✓ Sentinel mode example completed!");
    Ok(())
}
