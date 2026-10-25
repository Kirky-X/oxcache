// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存审计事件流（`audit` feature）
//!
//! [`AuditEventPublisher`] 端口：get/set/delete 操作发布结构化审计事件
//! （脱敏 key + 操作 + 时间戳 + operator 元数据），满足 Repudiation 维度
//! 的审计留痕。
//!
//! - [`NoOpAuditPublisher`]：默认实现，零开销；
//! - [`InMemoryAuditPublisher`]：有界环形缓冲，测试/调试用；
//! - [`TracingAuditPublisher`]（`telemetry` feature）：桥接到 `tracing`
//!   结构化事件（供日志库等订阅端承接）。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::audit::{AuditEvent, AuditAction, NoOpAuditPublisher};
//!
//! let cache = Cache::builder()
//!     .audit_publisher(Arc::new(InMemoryAuditPublisher::new(1024)))
//!     .build().await?;
//! // get/set/delete 自动发布审计事件
//! ```

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 审计动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    /// 读取命中
    Hit,
    /// 读取未命中
    Miss,
    /// 写入
    Set,
    /// 删除
    Delete,
    /// 容量淘汰
    Evict,
    /// 过期移除
    Expired,
    /// 清空
    Clear,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::Hit => "hit",
            AuditAction::Miss => "miss",
            AuditAction::Set => "set",
            AuditAction::Delete => "delete",
            AuditAction::Evict => "evict",
            AuditAction::Expired => "expired",
            AuditAction::Clear => "clear",
        }
    }
}

/// 结构化审计事件
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// 操作类型
    pub action: AuditAction,
    /// 脱敏后的缓存键
    pub key: Option<String>,
    /// 命名空间（可选）
    pub namespace: Option<String>,
    /// 操作者上下文（可选，调用方注入）
    pub operator: Option<String>,
    /// 事件时间戳（毫秒）
    pub timestamp_ms: u64,
    /// 附加元数据
    pub metadata: Vec<(String, String)>,
}

impl AuditEvent {
    /// 创建审计事件
    pub fn new(action: AuditAction) -> Self {
        Self {
            action,
            key: None,
            namespace: None,
            operator: None,
            timestamp_ms: now_ms(),
            metadata: Vec::new(),
        }
    }

    /// 设置脱敏键
    pub fn with_key(mut self, redacted_key: impl Into<String>) -> Self {
        self.key = Some(redacted_key.into());
        self
    }

    /// 设置命名空间
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// 设置操作者上下文
    pub fn with_operator(mut self, operator: impl Into<String>) -> Self {
        self.operator = Some(operator.into());
        self
    }

    /// 附加元数据
    pub fn with_metadata(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.metadata.push((k.into(), v.into()));
        self
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_millis() as u64
}

/// 审计键脱敏：与缓存事件 / 日志同一口径
///
/// 统一走 [`redact_cache_key`](crate::security::redact_cache_key)：同一公共
/// 事件类型（审计事件、缓存事件）不得因发布点不同而出现两套脱敏格式，
/// 消费方按格式解析会不一致。
///
/// 键由调用方提供，可能含多字节 UTF-8；切分一律按字符边界进行，
/// 任意输入都不得 panic（字节下标切片会在字符中间切出而 panic）。
pub fn redact_key_for_audit(key: &str) -> String {
    crate::security::redact_cache_key(key)
}

/// 审计事件发布端口（对象安全，可 `Arc<dyn AuditEventPublisher>` 注入）
///
/// 实现应为**非阻塞**（审计发布失败/慢不得影响主操作语义）。
pub trait AuditEventPublisher: Send + Sync + 'static {
    /// 发布一条审计事件
    fn publish(&self, event: AuditEvent);
}

/// 空实现（默认）：零开销
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpAuditPublisher;

impl AuditEventPublisher for NoOpAuditPublisher {
    fn publish(&self, _event: AuditEvent) {}
}

/// 有界环形缓冲实现（测试 / 调试快照）
pub struct InMemoryAuditPublisher {
    events: Mutex<VecDeque<AuditEvent>>,
    capacity: usize,
}

