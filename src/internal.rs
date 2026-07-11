// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
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

#[cfg(test)]
mod tests {
    use super::*;

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
