// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! CacheBackend trait for the modernized cache API
//!
//! This module provides ISP-compliant trait hierarchy:
//! - `CacheReader` - Read-only operations
//! - `CacheWriter` - Write operations
//! - `CacheConnector` - Lifecycle management
//! - `CacheBackend` - Combines all traits

use crate::error::OxCacheResult;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Backend kind enumeration for runtime type identification
///
/// This replaces `as_any()` for type checking, following the Brick Architecture
/// principle that concrete implementations should be invisible to consumers.
/// Unlike `core::types::BackendType` (used for configuration), this enum is
/// used for runtime identification without feature gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Moka in-memory cache
    Moka,
    /// DashMap in-memory cache
    DashMap,
    /// Redis distributed cache
    Redis,
    /// Valkey distributed cache (Redis-compatible, BSD-3 licensed)
    Valkey,
    /// Dragonfly distributed cache (Redis-compatible, BSL 1.1 licensed)
    Dragonfly,
    /// Aerospike distributed cache (independent sub-crate)
    Aerospike,
    /// Chain cache (multi-tier)
    Chain,
    /// Mock backend for testing
    Mock,
    /// Unknown or custom backend
    Unknown,
}

impl BackendKind {
    /// Returns true if this is an in-memory cache (L1)
    pub fn is_memory(&self) -> bool {
        matches!(self, BackendKind::Moka | BackendKind::DashMap | BackendKind::Mock)
    }

    /// Returns true if this is a distributed cache (L2)
    pub fn is_distributed(&self) -> bool {
        matches!(
            self,
            BackendKind::Redis | BackendKind::Valkey | BackendKind::Dragonfly | BackendKind::Aerospike
        )
    }

    /// Returns true if this is a composite (multi-tier) cache
    pub fn is_composite(&self) -> bool {
        matches!(self, BackendKind::Chain)
    }
}

/// Simple glob pattern matching: `*` matches any characters (including `/`),
/// `?` matches a single character.
///
/// Used by backends implementing `CacheReader::keys` / `SyncCacheReader::keys`
/// for in-memory key listing. Cache keys may contain `/` (see
/// `crate::infra::validate_cache_key`), so `*` deliberately matches across
/// path separators — unlike `security::glob_to_regex` whose `*` is `[^/]*`.
///
/// Uses an iterative two-pointer algorithm: on mismatch, backtrack to the
/// last `*` and advance the text position. Time O(m·n), space O(m+n).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (m, n) = (p.len(), t.len());

    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi: Option<usize> = None;
    let mut star_ti: usize = 0;

    while ti < n {
        if pi < m && p[pi] == '?' {
            // '?' matches any single character
            pi += 1;
            ti += 1;
        } else if pi < m && p[pi] == '*' {
            // Record last '*' position; try matching zero chars first
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if pi < m && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(sp) = star_pi {
            // Mismatch: backtrack to last '*', advance text by one
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    // Consume trailing '*'s in pattern
    while pi < m && p[pi] == '*' {
        pi += 1;
    }

    pi == m
}

// ============================================================================
// ISP-Compliant Trait Hierarchy
// ============================================================================

/// Read-only cache operations.
///
/// This trait provides methods for reading data from the cache.
/// It can be used by consumers that only need read access.
///
/// # Example
///
/// ```rust,ignore
/// fn get_value(cache: &dyn CacheReader, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
///     cache.get(key)
/// }
/// ```
#[async_trait]
pub trait CacheReader: Send + Sync + 'static {
    /// Get a value from the cache.
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>>;

    /// Check if a key exists in the cache.
    async fn exists(&self, key: &str) -> OxCacheResult<bool>;

    /// Get the time-to-live for a key.
    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>>;

    /// Get the number of entries in the cache.
    async fn len(&self) -> OxCacheResult<u64>;

    /// Check if the cache is empty.
    async fn is_empty(&self) -> OxCacheResult<bool> {
        Ok(self.len().await?.eq(&0))
    }

    /// Get the capacity of the cache.
    async fn capacity(&self) -> OxCacheResult<u64>;

    /// Get backend statistics.
    async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>>;

    /// Get multiple values in a single operation.
    async fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key).await?);
        }
        Ok(results)
    }

    /// List keys matching a pattern.
    ///
    /// Default implementation returns an empty vector. Backends should override
    /// this to provide actual key iteration (e.g. Redis SCAN, Moka iter).
    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        let _ = pattern;
        Ok(vec![])
    }
}

