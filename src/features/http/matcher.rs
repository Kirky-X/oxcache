//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 路径模式匹配器

/// 路径模式匹配器
///
/// 支持 glob 风格的路径模式匹配。
#[derive(Debug, Clone, Default)]
pub struct PathPatternMatcher;

impl PathPatternMatcher {
    /// 创建新的模式匹配器
    pub fn new() -> Self {
        Self
    }

    /// 检查路径是否匹配模式
    pub fn matches(&self, path: &str, pattern: &str) -> bool {
        glob_match::glob_match(pattern, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_exact_equal() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users", "/api/users"));
    }

    #[test]
    fn test_matches_exact_different() {
        let m = PathPatternMatcher::new();
        assert!(!m.matches("/api/users", "/api/posts"));
    }

    #[test]
    fn test_matches_empty_strings() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("", ""));
    }

    #[test]
    fn test_matches_single_star_match() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users", "/api/*"));
    }

    #[test]
    fn test_matches_single_star_no_match_extra_segment() {
        let m = PathPatternMatcher::new();
        assert!(!m.matches("/api/users/123", "/api/*"));
    }

    #[test]
    fn test_matches_single_star_middle_segment() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/v1/users", "/api/*/users"));
    }

    #[test]
    fn test_matches_single_star_middle_no_match_extra() {
        let m = PathPatternMatcher::new();
        assert!(!m.matches("/api/v1/v2/users", "/api/*/users"));
    }

    #[test]
    fn test_matches_single_star_prefix() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users", "/*/users"));
    }

    #[test]
    fn test_matches_double_star_root() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users/123", "/api/**"));
    }

    #[test]
    fn test_matches_double_star_deep() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users/123/profile/pic", "/api/**"));
    }

    #[test]
    fn test_matches_double_star_no_match() {
        let m = PathPatternMatcher::new();
        assert!(!m.matches("/other/users", "/api/**"));
    }

    #[test]
    fn test_matches_double_star_single_segment() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users", "/api/**"));
    }

    #[test]
    fn test_matches_double_star_with_prefix() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/v1/users/123", "/api/**/users/123"));
    }

    #[test]
    fn test_matches_segment_wildcard() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users", "/api/use*"));
    }

    #[test]
    fn test_matches_segment_partial() {
        let m = PathPatternMatcher::new();
        assert!(m.matches("/api/users", "/api/user*"));
    }

    #[test]
    fn test_matches_segment_no_match() {
        let m = PathPatternMatcher::new();
        assert!(!m.matches("/api/posts", "/api/user*"));
    }

    #[test]
    fn test_matches_no_star_different_length() {
        let m = PathPatternMatcher::new();
        assert!(!m.matches("/a/b", "/a"));
    }

    #[test]
    fn test_new_returns_default() {
        let m = PathPatternMatcher::new();
        let m2 = PathPatternMatcher::default();
        assert!(m.matches("/api/users", "/api/users"));
        assert!(m2.matches("/api/users", "/api/users"));
    }
}
