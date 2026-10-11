// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 自定义后端示例
//!
//! 本示例演示如何实现自定义 CacheBackend：
//! - 实现 CacheReader / CacheWriter / CacheConnector trait 层次
//! - 通过 CacheBuilder::backend_arc() 注入自定义后端
//! - 基于 HashMap 的简单内存后端实现
//! - 后端统计与健康检查
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_custom_backend
//! ```

use async_trait::async_trait;
use oxcache::Cache;
use oxcache::backend::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use oxcache::error::{OxCacheError, OxCacheResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use oxcache::i18n::messages::{
    MSG_EXAMPLE_CUSTOM_BACKEND_1_CREATE_A_CUSTOM, MSG_EXAMPLE_CUSTOM_BACKEND_2_INJECT_IT_VIA,
    MSG_EXAMPLE_CUSTOM_BACKEND_3_BASIC_CRUD_OPERATIONS,
    MSG_EXAMPLE_CUSTOM_BACKEND_4_TTL_OPERATIONS, MSG_EXAMPLE_CUSTOM_BACKEND_5_BATCH_OPERATIONS,
    MSG_EXAMPLE_CUSTOM_BACKEND_6_STATISTICS, MSG_EXAMPLE_CUSTOM_BACKEND_7_HEALTH_CHECK,
    MSG_EXAMPLE_CUSTOM_BACKEND_8_CAPACITY_LIMIT_DEMONSTRATION,
    MSG_EXAMPLE_CUSTOM_BACKEND_9_SHUTDOWN, MSG_EXAMPLE_CUSTOM_BACKEND_BACKEND_HEALTHY,
    MSG_EXAMPLE_CUSTOM_BACKEND_BATCH_WROTE_ENTRIES,
    MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_BUILT_WITH_THE, MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_CLOSED,
    MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_ENTRIES, MSG_EXAMPLE_CUSTOM_BACKEND_CAPACITY_FULL_THE_WRITE,
    MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE, MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_CUSTOM_BACKEND_EXPIRE_UPDATED_TO_120S,
    MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_CREATED_CAPACITY_100,
    MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_SHUT_DOWN_DATA,
    MSG_EXAMPLE_CUSTOM_BACKEND_HEALTH_CHECK_FAILED, MSG_EXAMPLE_CUSTOM_BACKEND_SET_WITH_TTL_60S,
    MSG_EXAMPLE_CUSTOM_BACKEND_UNEXPECTED_THE_WRITE_SUCCEEDED,
    MSG_EXAMPLE_CUSTOM_BACKEND_WROTE_3_ENTRIES_CAPACITY, t,
};

/// 带 TTL 信息的缓存条目
struct Entry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

impl Entry {
    fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|t| Instant::now() > t)
    }
}

/// 自定义后端：基于 HashMap 的简单内存缓存
///
/// 这是一个教学示例，展示如何实现 CacheBackend trait。
/// 生产环境应使用内置的 MokaMemoryBackend 或 DashMapMemoryBackend。
struct HashMapBackend {
    data: Mutex<HashMap<String, Entry>>,
    capacity: usize,
}

impl HashMapBackend {
    fn new(capacity: usize) -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
            capacity,
        }
    }

    fn cleanup_expired(&self) {
        let mut data = self.data.lock().expect("lock poisoned");
        data.retain(|_, entry| !entry.is_expired());
    }
}

#[async_trait]
impl CacheReader for HashMapBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let data = self.data.lock().expect("lock poisoned");
        match data.get(key) {
            Some(entry) if !entry.is_expired() => Ok(Some(entry.value.clone())),
            _ => Ok(None),
        }
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        let data = self.data.lock().expect("lock poisoned");
        Ok(data.get(key).is_some_and(|e| !e.is_expired()))
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let data = self.data.lock().expect("lock poisoned");
        match data.get(key) {
            Some(entry) if !entry.is_expired() => Ok(entry
                .expires_at
                .map(|t| t.saturating_duration_since(Instant::now()))),
            _ => Ok(None),
        }
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.cleanup_expired();
        let data = self.data.lock().expect("lock poisoned");
        Ok(data.len() as u64)
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        Ok(self.capacity as u64)
    }

    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        self.cleanup_expired();
        let data = self.data.lock().expect("lock poisoned");
        let mut stats = HashMap::new();
        stats.insert("type".into(), "HashMapBackend".into());
        stats.insert("entry_count".into(), data.len().to_string());
        stats.insert("capacity".into(), self.capacity.to_string());
        Ok(stats)
    }
}

#[async_trait]
impl CacheWriter for HashMapBackend {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let mut data = self.data.lock().expect("lock poisoned");

        // 容量检查 — 简单 LRU 策略：满时清理过期条目
        if data.len() >= self.capacity && !data.contains_key(key.as_ref()) {
            data.retain(|_, entry| !entry.is_expired());
            if data.len() >= self.capacity {
                return Err(OxCacheError::BackendError("capacity full".into()));
            }
        }