impl InMemoryAuditPublisher {
    /// 创建指定容量（上限）的内存发布器
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
            capacity: capacity.max(1),
        }
    }

    /// 当前缓冲事件快照
    pub fn snapshot(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 已缓冲事件数
    pub fn len(&self) -> usize {
        self.events.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AuditEventPublisher for InMemoryAuditPublisher {
    fn publish(&self, event: AuditEvent) {
        if let Ok(mut q) = self.events.lock() {
            if q.len() >= self.capacity {
                q.pop_front();
            }
            q.push_back(event);
        }
    }
}

/// tracing 桥（`telemetry` feature）：审计事件 → 结构化 tracing 事件
#[cfg(feature = "telemetry")]
pub struct TracingAuditPublisher;

#[cfg(feature = "telemetry")]
impl AuditEventPublisher for TracingAuditPublisher {
    fn publish(&self, event: AuditEvent) {
        use crate::i18n::messages::{MSG_LOG_AUDIT_EVENT, t};
        tracing::info!(
            target: "oxcache::audit",
            action = event.action.as_str(),
            key = event.key.as_deref().unwrap_or(""),
            namespace = event.namespace.as_deref().unwrap_or(""),
            operator = event.operator.as_deref().unwrap_or(""),
            timestamp_ms = event.timestamp_ms,
            "{}",
            t(MSG_LOG_AUDIT_EVENT, &[])
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn noop_publisher_accepts_events() {
        let publisher = NoOpAuditPublisher;
        publisher.publish(AuditEvent::new(AuditAction::Set).with_key("k"));
        publisher.publish(AuditEvent::new(AuditAction::Evict));
    }

    #[test]
    fn in_memory_publisher_is_empty_tracks_buffer() {
        let publisher = InMemoryAuditPublisher::new(1);
        assert!(publisher.is_empty(), "a fresh buffer should be empty");
        publisher.publish(AuditEvent::new(AuditAction::Set).with_key("k"));
        assert!(!publisher.is_empty());
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn tracing_publisher_emits_one_structured_event_per_publish() {
        use std::fmt;
        use tracing::field::{Field, Visit};
        use tracing::span::{Attributes, Id, Record};
        use tracing::{Event, Metadata, Subscriber};

        struct CountingSubscriber {
            events: std::sync::Arc<Mutex<Vec<(tracing::Level, String, String)>>>,
        }

        struct FieldVisitor {
            action: String,
            key: String,
        }

        impl Visit for FieldVisitor {
            fn record_str(&mut self, field: &Field, value: &str) {
                match field.name() {
                    "action" => self.action = value.to_string(),
                    "key" => self.key = value.to_string(),
                    _ => {}
                }
            }

            fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
                self.record_str(field, &format!("{value:?}"));
            }
        }

        impl Subscriber for CountingSubscriber {
            fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
                true
            }

            fn new_span(&self, _attrs: &Attributes<'_>) -> Id {
                Id::from_u64(1)
            }

            fn record(&self, _span: &Id, _values: &Record<'_>) {}

            fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

            fn event(&self, event: &Event<'_>) {
                let mut visitor = FieldVisitor {
                    action: String::new(),
                    key: String::new(),
                };
                event.record(&mut visitor);
                self.events.lock().unwrap().push((
                    *event.metadata().level(),
                    visitor.action,
                    visitor.key,
                ));
            }

            fn enter(&self, _span: &Id) {}

            fn exit(&self, _span: &Id) {}
        }

        let captured = std::sync::Arc::new(Mutex::new(Vec::new()));
        let sink = captured.clone();
        tracing::subscriber::with_default(
            std::sync::Arc::new(CountingSubscriber { events: sink }),
            || {
                TracingAuditPublisher.publish(
                    AuditEvent::new(AuditAction::Delete)
                        .with_key("user:42")
                        .with_namespace("users")
                        .with_operator("svc"),
                );
            },
        );

        let events = captured.lock().unwrap();
        assert_eq!(
            events.len(),
            1,
            "one publish should produce one tracing event"
        );
        assert_eq!(events[0].0, tracing::Level::INFO);
        assert_eq!(events[0].1, "delete");
        assert_eq!(events[0].2, "user:42");
    }

    #[test]
    fn events_carry_action_key_timestamp_and_metadata() {
        let event = AuditEvent::new(AuditAction::Expired)
            .with_key("user:1")
            .with_namespace("users")
            .with_operator("svc-cache")
            .with_metadata("reason", "ttl");
        assert_eq!(event.action, AuditAction::Expired);
        assert_eq!(event.key.as_deref(), Some("user:1"));
        assert_eq!(event.namespace.as_deref(), Some("users"));
        assert_eq!(event.operator.as_deref(), Some("svc-cache"));
        assert_eq!(event.metadata[0], ("reason".to_string(), "ttl".to_string()));
        assert!(event.timestamp_ms > 0);
    }

    #[test]
    fn action_display_names() {
        assert_eq!(AuditAction::Hit.as_str(), "hit");
        assert_eq!(AuditAction::Miss.as_str(), "miss");
        assert_eq!(AuditAction::Evict.as_str(), "evict");
        assert_eq!(AuditAction::Expired.as_str(), "expired");
    }

    #[test]
    fn sensitive_keys_are_masked() {
        let redacted = redact_key_for_audit("user:api_key:abcdef");
        assert!(
            !redacted.contains("abcdef"),
            "sensitive values must not appear in full"
        );

        // 标识符形态的段（用户标识 / 订单号）不得原样落日志
        assert_eq!(redact_key_for_audit("user:42"), "user:<id>");
        assert_eq!(redact_key_for_audit("order:98765"), "order:<id>");
        assert_eq!(redact_key_for_audit("alice@example.com"), "<id>");
    }

    /// 审计 / 缓存事件 / 日志共用同一脱敏口径
    #[test]
    fn audit_and_cache_redaction_share_one_format() {
        for key in [
            "user:42",
            "order:98765",
            "alice@example.com",
            "user:api_key:abcdef",
            "plain_key",
        ] {
            assert_eq!(
                redact_key_for_audit(key),
                crate::security::redact_cache_key(key),
                "redaction format must be identical across publish sites: {key}"
            );
        }
    }

    /// 对象安全：Arc<dyn AuditEventPublisher> 注入形态可用
    #[test]
    fn publisher_is_object_safe() {
        let concrete = Arc::new(InMemoryAuditPublisher::new(8));
        let publisher: Arc<dyn AuditEventPublisher> = concrete.clone();
        publisher.publish(AuditEvent::new(AuditAction::Clear));
        // dyn 端口与具体实现共享同一缓冲
        assert_eq!(concrete.len(), 1);
    }

    /// 多字节键不得触发 panic：键由调用方提供，含非 ASCII 时按字节切片会
    /// 切在字符中间（byte index is not a char boundary）而中止整个请求。
    #[test]
    fn multibyte_keys_never_panic() {
        for len in 0..80usize {
            let key = "中".repeat(len);
            let _ = redact_key_for_audit(&key);
            let _ = redact_key_for_audit(&format!("token{key}"));
            let _ = redact_key_for_audit(&format!("{key}token"));
        }
        // 敏感分支按字符取后缀
        let redacted = redact_key_for_audit("token密");
        assert!(redacted.ends_with("密"), "got {redacted}");
        // 截断分支落在字符边界上
        let long = "a".repeat(120) + "密";
        let redacted = redact_key_for_audit(&long);
        assert!(redacted.starts_with(&"a".repeat(31)), "got {redacted}");
        assert!(redacted.ends_with("..."), "got {redacted}");
        // 多字节键的截断同样按字符边界进行
        let multibyte = "中".repeat(120);
        let redacted = redact_key_for_audit(&multibyte);
        assert!(redacted.ends_with("..."), "got {redacted}");
        assert_eq!(redacted.chars().count(), 100);
    }
}
