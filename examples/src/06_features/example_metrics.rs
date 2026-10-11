// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 指标与可观测性示例
//!
//! 本示例演示 oxcache 的指标收集和导出功能：
//! - 获取缓存统计信息（CacheStats）
//! - 计算命中率（L1/L2/总体）
//! - 导出 Prometheus 格式指标
//! - 导出 JSON 格式指标
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_metrics
//! ```

use oxcache::{Cache, export_json_format, export_prometheus_format, get_enhanced_stats};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Metrics and observability example ===\n");

    println!("--- 1. Creating a cache and running operations ---");
    let cache: Cache<String, String> = Cache::builder().capacity(1000).build().await?;

    // 执行一些操作以产生指标
    for i in 0..20 {
        let key = format!("user:{}", i);
        let value = format!("value_{}", i);
        cache.set(&key, &value).await?;
    }

    // 读取一些键（产生命中）
    for i in 0..15 {
        let key = format!("user:{}", i);
        let _ = cache.get(&key).await?;
    }

    // 读取不存在的键（产生未命中）
    for i in 100..105 {
        let key = format!("user:{}", i);
        let _ = cache.get(&key).await?;
    }

    println!("  performed 20 writes and 20 reads (15 hits + 5 misses)");

    println!("\n--- 2. Backend statistics ---");
    let stats = cache.stats().await?;
    println!("  backend statistics:");
    for (key, value) in &stats {
        println!("    {}: {}", key, value);
    }

    println!("\n--- 3. Enhanced statistics (CacheStats) ---");
    let enhanced = get_enhanced_stats();
    println!("  L1 hits: {}", enhanced.l1_hits);
    println!("  L1 misses: {}", enhanced.l1_misses);
    println!("  L2 hits: {}", enhanced.l2_hits);
    println!("  L2 misses: {}", enhanced.l2_misses);
    println!("  L1 writes: {}", enhanced.l1_sets);
    println!("  L2 writes: {}", enhanced.l2_sets);
    println!("  total operations: {}", enhanced.total_operations);
    println!("  L1 entries: {}", enhanced.l1_item_count);

    println!("\n--- 4. Hit rate ---");
    println!("  L1 hit rate: {}", enhanced.l1_hit_rate_percent());
    println!("  L2 hit rate: {}", enhanced.l2_hit_rate_percent());
    println!(
        "  overall hit rate: {}",
        enhanced.overall_hit_rate_percent()
    );

    println!("\n--- 5. Prometheus format export ---");
    let prometheus = export_prometheus_format();
    println!("{}", prometheus);

    println!("\n--- 6. JSON format export ---");
    let json = export_json_format()?;
    println!("{}", json);

    println!("\n--- 7. CacheStats instance-method exports ---");
    let stats = get_enhanced_stats();
    let prom = stats.export_prometheus();
    println!("  CacheStats.export_prometheus() (first 200 characters):");
    println!("    {}", &prom[..prom.len().min(200)]);

    let json = stats.export_json()?;
    println!("  CacheStats.export_json() (first 200 characters):");
    println!("    {}", &json[..json.len().min(200)]);

    cache.clear().await?;

    println!("\n✓ Example complete");
    Ok(())
}
