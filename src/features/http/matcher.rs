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
        if pattern.contains("**") {
            self.matches_double_star(path, pattern)
        } else if pattern.contains('*') {
            self.matches_single_star(path, pattern)
        } else {
            path == pattern
        }
    }

    /// 匹配单个 *（匹配单个目录段）
    fn matches_single_star(&self, path: &str, pattern: &str) -> bool {
        let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();

        if path_parts.len() != pattern_parts.len() {
            return false;
        }

        for (path_part, pattern_part) in path_parts.iter().zip(pattern_parts.iter()) {
            if !self.matches_segment(pattern_part, path_part) {
                return false;
            }
        }

        true
    }

    /// 匹配 **（匹配任意数量的目录）
    fn matches_double_star(&self, path: &str, pattern: &str) -> bool {
        let regex_pattern = pattern.replace("**", "§§§").replace("*", "[^/]*").replace("§§§", ".*");

        if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
            re.is_match(path)
        } else {
            false
        }
    }

    /// 匹配单个段落的通配符
    fn matches_segment(&self, pattern: &str, segment: &str) -> bool {
        let regex_pattern: String = pattern
            .chars()
            .map(|c| match c {
                '*' => ".*".to_string(),
                c => regex::escape(&c.to_string()),
            })
            .collect();

        if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
            re.is_match(segment)
        } else {
            pattern == segment
        }
    }
}
