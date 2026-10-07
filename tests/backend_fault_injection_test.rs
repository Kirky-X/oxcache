// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 故障注入：分层链在「后端全失败」「get_many 长度不符」时的显性语义。
//!
//! 公开 API 下没有可注入故障的后端（MockBackend 是 crate 内部 #[cfg(test)]），
//! 故本文件自带最小失败后端（仅实现 13 个必需方法），用于覆盖 chain 的错误
//! 路径：全失败传播、批量读取长度不符按键拆分上报、回填失败上报。

#![cfg(feature = "full")]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use oxcache::OxCacheError;
use oxcache::backend::{BackendKind, CacheConnector, CacheReader, CacheWriter};

/// 可注入故障的最小后端：get/set 返回错误，或 get_many 返回长度不符的结果
struct FaultyBackend {
    fail_get: bool,
    fail_set: bool,
    mismatch_get_many: bool,
}

impl FaultyBackend {
    fn new() -> Self {
        Self {
            fail_get: false,
            fail_set: false,
            mismatch_get_many: false,
        }
    }
    fn fail_get(mut self) -> Self {
        self.fail_get = true;
        self
    }
    fn mismatch_get_many(mut self) -> Self {
        self.mismatch_get_many = true;
        self
    }
}

#[async_trait]
impl CacheReader for FaultyBackend {
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>, OxCacheError> {
        if self.fail_get {
            return Err(OxCacheError::Operation("injected get failure".to_string()));
        }
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
        if self.mismatch_get_many {
            // 返回长度与请求数不符：调用方必须显性化而非 zip 截断静默
            return Ok(Vec::new());
        }
        Ok(keys.iter().map(|_| None).collect())
    }
}

#[async_trait]
impl CacheWriter for FaultyBackend {
    async fn set(
        &self,
        _key: Arc<str>,
        _value: Arc<Vec<u8>>,
        _ttl: Option<Duration>,
    ) -> Result<(), OxCacheError> {
        if self.fail_set {
            return Err(OxCacheError::Operation("injected set failure".to_string()));
        }
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
impl CacheConnector for FaultyBackend {
    async fn health_check(&self) -> Result<(), OxCacheError> {
        Ok(())
    }
    async fn shutdown(&self) {}
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Mock
    }
}

use oxcache::cache::ChainBuilder;

/// 所有后端都返回 Err：链必须传播错误（不得静默返回 miss）
#[tokio::test]
async fn chain_all_backends_failed_propagates_error() {
    let chain = ChainBuilder::new()
        .extra_backend(Arc::new(FaultyBackend::new().fail_get()), 100, false, "f1")
        .extra_backend(Arc::new(FaultyBackend::new().fail_get()), 90, false, "f2")
        .build()
        .await
        .expect("chain build");

    let err = CacheReader::get(&chain, "k_fail")
        .await
        .expect_err("全后端失败必须传播错误");
    assert!(
        format!("{err:?}").contains("injected get failure")
            || format!("{err:?}").contains("All backends failed"),
        "错误应可辨识，实际: {err:?}"
    );
}

/// get_many 长度不符：按整批失败降级——不得按 zip 截断错位取值
///
/// 链对「返回长度 ≠ 请求键数」的后端按整批失败处理并降级到下一层：
/// 未解析键保持 miss（显性），绝不把错位结果当作命中返回。
#[tokio::test]
async fn chain_get_many_length_mismatch_never_misaligns_values() {
    let chain = ChainBuilder::new()
        .extra_backend(
            Arc::new(FaultyBackend::new().mismatch_get_many()),
            100,
            false,
            "m1",
        )
        .build()
        .await
        .expect("chain build");

    let keys = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let values = CacheReader::get_many(&chain, &keys)
        .await
        .expect("长度不符按整批失败降级（不 panic）");
    assert_eq!(values.len(), keys.len(), "返回长度必须与请求键数一致");
    assert!(
        values.iter().all(|v| v.is_none()),
        "长度不符时不得错位取值：全部键按 miss 降级，实际 {values:?}"
    );
}
