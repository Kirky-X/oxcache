// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 来源约束装饰器（`provenance` feature）
//!
//! [`ProvenanceBackend`] 在写入时封装来源证明元数据（信封内嵌），读取时
//! 按受信来源清单与新鲜度上限校验：不受信、过期、损坏或无证明（按策略）
//! 条目视为 miss 并删除，计数 `oxcache_provenance_rejected_total`。
//!
//! 信封布局：`[ver 1B][meta_len u32 BE][meta JSON][payload]`。

use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::{OxCacheError, OxCacheResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 来源信封版本字节
pub const PROVENANCE_ENVELOPE_VERSION: u8 = 1;

/// 写入来源证明
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// 来源标识（如 `user-service`）
    pub source: String,
    /// 数据抓取时刻
    pub fetched_at: SystemTime,
    /// 最大可接受陈旧时长
    pub max_staleness: Duration,
}

/// 来源信任策略
#[derive(Debug, Clone)]
pub struct ProvenancePolicy {
    /// 受信来源 glob 清单（空清单 = 全部不受信）
    pub trusted_sources: Vec<String>,
    /// 无证明条目是否拒绝（false = 放行，兼容存量数据）
    pub reject_unprovened: bool,
}

/// 信封内的元数据（JSON 序列化形状）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProvenanceMeta {
    source: String,
    fetched_at_unix_ms: u64,
    max_staleness_ms: u64,
}

impl From<&Provenance> for ProvenanceMeta {
    fn from(p: &Provenance) -> Self {
        let ms =
            |t: SystemTime| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        Self {
            source: p.source.clone(),
            fetched_at_unix_ms: ms(p.fetched_at),
            max_staleness_ms: p.max_staleness.as_millis() as u64,
        }
    }
}

impl ProvenanceMeta {
    fn to_provenance(&self) -> Provenance {
        Provenance {
            source: self.source.clone(),
            fetched_at: UNIX_EPOCH + Duration::from_millis(self.fetched_at_unix_ms),
            max_staleness: Duration::from_millis(self.max_staleness_ms),
        }
    }
}

