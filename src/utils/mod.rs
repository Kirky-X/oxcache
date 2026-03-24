//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 工具模块
//!
//! 提供缓存键生成器等工具函数。

pub(crate) mod key_generator;

use crate::error::CacheError;

const MAX_CACHE_KEY_LENGTH: usize = 1024;
const VALID_KEY_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w',
    'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T',
    'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '_', '.', ':', '/', '@',
];

pub fn validate_cache_key(key: &str) -> Result<(), CacheError> {
    if key.is_empty() {
        return Err(CacheError::InvalidInput("Cache key cannot be empty".to_string()));
    }

    if key.len() > MAX_CACHE_KEY_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "Cache key exceeds maximum length of {} bytes (got {} bytes)",
            MAX_CACHE_KEY_LENGTH,
            key.len()
        )));
    }

    for c in key.chars() {
        if !VALID_KEY_CHARS.contains(&c) {
            return Err(CacheError::InvalidInput(format!(
                "Cache key contains invalid character '{}'. Valid characters are: alphanumeric and -_.:/@",
                c
            )));
        }
    }

    Ok(())
}
