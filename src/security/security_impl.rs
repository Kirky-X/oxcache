// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Security impl - functions extracted from mod.rs

#[cfg(feature = "redis")]
use crate::error::{OxCacheError, OxCacheResult};
#[cfg(feature = "redis")]
use crate::i18n::messages::{
    MSG_DETAIL_LUA_INFINITE_LOOP_GOTO, MSG_DETAIL_LUA_INFINITE_LOOP_PATTERNS,
    MSG_DETAIL_LUA_INFINITE_LOOP_REPEAT, MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_1,
    MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_TRUE, MSG_DETAIL_LUA_NESTED_EVAL,
    MSG_DETAIL_LUA_SCRIPT_FORBIDDEN_PATTERN, MSG_DETAIL_LUA_SCRIPT_TOO_LONG,
    MSG_DETAIL_LUA_SCRIPT_TOO_MANY_KEYS, MSG_DETAIL_REDIS_KEY_COMMAND_INJECTION_CHARACTER,
    MSG_DETAIL_REDIS_KEY_CONTROL_CHARACTER, MSG_DETAIL_REDIS_KEY_PATH_TRAVERSAL_PATTERN,
    MSG_DETAIL_REDIS_KEY_SQL_INJECTION_PATTERN, MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_LONG,
    MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_MANY_WILDCARDS, MSG_DETAIL_REDIS_SQL_INJECTION_AUTH_BYPASS,
    MSG_DETAIL_REDIS_SQL_INJECTION_COMMENT, MSG_DETAIL_REDIS_SQL_INJECTION_DELETE,
    MSG_DETAIL_REDIS_SQL_INJECTION_DROP, MSG_DETAIL_REDIS_SQL_INJECTION_INSERT,
    MSG_DETAIL_REDIS_SQL_INJECTION_QUOTE_OR, MSG_DETAIL_REDIS_SQL_INJECTION_TAUTOLOGY,
    MSG_DETAIL_REDIS_SQL_INJECTION_UNION_SELECT, MSG_DETAIL_REDIS_SQL_INJECTION_XP_CMDSHELL, t,
};

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