        data.insert(
            key.to_string(),
            Entry {
                value: (*value).clone(),
                expires_at,
            },
        );
        Ok(())
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        let mut data = self.data.lock().expect("lock poisoned");
        data.remove(key);
        Ok(())
    }

    async fn clear(&self) -> OxCacheResult<()> {
        let mut data = self.data.lock().expect("lock poisoned");
        data.clear();
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let mut data = self.data.lock().expect("lock poisoned");
        match data.get_mut(key) {
            Some(entry) if !entry.is_expired() => {
                entry.expires_at = Some(Instant::now() + ttl);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[async_trait]
impl CacheConnector for HashMapBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        // HashMap 后端始终健康
        Ok(())
    }

    async fn shutdown(&self) {
        let mut data = self.data.lock().expect("lock poisoned");
        data.clear();
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_SHUT_DOWN_DATA,
                &[]
            )
        );
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Unknown
    }
}

// CacheBackend 通过 blanket impl 自动获得：
// impl<T: CacheReader + CacheWriter + CacheConnector + 'static> CacheBackend for T {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Item {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE, &[]));

    println!("{}", t(MSG_EXAMPLE_CUSTOM_BACKEND_1_CREATE_A_CUSTOM, &[]));
    let backend = Arc::new(HashMapBackend::new(100));
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_CREATED_CAPACITY_100,
            &[]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_CUSTOM_BACKEND_2_INJECT_IT_VIA, &[]));
    let cache: Cache<String, Item> = Cache::builder()
        .backend_arc(backend.clone())
        .build()
        .await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_BUILT_WITH_THE, &[])
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CUSTOM_BACKEND_3_BASIC_CRUD_OPERATIONS, &[])
    );
    let item = Item {
        id: 1,
        name: "test item".into(),
    };
    cache.set(&"item:1".to_string(), &item).await?;
    println!("  ✓ set item:1");

    let cached = cache.get(&"item:1".to_string()).await?;
    println!("  ✓ get item:1 = {:?}", cached);

    let exists = cache.exists(&"item:1".to_string()).await?;
    println!("  ✓ exists item:1 = {}", exists);

    cache.delete(&"item:1".to_string()).await?;
    println!("  ✓ delete item:1");

    let after_delete = cache.get(&"item:1".to_string()).await?;
    println!("  ✓ get after delete = {:?}", after_delete);

    println!("\n{}", t(MSG_EXAMPLE_CUSTOM_BACKEND_4_TTL_OPERATIONS, &[]));
    cache
        .set_with_ttl(
            &"temp".to_string(),
            &Item {
                id: 2,
                name: "temp".into(),
            },
            Some(Duration::from_secs(60)),
        )
        .await?;
    let remaining = cache.ttl(&"temp".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CUSTOM_BACKEND_SET_WITH_TTL_60S,
            &[("remaining", format!("{:?}", remaining))]
        )
    );

    let expired = cache
        .expire(&"temp".to_string(), Duration::from_secs(120))
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CUSTOM_BACKEND_EXPIRE_UPDATED_TO_120S,
            &[("expired", expired.to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CUSTOM_BACKEND_5_BATCH_OPERATIONS, &[])
    );
    let items: Vec<(String, Item)> = (0..5)
        .map(|i| {
            (
                format!("batch:{}", i),
                Item {
                    id: i,
                    name: format!("item {}", i),
                },
            )
        })
        .collect();
    for (key, item) in &items {
        cache.set(key, item).await?;
    }
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CUSTOM_BACKEND_BATCH_WROTE_ENTRIES,
            &[("items", items.len().to_string())]
        )
    );

    let count = cache.len().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_ENTRIES,
            &[("count", count.to_string())]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_CUSTOM_BACKEND_6_STATISTICS, &[]));
    let stats = cache.stats().await?;
    for (key, value) in &stats {
        println!("  {} = {}", key, value);
    }

    println!("\n{}", t(MSG_EXAMPLE_CUSTOM_BACKEND_7_HEALTH_CHECK, &[]));
    match cache.health_check().await {
        Ok(()) => println!("  {}", t(MSG_EXAMPLE_CUSTOM_BACKEND_BACKEND_HEALTHY, &[])),
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_CUSTOM_BACKEND_HEALTH_CHECK_FAILED,
                &[("e", e.to_string())]
            )
        ),
    }

    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_CUSTOM_BACKEND_8_CAPACITY_LIMIT_DEMONSTRATION,
            &[]
        )
    );
    let small_backend = Arc::new(HashMapBackend::new(3));
    let small_cache: Cache<String, String> =
        Cache::builder().backend_arc(small_backend).build().await?;

    for i in 0..3 {
        small_cache
            .set(&format!("k{}", i), &format!("v{}", i))
            .await?;
    }
    println!(
        "  {}",
        t(MSG_EXAMPLE_CUSTOM_BACKEND_WROTE_3_ENTRIES_CAPACITY, &[])
    );

    match small_cache
        .set(&"k_overflow".to_string(), &"overflow".to_string())
        .await
    {
        Ok(()) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_CUSTOM_BACKEND_UNEXPECTED_THE_WRITE_SUCCEEDED,
                &[]
            )
        ),
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_CUSTOM_BACKEND_CAPACITY_FULL_THE_WRITE,
                &[("e", e.to_string())]
            )
        ),
    }

    println!("\n{}", t(MSG_EXAMPLE_CUSTOM_BACKEND_9_SHUTDOWN, &[]));
    cache.shutdown().await;
    println!("  {}", t(MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_CLOSED, &[]));

    println!("\n{}", t(MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
