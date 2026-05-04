// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Re-export MockBackend for crate-internal test usage

#[cfg(test)]
pub use crate::backend::memory::mock::MockBackend;
