// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// OxCacheBuilder 测试套件
//
// 覆盖 OxCacheBuilder 的所有公共方法、链式调用和验证逻辑。

#[path = "../common/mod.rs"]
mod common;

use common::mock_backend::MockBackend;
use oxcache::builder::OxCacheBuilder;
use oxcache::cache::chain::ChainLink;
use std::time::Duration;

/// 测试 Default trait 实现
#[test]
fn test_default_trait() {
    let builder = OxCacheBuilder::default();
    assert!(builder.is_empty());
    assert_eq!(builder.backend_count(), 0);
}

/// 测试 link() 方法 - 添加 ChainLink
#[test]
fn test_link_method() {
    let backend = MockBackend::new("test", 50, false);
    let link = ChainLink::from_backend(backend);

    let result = OxCacheBuilder::new().link(link).build();
    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 1);
}

/// 测试 backends() 方法 - 批量添加后端
#[test]
fn test_backends_method() {
    let backends = vec![
        MockBackend::new("low", 50, true),
        MockBackend::new("high", 100, false),
        MockBackend::new("mid", 75, false),
    ];

    let result = OxCacheBuilder::new().backends(backends).build();
    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 3);

    // 验证已排序（高分在前）
    assert_eq!(cache.links()[0].score(), 100);
    assert_eq!(cache.links()[1].score(), 75);
    assert_eq!(cache.links()[2].score(), 50);
}

/// 测试 default_ttl() 方法
#[test]
fn test_default_ttl_method() {
    let backend = MockBackend::new("test", 50, false);

    let result = OxCacheBuilder::new()
        .backend(backend)
        .default_ttl(Duration::from_secs(300))
        .build();

    assert!(result.is_ok());
}

/// 测试 enable_backfill() 方法
#[test]
fn test_enable_backfill_method() {
    let backend = MockBackend::new("test", 50, false);

    let result = OxCacheBuilder::new().backend(backend).enable_backfill().build();

    assert!(result.is_ok());
}

/// 测试 disable_backfill() 方法
#[test]
fn test_disable_backfill_method() {
    let backend = MockBackend::new("test", 50, false);

    let result = OxCacheBuilder::new().backend(backend).disable_backfill().build();

    assert!(result.is_ok());
}

/// 测试 max_capacity() 方法
#[test]
fn test_max_capacity_method() {
    let backend = MockBackend::new("test", 50, false);

    let result = OxCacheBuilder::new().backend(backend).max_capacity(10000).build();

    assert!(result.is_ok());
}

/// 测试 build_async() 方法
#[tokio::test]
async fn test_build_async_method() {
    let backend = MockBackend::new("test", 50, false);

    let result = OxCacheBuilder::new().backend(backend).build_async().await;

    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 1);
}

/// 测试 backend_count() 方法
#[test]
fn test_backend_count_method() {
    let builder = OxCacheBuilder::new();
    assert_eq!(builder.backend_count(), 0);

    let backend1 = MockBackend::new("test1", 50, false);
    let builder = builder.backend(backend1);
    assert_eq!(builder.backend_count(), 1);

    let backend2 = MockBackend::new("test2", 100, false);
    let builder = builder.backend(backend2);
    assert_eq!(builder.backend_count(), 2);
}

/// 测试 is_empty() 方法
#[test]
fn test_is_empty_method() {
    let builder = OxCacheBuilder::new();
    assert!(builder.is_empty());

    let backend = MockBackend::new("test", 50, false);
    let builder = builder.backend(backend);
    assert!(!builder.is_empty());
}

/// 测试链式调用组合
#[test]
fn test_chained_calls() {
    let high = MockBackend::new("high", 100, false);
    let low = MockBackend::new("low", 50, true);

    let result = OxCacheBuilder::new()
        .backend(low)
        .backend(high)
        .default_ttl(Duration::from_secs(600))
        .enable_backfill()
        .max_capacity(5000)
        .build();

    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 2);
    // 验证排序
    assert_eq!(cache.links()[0].score(), 100);
    assert_eq!(cache.links()[1].score(), 50);
}

/// 测试混合使用 backend() 和 link()
#[test]
fn test_mixed_backend_and_link() {
    let backend = MockBackend::new("backend", 75, false);
    let link_backend = MockBackend::new("link", 100, false);
    let link = ChainLink::from_backend(link_backend);

    let result = OxCacheBuilder::new().backend(backend).link(link).build();

    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 2);
    // 验证排序
    assert_eq!(cache.links()[0].score(), 100);
    assert_eq!(cache.links()[1].score(), 75);
}

/// 测试多个配置选项组合
#[test]
fn test_multiple_config_options() {
    let backends = vec![MockBackend::new("m1", 50, false), MockBackend::new("m2", 75, false)];

    let result = OxCacheBuilder::new()
        .backends(backends)
        .default_ttl(Duration::from_secs(1800))
        .disable_backfill()
        .max_capacity(100000)
        .build();

    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 2);
}

