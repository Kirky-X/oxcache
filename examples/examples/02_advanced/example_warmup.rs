// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Cache warmup example
//
// This example demonstrates cache warmup strategies:
// - Pre-loading data on application startup

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct Config {
    key: String,
    value: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only configuration for simplicity (no Redis required)
    let mut services = HashMap::new();

    services.insert(
        "config_cache".to_string(),
        oxcache::config::ServiceConfig {
            l1: Some(oxcache::config::L1Config {
                max_capacity: 1000,
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

    let client = oxcache::get_client("config_cache")?;

    // Simulate warmup data
    let warmup_data: Vec<Config> = vec![
        Config {
            key: "database.url".to_string(),
            value: "postgres://localhost/db".to_string(),
        },
        Config {
            key: "api.rate_limit".to_string(),
            value: "1000".to_string(),
        },
        Config {
            key: "logging.level".to_string(),
            value: "info".to_string(),
        },
    ];

    println!(
        "Warming up cache with {} configuration entries...",
        warmup_data.len()
    );

    // Warmup strategy: Bulk load
    for cfg in &warmup_data {
        client
            .set(&format!("config:{}", cfg.key), cfg, None)
            .await?;
    }

    // Verify warmup
    println!("\nVerifying warmup...");
    let mut hit_count = 0;

    for cfg in &warmup_data {
        if client
            .get::<Config>(&format!("config:{}", cfg.key))
            .await?
            .is_some()
        {
            hit_count += 1;
        }
    }

    println!(
        "  Warmup hit rate: {}/{} ({}%)",
        hit_count,
        warmup_data.len(),
        hit_count * 100 / warmup_data.len()
    );

    println!("\nCache warmup benefits:");
    println!("  - Reduces cold-start latency");
    println!("  - Prevents cache thrashing on startup");
    println!("  - Improves user experience");

    println!("\n✓ Cache warmup example completed!");
    Ok(())
}
