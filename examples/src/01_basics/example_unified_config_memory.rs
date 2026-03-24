// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Type-Safe Configuration API - Memory-Only Cache Example
//
// This example demonstrates the type-safe configuration API for creating
// memory-only (L1) cache using UnifiedConfigBuilder and from_unified_config().

use oxcache::config::UnifiedConfigBuilder;
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Type-Safe Configuration API: Memory-Only Cache ===\n");

    // Step 1: Create type-safe configuration using builder API
    // This provides compile-time type checking and IDE support
    let config = UnifiedConfigBuilder::memory_only()
        .with_ttl(3600)           // Default TTL: 1 hour
        .with_l1_capacity(10000) // L1 cache capacity: 10,000 entries
        .build()?;

    println!("Configuration created:");
    println!("  - Backend type: Memory (L1 only)");
    println!("  - TTL: {} seconds", config.global.default_ttl);
    println!("  - L1 capacity: {} entries",
             config.backend.l1_options()["max_capacity"]);

    // Step 2: Create cache directly from configuration
    // The from_unified_config() method integrates UnifiedConfig with CacheBuilder
    let cache: Cache<String, User> = CacheBuilder::from_unified_config(&config)?
        .build()
        .await?;

    println!("\nCache created successfully!\n");

    // Step 3: Demonstrate cache operations
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    // Set a value
    println!("Setting user: {} (id={})", user.name, user.id);
    cache.set(&"user:1".to_string(), &user).await?;

    // Get the value
    println!("\nGetting user...");
    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!("  Retrieved: {} ({})", cached_user.name, cached_user.email);
        assert_eq!(cached_user.id, 1);
        assert_eq!(cached_user.name, "Alice");
    }

    // Delete the value
    println!("\nDeleting user...");
    cache.delete(&"user:1".to_string()).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_none());
    println!("  User deleted successfully");

    // Update the value
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("\nUpdating user...");
    cache.set(&"user:1".to_string(), &updated_user).await?;
    if let Some(cached) = cache.get(&"user:1".to_string()).await? {
        println!("  Updated: {} ({})", cached.name, cached.email);
    }

    // Health check
    println!("\nRunning health check...");
    let healthy = cache.health_check().await?;
    println!("  Cache health: {}", if healthy { "OK" } else { "FAILED" });

    // Demonstrate multiple entries
    println!("\nAdding multiple users...");
    for i in 2..=5 {
        let user = User {
            id: i,
            name: format!("User {}", i),
            email: format!("user{}@example.com", i),
        };
        cache.set(&format!("user:{}", i), &user).await?;
    }

    // Read multiple entries
    println!("\nReading all users...");
    for i in 1..=5 {
        if let Some(cached) = cache.get(&format!("user:{}", i)).await? {
            println!("  User {}: {} ({})", i, cached.name, cached.email);
        }
    }

    println!("\n✓ Type-Safe Configuration API example completed!");
    println!();
    println!("Benefits of Type-Safe API:");
    println!("  - Compile-time validation of configuration");
    println!("  - Full IDE autocomplete and type hints");
    println!("  - No runtime TOML parsing overhead");
    println!("  - Better error messages at compile time");
    Ok(())
}