/// Lua 无限循环检测正则模式（模式, 描述消息 ID）
#[cfg(feature = "redis")]
pub(super) static LUA_LOOP_PATTERNS: &[(&str, &str)] = &[
    (r"WHILE\s+TRUE", MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_TRUE),
    (r"WHILE\s+1", MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_1),
    (r"REPEAT", MSG_DETAIL_LUA_INFINITE_LOOP_REPEAT),
    (r"GOTO", MSG_DETAIL_LUA_INFINITE_LOOP_GOTO),
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

/// SQL 注入检测模式表（模式, 描述消息 ID）
#[cfg(feature = "redis")]
const SQL_INJECTION_PATTERNS: &[(&str, &str)] = &[
    ("' OR '", MSG_DETAIL_REDIS_SQL_INJECTION_QUOTE_OR),
    ("'--", MSG_DETAIL_REDIS_SQL_INJECTION_COMMENT),
    ("'; DROP", MSG_DETAIL_REDIS_SQL_INJECTION_DROP),
    ("'; DELETE", MSG_DETAIL_REDIS_SQL_INJECTION_DELETE),
    ("'; INSERT", MSG_DETAIL_REDIS_SQL_INJECTION_INSERT),
    ("UNION SELECT", MSG_DETAIL_REDIS_SQL_INJECTION_UNION_SELECT),
    ("xp_cmdshell", MSG_DETAIL_REDIS_SQL_INJECTION_XP_CMDSHELL),
    ("' OR '1'='1", MSG_DETAIL_REDIS_SQL_INJECTION_TAUTOLOGY),
    ("admin'--", MSG_DETAIL_REDIS_SQL_INJECTION_AUTH_BYPASS),
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
#[cfg_attr(docsrs, doc(cfg(feature = "redis")))]
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
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_REDIS_KEY_CONTROL_CHARACTER,
                &[("code", format!("{:04X}", c as u32))],
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
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_REDIS_KEY_SQL_INJECTION_PATTERN,
                &[("detail", t(description, &[]))],
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
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_REDIS_KEY_PATH_TRAVERSAL_PATTERN,
                &[("detail", pattern.to_string())],
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
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_REDIS_KEY_COMMAND_INJECTION_CHARACTER,
                &[("detail", format!("{c:?}"))],
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
#[cfg_attr(docsrs, doc(cfg(feature = "redis")))]
pub fn validate_lua_script(script: &str, key_count: usize) -> OxCacheResult<()> {
    if script.len() > MAX_LUA_SCRIPT_LENGTH {
        return Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_LUA_SCRIPT_TOO_LONG,
            &[
                ("max", MAX_LUA_SCRIPT_LENGTH.to_string()),
                ("actual", script.len().to_string()),
            ],
        )));
    }

    if key_count > MAX_LUA_SCRIPT_KEYS {
        return Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_LUA_SCRIPT_TOO_MANY_KEYS,
            &[
                ("max", MAX_LUA_SCRIPT_KEYS.to_string()),
                ("actual", key_count.to_string()),
            ],
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
    // Redis 命令调用（redis.call / redis.pcall）由命令级解析单独判定，
    // 此处只覆盖无法解析为调用的宿主语言逃逸构造
    let forbidden_patterns = [
        // OS commands
        ("OS.EXECUTE", "os.execute"),
        ("OS.EXEC", "os.exec"),
        ("IO.POPEN", "io.popen"),
        ("LOADSTRING", "loadstring"),
        ("LOAD(", "load()"),
    ];

    for (pattern, description) in &forbidden_patterns {
        if cleaned_upper.contains(pattern) {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_LUA_SCRIPT_FORBIDDEN_PATTERN,
                &[("detail", description.to_string())],
            )));
        }
    }

    // 命令级判定：按「被调用命令名」而非字面子串匹配，
    // 空白与字符串拼接变体（redis.call ( 'FLUSHALL' )、
    // redis.call('FLUSH'..'ALL')）与直写形式等价
    check_lua_redis_commands(script)?;

    if cleaned_upper.contains("REDIS.EVAL") || cleaned_upper.contains("REDIS.EVALSHA") {
        return Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_LUA_NESTED_EVAL,
            &[],
        )));
    }

    // 检查无限循环模式（使用预编译的正则表达式）
    for re in LUA_LOOP_REGEXES.iter() {
        if re.is_match(&cleaned_upper) {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_LUA_INFINITE_LOOP_PATTERNS,
                &[],
            )));
        }
    }

    Ok(())
}

/// 禁止经 `redis.call` / `redis.pcall` 执行的 Redis 命令
///
/// 按命令名判定（而非字面子串），空白与字符串拼接变体一律等价。
#[cfg(feature = "redis")]
const FORBIDDEN_REDIS_COMMANDS: &[&str] = &[
    "FLUSHALL", "FLUSHDB", "KEYS", "SHUTDOWN", "CONFIG", "DEBUG", "SAVE", "BGSAVE", "MONITOR",
    "SCRIPT", "EVAL", "EVALSHA",
];

/// Lua 词法单元
#[cfg(feature = "redis")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum LuaTok {
    /// 标识符 / 关键字
    Ident(String),
    /// 字符串字面量（引号或长字符串，已解转义）
    Str(String),
    /// 单字符标点
    Punct(char),
}

/// 校验 Lua 脚本中的 Redis 命令调用
///
/// 按「被调用命令名」判定：`redis.call ( 'FLUSHALL' )`、
/// `redis.call('FLUSH'..'ALL')`、`redis["call"]('FLUSHALL')` 与
/// `redis.call('FLUSHALL')` 等价，一律拒绝。函数引用别名
/// （`local f = redis.call` 后 `f('FLUSHALL')`）同样按命令名判定。
/// 命令名无法静态确定（变量、函数返回值、`unpack(ARGV)` 等）时按拒绝处理。
#[cfg(feature = "redis")]
fn check_lua_redis_commands(script: &str) -> OxCacheResult<()> {
    let toks = lex_lua(script);
    let aliases = collect_lua_aliases(&toks);
    for i in 0..toks.len() {
        if let Some(err) = check_lua_call_at(&toks, &aliases, i) {
            return Err(err);
        }
    }
    Ok(())
}

