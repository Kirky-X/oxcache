//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis client implementations

pub mod client;
pub mod provider;
pub mod unified;

// Re-export main types for convenience
pub use client::{RedisBackend, RedisMode, RedisBackendBuilder};
pub use provider::{RedisProvider, DefaultRedisProvider};
pub use unified::{UnifiedRedisBackend, UnifiedRedisManager, RedisConfig};
