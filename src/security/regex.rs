// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// Security utilities for regex and pattern validation
//
// Provides protection against ReDoS attacks and regex complexity limits.

// Public API surface — re-exported via security::mod.rs for external consumers.
#![allow(dead_code)]

use crate::error::{OxCacheError, OxCacheResult};
use regex::Regex;

/// Maximum allowed pattern length
pub const MAX_PATTERN_LENGTH: usize = 256;

/// Maximum number of wildcards allowed in a pattern
pub const MAX_WILDCARDS: usize = 10;

/// Compiles a regex pattern with safety checks
///
/// # Arguments
///
/// * `pattern` - The regex pattern to compile
///
/// # Returns
///
/// * `Ok(Regex)` - Successfully compiled regex
/// * `Err(OxCacheError)` - Compilation failed or pattern is unsafe
pub fn compile_regex(pattern: &str) -> OxCacheResult<regex::Regex> {
    // Check pattern length
    if pattern.len() > MAX_PATTERN_LENGTH {
        return Err(OxCacheError::InvalidInput(format!(
            "Regex pattern exceeds maximum length of {} bytes (got {})",
            MAX_PATTERN_LENGTH,
            pattern.len()
        )));
    }

    // Count wildcards (for potential ReDoS patterns)
    let wildcard_count = pattern.bytes().filter(|&b| b == b'*' || b == b'+').count();
    if wildcard_count > MAX_WILDCARDS {
        return Err(OxCacheError::InvalidInput(format!(
            "Regex pattern contains too many quantifiers ({} > {})",
            wildcard_count, MAX_WILDCARDS
        )));
    }

    // Check for dangerous patterns that could cause exponential backtracking
    // Patterns like (a+)+ or (a?)+ can cause ReDoS
    // We check for nested quantifiers which can cause catastrophic backtracking
    // Note: We use a more precise pattern to avoid false positives from glob conversions like [^/]
    let dangerous_patterns = [
        r"\([^)]*\)\++",          // (something)+ followed by one or more + (ReDoS pattern)
        r"\([^)]*(\([^)]*\))+\)", // Nested parentheses with quantifiers
    ];

    for dangerous in &dangerous_patterns {
        if let Ok(dangerous_regex) = Regex::new(dangerous) {
            if dangerous_regex.is_match(pattern) {
                return Err(OxCacheError::InvalidInput(
                    "Regex pattern contains potentially dangerous quantifier pattern".to_string(),
                ));
            }
        }
    }

    // Compile the regex
    Regex::new(pattern).map_err(|e| OxCacheError::InvalidInput(format!("Invalid regex pattern: {}", e)))
}

/// Matches a string against a compiled regex with input length check
///
/// # Arguments
///
/// * `regex` - The compiled regex
/// * `input` - The string to match against
///
/// # Returns
///
/// * `Ok(bool)` - Match result
/// * `Err(OxCacheError)` - Input too long
pub fn match_safe(regex: &Regex, input: &str) -> OxCacheResult<bool> {
    // Check input length for extremely long inputs
    if input.len() > 1_000_000 {
        return Err(OxCacheError::InvalidInput(
            "Input string too long for regex matching".to_string(),
        ));
    }

    Ok(regex.is_match(input))
}

/// Increments the wildcard counter and checks against the limit.
/// Returns `Err` if the count exceeds [`MAX_WILDCARDS`].
fn increment_wildcard(count: &mut usize) -> OxCacheResult<()> {
    *count += 1;
    if *count > MAX_WILDCARDS {
        return Err(OxCacheError::InvalidInput(format!(
            "Glob pattern contains too many wildcards (max {})",
            MAX_WILDCARDS
        )));
    }
    Ok(())
}

