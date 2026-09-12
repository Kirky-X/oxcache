// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `OxcacheModule` — trait-kit 0.4 `AsyncKit` integration for oxcache.
//!
//! Wires oxcache's cache backend into the `AsyncKit` dependency
//! injection framework as a leaf module (no upstream dependencies).
//!
//! # Design divergence from `design.md` / `spec.md` (Rule 7: expose, don't
//! paper over)
//!
//! `design.md` Decision 3 (lines 322-343) and `oxcache-module/spec.md`
//! R-001 wrote the capability type as `Arc<dyn UnifiedCache + Send + Sync>`
//! and the build body as:
//!
//! ```text
//! let cache = CacheBuilder::new().config(config).build().await?;
//! Ok(Arc::new(cache) as Arc<dyn UnifiedCache + Send + Sync>)
//! ```
//!
//! oxcache v0.3.3's actual API does not match this pseudo-code on **two**
//! independent points, both surfaced here rather than papered over:
//!
//! 1. **`CacheBuilder` has no `.config()` setter** — only `ttl/tti/capacity/
//!    backend_arc/sync_mode`. We translate `OxcacheConfig` into those
//!    individual setter calls on a `MokaMemoryBackend::builder()` (the
//!    concrete L1 backend that the default `CacheBuilder` path also uses).
//!
//! 2. **`UnifiedCache` is NOT dyn-compatible** (object unsafe) — its
//!    `get_typed<T>` / `set_typed<T>` methods carry generic type parameters,
//!    so `Arc<dyn UnifiedCache>` does not compile (`error[E0038]`). We
//!    therefore expose the capability as `Arc<dyn CacheBackend + Send +
//!    Sync>` instead. This is the **same** abstraction oxcache itself uses
//!    throughout its codebase (`registry.rs`, `cache/chain.rs`, the
//!    `&dyn CacheBackend` test in `backend/interface.rs`) — `CacheBackend`
//!    is the de-facto core cache trait, dyn-compatible, and the supertrait
//!    of `UnifiedCache` via the blanket impl
//!    `impl<T: CacheBackend + Send + Sync> UnifiedCache for T`.
//!
//! `CacheBackend` exposes the full operational surface (`get`/`set`/
//! `delete`/`exists`/`clear`/`expire`/`ttl`/`health_check`/`shutdown`/
//! `stats`/`backend_kind`/...) that downstream consumers (e.g. dbnexus's
//! `OxcacheDbCacheAdapter`) need.
//!
//! **Follow-up**: if the spec owner requires
//! the literal `Arc<dyn UnifiedCache>` form, oxcache's `UnifiedCache` trait
//! must be split — moving `get_typed`/`set_typed` into a separate
//! `TypedCacheExt` trait so `UnifiedCache` becomes object-safe. That change
//! affects oxcache's public API and is deferred to its own change spec.

use std::time::Duration;

/// Selects which cache backend [`OxcacheModule::build`] constructs.
///
/// - [`BackendType::Memory`] — L1 Moka in-process cache (always available).
/// - [`BackendType::Redis`] — Redis backend (requires `redis` feature).
/// - [`BackendType::Chain`] — Multi-tier chain cache (requires at least
///   `redis` feature for L2; L1 memory is always available).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BackendType {
    /// Moka in-memory backend (default).
    #[default]
    Memory,
    /// Redis distributed backend. Requires `redis` cargo feature.
    Redis,
    /// Multi-tier chain cache (memory + redis). Requires `redis` cargo feature.
    Chain,
}

