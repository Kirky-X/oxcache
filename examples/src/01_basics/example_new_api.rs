// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 新API使用示例
//
// 本示例演示新API (v0.2.0+) 的创建和使用缓存。
// 新API提供类型安全、独立的缓存接口。

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
    println!("新API使用示例");
    println!("======================\n");

    // ============================================================================
    // 1. Memory Cache
    // ============================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("1. 内存缓存 (仅L1)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let memory_cache: Cache<String, User> = Cache::new().await?;

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
    println!("✓ 从内存缓存检索用户: {:?}", cached_user.unwrap().name);

    // 带回退的缓存旁路模式
    let user: User = memory_cache
        .get_or(&"user:2".to_string(), || async {
            fetch_user_from_db(2).await
        })
        .await?;
    println!("✓ 通过回退检索用户: {:?}", user.name);

    // ============================================================================
    // 2. Redis Cache
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("2. Redis缓存 (仅L2)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let redis_cache: Cache<String, User> =
        Cache::redis("redis://127.0.0.1:6379").await?;

    redis_cache.set(&"user:3".to_string(), &user.clone()).await?;

    let cached_user: Option<User> = redis_cache.get(&"user:3".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ 从Redis缓存检索用户: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 3. Tiered Cache (L1 + L2)
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("3. 分层缓存 (L1 + L2)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let tiered_cache: Cache<String, User> =
        Cache::tiered(10000, "redis://127.0.0.1:6379").await?;

    tiered_cache.set(&"user:4".to_string(), &user.clone()).await?;

    // 首次获取 - 从L2获取，在L1缓存
    let cached_user: Option<User> = tiered_cache.get(&"user:4".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ 首次获取 (来自L2): {:?}", cached_user.unwrap().name);

    // 第二次获取 - 从L1获取 (快速)
    let cached_user: Option<User> = tiered_cache.get(&"user:4".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ 第二次获取 (来自L1): {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 4. Custom Key Type
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("4. 自定义键类型");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let custom_cache: Cache<UserId, User> = Cache::new().await?;

    let user_id = UserId(5);
    custom_cache.set(&user_id, &user).await?;

    let cached_user: Option<User> = custom_cache.get(&user_id).await?;
    assert!(cached_user.is_some());
    println!("✓ 使用自定义键检索用户: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 5. Advanced Configuration with Builder
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("5. 高级配置");
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
    println!("✓ 从高级缓存检索用户: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 6. TTL Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("6. TTL操作");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let ttl_cache: Cache<String, User> = Cache::new().await?;

    // 设置带TTL
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Duration::from_secs(60))
        .await?;

    // 获取TTL
    let ttl = ttl_cache.ttl(&"user:7".to_string()).await?;
    println!("✓ 用户:7的TTL: {:?}", ttl);

    // 刷新TTL
    ttl_cache
        .refresh_ttl(&"user:7".to_string(), Duration::from_secs(120))
        .await?;

    let new_ttl = ttl_cache.ttl(&"user:7".to_string()).await?;
    println!("✓ 刷新用户:7的TTL: {:?}", new_ttl);

    // ============================================================================
    // 7. Batch Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("7. 批量操作");
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

    println!("✓ 批量设置 {} 个用户", batch.len());

    // 获取多个值
    let mut retrieved_count = 0;
    for (key, _) in &batch {
        if let Some(_) = batch_cache.get(key).await? {
            retrieved_count += 1;
        }
    }

    println!("✓ 从缓存检索 {} 个用户", retrieved_count);

    // ============================================================================
    // 8. Delete Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("8. 删除操作");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let delete_cache: Cache<String, User> = Cache::new().await?;

    delete_cache.set(&"user:8".to_string(), &user).await?;

    // Check exists
    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!("✓ 用户:8存在: {}", exists);

    // 删除
    delete_cache.delete(&"user:8".to_string()).await?;

    // 删除后检查是否存在
    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!("✓ 删除后用户:8存在: {}", exists);

    // ============================================================================
    // 9. Clear Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("9. 清空操作");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let clear_cache: Cache<String, User> = Cache::new().await?;

    // Set multiple values
    for i in 1..=3 {
        clear_cache.set(&format!("user:{}", i), &user).await?;
    }

    println!("✓ 在缓存中设置3个用户");

    // 清空所有
    clear_cache.clear().await?;
    println!("✓ 已清除所有缓存条目");

    // 验证为空
    let exists = clear_cache.exists(&"user:1".to_string()).await?;
    println!("✓ 清空后用户:1存在: {}", exists);

    // ============================================================================
    // 10. Summary
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("10. 总结");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n新API特性:");
    println!("  ✓ 类型安全的缓存接口");
    println!("  ✓ 内存、Redis和分层缓存");
    println!("  ✓ 自定义键类型");
    println!("  ✓ 用于配置的构建器模式");
    println!("  ✓ TTL操作");
    println!("  ✓ 批量操作");
    println!("  ✓ 删除和清空操作");
    println!("  ✓ 带回退的缓存旁路模式");

    Ok(())
}

// 模拟数据库获取的辅助函数
async fn fetch_user_from_db(id: u64) -> User {
    // 模拟数据库延迟
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    User {
        id,
        name: format!("用户 {}", id),
        email: format!("用户{}@example.com", id),
    }
}
