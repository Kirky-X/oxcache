//! 指标收集示例
//!
//! 本示例演示如何使用 Oxcache 的指标收集功能。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_metrics
//!

use oxcache::Cache;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct MetricData {
    name: String,
    value: f64,
    timestamp: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 指标收集示例 ===\n");

    // 创建缓存用于存储指标数据
    let cache: Cache<String, MetricData> = Cache::new().await?;

    // 1. 收集应用指标
    println!("1. 收集应用指标");
    let metrics = vec![
        MetricData {
            name: "cpu_usage".to_string(),
            value: 45.5,
            timestamp: chrono::Local::now().to_rfc3339(),
        },
        MetricData {
            name: "memory_usage".to_string(),
            value: 62.3,
            timestamp: chrono::Local::now().to_rfc3339(),
        },
        MetricData {
            name: "disk_usage".to_string(),
            value: 78.1,
            timestamp: chrono::Local::now().to_rfc3339(),
        },
    ];

    println!("   添加指标数据...");
    for metric in &metrics {
        cache.set(&metric.name, metric, None).await?;
        println!("   ✓ {} = {}", metric.name, metric.value);
    }
    println!();

    // 2. 查询指标
    println!("2. 查询指标");
    let metric_names = ["cpu_usage", "memory_usage", "disk_usage"];
    for name in &metric_names {
        if let Some(m) = cache.get(name).await? {
            println!("   ✓ {} = {} (时间: {})", m.name, m.value, m.timestamp);
        }
    }
    println!();

    // 3. 模拟高频率指标更新
    println!("3. 模拟高频率指标更新");
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let value = (i as f64) / 10.0;
        cache
            .set(
                "realtime_metric",
                &MetricData {
                    name: "realtime_metric".to_string(),
                    value,
                    timestamp: chrono::Local::now().to_rfc3339(),
                },
                None,
            )
            .await?;
    }
    let elapsed = start.elapsed();
    println!("   ✓ 更新 1000 次指标，耗时: {:?}", elapsed);
    println!();

    // 4. 缓存统计
    println!("4. 缓存统计");
    let stats = cache.stats().await?;
    println!("   - 总条目数: {}", stats.item_count());
    println!("   - 命中次数: {}", stats.hit_count());
    println!("   - 未命中次数: {}", stats.miss_count());
    if stats.hit_count() + stats.miss_count() > 0 {
        let hit_rate = stats.hit_count() as f64
            / (stats.hit_count() + stats.miss_count()) as f64
            * 100.0;
        println!("   - 命中率: {:.2}%", hit_rate);
    }
    println!();

    // 清理
    println!("5. 清理测试数据");
    cache.clear().await?;
    println!("   ✓ 测试数据已清理\n");

    println!("=== 指标收集示例完成 ===");
    Ok(())
}