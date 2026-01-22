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

use oxcache::Cache;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a simple memory cache
    let cache: Cache<String, User> = Cache::new().await?;

    // Create test user
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    // Set a value in cache
    println!("Setting user: {}", user.name);
    cache.set(&"user:1".to_string(), &user).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_some());

    // Get a value from cache
    println!("Getting user...");
    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!(
            "Retrieved user: {} ({})",
            cached_user.name, cached_user.email
        );
        assert_eq!(cached_user.id, 1);
    }

    // Delete a value from cache
    println!("Deleting user...");
    cache.delete(&"user:1".to_string()).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_none());

    // Update a value
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("Updating user...");
    cache.set(&"user:1".to_string(), &updated_user).await?;
    if let Some(cached) = cache.get(&"user:1".to_string()).await? {
        println!("Updated user: {} ({})", cached.name, cached.email);
    }

    println!("\n✓ Basic operations example completed!");
    println!("  - Set: Store a value in cache");
    println!("  - Get: Retrieve a value from cache");
    println!("  - Delete: Remove a value from cache");
    println!("  - Update: Overwrite an existing value");
    Ok(())
}
