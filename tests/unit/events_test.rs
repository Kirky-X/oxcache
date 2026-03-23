// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 事件系统单元测试

use async_trait::async_trait;
use oxcache::events::{CacheEvent, CacheEventType, EventPublisher};

#[test]
fn test_cache_event_type_display() {
    assert_eq!(CacheEventType::Hit.to_string(), "hit");
    assert_eq!(CacheEventType::Miss.to_string(), "miss");
    assert_eq!(CacheEventType::Set.to_string(), "set");
    assert_eq!(CacheEventType::Delete.to_string(), "delete");
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
fn test_cache_event_type_custom() {
    let custom = CacheEventType::Custom("my_event".to_string());
    assert_eq!(custom.to_string(), "custom:my_event");
}

#[test]
fn test_cache_event_type_equality() {
    assert_eq!(CacheEventType::Hit, CacheEventType::Hit);
    assert_ne!(CacheEventType::Hit, CacheEventType::Miss);
}

#[test]
fn test_cache_event_new() {
    let event = CacheEvent::new(CacheEventType::Hit);
    assert_eq!(event.event_type, CacheEventType::Hit);
    assert!(event.key.is_none());
    assert!(event.latency_ms.is_none());
    assert!(event.error.is_none());
    assert!(event.metadata.is_empty());
}

#[test]
fn test_cache_event_with_key() {
    let event = CacheEvent::new(CacheEventType::Hit).with_key("user:123");
    assert_eq!(event.key, Some("user:123".to_string()));
}

#[test]
fn test_cache_event_with_latency() {
    let event = CacheEvent::new(CacheEventType::Get).with_latency(42);
    assert_eq!(event.latency_ms, Some(42));
}

#[test]
fn test_cache_event_with_error() {
    let event = CacheEvent::new(CacheEventType::Error).with_error("Connection refused");
    assert_eq!(event.error, Some("Connection refused".to_string()));
}

#[test]
fn test_cache_event_with_metadata() {
    let event = CacheEvent::new(CacheEventType::Hit)
        .with_metadata("node", "server1")
        .with_metadata("region", "us-east");

    assert_eq!(event.metadata.len(), 2);
    assert_eq!(event.metadata[0], ("node".to_string(), "server1".to_string()));
    assert_eq!(event.metadata[1], ("region".to_string(), "us-east".to_string()));
}

#[test]
fn test_cache_event_builder_pattern() {
    let event = CacheEvent::new(CacheEventType::Set)
        .with_key("item:456")
        .with_latency(15)
        .with_metadata("size", "1024");

    assert_eq!(event.event_type, CacheEventType::Set);
    assert_eq!(event.key, Some("item:456".to_string()));
    assert_eq!(event.latency_ms, Some(15));
    assert_eq!(event.metadata.len(), 1);
}

#[test]
fn test_cache_event_clone() {
    let event = CacheEvent::new(CacheEventType::Hit)
        .with_key("test_key")
        .with_latency(10);

    let cloned = event.clone();
    assert_eq!(event.event_type, cloned.event_type);
    assert_eq!(event.key, cloned.key);
    assert_eq!(event.latency_ms, cloned.latency_ms);
}

#[test]
fn test_cache_event_timestamp() {
    let event1 = CacheEvent::new(CacheEventType::Hit);
    std::thread::sleep(std::time::Duration::from_millis(10));
    let event2 = CacheEvent::new(CacheEventType::Hit);
    assert!(event2.timestamp >= event1.timestamp);
}

#[test]
fn test_cache_event_debug() {
    let event = CacheEvent::new(CacheEventType::Hit).with_key("test");
    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("Hit"));
    assert!(debug_str.contains("test"));
}

struct MockEventPublisher {
    events: std::sync::Arc<std::sync::Mutex<Vec<CacheEvent>>>,
}

impl MockEventPublisher {
    fn new() -> Self {
        Self {
            events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn get_events(&self) -> Vec<CacheEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl EventPublisher for MockEventPublisher {
    async fn publish(&self, event: CacheEvent) -> Result<(), oxcache::CacheError> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

#[tokio::test]
async fn test_event_publisher_publish() {
    let publisher = MockEventPublisher::new();
    let event = CacheEvent::new(CacheEventType::Hit).with_key("test_key");

    publisher.publish(event).await.unwrap();

    let events = publisher.get_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, CacheEventType::Hit);
    assert_eq!(events[0].key, Some("test_key".to_string()));
}

#[tokio::test]
async fn test_event_publisher_multiple_events() {
    let publisher = MockEventPublisher::new();

    publisher
        .publish(CacheEvent::new(CacheEventType::Hit).with_key("key1"))
        .await
        .unwrap();
    publisher
        .publish(CacheEvent::new(CacheEventType::Miss).with_key("key2"))
        .await
        .unwrap();
    publisher
        .publish(CacheEvent::new(CacheEventType::Set).with_key("key3"))
        .await
        .unwrap();

    let events = publisher.get_events();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, CacheEventType::Hit);
    assert_eq!(events[1].event_type, CacheEventType::Miss);
    assert_eq!(events[2].event_type, CacheEventType::Set);
}

#[test]
fn test_cache_event_error_with_all_fields() {
    let event = CacheEvent::new(CacheEventType::Error)
        .with_key("failed_key")
        .with_latency(100)
        .with_error("Timeout")
        .with_metadata("retry_count", "3");

    assert_eq!(event.event_type, CacheEventType::Error);
    assert_eq!(event.key, Some("failed_key".to_string()));
    assert_eq!(event.latency_ms, Some(100));
    assert_eq!(event.error, Some("Timeout".to_string()));
    assert_eq!(event.metadata.len(), 1);
}

#[test]
fn test_cache_event_batch_operations() {
    let start = CacheEvent::new(CacheEventType::BatchStart).with_metadata("batch_size", "100");
    let end = CacheEvent::new(CacheEventType::BatchEnd)
        .with_latency(500)
        .with_metadata("success_count", "98");

    assert_eq!(start.event_type, CacheEventType::BatchStart);
    assert_eq!(end.event_type, CacheEventType::BatchEnd);
    assert_eq!(end.latency_ms, Some(500));
}

#[test]
fn test_cache_event_connection_events() {
    let connect = CacheEvent::new(CacheEventType::Connect).with_metadata("host", "localhost:6379");
    let disconnect = CacheEvent::new(CacheEventType::Disconnect).with_metadata("reason", "timeout");

    assert_eq!(connect.event_type, CacheEventType::Connect);
    assert_eq!(disconnect.event_type, CacheEventType::Disconnect);
}
