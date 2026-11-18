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

/// 命名空间边界守卫装饰器
pub struct NamespaceGuardBackend {
    inner: Arc<dyn CacheBackend>,
    prefix: String,
    policy: NamespacePolicy,
    denied_total: AtomicU64,
}

impl NamespaceGuardBackend {
    /// 包装内层后端，声明命名空间与策略
    pub fn new(inner: Arc<dyn CacheBackend>, namespace: &str, policy: NamespacePolicy) -> Self {
        Self {
            inner,
            prefix: normalize_prefix(namespace),
            policy,
            denied_total: AtomicU64::new(0),
        }
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
        self.inner.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        if !self.is_in_namespace(key) {
            return Err(self.deny_key(key));
        }
        self.inner.delete(key).await
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
        for (key, _, _) in items {
            if !self.is_in_namespace(key) {
                return Err(self.deny_key(key));
            }
        }
        self.inner.set_many(items).await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        for key in keys {
            if !self.is_in_namespace(key) {
                return Err(self.deny_key(key));
            }
        }
        self.inner.delete_many(keys).await
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
}
