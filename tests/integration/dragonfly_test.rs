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

/// 创建 Dragonfly 后端
async fn make_dragonfly_backend(url: &str) -> DragonflyBackend {
    set_allow_insecure();
    DragonflyBackend::new(url, 4)
        .await
        .expect("Failed to connect to Dragonfly")
}

// ============================================================================
// T019: Dragonfly 基础集成测试
// ============================================================================

#[tokio::test]
async fn test_dragonfly_backend_kind() {
    let container = DragonflyContainer::start().await.expect("Failed to start Dragonfly");
    container.wait_ready().await.expect("Dragonfly not ready");
    let backend = make_dragonfly_backend(&container.url()).await;

    assert_eq!(backend.backend_kind(), BackendKind::Dragonfly);
}

#[tokio::test]
async fn test_dragonfly_atomic_writer_is_none() {
    let container = DragonflyContainer::start().await.expect("Failed to start Dragonfly");
    container.wait_ready().await.expect("Dragonfly not ready");
    let backend = make_dragonfly_backend(&container.url()).await;

    // Dragonfly atomic operations not yet verified
    assert!(backend.as_atomic_writer().is_none());
}

#[tokio::test]
async fn test_dragonfly_cache_writer_operations() {
    let container = DragonflyContainer::start().await.expect("Failed to start Dragonfly");
    container.wait_ready().await.expect("Dragonfly not ready");
    let backend = make_dragonfly_backend(&container.url()).await;

    // set
    backend
        .set(Arc::from("df:key1"), Arc::new(b"value1".to_vec()), None)
        .await
        .expect("set failed");

    // set with TTL
    backend
        .set(
            Arc::from("df:key2"),
            Arc::new(b"value2".to_vec()),
            Some(Duration::from_secs(60)),
        )
        .await
        .expect("set with TTL failed");

    // set_many
    let items = vec![
        (Arc::from("df:batch1"), Arc::new(b"b1".to_vec()), None),
        (Arc::from("df:batch2"), Arc::new(b"b2".to_vec()), None),
    ];
    backend.set_many(&items).await.expect("set_many failed");

    // delete
    backend.delete("df:key1").await.expect("delete failed");

    // delete_many
    let keys = vec!["df:batch1".to_string(), "df:batch2".to_string()];
    backend.delete_many(&keys).await.expect("delete_many failed");
}

#[tokio::test]
async fn test_dragonfly_cache_reader_operations() {
    let container = DragonflyContainer::start().await.expect("Failed to start Dragonfly");
    container.wait_ready().await.expect("Dragonfly not ready");
    let backend = make_dragonfly_backend(&container.url()).await;

    // Setup data
    backend
        .set(Arc::from("df:read1"), Arc::new(b"hello".to_vec()), None)
        .await
        .unwrap();
    backend
        .set(
            Arc::from("df:read2"),
            Arc::new(b"world".to_vec()),
            Some(Duration::from_secs(120)),
        )
        .await
        .unwrap();

    // get
    let val = backend.get("df:read1").await.unwrap();
    assert_eq!(val, Some(b"hello".to_vec()));

    // get nonexistent
    let val = backend.get("df:nonexistent").await.unwrap();
    assert_eq!(val, None);

    // exists
    assert!(backend.exists("df:read1").await.unwrap());
    assert!(!backend.exists("df:nonexistent").await.unwrap());

    // ttl
    let ttl = backend.ttl("df:read2").await.unwrap();
    assert!(ttl.is_some());
    let ttl = ttl.unwrap();
    assert!(ttl > Duration::from_secs(100));

    // expire
    let result = backend.expire("df:read1", Duration::from_secs(60)).await.unwrap();
    assert!(result);

    // expire nonexistent
    let result = backend.expire("df:nonexistent", Duration::from_secs(60)).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_dragonfly_cache_connector_operations() {
    let container = DragonflyContainer::start().await.expect("Failed to start Dragonfly");
    container.wait_ready().await.expect("Dragonfly not ready");
    let backend = make_dragonfly_backend(&container.url()).await;

    // health_check
    backend.health_check().await.expect("health_check failed");

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

    let container = DragonflyContainer::start().await.expect("Failed to start Dragonfly");
    container.wait_ready().await.expect("Dragonfly not ready");
    let dragonfly = make_dragonfly_backend(&container.url()).await;

    let moka = MokaMemoryBackend::new();

    // Moka(L1, score=100) + Dragonfly(L2, score=50)
    let chain = ChainCacheBuilder::default()
        .link(ChainLink::new(moka, 100, false, "moka"))
        .link(ChainLink::new(dragonfly, 50, true, "dragonfly"))
        .build();

    // Write through chain
    chain
        .set("chain:df_key1", b"chain_value".to_vec(), None)
        .await
        .expect("chain set failed");

    // Read from chain
    let val = chain.get("chain:df_key1").await.unwrap();
    assert_eq!(val, Some(b"chain_value".to_vec()));

    // Health check
    chain.health_check().await.expect("chain health_check failed");
}