/// 测试空构建器错误处理
#[test]
fn test_empty_builder_error() {
    let result = OxCacheBuilder::new().build();
    assert!(result.is_err());
    assert!(matches!(result, Err(oxcache::error::CacheError::InvalidInput(_))));
}

/// 测试默认状态：backfill 默认启用
#[test]
fn test_backfill_default_enabled() {
    // 新构建器应该默认启用回填
    let builder = OxCacheBuilder::new();

    // 添加后端后构建
    let backend = MockBackend::new("test", 50, false);
    let result = builder.backend(backend).build();

    assert!(result.is_ok());
}

/// 测试 backfill 开关切换
#[test]
fn test_backfill_toggle() {
    let backend = MockBackend::new("test", 50, false);

    // 启用后再禁用
    let result = OxCacheBuilder::new()
        .backend(backend.clone())
        .enable_backfill()
        .disable_backfill()
        .build();

    assert!(result.is_ok());
}

/// 测试 memory() 快捷方法
#[test]
fn test_memory_shortcut() {
    let result = OxCacheBuilder::memory(1000).build();
    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 1);
}

/// 测试 memory() 不同容量
#[test]
fn test_memory_different_capacities() {
    // 小容量
    let result1 = OxCacheBuilder::memory(10).build();
    assert!(result1.is_ok());

    // 大容量
    let result2 = OxCacheBuilder::memory(1000000).build();
    assert!(result2.is_ok());
}

/// 测试单个后端的完整链式调用
#[test]
fn test_single_backend_full_chain() {
    let backend = MockBackend::new("single", 100, false);

    let result = OxCacheBuilder::new()
        .backend(backend)
        .default_ttl(Duration::from_secs(3600))
        .enable_backfill()
        .max_capacity(10000)
        .build();

    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.links()[0].score(), 100);
}

/// 测试后端排序 - 相同分数不同持久化
#[test]
fn test_backend_sorting_same_score() {
    let persistent = MockBackend::new("persistent", 50, true);
    let non_persistent = MockBackend::new("non_persistent", 50, false);

    // 非持久化应该在同分数持久化之前
    let result = OxCacheBuilder::new()
        .backend(persistent)
        .backend(non_persistent)
        .build();

    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 2);
    // 非持久化应该在前
    assert!(!cache.links()[0].is_persistent());
    assert!(cache.links()[1].is_persistent());
}

/// 测试多个后端复杂排序
#[test]
fn test_complex_backend_sorting() {
    let backends = vec![
        MockBackend::new("b1", 30, true),
        MockBackend::new("b2", 100, false),
        MockBackend::new("b3", 50, false),
        MockBackend::new("b4", 100, true),
        MockBackend::new("b5", 75, false),
    ];

    let result = OxCacheBuilder::new().backends(backends).build();
    assert!(result.is_ok());

    let cache = result.unwrap();
    assert_eq!(cache.len(), 5);

    // 验证排序顺序
    // 分数 100：非持久化在前，然后持久化
    assert_eq!(cache.links()[0].score(), 100);
    assert!(!cache.links()[0].is_persistent());
    assert_eq!(cache.links()[1].score(), 100);
    assert!(cache.links()[1].is_persistent());

    // 分数 75
    assert_eq!(cache.links()[2].score(), 75);

    // 分数 50
    assert_eq!(cache.links()[3].score(), 50);

    // 分数 30
    assert_eq!(cache.links()[4].score(), 30);
}

/// 测试链式调用返回类型正确性
#[test]
fn test_chained_return_types() {
    let backend = MockBackend::new("test", 50, false);

    // 验证每个方法都返回 Self
    let builder = OxCacheBuilder::new();
    let builder = builder.backend(backend);
    let builder = builder.default_ttl(Duration::from_secs(100));
    let builder = builder.enable_backfill();
    let builder = builder.disable_backfill();
    let builder = builder.max_capacity(500);

    let result = builder.build();
    assert!(result.is_ok());
}

/// 测试 build_async 与 build 的一致性
#[tokio::test]
async fn test_build_async_consistency() {
    let backend1 = MockBackend::new("test1", 50, false);
    let backend2 = MockBackend::new("test2", 100, false);

    // 同步构建
    let sync_result = OxCacheBuilder::new()
        .backend(backend1.clone())
        .backend(backend2.clone())
        .default_ttl(Duration::from_secs(300))
        .build();

    // 异步构建
    let async_result = OxCacheBuilder::new()
        .backend(backend1)
        .backend(backend2)
        .default_ttl(Duration::from_secs(300))
        .build_async()
        .await;

    assert!(sync_result.is_ok());
    assert!(async_result.is_ok());

    let sync_cache = sync_result.unwrap();
    let async_cache = async_result.unwrap();

    // 验证结果一致
    assert_eq!(sync_cache.len(), async_cache.len());
}
