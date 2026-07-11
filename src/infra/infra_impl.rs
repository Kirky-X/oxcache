// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Infrastructure impl - functions extracted from mod.rs

use super::*;
use crate::error::OxCacheError;

/// Validate cache key format
pub fn validate_cache_key(key: &str) -> Result<(), OxCacheError> {
    crate::utils::key_generator::KeyGenerator::new().validate_key(key)
}
