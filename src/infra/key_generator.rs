//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 缓存键生成器工具类
//!
//! 提供标准化的缓存键生成、验证和管理功能：
//! - 基于模板的键生成
//! - 命名空间/前缀管理
//! - 键验证和规范化
//! - 长键的哈希指纹生成

use crate::error::CacheError;
#[cfg(feature = "moka")]
use moka::policy::EvictionPolicy;
#[cfg(feature = "bloom-filter")]
use murmur3::murmur3_32;

/// 默认键最大长度
const DEFAULT_MAX_KEY_LENGTH: usize = 256;

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
            max_key_length: DEFAULT_MAX_KEY_LENGTH,
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
            max_key_length: DEFAULT_MAX_KEY_LENGTH,
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

    /// 设置淘汰策略
    #[cfg(feature = "moka")]
    pub fn with_eviction_policy(self, _policy: EvictionPolicy) -> Self {
        // 暂时忽略淘汰策略，用于接口兼容性
        self
    }

    #[cfg(not(feature = "moka"))]
    pub fn with_eviction_policy(self, _policy: ()) -> Self {
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
    pub fn validate_key(&self, key: &str) -> Result<(), CacheError> {
        if key.is_empty() {
            return Err(CacheError::InvalidInput("Cache key cannot be empty".to_string()));
        }
        if key.len() > self.max_key_length {
            return Err(CacheError::InvalidInput(format!(
                "Cache key exceeds maximum length of {} characters",
                self.max_key_length
            )));
        }
        for c in key.chars() {
            if !VALID_KEY_CHARS.contains(&c) {
                return Err(CacheError::InvalidInput(format!(
                    "Cache key contains invalid character: '{}'",
                    c
                )));
            }
        }
        Ok(())
    }

    /// 生成哈希指纹（用于长键）
    #[cfg(feature = "bloom-filter")]
    pub fn generate_fingerprint(&self, key: &str) -> String {
        let key_bytes = key.as_bytes();
        let hash = murmur3_32(&mut &key_bytes[..], 0).unwrap_or(0);
        format!("_fp{:08x}", hash)
    }

    /// 生成规范化的键（自动处理长键和特殊字符）
    #[cfg(feature = "bloom-filter")]
    pub fn normalize(&self, key: &str) -> String {
        let key = key.trim().to_string();
        if key.len() <= self.max_key_length {
            key
        } else {
            let fingerprint = self.generate_fingerprint(&key);
            let max_key_length = self.max_key_length.saturating_sub(fingerprint.len());
            let truncated = &key[..max_key_length.max(1)];
            format!("{}{}", truncated, fingerprint)
        }
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

    // ========================================================================
    // Default / Constructor tests
    // ========================================================================

    #[test]
    fn test_new_uses_defaults() {
        let gen = KeyGenerator::new();
        assert_eq!(gen.namespace, DEFAULT_NAMESPACE);
        assert!(gen.prefix.is_empty());
        assert_eq!(gen.max_key_length, DEFAULT_MAX_KEY_LENGTH);
    }

    #[test]
    fn test_default_trait() {
        let gen = KeyGenerator::default();
        assert_eq!(gen.namespace, DEFAULT_NAMESPACE);
        assert!(gen.prefix.is_empty());
    }

    #[test]
    fn test_with_prefix_constructor() {
        let gen = KeyGenerator::with_prefix("session:");
        assert_eq!(gen.prefix, "session:");
        assert_eq!(gen.namespace, DEFAULT_NAMESPACE);
        assert_eq!(gen.max_key_length, DEFAULT_MAX_KEY_LENGTH);
    }

    // ========================================================================
    // Builder pattern tests
    // ========================================================================

    #[test]
    fn test_with_namespace() {
        let gen = KeyGenerator::new().with_namespace("app:v1");
        assert_eq!(gen.namespace, "app:v1");
    }

    #[test]
    fn test_with_prefix_str() {
        let gen = KeyGenerator::new().with_prefix_str("cache:");
        assert_eq!(gen.prefix, "cache:");
    }

    #[test]
    fn test_with_max_key_length() {
        let gen = KeyGenerator::new().with_max_key_length(512);
        assert_eq!(gen.max_key_length, 512);
    }

    #[test]
    fn test_builder_chaining() {
        let gen = KeyGenerator::new()
            .with_namespace("app:v2")
            .with_prefix_str("usr:")
            .with_max_key_length(128);
        assert_eq!(gen.namespace, "app:v2");
        assert_eq!(gen.prefix, "usr:");
        assert_eq!(gen.max_key_length, 128);
    }

    #[test]
    fn test_with_eviction_policy() {
        #[cfg(feature = "moka")]
        {
            let gen = KeyGenerator::new().with_eviction_policy(EvictionPolicy::lru());
            assert_eq!(gen.max_key_length, DEFAULT_MAX_KEY_LENGTH);
        }
        #[cfg(not(feature = "moka"))]
        {
            let gen = KeyGenerator::new().with_eviction_policy(());
            assert_eq!(gen.max_key_length, DEFAULT_MAX_KEY_LENGTH);
        }
    }

    // ========================================================================
    // generate() tests
    // ========================================================================

    #[test]
    fn test_generate_single_param() {
        let gen = KeyGenerator::new();
        let key = gen.generate("user:{id}", &[("id", "123")]);
        assert_eq!(key, "user:123");
    }

    #[test]
    fn test_generate_multiple_params() {
        let gen = KeyGenerator::new();
        let key = gen.generate("user:{id}:profile:{section}", &[("id", "42"), ("section", "prefs")]);
        assert_eq!(key, "user:42:profile:prefs");
    }

    #[test]
    fn test_generate_no_params() {
        let gen = KeyGenerator::new();
        let key = gen.generate("static-key", &[]);
        assert_eq!(key, "static-key");
    }

    #[test]
    fn test_generate_no_placeholders() {
        let gen = KeyGenerator::new();
        let key = gen.generate("literal-key", &[("id", "123")]);
        assert_eq!(key, "literal-key");
    }

    #[test]
    fn test_generate_empty_template() {
        let gen = KeyGenerator::new();
        let key = gen.generate("", &[("id", "123")]);
        assert_eq!(key, "");
    }

    #[test]
    fn test_generate_missing_placeholder() {
        let gen = KeyGenerator::new();
        let key = gen.generate("user:{id}", &[]);
        assert_eq!(key, "user:{id}");
    }

    #[test]
    fn test_generate_repeated_placeholder() {
        let gen = KeyGenerator::new();
        let key = gen.generate("{x}:{x}", &[("x", "val")]);
        assert_eq!(key, "val:val");
    }

    // ========================================================================
    // generate_full() tests
    // ========================================================================

    #[test]
    fn test_generate_full_default_namespace() {
        let gen = KeyGenerator::new();
        let key = gen.generate_full("user:{id}", &[("id", "123")]);
        assert_eq!(key, "user:123");
    }

    #[test]
    fn test_generate_full_custom_namespace() {
        let gen = KeyGenerator::new().with_namespace("app:v1");
        let key = gen.generate_full("user:{id}", &[("id", "123")]);
        assert_eq!(key, "app:v1:user:123");
    }

    #[test]
    fn test_generate_full_with_prefix() {
        let gen = KeyGenerator::with_prefix("session:");
        let key = gen.generate_full("user:{id}", &[("id", "123")]);
        assert_eq!(key, "session:user:123");
    }

    #[test]
    fn test_generate_full_namespace_and_prefix() {
        let gen = KeyGenerator::new().with_namespace("app:v1").with_prefix_str("cache:");
        let key = gen.generate_full("user:{id}", &[("id", "456")]);
        assert_eq!(key, "app:v1:cache:user:456");
    }

    // ========================================================================
    // apply_prefix() tests (private method, tested through generate_full)
    // ========================================================================

    #[test]
    fn test_apply_prefix_empty() {
        let gen = KeyGenerator::new();
        let key = gen.generate_full("mykey", &[]);
        assert_eq!(key, "mykey");
    }

    #[test]
    fn test_apply_prefix_non_empty() {
        let gen = KeyGenerator::with_prefix("pre:");
        let key = gen.generate_full("mykey", &[]);
        assert_eq!(key, "pre:mykey");
    }

    // ========================================================================
    // validate_key() tests
    // ========================================================================

    #[test]
    fn test_validate_key_valid() {
        let gen = KeyGenerator::new();
        assert!(gen.validate_key("valid-key_123.test:456@path").is_ok());
    }

    #[test]
    fn test_validate_key_empty() {
        let gen = KeyGenerator::new();
        let result = gen.validate_key("");
        assert!(result.is_err());
        match result.unwrap_err() {
            CacheError::InvalidInput(msg) => assert!(msg.contains("empty")),
            other => panic!("Expected InvalidInput, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_key_too_long() {
        let gen = KeyGenerator::new().with_max_key_length(10);
        let result = gen.validate_key("this-is-way-too-long");
        assert!(result.is_err());
        match result.unwrap_err() {
            CacheError::InvalidInput(msg) => assert!(msg.contains("exceeds maximum length")),
            other => panic!("Expected InvalidInput, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_key_invalid_char_space() {
        let gen = KeyGenerator::new();
        assert!(gen.validate_key("has space").is_err());
    }

    #[test]
    fn test_validate_key_invalid_char_hash() {
        let gen = KeyGenerator::new();
        assert!(gen.validate_key("has#hash").is_err());
    }

    #[test]
    fn test_validate_key_invalid_char_percent() {
        let gen = KeyGenerator::new();
        assert!(gen.validate_key("100%").is_err());
    }

    #[test]
    fn test_validate_key_boundary_length() {
        let gen = KeyGenerator::new().with_max_key_length(5);
        assert!(gen.validate_key("abcde").is_ok());
        assert!(gen.validate_key("abcdef").is_err());
    }

    #[test]
    fn test_validate_key_all_valid_char_categories() {
        let gen = KeyGenerator::new();
        assert!(gen.validate_key("abcdefghijklmnopqrstuvwxyz").is_ok());
        assert!(gen.validate_key("ABCDEFGHIJKLMNOPQRSTUVWXYZ").is_ok());
        assert!(gen.validate_key("0123456789").is_ok());
        assert!(gen.validate_key("a-b_c.d:e/f@g").is_ok());
    }

    // ========================================================================
    // namespaced_key() tests
    // ========================================================================

    #[test]
    fn test_namespaced_key_custom_namespace() {
        let gen = KeyGenerator::new().with_namespace("myapp");
        assert_eq!(gen.namespaced_key("user:1"), "myapp:user:1");
    }

    #[test]
    fn test_namespaced_key_default_namespace_omitted() {
        let gen = KeyGenerator::new();
        assert_eq!(gen.namespaced_key("user:1"), "user:1");
    }

    #[test]
    fn test_namespaced_key_empty_namespace() {
        let gen = KeyGenerator::new().with_namespace("");
        assert_eq!(gen.namespaced_key("user:1"), "user:1");
    }

    // ========================================================================
    // generate_fingerprint() tests (bloom-filter feature)
    // ========================================================================

    #[test]
    #[cfg(feature = "bloom-filter")]
    fn test_generate_fingerprint_format() {
        let gen = KeyGenerator::new();
        let fp = gen.generate_fingerprint("test-key");
        assert!(fp.starts_with("_fp"));
        assert_eq!(fp.len(), 11);
    }

    #[test]
    #[cfg(feature = "bloom-filter")]
    fn test_generate_fingerprint_deterministic() {
        let gen = KeyGenerator::new();
        let fp1 = gen.generate_fingerprint("same-key");
        let fp2 = gen.generate_fingerprint("same-key");
        assert_eq!(fp1, fp2);
    }

    #[test]
    #[cfg(feature = "bloom-filter")]
    fn test_generate_fingerprint_different_keys() {
        let gen = KeyGenerator::new();
        let fp1 = gen.generate_fingerprint("key-a");
        let fp2 = gen.generate_fingerprint("key-b");
        assert_ne!(fp1, fp2);
    }

    // ========================================================================
    // normalize() tests (bloom-filter feature)
    // ========================================================================

    #[test]
    #[cfg(feature = "bloom-filter")]
    fn test_normalize_short_key() {
        let gen = KeyGenerator::new();
        let result = gen.normalize("short-key");
        assert_eq!(result, "short-key");
    }

    #[test]
    #[cfg(feature = "bloom-filter")]
    fn test_normalize_trims_whitespace() {
        let gen = KeyGenerator::new();
        let result = gen.normalize("  padded  ");
        assert_eq!(result, "padded");
    }

    #[test]
    #[cfg(feature = "bloom-filter")]
    fn test_normalize_long_key_truncated_with_fingerprint() {
        let gen = KeyGenerator::new().with_max_key_length(50);
        let long_key = "a".repeat(200);
        let result = gen.normalize(&long_key);
        assert!(result.len() <= 50);
        assert!(result.contains("_fp"));
    }

    #[test]
    #[cfg(feature = "bloom-filter")]
    fn test_normalize_boundary_exact_length() {
        let gen = KeyGenerator::new().with_max_key_length(20);
        let exact_key = "b".repeat(20);
        let result = gen.normalize(&exact_key);
        assert_eq!(result, exact_key);
    }

    // ========================================================================
    // Clone / Debug tests
    // ========================================================================

    #[test]
    fn test_clone() {
        let gen = KeyGenerator::new().with_namespace("clone-test");
        let cloned = gen.clone();
        assert_eq!(gen.namespace, cloned.namespace);
        assert_eq!(gen.prefix, cloned.prefix);
        assert_eq!(gen.max_key_length, cloned.max_key_length);
    }

    #[test]
    fn test_debug_format() {
        let gen = KeyGenerator::new();
        let debug = format!("{:?}", gen);
        assert!(debug.contains("KeyGenerator"));
    }
}