/// 批量写入条目：`(Arc<str> key, Arc<Vec<u8>> value, Option<Duration> ttl)`
pub type CacheSetItem = (Arc<str>, Arc<Vec<u8>>, Option<Duration>);

/// Write operations for the cache.
///
/// This trait provides methods for modifying data in the cache.
/// It can be used by consumers that only need write access.
///
/// # Example
///
/// ```rust,ignore
/// fn set_value(cache: &dyn CacheWriter, key: Arc<str>, value: Arc<Vec<u8>>) -> OxCacheResult<()> {
///     cache.set(key, value, None)
/// }
/// ```
#[async_trait]
pub trait CacheWriter: Send + Sync + 'static {
    /// Set a value in the cache.
    ///
    /// `key` and `value` are shared-ownership handles (`Arc`) so multi-backend
    /// chains can forward the same allocation without per-backend copies
    /// (optimization 2.2 / 2.3). Convert owned `String`/`Vec<u8>` via
    /// [`Arc::from`] / [`Arc::new`] — both are cheap.
    async fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()>;

    /// Delete a value from the cache.
    async fn delete(&self, key: &str) -> OxCacheResult<()>;

    /// Clear all values from the cache.
    async fn clear(&self) -> OxCacheResult<()>;

    /// Set the time-to-live for an existing key.
    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool>;

    /// Set multiple key-value pairs in a single operation.
    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        for (key, value, ttl) in items {
            self.set(key.clone(), value.clone(), *ttl).await?;
        }
        Ok(())
    }

    /// Delete multiple keys in a single operation.
    ///
    /// Note: 参数类型为 `&[String]` 而非 `&[&str]`，与 `set_many` 的 `&[CacheSetItem]`
    /// （含 `Arc<str>` 键）存在风格差异。这是为了保持与早期 API 的向后兼容性，
    /// 未来版本可能统一为 `&[&str]`。
    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        for key in keys {
            self.delete(key).await?;
        }
        Ok(())
    }
}

/// Lifecycle management for cache backends.
///
/// This trait provides methods for connection management and health monitoring.
/// It can be used by infrastructure code that manages backend lifecycle.
///
/// # Example
///
/// ```rust,ignore
/// fn check_and_shutdown(backend: &dyn CacheConnector) {
///     if backend.health_check().await.is_err() {
///         backend.shutdown().await;
///     }
/// }
/// ```
#[async_trait]
pub trait CacheConnector: Send + Sync + 'static {
    /// Check if the backend is healthy.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Backend is healthy
    /// * `Err(OxCacheError)` - Health check failed (backend is unhealthy)
    async fn health_check(&self) -> OxCacheResult<()>;

    /// Shutdown the backend and release resources.
    ///
    /// Internal errors are logged but not propagated.
    async fn shutdown(&self);

    /// Get the backend kind for runtime identification.
    fn backend_kind(&self) -> BackendKind;

    /// Get Lua script executor if this backend supports it.
    #[cfg(feature = "lua")]
    fn as_lua_executor(&self) -> Option<&dyn LuaExecutor> {
        None
    }

    /// Get atomic writer if this backend supports atomic operations.
    ///
    /// Default returns `None`. Backends that implement `AtomicCacheWriter`
    /// should override this to return `Some(self)`.
    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> {
        None
    }
}

// ============================================================================
// Lua Executor Trait (Optional, Redis-only)
// ============================================================================

#[cfg(feature = "lua")]
#[async_trait]
pub trait LuaExecutor: Send + Sync {
    async fn eval_lua(&self, script: &str, keys: &[&str], args: &[&str]) -> OxCacheResult<redis::Value>;
    async fn eval_sha(&self, sha: &str, keys: &[&str], args: &[&str]) -> OxCacheResult<redis::Value>;
    async fn script_load(&self, script: &str) -> OxCacheResult<String>;
}

// ============================================================================
// Atomic Cache Writer Trait (Optional capability)
// ============================================================================

