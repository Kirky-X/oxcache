// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据分级装饰器集成测试（classification feature）

#![cfg(feature = "classification")]

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::CacheBackend;
use oxcache::backend::DashMapMemoryBackend;
use oxcache::cache::CacheBuilder;
use oxcache::features::classification::{ClassifyingBackend, RuleSetClassifier, Sensitivity};
#[cfg(feature = "integrity")]
use oxcache::features::encryption::integrity::IntegrityBackend;

fn classifier() -> RuleSetClassifier {
    RuleSetClassifier::new(Sensitivity::Public)
        .with_rule("credential:*", Sensitivity::Sensitive)
        .with_sensitive_ttl_cap(Duration::from_secs(60))
}

#[tokio::test]
async fn credential_key_roundtrip_via_backend_arc() {
    let raw: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let decor = Arc::new(ClassifyingBackend::new(raw, classifier(), [3u8; 32]));
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .backend_arc(decor)
        .build()
        .await
        .unwrap();

    cache
        .set(&"credential:token".to_string(), &"secret-value".to_string())
        .await
        .unwrap();
    assert_eq!(
        cache.get(&"credential:token".to_string()).await.unwrap(),
        Some("secret-value".to_string())
    );
    cache
        .set(&"public:key".to_string(), &"plain".to_string())
        .await
        .unwrap();
    assert_eq!(
        cache.get(&"public:key".to_string()).await.unwrap(),
        Some("plain".to_string())
    );
}

#[tokio::test]
async fn ttl_cap_applies_to_sensitive_entries() {
    let raw: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let decor = Arc::new(ClassifyingBackend::new(raw, classifier(), [3u8; 32]));
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .backend_arc(decor)
        .build()
        .await
        .unwrap();
    cache
        .set_with_ttl(
            &"credential:token".to_string(),
            &"secret".to_string(),
            Some(Duration::from_secs(3600)),
        )
        .await
        .unwrap();
    let ttl = cache.ttl(&"credential:token".to_string()).await.unwrap();
    assert!(ttl.is_some_and(|t| t <= Duration::from_secs(60)));
}

#[cfg(feature = "integrity")]
#[tokio::test]
async fn composed_chain_classifying_integrity_memory() {
    let raw: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let integrity: Arc<dyn CacheBackend> = Arc::new(IntegrityBackend::new(raw, [5u8; 32]));
    let classifying: Arc<dyn CacheBackend> =
        Arc::new(ClassifyingBackend::new(integrity, classifier(), [3u8; 32]));
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .backend_arc(classifying)
        .build()
        .await
        .unwrap();

    for k in ["credential:key", "public:key"] {
        cache
            .set(&k.to_string(), &"value".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get(&k.to_string()).await.unwrap(),
            Some("value".to_string()),
            "roundtrip failed for {k}"
        );
    }
}