/// Converts a glob pattern to regex with safety checks
///
/// # Arguments
///
/// * `pattern` - The glob pattern
/// * `double_star_allowed` - Whether to allow ** glob patterns
///
/// # Returns
///
/// * `Ok(String)` - Regex pattern
/// * `Err(OxCacheError)` - Pattern conversion failed or unsafe
pub fn glob_to_regex(pattern: &str, double_star_allowed: bool) -> OxCacheResult<String> {
    if pattern.len() > MAX_PATTERN_LENGTH {
        return Err(OxCacheError::InvalidInput(format!(
            "Glob pattern exceeds maximum length of {} bytes (got {})",
            MAX_PATTERN_LENGTH,
            pattern.len()
        )));
    }

    let mut regex_pattern = String::with_capacity(pattern.len() * 2);
    let mut chars = pattern.chars().peekable();
    let mut in_escape = false;
    let mut wildcard_count: usize = 0;

    while let Some(c) = chars.next() {
        if in_escape {
            regex_pattern.push_str(&regex::escape(&c.to_string()));
            in_escape = false;
            continue;
        }

        match c {
            '\\' if !in_escape => {
                in_escape = true;
            }
            '*' => {
                if double_star_allowed && chars.clone().next() == Some('*') {
                    chars.next();
                    increment_wildcard(&mut wildcard_count)?;
                    if chars.peek() == Some(&'/') {
                        chars.next();
                        regex_pattern.push_str("(?:.*/)?");
                    } else {
                        regex_pattern.push_str(".*");
                    }
                } else {
                    increment_wildcard(&mut wildcard_count)?;
                    regex_pattern.push_str("[^/]*");
                }
            }
            '?' => {
                increment_wildcard(&mut wildcard_count)?;
                regex_pattern.push('.');
            }
            '[' => {
                return Err(OxCacheError::InvalidInput(
                    "Character class '[...]' not allowed in glob patterns".to_string(),
                ));
            }
            '{' | '}' => {
                return Err(OxCacheError::InvalidInput(
                    "Brace expansion not allowed in glob patterns".to_string(),
                ));
            }
            c => regex_pattern.push_str(&regex::escape(&c.to_string())),
        }
    }

    if in_escape {
        return Err(OxCacheError::InvalidInput(
            "Glob pattern ends with trailing backslash".to_string(),
        ));
    }

    Ok(format!("^{}$", regex_pattern))
}

