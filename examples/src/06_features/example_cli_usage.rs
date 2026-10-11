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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_CLI_USAGE_ATTEMPTING_TO_RETRIEVE_5, MSG_EXAMPLE_CLI_USAGE_CACHE_CLEARED,
    MSG_EXAMPLE_CLI_USAGE_CACHE_CREATED_DEMO_CACHE, MSG_EXAMPLE_CLI_USAGE_CACHE_NAME_DEMO_CACHE,
    MSG_EXAMPLE_CLI_USAGE_CACHE_TYPE_L1_IN, MSG_EXAMPLE_CLI_USAGE_CHECK_METRICS_OXCACHE_METRICS,
    MSG_EXAMPLE_CLI_USAGE_CHECK_STATUS_OXCACHE_STATUS,
    MSG_EXAMPLE_CLI_USAGE_CLEAN_CACHE_OXCACHE_ADMIN, MSG_EXAMPLE_CLI_USAGE_CLEANING_UP,
    MSG_EXAMPLE_CLI_USAGE_CLI_COMMAND_FORMAT, MSG_EXAMPLE_CLI_USAGE_CURRENT_ENTRIES,
    MSG_EXAMPLE_CLI_USAGE_EXAMPLE_COMPLETE, MSG_EXAMPLE_CLI_USAGE_MAX_CAPACITY_1000,
    MSG_EXAMPLE_CLI_USAGE_OXCACHE_CLI_USAGE_EXAMPLE,
    MSG_EXAMPLE_CLI_USAGE_PERFORMING_CACHE_OPERATIONS,
    MSG_EXAMPLE_CLI_USAGE_RETRIEVED_5_CACHE_ENTRIES, MSG_EXAMPLE_CLI_USAGE_SET_10_CACHE_ENTRIES,
    MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_METRICS_OUTPUT,
    MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_STATUS_OUTPUT, MSG_EXAMPLE_CLI_USAGE_STATUS_RUNNING,
    MSG_EXAMPLE_CLI_USAGE_WARM_UP_CACHE_OXCACHE, t,
};

/// 演示 CLI 工具的基本功能
///
/// 注意：实际的 CLI 工具通过 `oxcache` 命令行程序使用，
/// 本示例展示如何在代码中获取类似的缓存状态和指标信息。
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!(
        "{}\n",
        t(MSG_EXAMPLE_CLI_USAGE_OXCACHE_CLI_USAGE_EXAMPLE, &[])
    );

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(1000).build().await?;

    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_CACHE_CREATED_DEMO_CACHE, &[]));

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CLI_USAGE_PERFORMING_CACHE_OPERATIONS, &[])
    );

    // 设置一些值
    for i in 0..10 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        cache.set(&key, &value).await?;
    }
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_SET_10_CACHE_ENTRIES, &[]));

    // 获取一些值（命中）
    for i in 0..5 {
        let key = format!("key_{}", i);
        let _ = cache.get(&key).await?;
    }
    println!(
        "{}",
        t(MSG_EXAMPLE_CLI_USAGE_RETRIEVED_5_CACHE_ENTRIES, &[])
    );

    // 尝试获取不存在的值（未命中）
    for i in 10..15 {
        let key = format!("key_{}", i);
        let _ = cache.get(&key).await?;
    }
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_ATTEMPTING_TO_RETRIEVE_5, &[]));

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_STATUS_OUTPUT, &[])
    );
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_CACHE_NAME_DEMO_CACHE, &[]));
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_CACHE_TYPE_L1_IN, &[]));
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_MAX_CAPACITY_1000, &[]));
    println!(
        "{}",
        t(
            MSG_EXAMPLE_CLI_USAGE_CURRENT_ENTRIES,
            &[("await", cache.len().await?.to_string())]
        )
    );
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_STATUS_RUNNING, &[]));

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_METRICS_OUTPUT, &[])
    );
    println!("# TYPE cache_entries counter");
    println!(
        "cache_entries{{cache=\"demo_cache\"}} {}",
        cache.len().await?
    );
    println!("# TYPE cache_capacity counter");
    println!("cache_capacity{{cache=\"demo_cache\"}} 1000");

    println!("\n{}", t(MSG_EXAMPLE_CLI_USAGE_CLI_COMMAND_FORMAT, &[]));
    println!(
        "{}",
        t(MSG_EXAMPLE_CLI_USAGE_CHECK_STATUS_OXCACHE_STATUS, &[])
    );
    println!(
        "{}",
        t(MSG_EXAMPLE_CLI_USAGE_CHECK_METRICS_OXCACHE_METRICS, &[])
    );
    println!(
        "{}",
        t(MSG_EXAMPLE_CLI_USAGE_CLEAN_CACHE_OXCACHE_ADMIN, &[])
    );
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_WARM_UP_CACHE_OXCACHE, &[]));

    println!("\n{}", t(MSG_EXAMPLE_CLI_USAGE_CLEANING_UP, &[]));
    cache.clear().await?;
    println!("{}", t(MSG_EXAMPLE_CLI_USAGE_CACHE_CLEARED, &[]));

    println!("\n{}", t(MSG_EXAMPLE_CLI_USAGE_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
