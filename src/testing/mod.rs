// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Testing module — internal test utilities

// MockBackend 位处 backend 模块（门控 any(memory,redis,disk)），随之门控
#[cfg(all(test, any(feature = "memory", feature = "redis", feature = "disk")))]
pub mod mock;

#[cfg(all(test, any(feature = "memory", feature = "redis", feature = "disk")))]
pub use mock::MockBackend;
