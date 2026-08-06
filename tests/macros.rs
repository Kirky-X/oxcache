// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Macros Tests Module
//
// Contains integration tests for the #[cached] proc macro.

#![cfg(feature = "macros")]
#![allow(clippy::duplicate_mod)]

#[path = "macros/skip_cache_write_test.rs"]
mod skip_cache_write_test;

#[path = "macros/sync_test.rs"]
mod sync_test;
