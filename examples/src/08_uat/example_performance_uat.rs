//! 性能 UAT 测试示例
//!
//! 本示例展示性能相关的用户验收测试 (UAT) 场景。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_performance_uat --release
//!

use std::sync::Arc;
use std::time::Instant;
use oxcache::Cache;

fn percentile(values: &mut Vec<f64>, p: f64) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = (values.len() as f64 * p / 100.0) as usize;
    values[idx.min(values.len() - 1)]
}

// 性能测试 1: 吞吐量测试
async fn test_throughput_requirements() -> Result<(), Box<dyn std::error::Error>> {
    println!("   性能测试 1: 吞吐量测试...");

    let cache: Arc<Cache<String, String>> = Arc::new(Cache::new().await?);
    let iterations = 10000;

    // 预热
    for i in 0..100 {
        cache.set(&format!("warmup:{}", i), "warmup", None).await?;
    }

    // 测试写入吞吐量
    let start = Instant::now();
    let mut handles = Vec::new();
    for i in 0..10 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for j in 0..iterations / 10 {
                cache
                    .set(&format!("write:{}:{}", i, j), "test_value", None)
                    .await?;
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await??;
    }
    let write_elapsed = start.elapsed();
    let write_throughput = iterations as f64 / write_elapsed.as_secs_f64();

    println!("     写入吞吐量: {:.2} writes/sec", write_throughput);
    assert!(write_throughput > 10000.0, "写入吞吐量应 > 10000 writes/sec");

    // 测试读取吞吐量
    let start = Instant::new();
    let mut handles = Vec::new();
    for i in 0..10 {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for j in 0..iterations / 10 {
                cache.get(&format!("write:{}:{}", i, j)).await?;
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.await??;
    }
    let read_elapsed = start.elapsed();
    let read_throughput = iterations as f64 / read_elapsed.as_secs_f64();

    println!("     读取吞吐量: {:.2} reads/sec", read_throughput);
    assert!(read_throughput > 10000.0, "读取吞吐量应 > 10000 reads/sec");

    // 清理
    cache.clear().await?;
    println!("   ✓ 吞吐量测试通过");

    Ok(())
}

// 性能测试 2: 延迟测试
async fn test_latency_requirements() -> Result<(), Box<dyn std::error::Error>> {
    println!("   性能测试 2: 延迟测试...");

    let cache: Cache<String, String> = Cache::new().await?;
    let iterations = 1000;

    // 预热
    for i in 0..100 {
        cache.set(&format!("latency:{}", i), "test", None).await?;
    }

    // 测试写入延迟
    let mut write_latencies = Vec::new();
    for i in 0..iterations {
        let start = Instant::now();
        cache
            .set(&format!("latency:write:{}", i), "test_value", None)
            .await?;
        write_latencies.push(start.elapsed().as_secs_f64() * 1_000_000.0); // 转换为微秒
    }

    let avg_write = write_latencies.iter().sum::<f64>() / write_latencies.len() as f64;
    let mut sorted = write_latencies.clone();
    let p99_write = percentile(&mut sorted, 99.0);

    println!("     写入延迟: avg={:.2}µs, p99={:.2}µs", avg_write, p99_write);
    assert!(avg_write < 1000.0, "平均写入延迟应 < 1000µs");

    // 测试读取延迟
    let mut read_latencies = Vec::new();
    for i in 0..100 {
        let start = Instant::new();
        cache.get(&format!("latency:{}", i)).await?;
        read_latencies.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }

    let avg_read = read_latencies.iter().sum::<f64>() / read_latencies.len() as f64;
    let mut sorted = read_latencies.clone();
    let p99_read = percentile(&mut sorted, 99.0);

    println!("     读取延迟: avg={:.2}µs, p99={:.2}µs", avg_read, p99_read);
    assert!(avg_read < 1000.0, "平均读取延迟应 < 1000µs");

    // 清理
    cache.clear().await?;
    println!("   ✓ 延迟测试通过");

    Ok(())
}

// 性能测试 3: 并发测试
async fn test_concurrency_requirements() -> Result<(), Box<dyn std::error::Error>> {
    println!("   性能测试 3: 并发测试...");

    let cache: Arc<Cache<String, String>> = Arc::new(Cache::new().await?);
    let num_tasks = 100;
    let iterations_per_task = 100;

    // 并发读写测试
    let start = Instant::new();
    let mut handles = Vec::new();

    for task_id in 0..num_tasks {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..iterations_per_task {
                let key = format!("concurrency:{}:{}", task_id, i);
                cache.set(&key, "test_value", None).await?;
                cache.get(&key).await?;
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    let elapsed = start.elapsed();
    let total_ops = num_tasks * iterations_per_task * 2; // 1 write + 1 read per iteration
    let throughput = total_ops as f64 / elapsed.as_secs_f64();

    println!("     并发操作数: {}", total_ops);
    println!("     耗时: {:?}", elapsed);
    println!("     吞吐量: {:.2} ops/sec", throughput);
    assert!(throughput > 5000.0, "并发吞吐量应 > 5000 ops/sec");

    // 验证数据一致性
    let cache_ref = cache.clone();
    let mut handles = Vec::new();
    for task_id in 0..num_tasks {
        let cache = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..iterations_per_task {
                let key = format!("concurrency:{}:{}", task_id, i);
                let value = cache.get(&key).await?;
                assert!(value.is_some(), "数据应该存在");
                assert_eq!(value.unwrap(), "test_value", "数据值应该正确");
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    // 清理
    cache.clear().await?;
    println!("   ✓ 并发测试通过");

    Ok(())
}

// 性能测试 4: 内存使用测试
async fn test_memory_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("   性能测试 4: 内存使用测试...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 添加 10000 个条目
    let entries = 10000;
    for i in 0..entries {
        cache
            .set(
                &format!("memory:{}", i),
                &"x".repeat(100), // 100 字节的值
                None,
            )
            .await?;
    }

    let stats = cache.stats().await?;
    let item_count = stats.item_count();

    assert_eq!(item_count, entries, "应该存在 {} 个条目", entries);

    // 清理
    cache.clear().await?;
    println!("   ✓ 内存使用测试通过");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 性能 UAT 测试示例 ===\n");

    println!("运行性能测试...\n");

    test_throughput_requirements().await?;
    test_latency_requirements().await?;
    test_concurrency_requirements().await?;
    test_memory_usage().await?;

    println!();
    println!("=== 所有性能 UAT 测试通过 ===");
    Ok(())
}
