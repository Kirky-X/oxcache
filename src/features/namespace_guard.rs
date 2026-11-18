// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 命名空间边界守卫（`namespace-guard` feature）
//!
//! [`NamespaceGuardBackend`] 包装任意 [`CacheBackend`]，在转发前校验键或模式
//! 是否落在声明的命名空间前缀内。前缀约定为 `ns:key`（冒号分隔），与
//! `KeyGenerator::with_namespace` 和 `TypedNamespace` 一致。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::namespace_guard::{NamespaceGuardBackend, NamespacePolicy};
//!
//! let backend = NamespaceGuardBackend::new(inner, "tenant:acme", NamespacePolicy::Isolated);
//! // backend.get("tenant:other:key") → Err(OxCacheError::AccessDenied(..))
//! ```

use crate::backend::interface::{BackendKind, CacheSetItem};
use crate::backend::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
use crate::error::{OxCacheError, OxCacheResult};
use crate::i18n::messages::{
    MSG_DETAIL_NAMESPACE_ACCESS_DENIED, MSG_DETAIL_NAMESPACE_PATTERN_ACCESS_DENIED, t,
};
use crate::security::redaction::redact_cache_key;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// 命名空间隔离策略。Isolated：命名空间前缀之外的键一律拒绝。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespacePolicy {
    /// 隔离：越界键、越界模式与全局清空一律拒绝
    Isolated,
}

/// 配额超限策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// 拒绝写入并计数
    Reject,
    /// 只淘汰本命名空间最旧条目
    EvictOldestInNamespace,
}

/// 命名空间配额：单实例装饰器内的条目数与字节数上限
#[derive(Debug, Clone)]
pub struct NamespaceQuota {
    /// 最大条目数
    pub max_entries: u64,
    /// 最大字节数（按序列化后 value 长度计）
    pub max_bytes: u64,
    /// 超限处理策略
    pub overflow_policy: OverflowPolicy,
}

/// 配额登记表：键 → (字节数, 插入时刻)
type QuotaLedger = dashmap::DashMap<Arc<str>, (u64, std::time::Instant)>;

/// 命名空间边界守卫装饰器
pub struct NamespaceGuardBackend {
    inner: Arc<dyn CacheBackend>,
    prefix: String,
    policy: NamespacePolicy,
    quota: Option<NamespaceQuota>,
    ledger: QuotaLedger,
    denied_total: AtomicU64,
}

impl NamespaceGuardBackend {
    /// 包装内层后端，声明命名空间与策略
    pub fn new(inner: Arc<dyn CacheBackend>, namespace: &str, policy: NamespacePolicy) -> Self {
        Self::build(inner, namespace, policy, None)
    }

    /// 包装内层后端，并声明命名空间配额
    pub fn with_quota(
        inner: Arc<dyn CacheBackend>,
        namespace: &str,
        policy: NamespacePolicy,
        quota: NamespaceQuota,
    ) -> Self {
        Self::build(inner, namespace, policy, Some(quota))
    }

    /// 内部构造器：配额可选，供 builder 与 `new`/`with_quota` 共用
    fn build(
        inner: Arc<dyn CacheBackend>,
        namespace: &str,
        policy: NamespacePolicy,
        quota: Option<NamespaceQuota>,
    ) -> Self {
        Self {
            inner,
            prefix: normalize_prefix(namespace),
            policy,
            quota,
            ledger: QuotaLedger::new(),
            denied_total: AtomicU64::new(0),
        }
    }

    /// 当前配额占用：(条目数, 字节数)
    pub fn quota_usage(&self) -> (u64, u64) {
        let entries = self.ledger.len() as u64;
        let bytes = self.ledger.iter().map(|e| e.value().0).sum();
        (entries, bytes)
    }

    /// 当前配额配置（无配额时为 `None`）
    pub fn quota(&self) -> Option<&NamespaceQuota> {
        self.quota.as_ref()
    }

    /// 归一化后的命名空间前缀（以 `':'` 结尾）
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// 当前策略
    pub fn policy(&self) -> NamespacePolicy {
        self.policy
    }

    /// 累计拒绝次数
    pub fn denied_total(&self) -> u64 {
        self.denied_total.load(Ordering::Relaxed)
    }

