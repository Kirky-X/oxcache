// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! oxcache integration modules with external frameworks.
//!
//! Integrations are feature-gated so the core cache library stays
//! dependency-free when integrations are not needed.

#[cfg(feature = "kit")]
pub mod kit;
