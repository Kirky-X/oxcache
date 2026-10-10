// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 批量写入集成测试 - 使用新API

use crate::common::{get_redis_url_insecure, is_redis_available, setup_logging};
use oxcache::Cache;

/// 测试批量写入（pipeline）
///
/// `Cache::set_many` 经 `RedisBackend::set_many`（单次 Pipeline，100 次 SET
/// 合并为一次网络往返）写入 100 项，再用 `get_many` 逐项校验值，最后用
/// `delete_many` 清理并确认全部删除。
#[tokio::test]
async fn test_batch_write_performance_writes_and_verifies_100_items() {
    if !is_redis_available().await {
        return;
    }

    setup_logging();

    // 使用正确的 Redis URL（根据环境变量决定是否使用 TLS）
    let redis_url = get_redis_url_insecure();

    // 使用新API创建Redis缓存实例
    let cache: Cache<String, i32> = Cache::redis(&redis_url).await.expect("create Redis cache");

    let keys: Vec<String> = (0..100).map(|i| format!("batch_key_{}", i)).collect();
    let values: Vec<i32> = (0..100).collect();

    // 单次 pipeline 批量写入 100 项
    cache
        .set_many(keys.iter().zip(values.iter()))
        .await
        .expect("pipeline set_many should succeed");

    // 逐项校验：100 项全部可读且值等于写入值
    let fetched = cache.get_many(keys.iter()).await.expect("get_many");
    assert_eq!(
        fetched.len(),
        100,
        "all 100 pipeline keys should be present"
    );
    println!(
        "pipeline set_many wrote {} items, get_many read {} items back",
        keys.len(),
        fetched.len()
    );
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(
            fetched.get(key),
            Some(&(i as i32)),
            "key {key} should roundtrip after pipeline write"
        );
    }

    // 单键读取交叉校验（与批量读同口径）
    let single = cache.get(&"batch_key_50".to_string()).await.unwrap();
    assert_eq!(single, Some(50), "single get should see pipeline write");

    // 清理测试数据（批量删除）并确认删除生效
    cache
        .delete_many(keys.iter())
        .await
        .expect("delete_many should succeed");
    let after_delete = cache.get_many(keys.iter()).await.expect("get_many");
    assert!(
        after_delete.is_empty(),
        "all keys should be gone after delete_many, got {}",
        after_delete.len()
    );

    // 关闭缓存
    let _ = cache.shutdown().await;
}
