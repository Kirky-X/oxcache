// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
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

use oxcache::{export_json_format, export_prometheus_format, get_enhanced_stats, Cache};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 指标与可观测性示例 ===\n");

    // 1. 创建缓存并执行操作
    println!("--- 1. 创建缓存并执行操作 ---");
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

    println!("  执行了 20 次写入，20 次读取（15 命中 + 5 未命中）");

    // 2. 获取后端统计
    println!("\n--- 2. 后端统计信息 ---");
    let stats = cache.stats().await?;
    println!("  后端统计:");
    for (key, value) in &stats {
        println!("    {}: {}", key, value);
    }

    // 3. 获取增强统计（CacheStats）
    println!("\n--- 3. 增强统计（CacheStats） ---");
    let enhanced = get_enhanced_stats();
    println!("  L1 命中: {}", enhanced.l1_hits);
    println!("  L1 未命中: {}", enhanced.l1_misses);
    println!("  L2 命中: {}", enhanced.l2_hits);
    println!("  L2 未命中: {}", enhanced.l2_misses);
    println!("  L1 写入: {}", enhanced.l1_sets);
    println!("  L2 写入: {}", enhanced.l2_sets);
    println!("  总操作数: {}", enhanced.total_operations);
    println!("  L1 条目数: {}", enhanced.l1_item_count);

    // 4. 计算命中率
    println!("\n--- 4. 命中率 ---");
    println!("  L1 命中率: {}", enhanced.l1_hit_rate_percent());
    println!("  L2 命中率: {}", enhanced.l2_hit_rate_percent());
    println!("  总体命中率: {}", enhanced.overall_hit_rate_percent());

    // 5. 导出 Prometheus 格式
    println!("\n--- 5. Prometheus 格式导出 ---");
    let prometheus = export_prometheus_format();
    println!("{}", prometheus);

    // 6. 导出 JSON 格式
    println!("\n--- 6. JSON 格式导出 ---");
    let json = export_json_format()?;
    println!("{}", json);

    // 7. 使用 CacheStats 实例方法导出
    println!("\n--- 7. CacheStats 实例方法导出 ---");
    let stats = get_enhanced_stats();
    let prom = stats.export_prometheus();
    println!("  CacheStats.export_prometheus() (前 200 字符):");
    println!("    {}", &prom[..prom.len().min(200)]);

    let json = stats.export_json()?;
    println!("  CacheStats.export_json() (前 200 字符):");
    println!("    {}", &json[..json.len().min(200)]);

    // 8. 清理
    cache.clear().await?;

    println!("\n✓ 示例完成");
    Ok(())
}
