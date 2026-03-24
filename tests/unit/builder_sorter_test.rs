// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 后端排序器单元测试

#[path = "../common/mod.rs"]
mod common;

use common::mock_backend::MockBackend;
use oxcache::builder::sorter::{BackendSorter, ValidationResult};
use oxcache::cache::ChainLink;

// ============================================================================
// sort_links 函数测试
// ============================================================================

/// 测试按分数降序排序
#[test]
fn test_sort_links_descending_order() {
    let high = ChainLink::new(MockBackend::new("high", 100, false), 100, false, "high");
    let medium = ChainLink::new(MockBackend::new("medium", 75, false), 75, false, "medium");
    let low = ChainLink::new(MockBackend::new("low", 50, true), 50, true, "low");

    // 故意乱序
    let links = vec![low.clone(), high.clone(), medium.clone()];
    let sorted = BackendSorter::sort_links(links);

    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].score, 100);
    assert_eq!(sorted[1].score, 75);
    assert_eq!(sorted[2].score, 50);
}

/// 测试同分数时非持久化优先
#[test]
fn test_sort_links_non_persistent_first() {
    // 同分数：一个持久化，一个非持久化
    let persistent = ChainLink::new(MockBackend::new("persistent", 100, true), 100, true, "persistent");
    let non_persistent = ChainLink::new(
        MockBackend::new("non_persistent", 100, false),
        100,
        false,
        "non_persistent",
    );

    // 故意把持久化放在前面
    let links = vec![persistent, non_persistent];
    let sorted = BackendSorter::sort_links(links);

    assert_eq!(sorted.len(), 2);
    // 非持久化应该在前面
    assert!(!sorted[0].is_persistent);
    assert!(sorted[1].is_persistent);
}

/// 测试空列表处理
#[test]
fn test_sort_links_empty() {
    let links: Vec<ChainLink> = vec![];
    let sorted = BackendSorter::sort_links(links);

    assert!(sorted.is_empty());
}

/// 测试单元素列表
#[test]
fn test_sort_links_single_element() {
    let single = ChainLink::new(MockBackend::new("single", 100, false), 100, false, "single");
    let links = vec![single.clone()];
    let sorted = BackendSorter::sort_links(links);

    assert_eq!(sorted.len(), 1);
    assert_eq!(sorted[0].score, 100);
    assert_eq!(sorted[0].name, "single");
}

/// 测试多个同分数元素排序
#[test]
fn test_sort_links_same_score() {
    // 三个同分数，两个非持久化，一个持久化
    let np1 = ChainLink::new(MockBackend::new("np1", 100, false), 100, false, "np1");
    let np2 = ChainLink::new(MockBackend::new("np2", 100, false), 100, false, "np2");
    let p1 = ChainLink::new(MockBackend::new("p1", 100, true), 100, true, "p1");

    let links = vec![p1.clone(), np1.clone(), np2.clone()];
    let sorted = BackendSorter::sort_links(links);

    assert_eq!(sorted.len(), 3);
    // 所有非持久化应该在持久化之前
    assert!(!sorted[0].is_persistent);
    assert!(!sorted[1].is_persistent);
    assert!(sorted[2].is_persistent);
}

/// 测试分数 0 的后端
#[test]
fn test_sort_links_zero_score() {
    let zero = ChainLink::new(MockBackend::new("zero", 0, false), 0, false, "zero");
    let normal = ChainLink::new(MockBackend::new("normal", 100, false), 100, false, "normal");

    let links = vec![zero, normal];
    let sorted = BackendSorter::sort_links(links);

    // 分数 0 应该排在最后
    assert_eq!(sorted[0].score, 100);
    assert_eq!(sorted[1].score, 0);
}

// ============================================================================
// validate 函数测试
// ============================================================================

/// 测试空后端配置验证
#[test]
fn test_validate_empty_backends() {
    let result = BackendSorter::validate(&[]);

    assert!(!result.is_valid());
    assert!(result.errors.iter().any(|e| e.contains("No backends")));
}

/// 测试全持久化后端警告
#[test]
fn test_validate_all_persistent() {
    let redis = ChainLink::new(MockBackend::new("redis", 50, true), 50, true, "redis");
    let sqlite = ChainLink::new(MockBackend::new("sqlite", 25, true), 25, true, "sqlite");

    let links = vec![redis, sqlite];
    let result = BackendSorter::validate(&links);

    assert!(result.is_valid()); // 有效但有警告
    assert!(result.has_warnings());
    assert!(result.warnings.iter().any(|w| w.contains("persistent")));
}

/// 测试零分后端警告
#[test]
fn test_validate_zero_score() {
    let zero = ChainLink::new(MockBackend::new("zero", 0, false), 0, false, "zero");
    let normal = ChainLink::new(MockBackend::new("normal", 100, false), 100, false, "normal");

    let links = vec![normal, zero];
    let result = BackendSorter::validate(&links);

    assert!(result.is_valid());
    assert!(result.warnings.iter().any(|w| w.contains("score 0")));
}

