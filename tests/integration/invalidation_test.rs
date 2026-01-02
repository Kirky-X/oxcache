//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存失效集成测试

#[path = "../common/mod.rs"]
mod common;

use futures::stream::StreamExt;
use oxcache::config::{
    CacheType, Config, GlobalConfig, InvalidationChannelConfig, L1Config, L2Config, RedisMode,
    ServiceConfig, TwoLevelConfig,
};
use oxcache::CacheManager;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

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

    let service_name = common::generate_unique_service_name("invalidation_test");
    let redis_url = "redis://127.0.0.1:6379";
    let channel_name = format!("cache:invalidate:{}", service_name);

    // 1. 创建两个独立的 CacheManager 实例，模拟两个不同的服务进程
    // 关键点：为了模拟两个实例，我们需要两个独立的 CacheManager，
    // 但由于 CacheManager 是全局单例，我们不能直接创建两个。
    //
    // 解决方案：
    // a) 在测试中，我们手动创建两个 client，让它们共享相同的 service_name 和配置，
    //    但它们是不同的实例，可以模拟多实例行为。
    // b) 更好的方法是，利用 CacheManager 支持多服务的特性，创建两个服务，
    //    但我们需要让它们监听同一个失效频道。这需要修改源码以支持自定义频道名。
    //
    // 为了不修改源码，我们采用一种混合方法：
    // - 初始化一个服务 (client1)
    // - 手动创建一个独立的 Redis 订阅者，模拟第二个实例的监听器。
    // - client1 执行删除操作，验证订阅者能收到消息。
    // - 独立的 Redis 发布者发送消息，验证 client1 的 L1 缓存被清除。

    let config = Config {
        config_version: Some(1),
        global: GlobalConfig::default(),
        services: {
            let mut map = HashMap::new();
            map.insert(
                service_name.clone(),
                ServiceConfig {
                    cache_type: CacheType::TwoLevel,
                    ttl: Some(60),
                    serialization: None,
                    l1: Some(L1Config {
                        max_capacity: 1000,
                        cleanup_interval_secs: 30, // 必须小于 TTL (60)
                        ..Default::default()
                    }),
                    l2: Some(L2Config {
                        mode: RedisMode::Standalone,
                        connection_string: redis_url.to_string().into(),
                        ..Default::default()
                    }),
                    two_level: Some(TwoLevelConfig {
                        invalidation_channel: Some(InvalidationChannelConfig::Custom(
                            channel_name.clone(),
                        )),
                        promote_on_hit: true,
                        enable_batch_write: false,
                        batch_size: 100,
                        batch_interval_ms: 50,
                        bloom_filter: None,
                        warmup: None,
                        max_key_length: Some(1024),
                        max_value_size: Some(1024 * 1024),
                    }),
                },
            );
            map
        },
    };

    // 初始化 CacheManager
    CacheManager::reset();
    CacheManager::init(config)
        .await
        .expect("CacheManager init failed");
    let client1 = oxcache::get_client(&service_name).expect("Failed to get client1");

    // 2. 准备一个独立的 Redis 客户端来模拟“另一个实例”的 Pub/Sub
    let redis_client = redis::Client::open(redis_url).expect("Failed to create redis client");
    #[allow(deprecated)]
    let mut pubsub_conn = redis_client
        .get_async_connection()
        .await
        .expect("Failed to get pubsub connection")
        .into_pubsub();

    // --- 场景1: 验证删除操作会发布失效消息 ---
    pubsub_conn
        .subscribe(&channel_name)
        .await
        .expect("Failed to subscribe");

    let key_to_delete = "key_to_delete";
    client1
        .set_bytes(key_to_delete, vec![1, 2, 3], None)
        .await
        .expect("Set failed");
    client1.delete(key_to_delete).await.expect("Delete failed");

    // 验证收到的消息
    let mut stream = pubsub_conn.on_message();
    let msg = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("Timeout waiting for pubsub message")
        .expect("Stream ended unexpectedly");

    let payload: String = msg.get_payload().expect("Failed to get payload");
    assert_eq!(
        payload, key_to_delete,
        "Should receive invalidation for deleted key"
    );

    drop(stream);
    pubsub_conn
        .unsubscribe(&channel_name)
        .await
        .expect("Failed to unsubscribe");

    // --- 场景2: 验证接收失效消息会清除 L1 缓存 ---
    let key_to_invalidate = "key_to_invalidate";

    // 先在 client1 中设置值，确保 L1 和 L2 都有数据
    client1
        .set_bytes(key_to_invalidate, vec![4, 5, 6], None)
        .await
        .expect("Set failed");

    // 确认数据存在
    assert!(client1
        .get_bytes(key_to_invalidate)
        .await
        .unwrap()
        .is_some());

    // 模拟另一个实例删除了 L2 的数据，并发送了失效消息
    let mut publish_conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .expect("Failed to get publish connection");
    redis::cmd("DEL")
        .arg(key_to_invalidate)
        .query_async::<i64>(&mut publish_conn)
        .await
        .expect("Failed to DEL key in L2");
    redis::cmd("PUBLISH")
        .arg(&channel_name)
        .arg(key_to_invalidate)
        .query_async::<i64>(&mut publish_conn)
        .await
        .expect("Failed to PUBLISH invalidation");

    // 等待消息处理
    sleep(Duration::from_millis(500)).await;

    // 再次获取，此时 L1 应该被清除，L2 也无数据，所以结果应为 None
    let result = client1
        .get_bytes(key_to_invalidate)
        .await
        .expect("Get failed");
    assert!(
        result.is_none(),
        "Key should be invalidated from L1 and not found in L2"
    );

    common::cleanup_service(&service_name).await;
}
