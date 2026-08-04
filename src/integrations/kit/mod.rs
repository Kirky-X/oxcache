// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! trait-kit 0.3 `AsyncKit` integration for oxcache.
//!
//! Enable via the `kit` cargo feature. Provides [`OxcacheModule`] — a leaf
//! module (no upstream dependencies) that constructs an oxcache
//! [`CacheBackend`](crate::backend::CacheBackend) capability during
//! [`AsyncKit::build`](trait_kit::AsyncKit::build).
//!
//! See `specmark/changes/trait-kit-async-integration/specs/oxcache-module/spec.md`
//! for the acceptance criteria driving this module.

pub mod module;
pub use module::{OxcacheConfig, OxcacheModule};
