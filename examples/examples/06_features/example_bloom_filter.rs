// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Bloom filter example
//
// This example demonstrates using Bloom filters for
// efficient membership checking to reduce cache misses.

use oxcache::CacheExt;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> oxcache::Result<()> {
    let mut services = HashMap::new();

    services.insert(
        "bloom_cache".to_string(),
        oxcache::config::ServiceConfig {
            two_level: Some(oxcache::config::TwoLevelConfig {
                bloom_filter: Some(oxcache::config::BloomFilterConfig {
                    expected_elements: 100000,
                    false_positive_rate: 0.01,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            l1: Some(oxcache::config::L1Config {
                max_capacity: 10000,
                ..Default::default()
            }),
            l2: Some(oxcache::config::L2Config {
                connection_string: secrecy::SecretString::new("redis://127.0.0.1:6380".into()),
                mode: oxcache::config::RedisMode::Standalone,
                default_ttl: Some(86400),
                connection_timeout_ms: 5000,
                command_timeout_ms: 10000,
                ..Default::default()
            }),
            cache_type: oxcache::config::CacheType::TwoLevel,
            ..Default::default()
        },
    );

    let config = oxcache::config::OxcacheConfig {
        services,
        ..Default::default()
    };
    if let Err(e) = oxcache::init(config).await {
        eprintln!("Init failed: {:?}", e);
        return Err(e);
    }

    // Use get_typed_client to access specific TwoLevelClient methods like check_bloom_filter
    let client = oxcache::manager::get_typed_client("bloom_cache")?;

    println!("Bloom Filter Example");
    println!("====================\n");
    println!("Bloom filter helps reduce cache misses by:");
    println!("  - Checking if a key likely exists before cache lookup");
    println!("  - Avoiding unnecessary cache queries for non-existent keys\n");

    // Add some keys to the bloom filter
    println!("Adding keys to bloom filter...");
    let keys = vec!["user:1", "user:2", "user:3", "product:100", "product:200"];
    for key in &keys {
        // In real implementation, set() automatically adds key to bloom filter
        client.set(key, &"value", None).await?;
        println!("  Added: {}", key);
    }

    println!("\nTesting membership...");

    // Test existing keys
    for key in &keys {
        // check_bloom_filter returns Result<Option<bool>>
        // Option<bool> is Some(true) if likely exists, Some(false) if definitely not, None if disabled
        let likely_exists = client.check_bloom_filter(key).await?.unwrap_or(false);
        println!("  {}: likely_exists={}", key, likely_exists);
    }

    // Test non-existing key
    let non_existing = "user:99999";
    let likely_exists = client.check_bloom_filter(non_existing).await?.unwrap_or(false);
    println!("  {}: likely_exists={}", non_existing, likely_exists);

    println!("\n✓ Bloom filter benefits:");
    println!("  - Reduces unnecessary cache lookups");
    println!("  - Memory-efficient (1-2 bytes per item)");
    println!("  - Fast membership testing O(k)");

    println!("\n✓ Bloom filter example completed!");
    Ok(())
}
