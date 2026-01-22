//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Comprehensive example demonstrating basic usage, manual control, and serialization.

use oxcache::manager::{get_client, CacheManager};
use oxcache::{config::SerializationType, CacheExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct AppConfig {
    theme: String,
    max_retries: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct LargeData {
    data: Vec<u8>,
}

// Simulate database query
async fn fetch_user_from_db(id: u64) -> Result<User, String> {
    println!("Fetching user {} from database...", id);
    sleep(Duration::from_millis(100)).await; // Simulate latency
    Ok(User {
        id,
        name: format!("User_{}", id),
        email: format!("user{}@example.com", id),
    })
}

// Simulate cached function
async fn get_user(id: u64) -> Result<User, String> {
    let client = get_client("default_service").expect("Default service not found");

    // Try to get from cache first
    if let Ok(Some(cached_user)) = client.get::<User>(&format!("user_{}", id)).await {
        return Ok(cached_user);
    }

    // Cache miss, get from database
    let user = fetch_user_from_db(id).await?;

    // Store in cache
    let _ = client.set(&format!("user_{}", id), &user, Some(60)).await;

    Ok(user)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Oxcache Comprehensive Example ---\n");

    // Initialize configuration
    let config = oxcache::config::OxcacheConfig::builder()
        .with_service(
            "default_service",
            oxcache::config::ServiceConfig::two_level(),
        )
        .build();

    println!("Initializing CacheManager...");
    if let Err(e) = CacheManager::init(config).await {
        eprintln!(
            "Failed to initialize cache manager: {}. Check if Redis is running.",
            e
        );
        println!("Continuing with potential limitations...");
    }

    // === Part 1: Basic Usage ===
    println!("\n=== Part 1: Basic Usage ===");

    // First call: cache miss
    println!("1. First Call (Cache Miss):");
    let start = std::time::Instant::now();
    let user1 = get_user(1).await?;
    println!("   Result: {:?}", user1);
    println!("   Time: {:?}", start.elapsed());

    // Second call: cache hit
    println!("2. Second Call (Cache Hit):");
    let start = std::time::Instant::now();
    let user2 = get_user(1).await?;
    println!("   Result: {:?}", user2);
    println!("   Time: {:?}", start.elapsed());

    assert_eq!(user1, user2);

    // === Part 2: Manual Control (L1/L2) ===
    println!("\n=== Part 2: Manual Control (L1/L2) ===");
    let client = get_client("default_service").expect("Default service not found");

    let app_config = AppConfig {
        theme: "dark".to_string(),
        max_retries: 5,
    };

    // Write to L1 only
    println!("1. Writing to L1 only (local session data)...");
    client
        .set_l1_only("local_session", &"temp_data", Some(60))
        .await?;
    let val: Option<String> = client.get("local_session").await?;
    println!("   Read from L1: {:?}", val);

    // Write to L2 only
    println!("2. Writing to L2 only (shared config)...");
    client
        .set_l2_only("global_config", &app_config, Some(3600))
        .await?;
    // Read (will pull from L2 and backfill L1)
    let fetched_config: Option<AppConfig> = client.get("global_config").await?;
    println!("   Read from L2: {:?}", fetched_config);

    // Standard Set (write both L1 and L2)
    println!("3. Writing to both L1 and L2...");
    client.set("shared_key", &"shared_value", None).await?;
    let val: Option<String> = client.get("shared_key").await?;
    println!("   Read value: {:?}", val);

    // Delete
    println!("4. Deleting 'shared_key'...");
    client.delete("shared_key").await?;
    let val: Option<String> = client.get("shared_key").await?;
    println!("   Value after delete: {:?}", val);

    println!("\nComprehensive example finished successfully.");
    Ok(())
}
