// Copyright (c) 2025-2026 Kirky.X
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
            Some(entry) if !entry.is_expired() => {
                Ok(entry.expires_at.map(|t| t.saturating_duration_since(Instant::now())))
            }
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
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let mut data = self.data.lock().expect("lock poisoned");

        // 容量检查 — 简单 LRU 策略：满时清理过期条目
        if data.len() >= self.capacity && !data.contains_key(key.as_ref()) {
            data.retain(|_, entry| !entry.is_expired());
            if data.len() >= self.capacity {
                return Err(OxCacheError::BackendError("容量已满".into()));
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
        println!("  [HashMapBackend] 已关闭，数据已清理");
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
    println!("=== 自定义后端示例 ===\n");

    // 1. 创建自定义后端实例
    println!("--- 1. 创建自定义后端 ---");
    let backend = Arc::new(HashMapBackend::new(100));
    println!("  ✓ HashMapBackend 创建成功 (capacity=100)");

    // 2. 通过 CacheBuilder 注入自定义后端
    println!("\n--- 2. 通过 CacheBuilder 注入 ---");
    let cache: Cache<String, Item> = Cache::builder().backend_arc(backend.clone()).build().await?;
    println!("  ✓ Cache 使用自定义后端构建成功");

    // 3. 基本 CRUD 操作
    println!("\n--- 3. 基本 CRUD 操作 ---");
    let item = Item {
        id: 1,
        name: "测试项目".into(),
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

    // 4. TTL 操作
    println!("\n--- 4. TTL 操作 ---");
    cache
        .set_with_ttl(
            &"temp".to_string(),
            &Item {
                id: 2,
                name: "临时".into(),
            },
            Some(Duration::from_secs(60)),
        )
        .await?;
    let remaining = cache.ttl(&"temp".to_string()).await?;
    println!("  ✓ set_with_ttl 60s, 剩余 TTL: {:?}", remaining);

    let expired = cache.expire(&"temp".to_string(), Duration::from_secs(120)).await?;
    println!("  ✓ expire 更新为 120s: {}", expired);

    // 5. 批量操作
    println!("\n--- 5. 批量操作 ---");
    let items: Vec<(String, Item)> = (0..5)
        .map(|i| {
            (
                format!("batch:{}", i),
                Item {
                    id: i,
                    name: format!("项目{}", i),
                },
            )
        })
        .collect();
    for (key, item) in &items {
        cache.set(key, item).await?;
    }
    println!("  ✓ 批量写入 {} 条", items.len());

    let count = cache.len().await?;
    println!("  ✓ 缓存条目数: {}", count);

    // 6. 统计信息
    println!("\n--- 6. 统计信息 ---");
    let stats = cache.stats().await?;
    for (key, value) in &stats {
        println!("  {} = {}", key, value);
    }

    // 7. 健康检查
    println!("\n--- 7. 健康检查 ---");
    match cache.health_check().await {
        Ok(()) => println!("  ✓ 后端健康"),
        Err(e) => println!("  ✗ 健康检查失败: {}", e),
    }

    // 8. 容量限制演示
    println!("\n--- 8. 容量限制演示 ---");
    let small_backend = Arc::new(HashMapBackend::new(3));
    let small_cache: Cache<String, String> = Cache::builder().backend_arc(small_backend).build().await?;

    for i in 0..3 {
        small_cache.set(&format!("k{}", i), &format!("v{}", i)).await?;
    }
    println!("  ✓ 写入 3 条（容量=3）");

    match small_cache
        .set(&"k_overflow".to_string(), &"overflow".to_string())
        .await
    {
        Ok(()) => println!("  意外：写入成功"),
        Err(e) => println!("  ✓ 容量已满，写入失败: {}", e),
    }

    // 9. 关闭
    println!("\n--- 9. 关闭 ---");
    cache.shutdown().await;
    println!("  ✓ 缓存已关闭");

    println!("\n✓ 自定义后端示例完成");
    Ok(())
}
