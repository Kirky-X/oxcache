// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存事件系统示例
//
//! 本示例演示如何使用 oxcache 的事件系统来监控缓存操作。
//
//! # 功能
//
//! - 创建和发布缓存事件
//! - 处理不同类型的缓存事件
//! - 使用事件进行性能监控和审计

use std::time::Instant;

use oxcache::{Cache, CacheEvent, CacheEventType, EventPublisher, OxCacheError};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_EVENTS_1_CREATING_AND_PUBLISHING, MSG_EXAMPLE_EVENTS_2_CONVENIENCE_METHODS,
    MSG_EXAMPLE_EVENTS_3_MONITORING_CACHE_OPERATION, MSG_EXAMPLE_EVENTS_4_BATCH_OPERATION_EVENTS,
    MSG_EXAMPLE_EVENTS_5_CUSTOM_EVENTS, MSG_EXAMPLE_EVENTS_6_ALL_EVENT_TYPES,
    MSG_EXAMPLE_EVENTS_EVENT_TYPE, MSG_EXAMPLE_EVENTS_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_EVENTS_OXCACHE_EVENT_SYSTEM_EXAMPLE, MSG_EXAMPLE_EVENTS_SET_OPERATION_TOOK, t,
};

/// 自定义事件发布器示例
///
/// 实现一个简单的事件发布器，将事件打印到控制台。
struct ConsoleEventPublisher;

#[async_trait::async_trait]
impl EventPublisher for ConsoleEventPublisher {
    /// 发布事件到控制台
    async fn publish(&self, event: CacheEvent) -> Result<(), OxCacheError> {
        let key_str = event.key.as_deref().unwrap_or("N/A");
        let latency_str = event
            .latency_ms
            .map(|l| format!("{}ms", l))
            .unwrap_or_else(|| "N/A".to_string());

        println!(
            "[EVENT] {} | key={} | latency={} | timestamp={}",
            event.event_type, key_str, latency_str, event.timestamp
        );

        if let Some(error) = &event.error {
            println!("        error: {}", error);
        }

        for (k, v) in &event.metadata {
            println!("        {}: {}", k, v);
        }
        Ok(())
    }

    /// 发布命中事件
    fn publish_hit(&self, key: String, latency_ms: u64) -> Result<(), OxCacheError> {
        println!("[HIT] key={} latency={}ms", key, latency_ms);
        Ok(())
    }

    /// 发布未命中事件
    fn publish_miss(&self, key: String, latency_ms: u64) -> Result<(), OxCacheError> {
        println!("[MISS] key={} latency={}ms", key, latency_ms);
        Ok(())
    }

    /// 发布设置事件
    fn publish_set(&self, key: String) -> Result<(), OxCacheError> {
        println!("[SET] key={}", key);
        Ok(())
    }

    /// 发布删除事件
    fn publish_delete(&self, key: String) -> Result<(), OxCacheError> {
        println!("[DELETE] key={}", key);
        Ok(())
    }

    /// 发布错误事件
    fn publish_error(&self, key: Option<String>, error: String) -> Result<(), OxCacheError> {
        let key_str = key.as_deref().unwrap_or("N/A");
        println!("[ERROR] key={} error={}", key_str, error);
        Ok(())
    }
}

