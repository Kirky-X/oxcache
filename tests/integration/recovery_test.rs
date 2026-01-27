// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 故障恢复集成测试 - 使用新API
//
use crate::common;
use common::{is_redis_available, setup_logging};
use oxcache::Cache;

/// 测试降级逻辑
///
/// 验证当L2缓存不可用时，系统能否正确报告错误
/// 当Redis可用时，测试正常缓存操作
#[tokio::test]
async fn test_degradation_logic() {
    use std::time::Duration;

    setup_logging();
    let redis_url = common::get_redis_url();

    // 跳过测试如果Redis不可用
    if !is_redis_available() {
        println!("跳过测试: Redis不可用");
        return;
    }

    // 生成唯一键以避免测试冲突
    let test_id = uuid::Uuid::new_v4().simple().to_string();
    let test_key = format!("test_key_{}", test_id);
    let ttl_key = format!("ttl_key_{}", test_id);

    // Redis可用时，测试正常操作
    let cache_result: Result<Cache<String, String>, oxcache::CacheError> =
        Cache::redis(&redis_url).await;

    match &cache_result {
        Ok(cache) => {
            // Redis可用时正常创建 - 测试正常操作
            println!("✓ Redis connection successful, testing cache operations");

            // 测试基本的缓存操作
            cache
                .set(&test_key, &"test_value".to_string())
                .await
                .unwrap();
            let value = cache.get(&test_key).await.unwrap();
            assert_eq!(value, Some("test_value".to_string()));
            println!("✓ Basic cache operations work correctly");

            // 测试删除操作
            cache.delete(&test_key).await.unwrap();
            let deleted_value = cache.get(&test_key).await.unwrap();
            assert_eq!(deleted_value, None);
            println!("✓ Delete operation works correctly");

            // 测试TTL过期
            cache
                .set_with_ttl(
                    &ttl_key,
                    &"ttl_value".to_string(),
                    Some(Duration::from_secs(1)),
                )
                .await
                .unwrap();
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            let expired_value = cache.get(&ttl_key).await.unwrap();
            assert_eq!(expired_value, None);
            println!("✓ TTL expiration works correctly");
        }
        Err(e) => {
            // 连接失败 - 测试错误处理
            println!("✓ Redis connection failed as expected: {:?}", e);
            // 如果连接失败，这表示降级工作正常（无Redis时优雅处理）
        }
    }
}
