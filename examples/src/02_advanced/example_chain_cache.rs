// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
//! ChainCache 链式缓存示例
//!
//! 本示例演示使用 ChainCache::builder() 创建多级缓存的方式。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_chain_cache
//! ```

use oxcache::backend::MokaMemoryBackend;
use oxcache::backend::RedisBackend;
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::Cache;
use oxcache::UnifiedCache;
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
    println!("=== 链式缓存示例 ===\n");

    // 示例 1: 使用 ChainCache::builder() 创建多级缓存
    println!("1. ChainCache::builder() 创建多级缓存");

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    println!("   连接 Redis: {}", redis_url);

    match RedisBackend::new(&redis_url).await {
        Ok(l2) => {
            let l1 = MokaMemoryBackend::builder().capacity(1000).build();

            let chain = ChainCache::builder()
                .link(ChainLink::from_backend(l1))
                .link(ChainLink::from_backend(l2))
                .enable_backfill()
                .default_time_to_live(Duration::from_secs(3600))
                .build();

            println!("   ✓ 创建了2级缓存链：L1(Moka) -> L2(Redis)\n");

            demo_chain_cache_operations(&chain).await?;

            chain.clear().await?;
        }
        Err(e) => {
            println!("   ✗ 无法连接到 Redis: {}", e);
            println!("   跳过 Redis 示例，使用仅内存缓存\n");

            let cache: Cache<String, User> = Cache::builder().build().await?;
            demo_simple_cache_operations(&cache).await?;
        }
    }

    // 示例 2: 仅内存缓存（适用于简单场景）
    println!("\n2. 仅内存缓存示例");
    let cache: Cache<String, User> = Cache::builder().build().await?;

    let user = User {
        id: 42,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    cache.set(&"user:42".to_string(), &user).await?;
    let cached = cache.get(&"user:42".to_string()).await?;
    println!("   存储: {:?}", user);
    println!("   读取: {:?}\n", cached);

    // 示例 3: 批量操作演示
    println!("3. 批量操作示例");

    let users = [
        User {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        },
        User {
            id: 2,
            name: "Charlie".to_string(),
            email: "charlie@example.com".to_string(),
        },
        User {
            id: 3,
            name: "Diana".to_string(),
            email: "diana@example.com".to_string(),
        },
    ];

    cache.set(&"user:1".to_string(), &users[0]).await?;
    cache.set(&"user:2".to_string(), &users[1]).await?;
    cache.set(&"user:3".to_string(), &users[2]).await?;

    println!("   批量存储了 {} 个用户", users.len());

    let keys = ["user:1".to_string(), "user:2".to_string(), "user:3".to_string()];
    let results = cache.get_many(keys.iter()).await?;
    println!("   批量读取: {} 个用户\n", results.len());

    println!("✓ 示例完成");

    Ok(())
}

#[allow(dead_code)]
async fn demo_chain_cache_operations(chain: &ChainCache) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    println!("   测试数据: {:?}\n", user);

    println!("   写入数据...");
    let user_bytes: Vec<u8> = serde_json::to_vec(&user)?;
    chain.set_bytes("user:1", user_bytes, None).await?;
    println!("   ✓ 数据已写入缓存链\n");

    println!("   读取数据：");
    let cached = chain.get_bytes("user:1").await?;
    match cached {
        Some(data) => match serde_json::from_slice::<User>(&data) {
            Ok(u) => println!("   ✓ 命中: {:?}\n", u),
            Err(_) => println!("   ✗ 数据解析失败\n"),
        },
        None => println!("   ✗ 未找到\n"),
    }

    let stats = chain.stats().await?;
    println!("   缓存统计:");
    for (key, value) in &stats {
        println!("     {}: {}", key, value);
    }

    println!();

    Ok(())
}

async fn demo_simple_cache_operations(
    cache: &Cache<String, User>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    println!("   测试数据: {:?}\n", user);

    println!("   写入数据...");
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   ✓ 数据已写入缓存\n");

    println!("   读取数据：");
    let cached = cache.get(&"user:1".to_string()).await?;
    match cached {
        Some(u) => println!("   ✓ 命中: {:?}\n", u),
        None => println!("   ✗ 未找到\n"),
    }

    let stats = cache.stats().await?;
    println!("   缓存统计:");
    for (key, value) in &stats {
        println!("     {}: {}", key, value);
    }

    println!();

    Ok(())
}
