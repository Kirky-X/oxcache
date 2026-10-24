// Copyright (c) 2025-2026 Kirky.X🌠
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
//! 标签覆盖 **cache key + value**（键以长度前缀接入 MAC 输入），
//! 与 [`EncryptedBackend`](super::EncryptedBackend) 的 AAD 绑定同构：
//! 不可信 L2 无法把 A 键的合法信封搬到 B 键下通过校验。
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

    /// 计算消息标签（绑定 cache key）
    ///
    /// key 参与 MAC 计算：同一 value 放在不同键下得到不同标签，
    /// 防止不可信 L2 把 A 键的合法信封原样搬到 B 键下仍通过校验。
    /// 与 [`EncryptedBackend`](super::EncryptedBackend) 的 AAD 绑定同构。
    pub fn sign(&self, key: &str, message: &[u8]) -> [u8; TAG_SIZE] {
        let mut mac = HmacSha256::new_from_slice(self.key.as_ref())
            .expect("HMAC accepts any key length; 32-byte key is valid");
        update_with_key(&mut mac, key, message);
        mac.finalize().into_bytes().into()
    }

    /// 常量时间校验（绑定 cache key）
    pub fn verify(&self, key: &str, message: &[u8], tag: &[u8]) -> bool {
        let mut mac = HmacSha256::new_from_slice(self.key.as_ref())
            .expect("HMAC accepts any key length; 32-byte key is valid");
        update_with_key(&mut mac, key, message);
        mac.verify_slice(tag).is_ok()
    }
}

