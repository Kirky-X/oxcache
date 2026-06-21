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
#[cfg(feature = "memory")]
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
    #[cfg(feature = "memory")]
    pub fn with_eviction_policy(self, _policy: EvictionPolicy) -> Self {
        // 暂时忽略淘汰策略，用于接口兼容性
        self
    }

    #[cfg(not(feature = "memory"))]
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
