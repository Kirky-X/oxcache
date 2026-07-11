// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Configuration validation constants and utilities

use crate::error::{CacheError, Result};

/// 配置验证常量
#[derive(Debug, Clone, Copy)]
pub struct ConfigValidation;

impl ConfigValidation {
    /// 最大缓存容量（10亿条目）
    pub const MAX_CAPACITY: u64 = 1_000_000_000;
    /// 最大 TTL（30天，以秒计）
    pub const MAX_TTL_SECS: u64 = 30 * 24 * 60 * 60;
    /// 最大 TTI（30天，以秒计）
    pub const MAX_TTI_SECS: u64 = 30 * 24 * 60 * 60;
    /// 自定义名称最大长度（256字符）
    pub const MAX_CUSTOM_NAME_LENGTH: usize = 256;
    /// 允许的自定义名称字符（字母、数字、下划线、连字符、点）
    pub const VALID_NAME_CHARS: &'static str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-";

    /// 验证自定义名称
    pub fn validate_custom_name(name: &str) -> Result<String> {
        if name.is_empty() {
            return Err(CacheError::InvalidInput(
                "Custom backend name cannot be empty".to_string(),
            ));
        }
        if name.len() > Self::MAX_CUSTOM_NAME_LENGTH {
            return Err(CacheError::InvalidInput(format!(
                "Custom backend name exceeds maximum length of {} characters",
                Self::MAX_CUSTOM_NAME_LENGTH
            )));
        }

        for ch in name.chars() {
            if !Self::VALID_NAME_CHARS.contains(ch) {
                return Err(CacheError::InvalidInput(format!(
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
            CacheError::InvalidInput(msg) => assert!(msg.contains("cannot be empty")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_custom_name_too_long() {
        let long_name = "a".repeat(ConfigValidation::MAX_CUSTOM_NAME_LENGTH + 1);
        let result = ConfigValidation::validate_custom_name(&long_name);
        assert!(result.is_err());
        match result.unwrap_err() {
            CacheError::InvalidInput(msg) => assert!(msg.contains("maximum length")),
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
}
