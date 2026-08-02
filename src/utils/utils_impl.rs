// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Utils impl - functions extracted from mod.rs

use crate::error::OxCacheError;

pub const MAX_CACHE_KEY_LENGTH: usize = 256;
pub(super) const VALID_KEY_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w',
    'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T',
    'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '_', '.', ':', '/', '@',
];

#[allow(dead_code)]
pub fn validate_cache_key(key: &str) -> Result<(), OxCacheError> {
    if key.is_empty() {
        return Err(OxCacheError::InvalidInput("Cache key cannot be empty".to_string()));
    }

    if key.len() > MAX_CACHE_KEY_LENGTH {
        return Err(OxCacheError::InvalidInput(format!(
            "Cache key exceeds maximum length of {} bytes (got {} bytes)",
            MAX_CACHE_KEY_LENGTH,
            key.len()
        )));
    }

    for c in key.chars() {
        if !VALID_KEY_CHARS.contains(&c) {
            return Err(OxCacheError::InvalidInput(format!(
                "Cache key contains invalid character '{}'. Valid characters are: alphanumeric and -_.:/@",
                c
            )));
        }
    }

    Ok(())
}
