// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 新 API 使用示例：类型安全、独立的缓存接口

use oxcache::{Cache, CacheKey, OxCacheResult};
use serde::{Deserialize, Serialize};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_NEW_API_1_IN_MEMORY_CACHE, MSG_EXAMPLE_NEW_API_2_REDIS_CACHE_L2,
    MSG_EXAMPLE_NEW_API_3_CUSTOM_KEY_TYPE, MSG_EXAMPLE_NEW_API_4_ADVANCED_CONFIGURATION_USING,
    MSG_EXAMPLE_NEW_API_5_TTL_OPERATIONS_USING, MSG_EXAMPLE_NEW_API_6_BATCH_OPERATIONS,
    MSG_EXAMPLE_NEW_API_7_DELETE_OPERATIONS, MSG_EXAMPLE_NEW_API_8_CLEAR_OPERATIONS,
    MSG_EXAMPLE_NEW_API_9_CACHE_ASIDE_MODE, MSG_EXAMPLE_NEW_API_10_SUMMARY,
    MSG_EXAMPLE_NEW_API_BATCH_OPERATIONS_SET_MANY, MSG_EXAMPLE_NEW_API_BATCH_SET_USERS,
    MSG_EXAMPLE_NEW_API_BUILDER_PATTERN_FOR_CONFIGURATION,
    MSG_EXAMPLE_NEW_API_CACHE_ASIDE_MODE_WITH, MSG_EXAMPLE_NEW_API_CLEARED_ALL_CACHE_ENTRIES,
    MSG_EXAMPLE_NEW_API_CUSTOM_KEY_TYPES_CACHEKEY, MSG_EXAMPLE_NEW_API_DELETE_AND_CLEAR_OPERATIONS,
    MSG_EXAMPLE_NEW_API_EXAMPLE_COMPLETE, MSG_EXAMPLE_NEW_API_EXISTENCE_CHECK_EXISTS,
    MSG_EXAMPLE_NEW_API_FEATURES, MSG_EXAMPLE_NEW_API_IN_MEMORY_CACHE_CACHE,
    MSG_EXAMPLE_NEW_API_REDIS_CACHE_CACHE_REDIS,
    MSG_EXAMPLE_NEW_API_REDIS_UNAVAILABLE_SKIPPING_REDIS,
    MSG_EXAMPLE_NEW_API_RETRIEVED_USERS_FROM_THE, MSG_EXAMPLE_NEW_API_SET_3_USERS_IN,
    MSG_EXAMPLE_NEW_API_SET_USER_7_WITH, MSG_EXAMPLE_NEW_API_TTL_OPERATIONS_SET_WITH,
    MSG_EXAMPLE_NEW_API_TYPE_SAFE_CACHE_INTERFACE, MSG_EXAMPLE_NEW_API_UPDATED_THE_TTL_OF,
    MSG_EXAMPLE_NEW_API_USAGE_EXAMPLE, MSG_EXAMPLE_NEW_API_USER_1_EXISTS_AFTER,
    MSG_EXAMPLE_NEW_API_USER_8_EXISTS, MSG_EXAMPLE_NEW_API_USER_8_EXISTS_AFTER,
    MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_MEMORY, MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_REDIS,
    MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_THE, MSG_EXAMPLE_NEW_API_USER_RETRIEVED_VIA_THE,
    MSG_EXAMPLE_NEW_API_USER_RETRIEVED_WITH_CUSTOM, t,
};

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
    println!("{}", t(MSG_EXAMPLE_NEW_API_USAGE_EXAMPLE, &[]));
    println!("======================\n");

    // Memory Cache
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_1_IN_MEMORY_CACHE, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let memory_cache: Cache<String, User> = Cache::builder().build().await?;

    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    memory_cache.set(&"user:1".to_string(), &user).await?;

    let cached_user: Option<User> = memory_cache.get(&"user:1".to_string()).await?;
    assert!(cached_user.is_some());
    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_MEMORY,
            &[("name", format!("{:?}", cached_user.unwrap().name))]
        )
    );

    // Redis Cache
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_2_REDIS_CACHE_L2, &[]));
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
                "{}",
                t(
                    MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_REDIS,
                    &[("name", format!("{:?}", cached_user.unwrap().name))]
                )
            );
        }
        Err(e) => {
            println!(
                "{}",
                t(
                    MSG_EXAMPLE_NEW_API_REDIS_UNAVAILABLE_SKIPPING_REDIS,
                    &[("e", format!("{:?}", e))]
                )
            );
        }
    }

    // Custom Key Type
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_3_CUSTOM_KEY_TYPE, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let custom_cache: Cache<UserId, User> = Cache::builder().build().await?;

    let user_id = UserId(5);
    custom_cache.set(&user_id, &user).await?;

    let cached_user: Option<User> = custom_cache.get(&user_id).await?;
    assert!(cached_user.is_some());
    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_USER_RETRIEVED_WITH_CUSTOM,
            &[("name", format!("{:?}", cached_user.unwrap().name))]
        )
    );

    // Advanced Configuration with Builder
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(
        "{}",
        t(MSG_EXAMPLE_NEW_API_4_ADVANCED_CONFIGURATION_USING, &[])
    );
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
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_THE,
            &[("name", format!("{:?}", cached_user.unwrap().name))]
        )
    );

    // TTL Operations (using set_with_ttl)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_5_TTL_OPERATIONS_USING, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let ttl_cache: Cache<String, User> = Cache::builder().build().await?;

    // 设置带TTL (60秒)
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Some(Duration::from_secs(60)))
        .await?;

    let cached_user = ttl_cache.get(&"user:7".to_string()).await?;
    assert!(cached_user.is_some());
    println!("{}", t(MSG_EXAMPLE_NEW_API_SET_USER_7_WITH, &[]));

    // 重新设置带新的TTL (120秒)
    ttl_cache
        .set_with_ttl(&"user:7".to_string(), &user, Some(Duration::from_secs(120)))
        .await?;
    println!("{}", t(MSG_EXAMPLE_NEW_API_UPDATED_THE_TTL_OF, &[]));

    // Batch Operations
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_6_BATCH_OPERATIONS, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let batch_cache: Cache<String, User> = Cache::builder().build().await?;

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

    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_BATCH_SET_USERS,
            &[("batch", batch.len().to_string())]
        )
    );

    let mut retrieved_count = 0;
    for (key, _) in &batch {
        if batch_cache.get(key).await?.is_some() {
            retrieved_count += 1;
        }
    }

    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_RETRIEVED_USERS_FROM_THE,
            &[("retrieved_count", retrieved_count.to_string())]
        )
    );

    // Delete Operations
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_7_DELETE_OPERATIONS, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let delete_cache: Cache<String, User> = Cache::builder().build().await?;

    delete_cache.set(&"user:8".to_string(), &user).await?;

    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_USER_8_EXISTS,
            &[("exists", exists.to_string())]
        )
    );

    delete_cache.delete(&"user:8".to_string()).await?;

    let exists = delete_cache.exists(&"user:8".to_string()).await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_USER_8_EXISTS_AFTER,
            &[("exists", exists.to_string())]
        )
    );

    // Clear Operations
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_8_CLEAR_OPERATIONS, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let clear_cache: Cache<String, User> = Cache::builder().build().await?;

    for i in 1..=3 {
        clear_cache.set(&format!("user:{}", i), &user).await?;
    }

    println!("{}", t(MSG_EXAMPLE_NEW_API_SET_3_USERS_IN, &[]));

    clear_cache.clear().await?;
    println!("{}", t(MSG_EXAMPLE_NEW_API_CLEARED_ALL_CACHE_ENTRIES, &[]));

    let exists = clear_cache.exists(&"user:1".to_string()).await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_USER_1_EXISTS_AFTER,
            &[("exists", exists.to_string())]
        )
    );

    // Get-or-Fallback Pattern
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_9_CACHE_ASIDE_MODE, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let fallback_cache: Cache<String, User> = Cache::builder().build().await?;

    // 首次获取 - 使用回退函数
    let user: User = fallback_cache
        .get_or(&"user:10".to_string(), || async {
            Ok::<User, oxcache::OxCacheError>(fetch_user_from_db(10).await)
        })
        .await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_NEW_API_USER_RETRIEVED_VIA_THE,
            &[("name", format!("{:?}", user.name))]
        )
    );

    // Summary
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", t(MSG_EXAMPLE_NEW_API_10_SUMMARY, &[]));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n{}", t(MSG_EXAMPLE_NEW_API_FEATURES, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_NEW_API_TYPE_SAFE_CACHE_INTERFACE, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_NEW_API_IN_MEMORY_CACHE_CACHE, &[]));
    println!("  {}", t(MSG_EXAMPLE_NEW_API_REDIS_CACHE_CACHE_REDIS, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_NEW_API_CUSTOM_KEY_TYPES_CACHEKEY, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_NEW_API_BUILDER_PATTERN_FOR_CONFIGURATION, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_NEW_API_TTL_OPERATIONS_SET_WITH, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_NEW_API_BATCH_OPERATIONS_SET_MANY, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_NEW_API_DELETE_AND_CLEAR_OPERATIONS, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_NEW_API_EXISTENCE_CHECK_EXISTS, &[]));
    println!("  {}", t(MSG_EXAMPLE_NEW_API_CACHE_ASIDE_MODE_WITH, &[]));

    println!("\n{}", t(MSG_EXAMPLE_NEW_API_EXAMPLE_COMPLETE, &[]));
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
