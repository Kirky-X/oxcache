// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
#[path = "common/mod.rs"]
pub mod common;

#[cfg(feature = "redis")]
#[path = "performance/performance_test.rs"]
mod performance_test;

#[cfg(feature = "redis")]
#[path = "performance/memory_tests.rs"]
mod memory_tests;

#[cfg(feature = "redis")]
#[path = "performance/memory_leak_test.rs"]
mod memory_leak_test;

#[path = "performance/miri_memory_test.rs"]
mod miri_memory_test;

#[cfg(feature = "redis")]
#[path = "performance/pipeline_performance_test.rs"]
mod pipeline_performance_test;
