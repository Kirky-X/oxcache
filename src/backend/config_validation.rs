// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Configuration validation constants and utilities

use crate::error::{OxCacheError, OxCacheResult};

/// 配置验证常量
#[derive(Debug, Clone, Copy)]
pub struct ConfigValidation;

impl ConfigValidation {
    /// 最大缓存容量（10亿条目）
    #[allow(dead_code)] // Public API constant — reserved for external validation consumers
    pub const MAX_CAPACITY: u64 = 1_000_000_000;
    /// 最大 TTL（30天，以秒计）
    #[allow(dead_code)] // Public API constant — mirrors core::constants::MAX_TTL_SECS for config validation
    pub const MAX_TTL_SECS: u64 = 30 * 24 * 60 * 60;
    /// 最大 TTI（30天，以秒计）
    #[allow(dead_code)] // Public API constant — mirrors core::constants value for config validation
    pub const MAX_TTI_SECS: u64 = 30 * 24 * 60 * 60;
    /// 自定义名称最大长度（256字符）
    pub const MAX_CUSTOM_NAME_LENGTH: usize = 256;
    /// 允许的自定义名称字符（字母、数字、下划线、连字符、点）
    pub const VALID_NAME_CHARS: &'static str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-";

    /// 检测连接的对端是否为 Valkey 服务器。
    ///
    /// 通过 `INFO server` 命令检查返回值中是否包含 "valkey" 标识。
    ///
    /// # Returns
    /// - `Ok(true)` — 对端为 Valkey 服务器
    /// - `Ok(false)` — 对端为 Redis 服务器
    /// - `Err(OxCacheError::Connection)` — 连接失败或命令执行失败
    #[cfg(feature = "redis")]
    pub fn detect_valkey(conn: &mut redis::Connection) -> OxCacheResult<bool> {
        let info: String = redis::cmd("INFO")
            .arg("server")
            .query(conn)
            .map_err(|e| OxCacheError::Connection(format!("Failed to query INFO server: {}", e)))?;
        // Valkey INFO server 输出中包含 "valkey" 标识
        // 例如: "redis_version:7.2.4\nvalkey_version:7.2.4\n..."
        Ok(info.to_ascii_lowercase().contains("valkey"))
    }

    /// O(1) 字符有效性检查：所有合法字符皆为 ASCII，直接用字节范围判断，
    /// 避免 `VALID_NAME_CHARS.contains(ch)` 的 O(n) 线性扫描。
    #[inline]
    fn is_valid_name_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')
    }

    /// 验证自定义名称
    pub fn validate_custom_name(name: &str) -> OxCacheResult<String> {
        if name.is_empty() {
            return Err(OxCacheError::InvalidInput(
                "Custom backend name cannot be empty".to_string(),
            ));
        }
        if name.chars().count() > Self::MAX_CUSTOM_NAME_LENGTH {
            return Err(OxCacheError::InvalidInput(format!(
                "Custom backend name exceeds maximum length of {} characters",
                Self::MAX_CUSTOM_NAME_LENGTH
            )));
        }

        for ch in name.chars() {
            if !Self::is_valid_name_char(ch) {
                return Err(OxCacheError::InvalidInput(format!(
                    "Custom backend name contains invalid character '{}'. Allowed characters: {}",
                    ch,
                    Self::VALID_NAME_CHARS
                )));
            }
        }

        Ok(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_custom_name_valid() {
        assert_eq!(
            ConfigValidation::validate_custom_name("my_backend").unwrap(),
            "my_backend"
        );
        assert_eq!(
            ConfigValidation::validate_custom_name("backend-1").unwrap(),
            "backend-1"
        );
        assert_eq!(ConfigValidation::validate_custom_name("app.v2").unwrap(), "app.v2");
        assert_eq!(ConfigValidation::validate_custom_name("ABC123").unwrap(), "ABC123");
        assert_eq!(ConfigValidation::validate_custom_name("a").unwrap(), "a");
    }

    #[test]
    fn test_validate_custom_name_empty() {
        let result = ConfigValidation::validate_custom_name("");
        assert!(result.is_err());
        match result.unwrap_err() {
            OxCacheError::InvalidInput(msg) => assert!(msg.contains("cannot be empty")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_custom_name_too_long() {
        let long_name = "a".repeat(ConfigValidation::MAX_CUSTOM_NAME_LENGTH + 1);
        let result = ConfigValidation::validate_custom_name(&long_name);
        assert!(result.is_err());
        match result.unwrap_err() {
            OxCacheError::InvalidInput(msg) => assert!(msg.contains("maximum length")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_custom_name_exactly_max_length() {
        let name = "a".repeat(ConfigValidation::MAX_CUSTOM_NAME_LENGTH);
        assert!(ConfigValidation::validate_custom_name(&name).is_ok());
    }

    #[test]
    fn test_validate_custom_name_invalid_chars() {
        // Space
        assert!(ConfigValidation::validate_custom_name("name with space").is_err());
        // Colon
        assert!(ConfigValidation::validate_custom_name("name:colon").is_err());
        // Slash
        assert!(ConfigValidation::validate_custom_name("name/slash").is_err());
        // At sign
        assert!(ConfigValidation::validate_custom_name("name@at").is_err());
        // Special chars
        assert!(ConfigValidation::validate_custom_name("name#hash").is_err());
        assert!(ConfigValidation::validate_custom_name("name$dollar").is_err());
    }

    #[test]
    fn test_validate_custom_name_all_valid_chars() {
        // All chars in VALID_NAME_CHARS should be accepted
        let name: String = ConfigValidation::VALID_NAME_CHARS.chars().collect();
        assert!(ConfigValidation::validate_custom_name(&name).is_ok());
    }

    #[test]
    fn test_config_validation_constants() {
        assert_eq!(ConfigValidation::MAX_CUSTOM_NAME_LENGTH, 256);
        // Verify VALID_NAME_CHARS contains expected characters
        assert!(ConfigValidation::VALID_NAME_CHARS.contains('a'));
        assert!(ConfigValidation::VALID_NAME_CHARS.contains('Z'));
        assert!(ConfigValidation::VALID_NAME_CHARS.contains('0'));
        assert!(ConfigValidation::VALID_NAME_CHARS.contains('_'));
        assert!(ConfigValidation::VALID_NAME_CHARS.contains('-'));
        assert!(ConfigValidation::VALID_NAME_CHARS.contains('.'));
        // Should not contain invalid chars
        assert!(!ConfigValidation::VALID_NAME_CHARS.contains(':'));
        assert!(!ConfigValidation::VALID_NAME_CHARS.contains('/'));
        assert!(!ConfigValidation::VALID_NAME_CHARS.contains(' '));
    }

    #[test]
    fn test_validate_custom_name_multibyte_unicode() {
        // L6 修复验证：长度检查应基于字符数而非字节数
        // 多字节 UTF-8 字符（如 emoji）不在 VALID_NAME_CHARS 中，会被字符检查拒绝
        // 但长度检查本身不应因字节数而误判
        let name_with_dots = "a.b.c";
        assert_eq!(name_with_dots.chars().count(), 5);
        assert_eq!(name_with_dots.len(), 5);
        assert!(ConfigValidation::validate_custom_name(name_with_dots).is_ok());

        // 验证 256 字符的纯 ASCII 名称可以通过（边界测试）
        let max_name = "a".repeat(256);
        assert!(ConfigValidation::validate_custom_name(&max_name).is_ok());

        // 验证 257 字符被拒绝
        let over_name = "a".repeat(257);
        assert!(ConfigValidation::validate_custom_name(&over_name).is_err());
    }
}
