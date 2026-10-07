// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 核心基础模块
//!
//! 提供缓存系统的基础类型、常量和事件定义。

// RedisCommand 仅被 redis 系子特性（redis/lua/lock/...）消费，非 redis
// 组合下无任何调用方，模块整体随 redis 门控编译，避免悬空调用面。
#[cfg(feature = "redis")]
pub mod command;
pub mod constants;
pub mod events;
pub mod types;

pub use types::{BackendType, CacheLayer, RedisModeType, SerializationType};

#[cfg(feature = "redis")]
pub use command::RedisCommand;

pub use events::{CacheEvent, CacheEventType, EventPublisher};
