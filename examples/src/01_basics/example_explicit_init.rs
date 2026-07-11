// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Example: Explicit cache initialization
//!
//! This example demonstrates the Brick Architecture pattern where
//! the application must explicitly initialize the cache registry
//! before using any cache operations.

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::interface::{CacheReader, CacheWriter};
use oxcache::backend::MokaMemoryBackend;
use oxcache::registry;

#[tokio::main]
async fn main() {
    println!("=== Explicit Cache Initialization Example ===\n");

    // Step 1: Check registry is not initialized
    println!("Step 1: Check registry status");
    println!("  is_initialized() = {}", registry::is_initialized());
    assert!(!registry::is_initialized(), "Registry should not be initialized yet");

    // Step 2: Create cache instances
    println!("\nStep 2: Create cache instances");
    let default_cache = Arc::new(MokaMemoryBackend::new());
    let user_cache = Arc::new(MokaMemoryBackend::new());

    // Step 3: Initialize registry (MUST be called before any cache operations)
    println!("\nStep 3: Initialize registry with default cache");
    registry::init(default_cache.clone());
    println!("  is_initialized() = {}", registry::is_initialized());
    assert!(registry::is_initialized(), "Registry should be initialized");

    // Step 4: Register additional caches
    println!("\nStep 4: Register additional caches");
    registry::register("users", user_cache.clone());
    println!("  Registered 'users' cache");

    // Step 5: Use the default cache
    println!("\nStep 5: Use the default cache");
    default_cache
        .set("key1", b"value1".to_vec(), Some(Duration::from_secs(60)))
        .await
        .expect("set failed");

    let value = default_cache.get("key1").await.expect("get failed");
    println!("  key1 = {:?}", value.as_deref().map(String::from_utf8_lossy));

    // Step 6: Use the users cache
    println!("\nStep 6: Use the users cache");
    user_cache
        .set("user:1", b"Alice".to_vec(), None)
        .await
        .expect("set failed");

    let user = user_cache.get("user:1").await.expect("get failed");
    println!("  user:1 = {:?}", user.as_deref().map(String::from_utf8_lossy));

    // Step 7: Retrieve cache from registry
    println!("\nStep 7: Retrieve cache from registry");
    if let Some(retrieved) = registry::get("users") {
        let val = retrieved.get("user:1").await.expect("get failed");
        println!(
            "  registry::get('users') -> user:1 = {:?}",
            val.as_deref().map(String::from_utf8_lossy)
        );
    }

    // Step 8: Remove cache from registry
    println!("\nStep 8: Remove cache from registry");
    let removed = registry::remove("users");
    println!("  Removed 'users' cache: {}", removed.is_some());
    assert!(registry::get("users").is_none(), "Cache should be removed");

    println!("\n=== Cache initialized successfully! ===");
}
