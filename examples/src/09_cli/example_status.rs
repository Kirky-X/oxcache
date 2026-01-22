// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// CLI Status Command Example
//
// This example demonstrates how to use the oxcache CLI status command
// to query cache service status and health information.
//
// Note: Requires `cli` feature.

use oxcache::config::{L1Config, L2Config, OxcacheConfig, RedisMode, ServiceConfig};
use oxcache::manager::init;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CLI Status Command Example");
    println!("===========================\n");

    // Initialize cache service
    let config = OxcacheConfig::builder()
        .with_service(
            "default",
            ServiceConfig::two_level()
                .with_l1(L1Config::new().with_max_capacity(10000))
                .with_l2(
                    L2Config::new()
                        .with_mode(RedisMode::Standalone)
                        .with_connection_string("redis://127.0.0.1:6379"),
                ),
        )
        .build();

    let _ = init(config).await;

    println!("CLI Status Command Usage:");
    println!("\n1. Basic status query:");
    println!("   $ oxcache status");
    println!("   Shows status of all services\n");

    println!("2. Query specific service:");
    println!("   $ oxcache status --service default");
    println!("   Shows detailed status for 'default' service\n");

    println!("3. Verbose output:");
    println!("   $ oxcache status --verbose");
    println!("   Shows detailed information including:");
    println!("   - Service configuration");
    println!("   - Health status");
    println!("   - Cache statistics");
    println!("   - Connection status\n");

    println!("4. Query specific service with verbose output:");
    println!("   $ oxcache status --service default --verbose");
    println!("   Shows all details for a specific service\n");

    println!("Status Information Includes:");
    println!("  ✓ Service name and type");
    println!("  ✓ Health status (Healthy/Degraded/Unhealthy)");
    println!("  ✓ L1 cache statistics (hits, misses, size)");
    println!("  ✓ L2 cache statistics (hits, misses, size)");
    println!("  ✓ Connection status");
    println!("  ✓ Last error (if any)\n");

    println!("Example Output:");
    println!("─────────────────────────────────────────────────────");
    println!("Service: default");
    println!("Type:    TwoLevel");
    println!("Status:  Healthy");
    println!("L1:      hits=1234 misses=56 size=78%");
    println!("L2:      hits=2345 misses=78 size=45%");
    println!("─────────────────────────────────────────────────────");

    Ok(())
}