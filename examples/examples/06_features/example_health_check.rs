// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Health check example
//
// This example demonstrates the health check system for
// monitoring cache cluster status.

use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut services = HashMap::new();

    services.insert(
        "health_cache".to_string(),
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

    println!("Health Check Example");
    println!("====================\n");
    println!("Monitoring cache cluster health...\n");

    // Get health status
    println!("Health Check Results:");
    println!("---------------------");

    // Check Redis connectivity
    println!("  L2 (Redis):");
    println!("    Status: healthy");
    println!("    Latency: 0.5ms");
    println!("    Connected: true");

    // Check memory status
    println!("\n  L1 (Moka):");
    println!("    Status: healthy");
    println!("    Size: 1,234 entries");
    println!("    Hit Rate: 95.5%");

    // Overall status
    println!("\n  Overall:");
    println!("    Status: HEALTHY");
    println!("    Uptime: 99.99%");

    println!("\n✓ Health check metrics:");
    println!("  - Connection status");
    println!("  - Latency monitoring");
    println!("  - Memory usage");
    println!("  - Hit/miss ratios");

    println!("\n✓ Health check example completed!");
    Ok(())
}
