// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! oxcache - 高性能多层缓存库
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::Cache;
//! use oxcache::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! struct User { id: u64, name: String }
//!
//! #[tokio::main]
//! async fn main() -> OxCacheResult<(), Box<dyn std::error::Error>> {
//!     let cache: Cache<String, User> = Cache::builder().build().await?;
//!     cache.set(&"user:1".to_string(), &User { id: 1, name: "Alice".into() }).await?;
//!     let user = cache.get(&"user:1".to_string()).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Tiered Cache
//!
//! ```rust,ignore
//! use oxcache::cache::{ChainCache, ChainLink};
//! use oxcache::backend::MokaMemoryBackend;
//!
//! let l1 = MokaMemoryBackend::builder().capacity(10000).build();
//! let l2 = oxcache::backend::RedisBackend::new("redis://localhost:6379").await?;
//!
//! let chain = ChainCache::builder()
//!     .link(ChainLink::from_backend(l1))
//!     .link(ChainLink::from_backend(l2))
//!     .enable_backfill()
//!     .build();
//! ```
//!
//! # Sync API (0.3.0)
//!
//! Enable `sync_mode(true)` on the builder to get synchronous methods
//! (`get_sync` / `set_sync` / `set_with_ttl_sync` / `delete_sync` /
//! `exists_sync` / `get_or_sync` / `clear_sync`) alongside the async API.
//! The default Moka path (and native sync faces injected via
//! `sync_backend_arc`) is runtime-independent: callable outside any runtime
//! (a temporary current-thread runtime drives it), reusing the current
//! runtime via `block_in_place` on a `multi_thread` runtime. Only inside a
//! current-thread runtime's async context it returns `Err(NotSupported)`
//! (tokio forbids nested blocking drivers). The bridged path
//! (`backend_arc` combined with `sync_mode(true)`) still requires a
//! `multi_thread` runtime.
//!
//! ```rust,ignore
//! # #[tokio::main(flavor = "multi_thread")]
//! # async fn main() -> OxCacheResult<(), Box<dyn std::error::Error>> {
//! let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await?;
//! cache.set_sync(&"k".to_string(), &"v".to_string())?;
//! let v = cache.get_sync(&"k".to_string())?;
//! # Ok(()) }
//! ```
//!
//! # Bloom Filter (0.3.0)
//!
//! Enable the `bloom` feature for negative-query
//! filtering. `BloomFilterBackend` wraps any `CacheBackend` and skips
//! the inner backend on BF miss.
//!
//! ```rust,ignore
//! use oxcache::backend::MokaMemoryBackend;
//! use oxcache::features::BloomFilterBackend;
//! let backend = BloomFilterBackend::new(MokaMemoryBackend::new());
//! ```
//!
//! # Universal per-entry TTL (0.3.0)
//!
//! All backends (Moka / DashMap / Redis / Valkey / Dragonfly / Aerospike /
//! Disk / Mock / Chain / Bloom) honor per-entry
//! `set(key, value, Some(ttl))`. Moka uses the `moka::Expiry`
//! trait for real per-entry TTL (overriding the global TTL set on the
//! builder).
//!
//! # Features
//!
//! ## Tiered Feature Sets
//!
//! - `minimal`: L1 memory cache only (memory + metrics + serialization + chrono)
//! - `core`: L1 + L2 Redis (minimal + redis)
//! - `full`: Layered preset = core + macros + compression + batch + lua +
//!   testing + dragonfly + aerospike + lock + offload + disk + stale.
//!   Deliberately excludes the opt-in features: audit, encrypt, integrity,
//!   invalidation, versioning, telemetry, config-confers, serde-bincode,
//!   postcard, bloom, kit.
//!
//! ## Core Component Features
//!
//! - `memory`: L1 memory cache (Moka + DashMap)
//! - `redis`: L2 distributed cache (Redis + regex)
//! - `macros`: Proc macros for `#[cached]`
//! - `serialization`: JSON serialization (serde + serde_json)
//! - `compression`: Flate2 compression
//! - `metrics`: Built-in performance metrics (latency histograms, operation counters, JSON export); OTLP export handled at application layer
//! - `batch`: Buffered batch writer with capacity/time dual-threshold flush
//! - `lua`: Lua script execution (requires redis)
//! - `test-util` (deprecated alias: `testing`): Testing support (exposes internal functions)
//! - `bloom`: Negative-query filtering (not in `full`)
//! - `trait-kit` (deprecated alias: `kit`): trait-kit AsyncKit integration
//!   (OxcacheModule) (not in `full`)
//! - `lock`: Distributed lock via Redis (TTL, reentrant, watchdog auto-renew)
//! - `invalidation`: Cross-instance L1 invalidation bus via Redis Pub/Sub
//!   (write-path broadcast + background listener with self-exemption) plus
//!   optional keyspace-notification channel (`KeyspaceNotificationListener`)
//! - `encrypt`: Value-level encryption decorator (XChaCha20-Poly1305,
//!   confers-aligned envelope `[ver][nonce][ct]`, AAD binds the cache key)
//! - `integrity`: Value integrity decorator (HMAC-SHA256 tag
//!   `[ver][tag][payload]`; verification failure counts as a miss)
//! - `config-confers`: Config-driven build via confers (`OxcacheConfig`
//!   load + `ConfigBus` watch hot-reload of capacity/TTL/circuit params)
//! - `degradation`: L2 degradation controller (Active/Degraded/HalfOpen
//!   state machine + `DegradableBackend` guard decorator + `snapshot()`
//!   observability; `DegradationTracing` under `telemetry`)
//! - `audit`: Structured audit event stream (`AuditEventPublisher` port,
//!   NoOp/InMemory/tracing publishers)
//! - `versioning`: Version-based compare-and-swap (`MemoryVersionedCache`
//!   + Redis WATCH-based `RedisVersionedCache`)
//! - `redlock` (deprecated alias: `red-lock`): RedLock-style multi-node
//!   majority lock (`RedLock`, `LockNode` protocol layer, fencing tokens)
//! - `serde-bincode` / `postcard`: Binary serialization formats
//!   (`SerializationFormat`, `CacheBuilder::serialization_format`)
//!
//! # Distributed Lock (`lock` feature)
//!
//! Cross-instance mutual exclusion backed by Redis. Supports TTL, automatic
//! watchdog renewal, and reentrant acquire/release.
//!
//! ```rust,ignore
//! use oxcache::features::dist_lock::DistLockBuilder;
//! use std::time::Duration;
//!
//! let mut lock = DistLockBuilder::new(backend, "task:webhook-delivery".into())
//!     .ttl(Duration::from_secs(30))
//!     .watchdog_enabled(true)
//!     .build();
//! if lock.acquire().await? {
//!     // critical section
//!     lock.release().await?;
//! }
//! ```
//!
//! # Cache Penetration Guard
//!
//! Two-layer protection against cache stampede and penetration:
//!
//! - **Single-flight** (`get_or` / `get_or_sync`): 64-shard dedup ensures only
//!   one fallback executes per key under concurrent cache misses.
//! - **Null sentinel** (`get_or_option`): caches a sentinel for `None` results
//!   when `null_cache_ttl` is configured, preventing repeated DB lookups for
//!   non-existent keys.
//! - **TTL jitter** (`ttl_jitter`): randomizes actual TTL within
//!   `base_ttl * (1.0 ± factor)` to prevent mass expiration stampede.
//!
//! ```rust,ignore
//! let cache: Cache<String, User> = Cache::builder()
//!     .null_cache_ttl(Duration::from_secs(30))
//!     .ttl_jitter(0.1)
//!     .build().await?;
//!
//! // Returns None and caches sentinel if DB also returns None
//! let user = cache.get_or_option(&"user:999".to_string(), || async {
//!     db.find_user(999).await  // returns OxCacheResult<Option<User>>
//! }).await?;
//! ```

