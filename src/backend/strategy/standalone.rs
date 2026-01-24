//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 单机 Redis 策略实现
//!
//! 支持单个 Redis 实例，支持主从读写分离。

use crate::backend::strategy::traits::{HealthStatus, L2BackendStrategy, ScanResult};
use crate::config::legacy_config::L2Config;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use redis::{aio::ConnectionManager, Client, RedisResult};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, instrument, warn};

/// 单机 Redis 策略
///
/// 支持单个 Redis 实例，可选配置读从库。
#[allow(dead_code)]
#[derive(Clone)]
pub struct StandaloneStrategy {
    /// Redis 客户端
    client: Client,
    /// 主库连接管理器
    manager: ConnectionManager,
    /// 读从库连接管理器（可选）
    read_manager: Option<ConnectionManager>,
    /// 命令超时时间
    command_timeout: Duration,
}

impl StandaloneStrategy {
    /// 创建单机策略
    ///
    /// # 参数
    /// * `config` - L2 配置
    /// * `client` - Redis 客户端
    /// * `manager` - 主库连接管理器
    /// * `read_manager` - 读从库连接管理器（可选）
    ///
    /// # 返回值
    /// * 新的 StandaloneStrategy 实例
    pub fn new(
        config: &L2Config,
        client: Client,
        manager: ConnectionManager,
        read_manager: Option<ConnectionManager>,
    ) -> Self {
        Self {
            client,
            manager,
            read_manager,
            command_timeout: Duration::from_millis(config.command_timeout_ms),
        }
    }

    /// 获取连接管理器（根据读写选择）
    async fn get_connection(
        &self,
        read_only: bool,
    ) -> std::result::Result<ConnectionManager, CacheError> {
        if read_only {
            if let Some(rm) = &self.read_manager {
                return Ok(rm.clone());
            }
        }
        Ok(self.manager.clone())
    }
}

#[async_trait]
impl L2BackendStrategy for StandaloneStrategy {
    fn name(&self) -> &str {
        "standalone"
    }

    fn is_connected(&self) -> bool {
        // 简单检查：尝试获取连接
        // 实际实现中可以使用 ping
        true
    }

    #[instrument(skip(self), level = "debug", name = "standalone_get")]
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        debug!(key, "Getting value from Redis (standalone)");