/// 把 cache key 与消息一起喂入 MAC
///
/// key 以长度前缀（u64 LE）接入，避免 key 与 message 的拼接歧义：
/// ("ab", "c") 与 ("a", "bc") 必须得到不同标签。
fn update_with_key(mac: &mut HmacSha256, key: &str, message: &[u8]) {
    mac.update(&(key.len() as u64).to_le_bytes());
    mac.update(key.as_bytes());
    mac.update(message);
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
    pub fn from_slice_key(
        inner: Arc<dyn CacheBackend>,
        key: &[u8],
    ) -> crate::error::OxCacheResult<Self> {
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
                if self.signer.verify(key, payload, tag) {
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
        let tag = self.signer.sign(&key, value.as_slice());
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
            let tag = self.signer.sign(key, value.as_slice());
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

    fn as_pattern_ops(&self) -> Option<&dyn crate::backend::CachePatternOps> {
        self.inner.as_pattern_ops()
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
        // 标签可用密钥独立复算验证（键参与 MAC）
        assert!(backend.signer().verify("k", b"payload!", tag));
        assert!(!backend.signer().verify("k", b"tampered!", tag));
    }

    /// 篡改检测：payload/tag/ver 任意字节被改，读取视为 miss（None）
    ///
    /// 完整性失败会递增全局指标（miss / integrity_failures_total），
    /// 与同模块的指标断言测试互斥执行，避免并发累加干扰读数。
    #[tokio::test]
    #[serial_test::serial]
    async fn tampered_payload_reads_as_miss() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        let mut raw = backend.inner.get("k").await.unwrap().unwrap();
        let payload_len = raw.len();
        raw[payload_len - 1] ^= 0xFF; // 翻转 payload 最后一个字节
        backend
            .inner
            .set(Arc::from("k"), Arc::new(raw), None)
            .await
            .unwrap();

        assert_eq!(
            backend.get("k").await.unwrap(),
            None,
            "tampered entries should be treated as a miss"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn tampered_tag_reads_as_miss() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        let mut raw = backend.inner.get("k").await.unwrap().unwrap();
        raw[1] ^= 0x01; // 翻转 tag 首字节
        backend
            .inner
            .set(Arc::from("k"), Arc::new(raw), None)
            .await
            .unwrap();

        assert_eq!(backend.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn tampered_version_reads_as_miss() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"original".to_vec()), None)
            .await
            .unwrap();

        let mut raw = backend.inner.get("k").await.unwrap().unwrap();
        raw[0] = 0xFF; // 未知版本
        backend
            .inner
            .set(Arc::from("k"), Arc::new(raw), None)
            .await
            .unwrap();

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
        backend
            .inner
            .set(Arc::from("k"), Arc::new(raw), None)
            .await
            .unwrap();

        #[cfg(feature = "metrics")]
        {
            let before = crate::infra::GLOBAL_UNIFIED_METRICS
                .get_counters()
                .l1_misses;
            let before_failures = crate::infra::GLOBAL_UNIFIED_METRICS
                .get_dynamic_metrics()
                .get("oxcache_integrity_failures_total")
                .map(|v| match v {
                    crate::infra::metrics::MetricValue::Counter(c) => *c,
                    _ => 0,
                })
                .unwrap_or(0);

            assert_eq!(backend.get("k").await.unwrap(), None);

            let after = crate::infra::GLOBAL_UNIFIED_METRICS
                .get_counters()
                .l1_misses;
            let after_failures = crate::infra::GLOBAL_UNIFIED_METRICS
                .get_dynamic_metrics()
                .get("oxcache_integrity_failures_total")
                .map(|v| match v {
                    crate::infra::metrics::MetricValue::Counter(c) => *c,
                    _ => 0,
                })
                .unwrap_or(0);

            // l1_misses 为进程级共享计数器：并行测试同样会递增，
            // 精确 +1 断言在并行执行下不成立；完整性失败路径必须至少计 1 次。
            assert!(
                after > before,
                "integrity failures should count toward l1_misses"
            );
            // 完整性失败专用计数器：同模块的失败注入测试经 #[serial] 互斥，
            // 本次失败必须精确递增 1。
            assert_eq!(
                after_failures,
                before_failures + 1,
                "the integrity failure counter should increment"
            );
        }
        #[cfg(not(feature = "metrics"))]
        {
            assert_eq!(backend.get("k").await.unwrap(), None);
        }
    }

    #[tokio::test]
    #[serial_test::serial]
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

    /// 与加密装饰器叠加顺序无关：两种嵌套顺序都正确
    #[cfg(feature = "encrypt")]
    #[tokio::test]
    async fn composes_with_encryption_in_both_orders() {
        use super::super::EncryptedBackend;

        let mk_inner =
            || -> Arc<dyn CacheBackend> { Arc::new(MockBackend::new("mock", 100, false)) };

        // 顺序 1：HMAC(Encrypted(inner))
        let a = IntegrityBackend::new(
            Arc::new(EncryptedBackend::new(mk_inner(), key32(7))),
            key32(8),
        );
        a.set(Arc::from("k"), Arc::new(b"both".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(a.get("k").await.unwrap(), Some(b"both".to_vec()));

        // 顺序 2：Encrypted(HMAC(inner))
        let b = EncryptedBackend::new(
            Arc::new(IntegrityBackend::new(mk_inner(), key32(8))),
            key32(7),
        );
        b.set(Arc::from("k"), Arc::new(b"both".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(b.get("k").await.unwrap(), Some(b"both".to_vec()));
    }

    #[test]
    fn hmac_key_length_validated_at_construction() {
        let inner: Arc<dyn CacheBackend> = Arc::new(MockBackend::new("mock", 100, false));
        let err = IntegrityBackend::from_slice_key(inner, b"tiny")
            .expect_err("a wrong key length must fail at construction");
        assert!(matches!(err, crate::error::OxCacheError::InvalidInput(_)));
    }

    /// Debug 输出不得泄露签名密钥材料。
    #[test]
    fn debug_output_redacts_signer_key_material() {
        let signer = HmacSigner::new(key32(1));
        let rendered = format!("{signer:?}");
        assert!(
            rendered.contains("<redacted 32-byte key>"),
            "HmacSigner Debug should mark redaction, got {rendered}"
        );

        let backend = integrity(1);
        let rendered = format!("{backend:?}");
        assert!(
            rendered.contains("Mock") && rendered.contains("<redacted>"),
            "IntegrityBackend Debug should name the inner kind and redact, got {rendered}"
        );

        // 精确 32 字节密钥走 from_slice 成功路径
        let from_slice =
            HmacSigner::from_slice(&key32(2)).expect("a 32-byte key should be accepted");
        assert_eq!(
            from_slice.sign("k", b"msg"),
            HmacSigner::new(key32(2)).sign("k", b"msg")
        );
    }

    /// 装饰器对 CacheReader/CacheWriter/CacheConnector 全操作面透明转发。
    #[tokio::test]
    async fn integrity_backend_delegates_full_surface() {
        let backend = integrity(1);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();

        assert!(backend.exists("k").await.unwrap());
        assert_eq!(backend.ttl("k").await.unwrap(), None);
        assert_eq!(backend.len().await.unwrap(), 1);
        assert_eq!(backend.capacity().await.unwrap(), 0);
        assert!(!backend.stats().await.unwrap().is_empty());
        assert_eq!(backend.keys("*").await.unwrap().len(), 1);

        assert!(backend.expire("k", Duration::from_secs(60)).await.unwrap());
        backend
            .set_many(&[(Arc::from("a"), Arc::new(b"1".to_vec()), None)])
            .await
            .unwrap();
        assert_eq!(backend.get("a").await.unwrap(), Some(b"1".to_vec()));
        backend.delete_many(&["a".to_string()]).await.unwrap();
        assert!(!backend.exists("a").await.unwrap());

        backend.delete("k").await.unwrap();
        assert!(!backend.exists("k").await.unwrap());
        backend.clear().await.unwrap();
        assert_eq!(backend.len().await.unwrap(), 0);

        backend.health_check().await.unwrap();
        assert_eq!(backend.backend_kind(), BackendKind::Mock);
        backend.shutdown().await;
    }

    /// 跨键重放必须校验失败：A 键的合法信封搬到 B 键下不得通过 HMAC 校验
    ///
    /// 与 `EncryptedBackend` 的 `aad_binds_ciphertext_to_key_name` 同构：
    /// 键名参与 MAC 计算即杜绝跨键搬运。
    #[tokio::test]
    async fn hmac_binds_envelope_to_cache_key() {
        let backend = integrity(1);
        backend
            .set(
                Arc::from("tenantA:user:1"),
                Arc::new(b"alice-secret".to_vec()),
                None,
            )
            .await
            .unwrap();

        // 不可信 L2 把 A 键的信封原样搬到 B 键
        let envelope = backend.inner.get("tenantA:user:1").await.unwrap().unwrap();
        backend
            .inner
            .set(Arc::from("tenantB:user:2"), Arc::new(envelope), None)
            .await
            .unwrap();

        assert_eq!(
            backend.get("tenantB:user:2").await.unwrap(),
            None,
            "cross-key replay must be rejected as a miss"
        );
        // 原键仍可正常读取
        assert_eq!(
            backend.get("tenantA:user:1").await.unwrap(),
            Some(b"alice-secret".to_vec())
        );
    }

    /// 键名与消息的拼接不得产生歧义
    #[test]
    fn hmac_key_message_boundary_is_unambiguous() {
        let signer = HmacSigner::new(key32(3));
        assert_ne!(signer.sign("ab", b"c"), signer.sign("a", b"bc"));
        assert_ne!(signer.sign("k1", b"v"), signer.sign("k2", b"v"));
    }
}
