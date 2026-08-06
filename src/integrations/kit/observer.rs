// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! `OxcacheBuildObserver` — trait-kit `BuildObserver` implementation for oxcache.
//!
//! Provides a no-op `BuildObserver` that can be registered with
//! `AsyncKit::with_observer` to satisfy the observer API contract.
//! Downstream users can provide their own `BuildObserver` implementations
//! for custom build-pipeline observability.
//!
//! Register via `kit.with_observer(Arc::new(OxcacheBuildObserver))`.

use trait_kit::prelude::*;

/// No-op build observer for oxcache modules.
///
/// All callbacks use the default trait implementations (no-op).
/// This type exists as a convenience for downstream users who want
/// to register an observer placeholder or extend it with custom logic.
pub struct OxcacheBuildObserver;

impl BuildObserver for OxcacheBuildObserver {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    /// `OxcacheBuildObserver` implements `BuildObserver` and all three
    /// default callbacks can be called without panicking.
    #[test]
    fn observer_callbacks_do_not_panic() {
        let obs = OxcacheBuildObserver;
        obs.on_module_start("test-module");
        obs.on_module_built("test-module", Duration::from_millis(5));
        let err = TraitKitError::MissingCapability { key: "test".into() };
        obs.on_build_error("test-module", &err);
    }

    /// `OxcacheBuildObserver` is `Send + Sync` (required by `BuildObserver`).
    #[test]
    fn observer_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OxcacheBuildObserver>();
    }

    /// `OxcacheBuildObserver` can be wrapped in `Arc` and passed to
    /// `AsyncKit::with_observer`.
    #[tokio::test]
    async fn observer_registers_with_async_kit() {
        let mut kit = AsyncKit::new();
        kit.set_config(crate::integrations::kit::OxcacheConfig::default());
        kit.with_observer(Arc::new(OxcacheBuildObserver));
        kit.register::<crate::integrations::kit::OxcacheModule>()
            .expect("register OxcacheModule");
        let _kit = kit.build().await.expect("AsyncKit::build");
    }
}
