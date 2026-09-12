// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 值完整性 HMAC 装饰器（`integrity` feature）
//!
//! [`IntegrityBackend`] 为 value 附加 **HMAC-SHA256** 标签，读取时校验，
//! 防不可信 L2 部署下的值篡改。与 [`EncryptedBackend`](super::EncryptedBackend)
//! 正交、可组合（加密自带 AEAD 完整性；"不加密只要防篡改"是更轻量的
//! 合规分级需求）。
//!
//! # 信封格式
//!
//! `[ver: u8][tag: 32B][payload]`
//!
//! # 失败语义
//!
//! 读取校验失败（篡改 payload/tag/ver、密钥不匹配）**视为 miss**：
//! 返回 `Ok(None)` 并计入 miss 指标与 `oxcache_integrity_failures_total`。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::encryption::IntegrityBackend;
//!
//! let backend = IntegrityBackend::new(inner_backend, [7u8; 32]);
//! backend.set("k".into(), Arc::new(b"value".to_vec()), None).await?;
//! assert_eq!(backend.get("k").await.unwrap(), Some(b"value".to_vec()));
//! ```

use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::OxCacheResult;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

/// HMAC 信封版本字节
pub const HMAC_ENVELOPE_VERSION: u8 = 1;

/// SHA-256 标签长度（32 字节）
const TAG_SIZE: usize = 32;

/// HMAC-SHA256 签名器
#[derive(Clone)]
pub struct HmacSigner {
    key: Arc<[u8; TAG_SIZE]>,
}

impl std::fmt::Debug for HmacSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 不输出密钥材料
        f.debug_struct("HmacSigner")
            .field("key", &"<redacted 32-byte key>")
            .finish()
    }
}

impl HmacSigner {
    /// 从 32 字节密钥创建签名器
    pub fn new(key: [u8; TAG_SIZE]) -> Self {
        Self { key: Arc::new(key) }
    }

    /// 从字节切片创建（长度错误返回 Err）
    pub fn from_slice(key: &[u8]) -> crate::error::OxCacheResult<Self> {
        let arr: [u8; TAG_SIZE] = key.try_into().map_err(|_| {
            crate::error::OxCacheError::InvalidInput(format!(
                "hmac signer key must be exactly {TAG_SIZE} bytes, got {}",
                key.len()
            ))
        })?;
        Ok(Self::new(arr))
    }

    /// 计算消息标签
    pub fn sign(&self, message: &[u8]) -> [u8; TAG_SIZE] {
        let mut mac = HmacSha256::new_from_slice(self.key.as_ref())
            .expect("HMAC accepts any key length; 32-byte key is valid");
        mac.update(message);
        mac.finalize().into_bytes().into()
    }

    /// 常量时间校验
    pub fn verify(&self, message: &[u8], tag: &[u8]) -> bool {
        let mut mac = HmacSha256::new_from_slice(self.key.as_ref())
            .expect("HMAC accepts any key length; 32-byte key is valid");
        mac.update(message);
        mac.verify_slice(tag).is_ok()
    }
}

/// 值完整性装饰器：value + HMAC-SHA256 标签，读校验失败视为 miss 并计数
pub struct IntegrityBackend {
    inner: Arc<dyn CacheBackend>,
    signer: HmacSigner,
}

impl std::fmt::Debug for IntegrityBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrityBackend")
            .field("inner", &self.inner.backend_kind())
            .field("signer", &"<redacted>")
            .finish()
    }
}

impl IntegrityBackend {
    /// 包装内层后端并注入 32 字节密钥
    pub fn new(inner: Arc<dyn CacheBackend>, key: [u8; TAG_SIZE]) -> Self {
        Self {
            inner,
            signer: HmacSigner::new(key),
        }
    }

    /// 从字节切片注入密钥（长度错误在构造期报错）
    pub fn from_slice_key(inner: Arc<dyn CacheBackend>, key: &[u8]) -> crate::error::OxCacheResult<Self> {
        Ok(Self {
            inner,
            signer: HmacSigner::from_slice(key)?,
        })
    }

    /// 签名器（诊断/测试）
    pub fn signer(&self) -> &HmacSigner {
        &self.signer
    }

