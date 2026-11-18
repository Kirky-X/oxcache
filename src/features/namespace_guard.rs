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

use crate::backend::CacheBackend;
use crate::error::OxCacheError;
use crate::i18n::messages::{
    MSG_DETAIL_NAMESPACE_ACCESS_DENIED, MSG_DETAIL_NAMESPACE_PATTERN_ACCESS_DENIED, t,
};
use crate::security::redaction::redact_cache_key;
use std::sync::Arc;

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
#[allow(dead_code)] // T004 将接入转发路径；中间态允许未使用的字段与 helper
pub struct NamespaceGuardBackend {
    inner: Arc<dyn CacheBackend>,
    prefix: String,
    policy: NamespacePolicy,
}

#[allow(dead_code)] // T004 将接入转发路径；中间态允许未使用的 helper
impl NamespaceGuardBackend {
    /// 包装内层后端，声明命名空间与策略
    pub fn new(inner: Arc<dyn CacheBackend>, namespace: &str, policy: NamespacePolicy) -> Self {
        Self {
            inner,
            prefix: normalize_prefix(namespace),
            policy,
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
}
