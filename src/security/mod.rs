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
//!
//! # 注意
//!
//! 这些验证函数是内部 API，仅供 crate 内部使用。
//! 外部用户应通过缓存 API 的安全封装来受益于这些验证。

#![allow(unused_doc_comments)]

// Submodules
pub mod log;
pub mod redaction;
pub mod regex;
pub mod validation;

// Re-exports for convenience (used by external tests via lib.rs re-exports)
#[allow(unused_imports)]
pub use log::{log_cache_key, sanitize_message};
#[allow(unused_imports)]
pub use redaction::{redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted};
#[allow(unused_imports)]
pub use regex::{compile_glob_pattern, compile_regex, glob_to_regex, match_safe};
#[allow(unused_imports)]
pub use validation::{validate_max_length, validate_no_dangerous_chars, validate_not_empty};

use crate::error::{CacheError, Result};

/// Lua 脚本最大长度 (10KB)
const MAX_LUA_SCRIPT_LENGTH: usize = 10 * 1024;

/// Lua 脚本最大键数量
const MAX_LUA_SCRIPT_KEYS: usize = 100;

/// SCAN 模式最大长度
const MAX_SCAN_PATTERN_LENGTH: usize = 256;

/// SCAN 模式最大通配符数量
const MAX_SCAN_WILDCARDS: usize = 10;

/// SCAN count 参数安全范围
const SCAN_COUNT_MIN: usize = 1;
const SCAN_COUNT_MAX: usize = 1000;

// ============================================================================
// 预编译的正则表达式
// ============================================================================

/// Lua 无限循环检测正则模式
static LUA_LOOP_PATTERNS: &[(&str, &str)] = &[
    (r"WHILE\s+TRUE", "WHILE TRUE 循环"),
    (r"WHILE\s+1", "WHILE 1 循环"),
    (r"REPEAT", "REPEAT 循环"),
    (r"GOTO", "GOTO 语句"),
];

/// 预编译的 Lua 循环检测正则
lazy_static::lazy_static! {
    static ref LUA_LOOP_REGEXES: Vec<::regex::Regex> = {
        LUA_LOOP_PATTERNS
            .iter()
            .map(|(pattern, _)| ::regex::Regex::new(pattern).expect("Invalid loop pattern regex"))
            .collect()
    };
}