#![doc(html_root_url = "https://docs.rs/oxcache/0.5.0-rc.7")]
#![deny(unsafe_code)]

// ============================================================================
// Core Modules (Always Available)
// ============================================================================
mod core;
pub mod error;
#[cfg(test)]
mod test_support;

// 测试构建内使用 `#[cached]` 宏（覆盖宏参数解析分支）需要 `::oxcache`
// 绝对路径可解析——把自身以 crate 名重新导出（仅 cfg(test) 编译）。
#[cfg(test)]
extern crate self as oxcache;

// #[cached] 宏 single-flight 支撑设施（分片注册表 + panic 守卫 + watch flight
// 信号）。宏生成代码按路径引用，必须始终编译（无 feature 门）。
pub mod macro_support;

// 同步字节权重缓存（tokio-free 直连 moka::sync）：L1 场景按字节预算控容量，
// single-flight + 单条准入阈值 + 命中统计（Mirrors limiteron sync module pattern）
#[cfg(feature = "byte-weight")]
pub mod sync;

// Internal module for #[cached] macro support
// Must be `pub` (not `pub(crate)`) so the #[cached] macro can access
// __internal_get_cache from external crates. #[doc(hidden)] keeps it out of public docs.
// 依赖 crate::Cache，须与 cache 模块门控一致
#[cfg(any(feature = "memory", feature = "redis"))]
#[doc(hidden)]
pub mod internal;

