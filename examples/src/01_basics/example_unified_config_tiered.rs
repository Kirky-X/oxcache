// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Type-Safe Configuration API - Tiered Cache Example
//
// This example demonstrates the type-safe configuration API for creating
// tiered (L1 + L2) cache using UnifiedConfigBuilder and from_unified_config().

use oxcache::config::UnifiedConfigBuilder;
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Product {
    id: u64,
    name: String,
    price: f64,
    description: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Type-Safe Configuration API: Tiered Cache (L1 + L2) ===\n");

    // Step 1: Create tiered cache configuration
    // This creates a two-level cache with L1 (memory) and L2 (Redis)
    let config = UnifiedConfigBuilder::tiered()
        .with_ttl(7200)                    // Default TTL: 2 hours
        .with_l1_capacity(10000)           // L1 memory cache: 10,000 entries
        .with_redis_url("redis://localhost:6379")  // L2 Redis connection
        .with_redis_mode("standalone")      // Redis mode
        .with_metrics(true)                 // Enable metrics
        .build();

    println!("Configuration created:");
    println!("  - Backend type: Tiered (L1 + L2)");
    println!("  - TTL: {} seconds", config.global.default_ttl);
    println!("  - L1 capacity: {} entries",
             config.backend.l1_options["max_capacity"]);
    println!("  - Redis URL: {}",
             config.backend.l2_options["connection_string"]);
    println!("  - Redis mode: {}", config.backend.l2_options["mode"]);
    println!("  - Metrics enabled: {}", config.metrics.enabled);

    // Step 2: Create cache from configuration
    // Note: This will fail if Redis is not running
    let cache: Cache<String, Product> = match CacheBuilder::from_unified_config(&config)
        .build()
        .await
    {
        Ok(cache) => {
            println!("\nCache created with Redis connection!");
            println!("  - Tiered cache: L1 (memory) + L2 (Redis)\n");
            cache
        }
        Err(e) => {
            println!("\nFailed to connect to Redis: {}", e);
            println!("  This is expected if Redis is not running.");
            println!("  The from_unified_config() method supports graceful fallback.");
            println!("  For testing without Redis, use memory_only() instead.\n");
            return Ok(());
        }
    };

    // Step 3: Demonstrate tiered cache operations
    // In tiered mode, writes go to both L1 and L2
    let product = Product {
        id: 1,
        name: "Rust Programming Language".to_string(),
        price: 49.99,
        description: "The official book about Rust".to_string(),
    };

    println!("Adding product: {} (${})", product.name, product.price);
    cache.set(&"product:1".to_string(), &product).await?;

    // Read from cache
    // First read hits L1, second read hits L1 (very fast)
    println!("\nReading product (first read - populates L1)...");
    if let Some(cached) = cache.get(&"product:1".to_string()).await? {
        println!("  Retrieved: {} (${})", cached.name, cached.price);
        println!("  Description: {}", cached.description);
        assert_eq!(cached.id, 1);
    }

    println!("\nReading product again (L1 hit - very fast)...");
    if let Some(cached) = cache.get(&"product:1".to_string()).await? {
        println!("  Retrieved: {} (${})", cached.name, cached.price);
    }

    // Health check
    println!("\nRunning health check...");
    let healthy = cache.health_check().await?;
    println!("  Cache health: {}", if healthy { "OK" } else { "DEGRADED" });

    // Demonstrate multiple products
    println!("\nAdding multiple products...");
    for i in 2..=5 {
        let product = Product {
            id: i,
            name: format!("Product {}", i),
            price: 10.0 * i as f64,
            description: format!("Description for product {}", i),
        };
        cache.set(&format!("product:{}", i), &product).await?;
    }

    // Read all products
    println!("\nReading all products...");
    for i in 1..=5 {
        if let Some(cached) = cache.get(&format!("product:{}", i)).await? {
            println!("  Product {}: {} (${})", i, cached.name, cached.price);
        }
    }

    // Demonstrate delete operation
    println!("\nDeleting product:1...");
    cache.delete(&"product:1".to_string()).await?;
    assert!(cache.get(&"product:1".to_string()).await?.is_none());
    println!("  Product deleted (removed from both L1 and L2)");

    // Demonstrate cache behavior difference
    println!("\n=== Cache Behavior in Tiered Mode ===");
    println!("  - Write: Data written to both L1 and L2");
    println!("  - Read: Check L1 first, then L2 if L1 miss");
    println!("  - Delete: Removed from both L1 and L2");
    println!("  - Auto-promotion: L2 hits promoted to L1");

    println!("\n✓ Tiered Cache example completed!");
    println!();
    println!("Benefits of Tiered Cache:");
    println!("  - L1: Sub-microsecond access for hot data");
    println!("  - L2: Persistent distributed cache");
    println!("  - Automatic L2→L1 promotion on hits");
    println!("  - Graceful degradation if Redis fails");
    println!("  - Data survives process restarts");

    Ok(())
}
