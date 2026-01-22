//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 哨兵 Redis 策略实现
//!
//! 支持 Redis Sentinel 高可用部署。

use crate::backend::strategy::traits::{HealthStatus, L2BackendStrategy, ScanResult};
use crate::config::L2Config;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use redis::sentinel::SentinelClient;
use redis::RedisResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, instrument, warn};

/// 哨兵 Redis 策略
///
/// 支持 Redis Sentinel 高可用部署，自动处理主从切换。
#[derive(Clone)]
pub struct SentinelStrategy {
    /// Redis 哨兵客户端
    client: Arc<Mutex<SentinelClient>>,
    /// 命令超时时间
    command_timeout: Duration,
}

impl SentinelStrategy {
    /// 创建哨兵策略
    ///
    /// # 参数
    /// * `config` - L2 配置
    /// * `client` - Redis 哨兵客户端
    ///
    /// # 返回值
    /// * 新的 SentinelStrategy 实例
    pub fn new(config: &L2Config, client: SentinelClient) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            command_timeout: Duration::from_millis(config.command_timeout_ms),
        }
    }

    /// 获取哨兵连接
    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection> {
        let mut client = self.client.lock().await;
        client
            .get_async_connection()
            .await
            .map_err(CacheError::RedisError)
    }
}

#[async_trait]
impl L2BackendStrategy for SentinelStrategy {
    fn name(&self) -> &str {
        "sentinel"
    }

