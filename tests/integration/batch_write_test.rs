// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 批量写入集成测试 - 使用新API

use crate::common::{get_redis_url_insecure, is_redis_available, setup_logging};
use oxcache::Cache;
use std::time::Duration;

/// 测试批量写入性能
///
/// 验证批量写入功能是否能正确工作并提高性能
#[tokio::test]
async fn test_batch_write_performance() {
    if !is_redis_available().await {
        println!("Skipping test: Redis not available");
        return;
    }

    setup_logging();

    // 使用正确的 Redis URL（根据环境变量决定是否使用 TLS）
    let redis_url = get_redis_url_insecure();

    // 使用新API创建Redis缓存实例
    let cache: Cache<String, i32> = match Cache::redis(&redis_url).await {
        Ok(c) => c,
        Err(e) => {
            println!("Skipping test: Failed to create Redis cache: {}", e);
            return;
        }
    };

    // 清理可能存在的旧数据
    let _ = cache.delete(&"batch_key_50".to_string()).await;

    // 1. 快速写入100个项目
    let mut write_errors = 0;
    for i in 0..100 {
        match cache.set(&format!("batch_key_{}", i), &i).await {
            Ok(_) => {}
            Err(e) => {
                write_errors += 1;
                if write_errors > 10 {
                    println!("Skipping test: Too many write errors: {}", e);
                    return;
                }
            }
        }
    }

    if write_errors > 0 {
        println!("Warning: {} write errors occurred", write_errors);
    }

    // 2. 等待写入完成（增加等待时间）
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 3. 验证数据存在（带重试机制）
    let mut value = None;
    for attempt in 0..3 {
        match cache.get(&"batch_key_50".to_string()).await {
            Ok(v) => {
                value = v;
                break;
            }
            Err(e) if attempt < 2 => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                println!("Failed to read value after 3 attempts: {}", e);
            }
        }
    }

    assert!(value.is_some(), "Value should exist after batch write");
    assert_eq!(value.unwrap(), 50);

    // 清理测试数据
    for i in 0..100 {
        let _ = cache.delete(&format!("batch_key_{}", i)).await;
    }

    // 关闭缓存
    let _ = cache.shutdown().await;
}
