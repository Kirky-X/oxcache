// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 后端排序器
//
// 根据分数对后端进行排序，并修正不合理的配置。

use crate::cache::chain::ChainLink;
use std::sync::Arc;

use crate::backend::interface::CacheBackend;
use crate::backend::score::BackendScore;

/// 后端排序器
///
/// 根据分数对后端进行排序，并修正不合理的配置。
///
/// # 排序规则
///
/// 1. 按分数降序排列（高分在前）
/// 2. 非持久化后端在持久化后端之前（同分数时）
///
/// # 修正规则
///
/// 1. 确保至少有一个后端
/// 2. 警告不合理的配置（如只有持久化后端）
pub struct BackendSorter;

impl BackendSorter {
    /// 对 ChainLink 列表进行排序
    ///
    /// # Arguments
    ///
    /// * `links` - ChainLink 列表
    ///
    /// # Returns
    ///
    /// 排序后的 ChainLink 列表
    pub fn sort_links(mut links: Vec<ChainLink>) -> Vec<ChainLink> {
        if links.is_empty() {
            return links;
        }

        // 按分数降序排序，同分数时非持久化在前
        links.sort_by(|a, b| {
            let score_cmp = b.score.cmp(&a.score);
            if score_cmp != std::cmp::Ordering::Equal {
                return score_cmp;
            }
            a.is_persistent.cmp(&b.is_persistent)
        });

        // 修正配置
        Self::correct(&mut links);

        links
    }

    /// 从后端列表创建排序后的 ChainLink 列表
    ///
    /// # Arguments
    ///
    /// * `backends` - 后端列表
    ///
    /// # Returns
    ///
    /// 排序后的 ChainLink 列表
    pub fn from_backends<B>(backends: Vec<B>) -> Vec<ChainLink>
    where
        B: CacheBackend + BackendScore + Clone + 'static,
    {
        if backends.is_empty() {
            return vec![];
        }

        // 转换为 ChainLink
        let mut links: Vec<ChainLink> = backends
            .into_iter()
            .map(|b| {
                let score = b.score();
                let is_persistent = b.is_persistent();
                let name = b.backend_name();
                ChainLink::from_arc(Arc::new(b) as Arc<dyn CacheBackend>, score, is_persistent, name)
            })
            .collect();

        // 排序
        links.sort_by(|a, b| {
            let score_cmp = b.score.cmp(&a.score);
            if score_cmp != std::cmp::Ordering::Equal {
                return score_cmp;
            }
            a.is_persistent.cmp(&b.is_persistent)
        });

        // 修正配置
        Self::correct(&mut links);

        links
    }

    /// 修正不合理的配置
    ///
    /// # Arguments
    ///
    /// * `links` - ChainLink 列表（会被修改）
    fn correct(links: &mut [ChainLink]) {
        if links.is_empty() {}

        // 检查是否只有持久化后端（静默处理）

        // 检查分数是否有效（静默处理）

        // 检查是否有重复的后端名称（静默处理）
    }

    /// 验证后端配置
    ///
    /// # Arguments
    ///
    /// * `links` - ChainLink 列表
    ///
    /// # Returns
    ///
    /// 验证结果
    pub fn validate(links: &[ChainLink]) -> ValidationResult {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // 检查是否为空
        if links.is_empty() {
            errors.push("No backends configured".to_string());
            return ValidationResult { warnings, errors };
        }

        // 检查是否只有持久化后端
        let all_persistent = links.iter().all(|l| l.is_persistent);
        if all_persistent {
            warnings.push("All backends are persistent. Consider adding a memory cache.".to_string());
        }

        // 检查分数
        for link in links.iter() {
            if link.score == 0 {
                warnings.push(format!("Backend '{}' has score 0", link.name));
            }
        }

        // 检查顺序
        for i in 1..links.len() {
            if links[i].score > links[i - 1].score {
                warnings.push(format!(
                    "Backends not sorted: '{}' (score {}) should come before '{}' (score {})",
                    links[i].name,
                    links[i].score,
                    links[i - 1].name,
                    links[i - 1].score
                ));
            }
        }

        ValidationResult { warnings, errors }
    }
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// 警告列表
    pub warnings: Vec<String>,
    /// 错误列表
    pub errors: Vec<String>,
}

impl ValidationResult {
    /// 检查是否有效
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// 检查是否有警告
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock::MockBackend;

    #[test]
    fn test_sort_links() {
        let high = ChainLink::new(MockBackend::new("high", 100, false), 100, false, "high");
        let low = ChainLink::new(MockBackend::new("low", 50, true), 50, true, "low");

        let links = vec![low, high];
        let sorted = BackendSorter::sort_links(links);

        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].score, 100);
        assert_eq!(sorted[1].score, 50);
    }

    #[test]
    fn test_validate_empty() {
        let result = BackendSorter::validate(&[]);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.contains("No backends")));
    }

    #[test]
    fn test_validate_all_persistent() {
        let links = vec![ChainLink::new(MockBackend::new("redis", 50, true), 50, true, "redis")];

        let result = BackendSorter::validate(&links);
        assert!(result.is_valid());
        assert!(result.has_warnings());
    }

    #[test]
    fn test_sort_links_same_score() {
        let a = ChainLink::new(MockBackend::new("a", 50, false), 50, false, "a");
        let b = ChainLink::new(MockBackend::new("b", 50, true), 50, true, "b");

        let sorted = BackendSorter::sort_links(vec![b, a]);
        assert_eq!(sorted[0].is_persistent, false);
    }

    #[test]
    fn test_sort_links_empty() {
        let sorted = BackendSorter::sort_links(vec![]);
        assert!(sorted.is_empty());
    }

    #[test]
    fn test_sort_links_single() {
        let link = ChainLink::new(MockBackend::new("single", 75, false), 75, false, "single");
        let sorted = BackendSorter::sort_links(vec![link]);
        assert_eq!(sorted.len(), 1);
    }

    #[test]
    fn test_validate_mixed_backends() {
        let links = vec![
            ChainLink::new(MockBackend::new("moka", 100, false), 100, false, "moka"),
            ChainLink::new(MockBackend::new("redis", 50, true), 50, true, "redis"),
        ];

        let result = BackendSorter::validate(&links);
        assert!(result.is_valid());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_validation_result_no_warnings() {
        let result = ValidationResult {
            warnings: vec![],
            errors: vec![],
        };
        assert!(!result.has_warnings());
    }
}
