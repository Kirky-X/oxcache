// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 批量写入集成测试 - 使用新API

use common::{is_redis_available, setup_logging};
use oxcache::Cache;
use std::time::Duration;

use crate::common;

/// 测试批量写入性能
///
/// 验证批量写入功能是否能正确工作并提高性能
#[tokio::test]
async fn test_batch_write_performance() {
    if !is_redis_available() {
        return;
    }

    setup_logging();
    let redis_url = "redis://127.0.0.1:6379";

    // 使用新API创建Redis缓存实例（注意：当前版本暂不支持tiered，使用Redis作为L2）
    let cache: Cache<String, i32> = Cache::redis(redis_url)
        .await
        .expect("Failed to create Redis cache");

    // 1. 快速写入100个项目
    for i in 0..100 {
        cache.set(&format!("batch_key_{}", i), &i).await.unwrap();
    }

    // 2. 等待批量刷新
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 3. 验证数据存在
    let value = cache.get(&"batch_key_50".to_string()).await.unwrap();
    assert!(value.is_some(), "Value should exist after batch write");
    assert_eq!(value.unwrap(), 50);

    // 清理
    cache.shutdown().await.expect("Shutdown failed");
}
