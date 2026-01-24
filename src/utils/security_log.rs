// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 安全日志工具模块
//
// 提供安全的日志记录功能，自动脱敏敏感信息

#![cfg_attr(doctest, allow(unused_imports))]

use crate::utils::redaction::redact_cache_key;

/// 安全日志宏 - 自动脱敏连接字符串
///
/// # 示例
///
/// ```rust,ignore
/// # // 注意：此示例需要完整的导入路径
/// # use oxcache::utils::security_log;
/// #
/// # // 记录连接字符串时会自动脱敏
/// # security_log::info("Redis URL: {}", "redis://user:password@localhost:6379");
/// # // 输出: Redis URL: redis://user:****@localhost:6379
/// ```
/// 安全记录信息级别日志
#[macro_export]
macro_rules! secure_info {
    ($($arg:tt)*) => {{
        use $crate::utils::redaction::redact_connection_string;
        tracing::info!("{}", format!($($arg)*)
            .replace(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '/' || c == '@' || c == '.', |c| {
                if c == '@' || (c.is_ascii_digit() && false) {
                    c
                } else {
                    c
                }
            })
            .split_inclusive("://")
            .map(|part| {
                if part.contains("password") || part.contains("secret") || part.contains("token") {
                    redact_connection_string(part)
                } else {
                    part.to_string()
                }
            })
            .collect::<String>()
        );
    }}
}

/// 安全记录调试级别日志
#[macro_export]
macro_rules! secure_debug {
    ($($arg:tt)*) => {{
        use $crate::utils::redaction::redact_connection_string;
        tracing::debug!("{}", format!($($arg)*)
            .split("://")
            .map(|part| {
                if part.contains("password") || part.contains("secret") || part.contains("token") {
                    redact_connection_string(&format!("://{}", part))
                } else {
                    part.to_string()
                }
            })
            .collect::<String>()
        );
    }}
}

/// 安全记录缓存键到日志
///
/// # 参数
/// * `level` - 日志级别
/// * `message` - 日志消息
/// * `key` - 缓存键
///
/// # 示例
///
/// ```rust,ignore
/// use crate::utils::security_log::log_cache_key;
///
/// log_cache_key("debug", "Cache access", "user_token_abc123");
/// // 日志输出: Cache access: ****c123
/// ```
pub fn log_cache_key(level: &str, message: &str, key: &str) {
    let redacted = redact_cache_key(key);

    match level {
        "info" => tracing::info!("{}: {}", message, redacted),
        "debug" => tracing::debug!("{}: {}", message, redacted),
        "warn" => tracing::warn!("{}: {}", message, redacted),
        "error" => tracing::error!("{}: {}", message, redacted),
        _ => tracing::info!("{}: {}", message, redacted),
    }
}

/// 安全格式化消息
///
/// 自动脱敏消息中的敏感信息
///
/// # 参数
/// * `message` - 原始消息
///
/// # 返回值
/// 脱敏后的消息
///
/// # 示例
///
/// ```rust,ignore
/// use crate::utils::security_log::sanitize_message;
///
/// let msg = sanitize_message("User token: user_token_abc123, password: secret123");
/// // 返回: User token: ****c123, password: ****
/// ```
pub fn sanitize_message(message: &str) -> String {
    let mut result = message.to_string();

    // 脱敏连接字符串
    if result.contains("://") {
        if let Some(start) = result.find("://") {
            let protocol = &result[..start];
            let after_protocol = &result[start + 3..];

            if let Some(at_pos) = after_protocol.find('@') {
                let user_part = &after_protocol[..at_pos];
                let host_part = &after_protocol[at_pos..];

                let sanitized_user: String = user_part
                    .chars()
                    .take_while(|c| *c != ':')
                    .chain(std::iter::once('*').chain(std::iter::once('*')).take(2))
                    .collect();

                result = format!("{}://{}{}", protocol, sanitized_user, host_part);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::redaction::{redact_cache_key, redact_connection_string};

    #[test]
    fn test_log_connection_string() {
        // 测试日志记录功能
        let conn_str = "redis://user:password123@localhost:6379";
        let redacted = redact_connection_string(conn_str);
        assert!(!redacted.contains("password123"));
        assert!(redacted.contains("user:****"));
    }

    #[test]
    fn test_log_cache_key() {
        // 测试缓存键脱敏
        let key = "user_token_abc123";
        let redacted = redact_cache_key(key);
        assert!(!redacted.contains("token"));
        assert!(redacted.starts_with("****"));
    }

    #[test]
    fn test_sanitize_message() {
        let msg = "Connection: redis://user:secret123@localhost:6379";
        let sanitized = sanitize_message(msg);

        assert!(!sanitized.contains("secret123"));
        assert!(sanitized.contains("**"));
    }
}
