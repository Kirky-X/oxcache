//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Cache 宏注册和 Lua 脚本方法

use super::Cache;
use crate::core::traits::{CacheKey, Cacheable};
use crate::error::{CacheError, Result};
use std::any::TypeId;
use std::sync::Arc;

impl<K, V> Cache<K, V>
where
    K: CacheKey,
    V: Cacheable,
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