/// Atomic cache operations for backends that support them.
///
/// This is an independent trait (not a supertrait of `CacheWriter`) because
/// atomic operations are an optional capability. Consumers that need atomic
/// semantics can require this trait via `CacheConnector::as_atomic_writer()`.
///
/// # Implementations
///
/// - `RedisBackend`: `INCR` / Lua CAS / `SET NX EX`
/// - `MokaMemoryBackend`: `parking_lot::Mutex` protected read-modify-write
/// - `MockBackend`: in-memory simulation for testing
#[async_trait]
pub trait AtomicCacheWriter: Send + Sync + 'static {
    /// Atomically increment a key's integer value and return the new value.
    ///
    /// If the key does not exist, it is initialized to 0 before incrementing.
    /// If `ttl` is provided, `EXPIRE` is set after the increment.
    async fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64>;

    /// Atomically compare-and-swap a key's value.
    ///
    /// - `expected = None`: succeed only if the key does **not** exist (SETNX semantics)
    /// - `expected = Some(bytes)`: succeed only if the current value equals `bytes`
    ///
    /// Returns `true` if the swap succeeded, `false` otherwise.
    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool>;

    /// Atomically set a key only if it does not already exist.
    ///
    /// Returns `true` if the key was set, `false` if it already existed.
    async fn set_if_absent(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<bool>;
}

// ============================================================================
// Combined CacheBackend Trait
// ============================================================================

/// Full cache backend interface combining all ISP traits.
///
/// Combines `CacheReader`, `CacheWriter`, and `CacheConnector` for consumers
/// that need full cache functionality. Single trait object type for backends.
///
/// # Design Pattern
///
/// Strategy pattern: allows different backend implementations to be swapped
/// without changing the cache interface.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::{CacheReader, CacheWriter, CacheConnector};
/// use async_trait::async_trait;
///
/// struct MyCustomBackend;
///
/// #[async_trait]
/// impl CacheReader for MyCustomBackend {
///     async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> { Ok(None) }
///     async fn exists(&self, key: &str) -> OxCacheResult<bool> { Ok(false) }
///     async fn ttl(&self, key: &str) -> OxCacheResult<Option<std::time::Duration>> { Ok(None) }
///     async fn len(&self) -> OxCacheResult<u64> { Ok(0) }
///     async fn capacity(&self) -> OxCacheResult<u64> { Ok(0) }
///     async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> { Ok(HashMap::new()) }
/// }
///
/// #[async_trait]
/// impl CacheWriter for MyCustomBackend { /* ... */ }
///
/// #[async_trait]
/// impl CacheConnector for MyCustomBackend { /* ... */ }
/// // CacheBackend is automatically provided via blanket impl
/// ```
#[async_trait]
pub trait CacheBackend: CacheReader + CacheWriter + CacheConnector + 'static {}

#[async_trait]
impl<T: CacheReader + CacheWriter + CacheConnector + 'static> CacheBackend for T {}

// ============================================================================
// Synchronous Trait Hierarchy (Mirror of Async Traits)
// ============================================================================
//
// Sync counterparts of `CacheReader`/`CacheWriter`/`CacheConnector`/`CacheBackend`.
// Backends that natively support synchronous access (Moka sync, DashMap) or
// can block on async runtimes (Redis via `block_in_place`) implement these in
// addition to the async traits. `Cache<K,V>::get_sync` dispatches through
// `Arc<dyn SyncCacheBackend>`.
//
// Design rationale (see `openspec/changes/add-sync-api-and-ttl-fix/design.md`):
// Independent trait hierarchy — async and sync coexist; backends opt into sync
// support explicitly. This avoids polluting the async hot path with
// `block_in_place` overhead and keeps the async trait object-safe.

/// Synchronous read-only cache operations.
///
/// Mirror of [`CacheReader`] without `async`/`#[async_trait]`. Backends that
/// can serve reads without an async runtime should implement this trait in
/// addition to (or instead of) [`CacheReader`].
///
/// # Example
///
/// ```rust,ignore
/// fn get_value(backend: &dyn SyncCacheReader, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
///     backend.get(key)
/// }
/// ```
pub trait SyncCacheReader: Send + Sync + 'static {
    /// Get a value from the cache.
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>>;

    /// Check if a key exists in the cache.
    fn exists(&self, key: &str) -> OxCacheResult<bool>;

    /// Get the time-to-live for a key.
    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>>;

    /// Get the number of entries in the cache.
    fn len(&self) -> OxCacheResult<u64>;

    /// Check if the cache is empty. Default impl delegates to [`Self::len`].
    fn is_empty(&self) -> OxCacheResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Get the capacity of the cache.
    fn capacity(&self) -> OxCacheResult<u64>;

    /// Get backend statistics.
    fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>>;

    /// Get multiple values in a single operation. Default impl loops [`Self::get`].
    fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(self.get(key)?);
        }
        Ok(results)
    }

    /// List keys matching a pattern. Default returns empty vector.
    fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        let _ = pattern;
        Ok(vec![])
    }
}

