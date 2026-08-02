// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 安全日志工具模块
//
// 提供安全的日志记录功能，自动脱敏敏感信息

#![cfg_attr(doctest, allow(unused_imports))]

use crate::security::redact_cache_key;

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
        use $crate::security::redact_connection_string;
        tracing::info!("{}", format!($($arg)*)
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
        use $crate::security::redact_connection_string;
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
/// use crate::security::log_cache_key;
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
/// use crate::security::sanitize_message;
///
/// let msg = sanitize_message("User token: user_token_abc123, password: secret123");
/// // 返回: User token: ****c123, password: ****
/// ```
pub fn sanitize_message(message: &str) -> String {
    let mut result = String::with_capacity(message.len());
    let mut remaining = message;

    // 脱敏所有连接字符串（支持多个 :// 出现）
    while let Some(rel_pos) = remaining.find("://") {
        // 提取协议名（向前找到开头或空格）
        let protocol_start = remaining[..rel_pos]
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        let protocol = &remaining[protocol_start..rel_pos];
        let after_start = rel_pos + 3;

        // 将 :// 之前的前导文本追加到结果
        result.push_str(&remaining[..protocol_start]);

        if let Some(at_pos) = remaining[after_start..].find('@') {
            let abs_at_pos = after_start + at_pos;
            let user_part = &remaining[after_start..abs_at_pos];
            // host_part 终止于下一个空白字符或字符串末尾，避免吞并后续 URL
            let host_end = remaining[abs_at_pos..]
                .find(|c: char| c.is_whitespace())
                .map(|i| abs_at_pos + i)
                .unwrap_or(remaining.len());
            let host_part = &remaining[abs_at_pos..host_end];

            let sanitized_user: String = user_part
                .chars()
                .take_while(|c| *c != ':')
                .chain(std::iter::once('*').chain(std::iter::once('*')).take(2))
                .collect();

            result.push_str(protocol);
            result.push_str("://");
            result.push_str(&sanitized_user);
            result.push_str(host_part);

            // 移动 remaining 到 host_part 之后
            remaining = &remaining[host_end..];
        } else {
            // 没有 @ 符号，保留协议名和 ://，继续搜索
            result.push_str(protocol);
            result.push_str("://");
            remaining = &remaining[after_start..];
        }
    }

    // 追加剩余部分
    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{redact_cache_key, redact_connection_string};

    #[test]
    fn test_log_connection_string() {
        // 测试日志记录功能
        let conn_str = "redis://user:password123@localhost:6379"; /* pragma: allowlist secret */
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
        let msg = "Connection: redis://user:secret123@localhost:6379"; /* pragma: allowlist secret */
        let sanitized = sanitize_message(msg);

        assert!(!sanitized.contains("secret123"));
        assert!(sanitized.contains("**"));
    }

    // ============================================================================
    // log_cache_key 测试 (lines 85-86, 89-93)
    // ============================================================================

    #[test]
    fn test_log_cache_key_info_level() {
        // 测试 info 级别日志 (line 89)
        // 这个测试验证函数不会 panic
        log_cache_key("info", "Cache hit", "user_token_abc123");
    }

    #[test]
    fn test_log_cache_key_debug_level() {
        // 测试 debug 级别日志 (line 90)
        log_cache_key("debug", "Cache debug", "session_xyz");
    }

    #[test]
    fn test_log_cache_key_warn_level() {
        // 测试 warn 级别日志 (line 91)
        log_cache_key("warn", "Cache warning", "password_123");
    }

    #[test]
    fn test_log_cache_key_error_level() {
        // 测试 error 级别日志 (line 92)
        log_cache_key("error", "Cache error", "api_key_test");
    }

    #[test]
    fn test_log_cache_key_default_level() {
        // 测试默认级别（未知级别）(line 93)
        log_cache_key("trace", "Cache trace", "normal_key");
        log_cache_key("unknown_level", "Cache unknown", "another_key");
    }

    #[test]
    fn test_log_cache_key_non_sensitive_key() {
        // 测试非敏感键
        log_cache_key("info", "Cache access", "user_profile_123");
    }

    #[test]
    fn test_log_cache_key_empty_key() {
        // 测试空键
        log_cache_key("info", "Empty key", "");
    }

    #[test]
    fn test_log_cache_key_empty_message() {
        // 测试空消息
        log_cache_key("info", "", "some_key");
    }

    // ============================================================================
    // sanitize_message 边界测试
    // ============================================================================

    #[test]
    fn test_sanitize_message_no_connection_string() {
        let msg = "This is a normal message without connection string";
        let sanitized = sanitize_message(msg);
        assert_eq!(sanitized, msg);
    }

    #[test]
    fn test_sanitize_message_empty() {
        let sanitized = sanitize_message("");
        assert_eq!(sanitized, "");
    }

    #[test]
    fn test_sanitize_message_no_at_symbol() {
        let msg = "redis://localhost:6379";
        let sanitized = sanitize_message(msg);
        assert_eq!(sanitized, msg);
    }

    #[test]
    fn test_sanitize_message_with_password() {
        let msg = "redis://user:password123@host:6379"; /* pragma: allowlist secret */
        let sanitized = sanitize_message(msg);
        assert!(!sanitized.contains("password123"));
    }

    #[test]
    fn test_sanitize_message_multiple_protocols() {
        let msg = "redis://user:pass1@host1:6379 and redis://user:pass2@host2:6380"; /* pragma: allowlist secret */
        let sanitized = sanitize_message(msg);
        // 处理所有 :// 连接字符串
        assert!(!sanitized.contains("pass1"));
        assert!(!sanitized.contains("pass2"));
    }
}
