// Copyright (c) 2025-2026 Kirky.X
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
    fn publish_hit(&self, key: impl Into<String>, latency_ms: u64) -> Result<(), OxCacheError> {
        println!("[HIT] key={} latency={}ms", key.into(), latency_ms);
        Ok(())
    }

    /// 发布未命中事件
    fn publish_miss(&self, key: impl Into<String>, latency_ms: u64) -> Result<(), OxCacheError> {
        println!("[MISS] key={} latency={}ms", key.into(), latency_ms);
        Ok(())
    }

    /// 发布设置事件
    fn publish_set(&self, key: impl Into<String>) -> Result<(), OxCacheError> {
        println!("[SET] key={}", key.into());
        Ok(())
    }

    /// 发布删除事件
    fn publish_delete(&self, key: impl Into<String>) -> Result<(), OxCacheError> {
        println!("[DELETE] key={}", key.into());
        Ok(())
    }

    /// 发布错误事件
    fn publish_error(&self, key: Option<String>, error: impl Into<String>) -> Result<(), OxCacheError> {
        let key_str = key.as_deref().unwrap_or("N/A");
        println!("[ERROR] key={} error={}", key_str, error.into());
        Ok(())
    }
}

/// 演示事件系统的基本用法
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== oxcache 事件系统示例 ===\n");

    // 1. 创建事件发布器
    let publisher = ConsoleEventPublisher;

    println!("--- 1. 创建和发布事件 ---");

    // 创建不同类型的事件
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

    // 2. 使用便捷方法发布事件
    println!("\n--- 2. 使用便捷方法 ---");

    publisher.publish_hit("cache:key:1", 3)?;
    publisher.publish_miss("cache:key:2", 15)?;
    publisher.publish_set("cache:key:3")?;
    publisher.publish_delete("cache:key:4")?;
    publisher.publish_error(Some("cache:key:5".to_string()), "Key not found")?;

    // 3. 实际缓存操作中的事件监控
    println!("\n--- 3. 缓存操作事件监控 ---");

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(100).build().await?;

    // 设置值并记录事件
    let start = Instant::now();
    let test_key = String::from("test_key");
    let test_value = b"test_value".to_vec();
    cache.set(&test_key, &test_value).await?;
    let elapsed = start.elapsed();
    publisher.publish_set(&test_key)?;
    println!("设置操作耗时: {:?}", elapsed);

    // 获取值并记录事件
    let start = Instant::now();
    let result = cache.get(&test_key).await?;
    let elapsed = start.elapsed();

    if result.is_some() {
        publisher.publish_hit(&test_key, elapsed.as_millis() as u64)?;
    } else {
        publisher.publish_miss(&test_key, elapsed.as_millis() as u64)?;
    }

    // 获取不存在的值
    let start = Instant::now();
    let nonexistent_key = String::from("nonexistent");
    let result = cache.get(&nonexistent_key).await?;
    let elapsed = start.elapsed();

    if result.is_some() {
        publisher.publish_hit(&nonexistent_key, elapsed.as_millis() as u64)?;
    } else {
        publisher.publish_miss(&nonexistent_key, elapsed.as_millis() as u64)?;
    }

    // 4. 批量操作事件
    println!("\n--- 4. 批量操作事件 ---");

    let batch_start = CacheEvent::new(CacheEventType::BatchStart).with_metadata("count", "5");
    publisher.publish(batch_start).await?;

    for i in 0..5 {
        let key = format!("batch_key_{}", i);
        let value = format!("value_{}", i).into_bytes();
        cache.set(&key, &value).await?;
        publisher.publish_set(&key)?;
    }

    let batch_end = CacheEvent::new(CacheEventType::BatchEnd)
        .with_metadata("count", "5")
        .with_latency(50);
    publisher.publish(batch_end).await?;

    // 5. 自定义事件
    println!("\n--- 5. 自定义事件 ---");

    let custom_event = CacheEvent::new(CacheEventType::Custom("warmup_complete".to_string()))
        .with_metadata("loaded_keys", "100")
        .with_metadata("duration_ms", "500");

    publisher.publish(custom_event).await?;

    // 6. 所有事件类型展示
    println!("\n--- 6. 所有事件类型 ---");

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
        println!("事件类型: {}", event_type);
    }

    println!("\n示例完成！");
    Ok(())
}
