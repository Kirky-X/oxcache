// Copyright (c) 2025-2026 Kirky.X
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
    println!("=== 数据库集成使用示例 ===
");

    // 1. 创建缓存实例
    println!("1. 创建缓存实例");
    let cache: Cache<String, User> = Cache::builder().build().await?;
    println!("   ✓ 内存缓存创建成功");
    println!();

    // 2. 连接字符串示例
    println!("2. 连接字符串示例");
    println!("   oxcache 支持通过连接字符串创建 Redis 缓存:");
    println!("   Cache::redis(\"redis://127.0.0.1:6379\")  — 单机模式");
    println!("   Cache::redis(\"redis://host1:6379,host2:6379\")  — 集群模式");
    println!("   详细示例请参考 example_redis_modes");
    println!();

    // 3. 模拟数据库加载器
    println!("3. 模拟数据库加载器");

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
    println!("   ✓ 模拟数据库加载器创建成功");
    println!();

    // 4. 缓存旁路模式实现
    println!("4. 缓存旁路模式实现");

    async fn get_user_with_cache_bypass(
        cache: &Cache<String, User>,
        db_loader: &MockDbLoader,
        key: &str,
    ) -> Result<Option<User>, Box<dyn std::error::Error>> {
        // 首先尝试从缓存获取
        println!("   尝试从缓存获取 '{}'", key);
        if let Some(user) = cache.get(&key.to_string()).await? {
            println!("   ✓ 缓存命中: {:?}", user.name);
            return Ok(Some(user));
        }

        println!("   × 缓存未命中，查询数据库");

        // 缓存未命中，从数据库加载
        match db_loader.load_user_from_db(key).await {
            Ok(Some(user)) => {
                println!("   ✓ 数据库查询成功: {:?}", user.name);

                // 将结果存入缓存
                cache.set(&key.to_string(), &user).await?;
                println!("   ✓ 结果已缓存");

                Ok(Some(user))
            }
            Ok(None) => {
                println!("   × 数据库中未找到用户");
                Ok(None)
            }
            Err(e) => {
                eprintln!("   × 数据库查询错误: {}", e);
                Err(e.into())
            }
        }
    }

    // 5. 测试缓存旁路模式
    println!("5. 测试缓存旁路模式");

    // 首次查询（未缓存）
    println!("   首次查询 'user:1':");
    let user1 = get_user_with_cache_bypass(&cache, &db_loader, "user:1").await?;
    if let Some(user) = user1 {
        println!("   结果: {:?}", user);
    }
    println!();

    // 第二次查询（缓存命中）
    println!("   第二次查询 'user:1':");
    let user1_cached = get_user_with_cache_bypass(&cache, &db_loader, "user:1").await?;
    if let Some(user) = user1_cached {
        println!("   结果: {:?}", user);
    }
    println!();

    // 查询不存在的用户
    println!("   查询不存在的用户 'user:999':");
    let user_missing = get_user_with_cache_bypass(&cache, &db_loader, "user:999").await?;
    match user_missing {
        Some(user) => println!("   意外：找到了用户 {:?}", user),
        None => println!("   ✓ 正确返回：用户不存在"),
    }
    println!();

    // 6. 批量加载示例
    println!("6. 批量加载示例");

    async fn batch_load_users_with_cache(
        cache: &Cache<String, User>,
        db_loader: &MockDbLoader,
        user_ids: &[&str],
    ) -> Result<Vec<Option<User>>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();

        for user_id in user_ids {
            println!("   处理用户 '{}':", user_id);

            // 尝试从缓存获取
            if let Some(user) = cache.get(&user_id.to_string()).await? {
                println!("     ✓ 缓存命中");
                results.push(Some(user));
                continue;
            }

            // 缓存未命中，从数据库加载
            match db_loader.load_user_from_db(user_id).await {
                Ok(Some(user)) => {
                    println!("     ✓ 数据库加载成功");

                    // 存入缓存
                    cache.set(&user_id.to_string(), &user).await?;
                    println!("     ✓ 已缓存");

                    results.push(Some(user));
                }
                Ok(None) => {
                    println!("     × 数据库中不存在");
                    results.push(None);
                }
                Err(e) => {
                    eprintln!("     × 加载错误: {}", e);
                    results.push(None);
                }
            }
        }

        Ok(results)
    }

    let user_ids = vec!["user:1", "user:2", "user:3", "user:4"];
    let batch_results = batch_load_users_with_cache(&cache, &db_loader, &user_ids).await?;

    println!("   批量加载结果:");
    for (i, result) in batch_results.iter().enumerate() {
        match result {
            Some(user) => println!("     {}: {}", user_ids[i], user.name),
            None => println!("     {}: 未找到", user_ids[i]),
        }
    }
    println!();

    // 7. 缓存更新和失效
    println!("7. 缓存更新和失效");

    // 模拟用户信息更新
    let updated_user = User {
        id: 1,
        name: "Alice Updated".to_string(),
        email: "alice.updated@example.com".to_string(),
    };

    println!("   更新用户信息到缓存...");
    cache.set(&"user:1".to_string(), &updated_user).await?;
    println!("   ✓ 用户信息已更新");

    // 验证更新
    if let Some(cached_user) = cache.get(&"user:1".to_string()).await? {
        println!("   验证更新结果: {:?}", cached_user.name);
    }
    println!();

    // 8. 缓存失效示例
    println!("8. 缓存失效示例");

    println!("   使 'user:2' 缓存失效...");
    cache.delete(&"user:2".to_string()).await?;
    println!("   ✓ 缓存已失效");

    // 验证失效 - 应该重新从数据库加载
    println!("   验证缓存失效...");
    let user2_after_invalidation = get_user_with_cache_bypass(&cache, &db_loader, "user:2").await?;
    if let Some(user) = user2_after_invalidation {
        println!("   重新加载成功: {:?}", user.name);
    }
    println!();

    // 9. 统计信息
    println!("9. 缓存统计信息");
    match cache.stats().await {
        Ok(stats) => {
            println!("   缓存类型: {}", stats.get("type").unwrap_or(&"N/A".to_string()));
            println!("   条目数: {}", stats.get("entry_count").unwrap_or(&"N/A".to_string()));
            println!("   容量: {}", stats.get("capacity").unwrap_or(&"N/A".to_string()));
        }
        Err(e) => println!("   获取统计信息失败: {}", e),
    }
    println!();

    // 10. 模拟分区策略
    println!("10. 模拟分区策略");

    // 简单的哈希分区策略
    fn simple_partition(key: &str, num_partitions: usize) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let hash_value = hasher.finish();
        (hash_value as usize) % num_partitions
    }

    let partitions = 4;
    let sample_keys = vec!["user:1", "user:2", "user:3", "user:4", "user:5", "user:6"];

    println!("   分区数量: {}", partitions);
    println!("   分区分配情况:");

    for key in &sample_keys {
        let partition = simple_partition(key, partitions);
        println!("     {} -> 分区 {}", key, partition);
    }
    println!();

    println!("=== 数据库集成示例完成 ===");
    println!("   数据库集成的主要优势:");
    println!("   - 缓存旁路：自动处理缓存未命中");
    println!("   - 减少数据库负载：提高缓存命中率");
    println!("   - 数据一致性：及时更新和失效");
    println!("   - 分区策略：支持大规模数据分布");
    println!("   - 连接管理：统一的连接字符串处理");

    Ok(())
}
