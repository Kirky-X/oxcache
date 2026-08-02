// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 缓存键生成器工具类
//!
//! 提供标准化的缓存键生成、验证和管理功能：
//! - 基于模板的键生成
//! - 命名空间/前缀管理
//! - 键验证和规范化
//! - 长键的哈希指纹生成

use crate::error::OxCacheError;
use crate::utils::MAX_CACHE_KEY_LENGTH;

/// 默认命名空间
const DEFAULT_NAMESPACE: &str = "default";

/// 有效的键字符集
const VALID_KEY_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w',
    'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T',
    'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '_', '.', ':', '/', '@',
];

/// 缓存键生成器
///
/// 提供标准化的缓存键生成、验证和管理功能。
/// 支持模板化键生成、命名空间管理、键验证和哈希指纹。
///
/// # 示例
///
/// ```
/// use oxcache::KeyGenerator;
///
/// let generator = KeyGenerator::new()
///     .with_namespace("app:v1");
///
/// let key = generator.generate_full("user:{id}", &[("id", "123")]);
/// assert_eq!(key, "app:v1:user:123");
/// ```
#[derive(Clone, Debug)]
pub struct KeyGenerator {
    namespace: String,
    prefix: String,
    max_key_length: usize,
}

impl Default for KeyGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyGenerator {
    /// 创建新的键生成器实例
    ///
    /// 使用默认配置（无命名空间、无前缀、256字符最大长度）
    pub fn new() -> Self {
        Self {
            namespace: DEFAULT_NAMESPACE.to_string(),
            prefix: String::new(),
            max_key_length: MAX_CACHE_KEY_LENGTH,
        }
    }

