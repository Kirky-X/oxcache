//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis client implementations

pub mod client;

// Re-export main types for convenience
pub use client::{RedisBackend, RedisBackendBuilder, RedisConfig, RedisMode};