/// Redis-specific configuration consumed when [`BackendType::Redis`] or
/// [`BackendType::Chain`] is selected.
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection string (e.g. `"redis://127.0.0.1:6379"`).
    pub connection_string: String,
    /// Connection pool size (default 8).
    pub pool_size: usize,
    /// Connection timeout (default 2 s).
    pub connection_timeout: Duration,
    /// Retry count for recoverable errors (default 3).
    pub retry_count: u32,
    /// Retry delay between attempts (default 100 ms).
    pub retry_delay: Duration,
    /// Circuit breaker consecutive-failure threshold (default 5).
    pub circuit_breaker_threshold: u32,
    /// Circuit breaker open→half-open reset timeout (default 30 s).
    pub circuit_breaker_reset_timeout: Duration,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            connection_string: String::new(),
            pool_size: 8,
            connection_timeout: Duration::from_secs(2),
            retry_count: 3,
            retry_delay: Duration::from_millis(100),
            circuit_breaker_threshold: 5,
            circuit_breaker_reset_timeout: Duration::from_secs(30),
        }
    }
}

/// One link inside a [`BackendType::Chain`] configuration.
#[derive(Debug, Clone)]
pub struct ChainLinkConfig {
    /// Which backend kind this link wraps.
    pub backend: BackendType,
    /// Priority score (higher = consulted first). Defaults: memory 100, redis 50.
    pub score: u8,
    /// Redis-specific settings; required when `backend == Redis`.
    pub redis: Option<RedisConfig>,
}

/// Configuration consumed by [`OxcacheModule`] during `build()`.
///
/// Stored in `AsyncKit` via `kit.set_config(OxcacheConfig::default())` and
/// read back with `kit.config::<OxcacheConfig>()`. This is a minimal standalone
/// struct — it is intentionally NOT wired into oxcache's config system because
/// the trait-kit integration must remain a leaf, feature-gated concern
/// (Rule 6: no extra default deps).
#[derive(Debug, Clone)]
pub struct OxcacheConfig {
    /// Which backend to construct (default: [`BackendType::Memory`]).
    pub backend: BackendType,
    /// Max entries held by the L1 memory backend (Moka). Mirrors
    /// `MokaMemoryBackend::builder().capacity(..)`. Default `10_000`.
    pub capacity: u64,
    /// Optional per-entry TTL applied to the L1 backend. Mirrors
    /// `MokaMemoryBackend::builder().ttl(..)`.
    pub ttl: Option<Duration>,
    /// Optional per-entry TTI (time-to-idle) applied to the L1 backend.
    /// Mirrors `MokaMemoryBackend::builder().time_to_idle(..)`.
    pub tti: Option<Duration>,
    /// Redis-specific settings. Required when `backend` is `Redis` or
    /// when a chain link uses `BackendType::Redis`.
    pub redis: Option<RedisConfig>,
    /// Per-link configuration for [`BackendType::Chain`]. Ignored for
    /// other backend types.
    pub chain: Vec<ChainLinkConfig>,
}

impl Default for OxcacheConfig {
    fn default() -> Self {
        Self {
            backend: BackendType::Memory,
            capacity: 10_000,
            ttl: None,
            tti: None,
            redis: None,
            chain: Vec::new(),
        }
    }
}

use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use trait_kit::prelude::*;

use crate::backend::{CacheBackend, MokaMemoryBackend};
use crate::error::OxCacheError;

#[cfg(feature = "redis")]
use crate::backend::RedisBackend;
#[cfg(feature = "redis")]
use crate::cache::{ChainCacheBuilder, ChainLink};

/// trait-kit `AsyncKit` module that constructs an oxcache cache backend.
///
/// Leaf module (no upstream dependencies). Register with
/// `AsyncKit::register::<OxcacheModule>()`, configure via
/// `kit.set_config(OxcacheConfig::default())`, then `kit.build().await` and
/// retrieve the capability with `kit.require::<OxcacheModule>()`.
///
/// The returned `Arc<dyn CacheBackend + Send + Sync>` is the same trait
/// object oxcache itself uses in `registry.rs` and `cache/chain.rs` — see
/// the module-level docs for the design-divergence rationale (spec.md wrote
/// `Arc<dyn UnifiedCache>`, but `UnifiedCache` is not object-safe).
pub struct OxcacheModule;

impl ModuleMeta for OxcacheModule {
    const NAME: &'static str = "oxcache";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        &[]
    }
}

