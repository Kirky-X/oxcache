//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Infrastructure module
//!
//! Provides infrastructure components: metrics, serialization, telemetry, warmup, db_loader, cli

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "serialization")]
pub mod serialization;

use crate::error::CacheError;

/// Validate cache key format
pub fn validate_cache_key(key: &str) -> Result<(), CacheError> {
    crate::utils::key_generator::KeyGenerator::new().validate_key(key)
}

#[cfg(feature = "metrics")]
pub use metrics::{export_json_format, export_prometheus_format, get_enhanced_stats, CacheStats};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cache_key_valid() {
        // 测试有效的缓存键
        assert!(validate_cache_key("user:123").is_ok());
        assert!(validate_cache_key("cache_key").is_ok());
        assert!(validate_cache_key("test-key-123").is_ok());
        assert!(validate_cache_key("a").is_ok());
    }

    #[test]
    fn test_validate_cache_key_empty() {
        // 空键应该返回错误
        let result = validate_cache_key("");
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_cache_key_with_invalid_chars() {
        // 包含无效字符的键应该返回错误
        let result = validate_cache_key("key with spaces");
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_cache_key_with_special_chars() {
        // 包含无效字符的键应该返回错误（@ 是有效字符）
        assert!(validate_cache_key("key#hash").is_err());
        assert!(validate_cache_key("key$dollar").is_err());
        assert!(validate_cache_key("key%percent").is_err());
        assert!(validate_cache_key("key!exclaim").is_err());
    }

    #[test]
    fn test_validate_cache_key_with_valid_chars() {
        // 包含有效字符的键应该通过
        assert!(validate_cache_key("key:123").is_ok());
        assert!(validate_cache_key("key-123").is_ok());
        assert!(validate_cache_key("key_123").is_ok());
        assert!(validate_cache_key("Key123").is_ok());
        assert!(validate_cache_key("KEY123").is_ok());
        assert!(validate_cache_key("12345").is_ok());
    }

    #[test]
    fn test_validate_cache_key_too_long() {
        // 过长的键应该返回错误
        let long_key = "a".repeat(10000);
        let result = validate_cache_key(&long_key);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_cache_key_with_dot() {
        // 测试包含点号的键
        assert!(validate_cache_key("user.profile").is_ok());
    }

    #[test]
    fn test_validate_cache_key_with_slash() {
        // 测试包含斜杠的键
        assert!(validate_cache_key("user/123").is_ok());
    }
}
