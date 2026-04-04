// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 安全模块测试 - 验证安全验证功能

use oxcache::{clamp_scan_count, validate_lua_script, validate_redis_key, validate_scan_pattern};

#[test]
fn test_validate_redis_key_valid() {
    assert!(validate_redis_key("user:123").is_ok());
    assert!(validate_redis_key("cache:data:value").is_ok());
    assert!(validate_redis_key("test_key").is_ok());
}

#[test]
fn test_validate_redis_key_invalid_cases() {
    // 测试空键
    assert!(validate_redis_key("").is_err());

    // 测试包含危险字符的键
    assert!(validate_redis_key("key\r\n").is_err());
    assert!(validate_redis_key("key\nvalue").is_err());
    assert!(validate_redis_key("key\0value").is_err());
}

#[test]
fn test_validate_lua_script_valid() {
    let script = "return redis.call('GET', KEYS[1])";
    assert!(validate_lua_script(script, 1).is_ok());
}

#[test]
fn test_validate_lua_script_invalid_cases() {
    // 测试过长脚本
    let long_script = "x".repeat(10 * 1024 + 1); // 超过10KB
    assert!(validate_lua_script(&long_script, 1).is_err());

    // 测试包含危险命令的脚本
    let dangerous_script = "return redis.call('FLUSHALL')";
    assert!(validate_lua_script(dangerous_script, 0).is_err());

    // 测试键数量过多
    assert!(validate_lua_script("return redis.call('GET', KEYS[1])", 101).is_err());
}

#[test]
fn test_validate_scan_pattern_valid() {
    assert!(validate_scan_pattern("user:*").is_ok());
    assert!(validate_scan_pattern("session:*:data").is_ok());
}

#[test]
fn test_validate_scan_pattern_invalid_cases() {
    // 测试过长模式
    let long_pattern = "x".repeat(257); // 超过256字符
    assert!(validate_scan_pattern(&long_pattern).is_err());

    // 测试过多通配符
    let many_wildcards = "*".repeat(11); // 超过10个通配符
    assert!(validate_scan_pattern(&many_wildcards).is_err());
}

#[test]
fn test_clamp_scan_count() {
    // 测试边界值
    assert_eq!(clamp_scan_count(0), 1); // 最小值
    assert_eq!(clamp_scan_count(500), 500); // 中间值
    assert_eq!(clamp_scan_count(1000), 1000); // 最大值
    assert_eq!(clamp_scan_count(2000), 1000); // 超过最大值
}
