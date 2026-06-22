//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
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
