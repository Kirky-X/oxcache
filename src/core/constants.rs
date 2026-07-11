// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 该模块定义了缓存系统中使用的常量。

use crate::core::command::RedisCommand;

// ============================================================================
// 缓存容量相关常量
// ============================================================================

/// 默认缓存容量
pub const DEFAULT_CACHE_CAPACITY: u64 = 10_000;

/// 最小缓存容量
pub const MIN_CACHE_CAPACITY: u64 = 100;

/// 最大缓存容量
pub const MAX_CACHE_CAPACITY: u64 = 10_000_000;

/// 类型安全的缓存容量封装
#[derive(Debug, Clone, Copy)]
pub struct CacheCapacity(u64);

impl CacheCapacity {
    /// 创建新的缓存容量，如果值超出范围则返回 None
    pub fn new(capacity: u64) -> Option<Self> {
        if (MIN_CACHE_CAPACITY..=MAX_CACHE_CAPACITY).contains(&capacity) {
            Some(Self(capacity))
        } else {
            None
        }
    }

    /// 获取容量值
    pub fn value(&self) -> u64 {
        self.0
    }

    /// 获取默认值
    pub fn default_capacity() -> Self {
        Self(DEFAULT_CACHE_CAPACITY)
    }
}

impl Default for CacheCapacity {
    fn default() -> Self {
        Self::default_capacity()
    }
}

// ============================================================================
// Redis 相关常量
// ============================================================================

/// 默认 Redis 连接 URL
pub const DEFAULT_REDIS_URL: &str = "redis://localhost:6379";

/// 默认 Redis 端口
pub const DEFAULT_REDIS_PORT: u16 = 6379;

/// 默认 Redis 主机
pub const DEFAULT_REDIS_HOST: &str = "localhost";

/// 默认 OTLP 收集器端点
pub const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4318";

/// 默认 Redis 连接池大小
pub const DEFAULT_POOL_SIZE: usize = 10;

/// 最小 Redis 连接池大小
pub const MIN_POOL_SIZE: usize = 1;

/// 最大 Redis 连接池大小
pub const MAX_POOL_SIZE: usize = 100;

/// 类型安全的连接池大小封装
#[derive(Debug, Clone, Copy)]
pub struct PoolSize(usize);

impl PoolSize {
    /// 创建新的池大小，如果值超出范围则返回 None
    pub fn new(size: usize) -> Option<Self> {
        if (MIN_POOL_SIZE..=MAX_POOL_SIZE).contains(&size) {
            Some(Self(size))
        } else {
            None
        }
    }

    /// 获取池大小值
    pub fn value(&self) -> usize {
        self.0
    }

    /// 获取默认值
    pub fn default_size() -> Self {
        Self(DEFAULT_POOL_SIZE)
    }
}

impl Default for PoolSize {
    fn default() -> Self {
        Self::default_size()
    }
}

/// 默认连接超时（秒）
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 5;

/// 默认命令超时（秒）
pub const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 5;

/// Redis SCAN 命令默认批量大小
pub const DEFAULT_SCAN_COUNT: i64 = 100;

// ============================================================================
// Lua 脚本相关常量
// ============================================================================

/// 最大 Lua 脚本大小（字节）
pub const MAX_LUA_SCRIPT_SIZE: usize = 10 * 1024; // 10KB

/// 最大 Lua 脚本键数量
pub const MAX_LUA_SCRIPT_KEYS: usize = 100;

// ============================================================================
// 序列化相关常量
// ============================================================================

/// 最大 JSON 反序列化大小（字节）
pub const MAX_JSON_SIZE: usize = 5 * 1024 * 1024; // 5MB

/// 最大 JSON 反序列化深度
pub const MAX_JSON_DEPTH: usize = 64;

// ============================================================================
// 键/值限制常量
// ============================================================================

/// 最大键长度（字节）
pub const MAX_KEY_LENGTH: usize = 1024;

/// 最大值大小（字节）
pub const MAX_VALUE_SIZE: usize = 100 * 1024 * 1024; // 100MB

// ============================================================================
// 批量操作相关常量
// ============================================================================

/// 批量操作默认批次大小
pub const DEFAULT_BATCH_SIZE: usize = 100;

/// 最大批量操作大小
pub const MAX_BATCH_SIZE: usize = 10_000;

// ============================================================================
// TTL 相关常量
// ============================================================================

/// 默认 TTL（秒）
pub const DEFAULT_TTL_SECS: u64 = 3600; // 1小时

/// 最大 TTL（秒）
pub const MAX_TTL_SECS: u64 = 30 * 24 * 60 * 60; // 30天

// ============================================================================
// 密码脱敏相关常量
// ============================================================================

/// 密码掩码星号数量
pub const PASSWORD_MASK_ASTERISKS: usize = 5;

