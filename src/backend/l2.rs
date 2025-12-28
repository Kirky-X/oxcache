//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了L2缓存后端的实现，基于Redis的分布式缓存。

use crate::backend::redis_provider::{DefaultRedisProvider, RedisProvider};
use crate::config::{L2Config, RedisMode};
use crate::error::{CacheError, Result};
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use std::sync::Arc;
use tracing::{debug, instrument};

/// L2缓存后端实现
///
/// 基于Redis的分布式缓存实现
#[derive(Clone)]
pub enum L2Backend {
    Standalone {
        client: Client,
        manager: ConnectionManager,
        read_manager: Box<Option<ConnectionManager>>,
        command_timeout_ms: u64,
    },
    Cluster {
        client: redis::cluster::ClusterClient,
        command_timeout_ms: u64,
    },
}

impl std::fmt::Debug for L2Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standalone { .. } => write!(f, "L2Backend::Standalone"),
            Self::Cluster { .. } => write!(f, "L2Backend::Cluster"),
        }
    }
}

impl L2Backend {
    /// 获取命令超时时间（毫秒）
    pub fn command_timeout_ms(&self) -> u64 {
        match self {
            L2Backend::Standalone {
                command_timeout_ms, ..
            } => *command_timeout_ms,
            L2Backend::Cluster {
                command_timeout_ms, ..
            } => *command_timeout_ms,
        }
    }

    /// 创建新的L2缓存后端实例
    ///
    /// # 参数
    ///
    /// * `config` - L2缓存配置
    ///
    /// # 返回值
    ///
    /// 返回新的L2Backend实例或错误
    #[instrument(skip(config), level = "info", name = "init_l2_backend")]
    pub async fn new(config: &L2Config) -> Result<Self> {
        Self::new_with_provider(config, Arc::new(DefaultRedisProvider)).await
    }

    /// 使用指定的Redis提供者创建新的L2缓存后端实例
    ///
    /// # 参数
    ///
    /// * `config` - L2缓存配置
    /// * `provider` - Redis提供者
    ///
    /// # 返回值
    ///
    /// 返回新的L2Backend实例或错误
    #[instrument(skip(config, provider), level = "info", fields(mode = ?config.mode))]
    pub async fn new_with_provider(
        config: &L2Config,
        provider: Arc<dyn RedisProvider>,
    ) -> Result<Self> {
        debug!("Initializing L2Backend with mode: {:?}", config.mode);
        match config.mode {
            RedisMode::Standalone => {
                let (client, manager) = provider.get_standalone_client(config).await?;
                Ok(L2Backend::Standalone {
                    client,
                    manager,
                    read_manager: Box::new(None),
                    command_timeout_ms: config.command_timeout_ms,
                })
            }
            RedisMode::Cluster => {
                let client = provider.get_cluster_client(config).await?;
                Ok(L2Backend::Cluster {
                    client,
                    command_timeout_ms: config.command_timeout_ms,
                })
            }
            RedisMode::Sentinel => {
                let (client, manager, read_manager) = provider.get_sentinel_client(config).await?;
                Ok(L2Backend::Standalone {
                    client,
                    manager,
                    read_manager: Box::new(read_manager),
                    command_timeout_ms: config.command_timeout_ms,
                })
            }
        }
    }

    #[cfg(test)]
    pub async fn new_failing(config: &L2Config) -> Result<Self> {
        use redis::ConnectionAddr;

        let connection_info = redis::ConnectionInfo {
            addr: ConnectionAddr::Tcp("10.255.255.1".to_string(), 6379),
            redis: redis::RedisConnectionInfo {
                db: 0,
                username: None,
                password: None,
                protocol: redis::ProtocolVersion::RESP2,
            },
        };

        let client = Client::open(connection_info)
            .map_err(|e| CacheError::Configuration(format!("Failed to create client: {}", e)))?;

        let manager = ConnectionManager::new(client.clone())
            .await
            .map_err(CacheError::RedisError)?;

        Ok(L2Backend::Standalone {
            client,
            manager,
            read_manager: Box::new(None),
            command_timeout_ms: config.command_timeout_ms,
        })
    }