    /// 记录一次拒绝并更新指标
    fn record_denied(&self) {
        self.denied_total.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "metrics")]
        crate::infra::GLOBAL_UNIFIED_METRICS.increment_counter("oxcache_namespace_denied_total", 1);
    }

    /// 键是否属于本命名空间
    fn is_in_namespace(&self, key: &str) -> bool {
        key.starts_with(&self.prefix)
    }

    /// 模式是否属于本命名空间
    fn pattern_in_namespace(&self, pattern: &str) -> bool {
        pattern.starts_with(&self.prefix)
    }

    /// 拒绝键访问的错误（键文本经脱敏）
    fn deny_key(&self, key: &str) -> OxCacheError {
        self.record_denied();
        OxCacheError::AccessDenied(t(
            MSG_DETAIL_NAMESPACE_ACCESS_DENIED,
            &[
                ("key", redact_cache_key(key)),
                ("namespace", self.prefix.clone()),
            ],
        ))
    }

    /// 拒绝模式访问的错误（模式文本经脱敏）
    fn deny_pattern(&self, pattern: &str) -> OxCacheError {
        self.record_denied();
        OxCacheError::AccessDenied(t(
            MSG_DETAIL_NAMESPACE_PATTERN_ACCESS_DENIED,
            &[
                ("pattern", redact_cache_key(pattern)),
                ("namespace", self.prefix.clone()),
            ],
        ))
    }

    /// 写入前的配额准入检查；超限按策略拒绝或淘汰最旧条目。
    async fn admit_write(&self, key: &Arc<str>, bytes: u64) -> OxCacheResult<()> {
        let Some(quota) = self.quota.as_ref() else {
            return Ok(());
        };
        // 已存在的键覆盖写不增加条目数，仅按字节差检查
        let current = self.ledger.get(key).map(|e| (e.value().0, e.value().1));
        let entries = self.ledger.len() as u64;
        let existing_bytes = current.map(|(b, _)| b).unwrap_or(0);
        let new_bytes = bytes;
        let projected_bytes =
            self.ledger.iter().map(|e| e.value().0).sum::<u64>() - existing_bytes + new_bytes;
        let projected_entries = if current.is_some() {
            entries
        } else {
            entries + 1
        };

        if projected_entries <= quota.max_entries && projected_bytes <= quota.max_bytes {
            return Ok(());
        }
        match quota.overflow_policy {
            OverflowPolicy::Reject => {
                self.record_quota_rejected();
                Err(self.deny_key(key))
            }
            OverflowPolicy::EvictOldestInNamespace => {
                self.evict_oldest_until(projected_entries, projected_bytes, quota)
                    .await
            }
        }
    }

    /// 按插入时间从旧到新淘汰，直至新写入可容纳
    async fn evict_oldest_until(
        &self,
        mut projected_entries: u64,
        mut projected_bytes: u64,
        quota: &NamespaceQuota,
    ) -> OxCacheResult<()> {
        let mut victims: Vec<(Arc<str>, std::time::Instant)> = self
            .ledger
            .iter()
            .map(|e| (e.key().clone(), e.value().1))
            .collect();
        victims.sort_by_key(|(_, at)| *at);
        for (victim, _at) in victims {
            if projected_entries <= quota.max_entries && projected_bytes <= quota.max_bytes {
                break;
            }
            if let Some((victim_bytes, _)) = self.ledger.get(&victim).map(|e| *e.value()) {
                self.inner.delete(&victim).await?;
                self.ledger.remove(&victim);
                projected_bytes -= victim_bytes;
                projected_entries -= 1;
                self.update_usage_gauges();
                self.record_quota_evicted();
            }
        }
        Ok(())
    }

    /// 删除后回收登记表并更新 gauge
    fn release_key(&self, key: &str) {
        if self.ledger.remove(key).is_some() {
            self.update_usage_gauges();
        }
    }

    /// 登记写入占用并更新 gauge
    fn record_write(&self, key: Arc<str>, bytes: u64) {
        self.ledger.insert(key, (bytes, std::time::Instant::now()));
        self.update_usage_gauges();
    }

    #[cfg(feature = "metrics")]
    fn update_usage_gauges(&self) {
        let (entries, bytes) = self.quota_usage();
        let metrics = &crate::infra::GLOBAL_UNIFIED_METRICS;
        metrics.set_gauge("oxcache_namespace_entries", entries as f64);
        metrics.set_gauge("oxcache_namespace_bytes", bytes as f64);
    }

    #[cfg(not(feature = "metrics"))]
    fn update_usage_gauges(&self) {}

    #[cfg(feature = "metrics")]
    fn record_quota_rejected(&self) {
        crate::infra::GLOBAL_UNIFIED_METRICS
            .increment_counter("oxcache_namespace_quota_rejected_total", 1);
    }

    #[cfg(not(feature = "metrics"))]
    fn record_quota_rejected(&self) {}

    #[cfg(feature = "metrics")]
    fn record_quota_evicted(&self) {
        crate::infra::GLOBAL_UNIFIED_METRICS
            .increment_counter("oxcache_namespace_quota_evicted_total", 1);
    }

    #[cfg(not(feature = "metrics"))]
    fn record_quota_evicted(&self) {}
}

