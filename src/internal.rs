//
// MIT License
//
// 内部模块 - 为 #[cached] 宏提供支持

use crate::client::CacheOps;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::OnceLock;

// ============================================================================
// Internal Global Cache Registry (for #[cached] macro)
// ============================================================================

static CACHE_REGISTRY: OnceLock<DashMap<String, Arc<dyn CacheOps + Send + Sync>>> = OnceLock::new();

fn get_registry() -> &'static DashMap<String, Arc<dyn CacheOps + Send + Sync>> {
    CACHE_REGISTRY.get_or_init(DashMap::new)
}

/// Register a cache instance for use with the #[cached] macro.
/// This is an internal function used by Cache::register_for_macro().
#[doc(hidden)]
pub fn __internal_register_cache(service_name: &str, cache: Arc<dyn CacheOps + Send + Sync>) {
    get_registry().insert(service_name.to_string(), cache);
}

/// Get a cache instance registered for the #[cached] macro.
/// This is an internal function called by the generated macro code.
#[doc(hidden)]
pub fn __internal_get_cache(service_name: &str) -> Option<Arc<dyn CacheOps + Send + Sync>> {
    get_registry().get(service_name).map(|r| r.value().clone())
}

/// Remove a cache registration (internal function for cleanup).
#[doc(hidden)]
pub fn __internal_remove_cache(service_name: &str) {
    let _ = get_registry().remove(service_name);
}

/// Clear all registered caches (internal function for cleanup).
#[doc(hidden)]
pub async fn __internal_clear_all() {
    let registry = get_registry();
    for entry in registry.iter() {
        let cache = entry.value();
        let _ = cache.shutdown().await;
    }
    registry.clear();
}

// ============================================================================
// Feature Information Functions (Public)
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