/// Lua 中的 redis 别名集合
///
/// - `tables`：`local r = redis` 形态的**表**别名（后续 `r.call(...)`）；
/// - `calls`：`local f = redis.call` 形态的**函数引用**别名（后续
///   `f('FLUSHALL')`）。函数引用不落在表别名里：它没有成员访问，凭
///   `redis.` 形态无法识别，漏掉即等于放开危险命令的别名通道。
#[cfg(feature = "redis")]
#[derive(Debug, Default)]
struct LuaAliases {
    tables: Vec<String>,
    calls: Vec<String>,
}

/// 检查位于 `idx` 的标识符是否为 redis 别名，且其后为受控调用
#[cfg(feature = "redis")]
fn check_lua_call_at(toks: &[LuaTok], aliases: &LuaAliases, idx: usize) -> Option<OxCacheError> {
    let LuaTok::Ident(name) = &toks[idx] else {
        return None;
    };

    // 函数引用别名：`local f = redis.call` 后 `f('FLUSHALL')` / `f 'FLUSHALL'`
    if aliases.calls.iter().any(|a| a.eq_ignore_ascii_case(name)) {
        return check_lua_alias_call(toks, idx + 1);
    }

    if !aliases.tables.iter().any(|a| a.eq_ignore_ascii_case(name)) {
        return None;
    }
    let (member, after_member) = parse_lua_member(toks, idx + 1)?;
    let member = member.to_ascii_lowercase();
    if member == "eval" || member == "evalsha" {
        return Some(OxCacheError::InvalidInput(t(
            MSG_DETAIL_LUA_NESTED_EVAL,
            &[],
        )));
    }
    if member != "call" && member != "pcall" {
        return None;
    }
    if !matches!(toks.get(after_member), Some(LuaTok::Punct('('))) {
        return None;
    }
    let (args, _) = parse_lua_args(toks, after_member);
    check_lua_command_args(&args)
}

/// 判定函数引用别名的调用点
///
/// 支持两种调用形态：`f(...)` 与 Lua 的字符串调用语法 `f 'CMD'`。
#[cfg(feature = "redis")]
fn check_lua_alias_call(toks: &[LuaTok], idx: usize) -> Option<OxCacheError> {
    match toks.get(idx) {
        Some(LuaTok::Punct('(')) => {
            let (args, _) = parse_lua_args(toks, idx);
            check_lua_command_args(&args)
        }
        Some(LuaTok::Str(cmd)) => {
            let upper = cmd.trim().to_ascii_uppercase();
            if FORBIDDEN_REDIS_COMMANDS.contains(&upper.as_str()) {
                return Some(forbidden_command_error(upper));
            }
            None
        }
        _ => None,
    }
}

/// 判定调用实参中的 Redis 命令名是否属于禁止清单
///
/// 命令名无法静态确定时按拒绝处理（fail closed）。
#[cfg(feature = "redis")]
fn check_lua_command_args(args: &[Vec<LuaTok>]) -> Option<OxCacheError> {
    let command = args
        .first()
        .and_then(|arg| fold_lua_strings(strip_outer_parens(arg)));
    match command {
        Some(cmd) => {
            let upper = cmd.trim().to_ascii_uppercase();
            if FORBIDDEN_REDIS_COMMANDS.contains(&upper.as_str()) {
                return Some(forbidden_command_error(upper));
            }
            None
        }
        None => Some(OxCacheError::InvalidInput(t(
            MSG_DETAIL_LUA_SCRIPT_FORBIDDEN_PATTERN,
            &[(
                "detail",
                "non-literal Redis command name (fail closed)".to_string(),
            )],
        ))),
    }
}

/// 构造「禁止的 Redis 命令」错误
#[cfg(feature = "redis")]
fn forbidden_command_error(command: String) -> OxCacheError {
    OxCacheError::InvalidInput(t(
        MSG_DETAIL_LUA_SCRIPT_FORBIDDEN_PATTERN,
        &[("detail", command)],
    ))
}

