// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
//! ChainCache 链式缓存示例
//!
//! 本示例演示如何使用 ChainCache 实现多级缓存策略：
//! - L1 内存缓存（快速访问）
//! - L2 Redis 缓存（持久化）
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_chain_cache
//! ```

use oxcache::backend::client::MokaMemoryBackend;
#[cfg(feature = "redis")]
use oxcache::backend::client::RedisBackend;
use oxcache::builder::OxCacheBuilder;
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== ChainCache 链式缓存示例 ===\n");

    // 示例 1: 使用 OxCacheBuilder 创建多级缓存（推荐方式）
    println!("1. 创建多级缓存（内存 + Redis）");

    #[cfg(feature = "redis")]
    {
        // 创建 L1 内存缓存
        let l1 = MokaMemoryBackend::builder().capacity(1000).build();

        // 创建 L2 Redis 缓存（需要 Redis 服务运行）
        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        println!("   连接 Redis: {}", redis_url);

        match RedisBackend::new(&redis_url).await {
            Ok(l2) => {
                // 使用 OxCacheBuilder 构建链式缓存
                let cache: Cache<String, User> = OxCacheBuilder::new()
                    .backend(l1)
                    .backend(l2)
                    .default_ttl(Duration::from_secs(3600))
                    .enable_backfill() // 启用回填：从 L2 读取时自动回填到 L1
                    .build()?
                    .into();

                println!("   ✓ 创建了2级缓存链：L1(Moka) -> L2(Redis)\n");

                // 演示数据操作
                demo_chain_cache_operations(&cache).await?;

                // 清理
                cache.clear().await?;
            }
            Err(e) => {
                println!("   ✗ 无法连接到 Redis: {}", e);
                println!("   跳过 Redis 示例，使用仅内存缓存\n");

                // 使用仅内存缓存
                let cache: Cache<String, User> = Cache::builder().build().await?;
                demo_chain_cache_operations(&cache).await?;
            }
        }
    }

    #[cfg(not(feature = "redis"))]
    {
        println!("   Redis feature 未启用，使用仅内存缓存\n");
        let cache: Cache<String, User> = Cache::builder().build().await?;
        demo_chain_cache_operations(&cache).await?;
    }

    // 示例 2: 仅内存缓存（适用于简单场景）
    println!("\n2. 仅内存缓存示例");
    let memory_cache: Cache<String, User> = OxCacheBuilder::memory(500).build()?.into();

    let user = User {
        id: 42,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    memory_cache.set(&"user:42".to_string(), &user).await?;
    let cached = memory_cache.get(&"user:42".to_string()).await?;
    println!("   存储: {:?}", user);
    println!("   读取: {:?}\n", cached);

    // 示例 3: 批量操作演示（使用 Pipeline 优化）
    println!("3. 批量操作示例");

    let users = vec![
        User { id: 1, name: "Alice".to_string(), email: "alice@example.com".to_string() },
        User { id: 2, name: "Charlie".to_string(), email: "charlie@example.com".to_string() },
        User { id: 3, name: "Diana".to_string(), email: "diana@example.com".to_string() },
    ];

    // 批量设置
    let items: Vec<(&String, &User)> = users.iter().map(|u| {
        let key = format!("user:{}", u.id);
        Box::leak(Box::new(key)) as &String
    }).zip(users.iter()).collect();

    // 简化版本
    memory_cache.set(&"user:1".to_string(), &users[0]).await?;
    memory_cache.set(&"user:2".to_string(), &users[1]).await?;
    memory_cache.set(&"user:3".to_string(), &users[2]).await?;

    println!("   批量存储了 {} 个用户", users.len());

    // 批量读取
    let keys = vec!["user:1".to_string(), "user:2".to_string(), "user:3".to_string()];
    let results = memory_cache.get_many(keys.iter()).await?;
    println!("   批量读取: {} 个用户\n", results.len());

    println!("✓ 示例完成");

    Ok(())
}

/// 演示链式缓存的基本操作
async fn demo_chain_cache_operations(cache: &Cache<String, User>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 创建测试数据
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    println!("   测试数据: {:?}\n", user);

    // 写入数据
    println!("   写入数据...");
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   ✓ 数据已写入缓存链\n");

    // 第一次读取（会从 L1 读取）
    println!("   第一次读取（从 L1 内存缓存）：");
    let cached = cache.get(&"user:1".to_string()).await?;
    match cached {
        Some(u) => println!("   ✓ 命中: {:?}\n", u),
        None => println!("   ✗ 未找到\n"),
    }

    // 演示缓存穿透和回填
    println!("   演示缓存穿透和回填：");
    println!("   - 当数据不在 L1 但在 L2 时");
    println!("   - 启用 backfill 后，会自动回填到 L1\n");

    // 统计信息
    let stats = cache.stats().await?;
    println!("   缓存统计:");
    for (key, value) in &stats {
        println!("     {}: {}", key, value);
    }

    println!();

    Ok(())
}
