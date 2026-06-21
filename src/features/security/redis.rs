//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis 和 Lua 安全验证
//!
//! 提供 Redis 键、Lua 脚本和 SCAN 操作的安全验证功能。

use crate::error::{CacheError, Result};

/// Lua 脚本最大长度 (10KB)
pub(crate) const MAX_LUA_SCRIPT_LENGTH: usize = 10 * 1024;

/// Lua 脚本最大键数量
pub(crate) const MAX_LUA_SCRIPT_KEYS: usize = 100;

/// SCAN 模式最大长度
pub(crate) const MAX_SCAN_PATTERN_LENGTH: usize = 256;

/// SCAN 模式最大通配符数量
pub(crate) const MAX_SCAN_WILDCARDS: usize = 10;

/// SCAN count 参数安全范围
pub(crate) const SCAN_COUNT_MIN: usize = 1;
pub(crate) const SCAN_COUNT_MAX: usize = 1000;

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
    pub(crate) static ref LUA_LOOP_REGEXES: Vec<::regex::Regex> = {
        LUA_LOOP_PATTERNS
            .iter()
            .map(|(pattern, _)| ::regex::Regex::new(pattern).expect("Invalid loop pattern regex"))
            .collect()
    };
}

/// 空白字符替换正则
lazy_static::lazy_static! {
    pub(crate) static ref WHITESPACE_REGEX: ::regex::Regex = ::regex::Regex::new(r"\s+").expect("Invalid whitespace regex");
}

// ============================================================================
// Helper Functions
// ============================================================================

/// 验证 Redis 键是否包含危险 Unicode 控制字符
pub(crate) fn check_unicode_control_chars(key: &str) -> Result<()> {
    for c in key.chars() {
        if c.is_control() && !matches!(c, '\r' | '\n' | '\0' | '\t') {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains control character: U+{:04X}",
                c as u32
            )));
        }
    }
    Ok(())
}

// ============================================================================
// Lua Script Preprocessing
// ============================================================================

/// 预处理 Lua 脚本，移除注释和字符串内容
///
/// 这可以防止通过注释、字符串拼接等方式绕过检查
pub(crate) fn preprocess_lua_script(script: &str) -> String {
    let mut result = String::with_capacity(script.len());

    let mut chars = script.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'-') {
            chars.next();
            let level = count_lua_long_string_level(&mut chars, 1);
            if level > 0 {
                skip_lua_long_string(&mut chars, level);
            } else {
                while let Some(&next_c) = chars.peek() {
                    if next_c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
        } else if c == '[' {
            let level = count_lua_long_string_level(&mut chars, 0);
            if level > 0 {
                skip_lua_long_string(&mut chars, level);
            } else {
                result.push('[');
            }
        } else if c == '"' {
            result.push('"');
            result.push('"');
            while let Some(&next_c) = chars.peek() {
                if next_c == '"' {
                    chars.next();
                    break;
                } else if next_c == '\\' {
                    chars.next();
                    if chars.next().is_none_or(|c| c != 'x') {
                        // Not a hex escape, skip the escaped char
                    } else {
                        // \xHH hex escape: skip up to 2 hex digits
                        for _ in 0..2 {
                            if chars.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                                chars.next();
                            }
                        }
                    }
                } else if next_c == '\n' {
                    break;
                } else {
                    chars.next();
                }
            }
        } else if c == '\'' {
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
                            if escaped == 'x' {
                                // \xHH hex escape: skip up to 2 hex digits
                                for _ in 0..2 {
                                    if chars.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                                        chars.next();
                                    }
                                }
                            } else if escaped.is_alphanumeric() || escaped == '_' {
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
            if !result.is_empty() && !result.ends_with(' ') {
                result.push(' ');
            }
        } else {
            result.push(c);
        }
    }

    WHITESPACE_REGEX.replace_all(&result, " ").to_string()
}

/// 计算 Lua 长字符串的级别（= 的数量）
fn count_lua_long_string_level(chars: &mut std::iter::Peekable<std::str::Chars>, start_level: usize) -> usize {
    let mut level = start_level;
    while let Some(&c) = chars.peek() {
        if c == '=' {
            level += 1;
            chars.next();
        } else if c == '[' {
            chars.next();
            return level;
        } else {
            break;
        }
    }
    0
}