/// 解析 `.member` 或 `['member']` 形式的成员访问
///
/// 返回 (成员名, 成员名之后的第一个 token 下标)。
#[cfg(feature = "redis")]
fn parse_lua_member(toks: &[LuaTok], idx: usize) -> Option<(String, usize)> {
    match toks.get(idx) {
        Some(LuaTok::Punct('.')) => match toks.get(idx + 1) {
            Some(LuaTok::Ident(name)) => Some((name.clone(), idx + 2)),
            _ => None,
        },
        Some(LuaTok::Punct('[')) => {
            // 找到匹配的 ']'，折叠其中的字符串拼接（['cal'..'l']）
            let mut depth = 1usize;
            let mut j = idx + 1;
            while j < toks.len() {
                match &toks[j] {
                    LuaTok::Punct('[') => depth += 1,
                    LuaTok::Punct(']') => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            if j >= toks.len() {
                return None;
            }
            let name = fold_lua_strings(&toks[idx + 1..j])?;
            Some((name, j + 1))
        }
        _ => None,
    }
}

/// 解析调用实参列表
///
/// 返回 (各实参的 token 序列, 调用结束后的 token 下标)。实参按顶层逗号切分，
/// 嵌套的括号 / 方括号 / 花括号内的逗号不切分。
#[cfg(feature = "redis")]
fn parse_lua_args(toks: &[LuaTok], open_idx: usize) -> (Vec<Vec<LuaTok>>, usize) {
    let mut args: Vec<Vec<LuaTok>> = Vec::new();
    let mut current: Vec<LuaTok> = Vec::new();
    let mut depth = 0usize;
    let mut i = open_idx + 1;
    while i < toks.len() {
        match &toks[i] {
            LuaTok::Punct('(') | LuaTok::Punct('[') | LuaTok::Punct('{') => {
                depth += 1;
                current.push(toks[i].clone());
            }
            LuaTok::Punct(')') => {
                if depth == 0 {
                    if !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                    }
                    return (args, i + 1);
                }
                depth -= 1;
                current.push(toks[i].clone());
            }
            LuaTok::Punct(']') | LuaTok::Punct('}') => {
                depth = depth.saturating_sub(1);
                current.push(toks[i].clone());
            }
            LuaTok::Punct(',') if depth == 0 => {
                args.push(std::mem::take(&mut current));
            }
            other => current.push(other.clone()),
        }
        i += 1;
    }
    // 未闭合的实参列表：命令名无法静态确定，交由调用方按拒绝处理
    if !current.is_empty() {
        args.push(current);
    }
    (args, toks.len())
}

/// 折叠字符串字面量拼接：`'a' .. 'b'` → `ab`
///
/// token 序列不是「字符串字面量（'..' 字符串字面量）*」形态时返回 None。
#[cfg(feature = "redis")]
fn fold_lua_strings(toks: &[LuaTok]) -> Option<String> {
    let mut out = String::new();
    let mut parts = 0usize;
    let mut i = 0usize;
    while let Some(LuaTok::Str(s)) = toks.get(i) {
        out.push_str(s);
        parts += 1;
        i += 1;
        match (toks.get(i), toks.get(i + 1)) {
            (Some(LuaTok::Punct('.')), Some(LuaTok::Punct('.'))) => {
                i += 2;
            }
            _ => break,
        }
    }
    if parts == 0 || i != toks.len() {
        return None;
    }
    Some(out)
}

/// 剥去 token 序列最外层成对的括号
#[cfg(feature = "redis")]
fn strip_outer_parens(toks: &[LuaTok]) -> &[LuaTok] {
    let mut current = toks;
    while current.len() >= 2
        && matches!(current.first(), Some(LuaTok::Punct('(')))
        && matches!(current.last(), Some(LuaTok::Punct(')')))
    {
        let mut depth = 0i32;
        let mut wraps_all = true;
        for (idx, tok) in current.iter().enumerate() {
            if let LuaTok::Punct(c) = tok {
                if *c == '(' {
                    depth += 1;
                } else if *c == ')' {
                    depth -= 1;
                    if depth == 0 && idx + 1 != current.len() {
                        wraps_all = false;
                        break;
                    }
                }
            }
        }
        if !wraps_all {
            break;
        }
        current = &current[1..current.len() - 1];
    }
    current
}

/// 收集 redis 的表别名与函数引用别名
///
/// - 表别名：`local r = redis`（赋值右侧紧跟成员访问时不是别名）；
/// - 函数引用别名：`local f = redis.call` / `local f = redis['call']`，
///   以及别名派生（`local g = f`）。函数引用必须单独收集：它没有成员
///   访问形态，按 `redis.` 识别不到，漏掉即等于放开危险命令的别名通道。
#[cfg(feature = "redis")]
fn collect_lua_aliases(toks: &[LuaTok]) -> LuaAliases {
    let mut aliases = LuaAliases {
        tables: vec!["redis".to_string()],
        calls: Vec::new(),
    };
    // 别名可层层派生（r = redis → f = r.call → g = f），迭代到不动点
    loop {
        let mut changed = false;
        for i in 0..toks.len().saturating_sub(2) {
            let LuaTok::Ident(name) = &toks[i] else {
                continue;
            };
            if !matches!(toks[i + 1], LuaTok::Punct('=')) {
                continue;
            }
            let LuaTok::Ident(rhs) = &toks[i + 2] else {
                continue;
            };

            // `X = <表别名>.<call|pcall>`：函数引用别名
            if aliases
                .tables
                .iter()
                .any(|table| table.eq_ignore_ascii_case(rhs))
                && let Some((member, after_member)) = parse_lua_member(toks, i + 3)
                && !matches!(toks.get(after_member), Some(LuaTok::Punct('(')))
            {
                let member = member.to_ascii_lowercase();
                if (member == "call" || member == "pcall")
                    && !aliases.calls.iter().any(|c| c.eq_ignore_ascii_case(name))
                {
                    aliases.calls.push(name.clone());
                    changed = true;
                }
                continue;
            }

            // `X = <函数引用别名>`：别名再派生
            if aliases.calls.iter().any(|c| c.eq_ignore_ascii_case(rhs))
                && !aliases.calls.iter().any(|c| c.eq_ignore_ascii_case(name))
            {
                aliases.calls.push(name.clone());
                changed = true;
                continue;
            }

            // `X = redis`：表别名（右侧紧跟成员访问时不是别名）
            if rhs.eq_ignore_ascii_case("redis")
                && !matches!(
                    toks.get(i + 3),
                    Some(LuaTok::Punct('.')) | Some(LuaTok::Punct('['))
                )
                && !aliases.tables.iter().any(|t| t.eq_ignore_ascii_case(name))
            {
                aliases.tables.push(name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    aliases
}

/// Lua 词法分析
///
/// 注释被整段丢弃；引号字符串与长字符串产出内容（已解转义）；
/// 其余产出标识符与单字符标点。
#[cfg(feature = "redis")]
fn lex_lua(script: &str) -> Vec<LuaTok> {
    let mut toks: Vec<LuaTok> = Vec::new();
    let mut chars = script.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        // 注释：行注释与长注释整段丢弃
        if c == '-' && chars.peek() == Some(&'-') {
            chars.next();
            skip_lua_comment(&mut chars);
            continue;
        }
        // 长字符串 [[...]] / [==[...]==]
        if c == '[' {
            if let Some(content) = lex_lua_long_string(&mut chars) {
                toks.push(LuaTok::Str(content));
            } else {
                toks.push(LuaTok::Punct('['));
            }
            continue;
        }
        // 引号字符串
        if c == '\'' || c == '"' {
            toks.push(LuaTok::Str(lex_lua_quoted_string(&mut chars, c)));
            continue;
        }
        // 标识符 / 关键字
        if c.is_alphabetic() || c == '_' {
            let mut name = String::new();
            name.push(c);
            while let Some(&n) = chars.peek() {
                if n.is_alphanumeric() || n == '_' {
                    name.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            toks.push(LuaTok::Ident(name));
            continue;
        }
        toks.push(LuaTok::Punct(c));
    }
    toks
}

/// 读取长字符串内容（`[` 已消费）
///
/// 非长字符串时返回 None 且不消费任何输入。
#[cfg(feature = "redis")]
fn lex_lua_long_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<String> {
    let mut probe = chars.clone();
    let mut level = 0usize;
    while probe.peek() == Some(&'=') {
        probe.next();
        level += 1;
    }
    if probe.peek() != Some(&'[') {
        return None;
    }
    for _ in 0..level {
        chars.next();
    }
    chars.next(); // '['
    let closing = format!("]{}]", "=".repeat(level));
    let mut content = String::new();
    loop {
        match chars.next() {
            None => break,
            Some(ch) => {
                content.push(ch);
                if content.ends_with(&closing) {
                    let cut = content.len() - closing.len();
                    content.truncate(cut);
                    break;
                }
            }
        }
    }
    Some(content)
}

/// 读取引号字符串内容（起始引号已消费）
#[cfg(feature = "redis")]
fn lex_lua_quoted_string(chars: &mut std::iter::Peekable<std::str::Chars>, quote: char) -> String {
    let mut content = String::new();
    while let Some(c) = chars.next() {
        match c {
            '\\' => lex_lua_escape(chars, &mut content),
            c if c == quote => break,
            '\n' => {
                content.push('\n');
                break;
            }
            c => content.push(c),
        }
    }
    content
}

/// 解析 Lua 字符串转义并写入输出
///
/// 转义必须被还原：`'\070LUSHALL'` 在 Lua 中等于 `'FLUSHALL'`，
/// 不还原会让危险命令名绕过命令级判定。
#[cfg(feature = "redis")]
fn lex_lua_escape(chars: &mut std::iter::Peekable<std::str::Chars>, out: &mut String) {
    let Some(escaped) = chars.next() else {
        return;
    };
    match escaped {
        'n' => out.push('\n'),
        't' => out.push('\t'),
        'r' => out.push('\r'),
        'a' => out.push('\x07'),
        'b' => out.push('\x08'),
        'f' => out.push('\x0c'),
        'v' => out.push('\x0b'),
        'x' => {
            let mut hex = String::new();
            for _ in 0..2 {
                match chars.peek() {
                    Some(c) if c.is_ascii_hexdigit() => {
                        hex.push(*c);
                        chars.next();
                    }
                    _ => break,
                }
            }
            match u8::from_str_radix(&hex, 16) {
                Ok(v) => out.push(v as char),
                Err(_) => {
                    out.push('x');
                    out.push_str(&hex);
                }
            }
        }
        d if d.is_ascii_digit() => {
            let mut num = String::new();
            num.push(d);
            for _ in 0..2 {
                match chars.peek() {
                    Some(c) if c.is_ascii_digit() => {
                        num.push(*c);
                        chars.next();
                    }
                    _ => break,
                }
            }
            match num.parse::<u8>() {
                Ok(v) => out.push(v as char),
                Err(_) => out.push_str(&num),
            }
        }
        other => out.push(other),
    }
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
#[cfg_attr(docsrs, doc(cfg(feature = "redis")))]
pub fn validate_scan_pattern(pattern: &str) -> OxCacheResult<()> {
    if pattern.len() > MAX_SCAN_PATTERN_LENGTH {
        return Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_LONG,
            &[
                ("max", MAX_SCAN_PATTERN_LENGTH.to_string()),
                ("actual", pattern.len().to_string()),
            ],
        )));
    }

    let wildcard_count = pattern.chars().filter(|c| *c == '*').count();

    if wildcard_count > MAX_SCAN_WILDCARDS {
        return Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_MANY_WILDCARDS,
            &[
                ("max", MAX_SCAN_WILDCARDS.to_string()),
                ("actual", wildcard_count.to_string()),
            ],
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
#[cfg_attr(docsrs, doc(cfg(feature = "redis")))]
pub fn clamp_scan_count(count: usize) -> usize {
    count.clamp(SCAN_COUNT_MIN, SCAN_COUNT_MAX)
}
