// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 双层缓存集成测试 - 真实 L1(Moka) + L2(Redis) 读写穿与回源链路

use std::sync::Arc;

use common::{get_redis_url_insecure, is_redis_available, setup_logging};
use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheBackend, CacheReader, CacheWriter};
use oxcache::cache::{ChainBuilder, L1Builder, L2Builder};

use crate::common;

/// 构建 L1(Moka) + L2(真实 Redis) 分层链，启用 L2→L1 回填。
async fn build_chain(l2: Arc<dyn CacheBackend>) -> oxcache::cache::ChainCache {
    ChainBuilder::new()
        .l1(L1Builder::new().capacity(128))
        .l2(L2Builder::new().custom(l2).persistent(true))
        .enable_backfill()
        .build()
        .await
        .expect("build L1+L2 chain")
}

/// 测试双层缓存流程
///
/// 验证 L1(Moka) + L2(Redis) 的真实读写穿与回源链路：
/// 1. set 写穿两级；2. L1 命中；3. L1 miss → L2 回源并回填 L1；
/// 4. 另一实例（独立 L1、共享 L2）经 L2 读穿；5. delete 两级失效。
#[tokio::test]
async fn test_two_level_cache_flow() {
    if !is_redis_available().await {
        return;
    }

    setup_logging();
    let redis_url = get_redis_url_insecure();
    let key = "flow_two_level:key1";

    let l2: Arc<dyn CacheBackend> = Arc::new(
        RedisBackend::new(&redis_url)
            .await
            .expect("connect L2 redis"),
    );
    let chain = build_chain(l2.clone()).await;

    // 链序：score 降序 → [l1, l2]
    assert_eq!(chain.links()[0].name(), "l1");
    assert_eq!(chain.links()[1].name(), "l2");
    let l1 = chain.links()[0].backend().clone();
    let l2 = chain.links()[1].backend().clone();

    // 清理上次运行遗留
    chain.delete(key).await.unwrap();

    // 1. 写入：两级都写穿
    chain.set(key, b"value1".to_vec(), None).await.unwrap();
    assert_eq!(
        l1.get(key).await.unwrap(),
        Some(b"value1".to_vec()),
        "set 应写穿 L1"
    );
    assert_eq!(
        l2.get(key).await.unwrap(),
        Some(b"value1".to_vec()),
        "set 应写穿 L2(Redis)"
    );

    // 2. 立即读取应命中 L1，且不应改变层级内容
    assert_eq!(chain.get(key).await.unwrap(), Some(b"value1".to_vec()));

    // 3. 清空 L1（模拟本实例 L1 失效/新执行实例）：L2 回源 + 回填 L1
    l1.delete(key).await.unwrap();
    assert_eq!(l1.get(key).await.unwrap(), None, "L1 已清空");
    assert_eq!(
        chain.get(key).await.unwrap(),
        Some(b"value1".to_vec()),
        "L1 miss 应回源 L2"
    );
    assert_eq!(
        l1.get(key).await.unwrap(),
        Some(b"value1".to_vec()),
        "回源后应回填 L1"
    );

    // 4. 第二个链实例（独立 L1、共享 L2）：L1 miss → L2 读穿并回填新 L1
    let chain2 = build_chain(l2.clone()).await;
    let l1_2 = chain2.links()[0].backend().clone();
    assert_eq!(l1_2.get(key).await.unwrap(), None, "新实例 L1 初始为空");
    assert_eq!(
        chain2.get(key).await.unwrap(),
        Some(b"value1".to_vec()),
        "新实例应经 L2 读穿"
    );
    assert_eq!(
        l1_2.get(key).await.unwrap(),
        Some(b"value1".to_vec()),
        "读穿后应回填新实例 L1"
    );

    // 5. delete 两级失效
    chain.delete(key).await.unwrap();
    assert_eq!(l1.get(key).await.unwrap(), None, "delete 应清 L1");
    assert_eq!(l2.get(key).await.unwrap(), None, "delete 应清 L2");
    assert_eq!(chain.get(key).await.unwrap(), None);
}
