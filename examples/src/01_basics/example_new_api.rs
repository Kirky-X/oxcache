// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 新 API 使用示例：类型安全、独立的缓存接口

use oxcache::{Cache, CacheKey, OxCacheResult};
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
async fn main() -> OxCacheResult<()> {
    println!("New API usage example");
    println!("======================\n");

    // === Memory Cache ===
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("1. In-memory cache (L1 only)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let memory_cache: Cache<String, User> = Cache::builder().build().await?;

    // 设置一个值
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    memory_cache.set(&"user:1".to_string(), &user).await?;

    // 获取一个值
    let cached_user: Option<User> = memory_cache.get(&"user:1".to_string()).await?;
    assert!(cached_user.is_some());
    println!(
        "✓ user retrieved from memory cache: {:?}",
        cached_user.unwrap().name
    );

    // === Redis Cache ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("2. Redis cache (L2 only)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // 注意: 需要先启动 Redis 服务
    // 使用 OXCACHE_ALLOW_INSECURE_REDIS=1 环境变量允许非TLS连接
    match Cache::redis("redis://127.0.0.1:6379").await {
        Ok(redis_cache) => {
            redis_cache
                .set(&"user:3".to_string(), &user.clone())
                .await?;
            let cached_user: Option<User> = redis_cache.get(&"user:3".to_string()).await?;
            assert!(cached_user.is_some());
            println!(
                "✓ user retrieved from Redis cache: {:?}",
                cached_user.unwrap().name
            );
        }
        Err(e) => {
            println!("⚠ Redis unavailable, skipping Redis test: {:?}", e);
        }
    }

    // === Custom Key Type ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("3. Custom key type");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let custom_cache: Cache<UserId, User> = Cache::builder().build().await?;

    let user_id = UserId(5);
    custom_cache.set(&user_id, &user).await?;

    let cached_user: Option<User> = custom_cache.get(&user_id).await?;
    assert!(cached_user.is_some());
    println!(
        "✓ user retrieved with custom key: {:?}",
        cached_user.unwrap().name
    );

    // === Advanced Configuration with Builder ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("4. Advanced configuration (using Builder)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    use std::time::Duration;

    // 使用 builder 创建高级配置缓存
    let advanced_cache: Cache<String, User> = Cache::builder()
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    advanced_cache.set(&"user:6".to_string(), &user).await?;
    let cached_user: Option<User> = advanced_cache.get(&"user:6".to_string()).await?;
    assert!(cached_user.is_some());
    println!(
        "✓ user retrieved from the advanced-config cache: {:?}",
        cached_user.unwrap().name
    );

    // === TTL Operations (using set_with_ttl) ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("5. TTL operations (using set_with_ttl)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let ttl_cache: Cache<String, User> = Cache::builder().build().await?;

    // 设置带TTL (60秒)
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Some(Duration::from_secs(60)))
        .await?;

    // 获取并验证 - 值存在
    let cached_user = ttl_cache.get(&"user:7".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ set user:7 with a 60 second TTL");

    // 重新设置带新的TTL (120秒)
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Some(Duration::from_secs(120)))
        .await?;
    println!("✓ updated the TTL of user:7 to 120 seconds");

    // === Batch Operations ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("6. Batch operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let batch_cache: Cache<String, User> = Cache::builder().build().await?;

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

    println!("✓ batch set {} users", batch.len());

    // 获取多个值
    let mut retrieved_count = 0;
    for (key, _) in &batch {
        if batch_cache.get(key).await?.is_some() {
            retrieved_count += 1;
        }
    }

    println!("✓ retrieved {} users from the cache", retrieved_count);

    // === Delete Operations ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("7. Delete operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let delete_cache: Cache<String, User> = Cache::builder().build().await?;

    delete_cache.set(&"user:8".to_string(), &user).await?;

    // Check exists
    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!("✓ user:8 exists: {}", exists);

    // 删除
    delete_cache.delete(&"user:8".to_string()).await?;

    // 删除后检查是否存在
    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!("✓ user:8 exists after delete: {}", exists);

    // === Clear Operations ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("8. Clear operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let clear_cache: Cache<String, User> = Cache::builder().build().await?;

    // Set multiple values
    for i in 1..=3 {
        clear_cache.set(&format!("user:{}", i), &user).await?;
    }

    println!("✓ set 3 users in the cache");

    // 清空所有
    clear_cache.clear().await?;
    println!("✓ cleared all cache entries");

    // 验证为空
    let exists = clear_cache.exists(&"user:1".to_string()).await?;
    println!("✓ user:1 exists after clear: {}", exists);

    // === Get-or-Fallback Pattern ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("9. Cache-aside mode with fallback (get_or)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let fallback_cache: Cache<String, User> = Cache::builder().build().await?;

    // 首次获取 - 使用回退函数
    let user: User = fallback_cache
        .get_or(&"user:10".to_string(), || async {
            Ok::<User, oxcache::OxCacheError>(fetch_user_from_db(10).await)
        })
        .await?;
    println!(
        "✓ user retrieved via the fallback function: {:?}",
        user.name
    );

    // === Summary ===
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("10. Summary");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nNew API features:");
    println!("  ✓ type-safe cache interface");
    println!("  ✓ in-memory cache (Cache::memory())");
    println!("  ✓ Redis cache (Cache::redis())");
    println!("  ✓ custom key types (CacheKey trait)");
    println!("  ✓ builder pattern for configuration (Cache::builder())");
    println!("  ✓ TTL operations (set_with_ttl)");
    println!("  ✓ batch operations (set_many, get_many)");
    println!("  ✓ delete and clear operations (delete, clear)");
    println!("  ✓ existence check (exists)");
    println!("  ✓ cache-aside mode with fallback (get_or)");

    println!("\n✅ New API example complete!");
    Ok(())
}

// 模拟数据库获取的辅助函数
async fn fetch_user_from_db(id: u64) -> User {
    // 模拟数据库延迟
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    User {
        id,
        name: format!("user {}", id),
        email: format!("user{}@example.com", id),
    }
}
