//! trait-kit 0.2.2 `AsyncKit` integration for oxcache.
//!
//! Enable via the `kit` cargo feature. Provides [`OxcacheModule`] — a leaf
//! module (no upstream dependencies) that constructs an oxcache
//! [`UnifiedCache`](crate::UnifiedCache) capability during
//! [`AsyncKit::build`](trait_kit::AsyncKit::build).
//!
//! See `specmark/changes/trait-kit-async-integration/specs/oxcache-module/spec.md`
//! for the acceptance criteria driving this module.

pub mod module;
pub use module::{OxcacheConfig, OxcacheModule};