    /// 创建带有应用前缀的键生成器
    ///
    /// # 示例
    ///
    /// ```
    /// use oxcache::KeyGenerator;
    ///
    /// let generator = KeyGenerator::with_prefix("session:");
    /// let key = generator.generate_full("user:{id}", &[("id", "123")]);
    /// assert_eq!(key, "session:user:123");
    /// ```
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            namespace: DEFAULT_NAMESPACE.to_string(),
            prefix: prefix.to_string(),
            max_key_length: MAX_CACHE_KEY_LENGTH,
        }
    }

    /// 设置命名空间
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.namespace = namespace.to_string();
        self
    }

    /// 设置前缀
    pub fn with_prefix_str(mut self, prefix: &str) -> Self {
        self.prefix = prefix.to_string();
        self
    }

    /// 设置最大键长度
    pub fn with_max_key_length(mut self, length: usize) -> Self {
        self.max_key_length = length;
        self
    }

    /// 生成缓存键
    ///
    /// 支持模板语法 {placeholder}，可以从参数中替换值
    ///
    /// # 示例
    ///
    /// ```
    /// use oxcache::KeyGenerator;
    ///
    /// let generator = KeyGenerator::new();
    /// let key = generator.generate("user:{id}", &[("id", "123")]);
    /// assert_eq!(key, "user:123");
    /// ```
    pub fn generate(&self, template: &str, params: &[(&str, &str)]) -> String {
        let mut result = template.to_string();
        for (key, value) in params {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// 生成带有命名空间和前缀的完整缓存键
    pub fn generate_full(&self, template: &str, params: &[(&str, &str)]) -> String {
        let key = self.generate(template, params);
        let prefixed = self.apply_prefix(&key);
        self.namespaced_key(&prefixed)
    }

    /// 应用前缀到键
    fn apply_prefix(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}{}", self.prefix, key)
        }
    }

    /// 验证键是否有效
    pub fn validate_key(&self, key: &str) -> Result<(), OxCacheError> {
        if key.is_empty() {
            return Err(OxCacheError::InvalidInput("Cache key cannot be empty".to_string()));
        }
        if key.len() > self.max_key_length {
            return Err(OxCacheError::InvalidInput(format!(
                "Cache key exceeds maximum length of {} characters",
                self.max_key_length
            )));
        }
        for c in key.chars() {
            if !VALID_KEY_CHARS.contains(&c) {
                return Err(OxCacheError::InvalidInput(format!(
                    "Cache key contains invalid character: '{}'",
                    c
                )));
            }
        }
        Ok(())
    }

    /// 生成带有命名空间的键
    pub fn namespaced_key(&self, key: &str) -> String {
        if self.namespace.is_empty() || self.namespace == DEFAULT_NAMESPACE {
            key.to_string()
        } else {
            format!("{}:{}", self.namespace, key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generator_new() {
        let key_gen = KeyGenerator::new();
        assert_eq!(key_gen.namespace, "default");
        assert_eq!(key_gen.prefix, "");
        assert_eq!(key_gen.max_key_length, 256);
    }

    #[test]
    fn test_key_generator_default() {
        let key_gen = KeyGenerator::default();
        assert_eq!(key_gen.namespace, "default");
    }

    #[test]
    fn test_key_generator_with_prefix() {
        let key_gen = KeyGenerator::with_prefix("session:");
        assert_eq!(key_gen.prefix, "session:");
        assert_eq!(key_gen.namespace, "default");
    }

    #[test]
    fn test_key_generator_with_namespace() {
        let key_gen = KeyGenerator::new().with_namespace("myapp");
        assert_eq!(key_gen.namespace, "myapp");
    }

    #[test]
    fn test_key_generator_with_prefix_str() {
        let key_gen = KeyGenerator::new().with_prefix_str("v2:");
        assert_eq!(key_gen.prefix, "v2:");
    }

    #[test]
    fn test_key_generator_with_max_key_length() {
        let key_gen = KeyGenerator::new().with_max_key_length(512);
        assert_eq!(key_gen.max_key_length, 512);
    }

    #[test]
    fn test_key_generator_generate_basic() {
        let key_gen = KeyGenerator::new();
        let key = key_gen.generate("user:{id}", &[("id", "123")]);
        assert_eq!(key, "user:123");
    }

    #[test]
    fn test_key_generator_generate_multiple_params() {
        let key_gen = KeyGenerator::new();
        let key = key_gen.generate("search:{type}:{query}", &[("type", "products"), ("query", "laptop")]);
        assert_eq!(key, "search:products:laptop");
    }

    #[test]
    fn test_key_generator_generate_no_params() {
        let key_gen = KeyGenerator::new();
        let key = key_gen.generate("static:key", &[]);
        assert_eq!(key, "static:key");
    }

    #[test]
    fn test_key_generator_generate_unreplaced_placeholder() {
        let key_gen = KeyGenerator::new();
        let key = key_gen.generate("user:{id}", &[]);
        assert_eq!(key, "user:{id}");
    }

    #[test]
    fn test_key_generator_generate_full_with_namespace() {
        let key_gen = KeyGenerator::new().with_namespace("app");
        let key = key_gen.generate_full("user:{id}", &[("id", "42")]);
        assert_eq!(key, "app:user:42");
    }

    #[test]
    fn test_key_generator_generate_full_with_prefix() {
        let key_gen = KeyGenerator::with_prefix("cache:").with_namespace("app");
        let key = key_gen.generate_full("user:{id}", &[("id", "1")]);
        assert_eq!(key, "app:cache:user:1");
    }

    #[test]
    fn test_key_generator_generate_full_default_namespace() {
        // Default namespace "default" should not be prefixed
        let key_gen = KeyGenerator::new();
        let key = key_gen.generate_full("user:{id}", &[("id", "1")]);
        assert_eq!(key, "user:1");
    }

    #[test]
    fn test_key_generator_namespaced_key_with_namespace() {
        let key_gen = KeyGenerator::new().with_namespace("myapp");
        assert_eq!(key_gen.namespaced_key("user:1"), "myapp:user:1");
    }

    #[test]
    fn test_key_generator_namespaced_key_default_namespace() {
        let key_gen = KeyGenerator::new();
        assert_eq!(key_gen.namespaced_key("user:1"), "user:1");
    }

    #[test]
    fn test_key_generator_namespaced_key_empty_namespace() {
        let key_gen = KeyGenerator::new().with_namespace("");
        assert_eq!(key_gen.namespaced_key("user:1"), "user:1");
    }

    #[test]
    fn test_key_generator_validate_key_valid() {
        let key_gen = KeyGenerator::new();
        assert!(key_gen.validate_key("user:123").is_ok());
        assert!(key_gen.validate_key("cache/item").is_ok());
        assert!(key_gen.validate_key("session@abc").is_ok());
        assert!(key_gen.validate_key("a.b-c_d").is_ok());
    }

    #[test]
    fn test_key_generator_validate_key_empty() {
        let key_gen = KeyGenerator::new();
        let result = key_gen.validate_key("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            crate::error::OxCacheError::InvalidInput(msg) => assert!(msg.contains("cannot be empty")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_key_generator_validate_key_too_long() {
        let key_gen = KeyGenerator::new().with_max_key_length(10);
        let result = key_gen.validate_key("this_key_is_way_too_long");
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            crate::error::OxCacheError::InvalidInput(msg) => assert!(msg.contains("maximum length")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_key_generator_validate_key_invalid_chars() {
        let key_gen = KeyGenerator::new();
        // Space is not in VALID_KEY_CHARS
        let result = key_gen.validate_key("key with spaces");
        assert!(result.is_err());
        // Null byte
        let result = key_gen.validate_key("key\0null");
        assert!(result.is_err());
        // Newline
        let result = key_gen.validate_key("key\nnewline");
        assert!(result.is_err());
    }

    #[test]
    fn test_key_generator_apply_prefix_empty() {
        let key_gen = KeyGenerator::new();
        assert_eq!(key_gen.apply_prefix("key"), "key");
    }

    #[test]
    fn test_key_generator_apply_prefix_nonempty() {
        let key_gen = KeyGenerator::with_prefix("cache:");
        assert_eq!(key_gen.apply_prefix("key"), "cache:key");
    }
}
