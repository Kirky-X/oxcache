// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 该模块定义了缓存系统中实际消费的常量。
//!
//! 仅收录 crate 内部有真实调用方的常量；仅被自身单元测试引用的
//! "参考值" 常量不在此保留（`mod core` 为私有模块，这些常量从 crate 根
//! 不可达，删除无 semver 影响）。

// ============================================================================
// 序列化相关常量
// ============================================================================

/// 最大 JSON 反序列化大小（字节）
///
/// 消费方在 serialization（json/binary 序列化器）与 warmup 面，故随
/// 该组 feature 门控，非对应组合不参与编译。
#[cfg(any(feature = "serialization", feature = "warmup"))]
pub const MAX_JSON_SIZE: usize = 5 * 1024 * 1024; // 5MB

/// 最大 JSON 反序列化深度
///
/// 消费方在 serialization 序列化器与 cache 接口/命名空间面。
#[cfg(any(
    feature = "serialization",
    feature = "memory",
    feature = "redis",
    feature = "disk"
))]
pub const MAX_JSON_DEPTH: usize = 64;

// ============================================================================
// 缓存穿透防护常量
// ============================================================================

/// Null sentinel 值：当 fallback 返回 None 时写入缓存，阻止穿透
/// 使用固定 magic bytes 避免与合法 JSON `null` 冲突
///
/// 仅 cache API 的 get_or_option 路径消费。
#[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
pub const NULL_SENTINEL: &[u8] = b"\x00OXNULL";

// ============================================================================
// 缓存雪崩防护常量
// ============================================================================

/// 运行时默认 TTL 抖动因子（±10% 均匀抖动）。
///
/// 审计 F06：0.0 意味着开箱无雪崩防护——同批写入的 key 同时过期。
/// 默认开启后 `set_with_ttl` 的实际 TTL = `ttl * (1 ± 0.1)`；需要精确 TTL
/// 的调用方可经 `CacheBuilder::ttl_jitter(0.0)` 显式关闭。
///
/// 仅 cache builder/API 消费。
#[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
pub const DEFAULT_TTL_JITTER_FACTOR: f64 = 0.1;

#[cfg(all(
    test,
    any(feature = "serialization", feature = "memory", feature = "redis")
))]
mod tests {
    use super::*;

    #[test]
    fn test_constants_values() {
        #[cfg(any(feature = "serialization", feature = "warmup"))]
        assert_eq!(MAX_JSON_SIZE, 5 * 1024 * 1024);
        #[cfg(any(
            feature = "serialization",
            feature = "memory",
            feature = "redis",
            feature = "disk"
        ))]
        assert_eq!(MAX_JSON_DEPTH, 64);
        #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
        assert_eq!(NULL_SENTINEL, b"\x00OXNULL");
    }
}
