// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 来源约束装饰器集成测试（provenance feature）

#![cfg(feature = "provenance")]

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oxcache::backend::DashMapMemoryBackend;
use oxcache::backend::{CacheBackend, CacheReader};
use oxcache::cache::CacheBuilder;
#[cfg(all(feature = "classification", feature = "integrity"))]
use oxcache::features::IntegrityBackend;
#[cfg(feature = "classification")]
use oxcache::features::classification::{ClassifyingBackend, RuleSetClassifier, Sensitivity};
use oxcache::features::provenance::{Provenance, ProvenanceBackend, ProvenancePolicy};

fn provenance(source: &str, at: SystemTime) -> Provenance {
    Provenance {
        source: source.to_string(),
        fetched_at: at,
        max_staleness: Duration::from_secs(30),
    }
}

#[tokio::test]
async fn provenance_write_then_read_success() {
    let raw: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let decor = Arc::new(ProvenanceBackend::new(
        raw,
        ProvenancePolicy {
            trusted_sources: vec!["user-service".to_string()],
            reject_unprovened: true,
        },
    ));
    decor
        .set_with_provenance(
            Arc::from("k"),
            Arc::new(b"fresh".to_vec()),
            Some(Duration::from_secs(3600)),
            provenance("user-service", SystemTime::now()),
        )
        .await
        .unwrap();
    assert_eq!(decor.get("k").await.unwrap(), Some(b"fresh".to_vec()));
}

#[tokio::test]
async fn stale_provenance_is_rejected_and_key_deleted() {
    let raw: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let decor = Arc::new(ProvenanceBackend::new(
        raw,
        ProvenancePolicy {
            trusted_sources: vec!["user-service".to_string()],
            reject_unprovened: true,
        },
    ));
    let old = UNIX_EPOCH + Duration::from_secs(1);
    decor
        .set_with_provenance(
            Arc::from("k"),
            Arc::new(b"stale".to_vec()),
            Some(Duration::from_secs(3600)),
            provenance("user-service", old),
        )
        .await
        .unwrap();
    assert_eq!(decor.get("k").await.unwrap(), None);
    assert!(!decor.exists("k").await.unwrap());
}

#[tokio::test]
async fn regular_set_is_rejected_when_policy_denies_unprovened() {
    let raw: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let decor = Arc::new(ProvenanceBackend::new(
        raw,
        ProvenancePolicy {
            trusted_sources: vec!["user-service".to_string()],
            reject_unprovened: true,
        },
    ));
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .backend_arc(decor)
        .build()
        .await
        .unwrap();
    cache
        .set(&"k".to_string(), &"unprovened".to_string())
        .await
        .unwrap();
    assert_eq!(cache.get(&"k".to_string()).await.unwrap(), None);
}

#[cfg(all(feature = "classification", feature = "integrity"))]
#[tokio::test]
async fn composed_chain_classifying_integrity_provenance_memory() {
    let raw: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let prov: Arc<dyn CacheBackend> = Arc::new(ProvenanceBackend::new(
        raw.clone(),
        ProvenancePolicy {
            trusted_sources: vec!["user-service".to_string()],
            reject_unprovened: false,
        },
    ));
    let integrity: Arc<dyn CacheBackend> = Arc::new(IntegrityBackend::new(prov.clone(), [5u8; 32]));
    let classifier = RuleSetClassifier::new(Sensitivity::Public);
    let classifying: Arc<dyn CacheBackend> = Arc::new(ClassifyingBackend::new(
        integrity.clone(),
        classifier,
        [3u8; 32],
    ));
    let cache: oxcache::Cache<String, String> = CacheBuilder::default()
        .backend_arc(classifying.clone())
        .build()
        .await
        .unwrap();
    cache
        .set(&"k".to_string(), &"value".to_string())
        .await
        .unwrap();
    assert_eq!(
        cache.get(&"k".to_string()).await.unwrap(),
        Some("value".to_string())
    );
}
