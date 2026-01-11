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
use murmur3::murmur3_32;
use regex::Regex;
use std::sync::Arc;

/// 默认键最大长度
const DEFAULT_MAX_KEY_LENGTH: usize = 256;

/// 默认命名空间
const DEFAULT_NAMESPACE: &str = "default";

/// 有效的键字符集
const VALID_KEY_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '-', '_', '.', ':', '/', '@',
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
/// let key = generator.generate("user:{id}", &[("id", "123")]);
/// assert_eq!(key, "app:v1:user:123");
/// ```
#[derive(Clone, Debug)]
pub struct KeyGenerator {
    namespace: String,
    prefix: String,
    max_key_length: usize,
    template_regex: Regex,
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
            template_regex: Regex::new(r"\{(\w+)\}").expect("Failed to compile template regex"),
        }
    }

    /// 创建带有应用前缀的键生成器
    ///
    /// # 示例
    ///
    /// ```
    /// use oxcache::KeyGenerator;
    ///
    /// let generator = KeyGenerator::with_prefix("myapp");
    /// let key = generator.generate("user:{id}", &[("id", "123")]);
    /// assert_eq!(key, "myapp:user:123");
    /// ```
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            ..Self::new()
        }
    }

    /// 设置命名空间
    ///
    /// 命名空间会自动添加到所有生成的键前面，用于避免不同应用或服务之间的键冲突。
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.namespace = namespace.to_string();
        self
    }

    /// 设置前缀
    ///
    /// 前缀会添加到键的最前面（命名空间之前）。
    pub fn with_prefix_str(mut self, prefix: &str) -> Self {
        self.prefix = prefix.to_string();
        self
    }

    /// 设置最大键长度
    ///
    /// 当生成的键超过此长度时，会返回哈希指纹而非原始键。
    /// 默认为 256 字符。
    pub fn with_max_key_length(mut self, length: usize) -> Self {
        self.max_key_length = length;
        self
    }

    /// 基于模板生成缓存键
    ///
    /// 模板使用 `{placeholder}` 格式的占位符，占位符会被对应的值替换。
    ///
    /// # 参数
    ///
    /// * `template` - 键模板，如 `"user:{id}:profile"` 或 `"session:{session_id}"`
    /// * `args` - 键值对数组，用于替换模板中的占位符
    ///
    /// # 示例
    ///
    /// ```
    /// use oxcache::KeyGenerator;
    ///
    /// let generator = KeyGenerator::new();
    /// let key = generator.generate("user:{id}", &[("id", "123")]);
    /// assert_eq!(key, "user:123");
    ///
    /// let key = generator.generate("product:{category}:{id}", &[
    ///     ("category", "electronics"),
    ///     ("id", "456")
    /// ]);
    /// assert_eq!(key, "product:electronics:456");
    /// ```
    pub fn generate(&self, template: &str, args: &[(&str, &str)]) -> String {
        let mut result = template.to_string();

        for (key, value) in args {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }

        self.apply_namespace_and_prefix(&result)
    }

    /// 验证键的格式是否有效
    ///
    /// # 参数
    ///
    /// * `key` - 要验证的键
    ///
    /// # 返回
    ///
    /// * `Ok(())` - 键格式有效
    /// * `Err(CacheError)` - 键格式无效，包含错误描述
    pub fn validate(&self, key: &str) -> Result<(), CacheError> {
        if key.is_empty() {
            return Err(CacheError::InvalidInput(
                "Cache key cannot be empty".to_string(),
            ));
        }

        if key.len() > self.max_key_length {
            return Err(CacheError::InvalidInput(format!(
                "Cache key exceeds maximum length of {} bytes (got {} bytes)",
                self.max_key_length,
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

    /// 规范化键
    ///
    /// 规范化操作包括：
    /// - 转换为小写
    /// - 移除首尾空白字符
    /// - 压缩连续的空白字符为单个下划线
    ///
    /// # 示例
    ///
    /// ```
    /// use oxcache::KeyGenerator;
    ///
    /// let generator = KeyGenerator::new();
    /// let normalized = generator.normalize("  User:123  ");
    /// assert_eq!(normalized, "user:123");
    /// ```
    pub fn normalize(&self, key: &str) -> String {
        key.trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_")
    }

    /// 生成键的哈希指纹
    ///
    /// 对于过长的键，生成一个 32 位 murmur3 哈希作为指纹。
    /// 哈希值以 "hash:" 前缀返回，确保不会与普通键冲突。
    ///
    /// # 参数
    ///
    /// * `key` - 要生成哈希的键
    ///
    /// # 返回
    ///
    /// 如果键长度超过最大长度，返回哈希指纹；否则返回原始键
    pub fn hash_fingerprint(&self, key: &str) -> String {
        if key.len() <= self.max_key_length {
            key.to_string()
        } else {
            let hash = murmur3_32(&mut key.as_bytes(), 0).expect("Failed to compute murmur3 hash");
            format!("hash:{:08x}", hash)
        }
    }

    /// 生成唯一的服务级键
    ///
    /// 用于生成服务隔离的缓存键，格式为：`{prefix}:{namespace}:{service}:{key}`
    pub fn service_key(&self, service: &str, key: &str) -> String {
        self.apply_namespace_and_prefix(&format!("{}:{}", service, key))
    }

    /// 获取当前命名空间
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// 获取当前前缀
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// 获取最大键长度
    pub fn max_key_length(&self) -> usize {
        self.max_key_length
    }

    /// 应用命名空间和前缀到键
    fn apply_namespace_and_prefix(&self, key: &str) -> String {
        let mut result = String::new();

        if !self.prefix.is_empty() {
            result.push_str(&self.prefix);
            result.push(':');
        }

        if !self.namespace.is_empty() && self.namespace != DEFAULT_NAMESPACE {
            result.push_str(&self.namespace);
            result.push(':');
        }

        result.push_str(key);
        result
    }
}

/// Arc 包装的 KeyGenerator，用于跨线程共享
pub type KeyGeneratorRef = Arc<KeyGenerator>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_basic() {
        let generator = KeyGenerator::new();
        let key = generator.generate("user:{id}", &[("id", "123")]);
        assert_eq!(key, "user:123");
    }

    #[test]
    fn test_generate_multiple_args() {
        let generator = KeyGenerator::new();
        let key = generator.generate(
            "product:{category}:{id}",
            &[("category", "electronics"), ("id", "456")],
        );
        assert_eq!(key, "product:electronics:456");
    }

    #[test]
    fn test_with_namespace() {
        let generator = KeyGenerator::new().with_namespace("app:v1");
        let key = generator.generate("user:{id}", &[("id", "123")]);
        assert_eq!(key, "app:v1:user:123");
    }

    #[test]
    fn test_with_prefix() {
        let generator = KeyGenerator::with_prefix("myapp").with_namespace("v1");
        let key = generator.generate("user:{id}", &[("id", "123")]);
        assert_eq!(key, "myapp:v1:user:123");
    }

    #[test]
    fn test_validate_valid_key() {
        let generator = KeyGenerator::new();
        assert!(generator.validate("user:123:profile").is_ok());
    }

    #[test]
    fn test_validate_empty_key() {
        let generator = KeyGenerator::new();
        assert!(generator.validate("").is_err());
    }

    #[test]
    fn test_validate_invalid_char() {
        let generator = KeyGenerator::new();
        assert!(generator.validate("user:123:profile\n").is_err());
    }

    #[test]
    fn test_normalize() {
        let generator = KeyGenerator::new();
        let normalized = generator.normalize("  User:123  ");
        assert_eq!(normalized, "user:123");
    }

    #[test]
    fn test_hash_fingerprint_short_key() {
        let generator = KeyGenerator::new();
        let key = "short_key";
        assert_eq!(generator.hash_fingerprint(key), key);
    }

    #[test]
    fn test_hash_fingerprint_long_key() {
        let generator = KeyGenerator::new().with_max_key_length(10);
        let long_key = "this_is_a_very_long_key_that_exceeds_limit";
        let fingerprint = generator.hash_fingerprint(long_key);
        assert!(fingerprint.starts_with("hash:"));
        assert!(fingerprint.len() <= 10 + 5); // "hash:" + 8 hex chars
    }

    #[test]
    fn test_service_key() {
        let generator = KeyGenerator::new().with_namespace("app:v1");
        let key = generator.service_key("user_service", "user:123");
        assert_eq!(key, "app:v1:user_service:user:123");
    }
}
