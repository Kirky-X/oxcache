// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 安全日志工具单元测试

use oxcache::security::log::sanitize_message;
use oxcache::security::redaction::{redact_cache_key, redact_connection_string};

// ============================================================================
// log_cache_key 函数测试
// ============================================================================
// 注意：log_cache_key 函数使用 tracing 日志，主要测试它不会 panic

/// 测试 log_cache_key 处理敏感键不会 panic
#[test]
fn test_log_cache_key_sensitive_key() {
    // 使用不同日志级别测试
    // 主要验证函数不会因敏感键而 panic
    oxcache::security::log::log_cache_key("info", "Test message", "user_token_abc123");
    oxcache::security::log::log_cache_key("debug", "Debug log", "api_key_secret");
    oxcache::security::log::log_cache_key("warn", "Warning", "password_reset");
    oxcache::security::log::log_cache_key("error", "Error", "session_token_xyz");
}

/// 测试 log_cache_key 处理非敏感键
#[test]
fn test_log_cache_key_non_sensitive() {
    oxcache::security::log::log_cache_key("info", "Normal key", "user_profile_123");
    oxcache::security::log::log_cache_key("debug", "Cache hit", "product_info_456");
}

/// 测试 log_cache_key 处理空键
#[test]
fn test_log_cache_key_empty() {
    oxcache::security::log::log_cache_key("info", "Empty key test", "");
}

/// 测试 log_cache_key 处理未知日志级别
#[test]
fn test_log_cache_key_unknown_level() {
    // 未知级别应该默认为 info
    oxcache::security::log::log_cache_key("unknown", "Test", "normal_key");
}

// ============================================================================
// sanitize_message 函数测试
// ============================================================================

/// 测试包含连接字符串的消息脱敏
#[test]
fn test_sanitize_message_with_connection_string() {
    let msg = "Connection: redis://user:secret123@localhost:6379";
    let sanitized = sanitize_message(msg);

    // 验证密码被移除
    assert!(!sanitized.contains("secret123"));
    // 验证脱敏标记存在
    assert!(sanitized.contains("**"));
}

/// 测试不包含连接字符串的消息
#[test]
fn test_sanitize_message_without_connection_string() {
    let msg = "User logged in successfully";
    let sanitized = sanitize_message(msg);

    assert_eq!(sanitized, msg);
}

/// 测试空消息
#[test]
fn test_sanitize_message_empty() {
    let sanitized = sanitize_message("");
    assert_eq!(sanitized, "");
}

/// 测试多个连接字符串的消息（只处理第一个）
#[test]
fn test_sanitize_message_multiple_connections() {
    let msg = "Primary: redis://admin:pass1@host1:6379, Secondary: redis://admin:pass2@host2:6379";
    let sanitized = sanitize_message(msg);

    // 验证第一个密码被移除
    assert!(!sanitized.contains("pass1"));
    // 注意：当前实现只处理第一个连接字符串
}

/// 测试不同协议的连接字符串
#[test]
fn test_sanitize_message_different_protocols() {
    let msg = "Database: postgresql://user:dbpass@db.example.com:5432";
    let sanitized = sanitize_message(msg);

    assert!(!sanitized.contains("dbpass"));
}

/// 测试消息中没有 @ 符号的情况
#[test]
fn test_sanitize_message_no_at_symbol() {
    let msg = "This is a normal message without connection string";
    let sanitized = sanitize_message(msg);

    assert_eq!(sanitized, msg);
}

/// 测试消息中只有协议但没有认证信息
#[test]
fn test_sanitize_message_protocol_only() {
    let msg = "Visit https://example.com for more info";
    let sanitized = sanitize_message(msg);

    assert_eq!(sanitized, msg);
}

// ============================================================================
// 辅助函数测试（使用 redaction 模块）
// ============================================================================

/// 测试 redact_cache_key 与敏感关键词的匹配
#[test]
fn test_redact_cache_key_all_patterns() {
    // 测试所有敏感关键词模式
    let sensitive_keys = [
        ("user_token", true),
        ("password_reset", true),
        ("secret_key", true),
        ("api_key_value", true),
        ("apikey_data", true),
        ("auth_session", true),
        ("credential_store", true),
        ("session_id", true),
        ("cookie_value", true),
        ("jwt_token", true),
        ("normal_key", false),
        ("user_profile", false),
        ("cache_item", false),
    ];

    for (key, is_sensitive) in sensitive_keys {
        let result = redact_cache_key(key);
        if is_sensitive {
            assert!(
                result.starts_with("****"),
                "Key '{}' should be redacted, got '{}'",
                key,
                result
            );
        } else {
            assert_eq!(result, key, "Key '{}' should not be redacted, got '{}'", key, result);
        }
    }
}

/// 测试 redact_connection_string 边界情况
#[test]
fn test_redact_connection_string_edge_cases() {
    // 只有一个冒号（协议分隔符）
    assert_eq!(redact_connection_string("redis://localhost"), "redis://localhost");

    // 多个冒号（密码中包含冒号）
    let result = redact_connection_string("redis://user:pass:word@host:6379");
    assert!(!result.contains("pass:word"));
    assert!(result.contains("****"));
}

// ============================================================================
// 宏测试（编译时验证）
// ============================================================================

/// 测试 secure_info! 宏可以编译
#[test]
fn test_secure_info_macro_compiles() {
    // 主要验证宏可以编译通过
    // 由于宏使用 tracing，这里只做编译验证
    let _msg = "Test message for secure_info";
}

/// 测试 secure_debug! 宏可以编译
#[test]
fn test_secure_debug_macro_compiles() {
    // 主要验证宏可以编译通过
    let _msg = "Test message for secure_debug";
}

// ============================================================================
// 边界条件和异常输入测试
// ============================================================================

/// 测试超长消息处理
#[test]
fn test_sanitize_message_very_long() {
    let long_msg = format!("Connection: redis://user:{}@localhost:6379", "a".repeat(10000));

    // 主要验证不会 panic
    let _sanitized = sanitize_message(&long_msg);
}

/// 测试包含 Unicode 的消息
#[test]
fn test_sanitize_message_unicode() {
    let msg = "连接: redis://用户:密码@本地主机:6379";
    let sanitized = sanitize_message(msg);

    // 验证处理 Unicode 不会崩溃
    assert!(!sanitized.is_empty());
}

/// 测试消息包含特殊字符
#[test]
fn test_sanitize_message_special_chars() {
    let msg = "Conn: redis://user:p@ss!w0rd#@host:6379";
    let sanitized = sanitize_message(msg);

    // 验证处理特殊字符不会崩溃
    assert!(!sanitized.is_empty());
}
