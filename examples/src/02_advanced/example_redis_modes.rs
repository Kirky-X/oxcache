// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis 多模式连接示例
//!
//! 本示例演示 oxcache 支持的三种 Redis 连接模式：
//! - Standalone（单机模式）：最常用的单节点连接
//! - Sentinel（哨兵模式）：高可用自动故障转移
//! - Cluster（集群模式）：水平扩展分片存储
//!
//! 运行方式：
//! ```bash
//! # 本地开发需允许非 TLS 连接（生产请使用 rediss://）：
//! # workspace 根（默认 features 未启用 redis 时仅打印指引后正常退出）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --example example_redis_modes
//! # 完整演示（二选一）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --features redis --example example_redis_modes
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run -p oxcache-examples --example example_redis_modes
//! ```

// 本文件由主包与 oxcache-examples 包共享（单源双注册）：cfg(feature) 判定的
// 是编译方包自身的 features——主包默认不含 redis，降级为指引输出保证
// `cargo run --example` 可解析运行；examples 包 default 含 redis，走完整演示。
#[cfg(feature = "redis")]
use oxcache::backend::{CacheReader, CacheWriter};
#[cfg(feature = "redis")]
use oxcache::backend::{RedisBackend, RedisMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "redis"))]
    {
        println!("=== Redis 多模式连接示例 ===\n");
        println!("  当前编译未启用 redis feature，无演示内容。");
        println!("  完整运行：cargo run --features redis --example example_redis_modes");
        println!("       或：cargo run -p oxcache-examples --example example_redis_modes");
        return Ok(());
    }

    #[cfg(feature = "redis")]
    run().await
}

#[cfg(feature = "redis")]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis 多模式连接示例 ===\n");

    // 1. Standalone 模式（最常用）
    println!("--- 1. Standalone 模式 ---");
    let standalone_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!("  连接: {}", standalone_url);

    let standalone = match RedisBackend::new(&standalone_url).await {
        Ok(backend) => backend,
        Err(e) => {
            println!("  ✗ Standalone 连接失败: {e}");
            println!(
                "    需要运行中的 Redis（可用 REDIS_URL 覆盖，默认 redis://127.0.0.1:6379），跳过 Standalone/Builder 演示"
            );
            print_mode_enum();
            println!("\n✓ 示例完成（Standalone 因环境不可用而跳过）");
            return Ok(());
        }
    };
    println!("  ✓ 连接成功，模式: {}", standalone.mode());

    // 基本操作测试
    standalone
        .set("mode:standalone".into(), b"hello".to_vec().into(), None)
        .await?;
    let value = standalone.get("mode:standalone").await?;
    println!(
        "  写入/读取: {:?}",
        value.map(|v| String::from_utf8_lossy(&v).to_string())
    );
    standalone.delete("mode:standalone").await?;

    // 2. 使用 Builder 显式指定模式
    println!("\n--- 2. 使用 Builder 显式指定 Standalone 模式 ---");
    let backend = RedisBackend::builder()
        .connection_string(&standalone_url)
        .mode(RedisMode::Standalone)
        .build()
        .await?;
    println!("  ✓ Builder 创建成功，模式: {}", backend.mode());

    // 3. Cluster 模式（需要 Redis Cluster 运行）
    println!("\n--- 3. Cluster 模式 ---");
    let cluster_available = std::env::var("REDIS_CLUSTER_AVAILABLE").is_ok();

    if cluster_available {
        let cluster_url = "redis://127.0.0.1:7000";
        println!("  连接: {}", cluster_url);

        match RedisBackend::new(cluster_url).await {
            Ok(cluster) => {
                println!("  ✓ Cluster 连接成功，模式: {}", cluster.mode());

                // Cluster 下的基本操作
                cluster
                    .set(
                        "mode:cluster".into(),
                        b"cluster_value".to_vec().into(),
                        None,
                    )
                    .await?;
                let value = cluster.get("mode:cluster").await?;
                println!(
                    "  写入/读取: {:?}",
                    value.map(|v| String::from_utf8_lossy(&v).to_string())
                );
                cluster.delete("mode:cluster").await?;
            }
            Err(e) => {
                println!("  ✗ Cluster 连接失败: {}", e);
            }
        }
    } else {
        println!("  ⚠ REDIS_CLUSTER_AVAILABLE 未设置，跳过 Cluster 测试");
        println!(
            "    启动 Cluster: cd tests/real_env && docker compose -f docker-compose.cluster.yml up -d"
        );
    }

    // 4. Sentinel 模式（需要 Redis Sentinel 运行）
    println!("\n--- 4. Sentinel 模式 ---");
    let sentinel_available = std::env::var("REDIS_SENTINEL_AVAILABLE").is_ok();

    if sentinel_available {
        let sentinel_url = "redis://127.0.0.1:26382";
        println!("  连接 Sentinel: {}", sentinel_url);

        // Sentinel 必须用显式模式构建：RedisBackend::new 默认 Standalone，
        // 直连哨兵端口会把哨兵进程当数据节点（SET 被哨兵拒绝）。
        match RedisBackend::builder()
            .connection_string(sentinel_url)
            .mode(RedisMode::Sentinel)
            .build()
            .await
        {
            Ok(sentinel) => {
                println!("  ✓ Sentinel 连接成功，模式: {}", sentinel.mode());

                // Sentinel 下的基本操作
                sentinel
                    .set(
                        "mode:sentinel".into(),
                        b"sentinel_value".to_vec().into(),
                        None,
                    )
                    .await?;
                let value = sentinel.get("mode:sentinel").await?;
                println!(
                    "  写入/读取: {:?}",
                    value.map(|v| String::from_utf8_lossy(&v).to_string())
                );
                sentinel.delete("mode:sentinel").await?;
            }
            Err(e) => {
                println!("  ✗ Sentinel 连接失败: {}", e);
            }
        }
    } else {
        println!("  ⚠ REDIS_SENTINEL_AVAILABLE 未设置，跳过 Sentinel 测试");
        println!(
            "    启动 Sentinel: cd tests/real_env && docker compose -f docker-compose.sentinel.yml up -d"
        );
    }

    // 5. RedisModeType 枚举展示
    print_mode_enum();

    println!("\n✓ 示例完成");
    Ok(())
}

/// 枚举展示段（Standalone 连接失败时作为独立降级出口复用）。
#[cfg(feature = "redis")]
fn print_mode_enum() {
    println!("\n--- 5. RedisModeType 枚举 ---");
    let modes = [
        RedisMode::Standalone,
        RedisMode::Sentinel,
        RedisMode::Cluster,
    ];
    for mode in &modes {
        println!("  模式: {} (Display: {})", mode, mode);
    }
}
