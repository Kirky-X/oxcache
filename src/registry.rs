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
    use crate::backend::memory::MokaMemoryBackend;

    /// Helper: ensure the registry is initialized (idempotent).
    /// Since OnceCell::set panics on second call, we guard with is_initialized()
    /// and use catch_unwind to handle the TOCTOU race between check and call.
    fn ensure_initialized() {
        if !is_initialized() {
            // Another test may initialize between our check and this call;
            // catch the panic to handle that race gracefully.
            let _ = std::panic::catch_unwind(|| init_empty());
        }
    }

    #[test]
    fn test_is_initialized_returns_bool() {
        // is_initialized should return a bool without panicking
        let _ = is_initialized();
    }

    #[test]
    fn test_get_returns_none_for_nonexistent() {
        // If not initialized, get returns None.
        // If initialized, get for a nonexistent key also returns None.
        assert!(get("definitely_does_not_exist_xyz_123").is_none());
    }

    #[test]
    fn test_remove_returns_none_for_nonexistent() {
        // remove returns None if not initialized or key doesn't exist
        assert!(remove("definitely_does_not_exist_xyz_456").is_none());
    }

    #[test]
    fn test_clear_does_not_panic() {
        // clear is a no-op if not initialized; should not panic either way
        clear();
    }

    #[test]
    fn test_register_get_remove_flow() {
        ensure_initialized();

        let backend: Arc<dyn CacheBackend> = Arc::new(MokaMemoryBackend::new());
        register("test_flow_backend", backend);

        // get should return Some
        let retrieved = get("test_flow_backend");
        assert!(retrieved.is_some(), "get should return Some after register");

        // remove should return Some
        let removed = remove("test_flow_backend");
        assert!(removed.is_some(), "remove should return Some");

        // get after remove should return None
        assert!(
            get("test_flow_backend").is_none(),
            "get should return None after remove"
        );
    }

    #[test]
    fn test_register_overwrites_existing() {
        ensure_initialized();

        let backend1: Arc<dyn CacheBackend> = Arc::new(MokaMemoryBackend::new());
        register("test_overwrite_backend", backend1);

        let backend2: Arc<dyn CacheBackend> = Arc::new(MokaMemoryBackend::new());
        register("test_overwrite_backend", backend2);

        // Should retrieve the latest registered backend without error
        assert!(get("test_overwrite_backend").is_some());

        // Cleanup
        remove("test_overwrite_backend");
    }

    #[test]
    fn test_clear_removes_all_caches() {
        ensure_initialized();

        let backend: Arc<dyn CacheBackend> = Arc::new(MokaMemoryBackend::new());
        register("test_clear_backend_1", backend.clone());
        register("test_clear_backend_2", backend.clone());

        assert!(get("test_clear_backend_1").is_some());
        assert!(get("test_clear_backend_2").is_some());

        clear();

        assert!(get("test_clear_backend_1").is_none());
        assert!(get("test_clear_backend_2").is_none());
    }

    #[test]
    fn test_is_initialized_true_after_init() {
        // After ensure_initialized(), is_initialized() should return true.
        // This tests the init_empty -> is_initialized path.
        ensure_initialized();
        assert!(is_initialized(), "is_initialized should return true after init");
    }
}
