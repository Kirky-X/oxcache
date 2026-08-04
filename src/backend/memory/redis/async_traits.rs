// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Async trait implementations for RedisBackend.

use super::builder::RedisMode;
use super::client::RedisBackend;
use crate::backend::interface::AtomicCacheWriter;
use crate::backend::memory::redis::error;
use crate::backend::{BackendKind, CacheConnector, CacheReader, CacheWriter};
use crate::backend::{BackendScore, Scores};
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use std::time::Duration;

/// Redis 最大 TTL 上界（秒）。Redis SETEX/EXPIRE 仅接受 i32::MAX 秒（~68 年）。
const REDIS_MAX_TTL_SECS: u64 = i32::MAX as u64;

/// 校验 TTL 值是否在 Redis 可接受范围内。
fn validate_redis_ttl(ttl: Duration) -> OxCacheResult<u64> {
    let secs = ttl.as_secs();
    if secs == 0 {
        return Err(OxCacheError::InvalidInput(
            "TTL must be at least 1 second for Redis SETEX/EXPIRE".to_string(),
        ));
    }
    if secs > REDIS_MAX_TTL_SECS {
        return Err(OxCacheError::InvalidInput(format!(
            "TTL {}s exceeds Redis maximum of {}s (~68 years)",
            secs, REDIS_MAX_TTL_SECS
        )));
    }
    Ok(secs)
}
use crate::security;
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
impl CacheReader for RedisBackend {
    async fn get(&self, key: &str) -> OxCacheResult<Option<Vec<u8>>> {
        security::validate_redis_key(key)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                redis::cmd(RedisCommand::Get.as_str())
                    .arg(key)
                    .query_async(&mut conn)
                    .await
                    .map_err(error::map_redis_error)
            }
        })
        .await
    }

    async fn exists(&self, key: &str) -> OxCacheResult<bool> {
        security::validate_redis_key(key)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                let n: i64 = redis::cmd(RedisCommand::Exists.as_str())
                    .arg(key)
                    .query_async(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                Ok(n > 0)
            }
        })
        .await
    }

    async fn ttl(&self, key: &str) -> OxCacheResult<Option<Duration>> {
        security::validate_redis_key(key)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                let n: i64 = redis::cmd(RedisCommand::Ttl.as_str())
                    .arg(key)
                    .query_async(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                if n <= 0 {
                    Ok(None)
                } else {
                    Ok(Some(Duration::from_secs(n as u64)))
                }
            }
        })
        .await
    }

    async fn len(&self) -> OxCacheResult<u64> {
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                let len: i64 = redis::cmd(RedisCommand::Dbsize.as_str())
                    .query_async(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                Ok(len as u64)
            }
        })
        .await
    }

    async fn is_empty(&self) -> OxCacheResult<bool> {
        Ok(self.len().await?.eq(&0))
    }

    async fn capacity(&self) -> OxCacheResult<u64> {
        Ok(0)
    }

    /// Collect backend statistics from Redis INFO command.
    ///
    /// Returns a map containing:
    /// - `"memory_info"`: raw output of `INFO memory`
    /// - `"connected_clients"`: current client count (parsed from `INFO clients`)
    /// - `"maxclients"`: maximum client limit (parsed from `INFO clients`)
    async fn stats(&self) -> OxCacheResult<HashMap<String, String>> {
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                let mut stats = HashMap::new();

                // INFO memory
                let memory_info: String = redis::cmd(RedisCommand::Info.as_str())
                    .arg("memory")
                    .query_async(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                stats.insert("memory_info".to_string(), memory_info);

                // INFO clients — parse connected_clients and maxclients
                let clients_info: String = redis::cmd(RedisCommand::Info.as_str())
                    .arg("clients")
                    .query_async(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                for line in clients_info.lines() {
                    let line = line.trim();
                    if let Some((key, value)) = line.split_once(':') {
                        let key = key.trim();
                        let value = value.trim();
                        if key == "connected_clients" || key == "maxclients" {
                            stats.insert(key.to_string(), value.to_string());
                        }
                    }
                }

                Ok(stats)
            }
        })
        .await
    }

    async fn get_many(&self, keys: &[String]) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }
        let keys_slice: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        self.get_many_pipeline(&keys_slice).await
    }

    async fn keys(&self, pattern: &str) -> OxCacheResult<Vec<String>> {
        security::validate_scan_pattern(pattern)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                let mut all_keys = Vec::new();
                let mut cursor = 0i64;
                loop {
                    let (new_cursor, batch): (i64, Vec<String>) = redis::cmd(RedisCommand::Scan.as_str())
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(pattern)
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut conn)
                        .await
                        .map_err(error::map_redis_error)?;
                    all_keys.extend(batch);
                    cursor = new_cursor;
                    if cursor == 0 {
                        break;
                    }
                }
                Ok(all_keys)
            }
        })
        .await
    }
}