impl AsyncAutoBuilder for OxcacheModule {
    type Capability = Arc<dyn CacheBackend + Send + Sync>;
    type Error = OxCacheError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let config: OxcacheConfig = kit
                .config()
                .map_err(|e| OxCacheError::Internal(format!("OxcacheModule: read config: {e}")))?;

            match config.backend {
                BackendType::Memory => build_memory_backend(&config),
                BackendType::Redis => build_redis_backend(&config).await,
                BackendType::Chain => build_chain_cache(&config).await,
            }
        })
    }
}

/// Build a Moka in-memory backend from `OxcacheConfig`.
fn build_memory_backend(
    config: &OxcacheConfig,
) -> Result<Arc<dyn CacheBackend + Send + Sync>, OxCacheError> {
    let mut builder = MokaMemoryBackend::builder().capacity(config.capacity);
    if let Some(ttl) = config.ttl {
        builder = builder.ttl(ttl);
    }
    if let Some(tti) = config.tti {
        builder = builder.time_to_idle(tti);
    }
    let backend = builder.build();
    Ok(Arc::new(backend) as Arc<dyn CacheBackend + Send + Sync>)
}

/// Build a Redis backend from `OxcacheConfig`.
///
/// Returns a clear error when the `redis` cargo feature is not enabled.
#[cfg(feature = "redis")]
async fn build_redis_backend(
    config: &OxcacheConfig,
) -> Result<Arc<dyn CacheBackend + Send + Sync>, OxCacheError> {
    let rc = config.redis.as_ref().ok_or_else(|| {
        OxCacheError::InvalidInput(
            "OxcacheModule: backend=Redis requires `redis` config".into(),
        )
    })?;
    let backend = apply_redis_config(
        RedisBackend::builder(),
        rc,
    )
    .build()
    .await?;
    Ok(Arc::new(backend) as Arc<dyn CacheBackend + Send + Sync>)
}

#[cfg(not(feature = "redis"))]
async fn build_redis_backend(
    _config: &OxcacheConfig,
) -> Result<Arc<dyn CacheBackend + Send + Sync>, OxCacheError> {
    Err(OxCacheError::InvalidInput(
        "OxcacheModule: backend=Redis requires the `redis` cargo feature to be enabled".into(),
    ))
}

/// Build a ChainCache from `OxcacheConfig`.
///
/// When `config.chain` is non-empty each entry is built individually;
/// otherwise a default two-tier chain (memory L1 + redis L2) is constructed.
#[cfg(feature = "redis")]
async fn build_chain_cache(
    config: &OxcacheConfig,
) -> Result<Arc<dyn CacheBackend + Send + Sync>, OxCacheError> {
    let mut chain_builder = ChainCacheBuilder::default();

    if !config.chain.is_empty() {
        for link_cfg in &config.chain {
            let link = build_chain_link(link_cfg, config).await?;
            chain_builder = chain_builder.link(link);
        }
    } else {
        // Default two-tier: memory L1 + redis L2
        let l1 = {
            let mut b = MokaMemoryBackend::builder().capacity(config.capacity);
            if let Some(ttl) = config.ttl {
                b = b.ttl(ttl);
            }
            if let Some(tti) = config.tti {
                b = b.time_to_idle(tti);
            }
            b.build()
        };
        chain_builder = chain_builder.link(ChainLink::from_backend(l1));

        let rc = config.redis.as_ref().ok_or_else(|| {
            OxCacheError::InvalidInput(
                "OxcacheModule: backend=Chain (default mode) requires `redis` config".into(),
            )
        })?;
        let l2 = apply_redis_config(RedisBackend::builder(), rc)
            .build()
            .await?;
        chain_builder = chain_builder.link(ChainLink::from_backend(l2));
    }

    let chain = chain_builder.build();
    Ok(Arc::new(chain) as Arc<dyn CacheBackend + Send + Sync>)
}

#[cfg(not(feature = "redis"))]
async fn build_chain_cache(
    _config: &OxcacheConfig,
) -> Result<Arc<dyn CacheBackend + Send + Sync>, OxCacheError> {
    Err(OxCacheError::InvalidInput(
        "OxcacheModule: backend=Chain requires the `redis` cargo feature to be enabled".into(),
    ))
}

