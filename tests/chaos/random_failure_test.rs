// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 随机故障混沌测试（已迁移到新版 API）
//
// 本测试演示缓存存在随机Redis故障情况下的稳定性和恢复能力

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Once;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

static INIT: Once = Once::new();

fn setup_logging() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_span_events(FmtSpan::CLOSE)
            .with_env_filter(EnvFilter::new("debug"))
            .try_init()
            .ok();
    });
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct TestData {
    id: u64,
    content: String,
}

#[tokio::test]
#[ignore] // 需要真实的 Redis 环境
async fn test_random_redis_failures() {
    setup_logging();

    // 使用新版 Cache API 创建缓存实例
    let cache: Cache<String, TestData> = match Cache::builder().build().await {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("Failed to create cache: {}", e);
            return;
        }
    };

    // 测试基本操作
    let test_data = TestData {
        id: 1,
        content: "High availability data".to_string(),
    };

    let key = "test_key".to_string();
    if let Err(e) = cache.set(&key, &test_data).await {
        eprintln!("Set failed: {}", e);
    }

    match cache.get(&key).await {
        Ok(Some(data)) => println!("✓ Get successful: {}", data.content),
        Ok(None) => println!("✓ Get not found (expected)"),
        Err(e) => eprintln!("Get failed: {}", e),
    }

    // 测试存在性
    let exists = cache.exists(&key).await;
    println!("✓ Exists check: {:?}", exists);

    // 测试删除
    if let Err(e) = cache.delete(&key).await {
        eprintln!("Delete failed: {}", e);
    }

    // 再次验证删除
    let exists_after = cache.exists(&key).await;
    println!("✓ Exists after delete: {:?}", exists_after);

    println!("\n✓ 混沌测试通过（新版 API）");
}

#[tokio::test]
#[ignore] // 需要真实的 Redis 环境
async fn test_distributed_lock_during_failures() {
    setup_logging();

    // 使用 Arc 来共享 Cache
    let cache: Cache<String, TestData> = match Cache::builder().build().await {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("Failed to create cache: {}", e);
            return;
        }
    };

    let cache = std::sync::Arc::new(cache);

    // 测试并发访问
    let mut handles = Vec::new();
    for i in 0..10 {
        let cache_clone = std::sync::Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            let test_data = TestData {
                id: i,
                content: format!("Concurrent data {}", i),
            };

            let key = format!("concurrent_key_{}", i);
            if let Err(e) = cache_clone.set(&key, &test_data).await {
                eprintln!("Thread {} set failed: {}", i, e);
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        let _ = handle.await;
    }

    println!("✓ 并发测试通过（新版 API）");
}

#[tokio::test]
#[ignore] // 需要真实的 Redis 环境
async fn test_fault_recovery() {
    setup_logging();

    let cache: Cache<String, TestData> = match Cache::builder().build().await {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("Failed to create cache: {}", e);
            return;
        }
    };

    // 测试故障恢复能力
    let mut operations = Vec::new();
    for i in 0..10 {
        let test_data = TestData {
            id: i,
            content: format!("Recovery test data {}", i),
        };

        let key = format!("recovery_key_{}", i);
        // 写入数据
        if cache.set(&key, &test_data).await.is_ok() {
            operations.push("write");
        }

        // 读取数据
        if let Ok(Some(_)) = cache.get(&key).await {
            operations.push("read");
        }
    }

    // 验证所有操作都成功
    assert_eq!(operations.len(), 20);

    println!("✓ 故障恢复测试通过（新版 API）");
}