/// 空白字符替换正则
lazy_static::lazy_static! {
    static ref WHITESPACE_REGEX: ::regex::Regex = ::regex::Regex::new(r"\s+").expect("Invalid whitespace regex");
}

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
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn validate_redis_key(key: &str) -> Result<()> {
    // 基础验证：使用共享验证工具
    use crate::security::validation::redis::{DANGEROUS_CHARS, MAX_KEY_LENGTH};
    use crate::security::validation::{validate_max_length, validate_no_dangerous_chars, validate_not_empty};

    validate_not_empty(key, "Redis key")?;
    validate_max_length(key, MAX_KEY_LENGTH, "Redis key")?;
    validate_no_dangerous_chars(key, &DANGEROUS_CHARS, "Redis key")?;

    // ========== 安全增强 ==========

    // 检查 Unicode 控制字符（CR, LF, NULL 已在基础验证中检查）
    for c in key.chars() {
        if c.is_control() && !DANGEROUS_CHARS.contains(&c) && c != '\t' {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains control character: U+{:04X}",
                c as u32
            )));
        }
    }

    // 检查 SQL 注入模式
    // 注意：这些模式用于检测潜在的 SQL 注入攻击，
    // 但在 Redis 键验证上下文中可能产生误报
    // 因此我们只保留明确的 SQL 注入签名模式
    const SQL_INJECTION_PATTERNS: &[(&str, &str)] = &[
        ("' OR '", "单引号后跟 OR 模式"),
        ("'--", "SQL 注释模式"),
        ("'; DROP", "SQL DROP 语句"),
        ("'; DELETE", "SQL DELETE 语句"),
        ("'; INSERT", "SQL INSERT 语句"),
        ("UNION SELECT", "SQL UNION 查询"),
        ("xp_cmdshell", "SQL Server 命令执行"),
        ("' OR '1'='1", "经典 SQL 注入永真条件"),
        ("admin'--", "SQL 认证绕过"),
    ];

    let key_upper = key.to_uppercase();
    for (pattern, description) in SQL_INJECTION_PATTERNS {
        if key_upper.contains(&pattern.to_uppercase()) {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains suspicious SQL injection pattern: {}",
                description
            )));
        }
    }

    // 检查路径遍历模式
    const PATH_TRAVERSAL_PATTERNS: &[&str] = &[
        "../",
        "..\\",
        "%2e%2e",
        "%252e%252e",
        "..%2f",
        "..%5c",
        "%2e%2e%2f",
        "%2e%2e%5c",
    ];

    for pattern in PATH_TRAVERSAL_PATTERNS {
        if key.to_lowercase().contains(&pattern.to_lowercase()) {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains path traversal pattern: {}",
                pattern
            )));
        }
    }

    // 检查命令注入模式
    // 直接检测危险字符，不使用可能绕过的不安全条件
    const COMMAND_INJECTION_CHARS: &[char] = &[';', '|', '&', '`'];

    for c in key.chars() {
        if COMMAND_INJECTION_CHARS.contains(&c) {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains potential command injection character: {:?}",
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
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
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
            MAX_LUA_SCRIPT_KEYS, key_count
        )));
    }

    // 预处理脚本：移除注释、字符串和多行内容，得到净化后的脚本
    let cleaned = preprocess_lua_script(script);
    let cleaned_upper = cleaned.to_uppercase();

    // 使用简单字符串检查代替正则表达式（避免原始字符串语法问题）
    // 预处理会保留 redis.call('X') 的格式: REDIS.CALL('X')
    let forbidden_patterns = [
        // FLUSHALL patterns
        ("REDIS.CALL('FLUSHALL')", "FLUSHALL"),
        ("REDIS.CALL(\"FLUSHALL\")", "FLUSHALL"),
        ("REDIS.PCALL('FLUSHALL')", "FLUSHALL via PCALL"),
        ("REDIS.PCALL(\"FLUSHALL\")", "FLUSHALL via PCALL"),
        // FLUSHDB patterns
        ("REDIS.CALL('FLUSHDB')", "FLUSHDB"),
        ("REDIS.CALL(\"FLUSHDB\")", "FLUSHDB"),
        ("REDIS.PCALL('FLUSHDB')", "FLUSHDB via PCALL"),
        ("REDIS.PCALL(\"FLUSHDB\")", "FLUSHDB via PCALL"),
        // KEYS patterns - with and without comma (for different argument styles)
        ("REDIS.CALL('KEYS'", "KEYS"),
        ("REDIS.CALL(\"KEYS\"", "KEYS"),
        ("REDIS.PCALL('KEYS'", "KEYS via PCALL"),
        ("REDIS.PCALL(\"KEYS\"", "KEYS via PCALL"),
        // SHUTDOWN patterns
        ("REDIS.CALL('SHUTDOWN')", "SHUTDOWN"),
        ("REDIS.CALL(\"SHUTDOWN\")", "SHUTDOWN"),
        // CONFIG patterns
        ("REDIS.CALL('CONFIG'", "CONFIG"),
        ("REDIS.CALL(\"CONFIG\"", "CONFIG"),
        // DEBUG patterns
        ("REDIS.CALL('DEBUG'", "DEBUG"),
        ("REDIS.CALL(\"DEBUG\"", "DEBUG"),
        // SAVE patterns
        ("REDIS.CALL('SAVE')", "SAVE"),
        ("REDIS.CALL(\"SAVE\")", "SAVE"),
        // BGSAVE patterns
        ("REDIS.CALL('BGSAVE')", "BGSAVE"),
        ("REDIS.CALL(\"BGSAVE\")", "BGSAVE"),
        // MONITOR patterns
        ("REDIS.CALL('MONITOR')", "MONITOR"),
        ("REDIS.CALL(\"MONITOR\")", "MONITOR"),
        // OS commands
        ("OS.EXECUTE", "os.execute"),
        ("OS.EXEC", "os.exec"),
        ("IO.POPEN", "io.popen"),
        ("LOADSTRING", "loadstring"),
        ("LOAD(", "load()"),
    ];

    // 检查每种危险模式
    for (pattern, description) in &forbidden_patterns {
        if cleaned_upper.contains(pattern) {
            return Err(CacheError::InvalidInput(format!(
                "Lua script contains forbidden pattern: {}",
                description
            )));
        }
    }

    // 检查嵌套 eval
    if cleaned_upper.contains("REDIS.EVAL") || cleaned_upper.contains("REDIS.EVALSHA") {
        return Err(CacheError::InvalidInput(
            "Lua script contains nested redis.eval/evalsha".to_string(),
        ));
    }

    // 检查无限循环模式（使用预编译的正则表达式）
    for re in LUA_LOOP_REGEXES.iter() {
        if re.is_match(&cleaned_upper) {
            return Err(CacheError::InvalidInput(
                "Lua script contains potential infinite loop patterns".to_string(),
            ));
        }
    }

    Ok(())
}

