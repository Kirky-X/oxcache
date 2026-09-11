// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 缓存审计事件流（`audit` feature，T309）
//!
//! [`AuditEventPublisher`] 端口：get/set/delete/evict/expired 等操作发布
//! 结构化审计事件（脱敏 key + 操作 + 时间戳 + operator 元数据），满足
//! Repudiation 维度的审计留痕。
//!
//! - [`NoOpAuditPublisher`]：默认实现，零开销；
//! - [`InMemoryAuditPublisher`]：有界环形缓冲，测试/调试用；
//! - [`TracingAuditPublisher`]（`telemetry` feature）：桥接到 `tracing`
//!   结构化事件（供 inklog 等订阅端承接，M3 方向）。
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

/// 审计键脱敏：敏感模式掩码 + 长键截断
pub fn redact_key_for_audit(key: &str) -> String {
    const SENSITIVE: [&str; 4] = ["token", "password", "secret", "api_key"];
    const MAX_LEN: usize = 32;
    let lower = key.to_ascii_lowercase();
    if SENSITIVE.iter().any(|p| lower.contains(p)) {
        let suffix = if key.len() > 2 { &key[key.len() - 2..] } else { key };
        return format!("<sensitive>…{suffix}");
    }
    if key.len() > MAX_LEN {
        format!("{}…(len={})", &key[..MAX_LEN], key.len())
    } else {
        key.to_string()
    }
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
        tracing::info!(
            target: "oxcache::audit",
            action = event.action.as_str(),
            key = event.key.as_deref().unwrap_or(""),
            namespace = event.namespace.as_deref().unwrap_or(""),
            operator = event.operator.as_deref().unwrap_or(""),
            timestamp_ms = event.timestamp_ms,
            "cache audit event"
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
    fn in_memory_publisher_ring_buffers() {
        let publisher = InMemoryAuditPublisher::new(3);
        for i in 0..5u32 {
            publisher.publish(AuditEvent::new(AuditAction::Hit).with_key(format!("k{i}")));
        }
        assert_eq!(publisher.len(), 3, "环形缓冲应保持容量上限");
        let events = publisher.snapshot();
        // 最旧的被挤掉
        assert_eq!(events[0].key.as_deref(), Some("k2"));
        assert_eq!(events[2].key.as_deref(), Some("k4"));
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
        assert!(redacted.starts_with("<sensitive>"), "got {redacted}");
        assert!(!redacted.contains("abcdef"), "敏感值不得完整出现");

        let long = "k".repeat(100);
        let redacted = redact_key_for_audit(&long);
        assert!(redacted.contains("len=100"), "长键应截断并带长度标注");
        assert!(redacted.len() < 50);

        let normal = redact_key_for_audit("user:1");
        assert_eq!(normal, "user:1");
    }

    /// 对象安全：Arc<dyn AuditEventPublisher> 注入形态可用
    #[test]
    fn publisher_is_object_safe() {
        let concrete = Arc::new(InMemoryAuditPublisher::new(8));
        let publisher: Arc<dyn AuditEventPublisher> = concrete.clone();
        publisher.publish(AuditEvent::new(AuditAction::Clear));
        // dyn 端口与具体类型共享同一缓冲
        assert_eq!(concrete.len(), 1);
    }
}
