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

pub use types::{BackendType, CacheLayer, RedisModeType, SerializationType};

// RedisCommand 仅被 redis 系子特性（redis/lua/lock/...）消费，
// 非 redis 组合下不导出，避免悬空的未使用再导出。
#[cfg(feature = "redis")]
pub use command::RedisCommand;

pub use events::{CacheEvent, CacheEventType, EventPublisher};
