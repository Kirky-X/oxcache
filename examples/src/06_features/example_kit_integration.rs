// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// trait-kit 集成示例
//
// 本示例演示 oxcache 的 trait-kit `kit` feature 集成：
// - OxcacheBuildObserver：构建观察者，监听模块构建事件
// - register_cache_shutdown：三阶段优雅关闭
// - register_cache_decorator：后端装饰器（类型注册，见下方限制说明）
//
// trait-kit 提供依赖注入和能力管理，oxcache 通过 OxcacheModule
// 注册为 kit 模块，获得 observer/shutdown/decorator 等生命周期管理。
//
// **已知限制**：trait-kit 0.4.1 的 `AsyncKit::decorate()` 存在 bug ——
// decorator 闭包被存储但在 `build()` 期间从未被调用。同步 `Kit::decorate()`
// 工作正常。`register_cache_decorator` 的 API 已就绪，待 trait-kit 修复后
// 即可生效。

use std::sync::Arc;

use oxcache::integrations::kit::{
    register_cache_decorator, register_cache_shutdown, OxcacheBuildObserver, OxcacheConfig,
    OxcacheModule,
};
use trait_kit::prelude::*;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === 1. 构建 AsyncKit + OxcacheModule ===
    println!("=== 1. 构建 AsyncKit + OxcacheModule ===");

    let mut kit = AsyncKit::new();
    kit.set_config(OxcacheConfig {
        capacity: 1000,
        ttl: Some(std::time::Duration::from_secs(300)),
        ..Default::default()
    });

    // 注册 observer：监听模块构建事件
    kit.with_observer(Arc::new(OxcacheBuildObserver));
    println!("已注册 OxcacheBuildObserver");

    // 注册 OxcacheModule
    kit.register::<OxcacheModule>()?;
    println!("已注册 OxcacheModule");

    // 注册装饰器（API 演示）
    // 注意：trait-kit 0.4.1 的 AsyncKit decorator 存在 bug，闭包不会在 build() 时执行。
    register_cache_decorator(&kit, |backend| {
        // 此闭包在 trait-kit 0.4.1 中不会被调用（AsyncKit bug）。
        // 同步 Kit::decorate() 工作正常。
        backend
    });
    println!("已注册装饰器（trait-kit 0.4.1: AsyncKit decorator 暂不生效）");

    // 构建 kit
    let built = kit.build().await?;
    println!("AsyncKit 构建完成\n");

    // === 2. 使用缓存后端 ===
    println!("=== 2. 使用缓存后端 ===");

    let backend = built.require::<OxcacheModule>()?;

    // 写入
    backend
        .set(
            Arc::from("user:1"),
            Arc::new(b"Alice".to_vec()),
            Some(std::time::Duration::from_secs(60)),
        )
        .await?;
    println!("set 'user:1' = 'Alice' (TTL=60s)");

    // 读取
    let value = backend.get("user:1").await?;
    println!(
        "get 'user:1' = {:?}",
        value.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    // 不存在的 key
    let miss = backend.get("user:999").await?;
    println!("get 'user:999' = {:?} (应为 None)", miss);
    println!();

    // === 3. 三阶段关闭 ===
    println!("=== 3. 三阶段关闭 ===");

    let shutdown_coord = AsyncShutdownCoordinator::new();
    register_cache_shutdown(&shutdown_coord, backend)?;
    println!("已注册三阶段关闭钩子:");
    println!("  - StopRequests: no-op");
    println!("  - DrainQueue: health_check 探活");
    println!("  - CloseConnections: backend.shutdown()");

    let result = shutdown_coord.shutdown().await;
    println!("关闭结果: {:?}\n", result);

    println!("=== 完成 ===");
    println!("trait-kit 集成演示结束。observer/shutdown 特性已展示。");
    println!("decorator API 已就绪，待 trait-kit 修复 AsyncKit bug 后生效。");

    Ok(())
}
