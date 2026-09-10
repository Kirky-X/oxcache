// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 跨实例失效总线（`invalidation` feature，T301）
//!
//! 多实例部署下各实例的 L1 进程内缓存相互独立；本模块提供基于
//! Redis Pub/Sub 的失效广播总线：
//!
//! - 写路径经 [`InvalidatingBackend`] 装饰器在 set/delete 后广播失效事件
//!   （key / namespace 粒度）；
//! - 各实例通过 [`InvalidationBus::spawn_listener`] 订阅总线并将本地 L1
//!   对应条目失效；
//! - 消息携带 `instance_id`，**自身发出的消息被豁免**（不重复失效）。
//!
//! # Mock 协议层测试
//!
//! [`InMemoryPubSubTransport`] 在协议层模拟 Pub/Sub 语义（订阅后发布、
//! 广播给全部订阅者、at-most-once），供无真实 Redis 的单测使用；
//! [`RedisPubSubTransport`] 为真实 Redis 实现。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::invalidation::{InvalidationBus, InvalidationConfig, RedisPubSubTransport};
//!
//! let transport = RedisPubSubTransport::new("redis://localhost:6379").await?;
//! let bus = InvalidationBus::new(transport, InvalidationConfig::new("instance-a"));
//! let l1 = /* Arc<dyn CacheBackend> */;
//! bus.spawn_listener(l1).await?;          // 订阅并失效本地 L1
//! bus.invalidate_key("user:1").await?;    // 广播失效事件
//! ```

pub mod bus;
pub mod decorator;
pub mod keyspace;
pub mod transport;

pub use bus::{InvalidationBus, InvalidationConfig, ListenerHandle};
pub use decorator::InvalidatingBackend;
pub use keyspace::{KeyspaceNotificationConfig, KeyspaceNotificationListener};
pub use transport::{
    InMemoryPubSubTransport, PubSubTransport, RedisPubSubTransport, SubscriptionReceiver,
};

use crate::error::{OxCacheError, OxCacheResult};
use serde::{Deserialize, Serialize};

/// Pub/Sub 默认通道名
pub const DEFAULT_CHANNEL: &str = "oxcache:invalidate";

/// 失效事件类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvalidationKind {
    /// 精确 key 失效
    Key,
    /// 命名空间（前缀）失效
    Namespace,
}

/// 失效事件信封（Pub/Sub wire 格式为 JSON）
///
/// `instance_id` 用于自失效豁免：监听端丢弃与自身实例 ID 相同的消息，
/// 避免写路径广播后重复失效本实例 L1。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvalidationMessage {
    /// 发出该事件的实例 ID
    pub instance_id: String,
    /// 失效粒度
    pub kind: InvalidationKind,
    /// 失效目标（key 或 namespace 前缀；`*` 表示全部）
    pub target: String,
    /// 事件时间戳（毫秒）
    pub timestamp_ms: u64,
}

impl InvalidationMessage {
    /// 构造 key 粒度失效事件
    pub fn key(instance_id: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            kind: InvalidationKind::Key,
            target: key.into(),
            timestamp_ms: now_ms(),
        }
    }

    /// 构造命名空间粒度失效事件
    pub fn namespace(instance_id: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            kind: InvalidationKind::Namespace,
            target: namespace.into(),
            timestamp_ms: now_ms(),
        }
    }

    /// 编码为 wire 格式（JSON 字符串）
    pub fn encode(&self) -> OxCacheResult<String> {
        serde_json::to_string(self).map_err(|e| OxCacheError::Serialization(e.to_string()))
    }

    /// 从 wire 格式解码
    pub fn decode(payload: &str) -> OxCacheResult<Self> {
        serde_json::from_str(payload).map_err(|e| OxCacheError::Serialization(e.to_string()))
    }

    /// 该消息是否来自指定实例（自失效豁免判定）
    pub fn is_from(&self, instance_id: &str) -> bool {
        self.instance_id == instance_id
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_key_roundtrip() {
        let msg = InvalidationMessage::key("inst-a", "user:1");
        assert_eq!(msg.kind, InvalidationKind::Key);
        assert_eq!(msg.target, "user:1");

        let encoded = msg.encode().unwrap();
        let decoded = InvalidationMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn message_namespace_roundtrip() {
        let msg = InvalidationMessage::namespace("inst-b", "users:");
        assert_eq!(msg.kind, InvalidationKind::Namespace);
        let decoded = InvalidationMessage::decode(&msg.encode().unwrap()).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn message_wire_format_is_snake_case_json() {
        let msg = InvalidationMessage::key("i", "k");
        let wire = msg.encode().unwrap();
        assert!(wire.contains("\"kind\":\"key\""), "wire: {wire}");
        assert!(wire.contains("\"instance_id\":\"i\""), "wire: {wire}");
        assert!(wire.contains("\"target\":\"k\""), "wire: {wire}");
    }

    #[test]
    fn message_decode_rejects_garbage() {
        assert!(InvalidationMessage::decode("not json").is_err());
    }

    #[test]
    fn message_self_exemption_check() {
        let msg = InvalidationMessage::key("inst-a", "k");
        assert!(msg.is_from("inst-a"));
        assert!(!msg.is_from("inst-b"));
    }
}
