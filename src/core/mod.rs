//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 核心基础模块
//!
//! 提供缓存系统的基础类型、常量、特性标志和事件定义。

pub mod constants;
pub mod events;
pub mod features;
pub mod types;

// Re-export commonly used items for convenience
pub use constants::*;
pub use events::{CacheEvent, CacheEventType, EventPublisher};
pub use features::FeatureSet;
pub use types::{BackendType, CacheLayer, RedisModeType, SerializationType};
