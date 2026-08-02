// Copyright (c) 2025-2026 Kirky.X
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
//! cd examples && cargo run --example example_redis_modes
//! ```

use oxcache::backend::{CacheReader, CacheWriter};
use oxcache::backend::{RedisBackend, RedisMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis 多模式连接示例 ===\n");

    // 1. Standalone 模式（最常用）
    println!("--- 1. Standalone 模式 ---");
    let standalone_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!("  连接: {}", standalone_url);

    let standalone = RedisBackend::new(&standalone_url).await?;
    println!("  ✓ 连接成功，模式: {}", standalone.mode());

    // 基本操作测试
    standalone.set("mode:standalone".into(), b"hello".to_vec().into(), None).await?;
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
                cluster.set("mode:cluster".into(), b"cluster_value".to_vec().into(), None).await?;
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
        println!("    启动 Cluster: cd tests/real_env && docker compose -f docker-compose.cluster.yml up -d");
    }

    // 4. Sentinel 模式（需要 Redis Sentinel 运行）
    println!("\n--- 4. Sentinel 模式 ---");
    let sentinel_available = std::env::var("REDIS_SENTINEL_AVAILABLE").is_ok();

    if sentinel_available {
        let sentinel_url = "redis://127.0.0.1:26382";
        println!("  连接 Sentinel: {}", sentinel_url);

        match RedisBackend::new(sentinel_url).await {
            Ok(sentinel) => {
                println!("  ✓ Sentinel 连接成功，模式: {}", sentinel.mode());

                // Sentinel 下的基本操作
                sentinel.set("mode:sentinel".into(), b"sentinel_value".to_vec().into(), None).await?;
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
        println!("    启动 Sentinel: cd tests/real_env && docker compose -f docker-compose.sentinel.yml up -d");
    }

    // 5. RedisModeType 枚举展示
    println!("\n--- 5. RedisModeType 枚举 ---");
    let modes = [RedisMode::Standalone, RedisMode::Sentinel, RedisMode::Cluster];
    for mode in &modes {
        println!("  模式: {} (Display: {})", mode, mode);
    }

    println!("\n✓ 示例完成");
    Ok(())
}
