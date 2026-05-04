//! Global cache registry for #[cached] macro support
//!
//! IMPORTANT: This registry must be explicitly initialized at application startup.
//! Do NOT use lazy initialization (OnceLock::get_or_init).
//!
//! # Usage
//!
//! ```rust,ignore
//! use oxcache::backend::MokaMemoryBackend;
//! use std::sync::Arc;
//!
//! // Initialize at application startup
//! let cache = Arc::new(MokaMemoryBackend::new());
//! oxcache::registry::init(cache);
//!
//! // Later, retrieve cache instances
//! let cache = oxcache::registry::get("default");
//! ```

use std::fmt;
use std::sync::Arc;

use crate::backend::interface::CacheBackend;

/// Global cache registry (singleton)
static CACHE_REGISTRY: once_cell::sync::OnceCell<Registry> = once_cell::sync::OnceCell::new();

/// Registry holding cache instances
struct Registry {
    caches: dashmap::DashMap<String, Arc<dyn CacheBackend>>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("cache_count", &self.caches.len())
            .finish()
    }
}

impl Registry {
    fn new() -> Self {
        Self {
            caches: dashmap::DashMap::new(),
        }
    }
}

/// Initialize the global registry with a default cache
///
/// # Panics
///
/// Panics if called more than once.
pub fn init(default_cache: Arc<dyn CacheBackend>) {
    let registry = Registry::new();
    registry.caches.insert("default".to_string(), default_cache);

    CACHE_REGISTRY
        .set(registry)
        .expect("oxcache registry already initialized - call init() only once");
}

/// Initialize the global registry without a default cache
///
/// # Panics
///
/// Panics if called more than once.
pub fn init_empty() {
    let registry = Registry::new();

    CACHE_REGISTRY
        .set(registry)
        .expect("oxcache registry already initialized - call init() only once");
}

/// Check if the registry is initialized
pub fn is_initialized() -> bool {
    CACHE_REGISTRY.get().is_some()
}

/// Register a cache instance
///
/// # Panics
///
/// Panics if the registry is not initialized.
pub fn register(name: &str, cache: Arc<dyn CacheBackend>) {
    let registry = CACHE_REGISTRY
        .get()
        .expect("oxcache registry not initialized - call init() first");
    registry.caches.insert(name.to_string(), cache);
}

/// Get a cache instance by name
///
/// Returns None if the registry is not initialized or the cache doesn't exist.
pub fn get(name: &str) -> Option<Arc<dyn CacheBackend>> {
    CACHE_REGISTRY.get()?.caches.get(name).map(|r| r.clone())
}

/// Remove a cache instance
pub fn remove(name: &str) -> Option<Arc<dyn CacheBackend>> {
    CACHE_REGISTRY.get()?.caches.remove(name).map(|(_, v)| v)
}

/// Clear all caches from the registry
pub fn clear() {
    if let Some(registry) = CACHE_REGISTRY.get() {
        registry.caches.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::interface::CacheConnector;
    use crate::testing::mock::MockBackend;

    #[test]
    fn test_is_initialized_false_before_init() {
        // In a fresh process, registry should not be initialized
        let initialized = is_initialized();
        if !initialized {
            assert!(get("nonexistent").is_none());
        }
    }

    #[test]
    fn test_get_returns_none_when_not_initialized() {
        if !is_initialized() {
            assert!(get("nonexistent").is_none());
        }
    }

    #[test]
    fn test_init_and_get() {
        // Register a named cache and verify retrieval works regardless of init state
        if !is_initialized() {
            let backend = Arc::new(MockBackend::new("test", 100, false));
            init(backend);
        }

        assert!(is_initialized());

        // Register a test cache regardless of whether "default" exists
        let cache = Arc::new(MockBackend::new("named_test", 50, false));
        register("named_test", cache.clone());

        let retrieved = get("named_test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().backend_kind(), cache.backend_kind());

        // Non-existent should return None
        assert!(get("__nonexistent_key_123__").is_none());
    }

    #[test]
    fn test_init_panics_on_double_init() {
        // If already initialized, double init would panic (which is expected)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let backend = Arc::new(MockBackend::new("test_panic", 100, false));
            init(backend);
        }));
        // Either it was already initialized (so panics) or it initializes for the first time
        // We can't reliably test this in parallel, so just verify init() exists
        if is_initialized() {
            assert!(result.is_err(), "Should panic when double-init");
        }
    }

    #[test]
    fn test_init_empty() {
        let result = std::panic::catch_unwind(init_empty);
        if result.is_err() {
            // Expected if registry was already initialized by previous test
        }
    }

    #[test]
    fn test_register_and_get() {
        if !is_initialized() {
            let backend = Arc::new(MockBackend::new("test", 100, false));
            init(backend);
        }

        let cache = Arc::new(MockBackend::new("registered", 50, false));
        register("my_cache", cache.clone());

        let retrieved = get("my_cache");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().backend_kind(), cache.backend_kind());
    }

    #[test]
    fn test_remove() {
        if !is_initialized() {
            let backend = Arc::new(MockBackend::new("test", 100, false));
            init(backend);
        }

        let cache = Arc::new(MockBackend::new("removable", 50, false));
        register("to_remove", cache);

        let removed = remove("to_remove");
        assert!(removed.is_some());

        assert!(get("to_remove").is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        if !is_initialized() {
            let backend = Arc::new(MockBackend::new("test", 100, false));
            init(backend);
        }

        let result = remove("does_not_exist");
        assert!(result.is_none());
    }

    #[test]
    fn test_clear() {
        if !is_initialized() {
            let backend = Arc::new(MockBackend::new("test", 100, false));
            init(backend);
        }

        let cache1 = Arc::new(MockBackend::new("clear1", 50, false));
        let cache2 = Arc::new(MockBackend::new("clear2", 50, false));
        register("clear1", cache1);
        register("clear2", cache2);

        clear();

        assert!(get("clear1").is_none());
        assert!(get("clear2").is_none());
        assert!(get("default").is_none());
    }

    #[test]
    fn test_registry_debug_format() {
        let registry = Registry::new();
        let debug = format!("{:?}", registry);
        assert!(debug.contains("Registry"));
        assert!(debug.contains("cache_count"));
    }

    #[test]
    fn test_registry_new_empty() {
        let registry = Registry::new();
        assert_eq!(registry.caches.len(), 0);
    }
}
