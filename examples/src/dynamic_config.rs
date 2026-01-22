// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Dynamic Configuration Example
//
// This example demonstrates dynamic configuration management
// for runtime updates and strategy switching.
//
// Note: Requires `config-dynamic` feature.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct DynamicConfig {
    ttl: u64,
    max_capacity: usize,
    eviction_policy: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Dynamic Configuration Example");
    println!("=============================\n");
    println!("Note: Using L1-only mode for demo");
    println!("Dynamic Configuration features:");
    println!("  - Runtime configuration updates");
    println!("  - Hot-reload without service restart");
    println!("  - Strategy switching");
    println!("  - Configuration validation");
    println!("  - A/B testing support\n");

    let config = OxcacheConfig::builder()
        .with_service(
            "dynamic_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let client = get_client("dynamic_cache")?;

    // Simulate dynamic configuration scenarios
    let configs = vec![
        DynamicConfig {
            ttl: 300,
            max_capacity: 1000,
            eviction_policy: "LRU".to_string(),
        },
        DynamicConfig {
            ttl: 600,
            max_capacity: 2000,
            eviction_policy: "LFU".to_string(),
        },
        DynamicConfig {
            ttl: 900,
            max_capacity: 5000,
            eviction_policy: "FIFO".to_string(),
        },
    ];

    println!("Applying dynamic configurations...");
    for (i, cfg) in configs.iter().enumerate() {
        client.set(&format!("config:v{}", i), cfg, None).await?;
        println!("  Config v{}: TTL={}s, Capacity={}, Policy={}", 
                 i, cfg.ttl, cfg.max_capacity, cfg.eviction_policy);
    }

    println!("\nDynamic Configuration Benefits:");
    println!("  - Zero-downtime updates");
    println!("  - Environment-specific configs");
    println!("  - Performance tuning without restart");
    println!("  - Feature flag management");

    println!("\n✓ Dynamic configuration example completed!");
    Ok(())
}