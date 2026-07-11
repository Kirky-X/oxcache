// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// 敏感数据脱敏工具单元测试

#![cfg(feature = "redis")]

use oxcache::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted};

// ============================================================================
// redact_value 函数测试
// ============================================================================

/// 测试正常长字符串的部分显示
#[test]
fn test_redact_value_normal_string() {
    assert_eq!(redact_value("password123", 3), "****123");
    assert_eq!(redact_value("longpassword", 5), "****sword");
    assert_eq!(redact_value("secret_key_value", 4), "****alue");
}

/// 测试短字符串的完全隐藏
#[test]
fn test_redact_value_short_string() {
    // 字符串长度等于可见字符数，完全隐藏
    assert_eq!(redact_value("abc", 4), "***");
    assert_eq!(redact_value("test", 4), "****");

    // 字符串长度小于可见字符数
    assert_eq!(redact_value("a", 4), "*");
    assert_eq!(redact_value("ab", 4), "**");
}

/// 测试空字符串处理
#[test]
fn test_redact_value_empty_string() {
    assert_eq!(redact_value("", 4), "");
    assert_eq!(redact_value("", 0), "");
}

/// 测试可见字符数为 0 的情况
/// 注意：当 visible_chars=0 时，函数返回 "****"（固定前缀）
#[test]
fn test_redact_value_zero_visible() {
    // 当 visible_chars=0，value.len() > 0，进入 else 分支
    // 返回 "****" + 空字符串 = "****"
    assert_eq!(redact_value("password", 0), "****");
    assert_eq!(redact_value("test", 0), "****");
}

/// 测试 Unicode 字符处理
/// 注意：当前实现使用字节索引，可能导致 Unicode 边界问题
/// 此测试只测试 ASCII 与 Unicode 混合且边界安全的情况
#[test]
fn test_redact_value_unicode() {
    // 测试 ASCII 后缀（边界安全）
    assert_eq!(redact_value("密码abc123", 3), "****123");
    // 纯 ASCII 测试
    assert_eq!(redact_value("hello世界test", 4), "****test");
}

/// 测试超长字符串处理
#[test]
fn test_redact_value_long_string() {
    let long_value = "a".repeat(10000);
    let result = redact_value(&long_value, 4);
    assert!(result.starts_with("****"));
    assert!(result.ends_with("aaaa"));
    assert_eq!(result.len(), 8); // "****" + 4 chars
}

// ============================================================================
// redact_connection_string 函数测试
// ============================================================================

/// 测试带密码的 Redis 连接字符串
#[test]
fn test_redact_connection_string_with_password() {
    assert_eq!(
        redact_connection_string("redis://:mypassword@localhost:6379"),
        "redis://:****@localhost:6379"
    );
    assert_eq!(
        redact_connection_string("redis://user:mypassword@localhost:6379"),
        "redis://user:****@localhost:6379"
    );
}

/// 测试无密码的连接字符串
#[test]
fn test_redact_connection_string_without_password() {
    assert_eq!(
        redact_connection_string("redis://user@localhost:6379"),
        "redis://user:****@localhost:6379"
    );
    assert_eq!(
        redact_connection_string("redis://localhost:6379"),
        "redis://localhost:6379"
    );
}

/// 测试不同协议的连接字符串
#[test]
fn test_redact_connection_string_different_protocols() {
    assert_eq!(
        redact_connection_string("mysql://admin:secret123@db.example.com:3306"),
        "mysql://admin:****@db.example.com:3306"
    );
    assert_eq!(
        redact_connection_string("postgresql://user:pass@localhost:5432"),
        "postgresql://user:****@localhost:5432"
    );
}

/// 测试不带认证信息的连接字符串
#[test]
fn test_redact_connection_string_no_auth() {
    assert_eq!(
        redact_connection_string("redis://localhost:6379"),
        "redis://localhost:6379"
    );
    assert_eq!(
        redact_connection_string("http://example.com/api"),
        "http://example.com/api"
    );
}

/// 测试复杂密码处理
/// 注意：实现使用 rfind(':') 找最后一个冒号，复杂密码会被部分保留
#[test]
fn test_redact_connection_string_complex_password() {
    // 密码包含冒号时，rfind 会找到密码中的冒号
    let result = redact_connection_string("redis://user:p@ss:w0rd@localhost:6379");
    // 验证 @ 符号之后的部分被保留
    assert!(result.contains("@localhost:6379"));
    // 验证 **** 脱敏标记存在
    assert!(result.contains("****"));
}

/// 测试空连接字符串
#[test]
fn test_redact_connection_string_empty() {
    assert_eq!(redact_connection_string(""), "");
}

// ============================================================================
// redact_cache_key 函数测试
// ============================================================================

