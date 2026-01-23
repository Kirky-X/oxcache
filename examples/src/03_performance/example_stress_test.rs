// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 压力测试示例
//
// 本示例在高负载下对缓存进行压力测试。
// 使用仅L1模式进行演示，无需Redis。

use oxcache::manager::{get_client, init};
use oxcache::{
    config::{L1Config, OxcacheConfig, ServiceConfig},
    CacheExt,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct StressTestData {
    id: u64,
    value: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use L1-only for stress test demo (no Redis required)
    let config = OxcacheConfig::builder()
        .with_service(
            "stress_cache",
            ServiceConfig::l1_only().with_l1(L1Config::new().with_max_capacity(100000)),
        )
        .build();

    let _ = init(config).await;

    let client = Arc::new(get_client("stress_cache")?);
    let semaphore = Arc::new(Semaphore::new(100)); // Limit concurrent operations
    let counter = Arc::new(AtomicU64::new(0)); // Simple ID counter
    let duration = std::time::Duration::from_secs(5);

    println!("压力测试示例 (仅L1模式)");
    println!("===================================\n");
    println!("开始进行 {} 秒压力测试...", duration.as_secs());
    println!("并发级别: 100");
    println!("\n测试期间将收集指标...\n");

    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    while start.elapsed() < duration {
        let client = client.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let id = counter.fetch_add(1, Ordering::SeqCst);

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let data = StressTestData {
                id,
                value: format!("stress_test_value_{}", id),
                timestamp: chrono::Utc::now(),
            };

            // Random operation: 60% read, 40% write
            let read = id % 10 < 6;
            if read {
                let _ = client
                    .get::<StressTestData>(&format!("stress:{}", id % 10000))
                    .await;
            } else {
                let _ = client
                    .set(&format!("stress:{}", id % 10000), &data, Some(3600))
                    .await;
            }
        }));
    }

    // Wait for all ongoing tasks to complete
    let handle_count = handles.len();
    for handle in handles {
        let _ = handle.await;
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (handle_count as f64) / elapsed.as_secs_f64();

    println!("=== 压力测试结果 ===");
    println!("持续时间: {:.2}s", elapsed.as_secs_f64());
    println!("总操作数: {}", handle_count);
    println!("吞吐量: {:.0} ops/sec", ops_per_sec);
    println!("\n注意: 演示使用仅L1模式。对于真实压力测试:");
    println!("  - 启用带Redis的双层缓存进行分布式测试");
    println!("  - 使用enable_batch_write获得更高写入吞吐量");
    println!("\n压力测试完成!");
    Ok(())
}