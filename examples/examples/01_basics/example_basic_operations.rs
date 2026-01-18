// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Basic CRUD operations example
//
// This example demonstrates fundamental cache operations:
// - Get: Retrieve cached values
// - Set: Store values in cache
// - Delete: Remove values from cache

use oxcache::CacheExt;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize cache configuration - L1 only (no Redis required)
    let mut services = HashMap::new();

    services.insert(
        "user_cache".to_string(),
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

    // Initialize the cache manager
    let _ = oxcache::init(config).await;

    // Get the cache client
    let client = oxcache::get_client("user_cache")?;

    // Create test user
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    // Set a value in cache
    println!("Setting user: {}", user.name);
    client.set("user:1", &user, Some(3600)).await?;
    assert!(client.get::<User>("user:1").await?.is_some());

    // Get a value from cache
    println!("Getting user...");
    if let Some(cached_user) = client.get::<User>("user:1").await? {
        println!(
            "Retrieved user: {} ({})",
            cached_user.name, cached_user.email
        );
        assert_eq!(cached_user.id, 1);
    }

    // Delete a value from cache
    println!("Deleting user...");
    client.delete("user:1").await?;
    assert!(client.get::<User>("user:1").await?.is_none());

    // Update a value
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("Updating user...");
    client.set("user:1", &updated_user, Some(3600)).await?;
    if let Some(cached) = client.get::<User>("user:1").await? {
        println!("Updated user: {} ({})", cached.name, cached.email);
    }

    println!("\n✓ Basic operations example completed!");
    println!("  - Set: Store a value in cache");
    println!("  - Get: Retrieve a value from cache");
    println!("  - Delete: Remove a value from cache");
    println!("  - Update: Overwrite an existing value");
    Ok(())
}
