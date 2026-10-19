// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Lua script execution for RedisBackend. NOSCRIPT on `eval_sha` returns an
//! explicit error (no auto-fallback): re-load the script via `script_load()`
//! or use `eval_lua()`.

use super::client::RedisBackend;
use super::error::map_redis_error;
use crate::backend::interface::LuaExecutor;
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use crate::security;
use async_trait::async_trait;
use std::future::Future;
use std::time::Duration;

/// Check if a Redis error is a NOSCRIPT error (script not cached).
fn is_noscript_error(e: &redis::RedisError) -> bool {
    e.to_string().contains("NOSCRIPT")
}

/// Lua 命令超时包装：`Duration::ZERO` 直通不限时，超时映射 `OxCacheError::Timeout`。
///
/// 超时经调用方的 `execute_once_with_breaker` 计入熔断器失败——Redis 挂起
/// 即降级信号，应推动跳闸。
async fn lua_timed<T>(
    timeout: Duration,
    fut: impl Future<Output = OxCacheResult<T>>,
) -> OxCacheResult<T> {
    if timeout.is_zero() {
        return fut.await;
    }
    match tokio::time::timeout(timeout, fut).await {
        Ok(result) => result,
        Err(_) => Err(OxCacheError::Timeout(format!(
            "Lua command timed out after {timeout:?}"
        ))),
    }
}

#[cfg(feature = "lua")]
#[async_trait]
impl LuaExecutor for RedisBackend {
    async fn eval_lua(
        &self,
        script: &str,
        keys: &[&str],
        args: &[&str],
    ) -> OxCacheResult<redis::Value> {
        security::validate_lua_script(script, keys.len())?;

        for key in keys {
            security::validate_redis_key(key)?;
        }

        // EVAL 非幂等：熔断器「Open 拒绝 + 记账」但绝不重试
        self.execute_once_with_breaker(|| async {
            let mut conn = self.conn();

            let mut cmd = redis::cmd(RedisCommand::Eval.as_str());
            cmd.arg(script).arg(keys.len());

            for key in keys {
                cmd.arg(key);
            }
            for arg in args {
                cmd.arg(arg);
            }

            lua_timed(self.lua_timeout(), async {
                cmd.query_async(&mut conn).await.map_err(map_redis_error)
            })
            .await
        })
        .await
    }

    /// Execute a Lua script by its SHA1 hash.
    ///
    /// Validates the SHA1 format and the key names, then runs `EVALSHA`
    /// directly. If Redis replies NOSCRIPT (script not cached), this method
    /// does **not** fall back to `eval_lua` (the original source is not
    /// available here): it returns an explicit error telling the caller to
    /// re-load the script via `script_load` or use `eval_lua` with the
    /// source.
    async fn eval_sha(
        &self,
        sha: &str,
        keys: &[&str],
        args: &[&str],
    ) -> OxCacheResult<redis::Value> {
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

        self.execute_once_with_breaker(|| async {
            let mut conn = self.conn();

            let mut cmd = redis::cmd(RedisCommand::EvalSha.as_str());
            cmd.arg(sha).arg(keys.len());

            for key in keys {
                cmd.arg(key);
            }
            for arg in args {
                cmd.arg(arg);
            }

            lua_timed(
                self.lua_timeout(),
                async {
                    match cmd.query_async(&mut conn).await {
                        Ok(result) => Ok(result),
                        Err(e) if is_noscript_error(&e) => {
                            // NOSCRIPT: script not cached, but we can't fall back to eval_lua
                            // without the original script source. Return the error.
                            // In practice, callers should use `eval_lua` directly if they
                            // have the source, or re-load the script via `script_load`.
                            Err(OxCacheError::Operation(format!(
                                "NOSCRIPT: script {sha} not cached. Re-load via script_load() or use eval_lua()"
                            )))
                        }
                        Err(e) => Err(map_redis_error(e)),
                    }
                },
            )
            .await
        })
        .await
    }

    async fn script_load(&self, script: &str) -> OxCacheResult<String> {
        security::validate_lua_script(script, 0)?;

        self.execute_once_with_breaker(|| async {
            let mut conn = self.conn();

            lua_timed(self.lua_timeout(), async {
                let sha: String = redis::cmd(RedisCommand::Script.as_str())
                    .arg("LOAD")
                    .arg(script)
                    .query_async(&mut conn)
                    .await
                    .map_err(map_redis_error)?;
                Ok(sha)
            })
            .await
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// pending future 在超时后返回 Timeout（文本含时长）
    #[tokio::test]
    async fn test_lua_timed_pending_times_out() {
        let result: OxCacheResult<redis::Value> =
            lua_timed(Duration::from_millis(20), std::future::pending()).await;
        match result {
            Err(OxCacheError::Timeout(msg)) => {
                assert!(msg.contains("20ms"), "unexpected message: {msg}")
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    /// 即时 future 原值透传
    #[tokio::test]
    async fn test_lua_timed_immediate_passthrough() {
        let result: OxCacheResult<u8> = lua_timed(Duration::from_secs(5), async { Ok(7) }).await;
        assert_eq!(result.unwrap(), 7);
    }

    /// ZERO = 不限时：慢 future 照常完成
    #[tokio::test]
    async fn test_lua_timed_zero_is_unbounded() {
        let result: OxCacheResult<u8> = lua_timed(Duration::ZERO, async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok(9)
        })
        .await;
        assert_eq!(result.unwrap(), 9);
    }
}
