// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! CLI 工具使用示例
//!
//! 本示例演示如何使用 oxcache 的命令行工具进行缓存管理。
//!
//! # 使用方法
//!
//! ```bash
//! # 查看缓存状态
//! oxcache status --verbose
//!
//! # 查看特定服务的缓存状态
//! oxcache status --service my-service
//!
//! # 获取缓存指标
//! oxcache metrics --prometheus
//!
//! # 管理操作
//! oxcache admin clean --cache my-cache
//! oxcache admin warmup --file warmup.json
//! ```

use oxcache::Cache;

/// 演示 CLI 工具的基本功能
///
/// 注意：实际的 CLI 工具通过 `oxcache` 命令行程序使用，
/// 本示例展示如何在代码中获取类似的缓存状态和指标信息。
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== oxcache CLI usage example ===\n");

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(1000).build().await?;

    println!("Cache created: demo_cache");

    println!("\n--- Performing cache operations ---");

    // 设置一些值
    for i in 0..10 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        cache.set(&key, &value).await?;
    }
    println!("Set 10 cache entries");

    // 获取一些值（命中）
    for i in 0..5 {
        let key = format!("key_{}", i);
        let _ = cache.get(&key).await?;
    }
    println!("Retrieved 5 cache entries (hits)");

    // 尝试获取不存在的值（未命中）
    for i in 10..15 {
        let key = format!("key_{}", i);
        let _ = cache.get(&key).await?;
    }
    println!("Attempting to retrieve 5 nonexistent entries (misses)");

    println!("\n--- Simulated `oxcache status` output ---");
    println!("Cache name: demo_cache");
    println!("Cache type: L1 in-memory cache");
    println!("Max capacity: 1000");
    println!("Current entries: {}", cache.len().await?);
    println!("Status: running");

    println!("\n--- Simulated `oxcache metrics` output ---");
    println!("# TYPE cache_entries counter");
    println!(
        "cache_entries{{cache=\"demo_cache\"}} {}",
        cache.len().await?
    );
    println!("# TYPE cache_capacity counter");
    println!("cache_capacity{{cache=\"demo_cache\"}} 1000");

    println!("\n--- CLI command format ---");
    println!("Check status: oxcache status --verbose");
    println!("Check metrics: oxcache metrics --prometheus");
    println!("Clean cache: oxcache admin clean --cache demo_cache");
    println!("Warm up cache: oxcache admin warmup --file warmup_data.json");

    println!("\n--- Cleaning up ---");
    cache.clear().await?;
    println!("Cache cleared");

    println!("\nExample complete!");
    Ok(())
}
