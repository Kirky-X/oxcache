// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 数据分级装饰器（`classification` feature）
//!
//! [`ClassifyingBackend`] 按调用方声明的键 glob 规则分级：`Sensitive` 条目
//! 以 XChaCha20-Poly1305 加密（AAD 绑定键名，信封与 `EncryptedBackend`
//! 兼容），并受可选 TTL 上限约束；`Public` / `Internal` 明文透传。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::classification::{ClassifyingBackend, RuleSetClassifier, Sensitivity};
//!
//! let classifier = RuleSetClassifier::new(Sensitivity::Public)
//!     .with_rule("credential:*", Sensitivity::Sensitive);
//! let backend = ClassifyingBackend::new(inner, classifier, [0u8; 32]);
//! ```

use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::OxCacheResult;
use crate::features::encryption::ValueCipher;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 数据敏感级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    /// 公开数据，明文透传
    Public,
    /// 内部数据，明文透传（仅作分级标记）
    Internal,
    /// 敏感数据，加密 + TTL 上限
    Sensitive,
}

/// 键 glob 规则分级器：先声明先匹配
#[derive(Debug, Clone)]
pub struct RuleSetClassifier {
    rules: Vec<(String, Sensitivity)>,
    default: Sensitivity,
    sensitive_ttl_cap: Option<Duration>,
}

impl RuleSetClassifier {
    /// 以默认分级创建分级器
    pub fn new(default: Sensitivity) -> Self {
        Self {
            rules: Vec::new(),
            default,
            sensitive_ttl_cap: None,
        }
    }

    /// 追加一条键 glob 规则（先声明先匹配）
    pub fn with_rule(mut self, key_glob: &str, sensitivity: Sensitivity) -> Self {
        self.rules.push((key_glob.to_string(), sensitivity));
        self
    }

    /// 设置 Sensitive 条目的 TTL 上限
    pub fn with_sensitive_ttl_cap(mut self, cap: Duration) -> Self {
        self.sensitive_ttl_cap = Some(cap);
        self
    }

    /// 分级一个键
    pub fn classify(&self, key: &str) -> Sensitivity {
        for (glob, sensitivity) in &self.rules {
            if crate::backend::interface::glob_match(glob, key) {
                return *sensitivity;
            }
        }
        self.default
    }

    /// Sensitive 条目的 TTL 上限
    pub fn sensitive_ttl_cap(&self) -> Option<Duration> {
        self.sensitive_ttl_cap
    }
}

/// 分级信封版本字节（后接 `ValueCipher` 的 `[ver][nonce][ciphertext]` 信封）
pub const CLASSIFICATION_ENVELOPE_VERSION: u8 = 1;

/// 按敏感级别选择性加密的装饰器
pub struct ClassifyingBackend {
    inner: Arc<dyn CacheBackend>,
    classifier: RuleSetClassifier,
    cipher: ValueCipher,
}

impl ClassifyingBackend {
    /// 以 32 字节密钥创建装饰器
    pub fn new(inner: Arc<dyn CacheBackend>, classifier: RuleSetClassifier, key: [u8; 32]) -> Self {
        Self {
            inner,
            classifier,
            cipher: ValueCipher::new(key),
        }
    }

    /// 从字节切片创建装饰器（长度须为 32 字节）
    pub fn from_slice_key(
        inner: Arc<dyn CacheBackend>,
        classifier: RuleSetClassifier,
        key: &[u8],
    ) -> OxCacheResult<Self> {
        Ok(Self {
            inner,
            classifier,
            cipher: ValueCipher::from_slice(key)?,
        })
    }

    /// 当前分级器
    pub fn classifier(&self) -> &RuleSetClassifier {
        &self.classifier
    }

    /// 应用 Sensitive TTL 上限：cap-only 时以 cap 作为 TTL
    fn capped_ttl(&self, ttl: Option<Duration>) -> Option<Duration> {
        match (ttl, self.classifier.sensitive_ttl_cap()) {
            (Some(t), Some(cap)) => Some(t.min(cap)),
            (Some(t), None) => Some(t),
            (None, cap) => cap,
        }
    }
}