// ============================================================================
// 安全相关常量
// ============================================================================

/// Forbidden Lua script commands
pub const FORBIDDEN_LUA_COMMANDS: &[RedisCommand] = &[
    RedisCommand::FlushAll,
    RedisCommand::FlushDb,
    RedisCommand::Keys,
    RedisCommand::Shutdown,
    RedisCommand::Debug,
    RedisCommand::Config,
    RedisCommand::Save,
    RedisCommand::BgSave,
    RedisCommand::BgRewriteAof,
    RedisCommand::SlaveOf,
    RedisCommand::ReplicaOf,
    RedisCommand::Cluster,
    RedisCommand::Admin,
    RedisCommand::Monitor,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_capacity_valid() {
        assert!(CacheCapacity::new(100).is_some());
        assert!(CacheCapacity::new(10_000).is_some());
        assert!(CacheCapacity::new(10_000_000).is_some());
    }

    #[test]
    fn test_cache_capacity_invalid() {
        assert!(CacheCapacity::new(0).is_none());
        assert!(CacheCapacity::new(99).is_none());
        assert!(CacheCapacity::new(10_000_001).is_none());
    }

    #[test]
    fn test_cache_capacity_default() {
        let cap = CacheCapacity::default();
        assert_eq!(cap.value(), DEFAULT_CACHE_CAPACITY);
    }

    #[test]
    fn test_cache_capacity_value() {
        let cap = CacheCapacity::new(5000).unwrap();
        assert_eq!(cap.value(), 5000);
    }

    #[test]
    fn test_cache_capacity_debug() {
        let cap = CacheCapacity::new(1000).unwrap();
        let debug = format!("{:?}", cap);
        assert!(debug.contains("CacheCapacity"));
    }

    #[test]
    fn test_pool_size_valid() {
        assert!(PoolSize::new(1).is_some());
        assert!(PoolSize::new(10).is_some());
        assert!(PoolSize::new(100).is_some());
    }

    #[test]
    fn test_pool_size_invalid() {
        assert!(PoolSize::new(0).is_none());
        assert!(PoolSize::new(101).is_none());
    }

    #[test]
    fn test_pool_size_default() {
        let size = PoolSize::default();
        assert_eq!(size.value(), DEFAULT_POOL_SIZE);
    }

    #[test]
    fn test_pool_size_value() {
        let size = PoolSize::new(50).unwrap();
        assert_eq!(size.value(), 50);
    }

    #[test]
    fn test_pool_size_debug() {
        let size = PoolSize::new(10).unwrap();
        let debug = format!("{:?}", size);
        assert!(debug.contains("PoolSize"));
    }

    #[test]
    fn test_constants_values() {
        assert_eq!(DEFAULT_CONNECT_TIMEOUT_SECS, 5);
        assert_eq!(DEFAULT_COMMAND_TIMEOUT_SECS, 5);
        assert_eq!(DEFAULT_SCAN_COUNT, 100);
        assert_eq!(MAX_LUA_SCRIPT_SIZE, 10 * 1024);
        assert_eq!(MAX_LUA_SCRIPT_KEYS, 100);
        assert_eq!(MAX_JSON_SIZE, 5 * 1024 * 1024);
        assert_eq!(MAX_JSON_DEPTH, 64);
        assert_eq!(MAX_KEY_LENGTH, 1024);
        assert_eq!(MAX_VALUE_SIZE, 100 * 1024 * 1024);
        assert_eq!(DEFAULT_BATCH_SIZE, 100);
        assert_eq!(MAX_BATCH_SIZE, 10_000);
        assert_eq!(DEFAULT_TTL_SECS, 3600);
        assert_eq!(MAX_TTL_SECS, 30 * 24 * 60 * 60);
        assert_eq!(PASSWORD_MASK_ASTERISKS, 5);
    }

    #[test]
    fn test_redis_url_constants() {
        assert_eq!(DEFAULT_REDIS_URL, "redis://localhost:6379");
        assert_eq!(DEFAULT_REDIS_PORT, 6379);
        assert_eq!(DEFAULT_REDIS_HOST, "localhost");
        assert_eq!(DEFAULT_OTLP_ENDPOINT, "http://localhost:4318");
    }

    #[test]
    fn test_forbidden_lua_commands_not_empty() {
        assert!(!FORBIDDEN_LUA_COMMANDS.is_empty());
        assert!(FORBIDDEN_LUA_COMMANDS.contains(&RedisCommand::FlushAll));
        assert!(FORBIDDEN_LUA_COMMANDS.contains(&RedisCommand::Shutdown));
        assert!(FORBIDDEN_LUA_COMMANDS.contains(&RedisCommand::Keys));
    }
}
