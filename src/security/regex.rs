// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Security utilities for regex and pattern validation
//
// Provides protection against ReDoS attacks and regex complexity limits.

use crate::error::{CacheError, Result};
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
/// * `Err(CacheError)` - Compilation failed or pattern is unsafe
pub fn compile_regex(pattern: &str) -> Result<regex::Regex> {
    // Check pattern length
    if pattern.len() > MAX_PATTERN_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "Regex pattern exceeds maximum length of {} bytes (got {})",
            MAX_PATTERN_LENGTH,
            pattern.len()
        )));
    }

    // Count wildcards (for potential ReDoS patterns)
    let wildcard_count = pattern.bytes().filter(|&b| b == b'*' || b == b'+').count();
    if wildcard_count > MAX_WILDCARDS {
        return Err(CacheError::InvalidInput(format!(
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
                return Err(CacheError::InvalidInput(
                    "Regex pattern contains potentially dangerous quantifier pattern".to_string(),
                ));
            }
        }
    }

    // Compile the regex
    Regex::new(pattern).map_err(|e| CacheError::InvalidInput(format!("Invalid regex pattern: {}", e)))
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
/// * `Err(CacheError)` - Input too long
pub fn match_safe(regex: &Regex, input: &str) -> Result<bool> {
    // Check input length for extremely long inputs
    if input.len() > 1_000_000 {
        return Err(CacheError::InvalidInput(
            "Input string too long for regex matching".to_string(),
        ));
    }

    Ok(regex.is_match(input))
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
/// * `Err(CacheError)` - Pattern conversion failed or unsafe
pub fn glob_to_regex(pattern: &str, double_star_allowed: bool) -> Result<String> {
    // Check pattern length
    if pattern.len() > MAX_PATTERN_LENGTH {
        return Err(CacheError::InvalidInput(format!(
            "Glob pattern exceeds maximum length of {} bytes (got {})",
            MAX_PATTERN_LENGTH,
            pattern.len()
        )));
    }

    // Count wildcards
    let single_star_count = pattern.bytes().filter(|&b| b == b'*').count();
    if double_star_allowed {
        // ** counts as 2 wildcards
        let double_star_count = pattern.matches("**").count();
        if single_star_count - (double_star_count * 2) > MAX_WILDCARDS {
            return Err(CacheError::InvalidInput(format!(
                "Glob pattern contains too many wildcards (max {})",
                MAX_WILDCARDS
            )));
        }
    } else if single_star_count > MAX_WILDCARDS {
        return Err(CacheError::InvalidInput(format!(
            "Glob pattern contains too many wildcards (max {})",
            MAX_WILDCARDS
        )));
    }

    // Convert glob to regex
    let mut regex_pattern = String::with_capacity(pattern.len() * 2);
    let mut chars = pattern.chars().peekable();
    let mut in_escape = false;

    while let Some(c) = chars.next() {
        if in_escape {
            regex_pattern.push_str(&regex::escape(&c.to_string()));
            in_escape = false;
            continue;
        }

        match c {
            '\\' if !in_escape => {
                if chars.peek() == Some(&'*') {
                    // \* means literal *
                    chars.next();
                    regex_pattern.push('*');
                } else {
                    in_escape = true;
                }
            }
            '*' => {
                if double_star_allowed && chars.clone().next() == Some('*') {
                    // ** matches any character including /
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        // **/ matches zero or more directories
                        chars.next();
                        regex_pattern.push_str("(?:.*/)?");
                    } else {
                        regex_pattern.push_str(".*");
                    }
                } else {
                    // * matches any character except /
                    regex_pattern.push_str("[^/]*");
                }
            }
            '?' => regex_pattern.push('.'),
            '[' => {
                // Character class - escape to prevent regex injection
                return Err(CacheError::InvalidInput(
                    "Character class '[...]' not allowed in glob patterns".to_string(),
                ));
            }
            '{' | '}' => {
                return Err(CacheError::InvalidInput(
                    "Brace expansion not allowed in glob patterns".to_string(),
                ));
            }
            c => regex_pattern.push_str(&regex::escape(&c.to_string())),
        }
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
/// * `Err(CacheError)` - Validation or compilation failed
pub fn compile_glob_pattern(pattern: &str, double_star_allowed: bool) -> Result<Regex> {
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
}
