// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// New API Usage Example
//
// This example demonstrates the new API (v0.2.0+) for creating and using caches.
// The new API provides a type-safe, independent cache interface.

use oxcache::{Cache, CacheKey};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct User {
    id: u64,
    name: String,
    email: String,
}

// 实现 CacheKey trait 用于自定义键类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UserId(u64);

impl CacheKey for UserId {
    fn to_key_string(&self) -> String {
        format!("user:{}", self.0)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("New API Usage Example");
    println!("======================\n");

    // ============================================================================
    // 1. Memory Cache
    // ============================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("1. Memory Cache (L1 Only)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let memory_cache: Cache<String, User> = Cache::new().await?;

    // Set a value
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    memory_cache.set(&"user:1".to_string(), &user).await?;

    // Get a value
    let cached_user: Option<User> = memory_cache.get(&"user:1".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ Retrieved user from memory cache: {:?}", cached_user.unwrap().name);

    // Cache-aside pattern with fallback
    let user: User = memory_cache
        .get_or(&"user:2".to_string(), || async {
            fetch_user_from_db(2).await
        })
        .await?;
    println!("✓ Retrieved user with fallback: {:?}", user.name);

    // ============================================================================
    // 2. Redis Cache
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("2. Redis Cache (L2 Only)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let redis_cache: Cache<String, User> =
        Cache::redis("redis://127.0.0.1:6379").await?;

    redis_cache.set(&"user:3".to_string(), &user.clone()).await?;

    let cached_user: Option<User> = redis_cache.get(&"user:3".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ Retrieved user from Redis cache: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 3. Tiered Cache (L1 + L2)
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("3. Tiered Cache (L1 + L2)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let tiered_cache: Cache<String, User> =
        Cache::tiered(10000, "redis://127.0.0.1:6379").await?;

    tiered_cache.set(&"user:4".to_string(), &user.clone()).await?;

    // First get - fetches from L2, caches in L1
    let cached_user: Option<User> = tiered_cache.get(&"user:4".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ First get (from L2): {:?}", cached_user.unwrap().name);

    // Second get - fetches from L1 (fast)
    let cached_user: Option<User> = tiered_cache.get(&"user:4".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ Second get (from L1): {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 4. Custom Key Type
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("4. Custom Key Type");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let custom_cache: Cache<UserId, User> = Cache::new().await?;

    let user_id = UserId(5);
    custom_cache.set(&user_id, &user).await?;

    let cached_user: Option<User> = custom_cache.get(&user_id).await?;
    assert!(cached_user.is_some());
    println!("✓ Retrieved user with custom key: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 5. Advanced Configuration with Builder
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("5. Advanced Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    use oxcache::builder::{BackendBuilder, CacheBuilder};
    use std::time::Duration;

    let advanced_cache: Cache<String, User> = CacheBuilder::new()
        .backend(
            BackendBuilder::tiered()
                .l1_capacity(5000)
                .l2_connection_string("redis://127.0.0.1:6379")
                .auto_promote(true)
        )
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    advanced_cache.set(&"user:6".to_string(), &user).await?;
    let cached_user: Option<User> = advanced_cache.get(&"user:6".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ Retrieved user from advanced cache: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 6. TTL Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("6. TTL Operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let ttl_cache: Cache<String, User> = Cache::new().await?;

    // Set with TTL
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Duration::from_secs(60))
        .await?;

    // Get TTL
    let ttl = ttl_cache.ttl(&"user:7".to_string()).await?;
    println!("✓ TTL for user:7: {:?}", ttl);

    // Refresh TTL
    ttl_cache
        .refresh_ttl(&"user:7".to_string(), Duration::from_secs(120))
        .await?;

    let new_ttl = ttl_cache.ttl(&"user:7".to_string()).await?;
    println!("✓ Refreshed TTL for user:7: {:?}", new_ttl);

    // ============================================================================
    // 7. Batch Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("7. Batch Operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let batch_cache: Cache<String, User> = Cache::new().await?;

    // Set multiple values
    let mut batch = Vec::new();
    for i in 1..=5 {
        let user = User {
            id: i,
            name: format!("User {}", i),
            email: format!("user{}@example.com", i),
        };
        batch.push((format!("user:{}", i), user));
    }

    for (key, value) in &batch {
        batch_cache.set(key, value).await?;
    }

    println!("✓ Set {} users in batch", batch.len());

    // Get multiple values
    let mut retrieved_count = 0;
    for (key, _) in &batch {
        if let Some(_) = batch_cache.get(key).await? {
            retrieved_count += 1;
        }
    }

    println!("✓ Retrieved {} users from cache", retrieved_count);

    // ============================================================================
    // 8. Delete Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("8. Delete Operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let delete_cache: Cache<String, User> = Cache::new().await?;

    delete_cache.set(&"user:8".to_string(), &user).await?;

    // Check exists
    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!("✓ User:8 exists: {}", exists);

    // Delete
    delete_cache.delete(&"user:8".to_string()).await?;

    // Check exists after delete
    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!("✓ User:8 exists after delete: {}", exists);

    // ============================================================================
    // 9. Clear Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("9. Clear Operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let clear_cache: Cache<String, User> = Cache::new().await?;

    // Set multiple values
    for i in 1..=3 {
        clear_cache.set(&format!("user:{}", i), &user).await?;
    }

    println!("✓ Set 3 users in cache");

    // Clear all
    clear_cache.clear().await?;
    println!("✓ Cleared all cache entries");

    // Verify empty
    let exists = clear_cache.exists(&"user:1".to_string()).await?;
    println!("✓ User:1 exists after clear: {}", exists);

    // ============================================================================
    // 10. Summary
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("10. Summary");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nNew API Features:");
    println!("  ✓ Type-safe cache interface");
    println!("  ✓ Memory, Redis, and Tiered caches");
    println!("  ✓ Custom key types");
    println!("  ✓ Builder pattern for configuration");
    println!("  ✓ TTL operations");
    println!("  ✓ Batch operations");
    println!("  ✓ Delete and clear operations");
    println!("  ✓ Cache-aside pattern with fallback");

    Ok(())
}

// Helper function to simulate database fetch
async fn fetch_user_from_db(id: u64) -> User {
    // Simulate database latency
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    User {
        id,
        name: format!("User {}", id),
        email: format!("user{}@example.com", id),
    }
}
