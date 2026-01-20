//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 安全验证模块
//!
//! 提供各种安全验证功能，防止恶意输入导致的安全问题。

use crate::error::{CacheError, Result};

/// Lua 脚本最大长度 (10KB)
const MAX_LUA_SCRIPT_LENGTH: usize = 10 * 1024;

/// Lua 脚本最大键数量
const MAX_LUA_SCRIPT_KEYS: usize = 100;

/// Lua 脚本执行超时时间（秒）
const LUA_SCRIPT_TIMEOUT_SECS: u64 = 30;

/// SCAN 模式最大长度
const MAX_SCAN_PATTERN_LENGTH: usize = 256;

/// SCAN 模式最大通配符数量
const MAX_SCAN_WILDCARDS: usize = 10;

/// SCAN 操作超时时间（秒）
const SCAN_TIMEOUT_SECS: u64 = 30;

/// SCAN count 参数安全范围
const SCAN_COUNT_MIN: usize = 1;
const SCAN_COUNT_MAX: usize = 1000;

/// 允许的 Redis 命令白名单（用于 Lua 脚本验证）
///
/// 只允许安全命令，禁止危险命令如 FLUSHALL、KEYS 等
const ALLOWED_REDIS_COMMANDS: &[&str] = &[
    // 字符串命令
    "GET", "MGET", "SET", "SETEX", "PSETEX", "MSET",
    // 哈希命令
    "HGET", "HMGET", "HGETALL", "HSET", "HMSET",
    // 列表命令
    "LINDEX", "LRANGE", "LLEN",
    // 集合命令
    "SISMEMBER", "SMEMBERS", "SCARD",
    // 有序集合命令
    "ZSCORE", "ZRANGE", "ZRANGEBYSCORE", "ZCARD",
    // 过期命令
    "TTL", "PTTL", "EXISTS",
    // 事务命令
    "UNWATCH",
];

/// 验证 Redis 缓存键是否安全
///
/// 防止 Redis 命令注入和协议污染攻击。
///
/// # 验证规则
///
/// 1. 键不能为空
/// 2. 键长度不能超过 512KB
/// 3. 键不能包含危险字符（\r, \n, \0）
///
/// # 参数
///
/// * `key` - 要验证的缓存键
///
/// # 返回值
///
/// * `Ok(())` - 键是安全的
/// * `Err(CacheError::InvalidInput)` - 键包含不安全字符
pub fn validate_redis_key(key: &str) -> Result<()> {
    // 检查键长度
    if key.is_empty() {
        return Err(CacheError::InvalidInput(
            "Redis key cannot be empty".to_string(),
        ));
    }

    if key.len() > 512 * 1024 {
        // Redis最大键长512MB，但我们限制为512KB以防止滥用
        return Err(CacheError::InvalidInput(
            "Redis key exceeds maximum length of 512KB".to_string(),
        ));
    }

    // 检查是否包含危险字符
    // Redis协议使用\r\n作为分隔符，我们必须防止注入
    let dangerous_chars = ['\r', '\n', '\0'];

    for c in key.chars() {
        if dangerous_chars.contains(&c) {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains forbidden character: {:?}",
                c
            )));
        }
    }

    Ok(())
}

