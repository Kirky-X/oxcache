// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Security impl - functions extracted from mod.rs

#[cfg(feature = "redis")]
use crate::error::{OxCacheError, OxCacheResult};

/// Lua 脚本最大长度 (10KB)
#[cfg(feature = "redis")]
pub(super) const MAX_LUA_SCRIPT_LENGTH: usize = 10 * 1024;

/// Lua 脚本最大键数量
#[cfg(feature = "redis")]
pub(super) const MAX_LUA_SCRIPT_KEYS: usize = 100;

/// SCAN 模式最大长度
#[cfg(feature = "redis")]
pub(super) const MAX_SCAN_PATTERN_LENGTH: usize = 256;

/// SCAN 模式最大通配符数量
#[cfg(feature = "redis")]
pub(super) const MAX_SCAN_WILDCARDS: usize = 10;

/// SCAN count 参数安全范围
#[cfg(feature = "redis")]
pub(super) const SCAN_COUNT_MIN: usize = 1;

/// SCAN count 参数安全范围
#[cfg(feature = "redis")]
pub(super) const SCAN_COUNT_MAX: usize = 1000;

/// Lua 无限循环检测正则模式
#[cfg(feature = "redis")]
pub(super) static LUA_LOOP_PATTERNS: &[(&str, &str)] = &[
    (r"WHILE\s+TRUE", "WHILE TRUE 循环"),
    (r"WHILE\s+1", "WHILE 1 循环"),
    (r"REPEAT", "REPEAT 循环"),
    (r"GOTO", "GOTO 语句"),
];

/// 预编译的 Lua 循环检测正则
#[cfg(feature = "redis")]
pub(super) static LUA_LOOP_REGEXES: ::once_cell::sync::Lazy<Vec<::regex::Regex>> =
    ::once_cell::sync::Lazy::new(|| {
        LUA_LOOP_PATTERNS
            .iter()
            .map(|(pattern, _)| ::regex::Regex::new(pattern).expect("Invalid loop pattern regex"))
            .collect()
    });

/// 空白字符替换正则
#[cfg(feature = "redis")]
pub(super) static WHITESPACE_REGEX: ::once_cell::sync::Lazy<::regex::Regex> =
    ::once_cell::sync::Lazy::new(|| ::regex::Regex::new(r"\s+").expect("Invalid whitespace regex"));

/// 方括号索引折叠正则（单引号变体）。
///
/// 防御形态：`redis['eval']("return 1")` 经预处理后引号被丢弃、方括号被丢弃，
/// 归一化为 `REDIS EVAL`，无法命中 `REDIS.EVAL` 黑名单。本正则在归一化末尾
/// 将 `['ident']` 折叠为 `.ident`，使方括号索引调用进入既有黑名单匹配。
/// 正则仅匹配标识符索引（`[A-Za-z_][A-Za-z0-9_]*`）；注意管线层面
/// `scan_quoted_string` 会先剥离字符串内容中的非标识符字符，因此
/// `['a-b']` 中的 `-` 先被剥除、`ab` 成为标识符后**同样会被折叠**——
/// 折叠结果仍为合法 Lua 且不产生黑名单命中，但不得依赖"非标识符
/// 索引保持原样"的假设。
#[cfg(feature = "redis")]
static BRACKET_INDEX_SINGLE: ::once_cell::sync::Lazy<::regex::Regex> =
    ::once_cell::sync::Lazy::new(|| {
        ::regex::Regex::new(r"\[\s*'([A-Za-z_][A-Za-z0-9_]*)'\s*\]")
            .expect("Invalid bracket-index single-quote regex")
    });

