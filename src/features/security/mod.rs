//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 安全验证模块
//!
//! 提供各种安全验证功能，防止恶意输入导致的安全问题。
//!
//! # 主要功能
//!
//! - Redis 键验证 - 防止命令注入和协议污染
//! - Lua 脚本验证 - 防止危险命令和阻塞脚本
//! - SCAN 模式验证 - 防止恶意通配符导致性能问题
//! - SQL/命令注入检测
//! - 路径遍历检测
//! - 数据脱敏
//! - 安全日志
//!
//! # 注意
//!
//! 这些验证函数是内部 API，仅供 crate 内部使用。
//! 外部用户应通过缓存 API 的安全封装来受益于这些验证。

#![allow(unused_doc_comments)]

// ============================================================================
// Submodules
// ============================================================================

pub mod injection;
pub mod log;
pub mod path;
pub mod redaction;
pub mod redis;
pub mod regex;
pub mod validation;

// ============================================================================
// Re-exports for convenience (used by external tests via lib.rs re-exports)
// ============================================================================

#[allow(unused_imports)]
pub use log::{log_cache_key, sanitize_message};

#[allow(unused_imports)]
pub use redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted};

#[allow(unused_imports)]
pub use regex::{compile_glob_pattern, compile_regex, glob_to_regex, match_safe};

#[allow(unused_imports)]
pub use validation::{validate_max_length, validate_no_dangerous_chars, validate_not_empty};

// Redis/Lua validation exports
pub use redis::{clamp_scan_count, validate_lua_script, validate_redis_key, validate_scan_pattern};

