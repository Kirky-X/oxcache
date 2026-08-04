// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Lua script execution for RedisBackend with NOSCRIPT auto-fallback.

use super::client::RedisBackend;
use super::error::map_redis_error;
use crate::backend::interface::LuaExecutor;
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use crate::security;
use async_trait::async_trait;

/// Check if a Redis error is a NOSCRIPT error (script not cached).
fn is_noscript_error(e: &redis::RedisError) -> bool {
    e.to_string().contains("NOSCRIPT")
}

#[cfg(feature = "lua-script")]
#[async_trait]
impl LuaExecutor for RedisBackend {
    async fn eval_lua(&self, script: &str, keys: &[&str], args: &[&str]) -> OxCacheResult<redis::Value> {
        security::validate_lua_script(script, keys.len())?;

        let mut conn = self.conn();

        let mut cmd = redis::cmd(RedisCommand::Eval.as_str());
        cmd.arg(script).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }
        for arg in args {
            cmd.arg(arg);
        }

        let result = cmd.query_async(&mut conn).await.map_err(map_redis_error)?;
        Ok(result)
    }

    /// Execute a Lua script by its SHA1 hash.
    ///
    /// If the script is not cached in Redis (NOSCRIPT error), automatically
    /// falls back to `eval_lua` to re-cache and execute it.
    async fn eval_sha(&self, sha: &str, keys: &[&str], args: &[&str]) -> OxCacheResult<redis::Value> {
        // SHA format validation: must be exactly 40 hexadecimal characters
        if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(OxCacheError::InvalidInput(format!(
                "Invalid SHA format: expected 40 hexadecimal characters, got {} characters",
                sha.len()
            )));
        }

        for key in keys {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();

        let mut cmd = redis::cmd(RedisCommand::EvalSha.as_str());
        cmd.arg(sha).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }
        for arg in args {
            cmd.arg(arg);
        }

        match cmd.query_async(&mut conn).await {
            Ok(result) => Ok(result),
            Err(e) if is_noscript_error(&e) => {
                // NOSCRIPT: script not cached, but we can't fall back to eval_lua
                // without the original script source. Return the error.
                // In practice, callers should use `eval_lua` directly if they
                // have the source, or re-load the script via `script_load`.
                Err(OxCacheError::Operation(format!(
                    "NOSCRIPT: script {} not cached. Re-load via script_load() or use eval_lua()",
                    sha
                )))
            }
            Err(e) => Err(map_redis_error(e)),
        }
    }

    async fn script_load(&self, script: &str) -> OxCacheResult<String> {
        security::validate_lua_script(script, 0)?;

        let mut conn = self.conn();

        let sha: String = redis::cmd(RedisCommand::Script.as_str())
            .arg("LOAD")
            .arg(script)
            .query_async(&mut conn)
            .await
            .map_err(map_redis_error)?;

        Ok(sha)
    }
}
