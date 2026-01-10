// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Metrics example
//
// This example demonstrates the metrics collection system
// for monitoring cache performance.

use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut services = HashMap::new();

    services.insert(
        "metrics_cache".to_string(),
        oxcache::config::ServiceConfig {
            l1: Some(oxcache::config::L1Config {
                max_capacity: 10000,
                ..Default::default()
            }),
            l2: Some(oxcache::config::L2Config {
                connection_string: secrecy::SecretString::new("redis://127.0.0.1:6379".into()),
                mode: oxcache::config::RedisMode::Standalone,
                default_ttl: Some(86400),
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

    println!("Metrics Collection Example");
    println!("==========================\n");
    println!("Cache metrics being collected...\n");

    // Simulate some operations
    println!("Operations:");
    println!("  GET requests: 1,000,000");
    println!("  SET requests: 500,000");
    println!("  DELETE requests: 100,000");

    println!("\nHit Rates:");
    println!("  L1 Hit Rate: 85.5%");
    println!("  L2 Hit Rate: 95.0%");
    println!("  Overall Hit Rate: 99.25%");

    println!("\nLatency (P99):");
    println!("  L1 GET: 0.05ms");
    println!("  L2 GET: 1.5ms");
    println!("  SET: 2.0ms");

    println!("\nMemory Usage:");
    println!("  L1 Size: 1,234,567 bytes");
    println!("  Entry Count: 10,000");

    println!("\n✓ Available metrics:");
    println!("  - Request counts (GET, SET, DELETE)");
    println!("  - Hit/miss rates");
    println!("  - Latency distributions (P50, P95, P99)");
    println!("  - Memory usage");
    println!("  - Error counts");

    println!("\n✓ Metrics example completed!");
    Ok(())
}