/// 方括号索引折叠正则（双引号变体），语义同 [`BRACKET_INDEX_SINGLE`]。
#[cfg(feature = "redis")]
static BRACKET_INDEX_DOUBLE: ::once_cell::sync::Lazy<::regex::Regex> =
    ::once_cell::sync::Lazy::new(|| {
        ::regex::Regex::new(r#"\[\s*"([A-Za-z_][A-Za-z0-9_]*)"\s*\]"#)
            .expect("Invalid bracket-index double-quote regex")
    });

/// SQL 注入检测模式表（模式, 描述）
#[cfg(feature = "redis")]
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

/// 路径遍历检测模式表
#[cfg(feature = "redis")]
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

/// 命令注入检测字符集
#[cfg(feature = "redis")]
const COMMAND_INJECTION_CHARS: &[char] = &[';', '|', '&', '`'];

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
/// * `Err(OxCacheError::InvalidInput)` - 键包含不安全字符
#[cfg(feature = "redis")]
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn validate_redis_key(key: &str) -> OxCacheResult<()> {
    use crate::security::{DANGEROUS_CHARS, MAX_KEY_LENGTH};
    use crate::security::{validate_max_length, validate_no_dangerous_chars, validate_not_empty};

    validate_not_empty(key, "Redis key")?;
    validate_max_length(key, MAX_KEY_LENGTH, "Redis key")?;
    validate_no_dangerous_chars(key, &DANGEROUS_CHARS, "Redis key")?;

    check_control_characters(key, &DANGEROUS_CHARS)?;
    check_sql_injection(key)?;
    check_path_traversal(key)?;
    check_command_injection(key)?;

    Ok(())
}

/// 检查 Unicode 控制字符（CR, LF, NULL 已在基础验证中检查）
#[cfg(feature = "redis")]
fn check_control_characters(key: &str, dangerous_chars: &[char]) -> OxCacheResult<()> {
    for c in key.chars() {
        if c.is_control() && !dangerous_chars.contains(&c) && c != '\t' {
            return Err(OxCacheError::InvalidInput(format!(
                "Redis key contains control character: U+{:04X}",
                c as u32
            )));
        }
    }
    Ok(())
}

/// 检查 SQL 注入模式
#[cfg(feature = "redis")]
fn check_sql_injection(key: &str) -> OxCacheResult<()> {
    let key_upper = key.to_uppercase();
    for (pattern, description) in SQL_INJECTION_PATTERNS {
        if key_upper.contains(&pattern.to_uppercase()) {
            return Err(OxCacheError::InvalidInput(format!(
                "Redis key contains suspicious SQL injection pattern: {}",
                description
            )));
        }
    }
    Ok(())
}

/// 检查路径遍历模式
#[cfg(feature = "redis")]
fn check_path_traversal(key: &str) -> OxCacheResult<()> {
    let key_lower = key.to_lowercase();
    for pattern in PATH_TRAVERSAL_PATTERNS {
        if key_lower.contains(&pattern.to_lowercase()) {
            return Err(OxCacheError::InvalidInput(format!(
                "Redis key contains path traversal pattern: {}",
                pattern
            )));
        }
    }
    Ok(())
}

/// 检查命令注入字符
#[cfg(feature = "redis")]
fn check_command_injection(key: &str) -> OxCacheResult<()> {
    for c in key.chars() {
        if COMMAND_INJECTION_CHARS.contains(&c) {
            return Err(OxCacheError::InvalidInput(format!(
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
/// * `Err(OxCacheError::InvalidInput)` - 脚本验证失败
#[cfg(feature = "redis")]
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn validate_lua_script(script: &str, key_count: usize) -> OxCacheResult<()> {
    // 检查脚本长度
    if script.len() > MAX_LUA_SCRIPT_LENGTH {
        return Err(OxCacheError::InvalidInput(format!(
            "Lua script exceeds maximum length of {} bytes (got {} bytes)",
            MAX_LUA_SCRIPT_LENGTH,
            script.len()
        )));
    }

    // 检查键数量
    if key_count > MAX_LUA_SCRIPT_KEYS {
        return Err(OxCacheError::InvalidInput(format!(
            "Lua script exceeds maximum key count of {} (got {} keys)",
            MAX_LUA_SCRIPT_KEYS, key_count
        )));
    }

    // 预处理脚本：移除注释、字符串和多行内容，得到净化后的脚本
    let cleaned = preprocess_lua_script(script);
    // 归一化反斜杠转义的引号（如 redis.call(\'FLUSHALL\')）：反斜杠若留在
    // 清洗文本中会让带引号的黑名单模式失配，从而绕过检测。字符串字面量
    // 内部的转义已在 scan_quoted_string 中处理，此处残留的转义引号均来自
    // 字符串上下文之外，还原为普通引号不会把无害脚本变成危险模式。
    let cleaned = cleaned.replace("\\'", "'").replace("\\\"", "\"");
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
            return Err(OxCacheError::InvalidInput(format!(
                "Lua script contains forbidden pattern: {}",
                description
            )));
        }
    }

    // 检查嵌套 eval
    if cleaned_upper.contains("REDIS.EVAL") || cleaned_upper.contains("REDIS.EVALSHA") {
        return Err(OxCacheError::InvalidInput(
            "Lua script contains nested redis.eval/evalsha".to_string(),
        ));
    }

    // 检查无限循环模式（使用预编译的正则表达式）
    for re in LUA_LOOP_REGEXES.iter() {
        if re.is_match(&cleaned_upper) {
            return Err(OxCacheError::InvalidInput(
                "Lua script contains potential infinite loop patterns".to_string(),
            ));
        }
    }

    Ok(())
}

/// 预处理 Lua 脚本，移除注释和字符串内容
///
/// 这可以防止通过注释、字符串拼接等方式绕过检查
#[cfg(feature = "redis")]
pub(super) fn preprocess_lua_script(script: &str) -> String {
    let mut result = String::with_capacity(script.len());
    let mut chars = script.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'-') {
            chars.next();
            skip_lua_comment(&mut chars);
        } else if c == '[' {
            if !try_skip_long_string(&mut chars) {
                result.push('[');
            }
        } else if c == '"' || c == '\'' {
            result.push(c);
            scan_quoted_string(&mut chars, &mut result, c);
        } else if c.is_whitespace() {
            if !result.is_empty() && !result.ends_with(' ') {
                result.push(' ');
            }
        } else {
            result.push(c);
        }
    }

    let result = WHITESPACE_REGEX.replace_all(&result, " ").to_string();

    // 方括号索引折叠：将 ['ident'] / ["ident"] 替换为 .ident
    // 使 redis['eval'](...) 归一化为 redis.eval(...) 从而命中既有 REDIS.EVAL 检查
    let result = BRACKET_INDEX_SINGLE.replace_all(&result, ".$1").to_string();
    BRACKET_INDEX_DOUBLE.replace_all(&result, ".$1").to_string()
}

/// 跳过 Lua 注释内容（`--` 已消费）。
/// 支持单行注释（至换行）和块注释（`--[...]=]`）。
#[cfg(feature = "redis")]
fn skip_lua_comment(chars: &mut std::iter::Peekable<std::str::Chars>) {
    // 必须先消费开括号 '[' 再计数：count_lua_long_string_level 一看到 '['
    // 就消费并立即返回 start_level，若不先消费，'=' 永远不会参与统计，
    // 块注释闭合符退化为 "]]"，--[==[…]==] 之后的载荷会被越界吞吃而对
    // 校验器不可见（CVE-184 类黑名单规范化不完备）。
    if chars.peek() == Some(&'[') {
        chars.next();
        let level = count_lua_long_string_level(chars, 1);
        if level > 0 {
            // count 起始 level 为 1（已消费的开括号计数），传给 skip 时归一化
            // 为真实级别：level-0 块注释对应 closer "]]"，level-N 对应
            // "]" + "=" * N + "]"。
            skip_lua_long_string(chars, level - 1);
            return;
        }
    }
    // 单行注释：消费至换行符（保留换行）
    while let Some(&next_c) = chars.peek() {
        if next_c == '\n' {
            break;
        }
        chars.next();
    }
}

/// 尝试跳过 Lua 长字符串（`[` 已消费）。
/// 返回 `true` 表示检测到长字符串并已跳过，`false` 表示不是长字符串。
#[cfg(feature = "redis")]
fn try_skip_long_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> bool {
    let level = count_lua_long_string_level(chars, 0);
    if level > 0 {
        skip_lua_long_string(chars, level);
        true
    } else {
        false
    }
}

/// 扫描一个 Lua 字符串字面量（单引号或双引号）的内容。
///
/// 保留标识符字符（字母/数字/下划线）、转义后的字符与括号用于后续的
/// 危险命令模式检测（调用形态如 `REDIS.CALL('FLUSHALL')` 依赖括号），
/// 移除其余非标识符内容。遇到闭合引号、未转义的换行或输入结束即停止。
/// 调用前 `quote` 已写入 `result`，函数负责扫描并写入闭合引号与保留内容，
/// 但不处理输入流的首字符（由主循环已完成）。
///
/// 位置：`chars` 指向字符串内容起点（引号后的第一个字符）。
#[cfg(feature = "redis")]
fn scan_quoted_string(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    quote: char,
) {
    while let Some(&next_c) = chars.peek() {
        if next_c == quote {
            chars.next();
            result.push(quote);
            break;
        } else if next_c == '\\' {
            chars.next();
            // 保留被转义字符原样参与关键词匹配：丢弃引号类字符会让
            // REDIS.CALL('FLUSHALL') 类载荷绕过带引号的黑名单模式。
            if let Some(escaped) = chars.next() {
                result.push(escaped);
            }
        } else if next_c == '\n' {
            break; // 未闭合的字符串
        } else if next_c.is_alphanumeric() || next_c == '_' {
            result.push(next_c);
            chars.next();
        } else if next_c == '(' || next_c == ')' {
            // 括号参与调用形态模式匹配，不可丢弃
            result.push(next_c);
            chars.next();
        } else {
            chars.next();
        }
    }
}

/// 计算 Lua 长字符串的级别（= 的数量）
/// 返回级别数，0 表示不是长字符串
/// chars 指针应该在 [ 之后的位置
#[cfg(feature = "redis")]
pub(super) fn count_lua_long_string_level(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    start_level: usize,
) -> usize {
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
/// level 是长字符串的真实级别（= 的数量）：level-0 对应闭合符 "]]"，
/// level-N 对应 "]" + "=" * N + "]"。
#[cfg(feature = "redis")]
pub(super) fn skip_lua_long_string(chars: &mut std::iter::Peekable<std::str::Chars>, level: usize) {
    let closing: String = format!("]{}]", "=".repeat(level));
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
/// * `Err(OxCacheError::InvalidInput)` - 模式验证失败
///
/// # 安全地验证 SCAN 模式
///
/// 防止恶意模式导致 Redis 性能问题。
#[cfg(feature = "redis")]
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn validate_scan_pattern(pattern: &str) -> OxCacheResult<()> {
    // 检查模式长度
    if pattern.len() > MAX_SCAN_PATTERN_LENGTH {
        return Err(OxCacheError::InvalidInput(format!(
            "SCAN pattern exceeds maximum length of {} characters (got {} characters)",
            MAX_SCAN_PATTERN_LENGTH,
            pattern.len()
        )));
    }

    // 计算通配符数量
    let wildcard_count = pattern.chars().filter(|c| *c == '*').count();

    if wildcard_count > MAX_SCAN_WILDCARDS {
        return Err(OxCacheError::InvalidInput(format!(
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
#[cfg(feature = "redis")]
#[cfg_attr(docsrs, doc(cfg(feature = "security")))]
pub fn clamp_scan_count(count: usize) -> usize {
    count.clamp(SCAN_COUNT_MIN, SCAN_COUNT_MAX)
}