/// 演示事件系统的基本用法
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!(
        "{}\n",
        t(MSG_EXAMPLE_EVENTS_OXCACHE_EVENT_SYSTEM_EXAMPLE, &[])
    );

    let publisher = ConsoleEventPublisher;

    println!("{}", t(MSG_EXAMPLE_EVENTS_1_CREATING_AND_PUBLISHING, &[]));

    let hit_event = CacheEvent::new(CacheEventType::Hit)
        .with_key("user:123")
        .with_latency(5);

    let miss_event = CacheEvent::new(CacheEventType::Miss)
        .with_key("user:456")
        .with_latency(12);

    let set_event = CacheEvent::new(CacheEventType::Set)
        .with_key("user:789")
        .with_metadata("size", "1024");

    let error_event = CacheEvent::new(CacheEventType::Error)
        .with_key("user:000")
        .with_error("Connection timeout");

    // 发布事件
    publisher.publish(hit_event).await?;
    publisher.publish(miss_event).await?;
    publisher.publish(set_event).await?;
    publisher.publish(error_event).await?;

    println!("\n{}", t(MSG_EXAMPLE_EVENTS_2_CONVENIENCE_METHODS, &[]));

    publisher.publish_hit("cache:key:1".to_string(), 3)?;
    publisher.publish_miss("cache:key:2".to_string(), 15)?;
    publisher.publish_set("cache:key:3".to_string())?;
    publisher.publish_delete("cache:key:4".to_string())?;
    publisher.publish_error(Some("cache:key:5".to_string()), "Key not found".to_string())?;

    println!(
        "\n{}",
        t(MSG_EXAMPLE_EVENTS_3_MONITORING_CACHE_OPERATION, &[])
    );

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(100).build().await?;

    let start = Instant::now();
    let test_key = String::from("test_key");
    let test_value = b"test_value".to_vec();
    cache.set(&test_key, &test_value).await?;
    let elapsed = start.elapsed();
    publisher.publish_set(test_key.clone())?;
    println!(
        "{}",
        t(
            MSG_EXAMPLE_EVENTS_SET_OPERATION_TOOK,
            &[("elapsed", format!("{:?}", elapsed))]
        )
    );

    let start = Instant::now();
    let result = cache.get(&test_key).await?;
    let elapsed = start.elapsed();

    if result.is_some() {
        publisher.publish_hit(test_key.clone(), elapsed.as_millis() as u64)?;
    } else {
        publisher.publish_miss(test_key.clone(), elapsed.as_millis() as u64)?;
    }

    let start = Instant::now();
    let nonexistent_key = String::from("nonexistent");
    let result = cache.get(&nonexistent_key).await?;
    let elapsed = start.elapsed();

    if result.is_some() {
        publisher.publish_hit(nonexistent_key.clone(), elapsed.as_millis() as u64)?;
    } else {
        publisher.publish_miss(nonexistent_key, elapsed.as_millis() as u64)?;
    }

    println!("\n{}", t(MSG_EXAMPLE_EVENTS_4_BATCH_OPERATION_EVENTS, &[]));

    let batch_start = CacheEvent::new(CacheEventType::BatchStart).with_metadata("count", "5");
    publisher.publish(batch_start).await?;

    for i in 0..5 {
        let key = format!("batch_key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        cache.set(&key, &value).await?;
        publisher.publish_set(key.clone())?;
    }

    let batch_end = CacheEvent::new(CacheEventType::BatchEnd)
        .with_metadata("count", "5")
        .with_latency(50);
    publisher.publish(batch_end).await?;

    println!("\n{}", t(MSG_EXAMPLE_EVENTS_5_CUSTOM_EVENTS, &[]));

    let custom_event = CacheEvent::new(CacheEventType::Custom("warmup_complete".to_string()))
        .with_metadata("loaded_keys", "100")
        .with_metadata("duration_ms", "500");

    publisher.publish(custom_event).await?;

    println!("\n{}", t(MSG_EXAMPLE_EVENTS_6_ALL_EVENT_TYPES, &[]));

    let all_types = [
        CacheEventType::Hit,
        CacheEventType::Miss,
        CacheEventType::Set,
        CacheEventType::Delete,
        CacheEventType::Expire,
        CacheEventType::Clear,
        CacheEventType::Get,
        CacheEventType::BatchStart,
        CacheEventType::BatchEnd,
        CacheEventType::Error,
        CacheEventType::Connect,
        CacheEventType::Disconnect,
        CacheEventType::Custom("custom_event".to_string()),
    ];

    for event_type in all_types {
        println!(
            "{}",
            t(
                MSG_EXAMPLE_EVENTS_EVENT_TYPE,
                &[("event_type", event_type.to_string())]
            )
        );
    }

    println!("\n{}", t(MSG_EXAMPLE_EVENTS_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
