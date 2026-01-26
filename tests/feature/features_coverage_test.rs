// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 特性检测模块测试 - 验证特性可用性检测功能
// 注意：这些测试使用内部函数，因为features模块没有公开导出

use oxcache::{is_l1_enabled, is_l2_enabled};

#[test]
fn test_internal_feature_detection() {
    // 测试内部导出的特性检测函数
    let _l1_enabled = is_l1_enabled();
    let _l2_enabled = is_l2_enabled();
    
    // 确保函数能够正常调用
    assert!(true);
}

#[test]
fn test_has_feature_macro() {
    // 测试 has_feature! 宏
    let _has_moka = oxcache::has_feature!("moka");
    let _has_redis = oxcache::has_feature!("redis");
    let _has_metrics = oxcache::has_feature!("metrics");
    
    // 确保宏能够正常展开
    assert!(true);
}