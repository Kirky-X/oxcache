// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `BloomFilterBackend` 面覆盖测试：builder 构造校验、prefill 预热、async 与
//! sync 双 trait 面（BF miss 短路 / 命中透传 / stats 注入 bloom_* 指标）。
//! 此前仅内联 SpyMock 测试覆盖 async 路径，sync 面与 prefill/builder 无测试。
//!
//! inner 采用真实内存后端 DashMapMemoryBackend（双面实现），验证行为而非
//! 查询计数：BF miss 短路的可观测结果是「未插入集合的 key 查不到后端已有
//! 条目」，经 prefill 后恢复可见。

#![cfg(all(feature = "bloom", feature = "memory"))]

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::memory::DashMapMemoryBackend;
use oxcache::backend::{
    BackendKind, BackendScore, CacheConnector, CacheReader, CacheSetItem, CacheWriter, Scores,
    SyncCacheConnector, SyncCacheReader, SyncCacheWriter,
};
use oxcache::features::bloom_filter::BloomFilterBackend;

fn dashmap_backend() -> DashMapMemoryBackend {
    DashMapMemoryBackend::default()
}

fn key(k: &str) -> Arc<str> {
    Arc::from(k)
}

fn value(v: &[u8]) -> Arc<Vec<u8>> {
    Arc::new(v.to_vec())
}

// ============================================================================
// 构造面：new / with_capacity_and_rate / builder / 访问器
// ============================================================================

#[tokio::test]
async fn constructors_and_accessors() {
    let inner = dashmap_backend();

    // 默认构造
    let b1 = BloomFilterBackend::new(inner.clone());
    assert_eq!(b1.bloom().capacity(), 100_000);
    let _ = b1.inner();

    // 显式容量与误判率
    let b2 = BloomFilterBackend::with_capacity_and_rate(inner, 1_000, 0.05);
    assert_eq!(b2.bloom().capacity(), 1_000);
    assert!((b2.bloom().false_positive_rate() - 0.05).abs() < 1e-9);

    // builder 缺 inner → 显性 Err
    let refused = BloomFilterBackend::<DashMapMemoryBackend>::builder()
        .capacity(500)
        .false_positive_rate(0.02)
        .build();
    let err = match refused {
        Err(e) => e,
        Ok(_) => panic!("缺 inner backend 的 builder 必须被拒绝"),
    };
    assert!(err.to_string().contains("inner backend is required"));

    // builder 完整路径
    let b3 = BloomFilterBackend::builder()
        .capacity(500)
        .false_positive_rate(0.02)
        .inner(dashmap_backend())
        .build()
        .unwrap();
    assert_eq!(b3.bloom().capacity(), 500);
    assert!((b3.bloom().false_positive_rate() - 0.02).abs() < 1e-9);
}

// ============================================================================
// prefill 预热：对共享后端已存在但不在过滤器内的 key 恢复可见
// ============================================================================

#[tokio::test]
async fn prefill_from_keys_and_backend() {
    let inner = dashmap_backend();
    // 后端已有条目（模拟重启前写入）
    CacheWriter::set(&inner, key("warm:1"), value(b"v1"), None)
        .await
        .unwrap();
    CacheWriter::set(&inner, key("warm:2"), value(b"v2"), None)
        .await
        .unwrap();

    let bf = BloomFilterBackend::new(inner.clone());

    // 未预热：BF miss → 短路返回 None，即使后端实际有值
    assert_eq!(CacheReader::get(&bf, "warm:1").await.unwrap(), None);
    assert!(!CacheReader::exists(&bf, "warm:1").await.unwrap());
    assert_eq!(CacheReader::ttl(&bf, "warm:1").await.unwrap(), None);

    // prefill_from_keys 灌入已知 key
    bf.prefill_from_keys(["warm:1", "warm:2"]);
    assert_eq!(
        CacheReader::get(&bf, "warm:1").await.unwrap().as_deref(),
        Some(&b"v1"[..])
    );
    assert!(CacheReader::exists(&bf, "warm:2").await.unwrap());

    // prefill_from_backend：按 pattern 回灌并返回回灌数量
    let bf2 = BloomFilterBackend::new(inner.clone());
    let count = bf2.prefill_from_backend("warm:*").await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        CacheReader::get(&bf2, "warm:2").await.unwrap().as_deref(),
        Some(&b"v2"[..])
    );
}

// ============================================================================
// async trait 面：BF miss 短路 / 命中透传 / stats 指标注入
// ============================================================================

