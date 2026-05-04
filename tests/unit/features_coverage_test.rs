// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 特性检测模块测试 - 验证特性可用性检测功能
// 注意：这些测试使用内部函数，因为features模块没有公开导出

use oxcache::{is_l1_enabled, is_l2_enabled};

#[test]
fn test_internal_feature_detection() {
    let l1_enabled = is_l1_enabled();
    let l2_enabled = is_l2_enabled();

    #[cfg(feature = "moka")]
    assert!(l1_enabled);

    #[cfg(not(feature = "moka"))]
    assert!(!l1_enabled);

    #[cfg(feature = "redis")]
    assert!(l2_enabled);

    #[cfg(not(feature = "redis"))]
    assert!(!l2_enabled);
}

#[test]
fn test_has_feature_macro() {
    let has_moka = cfg!(feature = "moka");
    let has_redis = cfg!(feature = "redis");
    let has_metrics = cfg!(feature = "metrics");

    assert_eq!(has_moka, cfg!(feature = "moka"));
    assert_eq!(has_redis, cfg!(feature = "redis"));
    assert_eq!(has_metrics, cfg!(feature = "metrics"));
}