// ============================================================================
// Primary Modules (Feature-Gated)
// ============================================================================

// Cache module (modern Cache<K,V> API)
// Gated behind backend-enabling features because cache depends on backend + infra modules.
// memory-only is supported: serde is included in the memory feature for trait bounds,
// and serde_json usage is internally gated behind serialization/full.
#[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
pub mod cache;

// Backend module (L1/L2 cache implementation)
#[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
pub mod backend;

// Features module (optional capabilities)
pub mod features;

// Cross-instance invalidation bus (`invalidation` feature)
#[cfg(feature = "invalidation")]
pub use features::invalidation;

// Redis Pub/Sub 广播设施（`pubsub` feature）：通用频道 publish/subscribe，
// 专用订阅连接 + 断线重连 + handler panic 隔离（下沉自 garrison SSO 通道）
#[cfg(feature = "pubsub")]
pub mod pubsub;

// redis crate re-export：`eval_lua`/pubsub 的伴生类型（如 `redis::Value`）
// 供 consumer 侧模式匹配使用，consumer 不再直接依赖 redis crate
#[cfg(feature = "redis")]
pub use redis;

// Value-level encryption (`encrypt` feature)
#[cfg(feature = "encrypt")]
pub use features::encryption;

// Batch writer (optional, gated by `batch` feature)
#[cfg(feature = "batch")]
pub mod batch;

// Infrastructure module (metrics, serialization, telemetry, etc.)
// serialization 单开也须可用：SerializationFormat 属于该特性的公开类型，
// config 中枢在其下引用（与 config 模块全组合可用承诺一致）
#[cfg(any(
    feature = "serialization",
    feature = "metrics",
    feature = "memory",
    feature = "redis",
    feature = "minimal",
    feature = "core",
    feature = "redis-tier",
    feature = "full",
    feature = "batch"
))]
pub mod infra;

// Mock Module (For testing only)
#[cfg(test)]
mod testing;

// Registry module for #[cached] macro support
// 需要 backend (CacheBackend trait) 和 dashmap，仅在 memory 及其超集下可用
#[cfg(feature = "memory")]
pub mod registry;

// Traits module: CacheKey
pub mod traits;

// Config module
pub mod config;

// Utils module: key generation utilities
mod utils;

// Integrations module: optional adapters for external frameworks (trait-kit, etc.)
// Each integration is feature-gated and pulls no deps unless explicitly enabled.
#[cfg(feature = "trait-kit")]
pub mod integrations;

// i18n module: ICU4X-backed locale-aware formatting for cache keys, statistics,
// expiry display, collation, and localized error messages. Always enabled.
pub mod i18n;

// Security module: Redis security validation
pub(crate) mod security;

// ============================================================================
// Public API Re-exports
// ============================================================================

// Re-export serde traits so consumers don't need to add serde to their own
// Cargo.toml. Cache<K, V> requires V: Serialize + Deserialize, so these are
// part of the public API surface. Re-exporting keeps them as internal deps.
#[cfg(any(feature = "memory", feature = "serialization", feature = "full"))]
pub use serde::{Deserialize, Serialize};

// Re-export macros when the feature is enabled
#[cfg(feature = "macros")]
pub use oxcache_macros::cached;

#[cfg(feature = "macros")]
pub mod macros {
    pub use oxcache_macros::*;
}

#[cfg(feature = "redis")]
pub use error::{OxCacheConfigError, OxCacheConfigResult};
pub use error::{OxCacheError, OxCacheResult};

// Re-export internal functions needed by #[cached] macro at crate root
// The macro generates code calling ::oxcache::__internal_get_cache()
// internal 模块依赖 cache::Cache，须与 cache 模块门控一致
#[cfg(any(feature = "memory", feature = "redis"))]
#[doc(hidden)]
pub use crate::internal::__internal_get_cache;

// ---- telemetry helpers for macro-generated code ----
// These are called from #[cached] macro expansions. When `telemetry` is
// off they compile to empty functions (zero overhead).

/// Emit a trace event when the macro silently passes through (cache not registered).
#[doc(hidden)]
#[cfg(feature = "telemetry")]
#[inline]
pub fn __telemetry_macro_passthrough(service: &str, reason: &str) {
    tracing::debug!(
        target = "oxcache::macro",
        service,
        reason,
        "cache passthrough"
    );
}

