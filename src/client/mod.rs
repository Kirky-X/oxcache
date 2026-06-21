//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Client module for cache operations
//!
//! NOTE: The legacy CacheOps and CacheExt traits have been removed.
//! Use the modern Cache<K, V> API with #[cached] macro instead.

pub mod db_loader;

use crate::error::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// CacheOps trait for backward compatibility
///
/// This trait provides basic cache operations with async methods.
#[async_trait]
pub trait CacheOps: Send + Sync + 'static {
    /// Get a value from cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(Some(bytes))` - Value found
    /// * `Ok(None)` - Key not found
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set a value in cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to set
    /// * `value` - The value bytes to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Value stored successfully
    async fn set(&self, key: &str, value: &[u8]) -> Result<()>;

    /// Delete a key from cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to delete
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Key deleted successfully
    async fn delete(&self, key: &str) -> Result<()>;

    /// Clear all entries
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Cache cleared successfully
    async fn clear(&self) -> Result<()>;

    /// Check if key exists
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to check
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Key exists
    /// * `Ok(false)` - Key does not exist
    async fn exists(&self, key: &str) -> Result<bool>;
}

/// Extension trait for getting CacheOps from cache types
pub trait CacheOpsExt {
    fn to_cache_ops(&self) -> Arc<dyn CacheOps + Send + Sync>;
}

impl<T: CacheOps + Clone + 'static> CacheOpsExt for T {
    fn to_cache_ops(&self) -> Arc<dyn CacheOps + Send + Sync> {
        Arc::new(self.clone())
    }
}
