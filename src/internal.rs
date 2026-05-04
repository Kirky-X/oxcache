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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // __internal_register_cache and __internal_get_cache tests
    // ========================================================================

    #[tokio::test]
    async fn test_register_and_get_cache() {
        let cache = Arc::new(Cache::builder().build().await.unwrap());

        __internal_register_cache("test_cache", cache.clone()).await;

        let retrieved = __internal_get_cache("test_cache");
        assert!(retrieved.is_some());

        assert!(Arc::ptr_eq(&retrieved.unwrap(), &cache));
    }

    #[tokio::test]
    async fn test_get_cache_nonexistent() {
        let result = __internal_get_cache("does_not_exist");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_cache_before_registry_init() {
        assert!(__internal_get_cache("never_registered").is_none());
    }

    // ========================================================================
    // Feature info tests
    // ========================================================================

    #[test]
    fn test_get_all_feature_info_contains_moka() {
        let features = get_all_feature_info();
        #[cfg(feature = "moka")]
        assert!(features.contains(&"moka"));
    }

    #[test]
    fn test_get_all_feature_info_contains_redis() {
        let features = get_all_feature_info();
        #[cfg(feature = "redis")]
        assert!(features.contains(&"redis"));
    }

    #[test]
    fn test_get_l1_feature_info() {
        let features = get_l1_feature_info();
        #[cfg(feature = "moka")]
        {
            assert!(features.contains(&"moka"));
        }
        #[cfg(not(feature = "moka"))]
        {
            assert!(features.is_empty());
        }
    }

    #[test]
    fn test_get_l2_feature_info() {
        let features = get_l2_feature_info();
        #[cfg(feature = "redis")]
        {
            assert!(features.contains(&"redis"));
        }
        #[cfg(not(feature = "redis"))]
        {
            assert!(features.is_empty());
        }
    }

    #[test]
    fn test_is_l1_enabled() {
        #[cfg(feature = "moka")]
        assert!(is_l1_enabled());
        #[cfg(not(feature = "moka"))]
        assert!(!is_l1_enabled());
    }

    #[test]
    fn test_is_l2_enabled() {
        #[cfg(feature = "redis")]
        assert!(is_l2_enabled());
        #[cfg(not(feature = "redis"))]
        assert!(!is_l2_enabled());
    }

    #[tokio::test]
    async fn test_register_cache_overwrites_existing() {
        let cache1 = Arc::new(Cache::builder().build().await.unwrap());
        __internal_register_cache("overwrite_test", cache1).await;

        let cache2 = Arc::new(Cache::builder().build().await.unwrap());
        __internal_register_cache("overwrite_test", cache2.clone()).await;

        let retrieved = __internal_get_cache("overwrite_test");
        assert!(retrieved.is_some());
        assert!(Arc::ptr_eq(&retrieved.unwrap(), &cache2));
    }
}
