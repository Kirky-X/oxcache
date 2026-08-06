// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
// tests/backend_interface_test.rs
// 此测试验证 CacheBackend trait 签名变化后编译正常
// FIX-02: 移除 as_any() 和 is() 方法，改用 backend_kind() 进行类型识别

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
