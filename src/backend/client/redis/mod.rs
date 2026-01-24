//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis client implementations

pub mod client;
pub mod provider;

// Re-export main types for convenience
pub use client::{RedisBackend, RedisMode, RedisBackendBuilder, RedisConfig};
pub use provider::{RedisProvider, DefaultRedisProvider};
