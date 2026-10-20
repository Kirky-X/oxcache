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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_SECURITY_VALIDATION_1_EVERY_USER_SUPPLIED,
    MSG_EXAMPLE_SECURITY_VALIDATION_1_REDIS_KEY_VALIDATION,
    MSG_EXAMPLE_SECURITY_VALIDATION_2_EVERY_USER_SUBMITTED,
    MSG_EXAMPLE_SECURITY_VALIDATION_2_LUA_SCRIPT_VALIDATION,
    MSG_EXAMPLE_SECURITY_VALIDATION_3_EVERY_SCAN_PATTERN,
    MSG_EXAMPLE_SECURITY_VALIDATION_3_SCAN_PATTERN_VALIDATION,
    MSG_EXAMPLE_SECURITY_VALIDATION_4_EVERY_SCAN_COUNT,
    MSG_EXAMPLE_SECURITY_VALIDATION_4_SCAN_COUNT_LIMITS,
    MSG_EXAMPLE_SECURITY_VALIDATION_5_SAFE_LOGGING_LOG,
    MSG_EXAMPLE_SECURITY_VALIDATION_5_USE_LOG_CACHE,
    MSG_EXAMPLE_SECURITY_VALIDATION_6_IN_PRACTICE_A, MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED,
    MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED_SCRIPT,
    MSG_EXAMPLE_SECURITY_VALIDATION_BEST_PRACTICES,
    MSG_EXAMPLE_SECURITY_VALIDATION_COMMAND_INJECTION_CHARACTER_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_CONTROL_CHARACTER_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_DANGEROUS_COMMAND_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_ERROR, MSG_EXAMPLE_SECURITY_VALIDATION_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_KEY_COUNT_PASSED,
    MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_WILDCARD_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_INFINITE_LOOP_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_INVALID, MSG_EXAMPLE_SECURITY_VALIDATION_INVALID_2,
    MSG_EXAMPLE_SECURITY_VALIDATION_KEY, MSG_EXAMPLE_SECURITY_VALIDATION_LENGTH_AND_KEY_COUNT,
    MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_PATTERN_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_SCRIPT_PASSED,
    MSG_EXAMPLE_SECURITY_VALIDATION_OXCACHE_SECURITY_VALIDATION_EXAMPLE,
    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED, MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_2,
    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_3, MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_4,
    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_5, MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_6,
    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_7, MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_8,
    MSG_EXAMPLE_SECURITY_VALIDATION_PATH_TRAVERSAL_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_RAW_CLAMPED, MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_2, MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_3,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_4, MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_5,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_6, MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_7,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_PATTERN,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_SCRIPT,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_KEY_COUNT,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_WILDCARDS_10,
    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_SCRIPT,
    MSG_EXAMPLE_SECURITY_VALIDATION_SQL_INJECTION_PATTERN_DETECTION,
    MSG_EXAMPLE_SECURITY_VALIDATION_VALID, MSG_EXAMPLE_SECURITY_VALIDATION_VALID_2,
    MSG_EXAMPLE_SECURITY_VALIDATION_VALID_SCRIPT_PASSED_VALIDATION,
    MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUBMITTED,
    MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUPPLIED, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志（用于 log_cache_key 演示）
    tracing_subscriber::fmt::init();

    println!(
        "{}\n",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_OXCACHE_SECURITY_VALIDATION_EXAMPLE,
            &[]
        )
    );

    // Redis 键验证 - 防止命令注入和协议污染
    println!(
        "{}\n",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_1_REDIS_KEY_VALIDATION, &[])
    );

    // 合法键
    let valid_keys = ["user:123", "cache:data:value", "test_key", "api:v1:users"];
    for key in &valid_keys {
        match validate_redis_key(key) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_VALID,
                    &[("key", key.to_string())]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_INVALID,
                    &[("key", key.to_string()), ("e", e.to_string())]
                )
            ),
        }
    }

    // 非法键 - 命令注入字符
    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_COMMAND_INJECTION_CHARACTER_DETECTION,
            &[]
        )
    );
    let injection_keys = ["key;value", "key|cat", "key&whoami", "key`ls`"];
    for key in &injection_keys {
        match validate_redis_key(key) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED,
                    &[("key", key.to_string())]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED,
                    &[("key", key.to_string()), ("e", e.to_string())]
                )
            ),
        }
    }

    // 非法键 - SQL 注入模式
    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_SQL_INJECTION_PATTERN_DETECTION,
            &[]
        )
    );
    let sql_injection_keys = [
        "' OR '1'='1",
        "'; DROP TABLE--",
        "admin'--",
        "UNION SELECT * FROM users",
    ];
    for key in &sql_injection_keys {
        match validate_redis_key(key) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_2,
                    &[("key", key.to_string())]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_2,
                    &[("key", key.to_string()), ("e", e.to_string())]
                )
            ),
        }
    }

    // 非法键 - 路径遍历
    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_PATH_TRAVERSAL_DETECTION,
            &[]
        )
    );
    let traversal_keys = ["../etc/passwd", "..\\windows\\system32", "%2e%2e%2f"];
    for key in &traversal_keys {
        match validate_redis_key(key) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_3,
                    &[("key", key.to_string())]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_3,
                    &[("key", key.to_string()), ("e", e.to_string())]
                )
            ),
        }
    }

    // 非法键 - 控制字符（CR/LF/NULL）
    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_CONTROL_CHARACTER_DETECTION,
            &[]
        )
    );
    let control_keys = ["key\r\nvalue", "key\nvalue", "key\0value"];
    for key in &control_keys {
        match validate_redis_key(key) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_4,
                    &[("key", format!("{:?}", key))]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_4,
                    &[("key", format!("{:?}", key)), ("e", e.to_string())]
                )
            ),
        }
    }

    // Lua 脚本验证 - 防止危险命令和阻塞脚本
    println!(
        "\n{}\n",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_2_LUA_SCRIPT_VALIDATION, &[])
    );

    // 合法脚本
    let safe_script = "return redis.call('GET', KEYS[1])";
    match validate_lua_script(safe_script, 1) {
        Ok(()) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_VALID_SCRIPT_PASSED_VALIDATION,
                &[]
            )
        ),
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_ERROR,
                &[("e", e.to_string())]
            )
        ),
    }

    // 危险命令 - FLUSHALL
    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_DANGEROUS_COMMAND_DETECTION,
            &[]
        )
    );
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
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_5,
                    &[("desc", desc.to_string())]
                )
            ),
            Err(_) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_5,
                    &[("desc", desc.to_string())]
                )
            ),
        }
    }

    // 无限循环检测
    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_INFINITE_LOOP_DETECTION, &[])
    );
    let loop_scripts = ["while true do end", "while 1 do end", "repeat until false"];
    for script in &loop_scripts {
        match validate_lua_script(script, 0) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_6,
                    &[("script", format!("{:?}", script))]
                )
            ),
            Err(_) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_6,
                    &[("script", format!("{:?}", script))]
                )
            ),
        }
    }

    // 脚本长度和键数量限制
    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_LENGTH_AND_KEY_COUNT, &[])
    );
    // 超长脚本（>10KB）
    let too_long_script = "x".repeat(10 * 1024 + 1);
    match validate_lua_script(&too_long_script, 1) {
        Ok(()) => println!(
            "  {}",
            t(MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_SCRIPT_PASSED, &[])
        ),
        Err(_) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_SCRIPT,
                &[]
            )
        ),
    }
    // 键数量超限（>100）
    match validate_lua_script("return 1", 101) {
        Ok(()) => println!(
            "  {}",
            t(MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_KEY_COUNT_PASSED, &[])
        ),
        Err(_) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_KEY_COUNT,
                &[]
            )
        ),
    }

    // SCAN 模式验证 - 防止 ReDoS 攻击
    println!(
        "\n{}\n",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_3_SCAN_PATTERN_VALIDATION,
            &[]
        )
    );

    // 合法模式
    let valid_patterns = ["user:*", "session:*:data", "cache?", "*"];
    for pattern in &valid_patterns {
        match validate_scan_pattern(pattern) {
            Ok(()) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_VALID_2,
                    &[("pattern", pattern.to_string())]
                )
            ),
            Err(e) => println!(
                "  {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_INVALID_2,
                    &[("pattern", pattern.to_string()), ("e", e.to_string())]
                )
            ),
        }
    }

    // 过多通配符（>10 个）
    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_WILDCARD_DETECTION,
            &[]
        )
    );
    let excessive_wildcards = "*".repeat(11);
    match validate_scan_pattern(&excessive_wildcards) {
        Ok(()) => println!("  {}", t(MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_7, &[])),
        Err(_) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_WILDCARDS_10,
                &[]
            )
        ),
    }

    // 过长模式（>256 字符）
    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_PATTERN_DETECTION,
            &[]
        )
    );
    let too_long_pattern = "x".repeat(257);
    match validate_scan_pattern(&too_long_pattern) {
        Ok(()) => println!("  {}", t(MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_8, &[])),
        Err(_) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_PATTERN,
                &[]
            )
        ),
    }

    // SCAN count 限制 - 防止 DoS
    println!(
        "\n{}\n",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_4_SCAN_COUNT_LIMITS, &[])
    );

    let test_counts = [0, 1, 100, 500, 1000, 1500, 5000, usize::MAX];
    for count in &test_counts {
        let clamped = clamp_scan_count(*count);
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_RAW_CLAMPED,
                &[
                    ("count", format!("{:>6}", count)),
                    ("clamped", clamped.to_string())
                ]
            )
        );
    }

    // 安全日志记录 - 自动脱敏缓存键
    println!(
        "\n{}\n",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_5_SAFE_LOGGING_LOG, &[])
    );

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
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_SECURITY_VALIDATION_KEY,
                &[
                    ("level", level.to_string()),
                    ("message", message.to_string()),
                    ("key", key.to_string())
                ]
            )
        );
    }

    // 实际应用场景：安全验证流程
    println!(
        "\n{}\n",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_6_IN_PRACTICE_A, &[])
    );

    // 模拟用户输入的缓存键
    let user_inputs = [
        "user:profile:123",       // 合法
        "data; DROP TABLE users", // SQL 注入
        "session:abc:def",        // 合法
        "../etc/passwd",          // 路径遍历
        "cache:hit",              // 合法
    ];

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUPPLIED,
            &[]
        )
    );
    for input in &user_inputs {
        match validate_redis_key(input) {
            Ok(()) => println!(
                "    {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED,
                    &[("input", input.to_string())]
                )
            ),
            Err(_) => println!(
                "    {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_7,
                    &[("input", input.to_string())]
                )
            ),
        }
    }

    // 模拟 Lua 脚本验证流程
    println!(
        "\n  {}",
        t(
            MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUBMITTED,
            &[]
        )
    );
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
            Ok(()) => println!(
                "    {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED_SCRIPT,
                    &[("name", name.to_string())]
                )
            ),
            Err(_) => println!(
                "    {}",
                t(
                    MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_SCRIPT,
                    &[("name", name.to_string())]
                )
            ),
        }
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_EXAMPLE_COMPLETE, &[])
    );
    println!(
        "\n{}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_BEST_PRACTICES, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_1_EVERY_USER_SUPPLIED, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_2_EVERY_USER_SUBMITTED, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_3_EVERY_SCAN_PATTERN, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_4_EVERY_SCAN_COUNT, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_SECURITY_VALIDATION_5_USE_LOG_CACHE, &[])
    );

    Ok(())
}
