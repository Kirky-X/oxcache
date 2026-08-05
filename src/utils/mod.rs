// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 工具模块
//!
//! 提供缓存键生成器等工具函数。

pub mod key_generator;
pub use key_generator::KeyGenerator;

mod utils_impl;

pub use utils_impl::MAX_CACHE_KEY_LENGTH;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_cache_key_length_value() {
        assert_eq!(MAX_CACHE_KEY_LENGTH, 256);
    }
}
