//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Moka-based memory backend implementation

use crate::backend::interface::{BackendKind, CacheConnector, CacheReader, CacheWriter};
// Sync trait 实现使用全限定路径（`crate::backend::interface::SyncCacheReader`），
// 避免将 sync trait 名导入本模块作用域后，经 `mod tests` 的 `use super::*`
// 与同名 async trait 方法（如 `get`）产生歧义。
use crate::backend::score::{BackendScore, Scores};
use crate::error::Result;
use crate::impl_backend_builder;
use async_trait::async_trait;
use moka::ops::compute::{CompResult, Op};
use moka::Expiry;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Moka 缓存条目：承载 value 与 per-entry 过期时间戳。
///
/// `expires_at=None` 表示该条目无 per-entry TTL（沿用 moka 全局 time_to_live /
/// time_to_idle 策略，或永不过期）。`Some(Instant)` 表示在该时刻过期。
///
/// 通过 [`MokaExpiry`] 将 `expires_at` 暴露给 moka 淘汰策略，使 moka 在
/// `expire_after_create` / `expire_after_update` 时知道真实过期时间。
#[derive(Clone, Debug)]
pub struct MokaEntry {
    pub value: Vec<u8>,
    pub expires_at: Option<Instant>,
}

/// [`Expiry`] 实现：把 [`MokaEntry`] 的 `expires_at` 转换为 moka 期望的
/// "从创建/更新时刻起的剩余 Duration"。
///
/// `expire_after_read` 使用默认实现（返回 `duration_until_expiry`，不变更过期），
/// 保证读操作不会意外延长或缩短 TTL。
#[derive(Default, Clone)]
pub struct MokaExpiry;

impl Expiry<String, MokaEntry> for MokaExpiry {
    fn expire_after_create(&self, _key: &String, val: &MokaEntry, created_at: Instant) -> Option<Duration> {
        val.expires_at.map(|e| e.saturating_duration_since(created_at))
    }

    fn expire_after_update(
        &self,
        _key: &String,
        val: &MokaEntry,
        updated_at: Instant,
        _duration_until_expiry: Option<Duration>,
    ) -> Option<Duration> {
        val.expires_at.map(|e| e.saturating_duration_since(updated_at))
    }
}

/// Moka-based memory backend
///
/// This backend uses Moka's high-performance in-memory cache with
/// LRU/TinyLFU eviction policies and built-in TTL support.
#[derive(Clone)]
pub struct MokaMemoryBackend {
    cache: Arc<moka::future::Cache<String, MokaEntry>>,
    capacity: u64,
}

impl_backend_builder!(MokaMemoryBackend, MokaMemoryBackendBuilder);

impl MokaMemoryBackend {
    /// Get the capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get the entry count
    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }
}

impl Default for MokaMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MokaMemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MokaMemoryBackend")
            .field("capacity", &self.capacity)
            .field("entry_count", &self.cache.entry_count())
            .finish()
    }
}

#[async_trait]
impl CacheReader for MokaMemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.cache.get(key).await.map(|e| e.value))
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.cache.contains_key(key))
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let now = Instant::now();
        Ok(self
            .cache
            .get(key)
            .await
            .and_then(|e| e.expires_at.and_then(|exp| exp.checked_duration_since(now))))
    }

    async fn len(&self) -> Result<u64> {
        Ok(self.cache.entry_count())
    }

    async fn is_empty(&self) -> Result<bool> {
        Ok(self.cache.entry_count() == 0)
    }

    async fn capacity(&self) -> Result<u64> {
        Ok(self.capacity)
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "moka".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert("entry_count".to_string(), self.cache.entry_count().to_string());
        Ok(stats)
    }
}

#[async_trait]
impl CacheWriter for MokaMemoryBackend {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let entry = MokaEntry { value, expires_at };
        self.cache.insert(key.to_string(), entry).await;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.cache.invalidate(key).await;
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let new_expires_at = Instant::now() + ttl;
        let result = self
            .cache
            .entry(key.to_string())
            .and_compute_with(|maybe_entry: Option<moka::Entry<String, MokaEntry>>| async move {
                match maybe_entry {
                    Some(entry) => {
                        let mut old = entry.into_value();
                        old.expires_at = Some(new_expires_at);
                        Op::Put(old)
                    }
                    None => Op::Nop,
                }
            })
            .await;
        match result {
            CompResult::ReplacedWith(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

#[async_trait]
impl CacheConnector for MokaMemoryBackend {
    async fn health_check(&self) -> Result<()> {
        // Moka is always healthy as it's in-memory
        Ok(())
    }

    async fn shutdown(&self) {
        self.cache.invalidate_all();
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Moka
    }
}

// ============================================================================
// Synchronous trait implementations (任务组 6)
// ============================================================================
//
// Moka 0.12 的 `future::Cache` 未暴露 `blocking_*` 方法，但 `get`/`insert`/
// `invalidate` 的前台 future 不依赖 tokio runtime 驱动（无 `tokio::spawn`/
// `tokio::time` 调用），可通过 `block_on` 安全轮询。`sync_block_on` 优先复用
// 当前 multi-thread runtime（`block_in_place`），否则惰性创建 current-thread
// runtime 供同步路径使用。

fn sync_block_on<F: std::future::Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| handle.block_on(fut))
        }
        _ => {
            use std::sync::OnceLock;
            static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            let rt = RT.get_or_init(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .expect("failed to build tokio runtime for sync moka ops")
            });
            rt.block_on(fut)
        }
    }
}

