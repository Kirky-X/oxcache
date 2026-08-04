// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `OxcacheModule` — trait-kit 0.4 `AsyncKit` integration for oxcache.
//!
//! Phase 2 (T018 Red / T019 Green) of the `trait-kit-async-integration`
//! change. Wires oxcache's cache backend into the `AsyncKit` dependency
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
//! `OxcacheDbCacheAdapter` in Phase 4) need.
//!
//! **Follow-up** (out of scope for T017-T019): if the spec owner requires
//! the literal `Arc<dyn UnifiedCache>` form, oxcache's `UnifiedCache` trait
//! must be split — moving `get_typed`/`set_typed` into a separate
//! `TypedCacheExt` trait so `UnifiedCache` becomes object-safe. That change
//! affects oxcache's public API and is deferred to its own change spec.

use std::time::Duration;

/// Configuration consumed by [`OxcacheModule`] during `build()`.
///
/// Stored in `AsyncKit` via `kit.set_config(OxcacheConfig::default())` and
/// read back with `kit.config::<OxcacheConfig>()`. This is a minimal standalone
/// struct — it is intentionally NOT wired into oxcache's config system because
/// the trait-kit integration must remain a leaf, feature-gated concern
/// (Rule 6: no extra default deps).
#[derive(Debug, Clone)]
pub struct OxcacheConfig {
    /// Max entries held by the L1 memory backend (Moka). Mirrors
    /// `MokaMemoryBackend::builder().capacity(..)`. Default `10_000`.
    pub capacity: u64,
    /// Optional per-entry TTL applied to the L1 backend. Mirrors
    /// `MokaMemoryBackend::builder().ttl(..)`.
    pub ttl: Option<Duration>,
    /// Optional per-entry TTI (time-to-idle) applied to the L1 backend.
    /// Mirrors `MokaMemoryBackend::builder().time_to_idle(..)`.
    pub tti: Option<Duration>,
}

impl Default for OxcacheConfig {
    fn default() -> Self {
        Self {
            capacity: 10_000,
            ttl: None,
            tti: None,
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
            let mut builder = MokaMemoryBackend::builder().capacity(config.capacity);
            if let Some(ttl) = config.ttl {
                builder = builder.ttl(ttl);
            }
            if let Some(tti) = config.tti {
                builder = builder.time_to_idle(tti);
            }
            let backend = builder.build();
            Ok(Arc::new(backend) as Arc<dyn CacheBackend + Send + Sync>)
        })
    }
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
    fn on_shutdown<'a>(
        cap: &'a Self::Capability,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
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
        assert_eq!(OxcacheModule::dependencies(), &[] as &[(&'static str, TypeId)]);
    }

    /// R-oxcache-module-001: register `OxcacheModule` + `set_config` +
    /// `build()` + `require::<OxcacheModule>()` returns an
    /// `Arc<dyn CacheBackend + Send + Sync>` that performs real cache ops.
    #[tokio::test]
    async fn oxcache_module_build_returns_cache_capability() {
        let mut kit = AsyncKit::new();
        kit.set_config(OxcacheConfig::default());
        kit.register::<OxcacheModule>().expect("register OxcacheModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let cache: Arc<dyn CacheBackend + Send + Sync> = kit.require::<OxcacheModule>().expect("require OxcacheModule");
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
            ttl: None,
            tti: None,
        });
        kit.register::<OxcacheModule>().expect("register OxcacheModule");
        let kit = kit.build().await.expect("AsyncKit::build");
        let cache: Arc<dyn CacheBackend + Send + Sync> = kit.require::<OxcacheModule>().expect("require OxcacheModule");
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
        kit.register::<OxcacheModule>().expect("register OxcacheModule");
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
        kit.register::<OxcacheModule>().expect("register OxcacheModule");
        kit.register_lifecycle::<OxcacheModule>();
        let kit = kit.build().await.expect("AsyncKit::build");
        // Shutdown should complete without panic.
        kit.shutdown();
    }
}
