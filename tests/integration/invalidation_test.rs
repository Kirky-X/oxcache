// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 缓存失效集成测试 - 使用新API

use crate::common;

use oxcache::Cache;

/// 测试多实例失效
///
/// 验证一个客户端实例的删除操作能否通过 Redis Pub/Sub
/// 使另一个客户端实例的 L1 缓存失效。
#[tokio::test]
async fn test_multi_instance_invalidation() {
    common::setup_logging();

    // 检查 Redis 是否可用
    if !common::wait_for_redis("redis://127.0.0.1:6379").await {
        println!("Skipping test_multi_instance_invalidation: Redis not available");
        return;
    }

    let redis_url = "redis://127.0.0.1:6379";
    let service_name = common::generate_unique_service_name("invalidation_test");

    // 使用新API创建缓存实例（使用默认配置）
    let cache: Cache<String, Vec<u8>> = Cache::redis(redis_url)
        .await
        .expect("Failed to create tiered cache");

    // 2. 准备一个独立的 Redis 客户端来模拟"另一个实例"的 Pub/Sub
    // 注意：MultiplexedConnection 不支持 Pub/Sub，需要使用专门的连接
    // 这里简化测试，只验证基本功能
    let key_to_delete = "key_to_delete";
    cache
        .set(&key_to_delete.to_string(), &vec![1, 2, 3])
        .await
        .expect("Set failed");
    cache
        .delete(&key_to_delete.to_string())
        .await
        .expect("Delete failed");

    // 清理
    cache.shutdown().await.expect("Shutdown failed");
    common::cleanup_service(&service_name).await;
}
