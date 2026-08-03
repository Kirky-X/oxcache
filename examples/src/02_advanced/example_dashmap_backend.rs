// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DashMap 后端示例
//!
//! 本示例演示 DashMapMemoryBackend 的使用，
//! 并对比 Moka 和 DashMap 后端的差异。

use oxcache::backend::{CacheReader, CacheWriter};
use oxcache::backend::{DashMapMemoryBackend, MokaMemoryBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DashMap 后端示例 ===\n");

    // 1. 创建 DashMap 后端
    println!("--- 1. 创建 DashMap 后端 ---");
    let dashmap = DashMapMemoryBackend::new();
    println!("  ✓ 创建 DashMap 后端");

    // 2. 基本操作
    println!("\n--- 2. 基本操作 ---");

    // 写入数据
    dashmap
        .set("key1".into(), b"value1".to_vec().into(), Some(Duration::from_secs(60)))
        .await?;
    println!("  写入: key1 = value1");

    // 读取数据
    let value = dashmap.get("key1").await?;
    println!(
        "  读取: key1 = {:?}",
        value.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    // 检查存在
    let exists = dashmap.exists("key1").await?;
    println!("  存在: key1 = {}", exists);

    // 3. 批量操作
    println!("\n--- 3. 批量操作 ---");

    let keys_values: Vec<(&str, Vec<u8>)> = vec![
        ("item:1", b"apple".to_vec()),
        ("item:2", b"banana".to_vec()),
        ("item:3", b"cherry".to_vec()),
    ];

    for (key, value) in &keys_values {
        dashmap
            .set((*key).into(), value.clone().into(), Some(Duration::from_secs(300)))
            .await?;
    }
    println!("  批量写入: {} 个键值对", keys_values.len());

    let keys: Vec<String> = vec!["item:1".to_string(), "item:2".to_string(), "item:3".to_string()];
    let results = dashmap.get_many(&keys).await?;
    println!("  批量读取: {} 个结果", results.len());
    for (key, value) in keys.iter().zip(results.iter()) {
        println!(
            "    {} = {:?}",
            key,
            value.as_ref().map(|v| String::from_utf8_lossy(v).to_string())
        );
    }

    // 4. 对比 Moka 和 DashMap
    println!("\n--- 4. 对比 Moka 和 DashMap ---");

    let moka = MokaMemoryBackend::builder().capacity(1000).build();
    let dashmap2 = DashMapMemoryBackend::new();

    // 写入相同数据
    for i in 0..10 {
        let key = format!("key:{}", i);
        let value = format!("value:{}", i).into_bytes();
        moka.set(key.as_str().into(), value.clone().into(), None).await?;
        dashmap2.set(key.as_str().into(), value.into(), None).await?;
    }

    let moka_count = moka.len().await?;
    let dashmap_count = dashmap2.len().await?;
    println!("  Moka 后端: {} 个键", moka_count);
    println!("  DashMap 后端: {} 个键", dashmap_count);

    // 5. 删除操作
    println!("\n--- 5. 删除操作 ---");

    dashmap.delete("key1").await?;
    println!("  删除: key1");

    let exists_after = dashmap.exists("key1").await?;
    println!("  存在检查: key1 = {}", exists_after);

    // 6. 清空操作
    println!("\n--- 6. 清空操作 ---");

    let count_before = dashmap.len().await?;
    println!("  清空前: {} 个键", count_before);

    dashmap.clear().await?;
    let count_after = dashmap.len().await?;
    println!("  清空后: {} 个键", count_after);

    // 7. 特性对比
    println!("\n--- 7. 特性对比 ---");
    println!("  Moka 后端特点:");
    println!("    - 支持 LRU/TinyLFU 淘汰策略");
    println!("    - 支持容量限制");
    println!("    - 支持自动过期");
    println!("    - 适合生产环境");
    println!();
    println!("  DashMap 后端特点:");
    println!("    - 纯并发 HashMap");
    println!("    - 无淘汰策略");
    println!("    - 手动 TTL 管理");
    println!("    - 适合简单场景或测试");

    println!("\n✓ 示例完成");
    Ok(())
}
