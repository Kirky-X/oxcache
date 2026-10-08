// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// 验证 CacheBackend trait 签名变更时编译正常：类型识别走 backend_kind()

use oxcache::backend::BackendKind;

/// 验证 BackendKind 枚举的基本功能
#[test]
fn test_backend_kind_is_memory() {
    assert!(BackendKind::Moka.is_memory());
    assert!(BackendKind::DashMap.is_memory());
    assert!(BackendKind::Mock.is_memory());
    assert!(!BackendKind::Redis.is_memory());
    assert!(!BackendKind::Chain.is_memory());
    assert!(!BackendKind::Unknown.is_memory());
}

/// 验证 BackendKind 枚举的分布式检测
#[test]
fn test_backend_kind_is_distributed() {
    assert!(BackendKind::Redis.is_distributed());
    assert!(!BackendKind::Moka.is_distributed());
    assert!(!BackendKind::DashMap.is_distributed());
    assert!(!BackendKind::Chain.is_distributed());
}

/// 验证 BackendKind 枚举的 PartialEq 实现
#[test]
fn test_backend_kind_equality() {
    assert_eq!(BackendKind::Moka, BackendKind::Moka);
    assert_ne!(BackendKind::Moka, BackendKind::DashMap);
}
