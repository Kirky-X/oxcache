//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Tiered cache backend with L1 (memory) and L2 (Redis) layers

use super::new_backend::CacheBackend;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Tiered cache backend with L1 (memory) and L2 (Redis)
///
/// This backend implements a two-tier caching strategy:
/// - L1: Fast in-memory cache (Moka)
/// - L2: Distributed Redis cache
///
/// Read operations check L1 first, then L2 on miss.
/// Write operations update both L1 and L2.
/// L1 is automatically populated from L2 on cache misses.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::{TieredBackend, MemoryBackend};
/// use std::sync::Arc;
///
/// // Create with default settings
/// let l1 = MemoryBackend::new();
/// let l2 = MemoryBackend::new(); // or RedisBackend::new("redis://...").await?
/// let backend = TieredBackend::new(l1, l2);
///
/// // Create with custom settings
/// let backend = TieredBackend::builder()
///     .l1(MemoryBackend::builder().capacity(10000).build())
///     .auto_promote(true)
///     .build();
/// ```
#[derive(Clone)]
pub struct TieredBackend {
    l1: Arc<dyn CacheBackend>,
    l2: Arc<dyn CacheBackend>,
    auto_promote: bool,
    degraded: Arc<tokio::sync::RwLock<bool>>,
}

impl TieredBackend {
    /// Create a new tiered backend
    ///
    /// # Arguments
    ///
    /// * `l1` - L1 (memory) backend
    /// * `l2` - L2 (backend)
    ///
    /// # Returns
    ///
    /// Configured TieredBackend instance
    pub fn new(l1: impl CacheBackend + 'static, l2: impl CacheBackend + 'static) -> Self {
        Self {
            l1: Arc::new(l1),
            l2: Arc::new(l2),
            auto_promote: true,
            degraded: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// Create a tiered backend from Arc<dyn CacheBackend>
    ///
    /// This is useful when the backends are created dynamically.
    pub fn from_arc(l1: Arc<dyn CacheBackend>, l2: Arc<dyn CacheBackend>) -> Self {
        Self {
            l1,
            l2,
            auto_promote: true,
            degraded: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// Create a new builder for configuring the tiered backend
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::backend::{TieredBackend, MemoryBackend, RedisBackend};
    ///
    /// let backend = TieredBackend::builder()
    ///     .l1(MemoryBackend::builder().capacity(10000).build())
    ///     .l2(RedisBackend::new("redis://localhost:6379").await?)
    ///     .auto_promote(true)
    ///     .build();
    /// ```
    pub fn builder() -> TieredBackendBuilder {
        TieredBackendBuilder::default()
    }

    /// Check if the backend is in degraded mode
    ///
    /// Returns true if L2 is unavailable and the backend is operating
    /// in L1-only mode.
    pub async fn is_degraded(&self) -> bool {
        *self.degraded.read().await
    }

    /// Set degraded mode
    async fn set_degraded(&self, degraded: bool) {
        *self.degraded.write().await = degraded;
    }
}

/// Builder for TieredBackend
#[derive(Default)]
pub struct TieredBackendBuilder {
    l1: Option<Arc<dyn CacheBackend>>,
    l2: Option<Arc<dyn CacheBackend>>,
    auto_promote: bool,
}

impl TieredBackendBuilder {
    /// Set the L1 (memory) backend
    ///
    /// # Arguments
    ///
    /// * `l1` - L1 backend instance
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn l1(mut self, l1: impl CacheBackend + 'static) -> Self {
        self.l1 = Some(Arc::new(l1));
        self
    }

    /// Set the L2 (Redis) backend
    ///
    /// # Arguments
    ///
    /// * `l2` - L2 backend instance
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn l2(mut self, l2: impl CacheBackend + 'static) -> Self {
        self.l2 = Some(Arc::new(l2));
        self
    }

    /// Enable or disable auto-promote
    ///
    /// When enabled, values from L2 are automatically promoted to L1 on cache misses.
    ///
    /// # Arguments
    ///
    /// * `auto_promote` - Whether to enable auto-promote (default: true)
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn auto_promote(mut self, auto_promote: bool) -> Self {
        self.auto_promote = auto_promote;
        self
    }

    /// Build the tiered backend
    ///
    /// # Returns
    ///
    /// Configured TieredBackend instance
    ///
    /// # Errors
    ///
    /// Returns `CacheError` if L1 or L2 backend is not set
    pub fn build(self) -> Result<TieredBackend> {
        let l1 = self
            .l1
            .ok_or_else(|| CacheError::ConfigError("L1 backend is required".to_string()))?;
        let l2 = self
            .l2
            .ok_or_else(|| CacheError::ConfigError("L2 backend is required".to_string()))?;

        Ok(TieredBackend {
            l1,
            l2,
            auto_promote: self.auto_promote,
            degraded: Arc::new(tokio::sync::RwLock::new(false)),
        })
    }
}

#[async_trait]
impl CacheBackend for TieredBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // Try L1 first
        if let Some(value) = self.l1.get(key).await? {
            return Ok(Some(value));
        }

        // Try L2 on L1 miss
        if let Some(value) = self.l2.get(key).await? {
            // Auto-promote to L1 if enabled
            if self.auto_promote {
                // Clone value for L1 promotion
                let value_clone = value.clone();
                if let Err(e) = self.l1.set(key, value_clone, None).await {
                    tracing::warn!("Failed to promote value to L1: {}", e);
                }
            }
            return Ok(Some(value));
        }

        Ok(None)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        // Set in both L1 and L2
        let value_clone = value.clone();

        // Set in L1 (always succeeds if L1 is healthy)
        if let Err(e) = self.l1.set(key, value, ttl).await {
            tracing::warn!("Failed to set value in L1: {}", e);
        }

        // Set in L2 (may fail if degraded)
        match self.l2.set(key, value_clone, ttl).await {
            Ok(_) => {
                // L2 is healthy, ensure we're not in degraded mode
                self.set_degraded(false).await;
                Ok(())
            }
            Err(e) => {
                // L2 failed, enter degraded mode
                tracing::warn!("L2 backend failed, entering degraded mode: {}", e);
                self.set_degraded(true).await;
                // Return degraded error
                Err(CacheError::Degraded(
                    "L2 backend unavailable, operating in L1-only mode".to_string(),
                ))
            }
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        // Delete from both L1 and L2
        let l1_result = self.l1.delete(key).await;
        let l2_result = self.l2.delete(key).await;

        // If L2 fails, enter degraded mode
        if l2_result.is_err() {
            self.set_degraded(true).await;
        }

        // Return error if both fail
        l1_result.or(l2_result)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        // Check L1 first
        if self.l1.exists(key).await? {
            return Ok(true);
        }

        // Check L2
        self.l2.exists(key).await
    }

    async fn clear(&self) -> Result<()> {
        // Clear both L1 and L2
        let l1_result = self.l1.clear().await;
        let l2_result = self.l2.clear().await;

        // If L2 fails, enter degraded mode
        if l2_result.is_err() {
            self.set_degraded(true).await;
        }

        l1_result.or(l2_result)
    }

    async fn close(&self) -> Result<()> {
        // Close both L1 and L2
        let l1_result = self.l1.close().await;
        let l2_result = self.l2.close().await;

        l1_result.or(l2_result)
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>> {
        // Check L1 first
        if let Ok(Some(ttl)) = self.l1.ttl(key).await {
            return Ok(Some(ttl));
        }

        // Check L2
        self.l2.ttl(key).await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        // Set TTL in both L1 and L2
        let l1_result = self.l1.expire(key, ttl).await;
        let l2_result = self.l2.expire(key, ttl).await;

        // If L2 fails, enter degraded mode
        if l2_result.is_err() {
            self.set_degraded(true).await;
        }

        // Return true if either succeeds
        Ok(l1_result.unwrap_or(false) || l2_result.unwrap_or(false))
    }

    async fn health_check(&self) -> Result<bool> {
        // Check if both L1 and L2 are healthy
        let l1_healthy = self.l1.health_check().await.unwrap_or(false);
        let l2_healthy = self.l2.health_check().await.unwrap_or(false);

        // Update degraded state based on L2 health
        if !l2_healthy && l1_healthy {
            self.set_degraded(true).await;
        } else {
            self.set_degraded(false).await;
        }

        // Consider healthy if at least L1 is working
        Ok(l1_healthy)
    }

    async fn stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("type".to_string(), "tiered".to_string());
        stats.insert("degraded".to_string(), self.is_degraded().await.to_string());
        stats.insert("auto_promote".to_string(), self.auto_promote.to_string());

        // Get L1 stats
        if let Ok(l1_stats) = self.l1.stats().await {
            for (key, value) in l1_stats {
                stats.insert(format!("l1_{}", key), value);
            }
        }

        // Get L2 stats
        if let Ok(l2_stats) = self.l2.stats().await {
            for (key, value) in l2_stats {
                stats.insert(format!("l2_{}", key), value);
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::memory::MemoryBackend;

    #[tokio::test]
    async fn test_tiered_backend_basic() {
        let l1 = MemoryBackend::new();
        let l2 = MemoryBackend::new(); // Use MemoryBackend as mock L2 for testing
        let backend = TieredBackend::new(l1, l2);

        // Test set and get
        backend.set("key1", b"value1".to_vec(), None).await.unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test exists
        assert!(backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());

        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_tiered_backend_l1_miss_l2_hit() {
        let l1 = MemoryBackend::new();
        let l2 = MemoryBackend::new();
        let backend = TieredBackend::new(l1.clone(), l2.clone());

        // Set value only in L2
        l2.set("key1", b"value1".to_vec(), None).await.unwrap();

        // Get should return value from L2 and promote to L1
        let value: Option<Vec<u8>> = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Now value should be in L1
        let exists: bool = l1.exists("key1").await.unwrap();
        assert!(exists);
    }

    #[tokio::test]
    async fn test_tiered_backend_stats() {
        let l1 = MemoryBackend::new();
        let l2 = MemoryBackend::new();
        let backend = TieredBackend::new(l1.clone(), l2.clone());

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.get("type"), Some(&"tiered".to_string()));
        assert_eq!(stats.get("degraded"), Some(&"false".to_string()));
        assert!(stats.contains_key("l1_type"));
        assert!(stats.contains_key("l2_type"));
    }
}
