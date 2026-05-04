use crate::Cache;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type MacroCacheMap = Mutex<HashMap<String, Arc<Cache<String, Vec<u8>>>>>;

static MACRO_CACHES: once_cell::sync::OnceCell<MacroCacheMap> = once_cell::sync::OnceCell::new();

fn caches() -> &'static MacroCacheMap {
    MACRO_CACHES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn __internal_register_cache(name: &str, cache: Arc<Cache<String, Vec<u8>>>) {
    if let Ok(mut map) = caches().lock() {
        map.insert(name.to_string(), cache);
    }
}

pub fn __internal_get_cache(name: &str) -> Option<Arc<Cache<String, Vec<u8>>>> {
    caches().lock().ok()?.get(name).cloned()
}

/// Get all feature information
pub fn get_all_feature_info() -> Vec<&'static str> {
    let mut features = Vec::new();
    if cfg!(feature = "moka") {
        features.push("moka");
    }
    if cfg!(feature = "redis") {
        features.push("redis");
    }
    features
}

/// Get L1 feature information
pub fn get_l1_feature_info() -> Vec<&'static str> {
    let mut features = Vec::new();
    if cfg!(feature = "moka") {
        features.push("moka");
    }
    features
}

/// Get L2 feature information
pub fn get_l2_feature_info() -> Vec<&'static str> {
    let mut features = Vec::new();
    if cfg!(feature = "redis") {
        features.push("redis");
    }
    features
}

/// Check if L1 is enabled
pub fn is_l1_enabled() -> bool {
    cfg!(feature = "moka")
}

/// Check if L2 is enabled
pub fn is_l2_enabled() -> bool {
    cfg!(feature = "redis")
}