/// 预处理 Lua 脚本，移除注释和字符串内容
///
/// 这可以防止通过注释、字符串拼接等方式绕过检查
fn preprocess_lua_script(script: &str) -> String {
    let mut result = String::with_capacity(script.len());

    let mut chars = script.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'-') {
            // 消费第二个 '-'，现在 chars 应该指向 '['
            chars.next();
            // 检查是否是 --[[ (多行注释开始)
            let level = count_lua_long_string_level(&mut chars, 1);
            if level > 0 {
                // 这是 --[=[=[ 多行注释
                skip_lua_long_string(&mut chars, level);
            } else {
                // 移除单行注释
                while let Some(&next_c) = chars.peek() {
                    if next_c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
        } else if c == '[' {
            // 检查是否是 Lua 长字符串 [[...]], [=[...]=], [==[...]==], 等
            let level = count_lua_long_string_level(&mut chars, 0);
            if level > 0 {
                // 移除 Lua 长字符串
                skip_lua_long_string(&mut chars, level);
            } else {
                result.push('[');
            }
        } else if c == '"' {
            // 移除字符串 ""
            result.push('"');
            result.push('"'); // 用空字符串替换
            while let Some(&next_c) = chars.peek() {
                if next_c == '"' {
                    chars.next(); // 跳过结束引号
                    break;
                } else if next_c == '\\' {
                    chars.next(); // 跳过转义字符
                    chars.next();
                } else if next_c == '\n' {
                    break; // 未闭合的字符串
                } else {
                    chars.next();
                }
            }
        } else if c == '\'' {
            // 处理单引号字符串：保留标识符字符用于模式检测，移除其他内容
            result.push('\'');
            let mut in_string = true;
            while in_string {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '\'' {
                        chars.next();
                        result.push('\'');
                        in_string = false;
                    } else if next_c == '\\' {
                        chars.next();
                        if let Some(escaped) = chars.next() {
                            if escaped.is_alphanumeric() || escaped == '_' {
                                result.push(escaped);
                            }
                        }
                    } else if next_c == '\n' {
                        break;
                    } else if next_c.is_alphanumeric() || next_c == '_' {
                        result.push(next_c);
                        chars.next();
                    } else {
                        chars.next();
                    }
                } else {
                    break;
                }
            }
        } else if c.is_whitespace() {
            // 规范化空白字符为空格
            if !result.is_empty() && !result.ends_with(' ') {
                result.push(' ');
            }
        } else {
            result.push(c);
        }
    }

    // 移除多余空格（使用预编译的正则表达式）
    WHITESPACE_REGEX.replace_all(&result, " ").to_string()
}

/// 计算 Lua 长字符串的级别（= 的数量）
/// 返回级别数，0 表示不是长字符串
/// chars 指针应该在 [ 之后的位置
fn count_lua_long_string_level(chars: &mut std::iter::Peekable<std::str::Chars>, start_level: usize) -> usize {
    let mut level = start_level;
    while let Some(&c) = chars.peek() {
        if c == '=' {
            level += 1;
            chars.next();
        } else if c == '[' {
            chars.next();
            return level; // 返回级别
        } else {
            break; // 不是长字符串
        }
    }
    0 // 不是长字符串
}

/// 跳过 Lua 长字符串内容
/// level 是长字符串的级别（= 的数量 + 1）
fn skip_lua_long_string(chars: &mut std::iter::Peekable<std::str::Chars>, level: usize) {
    let closing: String = format!("]{}{}]", "=".repeat(level - 1), "=".repeat(level - 1));
    let closing_chars: Vec<char> = closing.chars().collect();
    let mut pos = 0;
    let closing_len = closing.len();

    while let Some(c) = chars.next() {
        if c == ']' {
            // 检查是否是闭合括号
            let mut check_pos = 1;
            let mut is_match = true;
            while check_pos < closing_len {
                if let Some(&next_c) = chars.peek() {
                    if next_c == closing_chars[check_pos] {
                        chars.next();
                        check_pos += 1;
                    } else {
                        is_match = false;
                        break;
                    }
                } else {
                    is_match = false;
                    break;
                }
            }
            if is_match && check_pos == closing_len {
                break; // 找到闭合
            }
        }
        pos += 1;
        if pos > 1_000_000 {
            break; // 防止无限循环
        }
    }
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
///
/// # 安全地验证 SCAN 模式
///
/// 防止恶意模式导致 Redis 性能问题。
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
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
            MAX_SCAN_WILDCARDS, wildcard_count
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
/// 将 SCAN count 参数限制在安全范围内
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
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
        // Note: bare "1=1" patterns are not flagged to avoid false positives
        // on normal cache keys like "api_v1_data" or "user_1_status"
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
