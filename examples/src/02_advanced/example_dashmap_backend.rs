// Copyright (c) 2025-2026 Kirky.X🌠
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
    println!("=== DashMap backend example ===\n");

    println!("--- 1. Create a DashMap backend ---");
    let dashmap = DashMapMemoryBackend::new();
    println!("  ✓ DashMap backend created");

    println!("\n--- 2. Basic operations ---");

    // 写入数据
    dashmap
        .set(
            "key1".into(),
            b"value1".to_vec().into(),
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("  write: key1 = value1");

    // 读取数据
    let value = dashmap.get("key1").await?;
    println!(
        "  read: key1 = {:?}",
        value.map(|v| String::from_utf8_lossy(&v).to_string())
    );

    // 检查存在
    let exists = dashmap.exists("key1").await?;
    println!("  exists: key1 = {}", exists);

    println!("\n--- 3. Batch operations ---");

    let keys_values: Vec<(&str, Vec<u8>)> = vec![
        ("item:1", b"apple".to_vec()),
        ("item:2", b"banana".to_vec()),
        ("item:3", b"cherry".to_vec()),
    ];

    for (key, value) in &keys_values {
        dashmap
            .set(
                (*key).into(),
                value.clone().into(),
                Some(Duration::from_secs(300)),
            )
            .await?;
    }
    println!("  batch write: {} key-value pairs", keys_values.len());

    let keys: Vec<String> = vec![
        "item:1".to_string(),
        "item:2".to_string(),
        "item:3".to_string(),
    ];
    let results = dashmap.get_many(&keys).await?;
    println!("  batch read: {} results", results.len());
    for (key, value) in keys.iter().zip(results.iter()) {
        println!(
            "    {} = {:?}",
            key,
            value
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string())
        );
    }

    println!("\n--- 4. Comparing Moka and DashMap ---");

    let moka = MokaMemoryBackend::builder().capacity(1000).build();
    let dashmap2 = DashMapMemoryBackend::new();

    // 写入相同数据
    for i in 0..10 {
        let key = format!("key:{}", i);
        let value = format!("value:{}", i).into_bytes();
        moka.set(key.as_str().into(), value.clone().into(), None)
            .await?;
        dashmap2
            .set(key.as_str().into(), value.into(), None)
            .await?;
    }

    let moka_count = moka.len().await?;
    let dashmap_count = dashmap2.len().await?;
    println!("  Moka backend: {} keys", moka_count);
    println!("  DashMap backend: {} keys", dashmap_count);

    println!("\n--- 5. Delete operations ---");

    dashmap.delete("key1").await?;
    println!("  delete: key1");

    let exists_after = dashmap.exists("key1").await?;
    println!("  exists check: key1 = {}", exists_after);

    println!("\n--- 6. Clear operations ---");

    let count_before = dashmap.len().await?;
    println!("  before clear: {} keys", count_before);

    dashmap.clear().await?;
    let count_after = dashmap.len().await?;
    println!("  after clear: {} keys", count_after);

    println!("\n--- 7. Feature comparison ---");
    println!("  Moka backend traits:");
    println!("    - LRU/TinyLFU eviction policies");
    println!("    - capacity limits");
    println!("    - automatic expiry");
    println!("    - suitable for production");
    println!();
    println!("  DashMap backend traits:");
    println!("    - plain concurrent HashMap");
    println!("    - no eviction policy");
    println!("    - manual TTL management");
    println!("    - suited to simple scenarios or tests");

    println!("\n✓ Example complete");
    Ok(())
}
