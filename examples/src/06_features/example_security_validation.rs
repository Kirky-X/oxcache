// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
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

use oxcache::{clamp_scan_count, log_cache_key, validate_lua_script, validate_redis_key, validate_scan_pattern};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志（用于 log_cache_key 演示）
    tracing_subscriber::fmt::init();

    println!("=== oxcache 安全验证示例 ===\n");

    // ========================================================================
    // 1. Redis 键验证 - 防止命令注入和协议污染
    // ========================================================================
    println!("--- 1. Redis 键验证（validate_redis_key） ---\n");

    // 1.1 合法键
    let valid_keys = ["user:123", "cache:data:value", "test_key", "api:v1:users"];
    for key in &valid_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ 合法: '{}'", key),
            Err(e) => println!("  ✗ 非法: '{}' -> {}", key, e),
        }
    }

    // 1.2 非法键 - 命令注入字符
    println!("\n--- 命令注入字符检测 ---");
    let injection_keys = ["key;value", "key|cat", "key&whoami", "key`ls`"];
    for key in &injection_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ 通过: '{}'", key),
            Err(e) => println!("  ✗ 拦截: '{}' -> {}", key, e),
        }
    }

    // 1.3 非法键 - SQL 注入模式
    println!("\n--- SQL 注入模式检测 ---");
    let sql_injection_keys = [
        "' OR '1'='1",
        "'; DROP TABLE--",
        "admin'--",
        "UNION SELECT * FROM users",
    ];
    for key in &sql_injection_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ 通过: '{}'", key),
            Err(e) => println!("  ✗ 拦截: '{}' -> {}", key, e),
        }
    }

    // 1.4 非法键 - 路径遍历
    println!("\n--- 路径遍历检测 ---");
    let traversal_keys = ["../etc/passwd", "..\\windows\\system32", "%2e%2e%2f"];
    for key in &traversal_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ 通过: '{}'", key),
            Err(e) => println!("  ✗ 拦截: '{}' -> {}", key, e),
        }
    }

    // 1.5 非法键 - 控制字符（CR/LF/NULL）
    println!("\n--- 控制字符检测 ---");
    let control_keys = ["key\r\nvalue", "key\nvalue", "key\0value"];
    for key in &control_keys {
        match validate_redis_key(key) {
            Ok(()) => println!("  ✓ 通过: {:?}", key),
            Err(e) => println!("  ✗ 拦截: {:?} -> {}", key, e),
        }
    }

    // ========================================================================
    // 2. Lua 脚本验证 - 防止危险命令和阻塞脚本
    // ========================================================================
    println!("\n--- 2. Lua 脚本验证（validate_lua_script） ---\n");

    // 2.1 合法脚本
    let safe_script = "return redis.call('GET', KEYS[1])";
    match validate_lua_script(safe_script, 1) {
        Ok(()) => println!("  ✓ 合法脚本通过验证"),
        Err(e) => println!("  ✗ 错误: {}", e),
    }

    // 2.2 危险命令 - FLUSHALL
    println!("\n--- 危险命令检测 ---");
    let dangerous_scripts = [
        ("return redis.call('FLUSHALL')", "FLUSHALL 清空所有数据"),
        ("return redis.call('FLUSHDB')", "FLUSHDB 清空当前数据库"),
        ("return redis.call('KEYS', '*')", "KEYS 阻塞 Redis"),
        ("return redis.call('SHUTDOWN')", "SHUTDOWN 关闭服务器"),
        ("return redis.call('CONFIG', 'GET', '*')", "CONFIG 获取敏感配置"),
    ];
    for (script, desc) in &dangerous_scripts {
        match validate_lua_script(script, 0) {
            Ok(()) => println!("  ✓ 通过: {}", desc),
            Err(_) => println!("  ✗ 拦截: {}", desc),
        }
    }

    // 2.3 无限循环检测
    println!("\n--- 无限循环检测 ---");
    let loop_scripts = ["while true do end", "while 1 do end", "repeat until false"];
    for script in &loop_scripts {
        match validate_lua_script(script, 0) {
            Ok(()) => println!("  ✓ 通过: {:?}", script),
            Err(_) => println!("  ✗ 拦截: {:?}", script),
        }
    }

    // 2.4 脚本长度和键数量限制
    println!("\n--- 长度和键数量限制 ---");
    // 超长脚本（>10KB）
    let too_long_script = "x".repeat(10 * 1024 + 1);
    match validate_lua_script(&too_long_script, 1) {
        Ok(()) => println!("  ✓ 超长脚本通过"),
        Err(_) => println!("  ✗ 拦截超长脚本（>10KB）"),
    }
    // 键数量超限（>100）
    match validate_lua_script("return 1", 101) {
        Ok(()) => println!("  ✓ 键数量超限通过"),
        Err(_) => println!("  ✗ 拦截键数量超限（>100）"),
    }

    // ========================================================================
    // 3. SCAN 模式验证 - 防止 ReDoS 攻击
    // ========================================================================
    println!("\n--- 3. SCAN 模式验证（validate_scan_pattern） ---\n");

    // 3.1 合法模式
    let valid_patterns = ["user:*", "session:*:data", "cache?", "*"];
    for pattern in &valid_patterns {
        match validate_scan_pattern(pattern) {
            Ok(()) => println!("  ✓ 合法: '{}'", pattern),
            Err(e) => println!("  ✗ 非法: '{}' -> {}", pattern, e),
        }
    }

    // 3.2 过多通配符（>10 个）
    println!("\n--- 过多通配符检测 ---");
    let excessive_wildcards = "*".repeat(11);
    match validate_scan_pattern(&excessive_wildcards) {
        Ok(()) => println!("  ✓ 通过"),
        Err(_) => println!("  ✗ 拦截过多通配符（>10 个）"),
    }

    // 3.3 过长模式（>256 字符）
    println!("\n--- 过长模式检测 ---");
    let too_long_pattern = "x".repeat(257);
    match validate_scan_pattern(&too_long_pattern) {
        Ok(()) => println!("  ✓ 通过"),
        Err(_) => println!("  ✗ 拦截过长模式（>256 字符）"),
    }

    // ========================================================================
    // 4. SCAN count 限制 - 防止 DoS
    // ========================================================================
    println!("\n--- 4. SCAN count 限制（clamp_scan_count） ---\n");

    let test_counts = [0, 1, 100, 500, 1000, 1500, 5000, usize::MAX];
    for count in &test_counts {
        let clamped = clamp_scan_count(*count);
        println!("  原始: {:>6} -> 限制后: {}", count, clamped);
    }

    // ========================================================================
    // 5. 安全日志记录 - 自动脱敏缓存键
    // ========================================================================
    println!("\n--- 5. 安全日志记录（log_cache_key） ---\n");

    // log_cache_key 会自动调用 redact_cache_key 对敏感键脱敏
    // 注意：实际输出在 tracing 日志中，这里展示调用方式
    let keys_to_log = [
        ("info", "缓存命中", "user:123"),
        ("debug", "缓存未命中", "user_token_abc123"),
        ("warn", "缓存键可疑", "api_key_secret"),
        ("error", "缓存访问失败", "password_reset_token"),
    ];

    for (level, message, key) in &keys_to_log {
        // 调用 log_cache_key 记录日志（输出到 tracing）
        log_cache_key(level, message, key);
        println!("  [{}] {} (键: {})", level, message, key);
    }

    // ========================================================================
    // 6. 实际应用场景：安全验证流程
    // ========================================================================
    println!("\n--- 6. 实际应用场景：安全验证流程 ---\n");

    // 模拟用户输入的缓存键
    let user_inputs = [
        "user:profile:123",       // 合法
        "data; DROP TABLE users", // SQL 注入
        "session:abc:def",        // 合法
        "../etc/passwd",          // 路径遍历
        "cache:hit",              // 合法
    ];

    println!("  验证用户输入的缓存键:");
    for input in &user_inputs {
        match validate_redis_key(input) {
            Ok(()) => println!("    ✓ 接受: '{}'", input),
            Err(_) => println!("    ✗ 拒绝: '{}'", input),
        }
    }

    // 模拟 Lua 脚本验证流程
    println!("\n  验证用户提交的 Lua 脚本:");
    let lua_scripts = [
        ("原子计数器", "return redis.call('INCR', KEYS[1])", 1),
        ("危险清空", "return redis.call('FLUSHALL')", 0),
        (
            "条件更新",
            "local v=redis.call('GET',KEYS[1]) if v then redis.call('SET',KEYS[1],ARGV[1]) end return v",
            1,
        ),
    ];
    for (name, script, key_count) in &lua_scripts {
        match validate_lua_script(script, *key_count) {
            Ok(()) => println!("    ✓ 接受脚本: {}", name),
            Err(_) => println!("    ✗ 拒绝脚本: {}", name),
        }
    }

    println!("\n✓ 示例完成");
    println!("\n安全验证最佳实践:");
    println!("  1. 所有用户输入的缓存键必须经过 validate_redis_key 验证");
    println!("  2. 所有用户提交的 Lua 脚本必须经过 validate_lua_script 验证");
    println!("  3. 所有 SCAN 操作的模式必须经过 validate_scan_pattern 验证");
    println!("  4. 所有 SCAN 操作的 count 必须经过 clamp_scan_count 限制");
    println!("  5. 日志中记录缓存键时使用 log_cache_key 自动脱敏");

    Ok(())
}
