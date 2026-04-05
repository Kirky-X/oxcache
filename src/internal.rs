//! Internal module for #[cached] macro support
//!
//! This module provides internal functions that delegate to the registry module.
//! The actual implementation is in `src/registry.rs`.

use crate::Cache;
use std::sync::Arc;

/// Internal cache registration function
///
/// Note: This is a placeholder. The actual cache registration should use
/// `crate::registry::register()` directly with a CacheBackend implementation.
pub async fn __internal_register_cache(_name: &str, _cache: Arc<Cache<String, Vec<u8>>>) {
    // Placeholder implementation
    // In the future, this should integrate with the registry module
    // when Cache implements CacheBackend or provides a backend accessor
}

/// Internal cache retrieval function
///
/// Note: This is a placeholder. The actual cache retrieval should use
/// `crate::registry::get()` directly.
pub fn __internal_get_cache(_name: &str) -> Option<Arc<Cache<String, Vec<u8>>>> {
    // Placeholder implementation
    // Returns None as this requires further refactoring to integrate with registry
    None
}

/// Get all feature information
pub fn get_all_feature_info() -> Vec<&'static str> {
    vec!["features"]
}

/// Get L1 feature information
pub fn get_l1_feature_info() -> Vec<&'static str> {
    vec!["moka"]
}

/// Get L2 feature information
pub fn get_l2_feature_info() -> Vec<&'static str> {
    vec!["redis"]
}

/// Check if L1 is enabled
pub fn is_l1_enabled() -> bool {
    true
}

/// Check if L2 is enabled
pub fn is_l2_enabled() -> bool {
    true
}
