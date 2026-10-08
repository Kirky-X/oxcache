// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! CacheBackend trait for the modernized cache API
//!
//! This module provides ISP-compliant trait hierarchy:
//! - `CacheReader` - Read-only operations
//! - `CacheWriter` - Write operations
//! - `CacheConnector` - Lifecycle management
//! - `CacheBackend` - Combines all traits

use crate::error::{OxCacheError, OxCacheResult};
#[cfg(feature = "telemetry")]
use crate::i18n::messages::MSG_LOG_BRIDGE_SHUTDOWN_SKIPPED;
use crate::i18n::messages::{
    MSG_DETAIL_ASYNC_ATOMIC_CAS, MSG_DETAIL_ASYNC_ATOMIC_INCREMENT,
    MSG_DETAIL_ASYNC_ATOMIC_SET_IF_ABSENT, MSG_DETAIL_SYNC_ATOMIC_CAS,
    MSG_DETAIL_SYNC_ATOMIC_INCREMENT, MSG_DETAIL_SYNC_ATOMIC_SET_IF_ABSENT,
    MSG_DETAIL_SYNC_REQUIRES_MULTI_THREAD, MSG_DETAIL_SYNC_REQUIRES_RUNTIME,
    MSG_PANIC_BRIDGE_TEMP_RUNTIME, t,
};
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
    /// Disk-persistent embedded backend (redb)
    Disk,
    /// Unknown or custom backend
    Unknown,
}

impl BackendKind {
    /// Returns true if this is an in-memory cache (L1)
    pub fn is_memory(&self) -> bool {
        matches!(
            self,
            BackendKind::Moka | BackendKind::DashMap | BackendKind::Mock
        )
    }

    /// Returns true if this is a distributed cache (L2)
    pub fn is_distributed(&self) -> bool {
        matches!(
            self,
            BackendKind::Redis
                | BackendKind::Valkey
                | BackendKind::Dragonfly
                | BackendKind::Aerospike
        )
    }

    /// Returns true if this is a composite (multi-tier) cache
    pub fn is_composite(&self) -> bool {
        matches!(self, BackendKind::Chain)
    }

    /// Stable backend identifier used as the metrics label dimension
    /// (`oxcache_backend_<name>_operations_total`).
    pub fn name(&self) -> &'static str {
        match self {
            BackendKind::Moka => "moka",
            BackendKind::DashMap => "dashmap",
            BackendKind::Redis => "redis",
            BackendKind::Valkey => "valkey",
            BackendKind::Dragonfly => "dragonfly",
            BackendKind::Aerospike => "aerospike",
            BackendKind::Chain => "chain",
            BackendKind::Mock => "mock",
            BackendKind::Disk => "disk",
            BackendKind::Unknown => "unknown",
        }
    }
}

/// Simple glob pattern matching: `*` matches any characters (including `/`),
/// `?` matches a single character.
///
/// Used by backends implementing `CacheReader::keys` / `SyncCacheReader::keys`
/// for in-memory key listing. Cache keys may contain `/` (see
/// `crate::infra::validate_cache_key`), so `*` deliberately matches across
/// path separators.
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
    ///
    /// # Backend semantics
    ///
    /// On backends with an idle-based eviction policy (e.g. moka's
    /// `time_to_idle`), querying `ttl` counts as an access and may postpone
    /// idle eviction of the entry.
    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>>;

    /// Get the number of entries in the cache.
    ///
    /// # Backend semantics
    ///
    /// The Redis backend implements this via `DBSIZE`, which counts **all
    /// keys in the connected DB index** — not only keys written through this
    /// cache instance. Point the backend at a dedicated DB index when a
    /// precise per-cache count is required.
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
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()>;

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
    async fn eval_lua(
        &self,
        script: &str,
        keys: &[&str],
        args: &[&str],
    ) -> OxCacheResult<redis::Value>;
    async fn eval_sha(
        &self,
        sha: &str,
        keys: &[&str],
        args: &[&str],
    ) -> OxCacheResult<redis::Value>;
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
    async fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool>;
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
// Design rationale:
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
pub trait SyncCacheBackend:
    SyncCacheReader + SyncCacheWriter + SyncCacheConnector + 'static
{
}

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
    fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool>;
}

