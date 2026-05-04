//! Example: Explicit cache initialization
//!
//! This example demonstrates the Brick Architecture pattern where
//! the application must explicitly initialize the cache registry
//! before using any cache operations.

#[cfg(feature = "moka")]
use std::sync::Arc;
#[cfg(feature = "moka")]
use std::time::Duration;

#[cfg(feature = "moka")]
use oxcache::backend::interface::{CacheReader, CacheWriter};
#[cfg(feature = "moka")]
use oxcache::{init, is_initialized, new_in_memory, register};

#[cfg(feature = "moka")]
#[tokio::main]
async fn main() {
    println!("=== Explicit Cache Initialization Example ===\n");

    // Step 1: Check registry is not initialized
    println!("Step 1: Check registry status");
    println!("  is_initialized() = {}", is_initialized());
    assert!(!is_initialized(), "Registry should not be initialized yet");

    // Step 2: Create cache instances
    println!("\nStep 2: Create cache instances");
    let default_cache = Arc::new(new_in_memory());
    let user_cache = Arc::new(new_in_memory());

    // Step 3: Initialize registry (MUST be called before any cache operations)
    println!("\nStep 3: Initialize registry with default cache");
    init(default_cache.clone());
    println!("  is_initialized() = {}", is_initialized());
    assert!(is_initialized(), "Registry should be initialized");

    // Step 4: Register additional caches
    println!("\nStep 4: Register additional caches");
    register("users", user_cache.clone());
    println!("  Registered 'users' cache");

    // Step 5: Use the default cache
    println!("\nStep 5: Use the default cache");
    default_cache
        .set("key1", b"value1".to_vec(), Some(Duration::from_secs(60)))
        .await
        .expect("set failed");

    let value = default_cache.get("key1").await.expect("get failed");
    println!("  key1 = {:?}", value.map(String::from_utf8_lossy));

    // Step 6: Use the users cache
    println!("\nStep 6: Use the users cache");
    user_cache
        .set("user:1", b"Alice".to_vec(), None)
        .await
        .expect("set failed");

    let user = user_cache.get("user:1").await.expect("get failed");
    println!("  user:1 = {:?}", user.map(String::from_utf8_lossy));

    println!("\n=== Cache initialized successfully! ===");
}

#[cfg(not(feature = "moka"))]
fn main() {
    println!("This example requires the 'moka' feature.");
    println!("Run with: cargo run --example example_explicit_init --features moka");
}
