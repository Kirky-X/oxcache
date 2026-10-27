// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 临时执行探针（E2E 场景 22）：`ChainCache::iter_entries` 层失败语义 ——
//! 某层 `get_many` 整批失败时错误按键拆分（每键一条错误事件 + warn）、
//! 键继续降级到下一层、未命中以 `None` 结尾、方法不返回 `Err`。
//!
//! 依据：`src/cache/chain.rs:429-484`（实现）、`docs/API_REFERENCE.md:604-617`（契约）。

#![cfg(feature = "memory")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use oxcache::backend::{BackendKind, BackendScore, CacheConnector, CacheReader, CacheWriter};
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::{CacheEvent, EventPublisher, OxCacheError};

/// 只在 `get_many` 上整批失败的故障层（`get` 不可达：批量入口单独注入故障，
/// 以精确观察 `iter_entries` 的按键拆分行为）。
struct FaultyManyLayer {
    fail: bool,
}

#[async_trait]
impl CacheReader for FaultyManyLayer {
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>, OxCacheError> {
        Ok(None)
    }
    async fn exists(&self, _key: &str) -> Result<bool, OxCacheError> {
        Ok(false)
    }
    async fn ttl(&self, _key: &str) -> Result<Option<Duration>, OxCacheError> {
        Ok(None)
    }
    async fn len(&self) -> Result<u64, OxCacheError> {
        Ok(0)
    }
    async fn capacity(&self) -> Result<u64, OxCacheError> {
        Ok(0)
    }
    async fn stats(&self) -> Result<HashMap<String, String>, OxCacheError> {
        Ok(HashMap::new())
    }
    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>, OxCacheError> {
        if self.fail {
            return Err(OxCacheError::Operation(
                "injected get_many failure".to_string(),
            ));
        }
        Ok(keys.iter().map(|_| None).collect())
    }
}

#[async_trait]
impl CacheWriter for FaultyManyLayer {
    async fn set(
        &self,
        _key: Arc<str>,
        _value: Arc<Vec<u8>>,
        _ttl: Option<Duration>,
    ) -> Result<(), OxCacheError> {
        Ok(())
    }
    async fn delete(&self, _key: &str) -> Result<(), OxCacheError> {
        Ok(())
    }
    async fn clear(&self) -> Result<(), OxCacheError> {
        Ok(())
    }
    async fn expire(&self, _key: &str, _ttl: Duration) -> Result<bool, OxCacheError> {
        Ok(false)
    }
}

#[async_trait]
impl CacheConnector for FaultyManyLayer {
    async fn health_check(&self) -> Result<(), OxCacheError> {
        Ok(())
    }
    async fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Mock
    }
}

impl BackendScore for FaultyManyLayer {
    fn score(&self) -> u8 {
        100
    }
    fn is_persistent(&self) -> bool {
        false
    }
    fn backend_name(&self) -> &'static str {
        "faulty_many"
    }
}

/// 录制型事件订阅方：捕获 `publish_error` 到达的事件（仅用于断言）。
struct RecordingPublisher {
    errors: Mutex<Vec<(Option<String>, String)>>,
}

#[async_trait]
impl EventPublisher for RecordingPublisher {
    async fn publish(&self, _event: CacheEvent) -> Result<(), OxCacheError> {
        Ok(())
    }

    fn publish_error(&self, key: Option<String>, error: String) -> Result<(), OxCacheError> {
        self.errors
            .lock()
            .expect("recording publisher lock poisoned")
            .push((key, error));
        Ok(())
    }
}

impl RecordingPublisher {
    fn errors(&self) -> std::sync::MutexGuard<'_, Vec<(Option<String>, String)>> {
        self.errors
            .lock()
            .expect("recording publisher lock poisoned")
    }
}

/// 故障层整批失败：错误按键拆分（k1/k2/k3 各一条 `backend faulty_many` 事件）、
/// 键继续降级到下一层（moka 命中 k1）、未命中以 `None` 结尾（k2/k3）、
/// 方法不返回 `Err`（签名即 `Vec<(String, Option<Vec<u8>>)>`）。
#[tokio::test]
async fn probe22_iter_entries_layer_failure_splits_per_key_and_degrades() {
    // 下层 moka：预置 k1 命中值
    let moka = oxcache::backend::MokaMemoryBackend::builder()
        .capacity(64)
        .build();
    oxcache::backend::CacheWriter::set(&moka, "probe22-k1".into(), b"v1".to_vec().into(), None)
        .await
        .expect("prefill moka k1");

    let recorder = Arc::new(RecordingPublisher {
        errors: Mutex::new(Vec::new()),
    });

    let chain = ChainCache::builder()
        .event_publisher(recorder.clone())
        .link(ChainLink::new(
            FaultyManyLayer { fail: true },
            100,
            false,
            "faulty_many",
        ))
        .link(ChainLink::from_backend(moka))
        .build();

    let entries = chain
        .iter_entries(&["probe22-k1", "probe22-k2", "probe22-k3"])
        .await;

    // 1) 方法不返回 Err：签名即 Vec<(String, Option<Vec<u8>>)>，此处直接消费
    assert_eq!(entries.len(), 3, "output length must match input keys");

    // 2) 顺序与输入一致；k1 降级到下层命中；k2/k3 未命中以 None 结尾
    let k1 = entries.iter().find(|(k, _)| k == "probe22-k1").unwrap();
    assert_eq!(
        k1.1.as_deref(),
        Some(b"v1".as_slice()),
        "k1 must resolve from the lower layer after the top layer failed"
    );
    let k2 = entries.iter().find(|(k, _)| k == "probe22-k2").unwrap();
    assert!(k2.1.is_none(), "k2 must end as None");
    let k3 = entries.iter().find(|(k, _)| k == "probe22-k3").unwrap();
    assert!(k3.1.is_none(), "k3 must end as None");

    // 3) 错误按键拆分：故障层每键恰好一条 `backend faulty_many: …` 事件
    let errors = recorder.errors();
    assert_eq!(
        errors.len(),
        3,
        "each key must get its own split error event, got {errors:?}"
    );
    for key in ["probe22-k1", "probe22-k2", "probe22-k3"] {
        assert!(
            errors
                .iter()
                .any(|(k, e)| k.as_deref() == Some(key) && e.contains("backend faulty_many")),
            "missing split error event for key {key}: {errors:?}"
        );
    }
}

/// 对照组：故障层整批失败且无下层命中 → 全部键以 None 结尾，事件仍逐键拆分，
/// 方法不返回 Err。
#[tokio::test]
async fn probe22_iter_entries_all_layers_fail_yields_none_without_err() {
    let recorder = Arc::new(RecordingPublisher {
        errors: Mutex::new(Vec::new()),
    });
    let chain = ChainCache::builder()
        .event_publisher(recorder.clone())
        .link(ChainLink::new(
            FaultyManyLayer { fail: true },
            100,
            false,
            "faulty_many",
        ))
        .build();

    let entries = chain.iter_entries(&["probe22-x", "probe22-y"]).await;
    assert_eq!(entries.len(), 2);
    assert!(
        entries.iter().all(|(_, v)| v.is_none()),
        "all keys must end as None: {entries:?}"
    );

    let errors = recorder.errors();
    assert_eq!(errors.len(), 2, "per-key split events, got {errors:?}");
    assert!(
        errors
            .iter()
            .all(|(k, e)| k.is_some() && e.contains("backend faulty_many")),
        "every event must carry Some(key) and the faulty layer name: {errors:?}"
    );
}
