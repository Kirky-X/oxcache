// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Utils impl - functions extracted from mod.rs

use super::*;
use crate::error::OxCacheError;

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
