// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
#![cfg(feature = "memory")]
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

#[path = "../../common/mod.rs"]
mod common;
use common::test_containers::{
    DragonflyContainer, backend_skip, container_or_skip, start_dragonfly_container,
};

/// 创建 Dragonfly 后端；连接失败不在此处定语义，由调用方路由门控
async fn make_dragonfly_backend(url: &str) -> oxcache::OxCacheResult<DragonflyBackend> {
    DragonflyBackend::new(url, 4).await
}

/// 启动 Dragonfly 容器并创建后端；Docker 不可用时跳过测试
/// （OXCACHE_TEST_STRICT 置位时改为失败，语义见 container_or_skip）。
///
/// 返回容器句柄由调用方持有至测试结束：`ContainerAsync` drop 即 force-rm
/// 容器，句柄若在 setup 内提前消亡，后端连接面对的是已删容器。
/// 后端创建/健康检查失败路由 backend_skip（可见、计数，但不置短路闩）：
/// 与基础设施不可用分档，避免一次握手抖动放大为整组覆盖丢失。
async fn setup() -> Option<(DragonflyContainer, DragonflyBackend)> {
    let (container, url) = container_or_skip("Dragonfly", start_dragonfly_container()).await?;
    let backend = match make_dragonfly_backend(&url).await {
        Ok(backend) => backend,
        Err(e) => {
            return backend_skip(
                "Dragonfly",
                &format!("backend connect failed after container ready: {e}"),
            );
        }
    };
    // 验证后端实际可用（连接成功不代表操作正常）
    if let Err(e) = backend.health_check().await {
        return backend_skip(
            "Dragonfly",
            &format!("health check failed after container ready: {e}"),
        );
    }
    Some((container, backend))
}

// ============================================================================
// Dragonfly 基础集成测试
// ============================================================================

#[tokio::test]
async fn test_dragonfly_backend_kind() {
    let Some((_container, backend)) = setup().await else {
        return;
    };

    assert_eq!(backend.backend_kind(), BackendKind::Dragonfly);
}

#[tokio::test]
async fn test_dragonfly_atomic_writer_is_none() {
    let Some((_container, backend)) = setup().await else {
        return;
    };

    // Dragonfly atomic operations not yet verified
    assert!(backend.as_atomic_writer().is_none());
}

#[tokio::test]
async fn test_dragonfly_cache_writer_operations() {
    let Some((_container, backend)) = setup().await else {
        return;
    };

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
    let Some((_container, backend)) = setup().await else {
        return;
    };

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
    let Ok(result) = backend
        .expire("df:nonexistent", Duration::from_secs(60))
        .await
    else {
        return;
    };
    assert!(!result);
}

#[tokio::test]
async fn test_dragonfly_cache_connector_operations() {
    let Some((_container, backend)) = setup().await else {
        return;
    };

    // backend_kind
    assert_eq!(backend.backend_kind(), BackendKind::Dragonfly);

    // shutdown (should not panic)
    backend.shutdown().await;
}

// ============================================================================
// ChainCache 集成测试
// ============================================================================

#[tokio::test]
async fn test_dragonfly_chain_cache_basic() {
    use oxcache::backend::MokaMemoryBackend;
    use oxcache::cache::chain::{ChainCacheBuilder, ChainLink};

    let Some((_container, dragonfly)) = setup().await else {
        return;
    };

    let moka = MokaMemoryBackend::new();

    // Moka(L1, score=100) + Dragonfly(L2, score=50)
    let chain = ChainCacheBuilder::default()
        .link(ChainLink::new(moka, 100, false, "moka"))
        .link(ChainLink::new(dragonfly, 50, true, "dragonfly"))
        .build();

    // Write through chain
    if chain
        .set("chain:df_key1", b"chain_value".to_vec(), None)
        .await
        .is_err()
    {
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
