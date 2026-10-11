// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据库集成使用示例
//!
//! 本示例演示了 Oxcache 的数据库集成功能：
//! - 缓存旁路模式
//! - 数据库加载器
//! - 连接字符串解析
//! - 分区策略
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_database_integration
//! ```

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "=== Database integration usage example ===
"
    );

    println!("1. Creating a cache instance");
    let cache: Cache<String, User> = Cache::builder().build().await?;
    println!("   ✓ in-memory cache created");
    println!();

    println!("2. Connection string examples");
    println!("  oxcache can create a Redis cache from a connection string:");
    println!("   Cache::redis(\"redis://127.0.0.1:6379\")  - standalone mode");
    println!("   Cache::redis(\"redis://host1:6379,host2:6379\")  - cluster mode");
    println!("  see the example_redis_modes example for more");
    println!();

    println!("3. Simulating a database loader");

    // 创建一个模拟的数据库加载器
    struct MockDbLoader;

    impl MockDbLoader {
        async fn load_user_from_db(&self, user_id: &str) -> Result<Option<User>, String> {
            // 模拟数据库查询延迟
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // 模拟数据库中的一些用户数据
            match user_id {
                "user:1" => Ok(Some(User {
                    id: 1,
                    name: "Alice".to_string(),
                    email: "alice@example.com".to_string(),
                })),
                "user:2" => Ok(Some(User {
                    id: 2,
                    name: "Bob".to_string(),
                    email: "bob@example.com".to_string(),
                })),
                "user:3" => Ok(Some(User {
                    id: 3,
                    name: "Charlie".to_string(),
                    email: "charlie@example.com".to_string(),
                })),
                _ => Ok(None), // 模拟未找到用户
            }
        }
    }

    let db_loader = MockDbLoader;
    println!("   ✓ simulated database loader created");
    println!();

    println!("4. Implementing the cache-aside pattern");

    async fn get_user_with_cache_bypass(
        cache: &Cache<String, User>,
        db_loader: &MockDbLoader,
        key: &str,
    ) -> Result<Option<User>, Box<dyn std::error::Error>> {
        // 首先尝试从缓存获取
        println!("  trying to read '{}' from the cache", key);
        if let Some(user) = cache.get(&key.to_string()).await? {
            println!("   ✓ cache hit: {:?}", user.name);
            return Ok(Some(user));
        }

        println!("   × cache miss, querying the database");

        // 缓存未命中，从数据库加载
        match db_loader.load_user_from_db(key).await {
            Ok(Some(user)) => {
                println!("   ✓ database query succeeded: {:?}", user.name);

                // 将结果存入缓存
                cache.set(&key.to_string(), &user).await?;
                println!("   ✓ result cached");

                Ok(Some(user))
            }
            Ok(None) => {
                println!("   × user not found in the database");
                Ok(None)
            }
            Err(e) => {
                eprintln!("   × database query error: {}", e);
                Err(e.into())
            }
        }
    }

    println!("5. Testing the cache-aside pattern");

    // 首次查询（未缓存）
    println!("  first query for 'user:1':");
    let user1 = get_user_with_cache_bypass(&cache, &db_loader, "user:1").await?;
    if let Some(user) = user1 {
        println!("  result: {:?}", user);
    }
    println!();

    // 第二次查询（缓存命中）
    println!("  second query for 'user:1':");
    let user1_cached = get_user_with_cache_bypass(&cache, &db_loader, "user:1").await?;
    if let Some(user) = user1_cached {
        println!("  result: {:?}", user);
    }
    println!();

    // 查询不存在的用户
    println!("  querying the nonexistent user 'user:999':");
    let user_missing = get_user_with_cache_bypass(&cache, &db_loader, "user:999").await?;
    match user_missing {
        Some(user) => println!("  unexpected: found the user {:?}", user),
        None => println!("   ✓ correct: the user does not exist"),
    }
    println!();

    println!("6. Batch loading example");

    async fn batch_load_users_with_cache(
        cache: &Cache<String, User>,
        db_loader: &MockDbLoader,
        user_ids: &[&str],
    ) -> Result<Vec<Option<User>>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();

        for user_id in user_ids {
            println!("  processing user '{}':", user_id);

            // 尝试从缓存获取
            if let Some(user) = cache.get(&user_id.to_string()).await? {
                println!("     ✓ cache hit");
                results.push(Some(user));
                continue;
            }

            // 缓存未命中，从数据库加载
            match db_loader.load_user_from_db(user_id).await {
                Ok(Some(user)) => {
                    println!("     ✓ database load succeeded");

                    // 存入缓存
                    cache.set(&user_id.to_string(), &user).await?;
                    println!("     ✓ cached");

                    results.push(Some(user));
                }
                Ok(None) => {
                    println!("     × absent from the database");
                    results.push(None);
                }
                Err(e) => {
                    eprintln!("     × load error: {}", e);
                    results.push(None);
                }
            }
        }

        Ok(results)
    }

    let user_ids = vec!["user:1", "user:2", "user:3", "user:4"];
    let batch_results = batch_load_users_with_cache(&cache, &db_loader, &user_ids).await?;

    println!("  batch load results:");
    for (i, result) in batch_results.iter().enumerate() {
        match result {
            Some(user) => println!("     {}: {}", user_ids[i], user.name),
            None => println!("     {}: not found", user_ids[i]),
        }
    }
    println!();

    println!("7. Cache updates and invalidation");

    // 模拟用户信息更新
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("  updating the user record in the cache...");
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!("   ✓ user record updated");

    // 验证更新
    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!("  verifying the update: {:?}", cached_user.name);
    }
    println!();

    println!("8. Cache invalidation example");

    println!("  invalidating the cache for 'user:2'...");
    cache.delete(&"user:2".to_string()).await?;
    println!("   ✓ cache invalidated");

    // 验证失效 - 应该重新从数据库加载
    println!("  verifying invalidation...");
    let user2_after_invalidation = get_user_with_cache_bypass(&cache, &db_loader, "user:2").await?;
    if let Some(user) = user2_after_invalidation {
        println!("  reload succeeded: {:?}", user.name);
    }
    println!();

    println!("9. Cache statistics");
    match cache.stats().await {
        Ok(stats) => {
            println!(
                "  cache type: {}",
                stats.get("type").unwrap_or(&"N/A".to_string())
            );
            println!(
                "  entries: {}",
                stats.get("entry_count").unwrap_or(&"N/A".to_string())
            );
            println!(
                "  capacity: {}",
                stats.get("capacity").unwrap_or(&"N/A".to_string())
            );
        }
        Err(e) => println!("  failed to get statistics: {}", e),
    }
    println!();

    println!("10. Simulating a partitioning strategy");

    // 简单的哈希分区策略
    fn simple_partition(key: &str, num_partitions: usize) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let hash_value = hasher.finish();
        (hash_value as usize) % num_partitions
    }

    let partitions = 4;
    let sample_keys = vec!["user:1", "user:2", "user:3", "user:4", "user:5", "user:6"];

    println!("  partition count: {}", partitions);
    println!("  partition assignment:");

    for key in &sample_keys {
        let partition = simple_partition(key, partitions);
        println!("     {} -> partition {}", key, partition);
    }
    println!();

    println!("=== Database integration example complete ===");
    println!("  key benefits of database integration:");
    println!("  - cache-aside: cache misses handled automatically");
    println!("  - less database load: higher cache hit rates");
    println!("  - data consistency: prompt updates and invalidation");
    println!("  - partitioning: supports large-scale data distribution");
    println!("  - connection management: uniform connection-string handling");

    Ok(())
}
