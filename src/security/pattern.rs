// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! glob 模式校验（本地匹配后端通用）
//!
//! 本地 glob 匹配（[`crate::backend::interface::glob_match`]）对每个候选键
//! 都要跑一遍模式：单次比较代价 O(模式长 × 键长)，且模式会被逐键重复展开
//! 成 `Vec<char>`。模式长度不受约束时，一次模式操作（全表扫描）的 CPU 与
//! 分配开销可被调用方放大多倍，故模式长度须设上限。
//!
//! 与 `validate_scan_pattern` 的分工：后者面向 Redis SCAN（模式发往服务端
//! 求值，另含通配符数量上限），只在 `redis` 特性下编译；本模块面向本地
//! glob 匹配，与后端无关（内存 / 磁盘后端共用同一守卫）。
//!
//! 不限制通配符数量：本地匹配是迭代双指针算法，单次比较恒为 O(模式长 ×
//! 键长)，通配符多寡不改变代价量级——它只影响 Redis 服务端求值，故通配符
//! 上限留在 `validate_scan_pattern` 侧。

use crate::error::{OxCacheError, OxCacheResult};
use crate::i18n::messages::{MSG_DETAIL_PATTERN_TOO_LONG, t};

/// 本地 glob 模式最大长度（字符数）
///
/// 与 Redis SCAN 模式上限同值，便于调用方按同一阈值预校验。
pub const MAX_GLOB_PATTERN_LENGTH: usize = 256;

/// 校验本地 glob 模式
///
/// 模式长度按**字符**计（本地匹配按字符进行，代价与字符数相关，而非字节
/// 数）；超过 [`MAX_GLOB_PATTERN_LENGTH`] 即拒绝。
///
/// # 参数
///
/// * `pattern` - 待校验的 glob 模式
///
/// # 返回值
///
/// * `Ok(())` - 模式验证通过
/// * `Err(OxCacheError::InvalidInput)` - 模式长度超限
pub fn validate_glob_pattern(pattern: &str) -> OxCacheResult<()> {
    let len = pattern.chars().count();
    if len > MAX_GLOB_PATTERN_LENGTH {
        return Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_PATTERN_TOO_LONG,
            &[
                ("max", MAX_GLOB_PATTERN_LENGTH.to_string()),
                ("actual", len.to_string()),
            ],
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_pattern_passes() {
        assert!(validate_glob_pattern("user:*").is_ok());
        assert!(validate_glob_pattern("*").is_ok());
        assert!(validate_glob_pattern("").is_ok());
    }

    #[test]
    fn pattern_at_limit_passes() {
        let pattern = "x".repeat(MAX_GLOB_PATTERN_LENGTH);
        assert!(validate_glob_pattern(&pattern).is_ok());
    }

    #[test]
    fn pattern_over_limit_is_rejected() {
        let pattern = "x".repeat(MAX_GLOB_PATTERN_LENGTH + 1);
        let err = validate_glob_pattern(&pattern).unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(_)));
    }

    /// 长度按字符计：多字节字符的字节数超过上限但字符数未超限时仍应通过
    #[test]
    fn limit_counts_chars_not_bytes() {
        let pattern = "密".repeat(MAX_GLOB_PATTERN_LENGTH);
        assert!(pattern.len() > MAX_GLOB_PATTERN_LENGTH);
        assert!(validate_glob_pattern(&pattern).is_ok());
    }

    /// 通配符数量不设上限：本地匹配代价与通配符数量无关
    #[test]
    fn many_wildcards_are_accepted() {
        let pattern = "*".repeat(MAX_GLOB_PATTERN_LENGTH);
        assert!(validate_glob_pattern(&pattern).is_ok());
    }
}
