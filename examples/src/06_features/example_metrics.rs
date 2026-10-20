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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_METRICS_1_CREATING_A_CACHE, MSG_EXAMPLE_METRICS_2_BACKEND_STATISTICS,
    MSG_EXAMPLE_METRICS_3_ENHANCED_STATISTICS_CACHESTATS, MSG_EXAMPLE_METRICS_4_HIT_RATE,
    MSG_EXAMPLE_METRICS_5_PROMETHEUS_FORMAT_EXPORT, MSG_EXAMPLE_METRICS_6_JSON_FORMAT_EXPORT,
    MSG_EXAMPLE_METRICS_7_CACHESTATS_INSTANCE_METHOD,
    MSG_EXAMPLE_METRICS_AND_OBSERVABILITY_EXAMPLE, MSG_EXAMPLE_METRICS_BACKEND_STATISTICS,
    MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_JSON_FIRST,
    MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_PROMETHEUS_FIRST, MSG_EXAMPLE_METRICS_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_METRICS_L1_ENTRIES, MSG_EXAMPLE_METRICS_L1_HIT_RATE, MSG_EXAMPLE_METRICS_L1_HITS,
    MSG_EXAMPLE_METRICS_L1_MISSES, MSG_EXAMPLE_METRICS_L1_WRITES, MSG_EXAMPLE_METRICS_L2_HIT_RATE,
    MSG_EXAMPLE_METRICS_L2_HITS, MSG_EXAMPLE_METRICS_L2_MISSES, MSG_EXAMPLE_METRICS_L2_WRITES,
    MSG_EXAMPLE_METRICS_OVERALL_HIT_RATE, MSG_EXAMPLE_METRICS_PERFORMED_20_WRITES_AND,
    MSG_EXAMPLE_METRICS_TOTAL_OPERATIONS, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}\n",
        t(MSG_EXAMPLE_METRICS_AND_OBSERVABILITY_EXAMPLE, &[])
    );

    println!("{}", t(MSG_EXAMPLE_METRICS_1_CREATING_A_CACHE, &[]));
    let cache: Cache<String, String> = Cache::builder().capacity(1000).build().await?;

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

    println!("  {}", t(MSG_EXAMPLE_METRICS_PERFORMED_20_WRITES_AND, &[]));

    println!("\n{}", t(MSG_EXAMPLE_METRICS_2_BACKEND_STATISTICS, &[]));
    let stats = cache.stats().await?;
    println!("  {}", t(MSG_EXAMPLE_METRICS_BACKEND_STATISTICS, &[]));
    for (key, value) in &stats {
        println!("    {}: {}", key, value);
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_METRICS_3_ENHANCED_STATISTICS_CACHESTATS, &[])
    );
    let enhanced = get_enhanced_stats();
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L1_HITS,
            &[("l1_hits", enhanced.l1_hits.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L1_MISSES,
            &[("l1_misses", enhanced.l1_misses.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L2_HITS,
            &[("l2_hits", enhanced.l2_hits.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L2_MISSES,
            &[("l2_misses", enhanced.l2_misses.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L1_WRITES,
            &[("l1_sets", enhanced.l1_sets.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L2_WRITES,
            &[("l2_sets", enhanced.l2_sets.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_TOTAL_OPERATIONS,
            &[("total_operations", enhanced.total_operations.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L1_ENTRIES,
            &[("l1_item_count", enhanced.l1_item_count.to_string())]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_METRICS_4_HIT_RATE, &[]));
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L1_HIT_RATE,
            &[(
                "l1_hit_rate_percent",
                enhanced.l1_hit_rate_percent().to_string()
            )]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_L2_HIT_RATE,
            &[(
                "l2_hit_rate_percent",
                enhanced.l2_hit_rate_percent().to_string()
            )]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_METRICS_OVERALL_HIT_RATE,
            &[(
                "overall_hit_rate_percent",
                enhanced.overall_hit_rate_percent().to_string()
            )]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_METRICS_5_PROMETHEUS_FORMAT_EXPORT, &[])
    );
    let prometheus = export_prometheus_format();
    println!("{}", prometheus);

    println!("\n{}", t(MSG_EXAMPLE_METRICS_6_JSON_FORMAT_EXPORT, &[]));
    let json = export_json_format()?;
    println!("{}", json);

    println!(
        "\n{}",
        t(MSG_EXAMPLE_METRICS_7_CACHESTATS_INSTANCE_METHOD, &[])
    );
    let stats = get_enhanced_stats();
    let prom = stats.export_prometheus();
    println!(
        "  {}",
        t(MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_PROMETHEUS_FIRST, &[])
    );
    println!("    {}", &prom[..prom.len().min(200)]);

    let json = stats.export_json()?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_JSON_FIRST, &[])
    );
    println!("    {}", &json[..json.len().min(200)]);

    cache.clear().await?;

    println!("\n{}", t(MSG_EXAMPLE_METRICS_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