// ============================================================================
// Sync → Async Facade Adapter
// ============================================================================

/// Bridge a native synchronous backend into the async [`CacheBackend`] surface.
///
/// The async and sync trait hierarchies are intentionally separate (see
/// [`SyncCacheBackend`]), and `Arc<dyn SyncCacheBackend>` cannot be presented
/// as `Arc<dyn CacheBackend>` — the two share no supertrait relationship and
/// `trait_upcasting` does not apply. This adapter closes the gap for backends
/// that are sync-first: the adapter itself has no await point, so `.await` on
/// the facade resolves with the blocking semantics of the underlying sync
/// call. **The runtime requirements are those of the wrapped backend** —
/// e.g. `MokaMemoryBackend`'s sync surface drives via `sync_block_on`
/// (multi_thread runtime: `block_in_place`; outside a runtime: a temporary
/// current_thread runtime; inside a current_thread runtime async context:
/// explicit `NotSupported` error, not a panic), while purely synchronous
/// backends (e.g. DashMap) are safe in any environment.
///
/// The reverse direction (async → sync) exists both as a backend-side concern
/// (e.g. Redis's sync traits bridge via `block_in_place` + `handle.block_on`)
/// and as the generic [`AsyncToSyncBridge`] used by `CacheBuilder` when
/// `sync_mode(true)` combines with `backend_arc`.
///
/// Atomic capabilities are probed honestly through
/// [`SyncCacheConnector::as_sync_atomic_writer`]: the facade advertises
/// [`AtomicCacheWriter`] only when the inner backend implements it; call-path
/// probing returns `NotSupported` as a defensive fallback.
pub struct SyncBackendAdapter {
    inner: Arc<dyn SyncCacheBackend>,
}

impl SyncBackendAdapter {
    /// Wrap a native sync backend for presentation as a full backend.
    pub fn new(inner: Arc<dyn SyncCacheBackend>) -> Self {
        Self { inner }
    }
}

// 同步面三子 trait 委托；齐备后由 blanket impl 自动获得 SyncCacheBackend
impl SyncCacheReader for SyncBackendAdapter {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        self.inner.get(key)
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.inner.exists(key)
    }

    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.inner.ttl(key)
    }

    fn len(&self) -> OxCacheResult<u64> {
        self.inner.len()
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity()
    }

    fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        self.inner.stats()
    }

    fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        self.inner.get_many(keys)
    }

    fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern)
    }
}

impl SyncCacheWriter for SyncBackendAdapter {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        self.inner.set(key, value, ttl)
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key)
    }

    fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear()
    }

    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl)
    }

    fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        self.inner.set_many(items)
    }

    fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys)
    }
}

impl SyncCacheConnector for SyncBackendAdapter {
    fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check()
    }

    fn shutdown(&self) {
        self.inner.shutdown()
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }
}

// 同步面三子 trait 齐备，SyncCacheBackend 由 blanket 自动提供

#[async_trait]
impl CacheReader for SyncBackendAdapter {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        self.inner.get(key)
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        self.inner.exists(key)
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        self.inner.ttl(key)
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.inner.len()
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        self.inner.capacity()
    }

    async fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        self.inner.stats()
    }

    async fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        self.inner.get_many(keys)
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        self.inner.keys(pattern)
    }
}

#[async_trait]
impl CacheWriter for SyncBackendAdapter {
    async fn set(
        &self,
        key: Arc<str>,
        value: Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        self.inner.set(key, value, ttl)
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        self.inner.delete(key)
    }

