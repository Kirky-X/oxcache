// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Moka memory backend implementation

pub mod backend;

// Re-export main types for convenience
pub use backend::{
    default_memory_backend, moka_memory, moka_memory_with_capacity, moka_memory_with_capacity_and_ttl,
    MokaMemoryBackend, MokaMemoryBackendBuilder,
};
