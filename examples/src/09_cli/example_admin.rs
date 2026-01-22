// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// CLI Admin Command Example
//
// This example demonstrates how to use the oxcache CLI admin command
// to perform administrative operations like cleaning cache and managing warmup.
//
// Note: Requires `cli` feature.

use oxcache::config::{L1Config, L2Config, OxcacheConfig, RedisMode, ServiceConfig};
use oxcache::manager::init;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CLI Admin Command Example");
    println!("==========================\n");

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

    println!("CLI Admin Command Usage:");
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("1. Clean Cache Operation");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nClear L1 cache only:");
    println!("   $ oxcache admin clean --service default --l1");
    println!("\nClear L2 cache only:");
    println!("   $ oxcache admin clean --service default --l2");
    println!("\nClear WAL logs only:");
    println!("   $ oxcache admin clean --service default --wal");
    println!("\nClear all (L1, L2, WAL):");
    println!("   $ oxcache admin clean --service default --l1 --l2 --wal");
    println!("\nSkip confirmation (auto-confirm):");
    println!("   $ oxcache admin clean --service default --l1 --l2 --confirm");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("2. Warmup Management");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nStart warmup:");
    println!("   $ oxcache admin warmup --service default --start");
    println!("\nCheck warmup status:");
    println!("   $ oxcache admin warmup --service default --status");
    println!("\nStop warmup (note: cannot stop mid-execution):");
    println!("   $ oxcache admin warmup --service default --stop");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("3. Example Scenarios");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nScenario 1: Clean up after maintenance");
    println!("   $ oxcache admin clean --service default --l1 --l2 --wal --confirm");
    println!("   → Clears all cache layers and WAL logs");
    println!("   → Useful after database migration or schema changes\n");

    println!("Scenario 2: Pre-warm cache for high traffic");
    println!("   $ oxcache admin warmup --service default --start");
    println!("   $ oxcache admin warmup --service default --status");
    println!("   → Starts preloading hot data into cache");
    println!("   → Monitors warmup progress\n");

    println!("Scenario 3: Clear stale L1 cache");
    println!("   $ oxcache admin clean --service default --l1 --confirm");
    println!("   → Clears only L1 memory cache");
    println!("   → L2 Redis cache remains intact");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("4. Warmup Status Output");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nPending:");
    println!("   Status:          ⏳ PENDING");
    println!("   Progress:        0%\n");

    println!("In Progress:");
    println!("   Status:          🔄 IN PROGRESS");
    println!("   Progress:        67%");
    println!("   Items Processed: 670/1000\n");

    println!("Completed:");
    println!("   Status:          ✅ COMPLETED");
    println!("   Loaded Items:    998");
    println!("   Failed Items:    2\n");

    println!("Failed:");
    println!("   Status:          ❌ FAILED");
    println!("   Error:           Connection timeout");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("5. Safety Features");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n✓ Confirmation prompt for clean operations");
    println!("✓ Service validation before execution");
    println!("✓ Clear error messages");
    println!("✓ Progress tracking for warmup");

    Ok(())
}