/// 跳过 Lua 长字符串内容
fn skip_lua_long_string(chars: &mut std::iter::Peekable<std::str::Chars>, level: usize) {
    let closing: String = format!("]{}]", "=".repeat(level - 1));
    let closing_chars: Vec<char> = closing.chars().collect();
    let mut pos = 0;
    let closing_len = closing.len();

    while let Some(c) = chars.next() {
        if c == ']' {
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
                break;
            }
        }
        pos += 1;
        if pos > 1_000_000 {
            break;
        }
    }
}

// ============================================================================
// Public API
// ============================================================================

/// 验证 Redis 缓存键是否安全
///
/// 防止 Redis 坐标注入和协议污染攻击。
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn validate_redis_key(key: &str) -> Result<()> {
    crate::features::security::validation::redis::validate_key(key)?;
    check_unicode_control_chars(key)?;
    crate::features::security::injection::check_sql_injection(key)?;
    crate::features::security::path::check_path_traversal(key)?;
    crate::features::security::injection::check_command_injection(key)?;
    Ok(())
}

/// 验证 Lua 脚本
///
/// 防止恶意脚本执行危险命令或导致 Redis 阻塞。
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn validate_lua_script(script: &str, key_count: usize) -> Result<()> {
    if script.len() > MAX_LUA_SCRIPT_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "Lua script exceeds maximum length of {} bytes (got {} bytes)",
            MAX_LUA_SCRIPT_LENGTH,
            script.len()
        )));
    }

    if key_count > MAX_LUA_SCRIPT_KEYS {
        return Err(CacheError::InvalidInput(format!(
            "Lua script exceeds maximum key count of {} (got {} keys)",
            MAX_LUA_SCRIPT_KEYS, key_count
        )));
    }

    let cleaned = preprocess_lua_script(script);
    let cleaned_upper = cleaned.to_uppercase();

    // 检查危险 Redis 命令（通过 RedisCommand 枚举统一管理）
    let dangerous_commands: &[crate::core::command::RedisCommand] = &[
        crate::core::command::RedisCommand::FlushAll,
        crate::core::command::RedisCommand::FlushDb,
        crate::core::command::RedisCommand::Keys,
        crate::core::command::RedisCommand::Shutdown,
        crate::core::command::RedisCommand::Config,
        crate::core::command::RedisCommand::Debug,
        crate::core::command::RedisCommand::Save,
        crate::core::command::RedisCommand::BgSave,
        crate::core::command::RedisCommand::Monitor,
    ];

    for cmd in dangerous_commands {
        let cmd_str = cmd.as_str();
        // REDIS.CALL / REDIS.PCALL with single or double quotes
        for call_type in &["CALL", "PCALL"] {
            for quote in &["'", "\""] {
                let pattern = format!("REDIS.{}({}{}", call_type, quote, cmd_str);
                if cleaned_upper.contains(&pattern) {
                    return Err(CacheError::InvalidInput(format!(
                        "Lua script contains forbidden command: {} via redis.{}",
                        cmd_str,
                        call_type.to_lowercase(),
                    )));
                }
            }
        }
    }

    // 非 Redis 命令的危险模式
    let non_redis_patterns = [
        ("OS.EXECUTE", "os.execute"),
        ("OS.EXEC", "os.exec"),
        ("IO.POPEN", "io.popen"),
        ("LOADSTRING", "loadstring"),
        ("LOAD(", "load()"),
    ];

    for (pattern, description) in &non_redis_patterns {
        if cleaned_upper.contains(pattern) {
            return Err(CacheError::InvalidInput(format!(
                "Lua script contains forbidden pattern: {}",
                description
            )));
        }
    }

    if cleaned_upper.contains("REDIS.EVAL") || cleaned_upper.contains("REDIS.EVALSHA") {
        return Err(CacheError::InvalidInput(
            "Lua script contains nested redis.eval/evalsha".to_string(),
        ));
    }

    for re in LUA_LOOP_REGEXES.iter() {
        if re.is_match(&cleaned_upper) {
            return Err(CacheError::InvalidInput(
                "Lua script contains potential infinite loop patterns".to_string(),
            ));
        }
    }

    Ok(())
}

/// 验证 SCAN 模式
///
/// 防止 ReDoS（正则表达式拒绝服务）攻击。
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn validate_scan_pattern(pattern: &str) -> Result<()> {
    if pattern.len() > MAX_SCAN_PATTERN_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "SCAN pattern exceeds maximum length of {} characters (got {} characters)",
            MAX_SCAN_PATTERN_LENGTH,
            pattern.len()
        )));
    }

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
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn clamp_scan_count(count: usize) -> usize {
    count.clamp(SCAN_COUNT_MIN, SCAN_COUNT_MAX)
}
