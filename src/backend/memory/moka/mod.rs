//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Moka memory backend implementation

pub mod backend;

// Re-export main types for convenience
pub use backend::{
    MokaMemoryBackend, MokaMemoryBackendBuilder, default_memory_backend, moka_memory, moka_memory_with_capacity,
    moka_memory_with_capacity_and_ttl,
};
