// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
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

    println!("=== oxcache CLI 使用示例 ===\n");

    // 1. 创建缓存实例
    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(1000).build().await?;

    println!("缓存已创建: demo_cache");

    // 2. 执行一些缓存操作
    println!("\n--- 执行缓存操作 ---");

    // 设置一些值
    for i in 0..10 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        cache.set(&key, &value).await?;
    }
    println!("已设置 10 个缓存条目");

    // 获取一些值（命中）
    for i in 0..5 {
        let key = format!("key_{}", i);
        let _ = cache.get(&key).await?;
    }
    println!("已获取 5 个缓存条目（命中）");

    // 尝试获取不存在的值（未命中）
    for i in 10..15 {
        let key = format!("key_{}", i);
        let _ = cache.get(&key).await?;
    }
    println!("尝试获取 5 个不存在的条目（未命中）");

    // 3. 模拟 CLI status 命令输出
    println!("\n--- 模拟 `oxcache status` 输出 ---");
    println!("缓存名称: demo_cache");
    println!("缓存类型: L1 内存缓存");
    println!("最大容量: 1000");
    println!("当前条目数: {}", cache.len().await?);
    println!("状态: 运行中");

    // 4. 模拟 CLI metrics 命令输出
    println!("\n--- 模拟 `oxcache metrics` 输出 ---");
    println!("# TYPE cache_entries counter");
    println!("cache_entries{{cache=\"demo_cache\"}} {}", cache.len().await?);
    println!("# TYPE cache_capacity counter");
    println!("cache_capacity{{cache=\"demo_cache\"}} 1000");

    // 5. 演示 CLI 命令格式
    println!("\n--- CLI 命令格式 ---");
    println!("查看状态: oxcache status --verbose");
    println!("查看指标: oxcache metrics --prometheus");
    println!("清理缓存: oxcache admin clean --cache demo_cache");
    println!("预热缓存: oxcache admin warmup --file warmup_data.json");

    // 6. 清理资源
    println!("\n--- 清理资源 ---");
    cache.clear().await?;
    println!("缓存已清理");

    println!("\n示例完成！");
    Ok(())
}