    /// 尝试获取分布式锁
    ///
    /// 使用 SET NX PX 实现
    #[instrument(skip(self), level = "debug")]
    pub async fn lock(&self, key: &str, value: &str, ttl: u64) -> Result<bool> {
        let ttl_ms = ttl * 1000;
        debug!(
            "Attempting to acquire lock: key={}, value={}, ttl={}s ({}ms)",
            key, value, ttl, ttl_ms
        );
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: Option<String> = redis::cmd("SET")
                    .arg(key)
                    .arg(value)
                    .arg("NX")
                    .arg("PX")
                    .arg(ttl_ms)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| CacheError::BackendError(e.to_string()))?;
                debug!(
                    "Lock acquisition result: success={}, result={:?}",
                    result.is_some(),
                    result
                );
                Ok(result.is_some())
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client
                    .get_async_connection()
                    .await
                    .map_err(|e| CacheError::BackendError(e.to_string()))?;
                let result: Option<String> = redis::cmd("SET")
                    .arg(key)
                    .arg(value)
                    .arg("NX")
                    .arg("PX")
                    .arg(ttl_ms)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| CacheError::BackendError(e.to_string()))?;
                debug!(
                    "Lock acquisition result: success={}, result={:?}",
                    result.is_some(),
                    result
                );
                Ok(result.is_some())
            }
        }
    }

    /// 释放分布式锁
    ///
    /// 使用 Lua 脚本保证原子性
    #[instrument(skip(self), level = "debug")]
    pub async fn unlock(&self, key: &str, value: &str) -> Result<bool> {
        let script = redis::Script::new(
            r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
            "#,
        );

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: i32 = script
                    .key(key)
                    .arg(value)
                    .invoke_async(&mut conn)
                    .await
                    .map_err(|e| CacheError::BackendError(e.to_string()))?;
                Ok(result == 1)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client
                    .get_async_connection()
                    .await
                    .map_err(|e| CacheError::BackendError(e.to_string()))?;
                let result: i32 = script
                    .key(key)
                    .arg(value)
                    .invoke_async(&mut conn)
                    .await
                    .map_err(|e| CacheError::BackendError(e.to_string()))?;
                Ok(result == 1)
            }
        }
    }

    /// 获取带版本号的缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回缓存值和版本号的元组，如果不存在则返回None
    #[instrument(skip(self), level = "debug")]
    pub async fn get_with_version(&self, key: &str) -> Result<Option<(Vec<u8>, u64)>> {
        let script = redis::Script::new(
            r#"
            local val = redis.call('GET', KEYS[1])
            if not val then
                return nil
            end
            local ver = redis.call('GET', KEYS[1] .. ':version')
            if not ver then
                ver = "0"
            end
            return {val, ver}
            "#,
        );

        let result: Option<(Vec<u8>, String)> = match self {
            L2Backend::Standalone {
                manager,
                read_manager,
                ..
            } => {
                let mut conn = if let Some(rm) = read_manager.as_ref() {
                    rm.clone()
                } else {
                    manager.clone()
                };
                script.key(key).invoke_async(&mut conn).await?
            }
            L2Backend::Cluster { client, .. } => {
                script
                    .key(key)
                    .invoke_async(&mut client.get_async_connection().await?)
                    .await?
            }
        };

        match result {
            Some((v, s)) => Ok(Some((v, s.parse().unwrap_or(0)))),
            None => Ok(None),
        }
    }

    /// 设置带版本号的缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值（字节数组）
    /// * `ttl` - 过期时间（秒），None表示使用默认值3600秒
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self, value), level = "debug", fields(value_len = value.len()))]
    pub async fn set_with_version(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<u64>,
    ) -> Result<()> {
        debug!("Setting key: {} with ttl: {:?}", key, ttl);
        let ttl = ttl.unwrap_or(3600);

        // Lua脚本用于原子设置+版本递增
        let script = redis::Script::new(
            r#"
            redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[2])
            redis.call('INCR', KEYS[1] .. ':version')
            redis.call('EXPIRE', KEYS[1] .. ':version', ARGV[2])
            return 1
            "#,
        );

        let _: i32 = match self {
            L2Backend::Standalone { manager, .. } => {
                script
                    .clone()
                    .key(key)
                    .arg(&value)
                    .arg(ttl)
                    .invoke_async(&mut manager.clone())
                    .await?
            }
            L2Backend::Cluster { client, .. } => {
                script
                    .clone()
                    .key(key)
                    .arg(&value)
                    .arg(ttl)
                    .invoke_async(&mut client.get_async_connection().await?)
                    .await?
            }
        };

        Ok(())
    }

    /// 删除缓存项
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub async fn delete(&self, key: &str) -> Result<()> {
        debug!("Deleting key: {}", key);
        let version_key = format!("{}:version", key);
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let _: () = redis::pipe()
                    .del(key)
                    .del(&version_key)
                    .query_async(&mut conn)
                    .await?;
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let _: () = redis::pipe()
                    .del(key)
                    .del(&version_key)
                    .query_async(&mut conn)
                    .await?;
            }
        }
        Ok(())
    }

    /// 获取缓存项的剩余生存时间
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回剩余生存时间（秒），如果不存在则返回None
    #[instrument(skip(self), level = "debug")]
    pub async fn ttl(&self, key: &str) -> Result<Option<u64>> {
        let ttl: i64 = match self {
            L2Backend::Standalone { manager, .. } => manager.clone().ttl(key).await?,
            L2Backend::Cluster { client, .. } => {
                client.get_async_connection().await?.ttl(key).await?
            }
        };
        if ttl > 0 {
            Ok(Some(ttl as u64))
        } else {
            Ok(None)
        }
    }

    /// 检查连接是否正常
    ///
    /// # 返回值
    ///
    /// 返回操作结果，成功表示连接正常
    #[instrument(skip(self), level = "debug")]
    pub async fn ping(&self) -> Result<()> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                tracing::debug!("L2Backend ping: 尝试连接Redis...");
                let mut conn = manager.clone();
                tracing::debug!("L2Backend ping: 获取连接管理器克隆");
                match redis::cmd("PING").query_async::<String>(&mut conn).await {
                    Ok(response) => {
                        tracing::debug!("L2Backend ping: Redis响应成功: {}", response);
                        Ok(())
                    }
                    Err(e) => {
                        tracing::debug!("L2Backend ping: Redis连接失败: {}", e);
                        Err(e.into())
                    }
                }
            }
            L2Backend::Cluster { client, .. } => {
                tracing::debug!("L2Backend ping: 尝试连接Redis集群...");
                match redis::cmd("PING")
                    .query_async::<String>(&mut client.get_async_connection().await?)
                    .await
                {
                    Ok(response) => {
                        tracing::debug!("L2Backend ping: Redis集群响应成功: {}", response);
                        Ok(())
                    }
                    Err(e) => {
                        tracing::debug!("L2Backend ping: Redis集群连接失败: {}", e);
                        Err(e.into())
                    }
                }
            }
        }
    }

    /// 批量设置缓存项
    ///
    /// # 参数
    ///
    /// * `items` - 要设置的键值对向量
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self, items), level = "debug", fields(item_count = items.len()))]
    pub async fn pipeline_set_batch(
        &self,
        items: Vec<(String, Vec<u8>, Option<u64>)>,
    ) -> Result<()> {
        debug!("Pipeline batch set with {} items", items.len());
        let mut pipe = redis::pipe();

        for (key, value, ttl) in items {
            let ttl = ttl.unwrap_or(3600);
            let ttl_i64 = ttl.try_into().unwrap_or(3600);
            pipe.set(&key, value).arg("EX").arg(ttl_i64).ignore();
            pipe.incr(format!("{}:version", key), 1).ignore();
            pipe.expire(format!("{}:version", key), ttl_i64).ignore();
        }

        match self {
            L2Backend::Standalone { manager, .. } => {
                pipe.query_async::<()>(&mut manager.clone()).await?;
            }
            L2Backend::Cluster { client, .. } => {
                pipe.query_async::<()>(&mut client.get_async_connection().await?)
                    .await?;
            }
        }
        Ok(())
    }

    /// 批量删除缓存项
    ///
    /// # 参数
    ///
    /// * `keys` - 要删除的键向量
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self, keys), level = "debug", fields(key_count = keys.len()))]
    pub async fn pipeline_del_batch(&self, keys: Vec<String>) -> Result<()> {
        debug!("Pipeline batch delete with {} keys", keys.len());
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.del(&key).ignore();
            pipe.del(format!("{}:version", key)).ignore();
        }

        match self {
            L2Backend::Standalone { manager, .. } => {
                pipe.query_async::<()>(&mut manager.clone()).await?;
            }
            L2Backend::Cluster { client, .. } => {
                pipe.query_async::<()>(&mut client.get_async_connection().await?)
                    .await?;
            }
        }
        Ok(())
    }

    /// 通过管道重放WAL条目
    ///
    /// # 参数
    ///
    /// * `entries` - 要重放的WAL条目向量
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self, entries), level = "debug", fields(entry_count = entries.len()))]
    pub async fn pipeline_replay(
        &self,
        entries: Vec<crate::recovery::wal::WalEntry>,
    ) -> Result<()> {
        debug!("Replaying WAL with {} entries", entries.len());
        let mut pipe = redis::pipe();

        for entry in entries {
            match entry.operation {
                crate::recovery::wal::Operation::Set => {
                    if let Some(val) = entry.value {
                        pipe.set(&entry.key, val).ignore();
                        if let Some(t) = entry.ttl {
                            pipe.expire(&entry.key, (t as usize).try_into().unwrap())
                                .ignore();
                        }
                        pipe.incr(format!("{}:version", entry.key), 1).ignore();
                    }
                }
                crate::recovery::wal::Operation::Delete => {
                    pipe.del(&entry.key).ignore();
                    pipe.del(format!("{}:version", entry.key)).ignore();
                }
            }
        }

        match self {
            L2Backend::Standalone { manager, .. } => {
                pipe.query_async::<()>(&mut manager.clone()).await?;
            }
            L2Backend::Cluster { client, .. } => {
                pipe.query_async::<()>(&mut client.get_async_connection().await?)
                    .await?;
            }
        }
        Ok(())
    }

    /// 获取原始Redis客户端
    ///
    /// # 返回值
    ///
    /// 返回Redis客户端实例
    pub fn get_raw_client(&self) -> Result<Client> {
        match self {
            L2Backend::Standalone { client, .. } => Ok(client.clone()),
            L2Backend::Cluster { .. } => Err(CacheError::NotSupported(
                "get_raw_client is not supported in Cluster mode".to_string(),
            )),
        }
    }

    /// 设置字节数组缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 字节数组值
    /// * `ttl` - 过期时间（秒），None表示使用默认值3600秒
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self, value), level = "debug", fields(value_len = value.len()))]
    pub async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        self.set_with_version(key, value, ttl).await
    }

    /// 获取字节数组缓存值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回字节数组值，如果不存在则返回None
    #[instrument(skip(self), level = "debug")]
    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.get_with_version(key).await? {
            Some((value, _version)) => Ok(Some(value)),
            None => Ok(None),
        }
    }

    /// 检查键是否存在
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回键是否存在
    #[instrument(skip(self), level = "debug")]
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let exists: bool = redis::cmd("EXISTS").arg(key).query_async(&mut conn).await?;
                Ok(exists)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let exists: bool = redis::cmd("EXISTS").arg(key).query_async(&mut conn).await?;
                Ok(exists)
            }
        }
    }

    /// 仅当键不存在时设置值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值
    /// * `ttl` - 过期时间（秒）
    ///
    /// # 返回值
    ///
    /// 返回是否设置成功
    #[instrument(skip(self, value), level = "debug")]
    pub async fn set_nx(&self, key: &str, value: &str, ttl: Option<u64>) -> Result<bool> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: Option<String> = if let Some(ttl) = ttl {
                    redis::cmd("SET")
                        .arg(key)
                        .arg(value)
                        .arg("NX")
                        .arg("EX")
                        .arg(ttl)
                        .query_async(&mut conn)
                        .await?
                } else {
                    redis::cmd("SET")
                        .arg(key)
                        .arg(value)
                        .arg("NX")
                        .query_async(&mut conn)
                        .await?
                };
                Ok(result.is_some())
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: Option<String> = if let Some(ttl) = ttl {
                    redis::cmd("SET")
                        .arg(key)
                        .arg(value)
                        .arg("NX")
                        .arg("EX")
                        .arg(ttl)
                        .query_async(&mut conn)
                        .await?
                } else {
                    redis::cmd("SET")
                        .arg(key)
                        .arg(value)
                        .arg("NX")
                        .query_async(&mut conn)
                        .await?
                };
                Ok(result.is_some())
            }
        }
    }

    /// 增加键的值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回增加后的值
    #[instrument(skip(self), level = "debug")]
    pub async fn incr(&self, key: &str) -> Result<i64> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: i64 = redis::cmd("INCR").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: i64 = redis::cmd("INCR").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 设置键的过期时间
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `ttl` - 过期时间（秒）
    ///
    /// # 返回值
    ///
    /// 返回是否设置成功
    #[instrument(skip(self), level = "debug")]
    pub async fn expire(&self, key: &str, ttl: u64) -> Result<bool> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: bool = redis::cmd("EXPIRE")
                    .arg(key)
                    .arg(ttl)
                    .query_async(&mut conn)
                    .await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: bool = redis::cmd("EXPIRE")
                    .arg(key)
                    .arg(ttl)
                    .query_async(&mut conn)
                    .await?;
                Ok(result)
            }
        }
    }

    /// 获取键对应的值类型
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回值类型字符串
    #[instrument(skip(self), level = "debug")]
    pub async fn get_type(&self, key: &str) -> Result<String> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: String = redis::cmd("TYPE").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: String = redis::cmd("TYPE").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 清空 L2 缓存
    ///
    /// 注意：此操作会删除所有以服务名为前缀的缓存键
    ///
    /// # 参数
    ///
    /// * `service_name` - 服务名称，用于构建键前缀
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub async fn clear(&self, service_name: &str) -> Result<()> {
        debug!("L2 clear: 清空服务 {} 的所有缓存项", service_name);
        let pattern = format!("{}:*", service_name);

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let mut cursor = 0i64;
                loop {
                    let (next_cursor, keys): (i64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(&pattern)
                        .arg("COUNT")
                        .arg(1000)
                        .query_async(&mut conn)
                        .await?;

                    if !keys.is_empty() {
                        let mut pipe = redis::pipe();
                        for key in &keys {
                            pipe.del(key).ignore();
                            pipe.del(format!("{}:version", key)).ignore();
                        }
                        pipe.query_async::<()>(&mut conn).await?;
                    }

                    cursor = next_cursor;
                    if cursor == 0 {
                        break;
                    }
                }
            }
            L2Backend::Cluster { client, .. } => {
                let mut cursor = 0i64;
                loop {
                    let mut conn = client.get_async_connection().await?;
                    let (next_cursor, keys): (i64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(&pattern)
                        .arg("COUNT")
                        .arg(1000)
                        .query_async(&mut conn)
                        .await?;

                    if !keys.is_empty() {
                        let mut pipe = redis::pipe();
                        for key in &keys {
                            pipe.del(key).ignore();
                            pipe.del(format!("{}:version", key)).ignore();
                        }
                        pipe.query_async::<()>(&mut conn).await?;
                    }

                    cursor = next_cursor;
                    if cursor == 0 {
                        break;
                    }
                }
            }
        }

        debug!("L2 clear: 缓存已清空");
        Ok(())
    }
}
