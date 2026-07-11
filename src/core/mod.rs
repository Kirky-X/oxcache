// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 核心基础模块
//!
//! 提供缓存系统的基础类型、常量、特性标志和事件定义。

pub mod command;
pub mod constants;
pub mod events;
pub mod features;
pub mod types;

// Re-export commonly used items for convenience
pub use constants::{
    CacheCapacity, PoolSize, DEFAULT_BATCH_SIZE, DEFAULT_CACHE_CAPACITY, DEFAULT_COMMAND_TIMEOUT_SECS,
    DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_POOL_SIZE, DEFAULT_SCAN_COUNT, DEFAULT_TTL_SECS, FORBIDDEN_LUA_COMMANDS,
    MAX_BATCH_SIZE, MAX_CACHE_CAPACITY, MAX_JSON_DEPTH, MAX_JSON_SIZE, MAX_KEY_LENGTH, MAX_LUA_SCRIPT_KEYS,
    MAX_LUA_SCRIPT_SIZE, MAX_POOL_SIZE, MAX_TTL_SECS, MAX_VALUE_SIZE, MIN_CACHE_CAPACITY, MIN_POOL_SIZE,
    PASSWORD_MASK_ASTERISKS,
};

pub use types::{BackendType, CacheLayer, RedisModeType, SerializationType};
