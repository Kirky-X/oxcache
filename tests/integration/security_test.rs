// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 安全性集成测试 - 简化版本

use oxcache::security::{validate_lua_script, validate_redis_key, validate_scan_pattern};

use crate::common;

// ============================================================================
// 安全验证测试
// ============================================================================

#[tokio::test]
async fn test_lua_script_injection_attempts() {
    common::setup_logging();

    // 测试各种 Lua 脚本注入攻击

    // 攻击 1: FLUSHALL
    let result = validate_lua_script("return redis.call('FLUSHALL')", 0);
    assert!(result.is_err(), "FLUSHALL should be rejected");

    // 攻击 2: FLUSHDB
    let result = validate_lua_script("return redis.call('FLUSHDB')", 0);
    assert!(result.is_err(), "FLUSHDB should be rejected");

    // 攻击 3: KEYS 命令
    let result = validate_lua_script("return redis.call('KEYS', '*')", 0);
    assert!(result.is_err(), "KEYS command should be rejected");

    // 攻击 4: SHUTDOWN
    let result = validate_lua_script("return redis.call('SHUTDOWN')", 0);
    assert!(result.is_err(), "SHUTDOWN should be rejected");

    // 攻击 5: CONFIG 命令
    let result = validate_lua_script("return redis.call('CONFIG', 'GET', '*')", 0);
    assert!(result.is_err(), "CONFIG command should be rejected");

    // 攻击 6: DEBUG 命令
    let result = validate_lua_script("return redis.call('DEBUG', 'SEGFAULT')", 0);
    assert!(result.is_err(), "DEBUG command should be rejected");

    // 攻击 7: 大小写混合尝试绕过
    let result = validate_lua_script("return redis.call('flushall')", 0);
    assert!(
        result.is_err(),
        "Case-insensitive FLUSHALL should be rejected"
    );

    let result = validate_lua_script("return redis.call('FlushAll')", 0);
    assert!(result.is_err(), "Mixed case FLUSHALL should be rejected");

    // 攻击 8: 嵌套在注释中的恶意代码
    let result = validate_lua_script("--[[ malicious ]] return redis.call('FLUSHALL')", 0);
    assert!(
        result.is_err(),
        "Comment-hidden FLUSHALL should be rejected"
    );

    // 攻击 9: 字符串中的恶意代码（不应被检测，因为不在 redis.call 中）
    let result = validate_lua_script("local x = 'FLUSHALL'; return 'safe'", 0);
    assert!(
        result.is_ok(),
        "String containing FLUSHALL should be allowed"
    );

    println!("✓ All Lua script injection attempts were correctly rejected");
}

#[tokio::test]
async fn test_scan_pattern_redos_attempts() {
    common::setup_logging();

    // 测试各种 ReDoS 攻击模式

    // 攻击 1: 过多通配符
    let result = validate_scan_pattern(&"*".repeat(20));
    assert!(result.is_err(), "Too many wildcards should be rejected");

    // 攻击 2: 超长模式
    let result = validate_scan_pattern(&"x".repeat(300));
    assert!(result.is_err(), "Pattern too long should be rejected");

    // 攻击 3: 嵌套通配符
    let result = validate_scan_pattern("*:*:*:*:*:*:*:*:*:*:*:*");
    assert!(
        result.is_err(),
        "Too many nested wildcards should be rejected"
    );

    // 攻击 4: 边界情况 - 刚好 10 个通配符（应该通过）
    let result = validate_scan_pattern(&"*".repeat(10));
    assert!(result.is_ok(), "Exactly 10 wildcards should be allowed");

    // 攻击 5: 边界情况 - 11 个通配符（应该拒绝）
    let result = validate_scan_pattern(&"*".repeat(11));
    assert!(result.is_err(), "11 wildcards should be rejected");

    // 攻击 6: 复杂正则表达式模式
    let result = validate_scan_pattern("(a+)+test");
    assert!(
        result.is_ok(),
        "Complex pattern should be allowed (not ReDoS vulnerable)"
    );

    // 攻击 7: 混合通配符
    let result = validate_scan_pattern("user:*:data:*:profile");
    assert!(result.is_ok(), "Mixed wildcards pattern should be allowed");

    println!("✓ All SCAN ReDoS attempts were correctly handled");
}

#[tokio::test]
async fn test_redis_key_injection_attempts() {
    common::setup_logging();

    // 测试各种 Redis 键注入攻击

    // 攻击 1: 换行符注入
    let result = validate_redis_key("key\r\nvalue");
    assert!(result.is_err(), "CRLF injection should be rejected");

    // 攻击 2: 单独换行符
    let result = validate_redis_key("key\nvalue");
    assert!(result.is_err(), "LF injection should be rejected");

    // 攻击 3: 单独回车符
    let result = validate_redis_key("key\rvalue");
    assert!(result.is_err(), "CR injection should be rejected");

    // 攻击 4: 空字节注入
    let result = validate_redis_key("key\0value");
    assert!(result.is_err(), "Null byte injection should be rejected");

    // 攻击 5: 空键
    let result = validate_redis_key("");
    assert!(result.is_err(), "Empty key should be rejected");

    // 攻击 6: 过长的键
    let result = validate_redis_key(&"x".repeat(512 * 1024 + 1));
    assert!(result.is_err(), "Key exceeding 512KB should be rejected");

    // 攻击 7: 协议分隔符组合
    let result = validate_redis_key("key\r\n*3\r\n$3\r\nSET\r\n");
    assert!(
        result.is_err(),
        "Protocol injection attempt should be rejected"
    );

    // 合法键应该通过
    assert!(validate_redis_key("user:123").is_ok());
    assert!(validate_redis_key("cache:data:value").is_ok());
    assert!(validate_redis_key("test_key").is_ok());
    assert!(validate_redis_key("a:b:c:d:e").is_ok());

    println!("✓ All Redis key injection attempts were correctly rejected");
}

#[tokio::test]
async fn test_connection_string_redaction() {
    common::setup_logging();

    use oxcache::database::normalize_connection_string_with_redaction;

    // 测试 MySQL 连接字符串脱敏
    let mysql_with_pass = "mysql://user:password123@localhost:3306";

    println!("Testing: {}", mysql_with_pass);

    let result = normalize_connection_string_with_redaction(mysql_with_pass, true);
    println!("Result: {}", result);

    assert!(result.contains("****"), "Password should be redacted");
    assert!(
        !result.contains("password123"),
        "Original password should not be present"
    );
    assert!(result.contains("user"), "Username should be preserved");

    // 测试 PostgreSQL 连接字符串脱敏
    let postgres_with_pass = "postgres://admin:secret456@host:5432/db";
    let result = normalize_connection_string_with_redaction(postgres_with_pass, true);
    assert!(result.contains("****"), "Password should be redacted");

    // 测试不脱敏
    let result = normalize_connection_string_with_redaction(mysql_with_pass, false);
    assert!(
        result.contains("password123"),
        "Password should be preserved when redact=false"
    );

    // 测试 Redis 连接字符串脱敏
    let redis_with_pass = "redis://:mypassword@localhost:6379";
    println!("Testing Redis: {}", redis_with_pass);
    let result = normalize_connection_string_with_redaction(redis_with_pass, true);
    println!("Redis Result: {}", result);
    assert!(result.contains("****"), "Password should be redacted");

    println!("✓ Connection string redaction works correctly");
}
