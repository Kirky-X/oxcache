// Copyright (c) 2025-2026 Kirky.X
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
pub const MAX_JSON_SIZE: usize = 5 * 1024 * 1024; // 5MB

/// 最大 JSON 反序列化深度
pub const MAX_JSON_DEPTH: usize = 64;

// ============================================================================
// 缓存穿透防护常量
// ============================================================================

/// Null sentinel 值：当 fallback 返回 None 时写入缓存，阻止穿透
/// 使用固定 magic bytes 避免与合法 JSON `null` 冲突
pub const NULL_SENTINEL: &[u8] = b"\x00OXNULL";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_values() {
        assert_eq!(MAX_JSON_SIZE, 5 * 1024 * 1024);
        assert_eq!(MAX_JSON_DEPTH, 64);
        assert_eq!(NULL_SENTINEL, b"\x00OXNULL");
    }
}
