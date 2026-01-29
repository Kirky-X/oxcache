// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
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

use std::sync::Arc;
use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct AppConfig {
    key: String,
    value: String,
    description: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    username: String,
    role: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 缓存预热示例 ===\n");

    // 创建缓存
    let cache: Arc<Cache<String, String>> = Arc::new(Cache::builder().build().await?);

    // 1. 模拟从数据库加载配置
    println!("1. 模拟应用配置预热");
    let configs = vec![
        ("app:theme", "dark", "应用主题"),
        ("app:language", "zh-CN", "默认语言"),
        ("app:timezone", "Asia/Shanghai", "时区"),
        ("app:max_connections", "100", "最大连接数"),
        ("app:session_timeout", "3600", "会话超时时间"),
    ];

    println!("   从数据库加载配置...");
    for (key, value, desc) in &configs {
        let k = key.clone();
        let v = value.clone();
        cache.set(&k, &v).await?;
        println!("     加载配置: {} = {} ({})", k, v, desc);
    }
    println!("   ✓ 配置预热完成 ({} 个配置项)\n", configs.len());

    // 2. 模拟预加载热点用户数据
    println!("2. 模拟热点用户数据预热");
    let hot_users = vec![1, 2, 3, 4, 5, 10, 100, 101];

    println!("   预加载热点用户...");
    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    for user_id in &hot_users {
        let cache = cache.clone();
        let id = *user_id;
        let handle = tokio::spawn(async move {
            // 模拟从数据库查询用户
            let username = format!("user_{}", id);
            let role = if id == 1 { "admin" } else { "user" };
            cache.set(&format!("user:{}", id), &format!("{}:{}", username, role)).await?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    println!(
        "   ✓ 热点用户预热完成 ({} 个用户, 耗时: {:?})\n",
        hot_users.len(),
        elapsed
    );

    // 3. 验证预热数据
    println!("3. 验证预热数据");
    println!("   配置验证:");
    for (key, value, _) in &configs {
        let k = key.clone();
        let v = value.clone();
        let retrieved = cache.get(&k).await?;
        match retrieved {
            Some(val) if val == v => println!("     ✓ {} = {}", k, val),
            Some(val) => println!("     ✗ {} = {} (期望: {})", k, val, v),
            None => println!("     ✗ {} 未找到", k),
        }
    }

    println!("   \n   用户验证:");
    for user_id in &hot_users {
        let key = format!("user:{}", user_id);
        let retrieved = cache.get(&key).await?;
        match retrieved {
            Some(v) => println!("     ✓ {} = {}", key, v),
            None => println!("     ✗ {} 未找到", key),
        }
    }
    println!();

    // 4. 模拟缓存重建（故障恢复后）
    println!("4. 模拟缓存重建场景");
    println!("   清空缓存...");
    cache.clear().await?;

    println!("   重新预热...");
    let start = std::time::Instant::now();

    // 并发重新加载所有配置
    let mut handles = Vec::new();
    for (key, value, _) in &configs {
        let cache = cache.clone();
        let k = key.clone();
        let v = value.clone();
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
    println!("   ✓ 缓存重建完成，耗时: {:?}", elapsed);
    println!();

    // 5. 统计信息
    println!("5. 预热后统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.get("item_count").unwrap_or(&0));
    println!("   - 命中次数: {}", stats.get("hit_count").unwrap_or(&0));
    println!();

    println!("=== 缓存预热示例完成 ===");
    Ok(())
}