impl crate::backend::interface::SyncCacheReader for MokaMemoryBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(sync_block_on(self.cache.get(key)).map(|e| e.value))
    }

    fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.cache.contains_key(key))
    }

    fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        let now = Instant::now();
        Ok(sync_block_on(self.cache.get(key))
            .and_then(|e| e.expires_at.and_then(|exp| exp.checked_duration_since(now))))
    }

    fn len(&self) -> Result<u64> {
        Ok(self.cache.entry_count())
    }

    fn capacity(&self) -> Result<u64> {
        Ok(self.capacity)
    }

    fn stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "moka".to_string());
        stats.insert("capacity".to_string(), self.capacity.to_string());
        stats.insert("entry_count".to_string(), self.cache.entry_count().to_string());
        Ok(stats)
    }
}

impl crate::backend::interface::SyncCacheWriter for MokaMemoryBackend {
    fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let entry = MokaEntry { value, expires_at };
        sync_block_on(self.cache.insert(key.to_string(), entry));
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        sync_block_on(self.cache.invalidate(key));
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let new_expires_at = Instant::now() + ttl;
        let result = sync_block_on(self.cache.entry(key.to_string()).and_compute_with(
            |maybe_entry: Option<moka::Entry<String, MokaEntry>>| async move {
                match maybe_entry {
                    Some(entry) => {
                        let mut old = entry.into_value();
                        old.expires_at = Some(new_expires_at);
                        Op::Put(old)
                    }
                    None => Op::Nop,
                }
            },
        ));
        match result {
            CompResult::ReplacedWith(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

impl crate::backend::interface::SyncCacheConnector for MokaMemoryBackend {
    fn health_check(&self) -> Result<()> {
        // Moka is always healthy as it's in-memory
        Ok(())
    }

    fn shutdown(&self) {
        self.cache.invalidate_all();
    }

    fn backend_kind(&self) -> BackendKind {
        BackendKind::Moka
    }
}

// CacheBackend is automatically implemented via blanket implementation

impl BackendScore for MokaMemoryBackend {
    fn score(&self) -> u8 {
        Scores::MOKA
    }

    fn is_persistent(&self) -> bool {
        false
    }

    fn backend_name(&self) -> &'static str {
        "moka"
    }
}

/// Builder for MokaMemoryBackend
#[derive(Default)]
pub struct MokaMemoryBackendBuilder {
    capacity: u64,
    ttl: Option<Duration>,
    time_to_idle: Option<Duration>,
}

impl MokaMemoryBackendBuilder {
    /// Set the maximum number of entries
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the time-to-live for entries
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the time-to-idle for entries
    pub fn time_to_idle(mut self, ttl: Duration) -> Self {
        self.time_to_idle = Some(ttl);
        self
    }

    /// Build the Moka backend
    pub fn build(self) -> MokaMemoryBackend {
        // Use a reasonable default capacity if not set
        let capacity = if self.capacity > 0 {
            self.capacity
        } else {
            10_000 // Default capacity of 10,000 entries
        };

        let mut builder = moka::future::Cache::builder()
            .max_capacity(capacity)
            .expire_after(MokaExpiry);

        if let Some(ttl) = self.ttl {
            builder = builder.time_to_live(ttl);
        }

        if let Some(tti) = self.time_to_idle {
            builder = builder.time_to_idle(tti);
        }

        let cache = Arc::new(builder.build());

        MokaMemoryBackend { cache, capacity }
    }
}

/// Convenience function to create a Moka memory backend
pub fn moka_memory() -> MokaMemoryBackend {
    MokaMemoryBackend::new()
}

/// Convenience function to create a Moka memory backend with capacity
pub fn moka_memory_with_capacity(capacity: u64) -> MokaMemoryBackend {
    MokaMemoryBackend::builder().capacity(capacity).build()
}

/// Convenience function to create a Moka memory backend with capacity and TTL
pub fn moka_memory_with_capacity_and_ttl(capacity: u64, ttl: Duration) -> MokaMemoryBackend {
    MokaMemoryBackend::builder().capacity(capacity).ttl(ttl).build()
}

/// Default memory backend (Moka-based)
pub fn default_memory_backend() -> MokaMemoryBackend {
    moka_memory()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moka_backend_builder() {
        let backend = MokaMemoryBackend::builder()
            .capacity(1000)
            .ttl(Duration::from_secs(3600))
            .time_to_idle(Duration::from_secs(1800))
            .build();

        assert_eq!(backend.capacity(), 1000);
    }

    #[test]
    fn test_moka_backend_default() {
        let backend = MokaMemoryBackend::default();
        // Default capacity should be reasonable
        assert!(backend.capacity() > 0);
    }

    #[tokio::test]
    async fn test_moka_basic_operations() {
        let backend = MokaMemoryBackend::new();

        // Set a value
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();

        // Use tokio::time::sleep to ensure async operations complete
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Get the value
        let result = backend.get("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        // Check exists
        let exists = backend.exists("key1").await.unwrap();
        assert!(exists);

        // Delete
        backend.delete("key1").await.unwrap();

        // Verify deletion
        let exists_after = backend.exists("key1").await.unwrap();
        assert!(!exists_after);
    }

    #[test]
    fn test_convenience_functions() {
        let backend1 = moka_memory();
        let backend2 = moka_memory_with_capacity(1000);
        let backend3 = moka_memory_with_capacity_and_ttl(1000, Duration::from_secs(3600));

        assert!(backend1.capacity() > 0);
        assert_eq!(backend2.capacity(), 1000);
        assert_eq!(backend3.capacity(), 1000);
    }

    // ========================================================================
    // Per-entry TTL tests (spec: universal-per-entry-ttl)
    // ========================================================================

    #[tokio::test]
    async fn test_moka_set_with_ttl_expires_after_timeout() {
        let backend = MokaMemoryBackend::new();
        backend
            .set("k", b"v".to_vec(), Some(Duration::from_millis(50)))
            .await
            .unwrap();
        // 立即可读
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
        // 等待 100ms 后应过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        // moka 异步清理可能略有延迟，循环等待最多 500ms 确保过期
        let mut expired = false;
        for _ in 0..10 {
            if backend.get("k").await.unwrap().is_none() {
                expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(expired, "entry should expire after TTL");
    }

    #[tokio::test]
    async fn test_moka_set_with_ttl_readable_within_window() {
        let backend = MokaMemoryBackend::new();
        backend
            .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        // 60s TTL 内应可读
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn test_moka_set_without_ttl_uses_global_ttl() {
        // 用全局 30s TTL 构建后端
        let backend = MokaMemoryBackend::builder()
            .capacity(1000)
            .ttl(Duration::from_secs(30))
            .build();
        backend.set("k", b"v".to_vec(), None).await.unwrap();
        // 立即可读
        assert_eq!(backend.get("k").await.unwrap(), Some(b"v".to_vec()));
        // 全局 TTL 查询（per-entry 未设置时返回 None，符合 spec "无 TTL 键返回 None"）
        let ttl = backend.ttl("k").await.unwrap();
        assert_eq!(ttl, None, "set(None) with global TTL should report None per-entry");
    }

    #[tokio::test]
    async fn test_moka_ttl_returns_remaining() {
        let backend = MokaMemoryBackend::new();
        backend
            .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let ttl = backend.ttl("k").await.unwrap().expect("ttl should be Some");
        // 58s < ttl <= 60s
        assert!(
            ttl > Duration::from_secs(58),
            "ttl={} should be > 58s",
            ttl.as_secs_f64()
        );
        assert!(
            ttl <= Duration::from_secs(60),
            "ttl={} should be <= 60s",
            ttl.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn test_moka_ttl_returns_none_for_missing_key() {
        let backend = MokaMemoryBackend::new();
        assert_eq!(backend.ttl("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_moka_ttl_returns_none_for_no_ttl_key() {
        let backend = MokaMemoryBackend::new();
        backend.set("k", b"v".to_vec(), None).await.unwrap();
        assert_eq!(backend.ttl("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_moka_expire_extends_ttl() {
        let backend = MokaMemoryBackend::new();
        backend
            .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let ok = backend.expire("k", Duration::from_secs(120)).await.unwrap();
        assert!(ok, "expire on existing key should return true");
        let ttl = backend
            .ttl("k")
            .await
            .unwrap()
            .expect("ttl should be Some after expire");
        assert!(
            ttl > Duration::from_secs(118),
            "ttl={} should be > 118s",
            ttl.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn test_moka_expire_shrinks_ttl() {
        let backend = MokaMemoryBackend::new();
        backend
            .set("k", b"v".to_vec(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        let ok = backend.expire("k", Duration::from_millis(50)).await.unwrap();
        assert!(ok, "expire on existing key should return true");
        // 等待 100ms 后应过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut expired = false;
        for _ in 0..10 {
            if backend.get("k").await.unwrap().is_none() {
                expired = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(expired, "entry should expire after shrunk TTL");
    }

    #[tokio::test]
    async fn test_moka_expire_missing_key_returns_false() {
        let backend = MokaMemoryBackend::new();
        let ok = backend.expire("missing", Duration::from_secs(60)).await.unwrap();
        assert!(!ok, "expire on missing key should return false");
    }

    // ========================================================================
    // Synchronous trait hierarchy tests (任务组 6)
    //
    // 隔离在嵌套 `mod sync_tests` 内：sync trait 的 import 仅在此模块可见，
    // 避免与父模块 `mod tests` 中 async `CacheReader::get` 等同名方法产生
    // 歧义。方法调用通过 trait object (`&dyn SyncCacheReader` 等) 消歧。
    // ========================================================================
    mod sync_tests {
        use super::MokaMemoryBackend;
        use crate::backend::interface::{BackendKind, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};
        use std::time::Duration;

        #[test]
        fn test_moka_sync_get_set_basic() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("key1", b"value1".to_vec(), None).unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            assert_eq!(reader.get("key1").unwrap(), Some(b"value1".to_vec()));
            assert!(reader.exists("key1").unwrap());
            assert!(!reader.exists("key2").unwrap());
            assert!(reader.capacity().unwrap() > 0);

            let stats = reader.stats().unwrap();
            assert_eq!(stats.get("type"), Some(&"moka".to_string()));
        }

        #[test]
        fn test_moka_sync_set_with_ttl_expires() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("k", b"v".to_vec(), Some(Duration::from_millis(50))).unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            // 立即可读
            assert_eq!(reader.get("k").unwrap(), Some(b"v".to_vec()));

            // 等待过期（moka 读时按 per-entry TTL 校验，无需后台驱逐）
            std::thread::sleep(Duration::from_millis(120));
            let mut expired = false;
            for _ in 0..10 {
                if reader.get("k").unwrap().is_none() {
                    expired = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(expired, "entry should expire after TTL via sync get");
        }

        #[test]
        fn test_moka_sync_ttl_returns_remaining() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("k", b"v".to_vec(), Some(Duration::from_secs(60))).unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            let ttl = reader.ttl("k").unwrap().expect("ttl should be Some for TTL'd key");
            assert!(
                ttl > Duration::from_secs(58),
                "ttl={} should be > 58s",
                ttl.as_secs_f64()
            );
            assert!(
                ttl <= Duration::from_secs(60),
                "ttl={} should be <= 60s",
                ttl.as_secs_f64()
            );

            // 无 TTL 的 key 返回 None
            writer.set("no_ttl", b"v".to_vec(), None).unwrap();
            assert_eq!(reader.ttl("no_ttl").unwrap(), None);
            // 不存在的 key 返回 None
            assert_eq!(reader.ttl("missing").unwrap(), None);
        }

        #[test]
        fn test_moka_sync_expire_works() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("k", b"v".to_vec(), Some(Duration::from_secs(60))).unwrap();

            // expire 已存在 key → true，TTL 延长至 120s
            let ok = writer.expire("k", Duration::from_secs(120)).unwrap();
            assert!(ok, "expire on existing key should return true");

            let reader: &dyn SyncCacheReader = &backend;
            let new_ttl = reader.ttl("k").unwrap().expect("ttl should be Some after expire");
            assert!(
                new_ttl > Duration::from_secs(118),
                "new_ttl={} should be > 118s",
                new_ttl.as_secs_f64()
            );

            // expire 不存在 key → false
            let ok = writer.expire("missing", Duration::from_secs(10)).unwrap();
            assert!(!ok, "expire on missing key should return false");
        }

        #[test]
        fn test_moka_sync_delete_clear() {
            let backend = MokaMemoryBackend::new();

            let writer: &dyn SyncCacheWriter = &backend;
            writer.set("k1", b"v1".to_vec(), None).unwrap();
            writer.set("k2", b"v2".to_vec(), None).unwrap();

            let reader: &dyn SyncCacheReader = &backend;
            assert!(reader.exists("k1").unwrap());
            assert!(reader.exists("k2").unwrap());

            // delete 单个 key
            writer.delete("k1").unwrap();
            assert!(!reader.exists("k1").unwrap());
            assert!(reader.exists("k2").unwrap());

            // clear 清空全部
            writer.clear().unwrap();
            assert!(!reader.exists("k2").unwrap());
            assert_eq!(reader.len().unwrap(), 0);
            assert!(reader.is_empty().unwrap());

            // connector: health_check / shutdown / backend_kind
            let connector: &dyn SyncCacheConnector = &backend;
            connector.health_check().unwrap();
            assert_eq!(connector.backend_kind(), BackendKind::Moka);
            connector.shutdown();
        }
    }
}
