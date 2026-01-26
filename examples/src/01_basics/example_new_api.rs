// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 新API使用示例
//
// 本示例演示新API (v0.2.0+) 的创建和使用缓存。
// 新API提供类型安全、独立的缓存接口。

use oxcache::{Cache, CacheKey, Result};
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
async fn main() -> Result<()> {
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

    // ============================================================================
    // 2. Redis Cache
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("2. Redis缓存 (仅L2)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // 注意: 需要先启动 Redis 服务
    // 使用 OXCACHE_ALLOW_INSECURE_REDIS=1 环境变量允许非TLS连接
    match Cache::redis("redis://127.0.0.1:6379").await {
        Ok(redis_cache) => {
            redis_cache.set(&"user:3".to_string(), &user.clone()).await?;
            let cached_user: Option<User> = redis_cache.get(&"user:3".to_string()).await?;
            assert!(cached_user.is_some());
            println!("✓ 从Redis缓存检索用户: {:?}", cached_user.unwrap().name);
        }
        Err(e) => {
            println!("⚠ Redis不可用，跳过Redis测试: {:?}", e);
        }
    }

    // ============================================================================
    // 3. Custom Key Type
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("3. 自定义键类型");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let custom_cache: Cache<UserId, User> = Cache::new().await?;

    let user_id = UserId(5);
    custom_cache.set(&user_id, &user).await?;

    let cached_user: Option<User> = custom_cache.get(&user_id).await?;
    assert!(cached_user.is_some());
    println!("✓ 使用自定义键检索用户: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 4. Advanced Configuration with Builder
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("4. 高级配置 (使用 Builder)");
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
    println!("✓ 从高级配置缓存检索用户: {:?}", cached_user.unwrap().name);

    // ============================================================================
    // 5. TTL Operations (using set_with_ttl)
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("5. TTL操作 (使用 set_with_ttl)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let ttl_cache: Cache<String, User> = Cache::new().await?;

    // 设置带TTL (60秒)
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Some(Duration::from_secs(60)))
        .await?;

    // 获取并验证 - 值存在
    let cached_user = ttl_cache.get(&"user:7".to_string()).await?;
    assert!(cached_user.is_some());
    println!("✓ 设置带60秒TTL的用户:7");

    // 重新设置带新的TTL (120秒)
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Some(Duration::from_secs(120)))
        .await?;
    println!("✓ 更新用户:7的TTL为120秒");

    // ============================================================================
    // 6. Batch Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("6. 批量操作");
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
    // 7. Delete Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("7. 删除操作");
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
    // 8. Clear Operations
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("8. 清空操作");
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
    // 9. Get-or-Fallback Pattern
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("9. 带回退的缓存旁路模式 (get_or)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let fallback_cache: Cache<String, User> = Cache::new().await?;

    // 首次获取 - 使用回退函数
    let user: User = fallback_cache
        .get_or(&"user:10".to_string(), || async {
            Ok::<User, oxcache::CacheError>(fetch_user_from_db(10).await)
        })
        .await?;
    println!("✓ 通过回退函数检索用户: {:?}", user.name);

    // ============================================================================
    // 10. Summary
    // ============================================================================
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("10. 总结");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n新API特性:");
    println!("  ✓ 类型安全的缓存接口");
    println!("  ✓ 内存缓存 (Cache::new())");
    println!("  ✓ Redis缓存 (Cache::redis())");
    println!("  ✓ 自定义键类型 (CacheKey trait)");
    println!("  ✓ 用于配置的构建器模式 (Cache::builder())");
    println!("  ✓ TTL操作 (set_with_ttl)");
    println!("  ✓ 批量操作 (set_many, get_many)");
    println!("  ✓ 删除和清空操作 (delete, clear)");
    println!("  ✓ 存在检查 (exists)");
    println!("  ✓ 带回退的缓存旁路模式 (get_or)");

    println!("\n✅ 新API示例完成!");
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
