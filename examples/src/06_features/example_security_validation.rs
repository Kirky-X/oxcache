// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 安全验证示例
//!
//! 本示例演示 oxcache 的输入安全验证功能，防止恶意输入导致的安全问题：
//! - Redis 键验证（防命令注入、SQL 注入、路径遍历）
//! - Lua 脚本验证（防危险命令、无限循环）
//! - SCAN 模式验证（防 ReDoS 攻击）
//! - SCAN count 限制（防 DoS）
//! - 安全日志记录（自动脱敏缓存键）
//!
//! 与 `example_security.rs` 的区别：
//! - `example_security.rs` 演示**输出脱敏**（redact_value、redact_connection_string 等）
//! - 本示例演示**输入验证**（validate_redis_key、validate_lua_script 等）
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_security_validation
//! ```

use oxcache::{
    clamp_scan_count, log_cache_key, validate_lua_script, validate_redis_key, validate_scan_pattern,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志（用于 log_cache_key 演示）
    tracing_subscriber::fmt::init();

    println!("=== oxcache security validation example ===\n");

    // === Redis 键验证 - 防止命令注入和协议污染 ===
    println!("--- 1. Redis key validation (validate_redis_key) ---\n");

    // 合法键
    let valid_keys = ["user:123", "cache:data:value", "test_key", "api:v1:users"];
    for key in &valid_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ valid: '{}'", key),
            Err(e) => println!("  ✗ invalid: '{}' -> {}", key, e),
        }
    }

    // 非法键 - 命令注入字符
    println!("\n--- Command injection character detection ---");
    let injection_keys = ["key;value", "key|cat", "key&whoami", "key`ls`"];
    for key in &injection_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ passed: '{}'", key),
            Err(e) => println!("  ✗ rejected: '{}' -> {}", key, e),
        }
    }

    // 非法键 - SQL 注入模式
    println!("\n--- SQL injection pattern detection ---");
    let sql_injection_keys = [
        "' OR '1'='1",
        "'; DROP TABLE--",
        "admin'--",
        "UNION SELECT * FROM users",
    ];
    for key in &sql_injection_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ passed: '{}'", key),
            Err(e) => println!("  ✗ rejected: '{}' -> {}", key, e),
        }
    }

    // 非法键 - 路径遍历
    println!("\n--- Path traversal detection ---");
    let traversal_keys = ["../etc/passwd", "..\\windows\\system32", "%2e%2e%2f"];
    for key in &traversal_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ passed: '{}'", key),
            Err(e) => println!("  ✗ rejected: '{}' -> {}", key, e),
        }
    }

    // 非法键 - 控制字符（CR/LF/NULL）
    println!("\n--- Control character detection ---");
    let control_keys = ["key\r\nvalue", "key\nvalue", "key\0value"];
    for key in &control_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ passed: {:?}", key),
            Err(e) => println!("  ✗ rejected: {:?} -> {}", key, e),
        }
    }

    // === Lua 脚本验证 - 防止危险命令和阻塞脚本 ===
    println!("\n--- 2. Lua script validation (validate_lua_script) ---\n");

    // 合法脚本
    let safe_script = "return redis.call('GET', KEYS[1])";
    match validate_lua_script(safe_script, 1) {
        Ok(()) => println!("  ✓ valid script passed validation"),
        Err(e) => println!("  ✗ error: {}", e),
    }

    // 危险命令 - FLUSHALL
    println!("\n--- Dangerous command detection ---");
    let dangerous_scripts = [
        ("return redis.call('FLUSHALL')", "FLUSHALL flushes all data"),
        (
            "return redis.call('FLUSHDB')",
            "FLUSHDB flushes the current database",
        ),
        ("return redis.call('KEYS', '*')", "KEYS blocks Redis"),
        (
            "return redis.call('SHUTDOWN')",
            "SHUTDOWN shuts the server down",
        ),
        (
            "return redis.call('CONFIG', 'GET', '*')",
            "CONFIG exposes sensitive configuration",
        ),
    ];
    for (script, desc) in &dangerous_scripts {
        match validate_lua_script(script, 0) {
            Ok(()) => println!("  ✓ passed: {}", desc),
            Err(_) => println!("  ✗ rejected: {}", desc),
        }
    }

    // 无限循环检测
    println!("\n--- Infinite loop detection ---");
    let loop_scripts = ["while true do end", "while 1 do end", "repeat until false"];
    for script in &loop_scripts {
        match validate_lua_script(script, 0) {
            Ok(()) => println!("  ✓ passed: {:?}", script),
            Err(_) => println!("  ✗ rejected: {:?}", script),
        }
    }

    // 脚本长度和键数量限制
    println!("\n--- Length and key-count limits ---");
    // 超长脚本（>10KB）
    let too_long_script = "x".repeat(10 * 1024 + 1);
    match validate_lua_script(&too_long_script, 1) {
        Ok(()) => println!("  ✓ oversized script passed"),
        Err(_) => println!("  ✗ rejected an oversized script (>10KB)"),
    }
    // 键数量超限（>100）
    match validate_lua_script("return 1", 101) {
        Ok(()) => println!("  ✓ excess key count passed"),
        Err(_) => println!("  ✗ rejected excess key count (>100)"),
    }

    // === SCAN 模式验证 - 防止 ReDoS 攻击 ===
    println!("\n--- 3. SCAN pattern validation (validate_scan_pattern) ---\n");

    // 合法模式
    let valid_patterns = ["user:*", "session:*:data", "cache?", "*"];
    for pattern in &valid_patterns {
        match validate_scan_pattern(pattern) {
            Ok(()) => println!("  ✓ valid: '{}'", pattern),
            Err(e) => println!("  ✗ invalid: '{}' -> {}", pattern, e),
        }
    }

    // 过多通配符（>10 个）
    println!("\n--- Excess wildcard detection ---");
    let excessive_wildcards = "*".repeat(11);
    match validate_scan_pattern(&excessive_wildcards) {
        Ok(()) => println!("  ✓ passed"),
        Err(_) => println!("  ✗ rejected excess wildcards (>10)"),
    }

    // 过长模式（>256 字符）
    println!("\n--- Oversized pattern detection ---");
    let too_long_pattern = "x".repeat(257);
    match validate_scan_pattern(&too_long_pattern) {
        Ok(()) => println!("  ✓ passed"),
        Err(_) => println!("  ✗ rejected an oversized pattern (>256 characters)"),
    }

    // === SCAN count 限制 - 防止 DoS ===
    println!("\n--- 4. SCAN count limits (clamp_scan_count) ---\n");

    let test_counts = [0, 1, 100, 500, 1000, 1500, 5000, usize::MAX];
    for count in &test_counts {
        let clamped = clamp_scan_count(*count);
        println!("  raw: {:>6} -> clamped: {}", count, clamped);
    }

    // === 安全日志记录 - 自动脱敏缓存键 ===
    println!("\n--- 5. Safe logging (log_cache_key) ---\n");

    // log_cache_key 会自动调用 redact_cache_key 对敏感键脱敏
    // 注意：实际输出在 tracing 日志中，这里展示调用方式
    let keys_to_log = [
        ("info", "cache hit", "user:123"),
        ("debug", "cache miss", "user_token_abc123"),
        ("warn", "suspicious cache key", "api_key_secret"),
        ("error", "cache access failed", "password_reset_token"),
    ];

    for (level, message, key) in &keys_to_log {
        // 调用 log_cache_key 记录日志（输出到 tracing）
        log_cache_key(level, message, key);
        println!("  [{}] {} (key: {})", level, message, key);
    }

    // === 实际应用场景：安全验证流程 ===
    println!("\n--- 6. In practice: a safe validation flow ---\n");

    // 模拟用户输入的缓存键
    let user_inputs = [
        "user:profile:123",       // 合法
        "data; DROP TABLE users", // SQL 注入
        "session:abc:def",        // 合法
        "../etc/passwd",          // 路径遍历
        "cache:hit",              // 合法
    ];

    println!("  validating a user-supplied cache key:");
    for input in &user_inputs {
        match validate_redis_key(input) {
            Ok(()) => println!("    ✓ accepted: '{}'", input),
            Err(_) => println!("    ✗ rejected: '{}'", input),
        }
    }

    // 模拟 Lua 脚本验证流程
    println!("\n  validating a user-submitted Lua script:");
    let lua_scripts = [
        ("atomic counter", "return redis.call('INCR', KEYS[1])", 1),
        ("dangerous flush", "return redis.call('FLUSHALL')", 0),
        (
            "conditional update",
            "local v=redis.call('GET',KEYS[1]) if v then redis.call('SET',KEYS[1],ARGV[1]) end return v",
            1,
        ),
    ];
    for (name, script, key_count) in &lua_scripts {
        match validate_lua_script(script, *key_count) {
            Ok(()) => println!("    ✓ accepted script: {}", name),
            Err(_) => println!("    ✗ rejected script: {}", name),
        }
    }

    println!("\n✓ Example complete");
    println!("\nSecurity validation best practices:");
    println!("  1. every user-supplied cache key must pass validate_redis_key");
    println!("  2. every user-submitted Lua script must pass validate_lua_script");
    println!("  3. every SCAN pattern must pass validate_scan_pattern");
    println!("  4. every SCAN count must be clamped with clamp_scan_count");
    println!("  5. use log_cache_key to redact cache keys automatically in logs");

    Ok(())
}
