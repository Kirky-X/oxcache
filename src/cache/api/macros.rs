//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 宏注册和 Lua 脚本方法

use super::Cache;
use crate::core::traits::CacheKey;
use crate::error::{CacheError, Result};
use std::any::TypeId;
use std::sync::Arc;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub async fn register_for_macro(&self, service_name: &str)
    where
        K: 'static,
        V: 'static,
    {
        use crate::internal::__internal_register_cache;

        if TypeId::of::<K>() == TypeId::of::<String>() && TypeId::of::<V>() == TypeId::of::<Vec<u8>>() {
            let backend = self.backend.clone();
            let cache: Cache<String, Vec<u8>> = Cache::new_with_backend(backend);
            __internal_register_cache(service_name, Arc::new(cache)).await;
        }
    }

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
        cache.register_for_macro("test_service").await;
    }

    #[tokio::test]
    async fn test_register_for_macro_non_matching_types_not_registered() {
        let cache: Cache<String, String> = Cache::memory().await.unwrap();
        cache.register_for_macro("another_service").await;
    }

    #[tokio::test]
    async fn test_register_for_macro_multiple_services() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        cache.register_for_macro("svc_a").await;
        cache.register_for_macro("svc_b").await;
    }

    #[tokio::test]
    async fn test_register_for_macro_empty_service_name() {
        let cache: Cache<String, Vec<u8>> = Cache::memory().await.unwrap();
        cache.register_for_macro("").await;
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
