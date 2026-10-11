// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! ChainCache 链式缓存示例
//!
//! 本示例演示使用 ChainCache::builder() 创建多级缓存的方式。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_chain_cache
//! ```

use oxcache::Cache;
use oxcache::UnifiedCache;
use oxcache::backend::MokaMemoryBackend;
use oxcache::backend::RedisBackend;
use oxcache::cache::{ChainCache, ChainLink};
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
    println!("=== Chain cache example ===\n");

    println!("1. ChainCache::builder() creates a multi-tier cache");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    println!("  Connecting to Redis: {}", redis_url);

    match RedisBackend::new(&redis_url).await {
        Ok(l2) => {
            let l1 = MokaMemoryBackend::builder().capacity(1000).build();

            let chain = ChainCache::builder()
                .link(ChainLink::from_backend(l1))
                .link(ChainLink::from_backend(l2))
                .enable_backfill()
                .default_time_to_live(Duration::from_secs(3600))
                .build();

            println!("   ✓ created a 2-tier cache chain: L1(Moka) -> L2(Redis)\n");

            demo_chain_cache_operations(&chain).await?;

            chain.clear().await?;
        }
        Err(e) => {
            println!("   ✗ cannot connect to Redis: {}", e);
            println!("   skipping the Redis example, using an in-memory cache only\n");

            let cache: Cache<String, User> = Cache::builder().build().await?;
            demo_simple_cache_operations(&cache).await?;
        }
    }

    println!("\n2. In-memory-only cache example");
    let cache: Cache<String, User> = Cache::builder().build().await?;

    let user = User {
        id: 42,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    cache.set(&"user:42".to_string(), &user).await?;
    let cached = cache.get(&"user:42".to_string()).await?;
    println!("  store: {:?}", user);
    println!("  read: {:?}\n", cached);

    println!("3. Batch operations example");

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

    println!("  batch stored {} users", users.len());

    let keys = [
        "user:1".to_string(),
        "user:2".to_string(),
        "user:3".to_string(),
    ];
    let results = cache.get_many(keys.iter()).await?;
    println!("  batch read: {} users\n", results.len());

    println!("✓ Example complete");

    Ok(())
}

async fn demo_chain_cache_operations(
    chain: &ChainCache,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    println!("  test data: {:?}\n", user);

    println!("  writing data...");
    let user_bytes: Vec<u8> = serde_json::to_vec(&user)?;
    chain.set_bytes("user:1", user_bytes, None).await?;
    println!("   ✓ data written to the cache chain\n");

    println!("  reading data:");
    let cached = chain.get_bytes("user:1").await?;
    match cached {
        Some(data) => match serde_json::from_slice::<User>(&data) {
            Ok(u) => println!("   ✓ hit: {:?}\n", u),
            Err(_) => println!("   ✗ failed to parse the data\n"),
        },
        None => println!("   ✗ not found\n"),
    }

    let stats = chain.stats().await?;
    println!("  cache stats:");
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

    println!("  test data: {:?}\n", user);

    println!("  writing data...");
    cache.set(&"user:1".to_string(), &user).await?;
    println!("   ✓ data written to the cache\n");

    println!("  reading data:");
    let cached = cache.get(&"user:1".to_string()).await?;
    match cached {
        Some(u) => println!("   ✓ hit: {:?}\n", u),
        None => println!("   ✗ not found\n"),
    }

    let stats = cache.stats().await?;
    println!("  cache stats:");
    for (key, value) in &stats {
        println!("     {}: {}", key, value);
    }

    println!();

    Ok(())
}