/// 编码为 `[ver][meta_len u32 BE][meta JSON][payload]`
fn encode_envelope(meta: &Provenance, payload: &[u8]) -> OxCacheResult<Vec<u8>> {
    let json = serde_json::to_vec(&ProvenanceMeta::from(meta)).map_err(|e| {
        OxCacheError::Serialization(format!("provenance meta serialize failed: {e}"))
    })?;
    let meta_len = u32::try_from(json.len()).map_err(|_| {
        OxCacheError::InvalidInput("provenance meta exceeds u32::MAX bytes".to_string())
    })?;
    let mut out = Vec::with_capacity(1 + 4 + json.len() + payload.len());
    out.push(PROVENANCE_ENVELOPE_VERSION);
    out.extend_from_slice(&meta_len.to_be_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(payload);
    Ok(out)
}

/// 解码信封，返回 `(元数据, payload)`；meta_len 与实际不符视为损坏
fn decode_envelope(data: &[u8]) -> OxCacheResult<(Provenance, Vec<u8>)> {
    if data.len() < 5 || data[0] != PROVENANCE_ENVELOPE_VERSION {
        return Err(OxCacheError::Serialization(
            "provenance envelope missing or unknown version".to_string(),
        ));
    }
    let meta_len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;
    let json_end = 5 + meta_len;
    if json_end > data.len() {
        return Err(OxCacheError::Serialization(
            "provenance envelope meta_len exceeds payload".to_string(),
        ));
    }
    let meta: ProvenanceMeta = serde_json::from_slice(&data[5..json_end]).map_err(|e| {
        OxCacheError::Serialization(format!("provenance meta deserialize failed: {e}"))
    })?;
    Ok((meta.to_provenance(), data[json_end..].to_vec()))
}

/// 来源约束装饰器
pub struct ProvenanceBackend {
    inner: Arc<dyn CacheBackend>,
    policy: ProvenancePolicy,
}

impl ProvenanceBackend {
    /// 创建装饰器
    pub fn new(inner: Arc<dyn CacheBackend>, policy: ProvenancePolicy) -> Self {
        Self { inner, policy }
    }

    /// 当前策略
    pub fn policy(&self) -> &ProvenancePolicy {
        &self.policy
    }

    /// 写入带来源证明的条目
    pub async fn set_with_provenance(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
        provenance: Provenance,
    ) -> OxCacheResult<()> {
        let envelope = encode_envelope(&provenance, &value)?;
        self.inner.set(key, Arc::new(envelope), ttl).await
    }

    /// 拒绝读取：删除条目 + 计数 + miss
    async fn reject(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        self.inner.delete(key).await?;
        #[cfg(feature = "metrics")]
        crate::infra::GLOBAL_UNIFIED_METRICS
            .increment_counter("oxcache_provenance_rejected_total", 1);
        Ok(None)
    }

    /// 校验来源与新鲜度；通过返回 payload，否则返回 None
    fn validate(&self, provenance: &Provenance, payload: Vec<u8>) -> Option<Vec<u8>> {
        let trusted = self
            .policy
            .trusted_sources
            .iter()
            .any(|p| crate::backend::interface::glob_match(p, &provenance.source));
        if !trusted {
            return None;
        }
        if provenance.fetched_at.elapsed().unwrap_or(Duration::MAX) > provenance.max_staleness {
            return None;
        }
        Some(payload)
    }
}

#[async_trait::async_trait]
impl CacheReader for ProvenanceBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let raw = match self.inner.get(key).await? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };
        if raw.first() != Some(&PROVENANCE_ENVELOPE_VERSION) {
            return if self.policy.reject_unprovened {
                self.reject(key).await
            } else {
                Ok(Some(raw))
            };
        }
        match decode_envelope(&raw) {
            Ok((provenance, payload)) => match self.validate(&provenance, payload) {
                Some(payload) => Ok(Some(payload)),
                None => self.reject(key).await,
            },
            // 信封版本字节与 IntegrityBackend 相同（=1）；组合链下本层可能读到
            // 内层装饰器信封。reject_unprovened=false 时按无证明放行，不删除。
            Err(_) if !self.policy.reject_unprovened => Ok(Some(raw)),
            Err(_) => self.reject(key).await,
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
impl CacheWriter for ProvenanceBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        self.inner.set(key, value, ttl).await
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
        self.inner.set_many(items).await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys).await
    }
}

#[async_trait::async_trait]
impl CacheConnector for ProvenanceBackend {
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

    fn provenance(source: &str, fetched_at: SystemTime) -> Provenance {
        Provenance {
            source: source.to_string(),
            fetched_at,
            max_staleness: Duration::from_secs(60),
        }
    }

    fn backend(reject_unprovened: bool) -> ProvenanceBackend {
        ProvenanceBackend::new(
            Arc::new(MockBackend::new("mock", 100, false)),
            ProvenancePolicy {
                trusted_sources: vec!["user-service".to_string()],
                reject_unprovened,
            },
        )
    }

    #[test]
    fn envelope_roundtrip() {
        // 信封以毫秒存储时间戳，构造时对齐到毫秒避免精度差
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let p = provenance("user-service", UNIX_EPOCH + Duration::from_millis(now_ms));
        let encoded = encode_envelope(&p, b"payload").unwrap();
        assert_eq!(encoded[0], PROVENANCE_ENVELOPE_VERSION);
        let (decoded, payload) = decode_envelope(&encoded).unwrap();
        assert_eq!(decoded, p);
        assert_eq!(payload, b"payload");
    }

    #[test]
    fn envelope_meta_len_mismatch_is_corrupt() {
        let p = provenance("user-service", SystemTime::now());
        let mut encoded = encode_envelope(&p, b"payload").unwrap();
        encoded[4] = 200; // 伪造一个超长 meta_len
        assert!(decode_envelope(&encoded).is_err());
    }