#[async_trait::async_trait]
impl CacheReader for ClassifyingBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let raw = match self.inner.get(key).await? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };
        // 仅 Sensitive 键会被本装饰器加密；其他键的首字节可能恰好是
        // IntegrityBackend 信封版本（同为 1），不可误判为分级信封。
        if self.classifier.classify(key) != Sensitivity::Sensitive
            || raw.first() != Some(&CLASSIFICATION_ENVELOPE_VERSION)
        {
            return Ok(Some(raw));
        }
        match self.cipher.decrypt(&raw[1..], key.as_bytes()) {
            Ok(plaintext) => Ok(Some(plaintext)),
            Err(_) => {
                // 与 IntegrityBackend 校验失败同口径：解密失败视为 miss
                self.inner.delete(key).await?;
                #[cfg(feature = "metrics")]
                crate::infra::GLOBAL_UNIFIED_METRICS
                    .increment_counter("oxcache_classification_sensitive_total", 1);
                Ok(None)
            }
        }
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.inner.exists(key).await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.inner.ttl(key).await
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len().await
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity().await
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        self.inner.stats().await
    }

    async fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key).await?);
        }
        Ok(results)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern).await
    }
}

#[async_trait::async_trait]
impl CacheWriter for ClassifyingBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        match self.classifier.classify(&key) {
            Sensitivity::Sensitive => {
                let mut envelope = vec![CLASSIFICATION_ENVELOPE_VERSION];
                envelope.extend_from_slice(&self.cipher.encrypt(&value, key.as_bytes())?);
                let capped = self.capped_ttl(ttl);
                self.inner.set(key, Arc::new(envelope), capped).await?;
                #[cfg(feature = "metrics")]
                crate::infra::GLOBAL_UNIFIED_METRICS
                    .increment_counter("oxcache_classification_sensitive_total", 1);
                Ok(())
            }
            Sensitivity::Public | Sensitivity::Internal => self.inner.set(key, value, ttl).await,
        }
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key).await
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear().await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl).await
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        for (key, value, ttl) in items {
            self.set(key.clone(), value.clone(), *ttl).await?;
        }
        Ok(())
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await
    }
}