    /// 校验失败的统一处理：计 miss + 计完整性失败，返回 None
    fn record_integrity_failure(&self) -> Option<Vec<u8>> {
        #[cfg(feature = "metrics")]
        {
            use crate::infra::metrics::{CacheOpResult, CacheOpType, CacheOperation};
            crate::infra::GLOBAL_UNIFIED_METRICS.record_operation(CacheOperation {
                layer: crate::core::CacheLayer::L1,
                op_type: CacheOpType::Get,
                result: CacheOpResult::Miss,
            });
            crate::infra::GLOBAL_UNIFIED_METRICS
                .increment_counter("oxcache_integrity_failures_total", 1);
        }
        None
    }
}

#[async_trait::async_trait]
impl CacheReader for IntegrityBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        match self.inner.get(key).await? {
            Some(envelope) => {
                // 格式：[ver][tag 32B][payload]
                if envelope.is_empty() || envelope[0] != HMAC_ENVELOPE_VERSION {
                    return Ok(self.record_integrity_failure());
                }
                if envelope.len() < 1 + TAG_SIZE {
                    return Ok(self.record_integrity_failure());
                }
                let tag = &envelope[1..1 + TAG_SIZE];
                let payload = &envelope[1 + TAG_SIZE..];
                if self.signer.verify(payload, tag) {
                    Ok(Some(payload.to_vec()))
                } else {
                    Ok(self.record_integrity_failure())
                }
            }
            None => Ok(None),
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

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern).await
    }
}

#[async_trait::async_trait]
impl CacheWriter for IntegrityBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let tag = self.signer.sign(value.as_slice());
        let mut envelope = Vec::with_capacity(1 + TAG_SIZE + value.len());
        envelope.push(HMAC_ENVELOPE_VERSION);
        envelope.extend_from_slice(&tag);
        envelope.extend_from_slice(value.as_slice());
        self.inner.set(key, Arc::new(envelope), ttl).await
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
        let mut signed = Vec::with_capacity(items.len());
        for (key, value, ttl) in items {
            let tag = self.signer.sign(value.as_slice());
            let mut envelope = Vec::with_capacity(1 + TAG_SIZE + value.len());
            envelope.push(HMAC_ENVELOPE_VERSION);
            envelope.extend_from_slice(&tag);
            envelope.extend_from_slice(value.as_slice());
            signed.push((key.clone(), Arc::new(envelope), *ttl));
        }
        self.inner.set_many(&signed).await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await
    }
}

