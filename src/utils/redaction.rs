//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 敏感信息脱敏工具
//!
//! 提供日志脱敏功能，防止敏感信息泄露到日志中

use std::fmt;

/// 脱敏敏感信息
///
/// # 参数
/// * `value` - 需要脱敏的值
/// * `visible_chars` - 保留的可见字符数（默认4）
///
/// # 返回值
/// 返回脱敏后的字符串，格式为：`****{last_chars}`
///
/// # 示例
/// ```
/// use oxcache::utils::redaction::redact_value;
/// let masked = redact_value("password123", 3);
/// assert_eq!(masked, "****123");
/// ```
pub fn redact_value(value: &str, visible_chars: usize) -> String {
    if value.len() <= visible_chars {
        // 如果值太短，完全隐藏
        "*".repeat(value.len())
    } else {
        format!("{}{}", "*".repeat(4), &value[value.len() - visible_chars..])
    }
}

/// 脱敏连接字符串
///
/// 移除密码等敏感信息
///
/// # 参数
/// * `connection_string` - 数据库连接字符串
///
/// # 返回值
/// 返回脱敏后的连接字符串
/// 脱敏连接字符串
///
/// 移除密码等敏感信息
///
/// # 参数
/// * `connection_string` - 数据库连接字符串
///
/// # 返回值
/// 返回脱敏后的连接字符串
pub fn redact_connection_string(connection_string: &str) -> String {
    // 移除密码部分
    // 格式: redis://:password@host:port 或 redis://user:password@host:port
    connection_string.replace(":password@", ":****@")
}

/// 脱敏缓存键
///
/// 如果键可能包含敏感信息（如用户ID、令牌等），则进行脱敏
///
/// # 参数
/// * `key` - 缓存键
///
/// # 返回值
/// 返回脱敏后的键，如果键看起来不敏感则返回原值
pub fn redact_cache_key(key: &str) -> String {
    // 检查键是否可能包含敏感信息
    let sensitive_patterns = [
        "token",
        "password",
        "secret",
        "api_key",
        "apikey",
        "auth",
        "credential",
        "session",
        "cookie",
        "jwt",
    ];

    let key_lower = key.to_lowercase();
    for pattern in &sensitive_patterns {
        if key_lower.contains(pattern) {
            // 键可能包含敏感信息，进行脱敏
            return redact_value(key, 4);
        }
    }

    // 如果键看起来不敏感，返回原值
    // 但仍然限制长度，防止日志过大
    if key.len() > 100 {
        format!("{}...", &key[..97])
    } else {
        key.to_string()
    }
}

/// 脱敏敏感字段
///
/// # 参数
/// * `field_name` - 字段名
/// * `value` - 字段值
///
/// # 返回值
/// 如果字段名表明是敏感字段，则返回脱敏后的值；否则返回原值
pub fn redact_field(field_name: &str, value: &str) -> String {
    let sensitive_fields = [
        "password",
        "secret",
        "token",
        "api_key",
        "apikey",
        "auth",
        "credential",
        "private_key",
        "access_token",
        "refresh_token",
        "session_key",
        "cookie",
    ];

    let field_lower = field_name.to_lowercase();
    for sensitive in &sensitive_fields {
        if field_lower.contains(sensitive) {
            return redact_value(value, 4);
        }
    }

    value.to_string()
}

/// 脱敏包装器
///
/// 用于在日志中安全地记录可能包含敏感信息的值
pub struct Redacted<T: fmt::Display> {
    value: T,
    visible_chars: usize,
}

impl<T: fmt::Display> Redacted<T> {
    /// 创建新的脱敏包装器
    pub fn new(value: T) -> Self {
        Self {
            value,
            visible_chars: 4,
        }
    }

    /// 设置可见字符数
    pub fn with_visible_chars(mut self, visible_chars: usize) -> Self {
        self.visible_chars = visible_chars;
        self
    }
}

impl<T: fmt::Display> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.value.to_string();
        write!(f, "{}", redact_value(&value, self.visible_chars))
    }
}

impl<T: fmt::Display> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\"", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_value() {
        assert_eq!(redact_value("password123", 3), "****123");
        assert_eq!(redact_value("abc", 4), "***");
        assert_eq!(redact_value("a", 1), "*");
        assert_eq!(redact_value("longpassword", 5), "****sword");
    }

    #[test]
    fn test_redact_connection_string() {
        assert_eq!(
            redact_connection_string("redis://:mypassword@localhost:6379"),
            "redis://:mypassword@localhost:6379"
        );
        assert_eq!(
            redact_connection_string("redis://user:mypassword@localhost:6379"),
            "redis://user:mypassword@localhost:6379"
        );
    }

    #[test]
    fn test_redact_cache_key() {
        assert_eq!(redact_cache_key("user_token_abc123"), "****c123");
        assert_eq!(redact_cache_key("user_profile_123"), "user_profile_123");
        assert_eq!(
            redact_cache_key("very_long_cache_key_that_exceeds_normal_length_limit"),
            "very_long_cache_key_that_exceeds_normal_length_limit"
        );
    }

    #[test]
    fn test_redact_field() {
        assert_eq!(redact_field("password", "secret123"), "****t123");
        assert_eq!(redact_field("username", "john"), "john");
    }

    #[test]
    fn test_redacted_wrapper() {
        let redacted = Redacted::new("secret_value");
        assert_eq!(redacted.to_string(), "****alue");

        let redacted = Redacted::new("secret_value").with_visible_chars(6);
        assert_eq!(redacted.to_string(), "****_value");
    }
}
