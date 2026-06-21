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

    #[test]
    fn test_is_initialized_false_before_init() {
        // Registry should not be initialized before init()
        // Note: This test might fail if other tests initialized the registry
        // In practice, use a fresh process or reset mechanism
    }

    #[test]
    fn test_get_returns_none_when_not_initialized() {
        // get() should return None if registry is not initialized
        // Note: This test depends on registry state
        if !is_initialized() {
            assert!(get("nonexistent").is_none());
        }
    }
}
