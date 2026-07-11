// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Redis Pipeline 批量操作示例
//!
//! 本示例演示使用 Redis Pipeline 进行高效的批量操作：
//! - set_many_pipeline: 批量设置键值对
//! - get_many_pipeline: 批量获取键值
//! - delete_many_pipeline: 批量删除键
//!
//! Pipeline 通过单次网络往返执行多个命令，大幅提升批量操作性能。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_redis_pipeline
//! ```

use oxcache::backend::{CacheWriter, RedisBackend};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis Pipeline 批量操作示例 ===\n");

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!("连接 Redis: {}", redis_url);

    let backend = RedisBackend::new(&redis_url).await?;
    println!("✓ Redis 连接成功\n");

    // 1. 批量设置（带 TTL）
    println!("--- 1. 批量设置（set_many_pipeline） ---");
    // 先收集 String，再借用为 &str
    let owned_items: Vec<(String, Vec<u8>)> = (0..10)
        .map(|i| (format!("pipeline:key:{}", i), format!("value_{}", i).into_bytes()))
        .collect();
    let items: Vec<(&str, Vec<u8>)> = owned_items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    let start = Instant::now();
    backend.set_many_pipeline(&items, Some(Duration::from_secs(60))).await?;
    let elapsed = start.elapsed();
    println!("  批量设置 {} 个键，耗时: {:?}", items.len(), elapsed);

    // 2. 批量获取
    println!("\n--- 2. 批量获取（get_many_pipeline） ---");
    let owned_keys: Vec<String> = (0..10).map(|i| format!("pipeline:key:{}", i)).collect();
    let keys: Vec<&str> = owned_keys.iter().map(|s| s.as_str()).collect();

    let start = Instant::now();
    let results = backend.get_many_pipeline(&keys).await?;
    let elapsed = start.elapsed();

    println!("  批量获取 {} 个键，耗时: {:?}", keys.len(), elapsed);
    println!("  结果:");
    for (i, result) in results.iter().enumerate() {
        match result {
            Some(data) => println!("    key[{}] = {:?}", i, String::from_utf8_lossy(data)),
            None => println!("    key[{}] = (nil)", i),
        }
    }

    // 3. 对比：逐个设置 vs Pipeline
    println!("\n--- 3. 性能对比：逐个设置 vs Pipeline ---");

    // 逐个设置
    let start = Instant::now();
    for i in 0..10 {
        let key = format!("individual:key:{}", i);
        let value = format!("value_{}", i).into_bytes();
        backend.set(&key, value, None).await?;
    }
    let individual_elapsed = start.elapsed();

    // Pipeline 设置
    let owned_perf: Vec<(String, Vec<u8>)> = (0..10)
        .map(|i| (format!("pipeline:perf:{}", i), format!("value_{}", i).into_bytes()))
        .collect();
    let perf_items: Vec<(&str, Vec<u8>)> = owned_perf.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    let start = Instant::now();
    backend.set_many_pipeline(&perf_items, None).await?;
    let pipeline_elapsed = start.elapsed();

    println!("  逐个设置 10 个键: {:?}", individual_elapsed);
    println!("  Pipeline 设置 10 个键: {:?}", pipeline_elapsed);
    println!(
        "  Pipeline 快 {:.1}x",
        individual_elapsed.as_secs_f64() / pipeline_elapsed.as_secs_f64().max(0.000001)
    );

    // 4. 批量删除
    println!("\n--- 4. 批量删除（delete_many_pipeline） ---");
    let owned_perf_keys: Vec<String> = (0..10).map(|i| format!("pipeline:perf:{}", i)).collect();
    let owned_ind_keys: Vec<String> = (0..10).map(|i| format!("individual:key:{}", i)).collect();

    let mut all_owned: Vec<String> = owned_keys.clone();
    all_owned.extend(owned_perf_keys.iter().cloned());
    all_owned.extend(owned_ind_keys.iter().cloned());

    let all_keys: Vec<&str> = all_owned.iter().map(|s| s.as_str()).collect();

    backend.delete_many_pipeline(&all_keys).await?;
    println!("  批量删除 {} 个键", all_keys.len());

    // 验证删除
    let results = backend.get_many_pipeline(&all_keys).await?;
    let remaining = results.iter().filter(|r| r.is_some()).count();
    println!("  剩余键数: {} (应为 0)", remaining);

    println!("\n✓ 示例完成");
    Ok(())
}
