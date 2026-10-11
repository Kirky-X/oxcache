// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! #[cached] 宏使用示例
//!
//! 本示例演示了 Oxcache 的 #[cached] 宏功能：
//! - 零样板缓存装饰
//! - 自动序列化/反序列化
//! - 灵活的键生成策略
//! - TTL 控制
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_cached_macro
//! ```

use oxcache::Cache;
use oxcache::macros::cached;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

// 使用 #[cached] 宏一行代码启用缓存
#[cached(service = "user_cache", ttl = 600)]
async fn get_user(id: u64) -> Result<User, String> {
    // 模拟耗时的数据库查询
    println!("  executing the original function (database query)...");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("User {}", id),
    })
}

// 使用自定义键格式的缓存函数
#[cached(service = "user_cache", ttl = 600, key_prefix = "user")]
async fn get_user_custom_key(id: u64) -> Result<User, String> {
    println!("  executing the original function (custom key)...");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("Custom Key User {}", id),
    })
}

// 使用较短 TTL 的缓存函数（演示不同的 TTL 配置）
#[cached(service = "user_cache", ttl = 300)]
async fn get_hot_data(id: u64) -> Result<String, String> {
    println!("  executing the original function (hot data)...");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(format!("Hot Data for {}", id))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "=== #[cached] macro usage example ===
"
    );

    // 初始化缓存
    // 注意：register_for_macro 仅支持 Cache<String, Vec<u8>>
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

    // 注册缓存实例到全局管理器（供宏使用）
    cache.register_for_macro("user_cache").await?;
    println!(
        "✓ cache registered for use by the #[cached] macro
"
    );

    println!("1. Basic caching demonstration");
    println!("  first call of get_user(1) - runs the function and caches the result...");
    let user = get_user(1).await?;
    println!("  first call result: {:?}", user);

    println!("  second call of get_user(1) - served directly from cache...");
    let cached_user = get_user(1).await?;
    println!("  second call result: {:?}", cached_user);
    println!();

    println!("2. Custom key format demonstration");
    println!("  calling get_user_custom_key(2) - uses the 'user_' key prefix...");
    let user_custom = get_user_custom_key(2).await?;
    println!("  result: {:?}", user_custom);

    println!("  calling get_user_custom_key(2) again - served from cache...");
    let cached_custom = get_user_custom_key(2).await?;
    println!("  cached result: {:?}", cached_custom);
    println!();

    println!("3. Short TTL caching demonstration");
    println!("  calling get_hot_data(1) - uses a short TTL (300s)...");
    let hot_data = get_hot_data(1).await?;
    println!("  result: {}", hot_data);

    println!("  calling get_hot_data(1) again - served from cache...");
    let cached_hot_data = get_hot_data(1).await?;
    println!("  cached result: {}", cached_hot_data);
    println!();

    println!("4. Performance comparison");
    println!("  cache hits are typically much faster than running the original function");

    // 首次调用（未缓存）
    let start = std::time::Instant::now();
    let _first_call = get_user(3).await?;
    let first_duration = start.elapsed();
    println!("  first call took: {:?}", first_duration);

    // 第二次调用（缓存命中）
    let start = std::time::Instant::now();
    let _second_call = get_user(3).await?;
    let second_duration = start.elapsed();
    println!("  cache hit took: {:?}", second_duration);

    println!(
        "  speedup: {:.2}x",
        first_duration.as_millis() as f64 / second_duration.as_millis() as f64
    );
    println!();

    #[cached(service = "user_cache", ttl = 600)]
    async fn get_user_with_role(user_id: u64, role: String) -> Result<String, String> {
        println!("  executing the original function (multi-argument)...");
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        Ok(format!("User {} with role {}", user_id, role))
    }

    println!("5. Multi-argument function caching");
    println!("  calling get_user_with_role(4, \"admin\")...");
    let result = get_user_with_role(4, "admin".to_string()).await?;
    println!("  result: {}", result);

    println!("  calling with the same arguments again - served from cache...");
    let cached_result = get_user_with_role(4, "admin".to_string()).await?;
    println!("  cached result: {}", cached_result);

    println!("  calling with different arguments - runs the original function...");
    let diff_result = get_user_with_role(4, "user".to_string()).await?;
    println!("  different-argument result: {}", diff_result);
    println!();

    println!("=== #[cached] macro example complete ===");
    println!("  main advantages of the #[cached] macro:");
    println!("  - zero boilerplate: caching logic handled automatically");
    println!("  - type safe: checked at compile time");
    println!("  - flexible configuration: supports multiple caching strategies");
    println!("  - automatic serialization: no manual handling needed");
    println!("  - performance: significantly improves cache hit performance");

    Ok(())
}
