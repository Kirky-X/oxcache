// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 该模块提供了缓存功能信息查询功能。
// 同时提供宏使用的内部缓存注册表支持。

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::OnceLock;

// ============================================================================
// Internal Macro Cache Registry (for macro use only)
// ============================================================================

/// Internal cache registry for macro support.
/// This is NOT part of the public API - only the #[cached] macro uses it.
static MACRO_CACHE_REGISTRY: OnceLock<DashMap<String, Arc<dyn crate::client::CacheOps + Send + Sync>>> =
    OnceLock::new();

fn get_macro_registry() -> &'static DashMap<String, Arc<dyn crate::client::CacheOps + Send + Sync>> {
    MACRO_CACHE_REGISTRY.get_or_init(|| DashMap::new())
}

/// Register a cache instance for use with the #[cached] macro.
/// This is an internal function used by Cache::register_for_macro().
///
/// # Arguments
/// * `service_name` - The service name to register the cache under
/// * `cache` - The cache instance to register
#[doc(hidden)]
pub fn __internal_register_cache(
    service_name: &str,
    cache: Arc<dyn crate::client::CacheOps + Send + Sync>,
) {
    get_macro_registry().insert(service_name.to_string(), cache);
}

/// Get a cache instance registered for the #[cached] macro.
/// This is an internal function called by the generated macro code.
///
/// # Arguments
/// * `service_name` - The service name to look up
///
/// # Returns
/// Some(cache) if found, None if not found
#[doc(hidden)]
pub fn __internal_get_cache(
    service_name: &str,
) -> Option<Arc<dyn crate::client::CacheOps + Send + Sync>> {
    get_macro_registry().get(service_name).map(|r| r.value().clone())
}

/// Remove a cache registration.
/// This is an internal function for cleanup.
#[doc(hidden)]
pub fn __internal_remove_cache(service_name: &str) {
    get_macro_registry().remove(service_name);
}

/// Clear all registered caches.
/// This is an internal function for cleanup.
#[doc(hidden)]
pub fn __internal_clear_all() {
    get_macro_registry().clear();
}

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
