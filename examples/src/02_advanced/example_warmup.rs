// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存预热示例
//!
//! 本示例演示了 Oxcache 的缓存预热功能：
//! - 应用启动时预热缓存
//! - 预加载热点数据
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_warmup
//! ```

use oxcache::Cache;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Cache warmup example ===\n");

    // 创建缓存
    let cache: Arc<Cache<String, String>> = Arc::new(Cache::builder().build().await?);

    println!("1. Simulating application config warmup");
    let configs = vec![
        ("app:theme", "dark", "app theme"),
        ("app:language", "zh-CN", "default language"),
        ("app:timezone", "Asia/Shanghai", "time zone"),
        ("app:max_connections", "100", "max connections"),
        ("app:session_timeout", "3600", "session timeout"),
    ];

    println!("  loading config from the database...");
    for (key, value, desc) in &configs {
        let k = key.to_string();
        let v = value.to_string();
        cache.set(&k, &v).await?;
        println!("    loading config: {} = {} ({})", k, v, desc);
    }
    println!("   ✓ config warmup complete ({} items)\n", configs.len());

    println!("2. Simulating hot-user data warmup");
    let hot_users = vec![1, 2, 3, 4, 5, 10, 100, 101];

    println!("  preloading hot users...");
    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    for user_id in &hot_users {
        let cache = cache.clone();
        let id = *user_id;
        let handle = tokio::spawn(async move {
            // 模拟从数据库查询用户
            let username = format!("user_{}", id);
            let role = if id == 1 { "admin" } else { "user" };
            cache
                .set(&format!("user:{}", id), &format!("{}:{}", username, role))
                .await?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!(
        "   ✓ hot-user warmup complete ({} users, took: {:?})\n",
        hot_users.len(),
        elapsed
    );

    println!("3. Verifying the warmed data");
    println!("  Config verification:");
    for (key, value, _) in &configs {
        let k = key.to_string();
        let v = value.to_string();
        let retrieved = cache.get(&k).await?;
        match retrieved {
            Some(val) if val == v => println!("     ✓ {} = {}", k, val),
            Some(val) => println!("     ✗ {} = {} (expected: {})", k, val, v),
            None => println!("     ✗ {} not found", k),
        }
    }

    println!("   \n  User verification:");
    for user_id in &hot_users {
        let key = format!("user:{}", user_id);
        let retrieved = cache.get(&key).await?;
        match retrieved {
            Some(v) => println!("     ✓ {} = {}", key, v),
            None => println!("     ✗ {} not found", key),
        }
    }
    println!();

    println!("4. Simulating a cache rebuild");
    println!("  clearing the cache...");
    cache.clear().await?;

    println!("  rewarming...");
    let start = std::time::Instant::now();

    // 并发重新加载所有配置
    let mut handles = Vec::new();
    for (key, value, _) in &configs {
        let cache = cache.clone();
        let k = key.to_string();
        let v = value.to_string();
        let handle = tokio::spawn(async move {
            cache.set(&k, &v).await?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!("   ✓ cache rebuild complete, took: {:?}", elapsed);
    println!();

    println!("5. Post-warmup statistics");
    let stats = cache.stats().await?;
    println!(
        "  - total entries: {}",
        stats.get("item_count").map(String::as_str).unwrap_or("0")
    );
    println!(
        "  - hits: {}",
        stats.get("hit_count").map(String::as_str).unwrap_or("0")
    );
    println!();

    println!("=== Cache warmup example complete ===");
    Ok(())
}