    async fn clear(&self) -> OxCacheResult<()> {
        self.inner.clear()
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.inner.expire(key, ttl)
    }

    async fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        self.inner.set_many(items)
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        self.inner.delete_many(keys)
    }
}

#[async_trait]
impl CacheConnector for SyncBackendAdapter {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.inner.health_check()
    }

    async fn shutdown(&self) {
        self.inner.shutdown()
    }

    fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }

    // dyn 同步面无法呈现为 async 原子面：以自身为桥。探测语义须诚实——
    // 按 inner 的 as_sync_atomic_writer 判定，无能力即 None（与 direct
    // backend_arc 注入同一后端的探测结果一致），调用路径仅作兜底显性报错
    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> {
        match self.inner.as_sync_atomic_writer() {
            Some(_) => Some(self),
            None => None,
        }
    }
}

#[async_trait]
impl AtomicCacheWriter for SyncBackendAdapter {
    async fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        match self.inner.as_sync_atomic_writer() {
            Some(w) => w.incr(key, delta, ttl),
            None => Err(OxCacheError::NotSupported(t(
                MSG_DETAIL_SYNC_ATOMIC_INCREMENT,
                &[],
            ))),
        }
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        match self.inner.as_sync_atomic_writer() {
            Some(w) => w.compare_and_swap(key, expected, new, ttl),
            None => Err(OxCacheError::NotSupported(t(
                MSG_DETAIL_SYNC_ATOMIC_CAS,
                &[],
            ))),
        }
    }

    async fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        match self.inner.as_sync_atomic_writer() {
            Some(w) => w.set_if_absent(key, value, ttl),
            None => Err(OxCacheError::NotSupported(t(
                MSG_DETAIL_SYNC_ATOMIC_SET_IF_ABSENT,
                &[],
            ))),
        }
    }
}

// async 面三子 trait 齐备，CacheBackend 由 blanket 自动提供：
// SyncBackendAdapter 同时是完整的 async 与 sync 后端

// ============================================================================
// Async → Sync Bridge Adapter
// ============================================================================

/// async→sync 桥接共用的运行时句柄守卫
///
/// 桥接方法以 `block_in_place` + `handle.block_on` 阻塞等待 async 操作：
/// `block_in_place` 仅在多线程 runtime 可用，current_thread runtime 上会
/// panic，故在调用前显性拒绝；runtime 之外同样无可等待对象，显性报错。
pub(crate) fn multi_thread_bridge_handle() -> OxCacheResult<tokio::runtime::Handle> {
    let handle = tokio::runtime::Handle::try_current().map_err(|e| {
        OxCacheError::NotSupported(t(
            MSG_DETAIL_SYNC_REQUIRES_RUNTIME,
            &[("err", e.to_string())],
        ))
    })?;
    if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread {
        return Err(OxCacheError::NotSupported(t(
            MSG_DETAIL_SYNC_REQUIRES_MULTI_THREAD,
            &[],
        )));
    }
    Ok(handle)
}