/// Validates and compiles a glob pattern with safety checks
///
/// # Arguments
///
/// * `pattern` - The glob pattern
/// * `double_star_allowed` - Whether to allow ** glob patterns
///
/// # Returns
///
/// * `Ok(Regex)` - Compiled regex
/// * `Err(OxCacheError)` - Validation or compilation failed
pub fn compile_glob_pattern(pattern: &str, double_star_allowed: bool) -> OxCacheResult<Regex> {
    let regex_pattern = glob_to_regex(pattern, double_star_allowed)?;
    compile_regex(&regex_pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_regex_valid_pattern() {
        let result = compile_regex(".*");
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_regex_invalid_pattern() {
        let result = compile_regex("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_regex_dangerous_pattern() {
        // This pattern could cause ReDoS
        let result = compile_regex(r"(a+)+$");
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_regex_too_long() {
        let long_pattern = "a".repeat(MAX_PATTERN_LENGTH + 1);
        let result = compile_regex(&long_pattern);
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_regex_too_many_quantifiers() {
        let pattern = "*".repeat(MAX_WILDCARDS + 1);
        let result = compile_regex(&pattern);
        assert!(result.is_err());
    }

    #[test]
    fn test_glob_to_regex_simple() {
        let result = glob_to_regex("*.txt", false);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("file.txt"));
        assert!(!regex.is_match("file.md"));
    }

    #[test]
    fn test_glob_to_regex_disallowed_chars() {
        let result = glob_to_regex("[abc]", false);
        assert!(result.is_err());

        let result = glob_to_regex("{a,b}", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_glob_to_regex_too_long() {
        let long_pattern = "a".repeat(MAX_PATTERN_LENGTH + 1);
        let result = glob_to_regex(&long_pattern, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_glob_to_regex_too_many_wildcards() {
        let pattern = "*".repeat(MAX_WILDCARDS + 1);
        let result = glob_to_regex(&pattern, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_match_safe_valid() {
        let regex = Regex::new(".*").unwrap();
        let result = match_safe(&regex, "test");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_match_safe_too_long_input() {
        let regex = Regex::new(".*").unwrap();
        let long_input = "a".repeat(1_000_001);
        let result = match_safe(&regex, &long_input);
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_glob_pattern() {
        let result = compile_glob_pattern("*.rs", false);
        assert!(result.is_ok());
        let regex = result.unwrap();
        assert!(regex.is_match("test.rs"));
        assert!(!regex.is_match("test.txt"));
    }

    // ============================================================================
    // glob_to_regex 双星号测试 (lines 122-124)
    // ============================================================================

    #[test]
    fn test_glob_to_regex_double_star_allowed() {
        let result = glob_to_regex("**/*.rs", true);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("test.rs"));
        assert!(regex.is_match("dir/test.rs"));
        assert!(regex.is_match("dir/subdir/test.rs"));
    }

    #[test]
    fn test_glob_to_regex_double_star_too_many_wildcards() {
        // 双星号模式下通配符过多
        // 使用单个 * 分隔的字符，避免被识别为 **
        let pattern = "*a".repeat(MAX_WILDCARDS + 1);
        let result = glob_to_regex(&pattern, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_glob_to_regex_double_star_no_slash() {
        // ** 后面不是 / 的情况 (line 167)
        let result = glob_to_regex("**file", true);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("dir/file"));
        assert!(regex.is_match("file"));
    }

    #[test]
    fn test_glob_to_regex_double_star_with_slash() {
        // **/ 匹配零或多个目录 (lines 164-165)
        let result = glob_to_regex("**/file", true);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("file"));
        assert!(regex.is_match("dir/file"));
    }

    // ============================================================================
    // 转义字符测试 (lines 143-144, 149-155)
    // ============================================================================

    #[test]
    fn test_glob_to_regex_escape_character() {
        // 反斜杠转义非星号字符 (lines 143-144, 155)
        let result = glob_to_regex("\\a", false);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("a"));
    }

    #[test]
    fn test_glob_to_regex_escaped_star() {
        // \* 表示字面量 * — 通过 regex::escape 产生 \*
        let result = glob_to_regex("\\*", false);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        assert_eq!(regex_pattern, "^\\*$");
        // 验证编译后的正则匹配字面量 *
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("*"));
        assert!(!regex.is_match("a"));
    }

    #[test]
    fn test_glob_to_regex_backslash_at_end() {
        // 反斜杠在末尾（没有后续字符）应返回错误
        let result = glob_to_regex("test\\", false);
        assert!(result.is_err());
    }

    // ============================================================================
    // 问号通配符测试 (line 174)
    // ============================================================================

    #[test]
    fn test_glob_to_regex_question_mark() {
        // ? 匹配任意单个字符 (line 174)
        let result = glob_to_regex("?.txt", false);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("a.txt"));
        assert!(!regex.is_match("ab.txt"));
    }

    #[test]
    fn test_glob_to_regex_mixed_wildcards() {
        // 混合通配符
        let result = glob_to_regex("?*.txt", false);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("a.txt"));
        assert!(regex.is_match("ab.txt"));
        assert!(!regex.is_match(".txt"));
    }

    // ============================================================================
    // compile_regex 边界测试
    // ============================================================================

    #[test]
    fn test_compile_regex_empty_pattern() {
        let result = compile_regex("");
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_regex_exact_length_limit() {
        let pattern = "a".repeat(MAX_PATTERN_LENGTH);
        let result = compile_regex(&pattern);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_regex_exact_quantifier_limit() {
        // 恰好 MAX_WILDCARDS 个量词
        let pattern = "a*".repeat(MAX_WILDCARDS);
        let result = compile_regex(&pattern);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_regex_nested_parentheses_quantifier() {
        // 嵌套括号加量词 - 危险模式
        let result = compile_regex(r"((a+)+)");
        assert!(result.is_err());
    }

    // ============================================================================
    // match_safe 边界测试
    // ============================================================================

    #[test]
    fn test_match_safe_exact_limit() {
        let regex = Regex::new(".*").unwrap();
        let input = "a".repeat(1_000_000);
        let result = match_safe(&regex, &input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_match_safe_no_match() {
        let regex = Regex::new("^b+$").unwrap();
        let result = match_safe(&regex, "aaa");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    // ============================================================================
    // compile_glob_pattern 边界测试
    // ============================================================================

    #[test]
    fn test_compile_glob_pattern_double_star() {
        let result = compile_glob_pattern("**/*.rs", true);
        assert!(result.is_ok());
        let regex = result.unwrap();
        assert!(regex.is_match("test.rs"));
        assert!(regex.is_match("dir/test.rs"));
    }

    #[test]
    fn test_compile_glob_pattern_question_mark() {
        let result = compile_glob_pattern("?.txt", false);
        assert!(result.is_ok());
        let regex = result.unwrap();
        assert!(regex.is_match("a.txt"));
    }

    #[test]
    fn test_glob_to_regex_single_star_no_slash_match() {
        // * 匹配除 / 外的任意字符 (line 171)
        let result = glob_to_regex("*.txt", false);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("file.txt"));
        assert!(!regex.is_match("dir/file.txt"));
    }

    #[test]
    fn test_glob_to_regex_regular_character() {
        // 普通字符通过 regex::escape 处理
        let result = glob_to_regex("test.txt", false);
        assert!(result.is_ok());
        let regex_pattern = result.unwrap();
        let regex = Regex::new(&regex_pattern).unwrap();
        assert!(regex.is_match("test.txt"));
    }

    #[test]
    fn test_glob_to_regex_double_star_allowed_exact_limit() {
        // 双星号模式下恰好达到通配符限制
        let pattern = "**".repeat(MAX_WILDCARDS);
        let result = glob_to_regex(&pattern, true);
        assert!(result.is_ok());
    }
}