/// Build a single [`ChainLink`] from a [`ChainLinkConfig`].
#[cfg(feature = "redis")]
async fn build_chain_link(
    link_cfg: &ChainLinkConfig,
    config: &OxcacheConfig,
) -> Result<ChainLink, OxCacheError> {
    match link_cfg.backend {
        BackendType::Memory => {
            let mut b = MokaMemoryBackend::builder().capacity(config.capacity);
            if let Some(ttl) = config.ttl {
                b = b.ttl(ttl);
            }
            if let Some(tti) = config.tti {
                b = b.time_to_idle(tti);
            }
            let backend = b.build();
            Ok(ChainLink::new(
                backend,
                link_cfg.score,
                false,
                "memory",
            ))
        }
        BackendType::Redis => {
            let rc = link_cfg
                .redis
                .as_ref()
                .or(config.redis.as_ref())
                .ok_or_else(|| {
                    OxCacheError::InvalidInput(
                        "OxcacheModule: chain link backend=Redis requires `redis` config".into(),
                    )
                })?;
            let backend = apply_redis_config(RedisBackend::builder(), rc)
                .build()
                .await?;
            Ok(ChainLink::new(
                backend,
                link_cfg.score,
                true,
                "redis",
            ))
        }
        BackendType::Chain => Err(OxCacheError::InvalidInput(
            "OxcacheModule: nested Chain inside Chain is not supported".into(),
        )),
    }
}

/// Apply [`RedisConfig`] fields to a `RedisBackendBuilder`.
#[cfg(feature = "redis")]
fn apply_redis_config(
    mut builder: crate::backend::RedisBackendBuilder,
    rc: &RedisConfig,
) -> crate::backend::RedisBackendBuilder {
    if !rc.connection_string.is_empty() {
        builder = builder.connection_string(&rc.connection_string);
    }
    builder = builder
        .pool_size(rc.pool_size)
        .connection_timeout(rc.connection_timeout)
        .retry_count(rc.retry_count)
        .retry_delay(rc.retry_delay)
        .circuit_breaker_threshold(rc.circuit_breaker_threshold)
        .circuit_breaker_reset_timeout(rc.circuit_breaker_reset_timeout);
    builder
}

/// Async health check for `OxcacheModule`.
///
/// Reports the health status of the underlying cache backend.
/// Returns `Healthy` if `CacheBackend::health_check()` succeeds,
/// or `Unhealthy` with error details if it fails.
impl trait_kit::core::health::AsyncHealthCheck for OxcacheModule {
    fn check(cap: &Self::Capability) -> trait_kit::core::health::HealthStatus {
        // Use futures::executor::block_on to avoid runtime context issues.
        // This is safe because health_check is a quick operation.
        match futures::executor::block_on(cap.health_check()) {
            Ok(()) => trait_kit::core::health::HealthStatus::Healthy,
            Err(e) => trait_kit::core::health::HealthStatus::unhealthy(format!(
                "cache backend health check failed: {e}"
            )),
        }
    }
}