// ============================================================================
// Tests (moved from redis.rs for integration)
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use redis::{
        MAX_LUA_SCRIPT_KEYS, MAX_LUA_SCRIPT_LENGTH, MAX_SCAN_PATTERN_LENGTH, MAX_SCAN_WILDCARDS, SCAN_COUNT_MAX,
        SCAN_COUNT_MIN,
    };

    // ============================================================================
    // Redis 键验证测试
    // ============================================================================

    #[test]
    fn test_validate_redis_key_valid() {
        assert!(validate_redis_key("user:123").is_ok());
        assert!(validate_redis_key("cache:data:value").is_ok());
        assert!(validate_redis_key("test_key").is_ok());
    }

    #[test]
    fn test_validate_redis_key_empty() {
        let result = validate_redis_key("");
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_redis_key_too_long() {
        let key = "x".repeat(512 * 1024 + 1);
        let result = validate_redis_key(&key);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_redis_key_contains_crlf() {
        assert!(validate_redis_key("key\r\n").is_err());
        assert!(validate_redis_key("key\rvalue").is_err());
        assert!(validate_redis_key("key\nvalue").is_err());
    }

    #[test]
    fn test_validate_redis_key_contains_null() {
        assert!(validate_redis_key("key\0value").is_err());
    }

    // ============================================================================
    // Lua 脚本验证测试
    // ============================================================================

    #[test]
    fn test_validate_lua_script_valid() {
        let script = "return redis.call('GET', KEYS[1])";
        assert!(
            validate_lua_script(script, 1).is_ok(),
            "Valid Lua script should not return error"
        );
    }

    #[test]
    fn test_validate_lua_script_too_long() {
        let script = "x".repeat(MAX_LUA_SCRIPT_LENGTH + 1);
        let result = validate_lua_script(&script, 1);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_lua_script_too_many_keys() {
        let script = "return redis.call('GET', KEYS[1])";
        let result = validate_lua_script(script, MAX_LUA_SCRIPT_KEYS + 1);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_lua_script_flushall() {
        let script = "return redis.call('FLUSHALL')";
        let result = validate_lua_script(script, 0);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_lua_script_flushdb() {
        let script = "return redis.call('FLUSHDB')";
        let result = validate_lua_script(script, 0);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_lua_script_keys_command() {
        let script = "return redis.call('KEYS', '*')";
        let result = validate_lua_script(script, 0);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_lua_script_shutdown() {
        let script = "return redis.call('SHUTDOWN')";
        let result = validate_lua_script(script, 0);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_lua_script_case_insensitive() {
        let script = "return redis.call('flushall')";
        let result = validate_lua_script(script, 0);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_lua_script_safe_commands() {
        let script = r#"
            local val = redis.call('GET', KEYS[1])
            if val then
                redis.call('SETEX', KEYS[2], 60, val)
            end
            return val
        "#;
        assert!(validate_lua_script(script, 2).is_ok());
    }

    // ============================================================================
    // SCAN 模式验证测试
    // ============================================================================

    #[test]
    fn test_validate_scan_pattern_valid() {
        assert!(validate_scan_pattern("user:*").is_ok());
        assert!(validate_scan_pattern("session:*:data").is_ok());
        assert!(validate_scan_pattern("cache?").is_ok());
    }

    #[test]
    fn test_validate_scan_pattern_too_long() {
        let pattern = "x".repeat(MAX_SCAN_PATTERN_LENGTH + 1);
        let result = validate_scan_pattern(&pattern);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_scan_pattern_too_many_wildcards() {
        let pattern = "*".repeat(MAX_SCAN_WILDCARDS + 1);
        let result = validate_scan_pattern(&pattern);
        assert!(result.is_err());
        assert!(matches!(result, Err(CacheError::InvalidInput(_))));
    }

    #[test]
    fn test_validate_scan_pattern_exact_wildcard_limit() {
        let pattern = "*".repeat(MAX_SCAN_WILDCARDS);
        assert!(validate_scan_pattern(&pattern).is_ok());
    }

    #[test]
    fn test_clamp_scan_count() {
        assert_eq!(clamp_scan_count(0), SCAN_COUNT_MIN);
        assert_eq!(clamp_scan_count(500), 500);
        assert_eq!(clamp_scan_count(1000), SCAN_COUNT_MAX);
        assert_eq!(clamp_scan_count(2000), SCAN_COUNT_MAX);
    }

    // ============================================================================
    // 边界测试 - SQL 注入检测
    // ============================================================================

    #[test]
    fn test_sql_injection_patterns() {
        // 真正的 SQL 注入尝试应该被检测
        assert!(validate_redis_key("' OR '1'='1").is_err());
        assert!(validate_redis_key("'; DROP TABLE--").is_err());
        assert!(validate_redis_key("1 OR 1=1").is_err());
    }

    #[test]
    fn test_sql_injection_false_positive_prevention() {
        // 正常的键名不应该被误判
        assert!(validate_redis_key("order_status").is_ok());
        assert!(validate_redis_key("user_data_123").is_ok());
        assert!(validate_redis_key("api_response").is_ok());
    }

    #[test]
    fn test_command_injection_patterns() {
        // 命令注入尝试应该被检测（长键会触发检测）
        assert!(validate_redis_key("some_long_key_name;ls").is_err());
        assert!(validate_redis_key("some_long_key_name|cat").is_err());
        assert!(validate_redis_key("some_long_key_name&whoami").is_err());
        // 单独的危险字符也会被检测
        assert!(validate_redis_key("key;value").is_err());
        assert!(validate_redis_key("key|value").is_err());
    }

    #[test]
    fn test_path_traversal_patterns() {
        // 路径遍历尝试应该被检测
        assert!(validate_redis_key("../etc/passwd").is_err());
        assert!(validate_redis_key("..\\windows\\system32").is_err());
    }

    #[test]
    fn test_unicode_control_characters() {
        // Unicode 控制字符应该被检测
        assert!(validate_redis_key("key\u{0001}value").is_err());
        assert!(validate_redis_key("key\u{007F}value").is_err());
    }

    #[test]
    fn test_lua_script_edge_cases() {
        // Lua 脚本边界测试
        let script = "return 1";
        assert!(validate_lua_script(script, 0).is_ok());

        // 空的 Lua 脚本应该被允许（语法上有效，返回 nil）
        let script = "";
        assert!(validate_lua_script(script, 0).is_ok());

        // FLUSHALL 应该被检测
        let script = "return redis.call('FLUSHALL')";
        assert!(validate_lua_script(script, 0).is_err());
    }

    #[test]
    fn test_lua_script_comment_bypass_prevention() {
        // 测试通过注释绕过检测的防护
        let script = "--[[ FLUSHALL ]] return 1";
        assert!(validate_lua_script(script, 0).is_ok());

        let script = "return 1 -- FLUSHALL";
        assert!(validate_lua_script(script, 0).is_ok());
    }

    #[test]
    fn test_scan_pattern_edge_cases() {
        // SCAN 模式边界测试
        assert!(validate_scan_pattern("*").is_ok());
        assert!(validate_scan_pattern("?").is_ok());
        assert!(validate_scan_pattern("[a-z]").is_ok());
        // 空模式
        assert!(validate_scan_pattern("").is_ok());
    }

    #[test]
    fn test_redis_key_max_length_boundary() {
        // 精确的边界测试：512KB 是最大值
        let max_key = "x".repeat(512 * 1024);
        assert!(validate_redis_key(&max_key).is_ok());

        let over_max_key = "x".repeat(512 * 1024 + 1);
        assert!(validate_redis_key(&over_max_key).is_err());
    }

    #[test]
    fn test_lua_script_max_length_boundary() {
        // 精确的边界测试：10KB 是最大值
        let max_script = "x".repeat(MAX_LUA_SCRIPT_LENGTH);
        assert!(validate_lua_script(&max_script, 1).is_ok());

        let over_max_script = "x".repeat(MAX_LUA_SCRIPT_LENGTH + 1);
        assert!(validate_lua_script(&over_max_script, 1).is_err());
    }
}

// 测试辅助模块 - 为集成测试提供访问
// 注意：这些函数仅供测试使用，生产代码应使用公共 API
#[cfg(any(test, feature = "testing"))]
#[allow(unused_imports)]
pub mod test_helpers {
    pub use super::{clamp_scan_count, validate_lua_script, validate_redis_key, validate_scan_pattern};
}