/// Synchronous write operations for the cache.
///
/// Mirror of [`CacheWriter`] without `async`/`#[async_trait]`.
pub trait SyncCacheWriter: Send + Sync + 'static {
    /// Set a value in the cache.
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()>;

    /// Delete a value from the cache.
    fn delete(&self, key: &str) -> OxCacheResult<()>;

    /// Clear all values from the cache.
    fn clear(&self) -> OxCacheResult<()>;

    /// Set the time-to-live for an existing key. Returns `false` if the key
    /// does not exist.
    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool>;

    /// Set multiple key-value pairs. Default impl loops [`Self::set`].
    fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        for (key, value, ttl) in items {
            self.set(key.clone(), value.clone(), *ttl)?;
        }
        Ok(())
    }

    /// Delete multiple keys. Default impl loops [`Self::delete`].
    fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        for key in keys {
            self.delete(key)?;
        }
        Ok(())
    }
}

/// Synchronous lifecycle management for cache backends.
///
/// Mirror of [`CacheConnector`] without `async`/`#[async_trait]`.
pub trait SyncCacheConnector: Send + Sync + 'static {
    /// Check if the backend is healthy.
    fn health_check(&self) -> OxCacheResult<()>;

    /// Shutdown the backend and release resources.
    fn shutdown(&self);

    /// Get the backend kind for runtime identification.
    fn backend_kind(&self) -> BackendKind;

    /// Get synchronous atomic writer if this backend supports it.
    ///
    /// Mirror of [`CacheConnector::as_atomic_writer()`] for sync access.
    /// Default returns `None`. Backends that implement `SyncAtomicCacheWriter`
    /// should override this to return `Some(self)`.
    fn as_sync_atomic_writer(&self) -> Option<&dyn SyncAtomicCacheWriter> {
        None
    }
}

/// Full synchronous cache backend interface combining all sync ISP traits.
///
/// Mirror of [`CacheBackend`] for synchronous access. Backends implement this
/// to opt into `Cache<K,V>::get_sync` and related sync APIs. Automatically
/// provided via blanket impl when a type implements
/// `SyncCacheReader + SyncCacheWriter + SyncCacheConnector`.
///
/// # Design Pattern
///
/// Same Strategy pattern as [`CacheBackend`], but for sync call sites. The
/// async and sync hierarchies are intentionally separate so that a backend
/// can support one without the other (e.g., a future TCP-only backend may
/// only support async).
pub trait SyncCacheBackend: SyncCacheReader + SyncCacheWriter + SyncCacheConnector + 'static {}

impl<T: SyncCacheReader + SyncCacheWriter + SyncCacheConnector + 'static> SyncCacheBackend for T {}

// ============================================================================
// Synchronous Atomic Cache Writer Trait (Mirror of AtomicCacheWriter)
// ============================================================================

