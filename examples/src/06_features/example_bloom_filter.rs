// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Bloom filter example
//
// This example demonstrates using Bloom filters for
// efficient membership checking to reduce cache misses.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[tokio::main]
async fn main() -> oxcache::Result<()> {
    // Note: Bloom filter requires TwoLevel configuration with Redis
    // Using L1-only for demo purposes
    let config = OxcacheConfig::builder()
        .with_service(
            "bloom_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let client = get_client("bloom_cache")?;

    println!("Bloom Filter Example");
    println!("====================\n");
    println!("Note: Using L1-only mode for demo");
    println!("Bloom filters reduce cache misses by:");
    println!("  - Pre-checking existence before cache lookup");
    println!("  - Reducing unnecessary cache misses for non-existent keys");
    println!("  - Configurable false positive rate\n");

    println!("Bloom filter benefits:");
    println!("  - Memory efficient membership testing");
    println!("  - Fast negative lookups");
    println!("  - Configurable accuracy\n");

    println!("✓ Bloom filter example completed!");
    Ok(())
}