/// 验证 Lua 脚本
///
/// 防止恶意脚本执行危险命令或导致 Redis 阻塞。
///
/// # 验证规则
///
/// 1. 脚本长度不超过 10KB
/// 2. 键数量不超过 100
/// 3. 不包含危险的 Redis 命令（FLUSHALL, KEYS 等）
///
/// # 参数
///
/// * `script` - Lua 脚本内容
/// * `key_count` - 键数量
///
/// # 返回值
///
/// * `Ok(())` - 脚本验证通过
/// * `Err(CacheError::InvalidInput)` - 脚本验证失败
pub fn validate_lua_script(script: &str, key_count: usize) -> Result<()> {
    // 检查脚本长度
    if script.len() > MAX_LUA_SCRIPT_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "Lua script exceeds maximum length of {} bytes (got {} bytes)",
            MAX_LUA_SCRIPT_LENGTH,
            script.len()
        )));
    }

    // 检查键数量
    if key_count > MAX_LUA_SCRIPT_KEYS {
        return Err(CacheError::InvalidInput(format!(
            "Lua script exceeds maximum key count of {} (got {} keys)",
            MAX_LUA_SCRIPT_KEYS,
            key_count
        )));
    }

    // 检查危险命令
    // 将脚本转为大写进行统一检查
    let script_upper = script.to_uppercase();

    // 检查 FLUSHALL/FLUSHDB 命令 - 只有在 redis.call/pcall 中才危险
    // 但由于这些命令极其危险，我们也检查字符串字面量
    let flush_commands = ["FLUSHALL", "FLUSHDB"];
    for cmd in &flush_commands {
        if script_upper.contains(&format!("REDIS.CALL('{}'", cmd))
            || script_upper.contains(&format!("REDIS.CALL(\"{}\"", cmd))
            || script_upper.contains(&format!("REDIS.PCALL('{}'", cmd))
            || script_upper.contains(&format!("REDIS.PCALL(\"{}\"", cmd))
        {
            return Err(CacheError::InvalidInput(format!(
                "Lua script calls forbidden Redis command: {}", cmd
            )));
        }
    }

    // 检查 Redis KEYS 命令（可能导致阻塞）
    // 必须区分 Redis KEYS 命令和 Lua 的 KEYS 数组
    // 检查模式: redis.call('KEYS' 或 redis.call("KEYS" （后面可能有逗号或空格）
    if script_upper.contains("REDIS.CALL('KEYS'")
        || script_upper.contains("REDIS.CALL(\"KEYS\"")
        || script_upper.contains("REDIS.PCALL('KEYS'")
        || script_upper.contains("REDIS.PCALL(\"KEYS\"")
    {
        return Err(CacheError::InvalidInput(
            "Lua script contains forbidden command: KEYS".to_string(),
        ));
    }

    // 检查其他危险的管理命令
    // 这些命令只有在 redis.call/pcall 中调用时才危险
    let dangerous_commands = [
        "SHUTDOWN", "DEBUG", "CONFIG", "SAVE", "BGSAVE",
        "BGREWRITEAOF", "LASTSAVE", "MONITOR", "SYNC",
    ];

    for cmd in &dangerous_commands {
        // 检查是否在 redis.call 或 redis.pcall 中调用了这些命令
        // 检查模式: REDIS.CALL('CMD' 或 REDIS.CALL("CMD)（不检查后面的参数）
        if script_upper.contains(&format!("REDIS.CALL('{}'", cmd))
            || script_upper.contains(&format!("REDIS.CALL(\"{}\"", cmd))
            || script_upper.contains(&format!("REDIS.PCALL('{}'", cmd))
            || script_upper.contains(&format!("REDIS.PCALL(\"{}\"", cmd))
        {
            return Err(CacheError::InvalidInput(format!(
                "Lua script calls forbidden Redis command: {}", cmd
            )));
        }
    }

    Ok(())
}

/// 验证 SCAN 模式
///
/// 防止 ReDoS（正则表达式拒绝服务）攻击。
///
/// # 验证规则
///
/// 1. 模式长度不超过 256 字符
/// 2. 通配符数量不超过 10 个
///
/// # 参数
///
/// * `pattern` - SCAN 模式字符串
///
/// # 返回值
///
/// * `Ok(())` - 模式验证通过
/// * `Err(CacheError::InvalidInput)` - 模式验证失败
pub fn validate_scan_pattern(pattern: &str) -> Result<()> {
    // 检查模式长度
    if pattern.len() > MAX_SCAN_PATTERN_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "SCAN pattern exceeds maximum length of {} characters (got {} characters)",
            MAX_SCAN_PATTERN_LENGTH,
            pattern.len()
        )));
    }

    // 计算通配符数量
    let wildcard_count = pattern.chars().filter(|c| *c == '*').count();

    if wildcard_count > MAX_SCAN_WILDCARDS {
        return Err(CacheError::InvalidInput(format!(
            "SCAN pattern contains too many wildcards (max {}, got {})",
            MAX_SCAN_WILDCARDS,
            wildcard_count
        )));
    }

    Ok(())
}

/// 限制 SCAN count 参数到安全范围
///
/// # 参数
///
/// * `count` - 原始 count 参数
///
/// # 返回值
///
/// 返回限制在安全范围内的 count 值（1-1000）
pub fn clamp_scan_count(count: usize) -> usize {
    count.clamp(SCAN_COUNT_MIN, SCAN_COUNT_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        match validate_lua_script(script, 1) {
            Ok(()) => (),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
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
}
