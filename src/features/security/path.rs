//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 路径遍历检测

use crate::error::{CacheError, Result};

/// 检查路径遍历模式
pub(crate) fn check_path_traversal(key: &str) -> Result<()> {
    const PATH_TRAVERSAL_PATTERNS: &[&str] = &[
        "../",
        "..\\",
        "%2e%2e",
        "%252e%252e",
        "..%2f",
        "..%5c",
        "%2e%2e%2f",
        "%2e%2e%5c",
    ];

    for pattern in PATH_TRAVERSAL_PATTERNS {
        if key.to_lowercase().contains(&pattern.to_lowercase()) {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains path traversal pattern: {}",
                pattern
            )));
        }
    }
    Ok(())
}
