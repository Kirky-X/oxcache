//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块提供了缓存功能信息查询功能。



// ============================================================================
// Feature Information Functions
// ============================================================================

/// 获取 L1 缓存功能状态信息
pub fn get_l1_feature_info() -> &'static str {
    #[cfg(feature = "l1-moka")]
    {
        "L1 Cache (Moka): Enabled"
    }
    #[cfg(not(feature = "l1-moka"))]
    {
        "L1 Cache (Moka): Disabled (enable with 'l1-moka' feature)"
    }
}

/// 获取 L2 缓存功能状态信息
pub fn get_l2_feature_info() -> &'static str {
    #[cfg(feature = "l2-redis")]
    {
        "L2 Cache (Redis): Enabled"
    }
    #[cfg(not(feature = "l2-redis"))]
    {
        "L2 Cache (Redis): Disabled (enable with 'l2-redis' feature)"
    }
}

/// 获取所有功能状态信息
pub fn get_all_feature_info() -> Vec<&'static str> {
    vec![get_l1_feature_info(), get_l2_feature_info()]
}

/// 检查 L1 功能是否启用
pub fn is_l1_enabled() -> bool {
    cfg!(feature = "l1-moka")
}

/// 检查 L2 功能是否启用
pub fn is_l2_enabled() -> bool {
    cfg!(feature = "l2-redis")
}








