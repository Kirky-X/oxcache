// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Backend provider module for oxcache.
//!
//! Re-exports ISP-compliant trait hierarchy (`CacheReader`/`CacheWriter`/
//! `CacheConnector`/`CacheBackend`) and concrete backend implementations
//! (memory, Redis, Dragonfly, Aerospike, chain).

// Backend score system
pub mod score;

// Memory backend implementations
pub mod memory;

// Modernized API backend interface
pub mod interface;

// Configuration validation utilities
pub mod config_validation;

// Dragonfly backend (Redis-compatible, feature-gated)
#[cfg(feature = "dragonfly")]
pub mod dragonfly;

// Aerospike backend (independent protocol, feature-gated)
#[cfg(feature = "aerospike")]
pub mod aerospike;

// Disk-persistent embedded backend (redb, feature-gated)
#[cfg(feature = "disk")]
pub mod disk;

// Custom tiered backend configuration (always available)
#[cfg(any(feature = "memory", feature = "redis"))]
pub mod custom_tiered;

// Backend factory registry
#[cfg(any(feature = "memory", feature = "redis"))]
pub mod factory;

pub use factory::{BackendFactory, BackendRegistry, BackendSpec};

// Re-exports for new API
pub use interface::CacheSetItem;
pub use interface::{CacheBackend, CacheConnector, CacheReader, CacheWriter};
// Re-export atomic operation traits
pub use interface::{AtomicCacheWriter, SyncAtomicCacheWriter};
// Re-exports for synchronous API (任务组 5)
pub use interface::{SyncCacheBackend, SyncCacheConnector, SyncCacheReader, SyncCacheWriter};

// Re-export BackendKind for runtime type identification
pub use interface::BackendKind;

// Re-export LuaExecutor trait for Lua script execution
#[cfg(feature = "lua")]
pub use interface::LuaExecutor;

// Re-export ConfigValidation for configuration validation utilities
pub use config_validation::ConfigValidation;

// Dragonfly backend re-exports
#[cfg(feature = "dragonfly")]
pub use dragonfly::{DragonflyBackend, DragonflyRestrictions};

// Aerospike backend re-exports
#[cfg(feature = "aerospike")]
pub use aerospike::{AerospikeBackend, AerospikeConfig};

// Score system exports
pub use score::{BackendScore, Scores};

// Memory backend implementations
// 按依赖开启特性门控（dashmap: memory/offload；moka: memory/byte-weight；
// serde derive: memory/serialization），与 memory 模块内部门控保持一致
#[cfg(any(feature = "memory", feature = "offload"))]
pub use memory::DashMapMemoryBackend;
#[cfg(any(feature = "memory", feature = "serialization"))]
pub use memory::MemoryBackendType;
#[cfg(any(feature = "memory", feature = "byte-weight"))]
pub use memory::MokaMemoryBackend;
#[cfg(any(feature = "memory", feature = "offload"))]
pub use memory::dashmap_memory;
#[cfg(any(feature = "memory", feature = "byte-weight"))]
pub use memory::{default_memory_backend, moka_memory};

// Re-export MockBackend for crate-internal test usage
#[cfg(test)]
pub use memory::MockBackend;

#[cfg(feature = "redis")]
pub use memory::{RedisBackend, RedisBackendBuilder, RedisMode};

// Re-exports for custom tiered configuration
#[cfg(any(feature = "memory", feature = "redis"))]
pub use custom_tiered::LayerRestriction;

// 从 core::types 重新导出统一的枚举类型
pub use crate::core::{BackendType, CacheLayer};