/// Bridge an async [`CacheBackend`] handle into the sync surface.
///
/// Mirror of [`SyncBackendAdapter`] for the opposite direction: the two trait
/// hierarchies share no supertrait relationship, so `Arc<dyn CacheBackend>`
/// cannot be presented as `Arc<dyn SyncCacheBackend>` — the concrete sync
/// impl is erased at the `dyn` boundary. This adapter closes that gap
/// generically (used by `CacheBuilder` when `sync_mode(true)` combines with
/// `backend_arc`): every sync method bridges by
/// `block_in_place` + `handle.block_on` over the async method, so futures are
/// never dropped mid-flight (unlike noop-waker polling, which could abandon
/// partially-completed I/O).
///
/// # Runtime requirements
///
/// Calls require a **multi-thread Tokio runtime** (enforced by the
/// crate-internal runtime guard): outside any runtime or on a
/// current_thread runtime every sync method returns `Err(NotSupported)`.
/// Backends whose sync face can run without a runtime (e.g. Moka/DashMap
/// native sync) should be injected via `sync_backend_arc` instead.
///
/// **`worker_threads(1)` runtimes are hazardous for I/O-backed backends**
/// (e.g. Redis): `block_in_place` converts the sole worker into a blocking
/// thread, leaving no active worker to poll the reactor — a bridged call
/// waiting on network readiness may then **hang indefinitely**. Use
/// `worker_threads(2)` or more for bridged I/O, or drive such backends
/// through the async API only. (Same hazard applies to backend-provided sync
/// faces built on `block_in_place`, e.g. `RedisBackend`'s.)
///
/// `shutdown` is the one method that degrades instead of erroring: outside
/// any runtime it executes on a temporary current-thread runtime; inside a
/// current-thread runtime (nested blocking drivers are forbidden) it is
/// skipped and counted via `oxcache_bridge_shutdown_rejected_total`
/// (metrics feature) with a tracing warn (telemetry feature).
///
/// # Deadlock Warning
///
/// Mirrors the hazard documented on backend-provided sync faces (e.g.
/// `RedisBackend`): bridging blocks the calling thread. Invoking sync methods
/// from a task running on the same multi-thread runtime is legal
/// (`block_in_place` hands the worker off) but can starve the runtime under
/// sustained load. Prefer the async API inside async contexts.
///
/// Atomic capabilities are probed honestly: the bridge advertises
/// [`SyncAtomicCacheWriter`] only when the inner backend exposes
/// [`AtomicCacheWriter`] via [`CacheConnector::as_atomic_writer`].
pub struct AsyncToSyncBridge {
    inner: Arc<dyn CacheBackend>,
}

impl AsyncToSyncBridge {
    /// Wrap an async backend handle for presentation as a sync backend.
    pub fn new(inner: Arc<dyn CacheBackend>) -> Self {
        Self { inner }
    }
}

impl SyncCacheReader for AsyncToSyncBridge {
    fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::get(&*self.inner, key)))
    }

    fn exists(&self, key: &str) -> OxCacheResult<bool> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::exists(&*self.inner, key)))
    }

    fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::ttl(&*self.inner, key)))
    }

    fn len(&self) -> OxCacheResult<u64> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::len(&*self.inner)))
    }

    fn capacity(&self) -> OxCacheResult<u64> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::capacity(&*self.inner)))
    }

    fn stats(&self) -> OxCacheResult<std::collections::HashMap<String, String>> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::stats(&*self.inner)))
    }

    fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::get_many(&*self.inner, keys)))
    }

    fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheReader::keys(&*self.inner, pattern)))
    }
}

impl SyncCacheWriter for AsyncToSyncBridge {
    fn set(&self, key: Arc<str>, value: Arc<Vec<u8>>, ttl: Option<Duration>) -> OxCacheResult<()> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| {
            handle.block_on(CacheWriter::set(&*self.inner, key, value, ttl))
        })
    }

    fn delete(&self, key: &str) -> OxCacheResult<()> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::delete(&*self.inner, key)))
    }

    fn clear(&self) -> OxCacheResult<()> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::clear(&*self.inner)))
    }

    fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::expire(&*self.inner, key, ttl)))
    }

    fn set_many(&self, items: &[CacheSetItem]) -> OxCacheResult<()> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheWriter::set_many(&*self.inner, items)))
    }

    fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| {
            handle.block_on(CacheWriter::delete_many(&*self.inner, keys))
        })
    }
}

