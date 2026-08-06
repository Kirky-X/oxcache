// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! trait-kit 0.4 `AsyncKit` integration for oxcache.
//!
//! Enable via the `kit` cargo feature. Provides [`OxcacheModule`] — a leaf
//! module (no upstream dependencies) that constructs an oxcache
//! [`CacheBackend`](crate::backend::CacheBackend) capability during
//! [`AsyncKit::build`](trait_kit::AsyncKit::build).
//!
//! # Features
//!
//! - **AsyncKit integration** — `OxcacheModule` implements `AsyncAutoBuilder`
//!   to construct cache backends via the trait-kit dependency injection framework.
//! - **Health checks** — `AsyncHealthCheck` implementation reports backend health
//!   status via `kit.health_check::<OxcacheModule>()`.
//! - **Lifecycle hooks** — `AsyncLifecycle` implementation provides graceful
//!   shutdown via `kit.shutdown()`.
//!
//! See `specmark/changes/trait-kit-async-integration/specs/oxcache-module/spec.md`
//! for the acceptance criteria driving this module.

pub mod decorator;
pub mod module;
pub mod observer;
pub mod shutdown;

pub use decorator::{CacheBackendDecorator, register_cache_decorator};
pub use module::{OxcacheConfig, OxcacheModule};
pub use observer::OxcacheBuildObserver;
pub use shutdown::register_cache_shutdown;
