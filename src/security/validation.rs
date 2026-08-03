// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 通用验证工具模块
//!
//! 提供跨模块的字符串验证功能，避免代码重复

use crate::error::OxCacheError;

/// 验证字符串中是否包含危险字符
///
/// # Arguments
///
/// * `input` - 待验证的字符串
/// * `dangerous_chars` - 危险字符列表
/// * `error_context` - 错误上下文描述
///
/// # Returns
///
/// * `Ok(())` - 验证通过
/// * `Err(OxCacheError)` - 包含危险字符
///
/// # Examples
///
/// ```rust,ignore
/// use oxcache::utils::validation::validate_no_dangerous_chars;
///
/// let dangerous_chars = ['\r', '\n', '\0'];
/// validate_no_dangerous_chars("safe_string", &dangerous_chars, "Redis key")?;
/// ```
pub fn validate_no_dangerous_chars(
    input: &str,
    dangerous_chars: &[char],
    error_context: &str,
) -> crate::OxCacheResult<()> {
    for c in input.chars() {
        if dangerous_chars.contains(&c) {
            return Err(OxCacheError::InvalidInput(format!(
                "{} contains dangerous character '\\u{:04x}'",
                error_context, c as u32
            )));
        }
    }
    Ok(())
}

/// 验证字符串是否为空
///
/// # Arguments
///
/// * `input` - 待验证的字符串
/// * `error_context` - 错误上下文描述
///
/// # Returns
///
/// * `Ok(())` - 验证通过
/// * `Err(OxCacheError)` - 字符串为空
pub fn validate_not_empty(input: &str, error_context: &str) -> crate::OxCacheResult<()> {
    if input.is_empty() {
        return Err(OxCacheError::InvalidInput(format!("{} cannot be empty", error_context)));
    }
    Ok(())
}

/// 验证字符串长度
///
/// # Arguments
///
/// * `input` - 待验证的字符串
/// * `max_length` - 最大允许长度
/// * `error_context` - 错误上下文描述
///
/// # Returns
///
/// * `Ok(())` - 验证通过
/// * `Err(OxCacheError)` - 字符串过长
pub fn validate_max_length(input: &str, max_length: usize, error_context: &str) -> crate::OxCacheResult<()> {
    if input.len() > max_length {
        return Err(OxCacheError::InvalidInput(format!(
            "{} exceeds maximum length of {} (got {})",
            error_context,
            max_length,
            input.len()
        )));
    }
    Ok(())
}

/// Redis 键验证常量
pub mod redis {
    /// Redis 键的最大长度（Redis 协议限制）
    pub const MAX_KEY_LENGTH: usize = 512 * 1024;

    /// Redis 键中的危险字符（协议安全：CR, LF, NULL）
    pub const DANGEROUS_CHARS: [char; 3] = ['\r', '\n', '\0'];
}

pub use redis::{DANGEROUS_CHARS, MAX_KEY_LENGTH};

/// Lua 脚本验证常量
pub mod lua_script {
    /// Lua 脚本的最大长度
    pub const MAX_SCRIPT_LENGTH: usize = 10 * 1024;

    /// Lua 脚本中的危险模式（参考清单）。
    ///
    /// 注意：此常量仅供文档/参考用途，**不直接参与运行时验证**。
    /// 实际的 Lua 脚本危险命令检测由 `security_impl::validate_lua_script`
    /// 通过 `forbidden_patterns` 列表执行（包含预处理和大小写不敏感匹配）。
    #[allow(dead_code)]
    pub const DANGEROUS_PATTERNS: &[&str] = &[
        "FLUSHALL", "FLUSHDB", "KEYS", "SHUTDOWN", "DEBUG", "CONFIG", "SAVE", "BGSAVE", "MONITOR",
    ];

    /// 验证 Lua 脚本长度
    ///
    /// # Arguments
    ///
    /// * `script` - Lua 脚本内容
    ///
    /// # Returns
    ///
    /// * `Ok(())` - 长度有效
    /// * `Err(OxCacheError)` - 脚本过长
    #[allow(dead_code)]
    pub fn validate_length(script: &str) -> crate::OxCacheResult<()> {
        super::validate_max_length(script, MAX_SCRIPT_LENGTH, "Lua script")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_no_dangerous_chars_valid() {
        let dangerous_chars = ['\r', '\n', '\0'];
        let result = validate_no_dangerous_chars("safe_string", &dangerous_chars, "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_dangerous_chars_invalid() {
        let dangerous_chars = ['\r', '\n', '\0'];
        let result = validate_no_dangerous_chars("unsafe\nstring", &dangerous_chars, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_not_empty_valid() {
        let result = validate_not_empty("non_empty", "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_not_empty_invalid() {
        let result = validate_not_empty("", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_max_length_valid() {
        let result = validate_max_length("short", 100, "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_max_length_invalid() {
        let result = validate_max_length(&"a".repeat(101), 100, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_redis_validate_key_valid() {
        // redis::validate_key was removed; test the shared helpers directly
        let result = validate_not_empty("my_key", "Redis key");
        assert!(result.is_ok());
    }

    #[test]
    fn test_redis_validate_key_empty() {
        let result = validate_not_empty("", "Redis key");
        assert!(result.is_err());
    }

    #[test]
    fn test_redis_validate_key_dangerous_chars() {
        let result = validate_no_dangerous_chars("key\nwith\nnewlines", &redis::DANGEROUS_CHARS, "Redis key");
        assert!(result.is_err());
    }

    #[test]
    fn test_redis_validate_key_too_long() {
        let long_key = "a".repeat(redis::MAX_KEY_LENGTH + 1);
        let result = validate_max_length(&long_key, redis::MAX_KEY_LENGTH, "Redis key");
        assert!(result.is_err());
    }

    #[test]
    fn test_lua_validate_length_valid() {
        let script = "return 1";
        let result = lua_script::validate_length(script);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lua_validate_length_too_long() {
        let long_script = "a".repeat(lua_script::MAX_SCRIPT_LENGTH + 1);
        let result = lua_script::validate_length(&long_script);
        if result.is_ok() {
            panic!(
                "Expected error for script length {} > max {}, but got Ok",
                long_script.len(),
                lua_script::MAX_SCRIPT_LENGTH
            );
        }
    }
}
