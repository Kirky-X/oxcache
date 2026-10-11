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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_CHAIN_CACHE_1_CHAINCACHE_BUILDER_CREATES, MSG_EXAMPLE_CHAIN_CACHE_2_IN_MEMORY_ONLY,
    MSG_EXAMPLE_CHAIN_CACHE_3_BATCH_OPERATIONS_EXAMPLE, MSG_EXAMPLE_CHAIN_CACHE_BATCH_READ_USERS,
    MSG_EXAMPLE_CHAIN_CACHE_BATCH_STORED_USERS, MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS,
    MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS_2, MSG_EXAMPLE_CHAIN_CACHE_CANNOT_CONNECT_TO_REDIS,
    MSG_EXAMPLE_CHAIN_CACHE_CONNECTING_TO_REDIS, MSG_EXAMPLE_CHAIN_CACHE_CREATED_A_2_TIER,
    MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE, MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE_2,
    MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE, MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_CHAIN_CACHE_FAILED_TO_PARSE_THE, MSG_EXAMPLE_CHAIN_CACHE_HIT,
    MSG_EXAMPLE_CHAIN_CACHE_HIT_2, MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND,
    MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND_2, MSG_EXAMPLE_CHAIN_CACHE_READ,
    MSG_EXAMPLE_CHAIN_CACHE_READING_DATA, MSG_EXAMPLE_CHAIN_CACHE_READING_DATA_2,
    MSG_EXAMPLE_CHAIN_CACHE_SKIPPING_THE_REDIS_EXAMPLE, MSG_EXAMPLE_CHAIN_CACHE_STORE,
    MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA, MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA_2,
    MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA, MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA_2, t,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}\n", t(MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE, &[]));

    println!(
        "{}",
        t(MSG_EXAMPLE_CHAIN_CACHE_1_CHAINCACHE_BUILDER_CREATES, &[])
    );

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CHAIN_CACHE_CONNECTING_TO_REDIS,
            &[("redis_url", redis_url.to_string())]
        )
    );

    match RedisBackend::new(&redis_url).await {
        Ok(l2) => {
            let l1 = MokaMemoryBackend::builder().capacity(1000).build();

            let chain = ChainCache::builder()
                .link(ChainLink::from_backend(l1))
                .link(ChainLink::from_backend(l2))
                .enable_backfill()
                .default_time_to_live(Duration::from_secs(3600))
                .build();

            println!("   {}\n", t(MSG_EXAMPLE_CHAIN_CACHE_CREATED_A_2_TIER, &[]));

            demo_chain_cache_operations(&chain).await?;

            chain.clear().await?;
        }
        Err(e) => {
            println!(
                "   {}",
                t(
                    MSG_EXAMPLE_CHAIN_CACHE_CANNOT_CONNECT_TO_REDIS,
                    &[("e", e.to_string())]
                )
            );
            println!(
                "   {}\n",
                t(MSG_EXAMPLE_CHAIN_CACHE_SKIPPING_THE_REDIS_EXAMPLE, &[])
            );

            let cache: Cache<String, User> = Cache::builder().build().await?;
            demo_simple_cache_operations(&cache).await?;
        }
    }

    println!("\n{}", t(MSG_EXAMPLE_CHAIN_CACHE_2_IN_MEMORY_ONLY, &[]));
    let cache: Cache<String, User> = Cache::builder().build().await?;

    let user = User {
        id: 42,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };

    cache.set(&"user:42".to_string(), &user).await?;
    let cached = cache.get(&"user:42".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CHAIN_CACHE_STORE,
            &[("user", format!("{:?}", user))]
        )
    );
    println!(
        "  {}\n",
        t(
            MSG_EXAMPLE_CHAIN_CACHE_READ,
            &[("cached", format!("{:?}", cached))]
        )
    );

    println!(
        "{}",
        t(MSG_EXAMPLE_CHAIN_CACHE_3_BATCH_OPERATIONS_EXAMPLE, &[])
    );

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

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CHAIN_CACHE_BATCH_STORED_USERS,
            &[("users", users.len().to_string())]
        )
    );

    let keys = [
        "user:1".to_string(),
        "user:2".to_string(),
        "user:3".to_string(),
    ];
    let results = cache.get_many(keys.iter()).await?;
    println!(
        "  {}\n",
        t(
            MSG_EXAMPLE_CHAIN_CACHE_BATCH_READ_USERS,
            &[("results", results.len().to_string())]
        )
    );

    println!("{}", t(MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE_COMPLETE, &[]));

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

    println!(
        "  {}\n",
        t(
            MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA,
            &[("user", format!("{:?}", user))]
        )
    );

    println!("  {}", t(MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA, &[]));
    let user_bytes: Vec<u8> = serde_json::to_vec(&user)?;
    chain.set_bytes("user:1", user_bytes, None).await?;
    println!(
        "   {}\n",
        t(MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE, &[])
    );

    println!("  {}", t(MSG_EXAMPLE_CHAIN_CACHE_READING_DATA, &[]));
    let cached = chain.get_bytes("user:1").await?;
    match cached {
        Some(data) => match serde_json::from_slice::<User>(&data) {
            Ok(u) => println!(
                "   {}\n",
                t(MSG_EXAMPLE_CHAIN_CACHE_HIT, &[("u", format!("{:?}", u))])
            ),
            Err(_) => println!(
                "   {}\n",
                t(MSG_EXAMPLE_CHAIN_CACHE_FAILED_TO_PARSE_THE, &[])
            ),
        },
        None => println!("   {}\n", t(MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND, &[])),
    }

    let stats = chain.stats().await?;
    println!("  {}", t(MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS, &[]));
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

    println!(
        "  {}\n",
        t(
            MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA_2,
            &[("user", format!("{:?}", user))]
        )
    );

    println!("  {}", t(MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA_2, &[]));
    cache.set(&"user:1".to_string(), &user).await?;
    println!(
        "   {}\n",
        t(MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE_2, &[])
    );

    println!("  {}", t(MSG_EXAMPLE_CHAIN_CACHE_READING_DATA_2, &[]));
    let cached = cache.get(&"user:1".to_string()).await?;
    match cached {
        Some(u) => println!(
            "   {}\n",
            t(MSG_EXAMPLE_CHAIN_CACHE_HIT_2, &[("u", format!("{:?}", u))])
        ),
        None => println!("   {}\n", t(MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND_2, &[])),
    }

    let stats = cache.stats().await?;
    println!("  {}", t(MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS_2, &[]));
    for (key, value) in &stats {
        println!("     {}: {}", key, value);
    }

    println!();

    Ok(())
}
