// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `RedisVersionedCache` 真实 Redis 端点测试（testcontainers 门控）
//!
//! WATCH + MULTI/EXEC 乐观锁语义（版本不匹配拒绝、信封编解码、TTL 分支）
//! 只有真实 Redis 才能验证。Docker 不可用、启动超时或容器无法就绪时跳过
//! （`OXCACHE_TEST_STRICT` 置位时改为失败，语义见 `container_or_skip`）。

#![cfg(all(feature = "redis", feature = "versioning"))]

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use oxcache::features::versioning::{RedisVersionedCache, VersionedStore};

#[path = "../../common/mod.rs"]
mod common;
use common::test_containers::{RedisContainer, container_or_skip, start_redis_container};

/// 就绪的 Redis 容器与后端；不可用时返回 None（调用方跳过测试）
async fn setup() -> Option<(RedisContainer, Arc<RedisBackend>)> {
    let (container, url) = container_or_skip("Redis(versioning)", start_redis_container()).await?;
    let backend = Arc::new(RedisBackend::new(&url).await.expect("连接 Redis 后端"));
    Some((container, backend))
}

/// 版本化 CAS 全流程：创建（版本 0）→ 递增 → 过期版本拒绝 → 删除重置版本。
#[tokio::test]
async fn versioned_cas_roundtrip_rejects_stale_writes() {
    let Some((_container, backend)) = setup().await else {
        return;
    };
    let store = RedisVersionedCache::new(backend.clone());

    assert_eq!(store.version("cas:key").await.unwrap(), None);

    // expect 0 = 创建语义
    assert_eq!(
        store
            .compare_and_swap("cas:key", 0, b"first".to_vec(), None)
            .await
            .unwrap(),
        Some(1)
    );
    let value = store.get_versioned("cas:key").await.unwrap().unwrap();
    assert_eq!(value.version, 1);
    assert_eq!(value.data, b"first".to_vec());
    assert_eq!(store.version("cas:key").await.unwrap(), Some(1));

    // 版本匹配则递增
    assert_eq!(
        store
            .compare_and_swap(
                "cas:key",
                1,
                b"second".to_vec(),
                Some(Duration::from_secs(30))
            )
            .await
            .unwrap(),
        Some(2),
        "带 TTL 的写入应成功"
    );
    let ttl = backend.ttl("cas:key").await.unwrap();
    assert!(
        ttl.is_some_and(|d| d <= Duration::from_secs(30)),
        "TTL 分支应写入过期时间，got {ttl:?}"
    );

    // 过期版本（含更早版本与「期望不存在」）必须被拒绝，且不得脏写
    assert_eq!(
        store
            .compare_and_swap("cas:key", 1, b"stale".to_vec(), None)
            .await
            .unwrap(),
        None,
        "过期版本必须被拒绝"
    );
    assert_eq!(
        store
            .compare_and_swap("cas:key", 0, b"recreate".to_vec(), None)
            .await
            .unwrap(),
        None,
        "已存在键不得以版本 0 重建"
    );
    assert_eq!(
        store.get_versioned("cas:key").await.unwrap().unwrap().data,
        b"second".to_vec(),
        "被拒绝的写入不得落地"
    );

    store.delete("cas:key").await.unwrap();
    assert_eq!(store.version("cas:key").await.unwrap(), None);
    assert_eq!(
        store
            .compare_and_swap("cas:key", 0, b"fresh".to_vec(), None)
            .await
            .unwrap(),
        Some(1),
        "删除后应可以版本 0 重建"
    );
}

/// 缺失键：`get_versioned` 返回 None，`delete` 幂等成功。
#[tokio::test]
async fn versioned_missing_key_reads_none_and_delete_is_idempotent() {
    let Some((_container, backend)) = setup().await else {
        return;
    };
    let store = RedisVersionedCache::new(backend);

    assert!(store.get_versioned("absent:key").await.unwrap().is_none());
    assert_eq!(store.version("absent:key").await.unwrap(), None);
    store.delete("absent:key").await.unwrap();
    store.delete("absent:key").await.unwrap();
}

/// 信封长度不足（非本装饰器写入的裸值）必须报错而非按越界数据解码。
#[tokio::test]
async fn versioned_decode_rejects_short_envelope() {
    let Some((_container, backend)) = setup().await else {
        return;
    };
    backend
        .set(Arc::from("raw:key"), Arc::new(b"abc".to_vec()), None)
        .await
        .unwrap();

    let store = RedisVersionedCache::new(backend);
    let err = store
        .version("raw:key")
        .await
        .expect_err("截断信封必须报错");
    assert!(
        err.to_string().contains("envelope too short"),
        "应命中长度校验, got {err}"
    );
    assert!(store.get_versioned("raw:key").await.is_err());
}
