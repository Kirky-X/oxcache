// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Advanced scenarios E2E test binary entry point.
//!
//! References the actual test module under `tests/e2e/` and the shared
//! `common` test helpers. Each `tests/*.rs` file is compiled as a separate
//! binary by Cargo, so this entry file is required for
//! `cargo test --test advanced_scenarios_test` to discover and run the
//! scenarios defined in `tests/e2e/advanced_scenarios_test.rs`.

#![allow(clippy::duplicate_mod)]

#[path = "e2e/advanced_scenarios_test.rs"]
mod advanced_scenarios_test;

#[path = "common/mod.rs"]
mod common;
