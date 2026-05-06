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

    /// 验证容量值
    pub fn validate_capacity(capacity: u64) -> Result<u64> {
        if capacity == 0 {
            return Err(CacheError::InvalidInput("Capacity must be greater than 0".to_string()));
        }
        if capacity > Self::MAX_CAPACITY {
            return Err(CacheError::InvalidInput(format!(
                "Capacity {} exceeds maximum allowed value of {}",
                capacity,
                Self::MAX_CAPACITY
            )));
        }
        Ok(capacity)
    }

    /// 验证 TTL 值
    pub fn validate_ttl(ttl: u64) -> Result<u64> {
        if ttl == 0 {
            return Err(CacheError::InvalidInput("TTL must be greater than 0".to_string()));
        }
        if ttl > Self::MAX_TTL_SECS {
            return Err(CacheError::InvalidInput(format!(
                "TTL {} seconds exceeds maximum allowed value of {} seconds (30 days)",
                ttl,
                Self::MAX_TTL_SECS
            )));
        }
        Ok(ttl)
    }

    /// 验证 TTI 值
    pub fn validate_tti(tti: u64) -> Result<u64> {
        if tti > Self::MAX_TTI_SECS {
            return Err(CacheError::InvalidInput(format!(
                "Time to idle {} seconds exceeds maximum allowed value of {} seconds (30 days)",
                tti,
                Self::MAX_TTI_SECS
            )));
        }
        Ok(tti)
    }

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
