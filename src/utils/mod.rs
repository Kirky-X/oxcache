// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 工具模块
//!
//! 提供缓存键生成器等工具函数。

pub mod key_generator;
pub use key_generator::KeyGenerator;

mod utils_impl;

#[cfg(test)]
use utils_impl::{MAX_CACHE_KEY_LENGTH, VALID_KEY_CHARS};

#[allow(unused_imports)]
pub use utils_impl::validate_cache_key;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::OxCacheError;

    #[test]
    fn test_validate_cache_key_valid() {
        assert!(validate_cache_key("user:123").is_ok());
        assert!(validate_cache_key("cache/item").is_ok());
        assert!(validate_cache_key("session@abc").is_ok());
        assert!(validate_cache_key("a.b-c_d").is_ok());
        assert!(validate_cache_key("namespace:key:path").is_ok());
    }

    #[test]
    fn test_validate_cache_key_empty() {
        let result = validate_cache_key("");
        assert!(result.is_err());
        match result.unwrap_err() {
            OxCacheError::InvalidInput(msg) => assert!(msg.contains("cannot be empty")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_cache_key_too_long() {
        // MAX_CACHE_KEY_LENGTH is 1024
        let long_key = "a".repeat(MAX_CACHE_KEY_LENGTH + 1);
        let result = validate_cache_key(&long_key);
        assert!(result.is_err());
        match result.unwrap_err() {
            OxCacheError::InvalidInput(msg) => assert!(msg.contains("maximum length")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_cache_key_exactly_max_length() {
        let key = "a".repeat(MAX_CACHE_KEY_LENGTH);
        assert!(validate_cache_key(&key).is_ok());
    }

    #[test]
    fn test_validate_cache_key_invalid_chars() {
        // Space
        assert!(validate_cache_key("key with space").is_err());
        // Null byte
        assert!(validate_cache_key("key\0null").is_err());
        // Newline
        assert!(validate_cache_key("key\nnewline").is_err());
        // Tab
        assert!(validate_cache_key("key\ttab").is_err());
        // Special chars
        assert!(validate_cache_key("key#hash").is_err());
        assert!(validate_cache_key("key$dollar").is_err());
        assert!(validate_cache_key("key%percent").is_err());
    }

    #[test]
    fn test_validate_cache_key_all_valid_chars() {
        // Test all characters in VALID_KEY_CHARS
        let key: String = VALID_KEY_CHARS.iter().collect();
        assert!(validate_cache_key(&key).is_ok());
    }
}
