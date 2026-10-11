// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! get_or 单飞模式示例
//!
//! 本示例演示 Cache::get_or() 方法，该方法实现 single-flight 去重，
//! 防止缓存击穿（thundering herd problem）。
//!
//! 当多个并发请求同时查询同一个未命中的键时，只有第一个请求执行 fallback，
//! 其余请求等待结果，避免重复计算或数据库查询。

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct User {
    id: u64,
    name: String,
    email: String,
}

/// 模拟从数据库加载用户（耗时操作）
async fn load_user_from_db(id: u64) -> oxcache::OxCacheResult<User> {
    println!("  [DB] loading user {} from the database...", id);
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("User_{}", id),
        email: format!("user{}@example.com", id),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== get_or single-flight example ===\n");

    let cache: Arc<Cache<String, User>> = Arc::new(Cache::builder().capacity(100).build().await?);

    println!("--- 1. Basic usage ---");
    let key = "user:1".to_string();

    // 第一次调用：缓存未命中，执行 fallback
    println!("First get_or call:");
    let user1 = cache
        .get_or(&key, || async { load_user_from_db(1).await })
        .await?;
    println!("  result: {:?}\n", user1);

    // 第二次调用：缓存命中，不执行 fallback
    println!("Second get_or call:");
    let user2 = cache
        .get_or(&key, || async { load_user_from_db(1).await })
        .await?;
    println!("  result: {:?}\n", user2);

    println!("--- 2. Concurrent single-flight deduplication ---");
    let key2 = "user:2".to_string();

    // 同时发起 5 个并发请求查询同一个键
    let mut handles = Vec::new();
    for i in 0..5 {
        let cache_clone = Arc::clone(&cache);
        let key_clone = key2.clone();
        handles.push(tokio::spawn(async move {
            println!("  request {} started", i);
            let result = cache_clone
                .get_or(&key_clone, || async { load_user_from_db(2).await })
                .await;
            println!("  request {} completed", i);
            result
        }));
    }

    // 等待所有请求完成
    for handle in handles {
        let _ = handle.await?;
    }

    println!("\n  note: only one [DB] load runs; the other requests wait for the result");

    println!("\n--- 3. Comparison: concurrency without single-flight ---");
    let key3 = "user:3".to_string();

    // 先清除缓存
    cache.delete(&key3).await?;

    // 手动实现（无去重）：每个请求都执行 fallback
    let mut handles = Vec::new();
    for _i in 0..3 {
        let cache_clone = Arc::clone(&cache);
        let key_clone = key3.clone();
        handles.push(tokio::spawn(async move {
            // 先检查缓存
            if let Some(user) = cache_clone.get(&key_clone).await? {
                return Ok::<_, oxcache::OxCacheError>(user);
            }
            // 缓存未命中，执行 fallback
            let user = load_user_from_db(3).await?;
            cache_clone.set(&key_clone, &user).await?;
            Ok(user)
        }));
    }

    for handle in handles {
        let _ = handle.await?;
    }

    println!("  note: without single-flight, the [DB] load may run multiple times");

    println!("\n✓ Example complete");
    Ok(())
}