    #[test]
    fn envelope_json_field_names() {
        let p = provenance("user-service", SystemTime::now());
        let encoded = encode_envelope(&p, b"x").unwrap();
        let json = String::from_utf8_lossy(&encoded[5..]);
        assert!(json.contains("\"source\""));
        assert!(json.contains("fetched_at_unix_ms"));
        assert!(json.contains("max_staleness_ms"));
    }

    #[tokio::test]
    async fn trusted_and_fresh_returns_payload() {
        let decor = backend(true);
        decor
            .set_with_provenance(
                Arc::from("k"),
                Arc::new(b"value".to_vec()),
                None,
                provenance("user-service", SystemTime::now()),
            )
            .await
            .unwrap();
        assert_eq!(decor.get("k").await.unwrap(), Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn untrusted_source_is_rejected_and_deleted() {
        let decor = backend(true);
        decor
            .set_with_provenance(
                Arc::from("k"),
                Arc::new(b"value".to_vec()),
                None,
                provenance("attacker", SystemTime::now()),
            )
            .await
            .unwrap();
        assert_eq!(decor.get("k").await.unwrap(), None);
        assert!(!decor.exists("k").await.unwrap());
    }

    #[tokio::test]
    async fn stale_provenance_is_rejected_even_with_long_ttl() {
        let decor = backend(true);
        let old = SystemTime::now() - Duration::from_secs(120);
        decor
            .set_with_provenance(
                Arc::from("k"),
                Arc::new(b"value".to_vec()),
                Some(Duration::from_secs(3600)),
                provenance("user-service", old),
            )
            .await
            .unwrap();
        assert_eq!(decor.get("k").await.unwrap(), None);
        assert!(!decor.exists("k").await.unwrap());
    }

    #[tokio::test]
    async fn unprovened_entry_passes_when_policy_allows() {
        let decor = backend(false);
        decor
            .set(Arc::from("k"), Arc::new(b"plain".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(decor.get("k").await.unwrap(), Some(b"plain".to_vec()));
    }

    #[tokio::test]
    async fn corrupt_envelope_passes_through_when_policy_allows() {
        let raw = Arc::new(MockBackend::new("mock", 100, false));
        raw.set(
            Arc::from("k"),
            Arc::new(vec![PROVENANCE_ENVELOPE_VERSION, 1, 2, 3]),
            None,
        )
        .await
        .unwrap();
        let inner: Arc<dyn CacheBackend> = raw;
        let pass = ProvenanceBackend::new(
            inner,
            ProvenancePolicy {
                trusted_sources: vec!["user-service".to_string()],
                reject_unprovened: false,
            },
        );
        assert_eq!(
            pass.get("k").await.unwrap(),
            Some(vec![PROVENANCE_ENVELOPE_VERSION, 1, 2, 3])
        );
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    #[serial_test::serial]
    async fn rejected_read_increments_counter_and_deletes_entry() {
        use crate::infra::metrics::MetricValue;
        let decor = backend(true);
        decor
            .set(Arc::from("k"), Arc::new(b"plain".to_vec()), None)
            .await
            .unwrap();
        let before = crate::infra::GLOBAL_UNIFIED_METRICS
            .snapshot()
            .dynamic_metrics
            .get("oxcache_provenance_rejected_total")
            .and_then(|v| {
                if let MetricValue::Counter(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        assert_eq!(decor.get("k").await.unwrap(), None);
        assert!(!decor.exists("k").await.unwrap());
        let after = crate::infra::GLOBAL_UNIFIED_METRICS
            .snapshot()
            .dynamic_metrics
            .get("oxcache_provenance_rejected_total")
            .and_then(|v| {
                if let MetricValue::Counter(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        assert!(after > before, "before={before} after={after}");
    }

    #[tokio::test]
    async fn unprovened_entry_is_rejected_when_policy_denies() {
        let decor = backend(true);
        decor
            .set(Arc::from("k"), Arc::new(b"plain".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(decor.get("k").await.unwrap(), None);
        assert!(!decor.exists("k").await.unwrap());
    }
}