#[async_trait::async_trait]
impl CacheConnector for IntegrityBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check().await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;
    use crate::backend::interface::{CacheReader, CacheWriter};

    fn key32(seed: u8) -> [u8; TAG_SIZE] {
        let mut key = [0u8; TAG_SIZE];
        for (i, b) in key.iter_mut().enumerate() {
            *b = seed.wrapping_add(i as u8);
        }
        key
    }

    fn integrity(seed: u8) -> IntegrityBackend {
        IntegrityBackend::new(Arc::new(MockBackend::new("mock", 100, false)), key32(seed))
    }

    #[tokio::test]
    async fn hmac_roundtrip_is_transparent() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"trusted-value".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(
            backend.get("k").await.unwrap(),
            Some(b"trusted-value".to_vec())
        );
    }

    #[tokio::test]
    async fn raw_storage_format_is_ver_tag_payload() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"payload!".to_vec()), None)
            .await
            .unwrap();

        let raw = backend.inner.get("k").await.unwrap().unwrap();
        assert_eq!(raw[0], HMAC_ENVELOPE_VERSION);
        let tag = &raw[1..1 + TAG_SIZE];
        assert_eq!(raw.len(), 1 + TAG_SIZE + 8);
        // 标签可用密钥独立复算验证
        assert!(backend.signer().verify(b"payload!", tag));
        assert!(!backend.signer().verify(b"tampered!", tag));
    }

    /// 篡改检测：payload/tag/ver 任意字节被改，读取视为 miss（None）
    #[tokio::test]
    async fn tampered_payload_reads_as_miss() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        let mut raw = backend.inner.get("k").await.unwrap().unwrap();
        let payload_len = raw.len();
        raw[payload_len - 1] ^= 0xFF; // 翻转 payload 最后一个字节
        backend.inner.set(Arc::from("k"), Arc::new(raw), None).await.unwrap();

        assert_eq!(backend.get("k").await.unwrap(), None, "篡改后应视为 miss");
    }

    #[tokio::test]
    async fn tampered_tag_reads_as_miss() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        let mut raw = backend.inner.get("k").await.unwrap().unwrap();
        raw[1] ^= 0x01; // 翻转 tag 首字节
        backend.inner.set(Arc::from("k"), Arc::new(raw), None).await.unwrap();

        assert_eq!(backend.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn tampered_version_reads_as_miss() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        let mut raw = backend.inner.get("k").await.unwrap().unwrap();
        raw[0] = 0xFF; // 未知版本
        backend.inner.set(Arc::from("k"), Arc::new(raw), None).await.unwrap();

        assert_eq!(backend.get("k").await.unwrap(), None);
    }

    /// 篡改后读取：miss 计数 + 完整性失败计数（全局指标，串行执行避免并发干扰）
    #[tokio::test]
    #[serial_test::serial]
    async fn integrity_failure_counts_miss_metric() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        // 篡改 payload
        let mut raw = backend.inner.get("k").await.unwrap().unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        backend.inner.set(Arc::from("k"), Arc::new(raw), None).await.unwrap();

        #[cfg(feature = "metrics")]
        {
            let before = crate::infra::GLOBAL_UNIFIED_METRICS.get_counters().l1_misses;
            let before_failures = crate::infra::GLOBAL_UNIFIED_METRICS
                .get_dynamic_metrics()
                .get("oxcache_integrity_failures_total")
                .map(|v| match v {
                    crate::infra::metrics::MetricValue::Counter(c) => *c,
                    _ => 0,
                })
                .unwrap_or(0);

            assert_eq!(backend.get("k").await.unwrap(), None);

            let after = crate::infra::GLOBAL_UNIFIED_METRICS.get_counters().l1_misses;
            let after_failures = crate::infra::GLOBAL_UNIFIED_METRICS
                .get_dynamic_metrics()
                .get("oxcache_integrity_failures_total")
                .map(|v| match v {
                    crate::infra::metrics::MetricValue::Counter(c) => *c,
                    _ => 0,
                })
                .unwrap_or(0);

            assert_eq!(after, before + 1, "完整性失败应计 1 次 miss");
            assert_eq!(after_failures, before_failures + 1, "完整性失败计数应递增");
        }
        #[cfg(not(feature = "metrics"))]
        {
            assert_eq!(backend.get("k").await.unwrap(), None);
        }
    }

    #[tokio::test]
    async fn wrong_key_reads_as_miss_not_error() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        // 不同密钥验签失败 → miss 而非 Err
        let other = IntegrityBackend::new(backend.inner.clone(), key32(99));
        assert_eq!(other.get("k").await.unwrap(), None);
    }

    /// 与加密装饰器叠加顺序无关：两种嵌套顺序都正确（R-ox4-003）
    #[cfg(feature = "encrypt")]
    #[tokio::test]
    async fn composes_with_encryption_in_both_orders() {
        use super::super::EncryptedBackend;

        let mk_inner = || -> Arc<dyn CacheBackend> { Arc::new(MockBackend::new("mock", 100, false)) };

        // 顺序 1：HMAC(Encrypted(inner))
        let a = IntegrityBackend::new(
            Arc::new(EncryptedBackend::new(mk_inner(), key32(7))),
            key32(8),
        );
        a.set(Arc::from("k"), Arc::new(b"both".to_vec()), None).await.unwrap();
        assert_eq!(a.get("k").await.unwrap(), Some(b"both".to_vec()));

        // 顺序 2：Encrypted(HMAC(inner))
        let b = EncryptedBackend::new(
            Arc::new(IntegrityBackend::new(mk_inner(), key32(8))),
            key32(7),
        );
        b.set(Arc::from("k"), Arc::new(b"both".to_vec()), None).await.unwrap();
        assert_eq!(b.get("k").await.unwrap(), Some(b"both".to_vec()));
    }

    #[test]
    fn hmac_key_length_validated_at_construction() {
        let inner: Arc<dyn CacheBackend> = Arc::new(MockBackend::new("mock", 100, false));
        let err = IntegrityBackend::from_slice_key(inner, b"tiny")
            .expect_err("密钥长度错误必须在构造期报错");
        assert!(matches!(err, crate::error::OxCacheError::InvalidInput(_)));
    }
}
