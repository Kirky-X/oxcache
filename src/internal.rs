//! Internal module for #[cached] macro support
//! This module is intentionally documented

use crate::Cache;
use std::sync::Arc;

/// Internal cache registration function
pub async fn __internal_register_cache(_name: &str, _cache: Arc<Cache<String, Vec<u8>>>) {
    // Placeholder implementation
}

/// Internal cache retrieval function
pub fn __internal_get_cache(_name: &str) -> Option<Arc<Cache<String, Vec<u8>>>> {
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