#[async_trait]
impl CacheWriter for RedisBackend {
    async fn set(
        &self,
        key: std::sync::Arc<str>,
        value: std::sync::Arc<Vec<u8>>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        let key_ref = key.as_ref();
        security::validate_redis_key(key_ref)?;

        self.execute_with_retry(|| {
            let mut conn = self.conn();
            let key = key.clone();
            let value = value.clone();
            async move {
                if let Some(ttl) = ttl {
                    let ttl_secs = validate_redis_ttl(ttl)?;
                    redis::cmd(RedisCommand::SetEx.as_str())
                        .arg(key.as_ref())
                        .arg(ttl_secs)
                        .arg(value.as_ref())
                        .query_async::<()>(&mut conn)
                        .await
                        .map_err(error::map_redis_error)?;
                } else {
                    redis::cmd(RedisCommand::Set.as_str())
                        .arg(key.as_ref())
                        .arg(value.as_ref())
                        .query_async::<()>(&mut conn)
                        .await
                        .map_err(error::map_redis_error)?;
                }
                Ok(())
            }
        })
        .await
    }

    async fn delete(&self, key: &str) -> OxCacheResult<()> {
        security::validate_redis_key(key)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                redis::cmd(RedisCommand::Del.as_str())
                    .arg(key)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                Ok(())
            }
        })
        .await
    }

    /// Clear all keys from the Redis database.
    ///
    /// **Safety gate**: requires `dangerous_clear_enabled = true` in the builder.
    /// Uses SCAN + pipeline DEL for batch deletion (not per-key DEL).
    async fn clear(&self) -> OxCacheResult<()> {
        if !self.dangerous_clear_enabled() {
            return Err(OxCacheError::NotSupported(
                "Full-database clear() is disabled by default. \
                 Use RedisBackend::builder().dangerous_clear_enabled(true) to enable, \
                 or use clear_namespace(prefix) for safe prefix-based deletion."
                    .to_string(),
            ));
        }

        security::validate_scan_pattern("*")?;

        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                let mut cursor = 0i64;
                loop {
                    let (new_cursor, keys): (i64, Vec<String>) = redis::cmd(RedisCommand::Scan.as_str())
                        .arg(cursor)
                        .arg("MATCH")
                        .arg("*")
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut conn)
                        .await
                        .map_err(error::map_redis_error)?;

                    if !keys.is_empty() {
                        let mut pipe = redis::pipe();
                        for key in &keys {
                            pipe.cmd(RedisCommand::Del.as_str()).arg(key);
                        }
                        pipe.query_async::<()>(&mut conn)
                            .await
                            .map_err(error::map_redis_error)?;
                    }

                    cursor = new_cursor;
                    if cursor == 0 {
                        break;
                    }
                }
                Ok(())
            }
        })
        .await
    }

    async fn expire(&self, key: &str, ttl: Duration) -> OxCacheResult<bool> {
        security::validate_redis_key(key)?;
        let ttl_secs = validate_redis_ttl(ttl)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                let result: i64 = redis::cmd(RedisCommand::Expire.as_str())
                    .arg(key)
                    .arg(ttl_secs)
                    .query_async(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                Ok(result > 0)
            }
        })
        .await
    }

    async fn set_many(&self, items: &[crate::backend::CacheSetItem]) -> OxCacheResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        for (key, _, _) in items {
            security::validate_redis_key(key)?;
        }
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            let items = items.to_vec();
            async move {
                let mut pipe = redis::pipe();
                for (key, value, ttl) in &items {
                    if let Some(ttl) = ttl {
                        let ttl_secs = validate_redis_ttl(*ttl)?;
                        pipe.cmd(RedisCommand::SetEx.as_str())
                            .arg(key.as_ref())
                            .arg(ttl_secs)
                            .arg(value.as_ref().as_slice());
                    } else {
                        pipe.cmd(RedisCommand::Set.as_str())
                            .arg(key.as_ref())
                            .arg(value.as_ref().as_slice());
                    }
                }
                pipe.query_async::<()>(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                Ok(())
            }
        })
        .await
    }

    async fn delete_many(&self, keys: &[String]) -> OxCacheResult<()> {
        if keys.is_empty() {
            return Ok(());
        }

        let keys_slice: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        self.delete_many_pipeline(&keys_slice).await
    }
}

#[async_trait]
impl CacheConnector for RedisBackend {
    async fn health_check(&self) -> OxCacheResult<()> {
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                redis::cmd(RedisCommand::Ping.as_str())
                    .query_async::<String>(&mut conn)
                    .await
                    .map_err(error::map_redis_error)?;
                Ok(())
            }
        })
        .await
    }

    async fn shutdown(&self) {
        // Redis connection is managed by ConnectionManager; no explicit close needed.
    }

    fn backend_kind(&self) -> BackendKind {
        if self.mode() == RedisMode::ValkeyStandalone {
            BackendKind::Valkey
        } else {
            BackendKind::Redis
        }
    }

    #[cfg(feature = "lua-script")]
    fn as_lua_executor(&self) -> Option<&dyn crate::backend::interface::LuaExecutor> {
        Some(self)
    }

    fn as_atomic_writer(&self) -> Option<&dyn AtomicCacheWriter> {
        Some(self)
    }
}

