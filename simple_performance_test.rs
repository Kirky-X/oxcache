//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 简单性能测试脚本
//!
//! 直接测试基本操作性能

use std::time::Instant;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct TestData {
    id: u64,
    data: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Oxcache 性能基准测试 ===\n");

    // 测试1: L1缓存基本操作性能
    test_l1_basic_operations().await?;

    // 测试2: L1缓存不同数据大小性能
    test_l1_data_sizes().await?;

    // 测试3: L1缓存并发性能
    test_l1_concurrent_performance().await?;

    // 测试4: L1缓存吞吐量性能
    test_l1_throughput().await?;

    println!("\n=== 性能测试完成 ===");
    Ok(())
}

async fn test_l1_basic_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("测试1: L1缓存基本操作性能");
    
    // 使用最简化的配置
    let cache = oxcache::Cache::<String, TestData>::builder()
        .memory()
        .build()
        .await?;

    // 预填充数据
    for i in 0..1000 {
        let key = format!("test_key_{}", i);
        let data = TestData {
            id: i,
            data: vec![0u8; 100],
        };
        cache.set(&key, &data).await?;
    }

    // 测试GET命中性能
    let start = Instant::now();
    for _ in 0..10000 {
        let _: Option<TestData> = cache.get(&"test_key_42".to_string()).await?;
    }
    let get_hit_duration = start.elapsed();
    println!("  - GET命中 (10000次): {:?} ({:.2} ns/op", get_hit_duration, get_hit_duration.as_nanos() as f64 / 10000.0);

    // 测试GET未命中性能
    let start = Instant::now();
    for _ in 0..10000 {
        let _: Option<TestData> = cache.get(&"nonexistent_key".to_string()).await?;
    }
    let get_miss_duration = start.elapsed();
    println!("  - GET未命中 (10000次): {:?} ({:.2} ns/op)", get_miss_duration, get_miss_duration.as_nanos() as f64 / 10000.0);

    // 测试SET性能
    let start = Instant::now();
    for i in 0..10000 {
        let key = format!("set_test_key_{}", i);
        let data = TestData {
            id: i,
            data: vec![0u8; 100],
        };
        cache.set(&key, &data).await?;
    }
    let set_duration = start.elapsed();
    println!("  - SET (10000次): {:?} ({:.2} ns/op)", set_duration, set_duration.as_nanos() as f64 / 10000.0);

    println!("  平均延迟: GET命中={:.2}ns, GET未命中={:.2}ns, SET={:.2}ns", 
        get_hit_duration.as_nanos() as f64 / 10000.0,
        get_miss_duration.as_nanos() as f64 / 10000.0,
        set_duration.as_nanos() as f64 / 10000.0
    );
    
    // 计算QPS
    let total_ops = 30000;
    let total_time = get_hit_duration + get_miss_duration + set_duration;
    let qps = total_ops as f64 / total_time.as_secs_f64();
    println!("  吞吐量: {:.0} ops/sec\n", qps);
    
    Ok(())
}

async fn test_l1_data_sizes() -> Result<(), Box<dyn std::error::Error>> {
    println!("测试2: L1缓存不同数据大小性能");
    
    let cache = oxcache::Cache::<String, Vec<u8>>::memory().await?;

    let sizes = vec![10, 100, 1000, 10000];
    
    for size in sizes {
        let start = Instant::now();
        
        // 测试SET不同大小的数据
        for i in 0..1000 {
            let key = format!("size_test_{}_{}", size);
            let data = vec![0u8; size];
            cache.set(&key, &data).await?;
        }
        
        let set_duration = start.elapsed();
        
        // 测试GET不同大小的数据
        let start = Instant::now();
        for i in 0..1000 {
            let key = format!("size_test_{}_{}", size);
            let _: Option<Vec<u8>> = cache.get(&key).await?;
        }
        let get_duration = start.elapsed();
        
        let set_ns = set_duration.as_nanos() as f64 / 1000.0;
        let get_ns = get_duration.as_nanos() as f64 / 1000.0;
        
        println!("  - 数据大小: {}字节", size);
        println!("    SET: {:.2} ns/op", set_ns);
        println!("    GET: {:.2} ns/op", get_ns);
        println!("    平均: {:.2} ns/op", (set_ns + get_ns) / 2.0);
        
        // 清理数据
        for i in 0..1000 {
            let key = format!("size_test_{}_{}", size);
            cache.delete(&key).await?;
        }
    }
    
    println!();
    Ok(())
}

async fn test_l1_concurrent_performance() -> Result<(), Box<dyn std::error::Error>> {
    println!("测试3: L1缓存并发性能");
    
    let cache = oxcache::Cache::<String, TestData>::memory().await?;
    
    let concurrency_levels = vec![1, 10, 50, 100];
    
    for concurrency in concurrency_levels {
        println!("  并发级别: {}", concurrency);
        
        let start = Instant::now();
        let mut handles = Vec::new();
        
        for thread_id in 0..concurrency {
            let cache = cache.clone();
            let handle = tokio::spawn(async move {
                let mut ops = 0;
                let thread_start = Instant::now();
                
                for i in 0..100 {
                    let key = format!("concurrent_{}_{}", thread_id, i);
                    let data = TestData {
                        id: i,
                        data: vec![0u8; 50],
                    };
                    cache.set(&key, &data).await.unwrap();
                    let _: Option<TestData> = cache.get(&key).await.unwrap();
                    ops += 2;
                }
                
                let thread_duration = thread_start.elapsed();
                (thread_duration, ops)
            });
            handles.push(handle);
        }
        
        let mut total_ops = 0;
        let mut max_thread_time = std::time::Duration::ZERO;
        
        for handle in handles {
            let (thread_duration, ops) = handle.await?;
            total_ops += ops;
            if thread_duration > max_thread_time {
                max_thread_time = thread_duration;
            }
        }
        
        let total_duration = start.elapsed();
        let total_ns = total_duration.as_nanos() as f64;
        let ops_per_sec = total_ops as f64 / total_duration.as_secs_f64();
        
        println!("    总操作数: {}", total_ops);
        println!("    总时间: {:?}", total_duration);
        println!("    吞吐量: {:.0} ops/sec", ops_per_sec);
        println!("    平均延迟: {:.2} ns/op", total_ns / total_ops as f64);
        println!("    最大线程时间: {:?}", max_thread_time);
        println!();
    }
    
    Ok(())
}

async fn test_l1_throughput() -> Result<(), Box<dyn std::error::Error>> {
    println!("测试4: L1缓存吞吐量性能");
    
    let cache = oxcache::Cache::<String, TestData>::memory().await?;
    
    let ops_counts = vec![1000, 5000, 10000, 50000];
    
    for ops_count in ops_counts {
        println!("  操作数: {}", ops_count);
        
        let start = Instant::now();
        
        for i in 0..ops_count {
            let key = format!("throughput_key_{}", i);
            let data = TestData {
                id: i,
                data: vec![0u8; 80],  // 平均大小
            };
            cache.set(&key, &data).await?;
        }
        
        let duration = start.elapsed();
        let ops_per_sec = ops_count as f64 / duration.as_secs_f64();
        let avg_ns = duration.as_nanos() as f64 / ops_count as f64;
        
        println!("    时间: {:?}", duration);
        println!("    吞吐量: {:.0} ops/sec", ops_per_sec);
        println!("    平均延迟: {:.2} ns/op", avg_ns);
        
        // 清理数据
        for i in 0..ops_count {
            let key = format!("throughput_key_{}", i);
            cache.delete(&key).await?;
        }
        
        println!();
    }
    
    Ok(())
}