// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 命名空间边界守卫集成测试（namespace-guard feature）
//!
//! 覆盖 builder 注入守卫后的越界拒绝与命名空间内完整生命周期。

#![cfg(feature = "namespace-guard")]

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::{CacheBackend, CacheReader, CacheWriter};
use oxcache::cache::CacheBuilder;
use oxcache::error::OxCacheError;
use oxcache::features::namespace_guard::{NamespaceGuardBackend, NamespacePolicy};

fn value(v: &[u8]) -> Arc<Vec<u8>> {
    Arc::new(v.to_vec())
}

#[tokio::test]
async fn out_of_namespace_read_is_denied() {
    let inner = Arc::new(oxcache::backend::memory::dashmap::DashMapMemoryBackend::new());
    inner
        .set(Arc::from("tenant:other:key"), value(b"secret"), None)
        .await
        .unwrap();
    let guard = NamespaceGuardBackend::new(inner, "tenant:acme", NamespacePolicy::Isolated);
    let result = guard.get("tenant:other:key").await;
    assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
}

#[tokio::test]
async fn out_of_namespace_write_is_denied() {
    let inner = oxcache::backend::memory::dashmap::DashMapMemoryBackend::new();
    let guard =
        NamespaceGuardBackend::new(Arc::new(inner), "tenant:acme", NamespacePolicy::Isolated);
    let result = guard
        .set(Arc::from("tenant:other:key"), value(b"secret"), None)
        .await;
    assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
    assert!(matches!(
        guard.get("tenant:other:key").await,
        Err(OxCacheError::AccessDenied(_))
    ));
}

#[tokio::test]
async fn out_of_namespace_pattern_is_denied() {
    let inner = oxcache::backend::memory::dashmap::DashMapMemoryBackend::new();
    let guard =
        NamespaceGuardBackend::new(Arc::new(inner), "tenant:acme", NamespacePolicy::Isolated);
    let result = guard.keys("other:*").await;
    assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
    let result = CacheBackend::delete_by_pattern(&guard, "other:*").await;
    assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
}

#[tokio::test]
async fn in_namespace_full_lifecycle() {
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .namespace("tenant:acme", NamespacePolicy::Isolated)
        .build()
        .await
        .unwrap();

    let k = "tenant:acme:key".to_string();
    cache.set(&k, &"value".to_string()).await.unwrap();
    assert_eq!(cache.get(&k).await.unwrap(), Some("value".to_string()));
    cache.expire(&k, Duration::from_secs(60)).await.unwrap();
    assert!(cache.ttl(&k).await.unwrap().is_some());
    cache.delete(&k).await.unwrap();
    assert_eq!(cache.get(&k).await.unwrap(), None);
}