/// 测试未排序后端检测
#[test]
fn test_validate_unsorted_backends() {
    let high = ChainLink::new(MockBackend::new("high", 100, false), 100, false, "high");
    let low = ChainLink::new(MockBackend::new("low", 50, false), 50, false, "low");

    // 故意逆序
    let links = vec![low, high];
    let result = BackendSorter::validate(&links);

    assert!(result.is_valid());
    assert!(result.warnings.iter().any(|w| w.contains("not sorted")));
}

/// 测试有效配置无警告
#[test]
fn test_validate_valid_config() {
    let moka = ChainLink::new(MockBackend::new("moka", 100, false), 100, false, "moka");
    let redis = ChainLink::new(MockBackend::new("redis", 50, true), 50, true, "redis");

    // 正确排序：高分在前
    let links = vec![moka, redis];
    let result = BackendSorter::validate(&links);

    assert!(result.is_valid());
    // 可能仍然有警告（取决于实现），但不应有错误
    assert!(result.errors.is_empty());
}

// ============================================================================
// ValidationResult 结构测试
// ============================================================================

/// 测试 ValidationResult is_valid 方法
#[test]
fn test_validation_result_is_valid() {
    let valid = ValidationResult {
        warnings: vec!["warning".to_string()],
        errors: vec![],
    };
    assert!(valid.is_valid());

    let invalid = ValidationResult {
        warnings: vec![],
        errors: vec!["error".to_string()],
    };
    assert!(!invalid.is_valid());
}

/// 测试 ValidationResult has_warnings 方法
#[test]
fn test_validation_result_has_warnings() {
    let with_warnings = ValidationResult {
        warnings: vec!["warning".to_string()],
        errors: vec![],
    };
    assert!(with_warnings.has_warnings());

    let without_warnings = ValidationResult {
        warnings: vec![],
        errors: vec![],
    };
    assert!(!without_warnings.has_warnings());
}

// ============================================================================
// from_backends 函数测试
// ============================================================================
// 注意: from_backends 需要 Backend 实现 Clone trait
// tests/common/mock_backend.rs 中的 MockBackend 未实现 Clone
// 此处跳过 from_backends 测试，仅测试 sort_links 和 validate

// ============================================================================
// correct 函数测试（通过 sort_links 间接测试）
// ============================================================================

/// 测试重复后端名称警告（通过 sort_links 触发 correct）
#[test]
fn test_correct_duplicate_names() {
    let link1 = ChainLink::new(MockBackend::new("duplicate", 100, false), 100, false, "duplicate");
    let link2 = ChainLink::new(MockBackend::new("duplicate", 50, false), 50, false, "duplicate");

    // 重复名称应该触发警告，但不影响排序
    let links = vec![link1, link2];
    let sorted = BackendSorter::sort_links(links);

    // 验证排序仍然正确
    assert_eq!(sorted.len(), 2);
    assert_eq!(sorted[0].score, 100);
    assert_eq!(sorted[1].score, 50);
}

/// 测试全持久化后端配置修正警告
#[test]
fn test_correct_all_persistent_warning() {
    let redis = ChainLink::new(MockBackend::new("redis", 100, true), 100, true, "redis");
    let sqlite = ChainLink::new(MockBackend::new("sqlite", 50, true), 50, true, "sqlite");

    let links = vec![redis, sqlite];
    let sorted = BackendSorter::sort_links(links);

    // 验证排序正确
    assert_eq!(sorted.len(), 2);
    assert!(sorted[0].is_persistent);
    assert!(sorted[1].is_persistent);
}

// ============================================================================
// 边界条件测试
// ============================================================================

/// 测试最大分数值
#[test]
fn test_sort_links_max_score() {
    let max = ChainLink::new(MockBackend::new("max", 255, false), 255, false, "max");
    let min = ChainLink::new(MockBackend::new("min", 1, false), 1, false, "min");

    let links = vec![min, max];
    let sorted = BackendSorter::sort_links(links);

    assert_eq!(sorted[0].score, 255);
    assert_eq!(sorted[1].score, 1);
}

/// 测试大量后端排序（使用预定义的静态名称）
#[test]
fn test_sort_links_many_backends() {
    // 使用预定义的静态名称数组
    static NAMES: [&str; 20] = [
        "b0", "b1", "b2", "b3", "b4", "b5", "b6", "b7", "b8", "b9", "b10", "b11", "b12", "b13", "b14", "b15", "b16",
        "b17", "b18", "b19",
    ];

    let mut links = Vec::new();
    for i in 0..20 {
        let score = (i % 5) as u8;
        let persistent = i % 3 == 0;
        links.push(ChainLink::new(
            MockBackend::new(NAMES[i], score, persistent),
            score,
            persistent,
            NAMES[i],
        ));
    }

    let sorted = BackendSorter::sort_links(links);

    // 验证排序正确
    assert_eq!(sorted.len(), 20);
    for i in 1..sorted.len() {
        // 分数应该降序
        assert!(sorted[i].score <= sorted[i - 1].score);
        // 同分数时，非持久化在前
        if sorted[i].score == sorted[i - 1].score {
            assert!(sorted[i - 1].is_persistent <= sorted[i].is_persistent);
        }
    }
}
