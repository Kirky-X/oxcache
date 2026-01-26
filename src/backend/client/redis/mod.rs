//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis client implementations

pub mod client;
#[cfg(feature = "redis")]
pub mod provider;

// Re-export main types for convenience
pub use client::{RedisBackend, RedisBackendBuilder, RedisConfig, RedisMode};
#[cfg(feature = "redis")]
pub use provider::{DefaultRedisProvider, RedisProvider};