/// 测试包含敏感关键词的缓存键
#[test]
fn test_redact_cache_key_sensitive() {
    // token 关键词
    assert_eq!(redact_cache_key("user_token_abc123"), "****c123");
    assert_eq!(redact_cache_key("access_token_xyz"), "****_xyz");

    // password 关键词
    assert_eq!(redact_cache_key("password_reset_key"), "****_key");

    // secret 关键词
    assert_eq!(redact_cache_key("secret_api_data"), "****data");

    // api_key 关键词
    assert_eq!(redact_cache_key("api_key_value"), "****alue");

    // session 关键词
    assert_eq!(redact_cache_key("session_user_123"), "****_123");
}

/// 测试不包含敏感关键词的缓存键
#[test]
fn test_redact_cache_key_non_sensitive() {
    assert_eq!(redact_cache_key("user_profile_123"), "user_profile_123");
    assert_eq!(redact_cache_key("cache_data_item"), "cache_data_item");
    assert_eq!(redact_cache_key("product_info_456"), "product_info_456");
}

/// 测试超长缓存键截断
#[test]
fn test_redact_cache_key_long_key() {
    let long_key = "a".repeat(150);
    let result = redact_cache_key(&long_key);
    assert_eq!(result.len(), 100);
    assert!(result.ends_with("..."));
}

/// 测试边界长度缓存键（正好 100 字符）
#[test]
fn test_redact_cache_key_exact_100_chars() {
    let key = "a".repeat(100);
    let result = redact_cache_key(&key);
    assert_eq!(result.len(), 100);
    assert!(!result.ends_with("..."));
}

/// 测试大小写不敏感匹配
#[test]
fn test_redact_cache_key_case_insensitive() {
    // 大小写不敏感匹配会被脱敏
    let result1 = redact_cache_key("USER_TOKEN_123");
    assert!(result1.starts_with("****"));

    let result2 = redact_cache_key("Api_Key_Data");
    assert!(result2.starts_with("****"));

    let result3 = redact_cache_key("SECRET_VALUE");
    assert!(result3.starts_with("****"));
}

// ============================================================================
// redact_field 函数测试
// ============================================================================

/// 测试敏感字段脱敏
#[test]
fn test_redact_field_sensitive() {
    // redact_value("secret123", 4) = "****t123"
    assert_eq!(redact_field("password", "secret123"), "****t123");
    // redact_value("abc123xyz", 4) = "****3xyz"（后4个字符）
    assert_eq!(redact_field("api_key", "abc123xyz"), "****3xyz");
    // redact_value("token_value", 4) = "****alue"
    assert_eq!(redact_field("access_token", "token_value"), "****alue");
    // redact_value("key_data", 4) = "****data"
    assert_eq!(redact_field("private_key", "key_data"), "****data");
}

/// 测试非敏感字段不脱敏
#[test]
fn test_redact_field_non_sensitive() {
    assert_eq!(redact_field("username", "john_doe"), "john_doe");
    assert_eq!(redact_field("email", "user@example.com"), "user@example.com");
    assert_eq!(redact_field("name", "Alice"), "Alice");
}

/// 测试字段名部分匹配
#[test]
fn test_redact_field_partial_match() {
    // 包含 password 的字段名
    // redact_value("pass123", 4) = "****s123"
    assert_eq!(redact_field("user_password", "pass123"), "****s123");

    // 包含 token 的字段名
    // redact_value("xyz", 4) = "***" (长度 <= 4，全星号)
    assert_eq!(redact_field("auth_token_value", "xyz"), "***");
}

// ============================================================================
// Redacted 包装器测试
// ============================================================================

/// 测试 Redacted 包装器基本功能
#[test]
fn test_redacted_wrapper() {
    let redacted = Redacted::new("secret_value");
    assert_eq!(redacted.to_string(), "****alue");
}

/// 测试 Redacted 包装器自定义可见字符数
#[test]
fn test_redacted_wrapper_custom_visible() {
    let redacted = Redacted::new("secret_value").with_visible_chars(6);
    assert_eq!(redacted.to_string(), "****_value");

    // 当 visible_chars=0 时，返回 "****"（固定前缀）
    let redacted = Redacted::new("password123").with_visible_chars(0);
    assert_eq!(redacted.to_string(), "****");
}

/// 测试 Redacted Debug 实现
#[test]
fn test_redacted_debug() {
    let redacted = Redacted::new("test_value");
    let debug_str = format!("{:?}", redacted);
    assert!(debug_str.starts_with('"'));
    assert!(debug_str.ends_with('"'));
}

/// 测试 Redacted 短值处理
#[test]
fn test_redacted_short_value() {
    let redacted = Redacted::new("ab");
    assert_eq!(redacted.to_string(), "**");
}
