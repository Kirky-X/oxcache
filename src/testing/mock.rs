// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Re-export MockBackend for crate-internal test usage

#[cfg(test)]
pub use crate::backend::memory::mock::MockBackend;