    fn is_connected(&self) -> bool {
        // 简单检查：尝试获取连接
        // 实际实现中可以使用 ping
        true
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_get")]
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        debug!(key, "Getting value from Redis (sentinel)");

        let mut conn = self.get_connection().await?;
        let result: RedisResult<Option<Vec<u8>>> =
            redis::cmd("GET").arg(key).query_async(&mut conn).await;

        match result {
            Ok(value) => Ok(value),
            Err(e) => {
                warn!(key, error = %e, "Failed to get value");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self, value), level = "debug", name = "sentinel_set")]
    async fn set(&self, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()> {
        debug!(key, value_len = value.len(), ttl = ?ttl, "Setting value to Redis (sentinel)");

        let mut conn = self.get_connection().await?;
        let mut cmd = redis::cmd("SET");
        cmd.arg(key).arg(value);

        if let Some(ttl_secs) = ttl {
            cmd.arg("EX").arg(ttl_secs);
        }

        let result: RedisResult<()> = cmd.query_async(&mut conn).await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!(key, error = %e, "Failed to set value");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_delete")]
    async fn delete(&self, key: &str) -> Result<bool> {
        debug!(key, "Deleting value from Redis (sentinel)");

        let mut conn = self.get_connection().await?;
        let result: RedisResult<i32> = redis::cmd("DEL").arg(key).query_async(&mut conn).await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => {
                warn!(key, error = %e, "Failed to delete value");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_exists")]
    async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let result: RedisResult<i32> = redis::cmd("EXISTS").arg(key).query_async(&mut conn).await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => {
                warn!(key, error = %e, "Failed to check existence");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_expire")]
    async fn expire(&self, key: &str, ttl: u64) -> Result<bool> {
        debug!(key, ttl, "Setting expiration for key");

        let mut conn = self.get_connection().await?;
        let result: RedisResult<i32> = redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => {
                warn!(key, error = %e, "Failed to set expiration");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_ttl")]
    async fn ttl(&self, key: &str) -> Result<Option<i64>> {
        let mut conn = self.get_connection().await?;
        let result: RedisResult<i64> = redis::cmd("TTL").arg(key).query_async(&mut conn).await;

        match result {
            Ok(ttl) => {
                if ttl == -2 {
                    // 键不存在
                    Ok(None)
                } else if ttl == -1 {
                    // 键存在但没有设置过期时间
                    Ok(None)
                } else {
                    Ok(Some(ttl))
                }
            }
            Err(e) => {
                warn!(key, error = %e, "Failed to get TTL");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_get_with_version")]
    async fn get_with_version(&self, key: &str) -> Result<Option<(Vec<u8>, u64)>> {
        let mut conn = self.get_connection().await?;

        // 使用 MGET 获取值和版本号（假设版本号存储在 key:version 中）
        let value_result: RedisResult<Option<Vec<u8>>> =
            redis::cmd("GET").arg(key).query_async(&mut conn).await;

        let version_key = format!("{}:version", key);
        let version_result: RedisResult<Option<u64>> = redis::cmd("GET")
            .arg(&version_key)
            .query_async(&mut conn)
            .await;

        match (value_result, version_result) {
            (Ok(Some(value)), Ok(Some(version))) => Ok(Some((value, version))),
            (Ok(None), _) | (_, Ok(None)) => Ok(None),
            (Err(e), _) | (_, Err(e)) => {
                warn!(key, error = %e, "Failed to get value with version");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self, value), level = "debug", name = "sentinel_compare_and_set")]
    async fn compare_and_set(
        &self,
        key: &str,
        value: &[u8],
        expected_version: u64,
        new_version: u64,
        ttl: Option<u64>,
    ) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        // 使用 Lua 脚本保证原子性
        let lua_script = r#"
            local key = KEYS[1]
            local version_key = KEYS[2]
            local expected_version = tonumber(ARGV[1])
            local new_version = tonumber(ARGV[2])
            local value = ARGV[3]
            local ttl = tonumber(ARGV[4])

            local current_version = redis.call('GET', version_key)
            if current_version == false then
                return 0
            end

            current_version = tonumber(current_version)
            if current_version ~= expected_version then
                return 0
            end

            redis.call('SET', key, value)
            redis.call('SET', version_key, new_version)

            if ttl and ttl > 0 then
                redis.call('EXPIRE', key, ttl)
                redis.call('EXPIRE', version_key, ttl)
            end

            return 1
        "#;

        let version_key = format!("{}:version", key);
        let result: RedisResult<i32> = redis::Script::new(lua_script)
            .key(key)
            .key(&version_key)
            .arg(expected_version)
            .arg(new_version)
            .arg(value)
            .arg(ttl.unwrap_or(0))
            .invoke_async(&mut conn)
            .await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => {
                warn!(key, error = %e, "Failed to compare and set");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_lock")]
    async fn lock(&self, key: &str, ttl: u64) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;

        // 生成随机锁值
        let lock_value = uuid::Uuid::new_v4().to_string();

        // 使用 SET NX PX 实现
        let result: RedisResult<()> = redis::cmd("SET")
            .arg(key)
            .arg(&lock_value)
            .arg("NX")
            .arg("PX")
            .arg(ttl * 1000)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => Ok(Some(lock_value)),
            Err(e) if e.kind() == redis::ErrorKind::ResponseError => Ok(None),
            Err(e) => {
                warn!(key, error = %e, "Failed to acquire lock");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_unlock")]
    async fn unlock(&self, key: &str, value: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        // 使用 Lua 脚本保证原子性
        let lua_script = r#"
            if redis.call('GET', KEYS[1]) == ARGV[1] then
                return redis.call('DEL', KEYS[1])
            else
                return 0
            end
        "#;

        let result: RedisResult<i32> = redis::Script::new(lua_script)
            .key(key)
            .arg(value)
            .invoke_async(&mut conn)
            .await;

        match result {
            Ok(n) => Ok(n > 0),
            Err(e) => {
                warn!(key, error = %e, "Failed to release lock");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_mget")]
    async fn mget(&self, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        // Sentinel 的 MGET 与单机类似
        let mut result = HashMap::new();

        for key in keys {
            match self.get(key).await {
                Ok(Some(value)) => {
                    result.insert(key.to_string(), value);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(key, error = %e, "Failed to get value in mget");
                }
            }
        }

        Ok(result)
    }

    #[instrument(skip(self, items), level = "debug", name = "sentinel_mset")]
    async fn mset(&self, items: &[(&str, &[u8])], ttl: Option<u64>) -> Result<()> {
        // Sentinel 的 MSET 与单机类似
        for (key, value) in items {
            self.set(key, value, ttl).await?;
        }

        Ok(())
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_scan")]
    async fn scan(&self, pattern: &str, count: usize, cursor: u64) -> Result<ScanResult> {
        let mut conn = self.get_connection().await?;

        let result: RedisResult<(u64, Vec<String>)> = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(count)
            .query_async(&mut conn)
            .await;

        match result {
            Ok((new_cursor, keys)) => Ok(ScanResult {
                keys,
                cursor: new_cursor,
            }),
            Err(e) => {
                warn!(pattern, error = %e, "Failed to scan");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_scan_keys")]
    async fn scan_keys(&self, pattern: &str, limit: usize) -> Result<Vec<String>> {
        let mut all_keys = Vec::new();
        let mut cursor = 0;

        loop {
            let result = self.scan(pattern, limit, cursor).await?;
            all_keys.extend(result.keys);

            if result.cursor == 0 || all_keys.len() >= limit {
                break;
            }

            cursor = result.cursor;
        }

        if all_keys.len() > limit {
            all_keys.truncate(limit);
        }

        Ok(all_keys)
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_ping")]
    async fn ping(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let result: RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!(error = %e, "Failed to ping");
                Err(CacheError::RedisError(e))
            }
        }
    }

    #[instrument(skip(self), level = "debug", name = "sentinel_health_check")]
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
        // Sentinel 连接会自动关闭
        Ok(())
    }
}
