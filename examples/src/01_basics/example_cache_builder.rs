// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! CacheBuilder 完整用法示例
//!
//! 本示例演示 CacheBuilder 的所有配置选项，
//! 包括 TTL、TTI、容量、自定义后端等。

use oxcache::backend::MokaMemoryBackend;
use oxcache::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Product {
    id: u64,
    name: String,
    price: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CacheBuilder 完整用法示例 ===\n");

    // 1. 基本构建
    println!("--- 1. 基本构建 ---");
    let _cache1: Cache<String, Product> = CacheBuilder::default().build().await?;
    println!("  ✓ 创建基本缓存");

    // 2. 设置容量
    println!("\n--- 2. 设置容量 ---");
    let _cache2: Cache<String, Product> = CacheBuilder::default().capacity(10000).build().await?;
    println!("  ✓ 创建容量为 10000 的缓存");

    // 3. 设置 TTL（Time To Live）
    println!("\n--- 3. 设置 TTL ---");
    let _cache3: Cache<String, Product> = CacheBuilder::default().ttl(Duration::from_secs(3600)).build().await?;
    println!("  ✓ 创建 TTL 为 1 小时的缓存");

    // 4. 设置 TTI（Time To Idle）
    println!("\n--- 4. 设置 TTI ---");
    let _cache4: Cache<String, Product> = CacheBuilder::default().tti(Duration::from_secs(300)).build().await?;
    println!("  ✓ 创建 TTI 为 5 分钟的缓存");

    // 5. 组合配置
    println!("\n--- 5. 组合配置 ---");
    let cache5: Cache<String, Product> = CacheBuilder::default()
        .capacity(5000)
        .ttl(Duration::from_secs(1800))
        .tti(Duration::from_secs(600))
        .build()
        .await?;
    println!("  ✓ 创建组合配置缓存（容量=5000, TTL=30min, TTI=10min）");

    // 6. 使用自定义后端
    println!("\n--- 6. 使用自定义后端 ---");
    let backend = MokaMemoryBackend::builder().capacity(2000).build();
    let _cache6: Cache<String, Product> = CacheBuilder::default()
        .backend_arc(std::sync::Arc::new(backend) as std::sync::Arc<dyn oxcache::backend::CacheBackend>)
        .ttl(Duration::from_secs(120))
        .build()
        .await?;
    println!("  ✓ 创建使用自定义 Moka 后端的缓存");

    // 7. 实际使用
    println!("\n--- 7. 实际使用 ---");
    let cache = cache5;

    let product = Product {
        id: 1,
        name: "Laptop".to_string(),
        price: 999.99,
    };

    cache.set(&"product:1".to_string(), &product).await?;
    println!("  写入: {:?}", product);

    let cached = cache.get(&"product:1".to_string()).await?;
    println!("  读取: {:?}", cached);

    // 8. 带 TTL 的设置
    println!("\n--- 8. 带 TTL 的设置 ---");
    cache
        .set_with_ttl(
            &"product:2".to_string(),
            &Product {
                id: 2,
                name: "Phone".to_string(),
                price: 599.99,
            },
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("  写入带 60s TTL 的产品");

    let exists = cache.exists(&"product:2".to_string()).await?;
    println!("  存在检查: {}", exists);

    // 9. 统计信息
    println!("\n--- 9. 统计信息 ---");
    let stats = cache.stats().await?;
    println!("  缓存统计:");
    for (key, value) in &stats {
        println!("    {}: {}", key, value);
    }

    // 10. 健康检查
    println!("\n--- 10. 健康检查 ---");
    cache.health_check().await?;
    println!("  健康状态: ✓ 正常");

    // 清理
    cache.clear().await?;

    println!("\n✓ 示例完成");
    Ok(())
}
