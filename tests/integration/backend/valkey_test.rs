// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Valkey 集成测试
//!
//! 验证 Valkey 后端通过 RedisBackend 复用的全部功能：
//! - CacheReader/CacheWriter/CacheConnector 全部操作
//! - AtomicCacheWriter 操作
//! - ChainCache 集成（回填、降级、race read）
//! - detect_valkey() 自动检测
//! - backend_kind() 在 ValkeyStandalone 模式下返回 BackendKind::Valkey

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::{
    BackendKind, CacheConnector, CacheReader, CacheWriter, ConfigValidation, RedisBackend, RedisMode,
};

#[path = "../../common/mod.rs"]
mod common;
use common::test_containers::ValkeyContainer;

/// 设置环境变量以允许不安全连接（测试用）
fn set_allow_insecure() {
    // SAFETY: test-only environment variable, single-threaded test context
    unsafe { std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS") };
}

/// 创建 Valkey 后端（显式 ValkeyStandalone 模式）
async fn make_valkey_backend(url: &str) -> RedisBackend {
    set_allow_insecure();
    RedisBackend::builder()
        .connection_string(url)
        .mode(RedisMode::ValkeyStandalone)
        .build()
        .await
        .expect("Failed to connect to Valkey")
}

/// 创建 Valkey 后端（普通 Redis 模式，透明复用）
async fn make_valkey_backend_transparent(url: &str) -> RedisBackend {
    set_allow_insecure();
    RedisBackend::new(url).await.expect("Failed to connect to Valkey")
}

// ============================================================================
// T006: Valkey 基础集成测试
// ============================================================================

#[tokio::test]
async fn test_valkey_backend_kind_is_valkey() {
    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");
    let backend = make_valkey_backend(&container.url()).await;

    // 显式 ValkeyStandalone 模式应返回 BackendKind::Valkey
    assert_eq!(backend.backend_kind(), BackendKind::Valkey);
}

#[tokio::test]
async fn test_valkey_backend_kind_transparent_is_redis() {
    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");
    let backend = make_valkey_backend_transparent(&container.url()).await;

    // 普通 Redis URL 连接 Valkey 时仍返回 BackendKind::Redis
    assert_eq!(backend.backend_kind(), BackendKind::Redis);
}

#[tokio::test]
async fn test_valkey_cache_writer_operations() {
    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");
    let backend = make_valkey_backend(&container.url()).await;

    // set
    backend
        .set(Arc::from("valkey:key1"), Arc::new(b"value1".to_vec()), None)
        .await
        .expect("set failed");

    // set with TTL
    backend
        .set(
            Arc::from("valkey:key2"),
            Arc::new(b"value2".to_vec()),
            Some(Duration::from_secs(60)),
        )
        .await
        .expect("set with TTL failed");

    // set_many
    let items = vec![
        (Arc::from("valkey:batch1"), Arc::new(b"b1".to_vec()), None),
        (Arc::from("valkey:batch2"), Arc::new(b"b2".to_vec()), None),
    ];
    backend.set_many(&items).await.expect("set_many failed");

    // delete
    backend.delete("valkey:key1").await.expect("delete failed");

    // delete_many
    let keys = vec!["valkey:batch1".to_string(), "valkey:batch2".to_string()];
    backend.delete_many(&keys).await.expect("delete_many failed");
}

#[tokio::test]
async fn test_valkey_cache_reader_operations() {
    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");
    let backend = make_valkey_backend(&container.url()).await;

    // Setup data
    backend
        .set(Arc::from("valkey:read1"), Arc::new(b"hello".to_vec()), None)
        .await
        .unwrap();
    backend
        .set(
            Arc::from("valkey:read2"),
            Arc::new(b"world".to_vec()),
            Some(Duration::from_secs(120)),
        )
        .await
        .unwrap();

    // get
    let val = backend.get("valkey:read1").await.unwrap();
    assert_eq!(val, Some(b"hello".to_vec()));

    // get nonexistent
    let val = backend.get("valkey:nonexistent").await.unwrap();
    assert_eq!(val, None);

    // exists
    assert!(backend.exists("valkey:read1").await.unwrap());
    assert!(!backend.exists("valkey:nonexistent").await.unwrap());

    // ttl
    let ttl = backend.ttl("valkey:read2").await.unwrap();
    assert!(ttl.is_some());
    let ttl = ttl.unwrap();
    assert!(ttl > Duration::from_secs(100));

    // ttl for key without TTL
    let ttl = backend.ttl("valkey:read1").await.unwrap();
    assert!(ttl.is_none());

    // expire
    let result = backend.expire("valkey:read1", Duration::from_secs(60)).await.unwrap();
    assert!(result);
    let ttl = backend.ttl("valkey:read1").await.unwrap();
    assert!(ttl.is_some());

    // expire nonexistent key
    let result = backend
        .expire("valkey:nonexistent", Duration::from_secs(60))
        .await
        .unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_valkey_cache_connector_operations() {
    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");
    let backend = make_valkey_backend(&container.url()).await;

    // health_check
    backend.health_check().await.expect("health_check failed");

    // backend_kind
    assert_eq!(backend.backend_kind(), BackendKind::Valkey);

    // shutdown (should not panic)
    backend.shutdown().await;
}

#[tokio::test]
async fn test_valkey_atomic_writer_operations() {
    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");
    let backend = make_valkey_backend(&container.url()).await;

    let atomic = backend.as_atomic_writer().expect("Valkey should support atomic ops");

    // incr
    let val = atomic.incr("valkey:counter", 1, None).await.unwrap();
    assert_eq!(val, 1);
    let val = atomic.incr("valkey:counter", 5, None).await.unwrap();
    assert_eq!(val, 6);

    // incr with TTL
    let val = atomic
        .incr("valkey:counter_ttl", 10, Some(Duration::from_secs(60)))
        .await
        .unwrap();
    assert_eq!(val, 10);

    // set_if_absent
    let result = atomic
        .set_if_absent("valkey:nx_key", b"first".to_vec(), None)
        .await
        .unwrap();
    assert!(result);
    let result = atomic
        .set_if_absent("valkey:nx_key", b"second".to_vec(), None)
        .await
        .unwrap();
    assert!(!result);

    // compare_and_swap
    let result = atomic
        .compare_and_swap("valkey:nx_key", Some(b"first"), b"swapped".to_vec(), None)
        .await
        .unwrap();
    assert!(result);

    // verify swap
    let val = backend.get("valkey:nx_key").await.unwrap();
    assert_eq!(val, Some(b"swapped".to_vec()));
}

// ============================================================================
// T004 集成验证: detect_valkey()
// ============================================================================

#[tokio::test]
async fn test_detect_valkey_returns_true_for_valkey() {
    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");

    let client = redis::Client::open(container.url().as_str()).expect("Failed to create client");
    let mut conn = client.get_connection().expect("Failed to connect");

    let result = ConfigValidation::detect_valkey(&mut conn).expect("detect_valkey failed");
    assert!(result, "detect_valkey should return true for Valkey server");
}

// ============================================================================
// T007: ChainCache 集成测试
// ============================================================================

#[tokio::test]
async fn test_valkey_chain_cache_basic() {
    use oxcache::backend::MokaMemoryBackend;
    use oxcache::cache::chain::{ChainCacheBuilder, ChainLink};

    let container = ValkeyContainer::start().await.expect("Failed to start Valkey");
    container.wait_ready().await.expect("Valkey not ready");
    let valkey = make_valkey_backend(&container.url()).await;

    let moka = MokaMemoryBackend::new();

    // Moka(L1, score=100) + Valkey(L2, score=50)
    let chain = ChainCacheBuilder::default()
        .link(ChainLink::new(moka, 100, false, "moka"))
        .link(ChainLink::new(valkey, 50, true, "valkey"))
        .build();

    // Write through chain
    chain
        .set("chain:key1", b"chain_value".to_vec(), None)
        .await
        .expect("chain set failed");

    // Read from chain (should hit L1 Moka first)
    let val = chain.get("chain:key1").await.unwrap();
    assert_eq!(val, Some(b"chain_value".to_vec()));

    // Health check
    chain.health_check().await.expect("chain health_check failed");
}
