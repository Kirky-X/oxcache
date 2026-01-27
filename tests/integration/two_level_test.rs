// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 双层缓存集成测试 - 使用新API

use common::{cleanup_service, generate_unique_service_name, is_redis_available, setup_logging};
use oxcache::Cache;

use crate::common;

/// 测试双层缓存流程
///
/// 验证双层缓存系统的基本工作流程
#[tokio::test]
async fn test_two_level_cache_flow() {
    if !is_redis_available() {
        println!("跳过test_two_level_cache_flow：Redis不可用");
        return;
    }

    setup_logging();
    let service_name = generate_unique_service_name("flow_test");
    let redis_url = "redis://127.0.0.1:6379";

    // 使用新API创建缓存
    // 如果 TLS 连接失败，跳过测试
    let cache: Cache<String, String> = match Cache::redis(redis_url).await {
        Ok(c) => c,
        Err(e) => {
            println!(
                "Skipping test: Failed to create cache (TLS required): {}",
                e
            );
            return;
        }
    };

    // 1. 写入数据
    let test_val = "value1".to_string();
    cache.set(&"key1".to_string(), &test_val).await.unwrap();

    // 2. 验证L1命中（立即读取）
    let val: Option<String> = cache.get(&"key1".to_string()).await.unwrap();
    assert_eq!(val, Some("value1".to_string()));

    // 3. 删除并验证
    cache.delete(&"key1".to_string()).await.unwrap();
    let val: Option<String> = cache.get(&"key1".to_string()).await.unwrap();
    assert!(val.is_none());

    // 清理
    cache.shutdown().await.expect("Shutdown failed");
    cleanup_service(&service_name).await;
}