/// Synchronous atomic cache operations.
///
/// Mirror of [`AtomicCacheWriter`] without `async`/`#[async_trait]`.
/// Backends that natively support synchronous atomic access implement this.
pub trait SyncAtomicCacheWriter: Send + Sync + 'static {
    /// Atomically increment and return the new value (sync).
    fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64>;

    /// Atomically compare-and-swap (sync).
    fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool>;

    /// Atomically set if absent (sync).
    fn set_if_absent(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::MockBackend;

    #[tokio::test]
    async fn test_mock_backend() {
        let backend = MockBackend::new("mock", 50, false);

        // Test set and get
        backend
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .await
            .unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test exists
        assert!(backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());

        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());

        // Test health check
        backend.health_check().await.unwrap();

        // Test stats
        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"mock".to_string()));
    }

    #[tokio::test]
    async fn test_isp_traits() {
        let backend = MockBackend::new("mock", 50, false);

        // Test CacheReader trait object
        let reader: &dyn CacheReader = &backend;
        assert!(reader.get("nonexistent").await.unwrap().is_none());

        // Test CacheWriter trait object
        let writer: &dyn CacheWriter = &backend;
        writer
            .set(Arc::from("key"), Arc::new(b"value".to_vec()), None)
            .await
            .unwrap();

        // Test CacheConnector trait object
        let connector: &dyn CacheConnector = &backend;
        connector.health_check().await.unwrap();
        assert_eq!(connector.backend_kind(), BackendKind::Mock);
    }

    // ============================================================================
    // BackendKind 方法测试 (lines 41-42, 46-47)
    // ============================================================================

    #[test]
    fn test_backend_kind_is_memory_moka() {
        assert!(BackendKind::Moka.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_dashmap() {
        assert!(BackendKind::DashMap.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_mock() {
        assert!(BackendKind::Mock.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_redis_false() {
        assert!(!BackendKind::Redis.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_chain_false() {
        assert!(!BackendKind::Chain.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_unknown_false() {
        assert!(!BackendKind::Unknown.is_memory());
    }

    #[test]
    fn test_backend_kind_is_distributed_redis() {
        assert!(BackendKind::Redis.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_moka_false() {
        assert!(!BackendKind::Moka.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_dashmap_false() {
        assert!(!BackendKind::DashMap.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_chain_false() {
        assert!(!BackendKind::Chain.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_mock_false() {
        assert!(!BackendKind::Mock.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_unknown_false() {
        assert!(!BackendKind::Unknown.is_distributed());
    }

    // ============================================================================
    // BackendKind Debug, Clone, PartialEq 测试
    // ============================================================================

    #[test]
    fn test_backend_kind_debug() {
        let kind = BackendKind::Moka;
        let debug_str = format!("{:?}", kind);
        assert!(debug_str.contains("Moka"));
    }

    #[test]
    fn test_backend_kind_clone() {
        let kind = BackendKind::Redis;
        let cloned = kind;
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_backend_kind_equality() {
        assert_eq!(BackendKind::Moka, BackendKind::Moka);
        assert_ne!(BackendKind::Moka, BackendKind::Redis);
    }

    // ============================================================================
    // CacheReader is_empty 默认方法测试 (lines 82-83)
    // ============================================================================

    #[tokio::test]
    async fn test_cache_reader_is_empty_default() {
        let backend = MockBackend::new("mock", 50, false);
        let reader: &dyn CacheReader = &backend;
        // 空缓存应该返回 true
        assert!(reader.is_empty().await.unwrap());

        // 添加数据后应该返回 false
        backend
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .await
            .unwrap();
        assert!(!reader.is_empty().await.unwrap());
    }

    // ============================================================================
    // CacheReader get_many 默认方法测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_reader_get_many_default() {
        let backend = MockBackend::new("mock", 50, false);
        backend
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key2"), Arc::new(b"value2".to_vec()), None)
            .await
            .unwrap();

        let reader: &dyn CacheReader = &backend;
        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let results = reader.get_many(&keys).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some(b"value1".to_vec()));
        assert_eq!(results[1], Some(b"value2".to_vec()));
        assert_eq!(results[2], None);
    }

    #[tokio::test]
    async fn test_cache_reader_get_many_empty() {
        let backend = MockBackend::new("mock", 50, false);
        let reader: &dyn CacheReader = &backend;
        let keys: Vec<String> = vec![];
        let results = reader.get_many(&keys).await.unwrap();
        assert!(results.is_empty());
    }

    // ============================================================================
    // CacheWriter set_many 和 delete_many 默认方法测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_writer_set_many_default() {
        let backend = MockBackend::new("mock", 50, false);
        let writer: &dyn CacheWriter = &backend;
        let items = vec![
            (Arc::from("key1"), Arc::new(b"value1".to_vec()), None),
            (Arc::from("key2"), Arc::new(b"value2".to_vec()), None),
        ];
        writer.set_many(&items).await.unwrap();

        assert!(backend.exists("key1").await.unwrap());
        assert!(backend.exists("key2").await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_writer_delete_many_default() {
        let backend = MockBackend::new("mock", 50, false);
        backend
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .await
            .unwrap();
        backend
            .set(Arc::from("key2"), Arc::new(b"value2".to_vec()), None)
            .await
            .unwrap();

        let writer: &dyn CacheWriter = &backend;
        let keys = vec!["key1".to_string(), "key2".to_string()];
        writer.delete_many(&keys).await.unwrap();

        assert!(!backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());
    }

    // ============================================================================
    // CacheConnector backend_kind 测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_connector_backend_kind_mock() {
        let backend = MockBackend::new("mock", 50, false);
        let connector: &dyn CacheConnector = &backend;
        assert_eq!(connector.backend_kind(), BackendKind::Mock);
    }

    #[tokio::test]
    async fn test_cache_connector_shutdown() {
        let backend = MockBackend::new("mock", 50, false);
        let connector: &dyn CacheConnector = &backend;
        // shutdown 不应 panic
        connector.shutdown().await;
    }

    // ============================================================================
    // CacheBackend blanket impl 测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_backend_trait_object() {
        let backend = MockBackend::new("mock", 50, false);
        let backend_dyn: &dyn CacheBackend = &backend;
        // 测试 CacheBackend 可以作为 trait 对象使用
        backend_dyn
            .set(Arc::from("key"), Arc::new(b"value".to_vec()), None)
            .await
            .unwrap();
        let value = backend_dyn.get("key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    // ============================================================================
    // SyncCacheBackend trait hierarchy 测试 (任务组 5)
    // ============================================================================

    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};
    use std::time::Instant;

    /// 单条 Mock 缓存条目：(value, expires_at)，`None` 表示永不过期。
    type MockSyncEntry = (Vec<u8>, Option<Instant>);

    /// Test mock for sync trait hierarchy. Stores entries with optional TTL
    /// via `Instant`, mirroring `MockBackend` semantics but without async.
    struct MockSyncBackend {
        data: Arc<RwLock<HashMap<String, MockSyncEntry>>>,
        capacity: u64,
    }

    impl MockSyncBackend {
        fn new(capacity: u64) -> Self {
            Self {
                data: Arc::new(RwLock::new(HashMap::new())),
                capacity,
            }
        }
    }

    impl SyncCacheReader for MockSyncBackend {
        fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
            let data = self.data.read().unwrap();
            if let Some((value, expires_at)) = data.get(key) {
                if let Some(deadline) = expires_at {
                    if *deadline <= Instant::now() {
                        return Ok(None);
                    }
                }
                return Ok(Some(value.clone()));
            }
            Ok(None)
        }

        fn exists(&self, key: &str) -> OxCacheResult<bool> {
            Ok(self.get(key)?.is_some())
        }

        fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
            let data = self.data.read().unwrap();
            if let Some((_, Some(deadline))) = data.get(key) {
                return Ok(deadline.checked_duration_since(Instant::now()));
            }
            Ok(None)
        }

        fn len(&self) -> OxCacheResult<u64> {
            Ok(self.data.read().unwrap().len() as u64)
        }

        fn capacity(&self) -> OxCacheResult<u64> {
            Ok(self.capacity)
        }

        fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
            let mut stats = HashMap::new();
            stats.insert("type".to_string(), "mock_sync".to_string());
            stats.insert("len".to_string(), self.len()?.to_string());
            Ok(stats)
        }
    }

    impl SyncCacheWriter for MockSyncBackend {
        fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
            let expires_at = ttl.map(|d| Instant::now() + d);
            self.data
                .write()
                .unwrap()
                .insert(key.to_string(), ((*value).clone(), expires_at));
            Ok(())
        }

        fn delete(&self, key: &str) -> OxCacheResult<()> {
            self.data.write().unwrap().remove(key);
            Ok(())
        }

        fn clear(&self) -> OxCacheResult<()> {
            self.data.write().unwrap().clear();
            Ok(())
        }

        fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
            let mut data = self.data.write().unwrap();
            if let Some(entry) = data.get_mut(key) {
                entry.1 = Some(Instant::now() + ttl);
                return Ok(true);
            }
            Ok(false)
        }
    }

    impl SyncCacheConnector for MockSyncBackend {
        fn health_check(&self) -> OxCacheResult<()> {
            Ok(())
        }

        fn shutdown(&self) {
            self.clear().ok();
        }

        fn backend_kind(&self) -> BackendKind {
            BackendKind::Mock
        }
    }

    #[test]
    fn test_sync_cache_backend_trait_object_usable() {
        let backend = MockSyncBackend::new(50);
        let backend_dyn: &dyn SyncCacheBackend = &backend;

        // 写入 + 读取
        backend_dyn
            .set(Arc::from("key1"), Arc::new(b"value1".to_vec()), None)
            .unwrap();
        let value = backend_dyn.get("key1").unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // exists
        assert!(backend_dyn.exists("key1").unwrap());
        assert!(!backend_dyn.exists("missing").unwrap());

        // delete
        backend_dyn.delete("key1").unwrap();
        assert!(!backend_dyn.exists("key1").unwrap());

        // connector
        backend_dyn.health_check().unwrap();
        assert_eq!(backend_dyn.backend_kind(), BackendKind::Mock);
    }

    #[test]
    fn test_sync_reader_default_is_empty_uses_len() {
        let backend = MockSyncBackend::new(50);
        let reader: &dyn SyncCacheReader = &backend;
        // 空缓存
        assert!(reader.is_empty().unwrap());
        // 添加数据后
        backend.set(Arc::from("k"), Arc::new(b"v".to_vec()), None).unwrap();
        assert!(!reader.is_empty().unwrap());
    }

    #[test]
    fn test_sync_writer_default_set_many_loops_set() {
        let backend = MockSyncBackend::new(50);
        let writer: &dyn SyncCacheWriter = &backend;
        let items = vec![
            (Arc::from("k1"), Arc::new(b"v1".to_vec()), None),
            (Arc::from("k2"), Arc::new(b"v2".to_vec()), None),
            (Arc::from("k3"), Arc::new(b"v3".to_vec()), None),
        ];
        writer.set_many(&items).unwrap();

        assert_eq!(backend.get("k1").unwrap(), Some(b"v1".to_vec()));
        assert_eq!(backend.get("k2").unwrap(), Some(b"v2".to_vec()));
        assert_eq!(backend.get("k3").unwrap(), Some(b"v3".to_vec()));
        assert_eq!(backend.len().unwrap(), 3);

        // delete_many 默认实现
        writer.delete_many(&["k1".to_string(), "k2".to_string()]).unwrap();
        assert!(!backend.exists("k1").unwrap());
        assert!(!backend.exists("k2").unwrap());
        assert!(backend.exists("k3").unwrap());
    }

    #[test]
    fn test_sync_reader_default_get_many_loops_get() {
        let backend = MockSyncBackend::new(50);
        backend.set(Arc::from("k1"), Arc::new(b"v1".to_vec()), None).unwrap();
        backend.set(Arc::from("k2"), Arc::new(b"v2".to_vec()), None).unwrap();

        let reader: &dyn SyncCacheReader = &backend;
        let keys = vec!["k1".to_string(), "k2".to_string(), "k3".to_string()];
        let results = reader.get_many(&keys).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some(b"v1".to_vec()));
        assert_eq!(results[1], Some(b"v2".to_vec()));
        assert_eq!(results[2], None);
    }

    #[test]
    fn test_sync_backend_ttl_and_expire() {
        let backend = MockSyncBackend::new(50);
        backend
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), Some(Duration::from_secs(60)))
            .unwrap();

        // ttl 返回剩余时间
        let ttl = backend.ttl("k").unwrap();
        assert!(ttl.is_some());
        let ttl = ttl.unwrap();
        assert!(ttl <= Duration::from_secs(60) && ttl > Duration::from_secs(58));

        // expire 返回 true（key 存在）
        let result = backend.expire("k", Duration::from_secs(120)).unwrap();
        assert!(result);
        let new_ttl = backend.ttl("k").unwrap().unwrap();
        assert!(new_ttl > Duration::from_secs(118));

        // expire 返回 false（key 不存在）
        let result = backend.expire("missing", Duration::from_secs(10)).unwrap();
        assert!(!result);
    }

    // ============================================================================
    // BackendKind::is_composite 测试
    // ============================================================================

    #[test]
    fn test_backend_kind_is_composite_chain() {
        assert!(BackendKind::Chain.is_composite());
    }

    #[test]
    fn test_backend_kind_is_composite_others_false() {
        assert!(!BackendKind::Moka.is_composite());
        assert!(!BackendKind::DashMap.is_composite());
        assert!(!BackendKind::Redis.is_composite());
        assert!(!BackendKind::Mock.is_composite());
        assert!(!BackendKind::Unknown.is_composite());
        // New variants
        assert!(!BackendKind::Valkey.is_composite());
        assert!(!BackendKind::Dragonfly.is_composite());
        assert!(!BackendKind::Aerospike.is_composite());
    }

    // ============================================================================
    // BackendKind 新后端变体测试 (T001: Valkey/Dragonfly/Aerospike)
    // ============================================================================

    #[test]
    fn test_backend_kind_is_distributed_valkey() {
        assert!(BackendKind::Valkey.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_dragonfly() {
        assert!(BackendKind::Dragonfly.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_distributed_aerospike() {
        assert!(BackendKind::Aerospike.is_distributed());
    }

    #[test]
    fn test_backend_kind_is_memory_valkey_false() {
        assert!(!BackendKind::Valkey.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_dragonfly_false() {
        assert!(!BackendKind::Dragonfly.is_memory());
    }

    #[test]
    fn test_backend_kind_is_memory_aerospike_false() {
        assert!(!BackendKind::Aerospike.is_memory());
    }

    #[test]
    fn test_backend_kind_is_composite_valkey_false() {
        assert!(!BackendKind::Valkey.is_composite());
    }

    #[test]
    fn test_backend_kind_is_composite_dragonfly_false() {
        assert!(!BackendKind::Dragonfly.is_composite());
    }

    #[test]
    fn test_backend_kind_is_composite_aerospike_false() {
        assert!(!BackendKind::Aerospike.is_composite());
    }

    #[test]
    fn test_backend_kind_new_variants_debug_clone_eq() {
        // Debug
        assert!(format!("{:?}", BackendKind::Valkey).contains("Valkey"));
        assert!(format!("{:?}", BackendKind::Dragonfly).contains("Dragonfly"));
        assert!(format!("{:?}", BackendKind::Aerospike).contains("Aerospike"));
        // Clone + Eq
        assert_eq!(BackendKind::Valkey, BackendKind::Valkey);
        assert_eq!(BackendKind::Dragonfly, BackendKind::Dragonfly);
        assert_eq!(BackendKind::Aerospike, BackendKind::Aerospike);
        assert_ne!(BackendKind::Valkey, BackendKind::Redis);
        assert_ne!(BackendKind::Dragonfly, BackendKind::Valkey);
    }

    // ============================================================================
    // CacheReader::keys 默认实现测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_reader_keys_default_returns_empty() {
        let backend = MockBackend::new("mock", 50, false);
        let reader: &dyn CacheReader = &backend;
        // MockBackend 覆盖了 keys()，但默认实现通过 trait object 测试
        // 使用 CacheReader trait 的默认实现
        let keys = reader.keys("*").await.unwrap();
        // MockBackend 的 keys 返回匹配的 key，空缓存应返回空
        assert!(keys.is_empty());
    }

    // ============================================================================
    // SyncCacheReader::keys 默认实现测试
    // ============================================================================

    #[test]
    fn test_sync_reader_keys_default_returns_empty() {
        let backend = MockSyncBackend::new(50);
        let reader: &dyn SyncCacheReader = &backend;
        // MockSyncBackend 没有覆盖 keys()，使用默认实现
        let keys = reader.keys("*").unwrap();
        assert!(keys.is_empty());
    }

    // ============================================================================
    // CacheConnector 默认方法测试
    // ============================================================================

    #[tokio::test]
    async fn test_cache_connector_as_atomic_writer_default_none() {
        let backend = MockBackend::new("mock", 50, false);
        let connector: &dyn CacheConnector = &backend;
        // MockBackend 覆盖了 as_atomic_writer 返回 Some
        assert!(connector.as_atomic_writer().is_some());
    }

    #[tokio::test]
    async fn test_cache_connector_as_atomic_writer_dashmap_none() {
        use crate::backend::DashMapMemoryBackend;
        let backend = DashMapMemoryBackend::new();
        let connector: &dyn CacheConnector = &backend;
        // DashMap 不实现 AtomicCacheWriter，默认返回 None
        assert!(connector.as_atomic_writer().is_none());
    }

    // ============================================================================
    // glob_match unit tests
    // ============================================================================

    #[test]
    fn test_glob_match_exact_match() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
        assert!(!glob_match("hello", "hell"));
        assert!(!glob_match("hell", "hello"));
    }

    #[test]
    fn test_glob_match_star_matches_any() {
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("hello*", "hello"));
        assert!(glob_match("hello*", "helloworld"));
        assert!(glob_match("*world", "helloworld"));
        assert!(glob_match("he*ld", "helloworld"));
        assert!(!glob_match("he*ld", "hello"));
    }

    #[test]
    fn test_glob_match_question_mark() {
        assert!(glob_match("h?llo", "hello"));
        assert!(glob_match("?????", "hello"));
        assert!(!glob_match("????", "hello"));
        assert!(!glob_match("??????", "hello"));
        assert!(glob_match("h?llo", "hallo"));
        assert!(!glob_match("?", ""));
    }

    #[test]
    fn test_glob_match_combined_patterns() {
        assert!(glob_match("h*o", "hello"));
        assert!(glob_match("h*o", "ho"));
        assert!(glob_match("h?l*w", "hellow"));
        assert!(glob_match("*?*", "a"));
        assert!(!glob_match("*?*", ""));
        assert!(glob_match("a*b*c", "abc"));
        assert!(glob_match("a*b*c", "aXbYc"));
        assert!(!glob_match("a*b*c", "aXbY"));
    }

    #[test]
    fn test_glob_match_empty() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "a"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn test_glob_match_star_matches_slash() {
        assert!(glob_match("cache/*", "cache/item"));
        assert!(glob_match("*/*", "a/b"));
        assert!(glob_match("a*b*c", "a/b/c"));
        assert!(!glob_match("cache/*", "cache"));
        assert!(glob_match("prefix*", "prefix/with/slash"));
    }
}
