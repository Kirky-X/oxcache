// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Cache 宏注册和 Lua 脚本方法

use super::Cache;
use crate::error::{CacheError, Result};
use crate::traits::CacheKey;
use std::sync::Arc;

impl Cache<String, Vec<u8>> {
    /// Register this cache for use with the `#[cached]` macro.
    ///
    /// Only `Cache<String, Vec<u8>>` can be registered for macro usage.
    ///
    /// # Arguments
    ///
    /// * `service_name` - Unique service name for the macro
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Registration successful
    /// * `Err(CacheError)` - Registration failed
    pub async fn register_for_macro(&self, service_name: &str) -> Result<()> {
        use crate::internal::__internal_register_cache;

        if service_name.is_empty() {
            return Err(CacheError::InvalidInput("service_name must not be empty".to_string()));
        }

        let backend = self.backend.clone();
        let mut cache: Cache<String, Vec<u8>> = Cache::new_with_backend(backend);
        // Propagate backend_sync so the #[cached(sync)] macro can use sync byte ops.
        if let Some(sync_backend) = &self.backend_sync {
            cache.set_sync_backend(sync_backend.clone());
        }
        __internal_register_cache(service_name, Arc::new(cache)).await;
        Ok(())
    }
}

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    #[cfg(feature = "lua-script")]
    pub async fn eval_lua(&self, _script: &str, _keys: &[&str], _args: &[&str]) -> Result<redis::Value> {
        let executor = self.backend.as_lua_executor().ok_or_else(|| {
            CacheError::Operation(
                "Lua scripts require a Redis backend. Current backend does not support Lua execution.".to_string(),
            )
        })?;
        executor.eval_lua(_script, _keys, _args).await
    }

    #[cfg(feature = "lua-script")]
    pub async fn eval_sha(&self, _sha: &str, _keys: &[&str], _args: &[&str]) -> Result<redis::Value> {
        let executor = self.backend.as_lua_executor().ok_or_else(|| {
            CacheError::Operation(
                "Lua scripts require a Redis backend. Current backend does not support Lua execution.".to_string(),
            )
        })?;
        executor.eval_sha(_sha, _keys, _args).await
    }

    #[cfg(feature = "lua-script")]
    pub async fn script_load(&self, _script: &str) -> Result<String> {
        let executor = self.backend.as_lua_executor().ok_or_else(|| {
            CacheError::Operation(
                "Lua scripts require a Redis backend. Current backend does not support Lua execution.".to_string(),
            )
        })?;
        executor.script_load(_script).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::api::Cache;

    // ========================================================================
    // register_for_macro tests
    // ========================================================================

    #[tokio::test]
    async fn test_register_for_macro_string_vec_u8() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let result = cache.register_for_macro("test_service").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_register_for_macro_multiple_services() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        assert!(cache.register_for_macro("svc_a").await.is_ok());
        assert!(cache.register_for_macro("svc_b").await.is_ok());
    }

    #[tokio::test]
    async fn test_register_for_macro_empty_service_name() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let result = cache.register_for_macro("").await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Lua script feature-gated tests
    // ========================================================================

    #[tokio::test]
    #[cfg(feature = "lua-script")]
    async fn test_eval_lua_returns_error_on_non_redis_backend() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let result = cache.eval_lua("return 1", &[], &[]).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            CacheError::Operation(msg) => {
                assert!(msg.contains("Lua scripts require a Redis backend"));
            }
            _ => panic!("Expected Operation error, got {:?}", err),
        }
    }

    #[tokio::test]
    #[cfg(feature = "lua-script")]
    async fn test_eval_sha_returns_error_on_non_redis_backend() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let result = cache.eval_sha("abc123", &[], &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[cfg(feature = "lua-script")]
    async fn test_script_load_returns_error_on_non_redis_backend() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        let result = cache.script_load("return 1").await;
        assert!(result.is_err());
    }
}
