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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTER_STANDALONE_TYPE,
    MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTERBACKEND_DECORATOR,
    MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_AFTER,
    MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_SHOULD,
    MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_MISSING_KEY_SHOULD,
    MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTER_CAPACITY_10000,
    MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTERBACKEND_WRAPPING_MOK,
    MSG_EXAMPLE_BLOOM_FILTER_EXAMPLE_COMPLETE, MSG_EXAMPLE_BLOOM_FILTER_GET_USER_999_FILTERED,
    MSG_EXAMPLE_BLOOM_FILTER_INNER_BACKEND_SCORE, MSG_EXAMPLE_BLOOM_FILTER_INSERTING_EXISTING_KEY,
    MSG_EXAMPLE_BLOOM_FILTER_SET_TEMP_60S_TTL, MSG_EXAMPLE_BLOOM_FILTER_TTL_PASSTHROUGH,
    MSG_EXAMPLE_BLOOM_FILTER_TTL_TEMP_SHOULD_BE, t,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === BloomFilter 独立类型 ===
    println!(
        "{}",
        t(MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTER_STANDALONE_TYPE, &[])
    );
    let bf = BloomFilter::new(10_000, 0.01);
    println!(
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTER_CAPACITY_10000,
            &[]
        )
    );

    bf.insert("existing_key");
    println!(
        "{}",
        t(MSG_EXAMPLE_BLOOM_FILTER_INSERTING_EXISTING_KEY, &[])
    );

    println!(
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_SHOULD,
            &[("existing_key", bf.contains("existing_key").to_string())]
        )
    );
    println!(
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_MISSING_KEY_SHOULD,
            &[("missing_key", bf.contains("missing_key").to_string())]
        )
    );

    bf.clear();
    println!(
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_AFTER,
            &[("existing_key", bf.contains("existing_key").to_string())]
        )
    );

    // === BloomFilterBackend 装饰器 ===
    println!(
        "\n{}",
        t(MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTERBACKEND_DECORATOR, &[])
    );
    let inner = MokaMemoryBackend::new();
    println!(
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_INNER_BACKEND_SCORE,
            &[
                ("backend_name", inner.backend_name().to_string()),
                ("score", inner.score().to_string())
            ]
        )
    );

    let backend = BloomFilterBackend::builder()
        .capacity(10_000)
        .false_positive_rate(0.01)
        .inner(inner)
        .build()?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTERBACKEND_WRAPPING_MOK,
            &[]
        )
    );

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
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_GET_USER_999_FILTERED,
            &[("value", format!("{:?}", value))]
        )
    );

    // === TTL 透传 ===
    println!("\n{}", t(MSG_EXAMPLE_BLOOM_FILTER_TTL_PASSTHROUGH, &[]));
    backend
        .set(
            "temp".into(),
            b"temp_value".to_vec().into(),
            Some(Duration::from_secs(60)),
        )
        .await?;
    println!("{}", t(MSG_EXAMPLE_BLOOM_FILTER_SET_TEMP_60S_TTL, &[]));

    let ttl: Option<Duration> = backend.ttl("temp").await?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_BLOOM_FILTER_TTL_TEMP_SHOULD_BE,
            &[("as_secs", format!("{:?}", ttl.map(|d| d.as_secs())))]
        )
    );

    // === stats ===
    println!("\n=== stats ===");
    let stats = backend.stats().await?;
    println!("Backend stats:");
    for (key, value) in &stats {
        println!("  {} = {}", key, value);
    }

    println!("\n{}", t(MSG_EXAMPLE_BLOOM_FILTER_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
