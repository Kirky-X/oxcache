// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 缓存失效集成测试：真实 Redis Pub/Sub 跨实例 L1 失效

#![cfg(all(
    feature = "redis",
    feature = "pubsub",
    feature = "invalidation",
    feature = "memory"
))]

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::common::{get_redis_url_insecure, is_redis_available, setup_logging};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheBackend, CacheReader, CacheWriter};
use oxcache::cache::{ChainBuilder, ChainCache, L1Builder, L2Builder};
use oxcache::features::invalidation::transport::RedisPubSubTransport;
use oxcache::features::invalidation::{InvalidationBus, InvalidationConfig};

/// 一个缓存实例：链（L1 Moka + L2 Redis，写路径失效广播）+ 本地 L1 句柄。
struct Instance {
    chain: ChainCache,
    l1: Arc<dyn CacheBackend>,
}

/// 构建实例：独立 L1、独立 Redis 连接、独立 instance_id，共用通道。
async fn build_instance(
    url: &str,
    instance_id: &str,
    channel: &str,
) -> (Instance, Arc<InvalidationBus>) {
    let l2: Arc<dyn CacheBackend> =
        Arc::new(RedisBackend::new(url).await.expect("connect L2 redis"));
    let transport = Arc::new(
        RedisPubSubTransport::new(url)
            .await
            .expect("create pubsub transport"),
    );
    let bus = Arc::new(InvalidationBus::new(
        transport,
        InvalidationConfig::new(instance_id).with_channel(channel),
    ));
    let chain = ChainBuilder::new()
        .l1(L1Builder::new().capacity(128))
        .l2(L2Builder::new().custom(l2).persistent(true))
        .enable_backfill()
        .with_invalidation(bus.clone())
        .build()
        .await
        .expect("build L1+L2 chain");
    let l1 = chain.links()[0].backend().clone();
    (Instance { chain, l1 }, bus)
}

/// 测试多实例失效
///
/// 验证一个客户端实例的写/删操作能否通过 Redis Pub/Sub
/// 使另一个客户端实例的 L1 缓存失效：
/// 1. A 写入 → B 经 L2 读穿并回填 B 的 L1；
/// 2. A 删除 → 广播 → B 的 L1 条目被失效，B 再读为 miss（L1+L2 都无）；
/// 3. 自失效豁免：A 自己广播的消息不清 A 的 L1。
#[tokio::test]
async fn test_multi_instance_invalidation() {
    setup_logging();

    if !is_redis_available().await {
        println!("[TEST-SKIP] test_multi_instance_invalidation: Redis not available");
        return;
    }

    let redis_url = get_redis_url_insecure();
    // 独立通道，避免与本进程其他测试/历史订阅者串扰
    let channel = format!("oxcache:it:invalidate:{}", uuid::Uuid::new_v4().simple());
    let key = "key_to_delete";

    let (instance_a, _bus_a) = build_instance(&redis_url, "instance-a", &channel).await;
    let (instance_b, bus_b) = build_instance(&redis_url, "instance-b", &channel).await;

    // B 订阅总线：A 的失效广播将删除 B 本地 L1 的对应键
    let listener_b = bus_b
        .spawn_listener(instance_b.l1.clone())
        .await
        .expect("spawn listener b");

    let _ = instance_a.chain.delete(key).await;

    // 1. A 写入：A 的 L1 保留（自豁免），B 未持有
    instance_a
        .chain
        .set(key, b"v1".to_vec(), None)
        .await
        .expect("A set");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        instance_a.l1.get(key).await.unwrap(),
        Some(b"v1".to_vec()),
        "self-broadcast invalidation messages must be exempted; A's L1 should remain"
    );
    assert_eq!(instance_b.l1.get(key).await.unwrap(), None);

    // 2. B 读穿 L2 并回填 B 的 L1
    assert_eq!(
        instance_b.chain.get(key).await.unwrap(),
        Some(b"v1".to_vec()),
        "B should read A's written value through L2"
    );
    assert_eq!(
        instance_b.l1.get(key).await.unwrap(),
        Some(b"v1".to_vec()),
        "B's L1 should have the entry after a read-through (for the later invalidation check)"
    );

    // 3. A 删除 → 经 Pub/Sub 使 B 的 L1 失效
    instance_a.chain.delete(key).await.expect("A delete");
    let deadline = Instant::now() + Duration::from_secs(5);
    while instance_b.l1.get(key).await.unwrap().is_some() {
        assert!(
            Instant::now() < deadline,
            "B's L1 was not invalidated by Pub/Sub within 5s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        instance_b.chain.get(key).await.unwrap(),
        None,
        "B should miss in L1 (already invalidated) and L2 should be deleted"
    );

    drop(listener_b);
}