impl SyncCacheConnector for AsyncToSyncBridge {
    fn health_check(&self) -> OxCacheResult<()> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| handle.block_on(CacheConnector::health_check(&*self.inner)))
    }

    fn shutdown(&self) {
        match tokio::runtime::Handle::try_current() {
            // 多线程 runtime：block_in_place 安全
            Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| {
                    handle.block_on(CacheConnector::shutdown(&*self.inner))
                });
            }
            // current_thread runtime 异步上下文：tokio 禁止嵌套阻塞驱动，
            // 桥接无法执行——计数器 + telemetry warn 后跳过（拒绝必须可观测）；
            // 与 confers 重载拒绝计数器（8cad1c3）同款默认可见信号
            Ok(_) => {
                record_bridge_shutdown_rejected();
                warn_bridge_shutdown_rejected();
            }
            // runtime 之外（sync API 的自然调用场景）：临时 current_thread
            // runtime 驱动真实 shutdown，不做静默跳过（moka sync_block_on
            // 的 Err(_) 分支同款兜底）
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap_or_else(|e| panic!("{}: {e:?}", t(MSG_PANIC_BRIDGE_TEMP_RUNTIME, &[])));
                rt.block_on(CacheConnector::shutdown(&*self.inner));
            }
        }
    }

    fn backend_kind(&self) -> BackendKind {
        CacheConnector::backend_kind(&*self.inner)
    }

    // 探测语义须诚实：inner 的 async 原子面在场才广告同步原子能力，
    // 桥接自身不虚构能力（调用路径以 NotSupported 兜底）
    fn as_sync_atomic_writer(&self) -> Option<&dyn SyncAtomicCacheWriter> {
        if self.inner.as_atomic_writer().is_some() {
            Some(self)
        } else {
            None
        }
    }
}

// 同步面三子 trait 齐备，SyncCacheBackend 由 blanket 自动提供

impl SyncAtomicCacheWriter for AsyncToSyncBridge {
    fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| {
            handle.block_on(match self.inner.as_atomic_writer() {
                Some(w) => w.incr(key, delta, ttl),
                None => {
                    return Err(OxCacheError::NotSupported(t(
                        MSG_DETAIL_ASYNC_ATOMIC_INCREMENT,
                        &[],
                    )));
                }
            })
        })
    }

    fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| {
            handle.block_on(match self.inner.as_atomic_writer() {
                Some(w) => w.compare_and_swap(key, expected, new, ttl),
                None => {
                    return Err(OxCacheError::NotSupported(t(
                        MSG_DETAIL_ASYNC_ATOMIC_CAS,
                        &[],
                    )));
                }
            })
        })
    }

    fn set_if_absent(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        let handle = multi_thread_bridge_handle()?;
        tokio::task::block_in_place(|| {
            handle.block_on(match self.inner.as_atomic_writer() {
                Some(w) => w.set_if_absent(key, value, ttl),
                None => {
                    return Err(OxCacheError::NotSupported(t(
                        MSG_DETAIL_ASYNC_ATOMIC_SET_IF_ABSENT,
                        &[],
                    )));
                }
            })
        })
    }
}

/// current_thread runtime 下桥接 shutdown 无法执行的计数（metrics feature：
/// 默认预设可见；与 confers 重载拒绝计数器同款机制）
#[cfg(feature = "metrics")]
#[inline]
fn record_bridge_shutdown_rejected() {
    crate::infra::metrics::unified::GLOBAL_UNIFIED_METRICS
        .increment_counter("oxcache_bridge_shutdown_rejected_total", 1);
}

#[cfg(not(feature = "metrics"))]
#[inline]
fn record_bridge_shutdown_rejected() {}

/// 同上场景的 telemetry 增强信号（telemetry feature 下 tracing warn）
#[cfg(feature = "telemetry")]
#[inline]
fn warn_bridge_shutdown_rejected() {
    tracing::warn!(
        target = "oxcache::backend",
        "{}",
        t(MSG_LOG_BRIDGE_SHUTDOWN_SKIPPED, &[])
    );
}

#[cfg(not(feature = "telemetry"))]
#[inline]
fn warn_bridge_shutdown_rejected() {}

#[cfg(test)]
mod tests;
