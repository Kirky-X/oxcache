//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! DashMap memory backend implementation

pub mod backend;

// Re-export main types for convenience
pub use backend::{DashMapMemoryBackend, DashMapBackendBuilder, dashmap_memory, dashmap_memory_with_capacity, dashmap_memory_with_capacity_and_ttl};
