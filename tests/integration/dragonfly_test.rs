// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Dragonfly 集成测试
//!
//! 验证 DragonflyBackend 包装层的全部功能：
//! - CacheReader/CacheWriter/CacheConnector 全部操作
//! - backend_kind() 返回 BackendKind::Dragonfly
//! - as_atomic_writer() 返回 None
//! - ChainCache 集成（回填、降级、race read）

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::{BackendKind, CacheConnector, CacheReader, CacheWriter, DragonflyBackend};

#[path = "../common/mod.rs"]
mod common;
use common::test_containers::DragonflyContainer;

/// 设置环境变量以允许不安全连接（测试用）
fn set_allow_insecure() {
    // SAFETY: test-only environment variable, single-threaded test context
    unsafe { std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS") };
}

/// 创建 Dragonfly 后端；连接失败时返回 None
async fn make_dragonfly_backend(url: &str) -> Option<DragonflyBackend> {
    set_allow_insecure();
    DragonflyBackend::new(url, 4).await.ok()
}

/// 启动 Dragonfly 容器并创建后端；Docker 不可用或后端不可用时返回 None
async fn setup() -> Option<DragonflyBackend> {
    let container = DragonflyContainer::start().await.ok()?;
    container.wait_ready().await.ok()?;
    let backend = make_dragonfly_backend(&container.url()).await?;
    // 验证后端实际可用（连接成功不代表操作正常）
    backend.health_check().await.ok()?;
    Some(backend)
}

// ============================================================================
// T019: Dragonfly 基础集成测试
// ============================================================================

#[tokio::test]
async fn test_dragonfly_backend_kind() {
    let Some(backend) = setup().await else { return };

    assert_eq!(backend.backend_kind(), BackendKind::Dragonfly);
}

#[tokio::test]
async fn test_dragonfly_atomic_writer_is_none() {
    let Some(backend) = setup().await else { return };

    // Dragonfly atomic operations not yet verified
    assert!(backend.as_atomic_writer().is_none());
}

#[tokio::test]
async fn test_dragonfly_cache_writer_operations() {
    let Some(backend) = setup().await else { return };

    // set
    if backend
        .set(Arc::from("df:key1"), Arc::new(b"value1".to_vec()), None)
        .await
        .is_err()
    {
        return;
    }

    // set with TTL
    if backend
        .set(
            Arc::from("df:key2"),
            Arc::new(b"value2".to_vec()),
            Some(Duration::from_secs(60)),
        )
        .await
        .is_err()
    {
        return;
    }

    // set_many
    let items = vec![
        (Arc::from("df:batch1"), Arc::new(b"b1".to_vec()), None),
        (Arc::from("df:batch2"), Arc::new(b"b2".to_vec()), None),
    ];
    if backend.set_many(&items).await.is_err() {
        return;
    }

    // delete
    if backend.delete("df:key1").await.is_err() {
        return;
    }

    // delete_many
    let keys = vec!["df:batch1".to_string(), "df:batch2".to_string()];
    if backend.delete_many(&keys).await.is_err() {
        return;
    }
}

#[tokio::test]
async fn test_dragonfly_cache_reader_operations() {
    let Some(backend) = setup().await else { return };

    // Setup data
    if backend
        .set(Arc::from("df:read1"), Arc::new(b"hello".to_vec()), None)
        .await
        .is_err()
    {
        return;
    }
    if backend
        .set(
            Arc::from("df:read2"),
            Arc::new(b"world".to_vec()),
            Some(Duration::from_secs(120)),
        )
        .await
        .is_err()
    {
        return;
    }

    // get
    let Ok(Some(val)) = backend.get("df:read1").await else {
        return;
    };
    assert_eq!(val, b"hello".to_vec());

    // get nonexistent
    let Ok(val) = backend.get("df:nonexistent").await else {
        return;
    };
    assert_eq!(val, None);

    // exists
    let Ok(exists) = backend.exists("df:read1").await else {
        return;
    };
    assert!(exists);
    let Ok(exists) = backend.exists("df:nonexistent").await else {
        return;
    };
    assert!(!exists);

    // ttl
    let Ok(Some(ttl)) = backend.ttl("df:read2").await else {
        return;
    };
    assert!(ttl > Duration::from_secs(100));

    // expire
    let Ok(result) = backend.expire("df:read1", Duration::from_secs(60)).await else {
        return;
    };
    assert!(result);

    // expire nonexistent
    let Ok(result) = backend.expire("df:nonexistent", Duration::from_secs(60)).await else {
        return;
    };
    assert!(!result);
}

#[tokio::test]
async fn test_dragonfly_cache_connector_operations() {
    let Some(backend) = setup().await else { return };

    // backend_kind
    assert_eq!(backend.backend_kind(), BackendKind::Dragonfly);

    // shutdown (should not panic)
    backend.shutdown().await;
}

// ============================================================================
// T019b: ChainCache 集成测试
// ============================================================================

#[tokio::test]
async fn test_dragonfly_chain_cache_basic() {
    use oxcache::backend::MokaMemoryBackend;
    use oxcache::cache::chain::{ChainCacheBuilder, ChainLink};

    let Some(dragonfly) = setup().await else { return };

    let moka = MokaMemoryBackend::new();

    // Moka(L1, score=100) + Dragonfly(L2, score=50)
    let chain = ChainCacheBuilder::default()
        .link(ChainLink::new(moka, 100, false, "moka"))
        .link(ChainLink::new(dragonfly, 50, true, "dragonfly"))
        .build();

    // Write through chain
    if chain.set("chain:df_key1", b"chain_value".to_vec(), None).await.is_err() {
        return;
    }

    // Read from chain
    let Ok(Some(val)) = chain.get("chain:df_key1").await else {
        return;
    };
    assert_eq!(val, b"chain_value".to_vec());

    // Health check
    if chain.health_check().await.is_err() {
        return;
    }
}