#[tokio::test]
async fn async_trait_surface_with_bloom_semantics() {
    let bf = BloomFilterBackend::new(dashmap_backend());

    // set 成功后进入过滤器 → get/exists/ttl 透传
    CacheWriter::set(&bf, key("k"), value(b"v"), Some(Duration::from_secs(30)))
        .await
        .unwrap();
    assert_eq!(
        CacheReader::get(&bf, "k").await.unwrap().as_deref(),
        Some(&b"v"[..])
    );
    assert!(CacheReader::exists(&bf, "k").await.unwrap());
    assert!(CacheReader::ttl(&bf, "k").await.unwrap().is_some());
    // 未插入 key：BF miss → ttl 明确 None
    assert_eq!(CacheReader::ttl(&bf, "ghost").await.unwrap(), None);

    // len/capacity 透传
    assert_eq!(CacheReader::len(&bf).await.unwrap(), 1);
    let cap = CacheReader::capacity(&bf).await.unwrap();
    let _ = cap;

    // stats 注入 bloom_* 指标
    let stats = CacheReader::stats(&bf).await.unwrap();
    assert!(stats.contains_key("bloom_capacity"));
    assert!(stats.contains_key("bloom_load_factor"));
    assert!(stats.contains_key("bloom_false_positive_rate"));
    assert!(stats.contains_key("bloom_estimated_count"));

    // set_many 透传（不进过滤器——批量路径无 BF 更新语义）
    let items: Vec<CacheSetItem> = vec![(key("m"), value(b"mv"), None)];
    CacheWriter::set_many(&bf, &items).await.unwrap();

    // delete 透传（BF 不支持删除，不误改）
    CacheWriter::delete(&bf, "k").await.unwrap();
    assert!(!CacheReader::exists(&bf, "k").await.unwrap());

    // expire 透传
    assert!(
        CacheWriter::expire(&bf, "m", Duration::from_secs(5))
            .await
            .unwrap()
    );

    // health/shutdown/kind/score 透传
    CacheConnector::health_check(&bf).await.unwrap();
    CacheConnector::shutdown(&bf).await;
    assert_eq!(CacheConnector::backend_kind(&bf), BackendKind::DashMap);
    assert_eq!(BackendScore::score(&bf), Scores::DASHMAP);
    assert_eq!(BackendScore::backend_name(&bf), "dashmap");

    // clear 透传并清空过滤器
    CacheWriter::clear(&bf).await.unwrap();
    assert_eq!(CacheReader::len(&bf).await.unwrap(), 0);
    assert_eq!(bf.bloom().len(), 0);
}

// ============================================================================
// sync trait 面（B: CacheBackend + SyncCacheBackend）
// ============================================================================

#[test]
fn sync_trait_surface_with_bloom_semantics() {
    let bf = BloomFilterBackend::new(dashmap_backend());

    // sync set → 过滤器更新 → sync get/exists 透传
    SyncCacheWriter::set(&bf, key("s"), value(b"sv"), None).unwrap();
    assert_eq!(
        SyncCacheReader::get(&bf, "s").unwrap().as_deref(),
        Some(&b"sv"[..])
    );
    assert!(SyncCacheReader::exists(&bf, "s").unwrap());

    // BF miss → 短路
    assert_eq!(SyncCacheReader::get(&bf, "ghost").unwrap(), None);
    assert!(!SyncCacheReader::exists(&bf, "ghost").unwrap());
    assert_eq!(SyncCacheReader::ttl(&bf, "ghost").unwrap(), None);

    // len/capacity/stats 透传 + bloom_* 注入
    assert_eq!(SyncCacheReader::len(&bf).unwrap(), 1);
    let _ = SyncCacheReader::capacity(&bf).unwrap();
    let stats = SyncCacheReader::stats(&bf).unwrap();
    assert!(stats.contains_key("bloom_capacity"));
    assert!(stats.contains_key("bloom_estimated_count"));

    // set_many/delete_many 透传
    let items: Vec<CacheSetItem> = vec![(key("sm"), value(b"x"), None)];
    SyncCacheWriter::set_many(&bf, &items).unwrap();
    SyncCacheWriter::delete_many(&bf, &["sm".to_string()]).unwrap();

    // expire/delete 透传
    assert!(SyncCacheWriter::expire(&bf, "s", Duration::from_secs(5)).unwrap());
    SyncCacheWriter::delete(&bf, "s").unwrap();
    assert!(!SyncCacheReader::exists(&bf, "s").unwrap());

    // sync connector 透传
    SyncCacheConnector::health_check(&bf).unwrap();
    SyncCacheConnector::shutdown(&bf);
    assert_eq!(SyncCacheConnector::backend_kind(&bf), BackendKind::DashMap);

    // sync clear → 过滤器同步清空
    SyncCacheWriter::set(&bf, key("s2"), value(b"v2"), None).unwrap();
    SyncCacheWriter::clear(&bf).unwrap();
    assert_eq!(SyncCacheReader::len(&bf).unwrap(), 0);
}

/// 哈希函数数不可达时构造期显性 panic（不得静默退化精度）
#[test]
#[should_panic]
fn bloom_filter_unreachable_hash_count_panics() {
    // 容量 1 与 64 个哈希函数在数学上不可达 → 构造期必须 panic 而非静默降级
    let _: oxcache::features::BloomFilter<str> =
        oxcache::features::BloomFilter::new_with_hash_count(1, 0.5, 64);
}