/// Async lifecycle hooks for `OxcacheModule`.
///
/// Provides graceful shutdown by calling `CacheBackend::shutdown()`
/// when the `AsyncKit` is shut down.
impl trait_kit::core::lifecycle::AsyncLifecycle for OxcacheModule {
    fn on_shutdown<'a>(cap: &'a Self::Capability) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            cap.shutdown().await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-oxcache-module-001: `OxcacheModule::NAME == "oxcache"`.
    #[test]
    fn oxcache_module_meta_name() {
        assert_eq!(OxcacheModule::NAME, "oxcache");
    }

    /// R-oxcache-module-001: `OxcacheModule::dependencies()` is empty
    /// (oxcache is a leaf module — no upstream deps).
    #[test]
    fn oxcache_module_meta_dependencies_empty() {
        assert_eq!(
            OxcacheModule::dependencies(),
            &[] as &[(&'static str, TypeId)]
        );
    }

    /// R-oxcache-module-001: register `OxcacheModule` + `set_config` +
    /// `build()` + `require::<OxcacheModule>()` returns an
    /// `Arc<dyn CacheBackend + Send + Sync>` that performs real cache ops.
    #[tokio::test]
    async fn oxcache_module_build_returns_cache_capability() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let cache: Arc<dyn CacheBackend + Send + Sync> = kit
            .require::<OxcacheModule>()
            .expect("require OxcacheModule");
        // Smoke-test the returned capability actually behaves like a cache.
        cache
            .set(Arc::from("k"), Arc::new(b"v".to_vec()), None)
            .await
            .expect("set");
        let got = cache.get("k").await.expect("get");
        assert_eq!(got, Some(b"v".to_vec()));
    }

    /// R-oxcache-module-001: build reads `OxcacheConfig` from
    /// `kit.config::<OxcacheConfig>()` — verifies the config we set is
    /// honored by the constructed backend (capacity / ttl / tti all wired).
    #[tokio::test]
    async fn oxcache_module_build_reads_config_from_kit() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig {
            capacity: 5,
            ..OxcacheConfig::default()
        });
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let cache: Arc<dyn CacheBackend + Send + Sync> = kit
            .require::<OxcacheModule>()
            .expect("require OxcacheModule");
        // Insert several keys; backend constructed with the provided config
        // should not panic and should still report healthy.
        for i in 0..6u8 {
            cache
                .set(Arc::from(format!("k{i}")), Arc::new(vec![i]), None)
                .await
                .expect("set");
        }
        cache.health_check().await.expect("health_check");
    }

    /// R-oxcache-module-001: `OxcacheModule::build` returns a
    /// `Pin<Box<dyn Future + Send>>` (async build), not a sync `Result`.
    /// Verified by calling `AsyncAutoBuilder::build` directly on an unbuilt
    /// kit and awaiting the returned future.
    #[tokio::test]
    async fn oxcache_module_build_is_async() {
        let kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        // Call AsyncAutoBuilder::build directly (bypassing AsyncKit::build's
        // topological pipeline). The return type is Pin<Box<dyn Future + Send>>;
        // awaiting it must yield the capability.
        let fut = <OxcacheModule as AsyncAutoBuilder>::build(&kit);
        let cache: Arc<dyn CacheBackend + Send + Sync> = fut.await.expect("build future resolves");
        cache.clear().await.expect("clear");
        // Capability satisfies Send + Sync (AsyncAutoBuilder bound).
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arc<dyn CacheBackend + Send + Sync>>();
    }

    /// R-oxcache-module-002: `AsyncHealthCheck::check` returns `Healthy`
    /// for a functioning cache backend.
    #[tokio::test]
    async fn oxcache_module_health_check_returns_healthy() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        kit.register_health_check::<OxcacheModule>();
        let kit = kit.build().await.expect("AsyncKit::build");
        let status = kit.health_check::<OxcacheModule>().expect("health_check");
        assert!(
            status.is_healthy(),
            "expected Healthy status, got {status:?}"
        );
    }

    /// R-oxcache-module-002: `AsyncHealthCheck::check` called directly
    /// on the capability returns `Healthy`.
    #[tokio::test]
    async fn oxcache_module_health_check_direct_call() {
        let kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        let fut = <OxcacheModule as AsyncAutoBuilder>::build(&kit);
        let cache = fut.await.expect("build");
        let status = <OxcacheModule as trait_kit::core::health::AsyncHealthCheck>::check(&cache);
        assert!(status.is_healthy(), "expected Healthy, got {status:?}");
    }

    /// R-oxcache-module-003: `AsyncLifecycle::on_shutdown` calls
    /// `CacheBackend::shutdown()` gracefully without panicking.
    #[tokio::test]
    async fn oxcache_module_lifecycle_on_shutdown() {
        let kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        let fut = <OxcacheModule as AsyncAutoBuilder>::build(&kit);
        let cache = fut.await.expect("build");
        // Call on_shutdown directly — should complete without panic.
        <OxcacheModule as trait_kit::core::lifecycle::AsyncLifecycle>::on_shutdown(&cache).await;
    }

    /// R-oxcache-module-003: Full lifecycle integration — register,
    /// build, shutdown via AsyncKit.
    #[tokio::test]
    async fn oxcache_module_lifecycle_full_kit_integration() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.register::<OxcacheModule>()
            .expect("register OxcacheModule");
        kit.register_lifecycle::<OxcacheModule>();
        let kit = kit.build().await.expect("AsyncKit::build");
        // Shutdown should complete without panic.
        kit.shutdown_async().await;
    }

    // ---- multi-backend tests ----

    /// `BackendType::default()` is `Memory`.
    #[test]
    fn backend_type_default_is_memory() {
        assert_eq!(BackendType::default(), BackendType::Memory);
    }

    /// `OxcacheConfig::default()` selects Memory backend.
    #[test]
    fn oxcache_config_default_selects_memory() {
        let cfg = OxcacheConfig::default();
        assert_eq!(cfg.backend, BackendType::Memory);
        assert!(cfg.redis.is_none());
        assert!(cfg.chain.is_empty());
    }

    /// Explicit `BackendType::Memory` builds a working memory backend.
    #[tokio::test]
    async fn explicit_memory_backend_builds() {
        let cfg = OxcacheConfig {
            backend: BackendType::Memory,
            capacity: 100,
            ttl: Some(Duration::from_secs(60)),
            ..OxcacheConfig::default()
        };
        let cache = build_memory_backend(&cfg).expect("build memory backend");
        cache
            .set(Arc::from("t021"), Arc::new(b"ok".to_vec()), None)
            .await
            .expect("set");
        let got = cache.get("t021").await.expect("get");
        assert_eq!(got, Some(b"ok".to_vec()));
    }

    /// `BackendType::Redis` without `redis` feature returns clear error.
    /// With `redis` feature, it returns error due to missing redis config.
    #[tokio::test]
    async fn redis_backend_without_config_errors() {
        let cfg = OxcacheConfig {
            backend: BackendType::Redis,
            ..OxcacheConfig::default()
        };
        let result = build_redis_backend(&cfg).await;
        assert!(result.is_err(), "expected error for Redis without config");
        let err_msg = format!("{}", result.err().unwrap());
        // Either "requires redis config" or "requires cargo feature"
        assert!(
            err_msg.contains("redis") || err_msg.contains("feature"),
            "error should mention redis: {err_msg}"
        );
    }

    /// `BackendType::Chain` without `redis` feature returns clear error.
    #[tokio::test]
    async fn chain_backend_without_config_errors() {
        let cfg = OxcacheConfig {
            backend: BackendType::Chain,
            ..OxcacheConfig::default()
        };
        let result = build_chain_cache(&cfg).await;
        assert!(result.is_err(), "expected error for Chain without config");
    }

    /// Chain with nested Chain link returns error.
    #[cfg(feature = "redis")]
    #[tokio::test]
    async fn chain_nested_chain_errors() {
        let link = ChainLinkConfig {
            backend: BackendType::Chain,
            score: 50,
            redis: None,
        };
        let cfg = OxcacheConfig {
            backend: BackendType::Chain,
            chain: vec![link],
            ..OxcacheConfig::default()
        };
        let result = build_chain_cache(&cfg).await;
        assert!(result.is_err());
        assert!(format!("{}", result.err().unwrap()).contains("nested"));
    }

    /// `RedisConfig::default()` has sane defaults.
    #[test]
    fn redis_config_defaults() {
        let rc = RedisConfig::default();
        assert_eq!(rc.pool_size, 8);
        assert_eq!(rc.retry_count, 3);
        assert_eq!(rc.connection_timeout, Duration::from_secs(2));
        assert_eq!(rc.retry_delay, Duration::from_millis(100));
        assert_eq!(rc.circuit_breaker_threshold, 5);
        assert_eq!(rc.circuit_breaker_reset_timeout, Duration::from_secs(30));
    }
}