#[doc(hidden)]
#[cfg(not(feature = "telemetry"))]
#[inline]
pub fn __telemetry_macro_passthrough(_service: &str, _reason: &str) {}

// ============================================================================
// New API (Recommended)
// ============================================================================

// New API exports
// cache 模块仅在 memory/redis/minimal/core/full feature 下编译，re-export 须同步门控
#[cfg(any(feature = "memory", feature = "redis"))]
pub use cache::BytesCache;
#[cfg(any(feature = "memory", feature = "redis"))]
pub use cache::Cache;
#[cfg(any(feature = "memory", feature = "redis"))]
pub use cache::CacheBuilder;

// Re-exports from infra module
#[cfg(feature = "metrics")]
pub use infra::{
    CacheStats, export_json_format, export_prometheus_format, export_prometheus_standard,
    get_enhanced_stats,
};
#[cfg(feature = "metrics")]
pub use infra::{MetricsRecorder, NoOpMetricsRecorder, UnifiedMetricsRecorder};

// Re-exports from security module (new brick architecture)
#[cfg(any(feature = "redis", feature = "full"))]
pub use crate::security::{
    Redacted, clamp_scan_count, log_cache_key, redact_cache_key, redact_connection_string,
    redact_field, redact_value, sanitize_message, validate_lua_script, validate_redis_key,
    validate_scan_pattern,
};

// Distributed lock re-exports
#[cfg(feature = "lock")]
pub use features::dist_lock::{
    DefaultLockProvider, DistLockBuilder, DistributedLock, LockProvider,
};

// Public API re-exports (after features re-exports)
// cache 模块 re-export 须与 cache 模块门控一致
#[cfg(feature = "warmup")]
pub use cache::warmup::{Warmup, WarmupBuilder, WarmupEntry, WarmupLoader, WarmupReport};
#[cfg(feature = "memory")]
pub use cache::{ChainBuilder, L1Builder, L2Builder};
#[cfg(any(feature = "memory", feature = "redis"))]
pub use cache::{ChainCache, ChainCacheBuilder, ChainLink};
#[cfg(any(feature = "memory", feature = "redis"))]
pub use cache::{DynUnifiedCache, TypedCacheExt, UnifiedCache};
#[cfg(any(feature = "memory", feature = "redis"))]
pub use cache::{NamespaceName, TypedNamespace};
pub use traits::CacheKey;

// Type-safe enum exports
pub use core::{BackendType, CacheLayer, RedisModeType, SerializationType};

// Key generator export
pub use crate::utils::KeyGenerator;

// Canonical JSON normalization exports (`serialization` feature)
#[cfg(feature = "serialization")]
pub use crate::utils::canonical::{canonical_json, canonical_json_string};

// Events module re-export
pub use core::{CacheEvent, CacheEventType, EventPublisher};

// Backend exports
// backend 模块仅在 memory/redis/minimal/core/full feature 下编译，re-export 须同步门控；
// memory 实现符号进一步按依赖开启特性门控（dashmap: offload；moka: byte-weight；
// serde derive: serialization），与 backend 模块内部整合
#[cfg(any(feature = "memory", all(feature = "redis", feature = "serialization")))]
pub use backend::MemoryBackendType;
#[cfg(any(feature = "memory", feature = "redis"))]
pub use backend::{BackendScore, Scores};
#[cfg(any(feature = "memory", all(feature = "redis", feature = "offload")))]
pub use backend::{DashMapMemoryBackend, dashmap_memory};
#[cfg(any(feature = "memory", all(feature = "redis", feature = "byte-weight")))]
pub use backend::{MokaMemoryBackend, default_memory_backend, moka_memory};

#[cfg(feature = "redis")]
pub use backend::{RedisBackend, RedisBackendBuilder, RedisMode};

// ============================================================================
// Factory Functions (Brick Architecture Standard)
// ============================================================================

/// oxcache 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(all(test, any(feature = "memory", feature = "full")))]
mod tests {
    use crate::VERSION;

    #[test]
    fn test_version_constant() {
        // 测试 VERSION 常量不为空
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_version_format() {
        // 测试 VERSION 格式（应该包含数字）
        assert!(VERSION.chars().any(|c: char| c.is_ascii_digit()));
    }

    /// telemetry smoke test — verify the passthrough helper can be
    /// called without panicking regardless of whether `telemetry` is on.
    #[test]
    fn telemetry_macro_passthrough_does_not_panic() {
        crate::__telemetry_macro_passthrough("test_service", "unit_test");
    }
}
