// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! StaleWhileRevalidateBackend 透传面与双时间戳重算覆盖：
//!
//! - `stats` 注入 `stale_ttl_ms`、`keys` 宽口径透传
//! - `expire` 重算逻辑/物理双时间戳（stale 窗口延长）
//! - `set_many` 逐项包装（None TTL 透传）
//! - `stale_ttl()` 构造值回读

#![cfg(feature = "full")]

use std::sync::Arc;
use std::time::Duration;

use oxcache::OxCacheError;
use oxcache::backend::memory::MokaMemoryBackend;
use oxcache::backend::{CacheConnector, CacheReader, CacheWriter};
use oxcache::features::stale::StaleWhileRevalidateBackend;

#[tokio::test]
async fn stats_embeds_stale_ttl_and_keys_pass_through() {
    let inner = Arc::new(MokaMemoryBackend::new());
    let stale = StaleWhileRevalidateBackend::new(inner.clone(), Duration::from_secs(90));
    assert_eq!(stale.stale_ttl(), Duration::from_secs(90));

    CacheWriter::set(&stale, Arc::from("u:1"), Arc::new(b"a".to_vec()), None)
        .await
        .unwrap();
    CacheWriter::set(&stale, Arc::from("s:1"), Arc::new(b"b".to_vec()), None)
        .await
        .unwrap();

    let stats = CacheReader::stats(&stale).await.unwrap();
    assert_eq!(stats.get("stale_ttl_ms").map(String::as_str), Some("90000"));

    // keys 宽口径：stale 窗口内条目也在列
    let keys = CacheReader::keys(&stale, "u:*").await.unwrap();
    assert!(keys.contains(&"u:1".to_string()), "got: {keys:?}");
    assert!(!keys.contains(&"s:1".to_string()));
}

#[tokio::test]
async fn expire_rewraps_double_timestamps() {
    let inner = Arc::new(MokaMemoryBackend::new());
    let stale = StaleWhileRevalidateBackend::new(inner.clone(), Duration::from_secs(60));

    CacheWriter::set(&stale, Arc::from("e"), Arc::new(b"v".to_vec()), None)
        .await
        .unwrap();
    // 命中：expire 重写双时间戳
    assert!(
        CacheWriter::expire(&stale, "e", Duration::from_secs(30))
            .await
            .unwrap()
    );
    // 未命中：false
    assert!(
        !CacheWriter::expire(&stale, "ghost", Duration::from_secs(30))
            .await
            .unwrap()
    );

    // 物理窗口 = 逻辑 TTL + stale_ttl，值仍可读
    assert_eq!(
        CacheReader::get(&stale, "e").await.unwrap().as_deref(),
        Some(&b"v"[..])
    );
}

#[tokio::test]
async fn set_many_wraps_each_item_and_preserves_none_ttl() {
    let inner = Arc::new(MokaMemoryBackend::new());
    let stale = StaleWhileRevalidateBackend::new(inner.clone(), Duration::from_secs(45));

    let items = vec![
        (
            Arc::from("m1"),
            Arc::new(b"with-ttl".to_vec()),
            Some(Duration::from_secs(20)),
        ),
        (Arc::from("m2"), Arc::new(b"no-ttl".to_vec()), None),
    ];
    CacheWriter::set_many(&stale, &items).await.unwrap();

    assert_eq!(
        CacheReader::get(&stale, "m1").await.unwrap().as_deref(),
        Some(&b"with-ttl"[..])
    );
    assert_eq!(
        CacheReader::get(&stale, "m2").await.unwrap().as_deref(),
        Some(&b"no-ttl"[..])
    );
}

#[tokio::test]
async fn connector_delegates_and_len_capacity_flow_through() {
    let inner = Arc::new(MokaMemoryBackend::new());
    let stale = StaleWhileRevalidateBackend::new(inner.clone(), Duration::from_secs(45));

    CacheWriter::set(&stale, Arc::from("c"), Arc::new(b"v".to_vec()), None)
        .await
        .unwrap();
    let _ = CacheReader::len(&stale).await.unwrap();
    let _ = CacheReader::capacity(&stale).await.unwrap();
    CacheConnector::health_check(&stale).await.unwrap();
    CacheConnector::shutdown(&stale).await;

    // 删除透传：删后 exists 为 false
    CacheWriter::delete(&stale, "c").await.unwrap();
    assert!(!CacheReader::exists(&stale, "c").await.unwrap());
}

// 错误类型锚定（文档化 OxCacheError 来源，防未使用导入告警漂移）
#[allow(unused)]
fn _err_anchor(_e: OxCacheError) {}
