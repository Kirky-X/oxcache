//! 延迟基准测试
//!
//! 本示例测试 Oxcache 的读写延迟性能。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_latency_benchmark --release
//!

use std::sync::Arc;
use std::time::Instant;
use oxcache::Cache;

fn percentile(values: &mut Vec<f64>, p: f64) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = (values.len() as f64 * p / 100.0) as usize;
    values[idx.min(values.len() - 1)]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 延迟基准测试 ===\n");

    let iterations = 10_000;

    // 创建缓存
    let cache: Arc<Cache<String, String>> = Arc::new(Cache::new().await?);

    // 预热
    println!("预热缓存...");
    for i in 0..1000 {
        cache
            .set(&format!("key:{}", i), &format!("value:{}", i), None)
            .await?;
    }
    println!("预热完成\n");

    // 1. 写入延迟测试
    println!("1. 写入延迟测试");
    let mut write_latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let start = Instant::now();
        cache
            .set(&format!("write:{}", i), &format!("value:{}", i), None)
            .await?;
        let elapsed = start.elapsed();
        write_latencies.push(elapsed.as_nanos() as f64);
    }

    let avg_write_latency = write_latencies.iter().sum::<f64>() / write_latencies.len() as f64;
    let mut write_sorted = write_latencies.clone();

    println!("  - 样本数: {}", iterations);
    println!("  - 平均延迟: {:.2} ns", avg_write_latency);
    println!("  - P50: {:.2} ns", percentile(&mut write_sorted, 50.0));
    println!("  - P95: {:.2} ns", percentile(&mut write_sorted, 95.0));
    println!("  - P99: {:.2} ns", percentile(&mut write_sorted, 99.0));
    println!("  - 最大延迟: {:.2} ns", write_sorted.iter().max().unwrap());
    println!();

    // 2. 读取延迟测试
    println!("2. 读取延迟测试");
    let mut read_latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let key = format!("key:{}", i % 1000);
        let start = Instant::now();
        let _ = cache.get(&key).await?;
        let elapsed = start.elapsed();
        read_latencies.push(elapsed.as_nanos() as f64);
    }

    let avg_read_latency = read_latencies.iter().sum::<f64>() / read_latencies.len() as f64;
    let mut read_sorted = read_latencies.clone();

    println!("  - 样本数: {}", iterations);
    println!("  - 平均延迟: {:.2} ns", avg_read_latency);
    println!("  - P50: {:.2} ns", percentile(&mut read_sorted, 50.0));
    println!("  - P95: {:.2} ns", percentile(&mut read_sorted, 95.0));
    println!("  - P99: {:.2} ns", percentile(&mut read_sorted, 99.0));
    println!("  - 最大延迟: {:.2} ns", read_sorted.iter().max().unwrap());
    println!();

    // 3. 并发读取延迟测试
    println!("3. 并发读取延迟测试 (10 线程)");
    let mut handles = Vec::new();
    let cache = cache.clone();

    for _ in 0..10 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            let mut latencies = Vec::with_capacity(iterations / 10);
            for i in 0..iterations / 10 {
                let key = format!("key:{}", i % 1000);
                let start = Instant::now();
                let _ = cache.get(&key).await?;
                let elapsed = start.elapsed();
                latencies.push(elapsed.as_nanos() as f64);
            }
            Ok::<Vec<f64>, Box<dyn std::error::Error>>(latencies)
        });
        handles.push(handle);
    }

    let mut concurrent_latencies = Vec::new();
    for handle in handles {
        concurrent_latencies.extend(handle.await??);
    }

    let avg_concurrent_latency = concurrent_latencies.iter().sum::<f64>() / concurrent_latencies.len() as f64;
    let mut concurrent_sorted = concurrent_latencies.clone();

    println!("  - 样本数: {}", concurrent_latencies.len());
    println!("  - 平均延迟: {:.2} ns", avg_concurrent_latency);
    println!("  - P50: {:.2} ns", percentile(&mut concurrent_sorted, 50.0));
    println!("  - P95: {:.2} ns", percentile(&mut concurrent_sorted, 95.0));
    println!("  - P99: {:.2} ns", percentile(&mut concurrent_sorted, 99.0));
    println!();

    // 4. 删除延迟测试
    println!("4. 删除延迟测试");
    let mut delete_latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let key = format!("delete:{}", i);
        cache.set(&key, "temp", None).await?;

        let start = Instant::now();
        cache.delete(&key).await?;
        let elapsed = start.elapsed();
        delete_latencies.push(elapsed.as_nanos() as f64);
    }

    let avg_delete_latency = delete_latencies.iter().sum::<f64>() / delete_latencies.len() as f64;
    let mut delete_sorted = delete_latencies.clone();

    println!("  - 样本数: {}", iterations);
    println!("  - 平均延迟: {:.2} ns", avg_delete_latency);
    println!("  - P50: {:.2} ns", percentile(&mut delete_sorted, 50.0));
    println!("  - P95: {:.2} ns", percentile(&mut delete_sorted, 95.0));
    println!("  - P99: {:.2} ns", percentile(&mut delete_sorted, 99.0));
    println!();

    // 总结
    println!("5. 延迟总结");
    println!("  ┌─────────────┬────────────┐");
    println!("  │ 操作类型     │ 平均延迟   │");
    println!("  ├─────────────┼────────────┤");
    println!(
        "  │ 写入        │ {:.2} ns   │",
        avg_write_latency
    );
    println!(
        "  │ 读取        │ {:.2} ns   │",
        avg_read_latency
    );
    println!(
        "  │ 并发读取    │ {:.2} ns   │",
        avg_concurrent_latency
    );
    println!(
        "  │ 删除        │ {:.2} ns   │",
        avg_delete_latency
    );
    println!("  └─────────────┴────────────┘");

    println!("\n=== 延迟基准测试完成 ===");
    Ok(())
}