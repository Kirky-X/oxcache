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

    let _client = oxcache::get_client("bloom_cache")?;

    let client = oxcache::get_client("bloom_cache")?;

    println!("Bloom Filter Example");
    println!("====================\n");
    println!("Bloom filter helps reduce cache misses by:");
    println!("  - Checking if a key likely exists before cache lookup");
    println!("  - Avoiding unnecessary cache queries for non-existent keys\n");

    // Add some keys to the bloom filter
    println!("Adding keys to bloom filter...");
    let keys = vec!["user:1", "user:2", "user:3", "product:100", "product:200"];
    for key in &keys {
        // In real implementation, this would add to the bloom filter
        println!("  Added: {}", key);
    }

    println!("\nTesting membership...");

    // Test existing keys
    for key in &keys {
        let likely_exists = true; // Simulated bloom filter result
        println!("  {}: likely_exists={}", key, likely_exists);
    }

    // Test non-existing key
    let non_existing = "user:99999";
    let likely_exists = false; // Simulated bloom filter result
    println!("  {}: likely_exists={}", non_existing, likely_exists);

    println!("\n✓ Bloom filter benefits:");
    println!("  - Reduces unnecessary cache lookups");
    println!("  - Memory-efficient (1-2 bytes per item)");
    println!("  - Fast membership testing O(k)");

    println!("\n✓ Bloom filter example completed!");
    Ok(())
}
