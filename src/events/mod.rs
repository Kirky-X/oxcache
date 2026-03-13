// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
//! 缓存事件系统
//!
//! 提供缓存事件的发布和订阅机制，支持监控缓存操作、性能跟踪和自定义处理逻辑。

use async_trait::async_trait;
use std::fmt;

/// 缓存事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheEventType {
    /// 缓存命中
    Hit,
    /// 缓存未命中
    Miss,
    /// 缓存设置
    Set,
    /// 缓存删除
    Delete,
    /// 缓存过期
    Expire,
    /// 缓存清除
    Clear,
    /// 缓存获取（包含命中和未命中）
    Get,
    /// 批量操作开始
    BatchStart,
    /// 批量操作结束
    BatchEnd,
    /// 错误发生
    Error,
    /// 连接建立
    Connect,
    /// 连接断开
    Disconnect,
    /// 自定义事件
    Custom(String),
}

impl fmt::Display for CacheEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheEventType::Hit => write!(f, "hit"),
            CacheEventType::Miss => write!(f, "miss"),
            CacheEventType::Set => write!(f, "set"),
            CacheEventType::Delete => write!(f, "delete"),
            CacheEventType::Expire => write!(f, "expire"),
            CacheEventType::Clear => write!(f, "clear"),
            CacheEventType::Get => write!(f, "get"),
            CacheEventType::BatchStart => write!(f, "batch_start"),
            CacheEventType::BatchEnd => write!(f, "batch_end"),
            CacheEventType::Error => write!(f, "error"),
            CacheEventType::Connect => write!(f, "connect"),
            CacheEventType::Disconnect => write!(f, "disconnect"),
            CacheEventType::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

/// 缓存事件
#[derive(Debug, Clone)]
pub struct CacheEvent {
    /// 事件类型
    pub event_type: CacheEventType,
    /// 缓存键
    pub key: Option<String>,
    /// 事件时间戳（毫秒）
    pub timestamp: u64,
    /// 延迟（毫秒）
    pub latency_ms: Option<u64>,
    /// 错误信息（如果事件是错误）
    pub error: Option<String>,
    /// 额外数据
    pub metadata: Vec<(String, String)>,
}

impl CacheEvent {
    /// 创建新的缓存事件
    pub fn new(event_type: CacheEventType) -> Self {
        Self {
            event_type,
            key: None,
            timestamp: current_timestamp_ms(),
            latency_ms: None,
            error: None,
            metadata: Vec::new(),
        }
    }

    /// 设置缓存键
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// 设置延迟
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// 设置错误
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push((key.into(), value.into()));
        self
    }
}

/// 获取当前时间戳（毫秒）
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 事件发布器 Trait
///
/// 用于发布缓存事件。
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// 发布事件
    async fn publish(&self, event: CacheEvent);

    /// 发布命中事件
    fn publish_hit(&self, _key: impl Into<String>, _latency_ms: u64) {
        // 默认实现需要是同步的，因为 trait 中不能有异步默认实现
        // 子类可以覆盖这个方法提供异步版本
    }

    /// 发布未命中事件
    fn publish_miss(&self, _key: impl Into<String>, _latency_ms: u64) {
        // 默认实现需要是同步的
    }

    /// 发布设置事件
    fn publish_set(&self, _key: impl Into<String>) {
        // 默认实现需要是同步的
    }

    /// 发布删除事件
    fn publish_delete(&self, _key: impl Into<String>) {
        // 默认实现需要是同步的
    }

    /// 发布错误事件
    fn publish_error(&self, _key: Option<String>, _error: impl Into<String>) {
        // 默认实现需要是同步的
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_event_creation() {
        let event = CacheEvent::new(CacheEventType::Hit).with_key("test_key");
        assert_eq!(event.event_type, CacheEventType::Hit);
        assert_eq!(event.key, Some("test_key".to_string()));
    }

    #[test]
    fn test_cache_event_type_display() {
        assert_eq!(CacheEventType::Hit.to_string(), "hit");
        assert_eq!(CacheEventType::Miss.to_string(), "miss");
        assert_eq!(CacheEventType::Custom("test".to_string()).to_string(), "custom:test");
    }

    #[test]
    fn test_cache_event_with_value() {
        // CacheEvent doesn't have with_value method, test with available methods
        let event = CacheEvent::new(CacheEventType::Set).with_key("user:1").with_latency(5);
        assert_eq!(event.key, Some("user:1".to_string()));
        assert_eq!(event.latency_ms, Some(5));
    }

    #[test]
    fn test_cache_event_with_metadata() {
        let event = CacheEvent::new(CacheEventType::Miss)
            .with_key("missing_key")
            .with_latency(5)
            .with_metadata("test_node", "test_service");

        assert_eq!(event.latency_ms, Some(5));
        assert_eq!(event.metadata.len(), 1);
        assert_eq!(event.metadata[0], ("test_node".to_string(), "test_service".to_string()));
    }

    #[test]
    fn test_cache_event_error() {
        let event = CacheEvent::new(CacheEventType::Error)
            .with_key("error_key")
            .with_error("Connection timeout");

        assert_eq!(event.event_type, CacheEventType::Error);
        assert_eq!(event.error, Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_cache_event_types_complete() {
        // Test all event types have proper Display implementation
        assert_eq!(CacheEventType::Expire.to_string(), "expire");
        assert_eq!(CacheEventType::Clear.to_string(), "clear");
        assert_eq!(CacheEventType::Get.to_string(), "get");
        assert_eq!(CacheEventType::BatchStart.to_string(), "batch_start");
        assert_eq!(CacheEventType::BatchEnd.to_string(), "batch_end");
        assert_eq!(CacheEventType::Error.to_string(), "error");
        assert_eq!(CacheEventType::Connect.to_string(), "connect");
        assert_eq!(CacheEventType::Disconnect.to_string(), "disconnect");
    }

    #[test]
    fn test_cache_event_with_all_optional_fields() {
        let event = CacheEvent::new(CacheEventType::Get)
            .with_key("test_key")
            .with_latency(100)
            .with_metadata("node1", "service1")
            .with_error("test error");

        assert_eq!(event.event_type, CacheEventType::Get);
        assert_eq!(event.key, Some("test_key".to_string()));
        assert_eq!(event.latency_ms, Some(100));
        assert_eq!(event.metadata.len(), 1); // Only one metadata pair added
        assert_eq!(event.error, Some("test error".to_string()));
    }

    #[test]
    fn test_cache_event_clone() {
        let event = CacheEvent::new(CacheEventType::Hit).with_key("key").with_latency(10);

        let cloned = event.clone();
        assert_eq!(event.event_type, cloned.event_type);
        assert_eq!(event.key, cloned.key);
        assert_eq!(event.latency_ms, cloned.latency_ms);
    }
}
