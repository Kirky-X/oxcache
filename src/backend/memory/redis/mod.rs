// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Redis client implementations

pub mod client;

// Internal modules
pub(crate) mod async_traits;
pub(crate) mod builder;
pub(crate) mod circuit_breaker;
pub(crate) mod error;
#[cfg(feature = "lua-script")]
pub(crate) mod lua_executor;
pub(crate) mod namespace;
pub(crate) mod pipeline;
pub(crate) mod retry;
pub(crate) mod sync_traits;

#[cfg(test)]
#[allow(unsafe_code)]
mod tests;

// Re-export main types for convenience
pub use builder::{RedisBackendBuilder, RedisMode};
pub use client::RedisBackend;