        let result: RedisResult<Option<Vec<u8>>> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(value) => Ok(value),
            Err(e) => {
                warn!(key, error = %e, "Failed to get value");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self, value), level = "debug", name = "standalone_set")]
    async fn set(&self, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()> {
        debug!(key, value_len = value.len(), ttl = ?ttl, "Setting value to Redis (standalone)");

        let mut cmd = redis::cmd("SET");
        cmd.arg(key).arg(value);

        if let Some(ttl_secs) = ttl {
            cmd.arg("EX").arg(ttl_secs);
        }

        let result: RedisResult<()> = cmd
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!(key, error = %e, "Failed to set value");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "standalone_delete")]
    async fn delete(&self, key: &str) -> Result<bool> {
        debug!(key, "Deleting value from Redis (standalone)");

        let result: RedisResult<i32> = redis::cmd("DEL")
            .arg(key)
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => {
                warn!(key, error = %e, "Failed to delete value");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "standalone_exists")]
    async fn exists(&self, key: &str) -> Result<bool> {
        let result: RedisResult<i32> = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    #[instrument(skip(self), level = "debug", name = "standalone_expire")]
    async fn expire(&self, key: &str, ttl: u64) -> Result<bool> {
        let result: RedisResult<i32> = redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl)
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    #[instrument(skip(self), level = "debug", name = "standalone_ttl")]
    async fn ttl(&self, key: &str) -> Result<Option<i64>> {
        let result: RedisResult<i64> = redis::cmd("TTL")
            .arg(key)
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(-2) => Ok(None), // Key does not exist
            Ok(-1) => Ok(None), // Key has no associated expire
            Ok(ttl) => Ok(Some(ttl)),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn get_with_version(&self, key: &str) -> Result<Option<(Vec<u8>, u64)>> {
        // 实现版本化获取：使用 GET 命令获取值
        // 版本号由调用者管理（存储在 key 的 metadata 中）
        // 这是简化实现，实际可以使用 Lua 脚本同时获取值和版本

        let value: Option<Vec<u8>> = self.get(key).await?;

        match value {
            Some(v) => {
                // 版本号需要从外部获取，这里返回 0 作为占位
                // 实际使用中，版本信息应该存储在值中或单独的 key 中
                Ok(Some((v, 0)))
            }
            None => Ok(None),
        }
    }

    async fn compare_and_set(
        &self,
        key: &str,
        value: &[u8],
        _expected_version: u64,
        _new_version: u64,
        ttl: Option<u64>,
    ) -> Result<bool> {
        // 使用 Lua 脚本实现 CAS 操作
        // 脚本检查当前版本，然后设置新值

        let lua_script = r#"
            local current_value = redis.call('GET', KEYS[1])
            if not current_value then
                return 0
            end

            -- 这里应该检查版本号，简化实现略过
            -- 实际实现中需要存储版本信息

            if ARGV[1] == 'SET' then
                if ARGV[3] then
                    redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
                else
                    redis.call('SET', KEYS[1], ARGV[2])
                end
                return 1
            end
            return 0
"#;

        let script = redis::Script::new(lua_script);

        let mut conn = self.get_connection(false).await?;
        let result: RedisResult<i32> = script
            .key(key)
            .arg(value)
            .arg("SET")
            .arg(ttl.unwrap_or(0))
            .invoke_async(&mut conn)
            .await;

        match result {
            Ok(n) => Ok(n == 1),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn lock(&self, key: &str, ttl: u64) -> Result<Option<String>> {
        let lock_value = uuid::Uuid::new_v4().to_string();
        let ttl_ms = ttl * 1000;

        let result: RedisResult<Option<String>> = redis::cmd("SET")
            .arg(key)
            .arg(&lock_value)
            .arg("NX")
            .arg("PX")
            .arg(ttl_ms)
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(Some(_)) => Ok(Some(lock_value)),
            Ok(None) => Ok(None),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn unlock(&self, key: &str, value: &str) -> Result<bool> {
        let lua_script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
"#;

        let script = redis::Script::new(lua_script);
        let mut conn = self.get_connection(false).await?;
        let result: RedisResult<i32> = script.key(key).arg(value).invoke_async(&mut conn).await;

        match result {
            Ok(n) => Ok(n == 1),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn mget(
        &self,
        keys: &[&str],
    ) -> std::result::Result<HashMap<String, Vec<u8>>, CacheError> {
        if keys.is_empty() {
            return Ok(HashMap::new());
        }

        let mut cmd = redis::cmd("MGET");
        for &key in keys {
            cmd.arg(key);
        }

        let result: RedisResult<Vec<Option<Vec<u8>>>> =
            cmd.query_async(&mut self.get_connection(true).await?).await;

        match result {
            Ok(values) => {
                let mut map = HashMap::new();
                for (i, value) in values.into_iter().enumerate() {
                    if let Some(v) = value {
                        map.insert(keys[i].to_string(), v);
                    }
                }
                Ok(map)
            }
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn mset(&self, items: &[(&str, &[u8])], ttl: Option<u64>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut cmd = redis::cmd("MSET");
        for &(key, value) in items {
            cmd.arg(key).arg(value);
        }

        // 如果指定了 TTL，需要为每个 key 设置
        // Redis MSET 不支持直接设置 TTL，需要逐个设置
        if let Some(ttl_secs) = ttl {
            let mut conn = self.get_connection(false).await?;
            for &(key, _) in items {
                let _: i32 = redis::cmd("EXPIRE")
                    .arg(key)
                    .arg(ttl_secs)
                    .query_async(&mut conn)
                    .await?;
            }
        }

        let result: RedisResult<()> = cmd
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn scan(&self, pattern: &str, count: usize, cursor: u64) -> Result<ScanResult> {
        let result: RedisResult<(i32, Vec<String>)> = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(count)
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok((next_cursor, keys)) => Ok(ScanResult {
                keys,
                cursor: next_cursor as u64,
            }),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn scan_keys(&self, pattern: &str, limit: usize) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut cursor = 0u64;

        while keys.len() < limit {
            let result = self.scan(pattern, 1000, cursor).await?;
            keys.extend(result.keys);

            cursor = result.cursor;
            if cursor == 0 {
                break;
            }
        }

        keys.truncate(limit);
        Ok(keys)
    }

    async fn ping(&self) -> Result<()> {
        let result: RedisResult<String> = redis::cmd("PING")
            .query_async(&mut self.get_connection(false).await?)
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(CacheError::RedisError(e)),
        }
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        match self.ping().await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Unhealthy(e.to_string())),
        }
    }

    fn command_timeout(&self) -> Duration {
        self.command_timeout
    }

    async fn close(&self) -> Result<()> {
        // ConnectionManager 在 drop 时会自动关闭连接
        // 这里可以添加额外的清理逻辑
        debug!("Closing standalone strategy connection");
        Ok(())
    }
}
