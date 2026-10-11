// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Bloom Filter 示例
//
// 本示例演示 oxcache 的 Bloom Filter 功能：
// - BloomFilter 独立类型：insert / contains / clear
// - BloomFilterBackend 装饰器：过滤负查询，减少 inner backend 访问
// - TTL 透传：BF 装饰器不破坏 inner 的 per-entry TTL
//
// Bloom Filter 适用于"负查询过滤"场景：当大量查询的 key 不存在时，
// BF 可以在 O(1) 时间内判断 key "绝对不存在"，避免穿透到 inner backend。

use std::time::Duration;

use oxcache::BackendScore;
use oxcache::backend::MokaMemoryBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use oxcache::features::{BloomFilter, BloomFilterBackend};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === BloomFilter 独立类型 ===
    println!("=== BloomFilter standalone type ===");
    let bf = BloomFilter::new(10_000, 0.01);
    println!("Creating BloomFilter (capacity=10000, fpr=0.01)");

    bf.insert("existing_key");
    println!("inserting 'existing_key'");

    println!(
        "contains 'existing_key' = {} (should be true)",
        bf.contains("existing_key")
    );
    println!(
        "contains 'missing_key'  = {} (should be false)",
        bf.contains("missing_key")
    );

    bf.clear();
    println!(
        "contains 'existing_key' after clear = {} (should be false)",
        bf.contains("existing_key")
    );

    // === BloomFilterBackend 装饰器 ===
    println!("\n=== BloomFilterBackend decorator ===");
    let inner = MokaMemoryBackend::new();
    println!(
        "inner backend: {} (score={})",
        inner.backend_name(),
        inner.score()
    );

    let backend = BloomFilterBackend::builder()
        .capacity(10_000)
        .false_positive_rate(0.01)
        .inner(inner)
        .build()?;
    println!("Creating BloomFilterBackend (wrapping Moka)");

    // set 更新 BF 和 inner
    backend
        .set("user:1".into(), b"Alice".to_vec().into(), None)
        .await?;
    println!("\nset 'user:1' = 'Alice'");

    // get 命中：BF 命中 → 查询 inner → 返回值
    let value = backend.get("user:1").await?;
    println!("get 'user:1' = {:?}", value);

    // get 未命中：BF 未命中 → 跳过 inner → 返回 None
    let value = backend.get("user:999").await?;
    println!(
        "get 'user:999' = {:?} (filtered by the BF, inner not queried)",
        value
    );

    // === TTL 透传 ===
    println!("\n=== TTL passthrough ===");
    backend
        .set(
            "temp".into(),
            b"temp_value".to_vec().into(),
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("set 'temp' (60s TTL)");

    let ttl: Option<Duration> = backend.ttl("temp").await?;
    println!(
        "ttl('temp') = {:?} (should be about 60s)",
        ttl.map(|d| d.as_secs())
    );

    // === stats ===
    println!("\n=== stats ===");
    let stats = backend.stats().await?;
    println!("Backend stats:");
    for (key, value) in &stats {
        println!("  {} = {}", key, value);
    }

    println!("\nBloom Filter example complete!");
    Ok(())
}
