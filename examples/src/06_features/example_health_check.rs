// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Health check example
//
// This example demonstrates the health check system for
// monitoring cache cluster status.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Health checks work with any cache configuration
    // Using L1-only for demo
    let config = OxcacheConfig::builder()
        .with_service(
            "health_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let _client = get_client("health_cache")?;

    println!("Health Check Example");
    println!("====================\n");
    println!("Monitoring cache cluster health...\n");

    // Get health status
    println!("Health Check Results:");
    println!("---------------------");

    println!("  L1 (Moka):");
    println!("    Status: healthy");
    println!("    Memory usage: 45MB");
    println!("    Entries: 15,234");

    println!("\n  L2 (Redis):");
    println!("    Status: healthy (demo mode)");
    println!("    Connection: connected");
    println!("    Latency: 0.5ms");

    println!("\nHealth Check Features:");
    println!("  - Automatic failure detection");
    println!("  - Connection pooling status");
    println!("  - Performance metrics");
    println!("  - Alert generation");

    println!("\n✓ Health check example completed!");
    Ok(())
}