impl BackendScore for RedisBackend {
    fn score(&self) -> u8 {
        Scores::REDIS
    }

    fn is_persistent(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &'static str {
        "redis"
    }
}

// ============================================================================
// AtomicCacheWriter Implementation
// ============================================================================

#[async_trait]
impl AtomicCacheWriter for RedisBackend {
    async fn incr(&self, key: &str, delta: i64, ttl: Option<Duration>) -> OxCacheResult<i64> {
        security::validate_redis_key(key)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            async move {
                if let Some(ttl) = ttl {
                    // Atomic INCR + EXPIRE via Lua script to avoid non-atomic two-step
                    let ttl_secs = validate_redis_ttl(ttl)?;
                    let delta_owned = delta.to_string();
                    let script = "local r = redis.call('INCRBY', KEYS[1], ARGV[1]); \
                         redis.call('EXPIRE', KEYS[1], ARGV[2]); \
                         return r";
                    let result: i64 = redis::cmd(RedisCommand::Eval.as_str())
                        .arg(script)
                        .arg(1)
                        .arg(key)
                        .arg(delta_owned.as_str())
                        .arg(ttl_secs)
                        .query_async(&mut conn)
                        .await
                        .map_err(error::map_redis_error)?;
                    Ok(result)
                } else if delta == 1 {
                    let result: i64 = redis::cmd(RedisCommand::Incr.as_str())
                        .arg(key)
                        .query_async(&mut conn)
                        .await
                        .map_err(error::map_redis_error)?;
                    Ok(result)
                } else {
                    let result: i64 = redis::cmd(RedisCommand::IncrBy.as_str())
                        .arg(key)
                        .arg(delta)
                        .query_async(&mut conn)
                        .await
                        .map_err(error::map_redis_error)?;
                    Ok(result)
                }
            }
        })
        .await
    }

    async fn compare_and_swap(
        &self,
        key: &str,
        expected: Option<&[u8]>,
        new: Vec<u8>,
        ttl: Option<Duration>,
    ) -> OxCacheResult<bool> {
        security::validate_redis_key(key)?;

        // Static Lua scripts — avoid per-call heap allocation
        static NO_EXPECT_WITH_TTL: &str = "if redis.call('EXISTS', KEYS[1]) == 0 then \
             redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2]) \
             return 1 else return 0 end";
        static NO_EXPECT_WITHOUT_TTL: &str = "if redis.call('EXISTS', KEYS[1]) == 0 then \
             redis.call('SET', KEYS[1], ARGV[1]) \
             return 1 else return 0 end";
        static WITH_EXPECT_WITH_TTL: &str = "if redis.call('GET', KEYS[1]) == ARGV[1] then \
             redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3]) \
             return 1 else return 0 end";
        static WITH_EXPECT_WITHOUT_TTL: &str = "if redis.call('GET', KEYS[1]) == ARGV[1] then \
             redis.call('SET', KEYS[1], ARGV[2]) \
             return 1 else return 0 end";

        let lua_script: &str = match expected {
            None => {
                if ttl.is_some() {
                    NO_EXPECT_WITH_TTL
                } else {
                    NO_EXPECT_WITHOUT_TTL
                }
            }
            Some(_) => {
                if ttl.is_some() {
                    WITH_EXPECT_WITH_TTL
                } else {
                    WITH_EXPECT_WITHOUT_TTL
                }
            }
        };

        // Validate TTL before entering retry closure
        let ttl_secs = match ttl {
            Some(t) => Some(validate_redis_ttl(t)?),
            None => None,
        };

        self.execute_with_retry(|| {
            let mut conn = self.conn();
            let new = new.clone();
            async move {
                let mut cmd = redis::cmd(RedisCommand::Eval.as_str());
                cmd.arg(lua_script).arg(1).arg(key);
                match expected {
                    None => {
                        cmd.arg(new.as_slice());
                        if let Some(secs) = ttl_secs {
                            cmd.arg(secs);
                        }
                    }
                    Some(exp_bytes) => {
                        cmd.arg(exp_bytes);
                        cmd.arg(new.as_slice());
                        if let Some(secs) = ttl_secs {
                            cmd.arg(secs);
                        }
                    }
                }
                let result: i64 = cmd.query_async(&mut conn).await.map_err(error::map_redis_error)?;
                Ok(result == 1)
            }
        })
        .await
    }

    async fn set_if_absent(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> OxCacheResult<bool> {
        security::validate_redis_key(key)?;
        self.execute_with_retry(|| {
            let mut conn = self.conn();
            let value = value.clone();
            async move {
                let mut cmd = redis::cmd(RedisCommand::Set.as_str());
                cmd.arg(key).arg(value.as_slice()).arg("NX");
                if let Some(ttl) = ttl {
                    let ttl_secs = validate_redis_ttl(ttl)?;
                    cmd.arg("EX").arg(ttl_secs);
                }
                let result: Option<redis::Value> = cmd.query_async(&mut conn).await.map_err(error::map_redis_error)?;
                Ok(result.is_some())
            }
        })
        .await
    }
}