#[async_trait::async_trait]
impl CacheConnector for ClassifyingBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }

    fn as_pattern_ops(&self) -> Option<&dyn crate::backend::CachePatternOps> {
        self.inner.as_pattern_ops()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    fn classifier() -> RuleSetClassifier {
        RuleSetClassifier::new(Sensitivity::Public)
            .with_rule("credential:*", Sensitivity::Sensitive)
            .with_rule("session:*", Sensitivity::Internal)
    }

    fn backend() -> ClassifyingBackend {
        ClassifyingBackend::new(
            Arc::new(MockBackend::new("mock", 100, false)),
            classifier(),
            [7u8; 32],
        )
    }

    #[test]
    fn rule_hit_returns_declared_sensitivity() {
        let c = classifier();
        assert_eq!(c.classify("credential:api-key"), Sensitivity::Sensitive);
        assert_eq!(c.classify("session:abc"), Sensitivity::Internal);
    }

    #[test]
    fn no_rule_hit_returns_default() {
        assert_eq!(classifier().classify("public:key"), Sensitivity::Public);
    }

    #[test]
    fn first_declared_rule_wins() {
        let c = RuleSetClassifier::new(Sensitivity::Public)
            .with_rule("*", Sensitivity::Internal)
            .with_rule("session:*", Sensitivity::Sensitive);
        assert_eq!(c.classify("session:a"), Sensitivity::Internal);
    }

    #[test]
    fn ttl_cap_is_exposed() {
        let c = classifier().with_sensitive_ttl_cap(Duration::from_secs(30));
        assert_eq!(c.sensitive_ttl_cap(), Some(Duration::from_secs(30)));
    }

    #[tokio::test]
    async fn sensitive_payload_starts_with_version_and_has_no_plaintext() {
        let raw = Arc::new(MockBackend::new("mock", 100, false));
        let decor = ClassifyingBackend::new(raw.clone(), classifier(), [7u8; 32]);
        decor
            .set(
                Arc::from("credential:key"),
                Arc::new(b"super-secret-value".to_vec()),
                None,
            )
            .await
            .unwrap();
        let stored = raw.get("credential:key").await.unwrap().unwrap();
        assert_eq!(stored.first(), Some(&CLASSIFICATION_ENVELOPE_VERSION));
        assert!(!stored.windows(4).any(|w| w == b"supe"));
    }

    #[tokio::test]
    async fn public_payload_is_byte_identical() {
        let raw = Arc::new(MockBackend::new("mock", 100, false));
        let decor = ClassifyingBackend::new(raw.clone(), classifier(), [7u8; 32]);
        decor
            .set(Arc::from("public:key"), Arc::new(b"plain".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(
            raw.get("public:key").await.unwrap(),
            Some(b"plain".to_vec())
        );
    }

    #[tokio::test]
    async fn sensitive_roundtrip_and_wrong_key_is_miss() {
        let decor = backend();
        let _ = &decor;
        decor
            .set(
                Arc::from("credential:key"),
                Arc::new(b"secret".to_vec()),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            decor.get("credential:key").await.unwrap(),
            Some(b"secret".to_vec())
        );

        let raw = Arc::new(MockBackend::new("mock", 100, false));
        let wrong = ClassifyingBackend::new(raw.clone(), classifier(), [9u8; 32]);
        raw.set(
            Arc::from("credential:key"),
            Arc::new(vec![CLASSIFICATION_ENVELOPE_VERSION, 0u8]),
            None,
        )
        .await
        .unwrap();
        assert_eq!(wrong.get("credential:key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn ttl_is_capped_for_sensitive() {
        let raw = Arc::new(MockBackend::new("mock", 100, false));
        let c = classifier().with_sensitive_ttl_cap(Duration::from_secs(10));
        let decor = ClassifyingBackend::new(raw.clone(), c, [7u8; 32]);
        decor
            .set(
                Arc::from("credential:key"),
                Arc::new(b"secret".to_vec()),
                Some(Duration::from_secs(3600)),
            )
            .await
            .unwrap();
        assert!(decor.ttl("credential:key").await.unwrap() <= Some(Duration::from_secs(10)));
    }

    #[tokio::test]
    async fn cap_only_without_ttl_sets_cap_as_ttl() {
        let raw = Arc::new(MockBackend::new("mock", 100, false));
        let c = classifier().with_sensitive_ttl_cap(Duration::from_secs(15));
        let decor = ClassifyingBackend::new(raw, c, [7u8; 32]);
        decor
            .set(
                Arc::from("credential:key"),
                Arc::new(b"secret".to_vec()),
                None,
            )
            .await
            .unwrap();
        let ttl = decor.ttl("credential:key").await.unwrap().unwrap();
        assert!(ttl <= Duration::from_secs(15) && ttl > Duration::from_secs(14));
    }

    #[tokio::test]
    async fn public_ttl_is_not_capped() {
        let raw = Arc::new(MockBackend::new("mock", 100, false));
        let c = classifier().with_sensitive_ttl_cap(Duration::from_secs(10));
        let decor = ClassifyingBackend::new(raw, c, [7u8; 32]);
        decor
            .set(
                Arc::from("public:key"),
                Arc::new(b"plain".to_vec()),
                Some(Duration::from_secs(3600)),
            )
            .await
            .unwrap();
        let ttl = decor.ttl("public:key").await.unwrap().unwrap();
        assert!(ttl <= Duration::from_secs(3600) && ttl > Duration::from_secs(3599));
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    #[serial_test::serial]
    async fn sensitive_set_increments_classification_counter() {
        use crate::infra::metrics::MetricValue;
        let decor = backend();
        decor
            .set(Arc::from("public:key"), Arc::new(b"plain".to_vec()), None)
            .await
            .unwrap();
        let before = crate::infra::GLOBAL_UNIFIED_METRICS
            .snapshot()
            .dynamic_metrics
            .get("oxcache_classification_sensitive_total")
            .and_then(|v| {
                if let MetricValue::Counter(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        decor
            .set(
                Arc::from("credential:key"),
                Arc::new(b"secret".to_vec()),
                None,
            )
            .await
            .unwrap();
        let after = crate::infra::GLOBAL_UNIFIED_METRICS
            .snapshot()
            .dynamic_metrics
            .get("oxcache_classification_sensitive_total")
            .and_then(|v| {
                if let MetricValue::Counter(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        assert_eq!(after, before + 1);
    }

    #[tokio::test]
    async fn exists_delete_expire_keys_passthrough() {
        let decor = backend();
        decor
            .set(
                Arc::from("credential:key"),
                Arc::new(b"secret".to_vec()),
                None,
            )
            .await
            .unwrap();
        assert!(decor.exists("credential:key").await.unwrap());
        decor
            .expire("credential:key", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(decor.keys("credential:*").await.unwrap().len() == 1);
        decor.delete("credential:key").await.unwrap();
        assert_eq!(decor.get("credential:key").await.unwrap(), None);
    }
}