/// 把命名空间归一化为以 `':'` 结尾的前缀；已带尾冒号时不重复追加
fn normalize_prefix(namespace: &str) -> String {
    if namespace.ends_with(':') {
        namespace.to_string()
    } else {
        format!("{namespace}:")
    }
}

#[async_trait::async_trait]
impl CacheReader for NamespaceGuardBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        if !self.is_in_namespace(key) {
            return Err(self.deny_key(key));
        }
        self.inner.get(key).await
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        if !self.is_in_namespace(key) {
            return Err(self.deny_key(key));
        }
        self.inner.exists(key).await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        if !self.is_in_namespace(key) {
            return Err(self.deny_key(key));
        }
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
        for key in keys {
            if !self.is_in_namespace(key) {
                return Err(self.deny_key(key));
            }
        }
        self.inner.get_many(keys).await
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        if !self.pattern_in_namespace(pattern) {
            return Err(self.deny_pattern(pattern));
        }
        self.inner.keys(pattern).await
    }
}

#[async_trait::async_trait]
impl CacheWriter for NamespaceGuardBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        if !self.is_in_namespace(&key) {
            return Err(self.deny_key(&key));
        }
        let bytes = value.len() as u64;
        self.admit_write(&key, bytes).await?;
        self.inner.set(key.clone(), value, ttl).await?;
        self.record_write(key, bytes);
        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        if !self.is_in_namespace(key) {
            return Err(self.deny_key(key));
        }
        self.inner.delete(key).await?;
        self.release_key(key);
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        Err(self.deny_pattern("*"))
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        if !self.is_in_namespace(key) {
            return Err(self.deny_key(key));
        }
        self.inner.expire(key, ttl).await
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        for (key, value, _) in items {
            if !self.is_in_namespace(key) {
                return Err(self.deny_key(key));
            }
            self.admit_write(key, value.len() as u64).await?;
        }
        self.inner.set_many(items).await?;
        for (key, value, _) in items {
            self.record_write(key.clone(), value.len() as u64);
        }
        Ok(())
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        for key in keys {
            if !self.is_in_namespace(key) {
                return Err(self.deny_key(key));
            }
        }
        self.inner.delete_many(keys).await?;
        for key in keys {
            self.release_key(key);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl CacheConnector for NamespaceGuardBackend {
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
        // 守卫自身接管模式校验，不委托内层的原生模式操作（否则越界模式会绕过校验）
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    fn guard(namespace: &str) -> NamespaceGuardBackend {
        NamespaceGuardBackend::new(
            Arc::new(MockBackend::new("mock", 100, false)),
            namespace,
            NamespacePolicy::Isolated,
        )
    }

    #[test]
    fn prefix_normalization_adds_trailing_colon() {
        assert_eq!(guard("tenant:acme").prefix(), "tenant:acme:");
    }

    #[test]
    fn prefix_normalization_keeps_existing_trailing_colon() {
        assert_eq!(guard("tenant:acme:").prefix(), "tenant:acme:");
    }

    #[test]
    fn key_membership_uses_exact_prefix() {
        let guard = guard("tenant:acme");
        assert!(guard.is_in_namespace("tenant:acme:key"));
        assert!(!guard.is_in_namespace("tenant:acme2:key"));
        assert!(!guard.is_in_namespace("tenant:other:key"));
        assert!(!guard.is_in_namespace("tenant:acme"));
    }

    #[test]
    fn pattern_membership_uses_exact_prefix() {
        let guard = guard("tenant:acme");
        assert!(guard.pattern_in_namespace("tenant:acme:*"));
        assert!(!guard.pattern_in_namespace("tenant:acme2:*"));
    }

    #[test]
    fn deny_key_message_carries_redacted_key_and_namespace() {
        let guard = guard("tenant:acme");
        let err = guard.deny_key("tenant:other:session:secret-token-value");
        let msg = err.to_string();
        assert!(msg.contains("tenant:acme:"), "unexpected: {msg}");
        assert!(msg.contains("Namespace access denied"), "unexpected: {msg}");
        assert!(!msg.contains("secret-token-value"), "unexpected: {msg}");
    }

    #[test]
    fn deny_pattern_message_carries_namespace() {
        let guard = guard("tenant:acme");
        let err = guard.deny_pattern("other:*");
        let msg = err.to_string();
        assert!(msg.contains("other:*"), "unexpected: {msg}");
        assert!(msg.contains("tenant:acme:"), "unexpected: {msg}");
    }

    #[test]
    fn policy_is_exposed() {
        assert_eq!(guard("tenant:acme").policy(), NamespacePolicy::Isolated);
    }

    fn guarded() -> (NamespaceGuardBackend, Arc<MockBackend>) {
        let inner = Arc::new(MockBackend::new("mock", 100, false));
        let guard =
            NamespaceGuardBackend::new(inner.clone(), "tenant:acme", NamespacePolicy::Isolated);
        (guard, inner)
    }

    #[tokio::test]
    async fn denied_total_increments_for_each_rejection() {
        use crate::backend::CacheReader;
        let (guard, _inner) = guarded();
        assert_eq!(guard.denied_total(), 0);
        for _ in 0..3 {
            assert!(guard.get("tenant:other:key").await.is_err());
        }
        assert_eq!(guard.denied_total(), 3);
    }

    #[tokio::test]
    async fn in_namespace_access_does_not_increment_denied_total() {
        use crate::backend::CacheReader;
        let (guard, _inner) = guarded();
        guard.get("tenant:acme:key").await.unwrap();
        assert_eq!(guard.denied_total(), 0);
    }

    #[tokio::test]
    async fn out_of_namespace_get_is_denied_without_touching_inner() {
        let (guard, inner) = guarded();
        inner
            .set(Arc::from("tenant:other:key"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        let result = guard.get("tenant:other:key").await;
        assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
    }

    #[tokio::test]
    async fn out_of_namespace_set_is_denied_without_writing_inner() {
        use crate::backend::CacheWriter;
        let (guard, inner) = guarded();
        let result = guard
            .set(Arc::from("tenant:other:key"), Arc::new(b"v".to_vec()), None)
            .await;
        assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
        assert!(!inner.exists("tenant:other:key").await.unwrap());
    }

    #[tokio::test]
    async fn in_namespace_operations_pass_through() {
        use crate::backend::CacheWriter;
        let (guard, _inner) = guarded();
        guard
            .set(
                Arc::from("tenant:acme:key"),
                Arc::new(b"value".to_vec()),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            guard.get("tenant:acme:key").await.unwrap(),
            Some(b"value".to_vec())
        );
        assert!(guard.exists("tenant:acme:key").await.unwrap());
        guard.delete("tenant:acme:key").await.unwrap();
        assert_eq!(guard.get("tenant:acme:key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn out_of_namespace_delete_is_denied() {
        use crate::backend::CacheWriter;
        let (guard, inner) = guarded();
        inner
            .set(Arc::from("tenant:other:key"), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        let result = guard.delete("tenant:other:key").await;
        assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
        assert!(inner.exists("tenant:other:key").await.unwrap());
    }

    #[tokio::test]
    async fn out_of_namespace_pattern_is_denied() {
        use crate::backend::CacheReader;
        let (guard, _inner) = guarded();
        let result = guard.keys("tenant:other:*").await;
        assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
        let result = guard.delete_by_pattern("tenant:other:*").await;
        assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
    }

    #[tokio::test]
    async fn clear_is_always_denied() {
        use crate::backend::CacheWriter;
        let (guard, _inner) = guarded();
        let result = guard.clear().await;
        assert!(matches!(result, Err(OxCacheError::AccessDenied(_))));
    }

    fn quota_guard(
        max_entries: u64,
        max_bytes: u64,
        policy: OverflowPolicy,
    ) -> NamespaceGuardBackend {
        NamespaceGuardBackend::with_quota(
            Arc::new(MockBackend::new("mock", 100, false)),
            "tenant:acme",
            NamespacePolicy::Isolated,
            NamespaceQuota {
                max_entries,
                max_bytes,
                overflow_policy: policy,
            },
        )
    }

    #[tokio::test]
    async fn quota_reject_at_entry_limit_and_release_after_delete() {
        use crate::backend::CacheWriter;
        let guard = quota_guard(2, 1000, OverflowPolicy::Reject);
        guard
            .set(Arc::from("tenant:acme:k1"), Arc::new(vec![1u8; 10]), None)
            .await
            .unwrap();
        guard
            .set(Arc::from("tenant:acme:k2"), Arc::new(vec![1u8; 10]), None)
            .await
            .unwrap();
        assert_eq!(guard.quota_usage().0, 2);
        let err = guard
            .set(Arc::from("tenant:acme:k3"), Arc::new(vec![1u8; 10]), None)
            .await;
        assert!(matches!(err, Err(OxCacheError::AccessDenied(_))));
        assert_eq!(guard.quota_usage().0, 2);
        guard.delete("tenant:acme:k1").await.unwrap();
        assert_eq!(guard.quota_usage().0, 1);
        guard
            .set(Arc::from("tenant:acme:k3"), Arc::new(vec![1u8; 10]), None)
            .await
            .unwrap();
        assert_eq!(guard.quota_usage().0, 2);
    }

    #[tokio::test]
    async fn quota_reject_at_byte_limit() {
        use crate::backend::CacheWriter;
        let guard = quota_guard(10, 25, OverflowPolicy::Reject);
        guard
            .set(Arc::from("tenant:acme:k1"), Arc::new(vec![1u8; 20]), None)
            .await
            .unwrap();
        let err = guard
            .set(Arc::from("tenant:acme:k2"), Arc::new(vec![1u8; 10]), None)
            .await;
        assert!(matches!(err, Err(OxCacheError::AccessDenied(_))));
    }

    #[tokio::test]
    async fn evict_oldest_frees_space_for_new_key() {
        use crate::backend::{CacheReader, CacheWriter};
        let guard = quota_guard(2, 1000, OverflowPolicy::EvictOldestInNamespace);
        guard
            .set(Arc::from("tenant:acme:old"), Arc::new(vec![1u8; 10]), None)
            .await
            .unwrap();
        guard
            .set(Arc::from("tenant:acme:mid"), Arc::new(vec![1u8; 10]), None)
            .await
            .unwrap();
        guard
            .set(Arc::from("tenant:acme:new"), Arc::new(vec![1u8; 10]), None)
            .await
            .unwrap();
        assert_eq!(guard.quota_usage().0, 2);
        assert_eq!(guard.get("tenant:acme:old").await.unwrap(), None);
        assert!(guard.get("tenant:acme:mid").await.unwrap().is_some());
        assert!(guard.get("tenant:acme:new").await.unwrap().is_some());
    }

    #[cfg(feature = "metrics")]
    #[tokio::test]
    #[serial_test::serial]
    async fn quota_usage_gauges_track_writes_and_deletes() {
        use crate::backend::CacheWriter;
        use crate::infra::metrics::MetricValue;
        let guard = quota_guard(10, 10000, OverflowPolicy::Reject);
        guard
            .set(Arc::from("tenant:acme:k1"), Arc::new(vec![0u8; 100]), None)
            .await
            .unwrap();
        guard
            .set(Arc::from("tenant:acme:k2"), Arc::new(vec![0u8; 100]), None)
            .await
            .unwrap();
        let snapshot = crate::infra::GLOBAL_UNIFIED_METRICS.snapshot();
        assert!(matches!(
            snapshot.dynamic_metrics.get("oxcache_namespace_entries"),
            Some(MetricValue::Gauge(2.0))
        ));
        assert!(matches!(
            snapshot.dynamic_metrics.get("oxcache_namespace_bytes"),
            Some(MetricValue::Gauge(200.0))
        ));
        guard.delete("tenant:acme:k1").await.unwrap();
        guard.delete("tenant:acme:k2").await.unwrap();
        let snapshot = crate::infra::GLOBAL_UNIFIED_METRICS.snapshot();
        assert!(matches!(
            snapshot.dynamic_metrics.get("oxcache_namespace_entries"),
            Some(MetricValue::Gauge(0.0))
        ));
        assert!(matches!(
            snapshot.dynamic_metrics.get("oxcache_namespace_bytes"),
            Some(MetricValue::Gauge(0.0))
        ));
    }
}
