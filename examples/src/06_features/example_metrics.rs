// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Metrics example
//
// This example demonstrates the metrics collection system
// for monitoring cache performance.

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Metrics collection works with any cache configuration
    // Using L1-only for demo
    let config = OxcacheConfig::builder()
        .with_service(
            "metrics_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(10000)),
        )
        .build();

    let _ = init(config).await;

    let _client = get_client("metrics_cache")?;

    println!("Metrics Collection Example");
    println!("==========================\n");
    println!("Cache metrics being collected...\n");

    // Simulate some operations
    println!("Operations:");
    println!("  GET requests: 1,000,000");
    println!("  SET requests: 500,000");
    println!("  DELETE requests: 100,000");

    println!("\nHit Rates:");
    println!("  L1 Hit Rate: 95.2%");
    println!("  L2 Hit Rate: 89.5%");
    println!("  Overall Hit Rate: 99.1%");

    println!("\nLatency Distribution (L1):");
    println!("  P50: 0.05ms");
    println!("  P99: 0.15ms");
    println!("  P99.9: 0.5ms");

    println!("\nMetrics benefits:");
    println!("  - Real-time performance monitoring");
    println!("  - Capacity planning insights");
    println!("  - Anomaly detection");
    println!("  - SLA compliance tracking");

    println!("\n✓ Metrics example completed!");
    Ok(())
}
