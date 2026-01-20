//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了L2缓存后端的实现，基于Redis的分布式缓存。

use crate::backend::redis_provider::{DefaultRedisProvider, RedisProvider};
use crate::client::redis_native::{RedisNativeOps, ScanKeyIterator, ZSetMember};
use crate::config::{L2Config, RedisMode};
use crate::error::{CacheError, Result};
#[cfg(feature = "l2-redis")]
use crate::security::{clamp_scan_count, validate_lua_script, validate_redis_key, validate_scan_pattern};
use async_trait::async_trait;
use dashmap::DashMap;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, instrument, warn};

// Version cache eviction constants
const VERSION_CACHE_MAX_SIZE: usize = 10000;
const VERSION_CACHE_EVICTION_BATCH: usize = 1000;

/// 验证并返回安全的Redis键
///
/// # 参数
/// * `key` - 要验证的缓存键
///
/// # 返回值
/// * `Ok(&str)` - 验证通过的键引用
/// * `Err(CacheError)` - 键验证失败
fn ensure_safe_key(key: &str) -> Result<&str> {
    #[cfg(feature = "l2-redis")]
    validate_redis_key(key)?;
    Ok(key)
}

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
        version_cache: Arc<DashMap<String, u64>>,
    },
    Cluster {
        client: redis::cluster::ClusterClient,
        command_timeout_ms: u64,
        version_cache: Arc<DashMap<String, u64>>,
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
                    version_cache: Arc::new(DashMap::new()),
                })
            }
            RedisMode::Cluster => {
                let client = provider.get_cluster_client(config).await?;
                Ok(L2Backend::Cluster {
                    client,
                    command_timeout_ms: config.command_timeout_ms,
                    version_cache: Arc::new(DashMap::new()),
                })
            }
            RedisMode::Sentinel => {
                let (client, manager, read_manager) = provider.get_sentinel_client(config).await?;
                Ok(L2Backend::Standalone {
                    client,
                    manager,
                    read_manager: Box::new(read_manager),
                    command_timeout_ms: config.command_timeout_ms,
                    version_cache: Arc::new(DashMap::new()),
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
            version_cache: Arc::new(DashMap::new()),
        })
    }

    /// 尝试获取分布式锁
    ///
    /// 使用 SET NX PX 实现，自动生成安全的随机锁值
    ///
    /// # 返回值
    ///
    /// * `Ok(Some(value))` - 成功获取锁，value 为生成的锁值
    /// * `Ok(None)` - 锁已被其他进程持有
    /// * `Err(...)` - 发生错误
    #[instrument(skip(self), level = "debug")]
    pub async fn lock(&self, key: &str, ttl: u64) -> Result<Option<String>> {
        // 生成安全的随机 UUID 作为锁值
        let lock_value = uuid::Uuid::new_v4().to_string();
        let ttl_ms = ttl * 1000;
        debug!(
            "Attempting to acquire lock: key={}, value={}, ttl={}s ({}ms)",
            key, lock_value, ttl, ttl_ms
        );
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: Option<String> = redis::cmd("SET")
                    .arg(key)
                    .arg(&lock_value)
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
                if result.is_some() {
                    Ok(Some(lock_value))
                } else {
                    Ok(None)
                }
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client
                    .get_async_connection()
                    .await
                    .map_err(|e| CacheError::BackendError(e.to_string()))?;
                let result: Option<String> = redis::cmd("SET")
                    .arg(key)
                    .arg(&lock_value)
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
                if result.is_some() {
                    Ok(Some(lock_value))
                } else {
                    Ok(None)
                }
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
        // 先尝试从缓存获取版本号（无锁读取）
        let _cached_version = match self {
            L2Backend::Standalone { version_cache, .. } => {
                version_cache.get(key).map(|v| *v.value())
            }
            L2Backend::Cluster { version_cache, .. } => version_cache.get(key).map(|v| *v.value()),
        };

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
            Some((v, s)) => {
                let version = s.parse().unwrap_or(0);
                // 更新缓存（无锁写入）
                match self {
                    L2Backend::Standalone { version_cache, .. } => {
                        // 使用 LRU 策略：如果缓存超过 10000，移除 1000 个最旧的条目
                        if version_cache.len() > 10000 {
                            let mut to_remove = Vec::new();
                            for entry in version_cache.iter() {
                                to_remove.push(entry.key().clone());
                                if to_remove.len() >= 1000 {
                                    break;
                                }
                            }
                            for key in to_remove {
                                version_cache.remove(&key);
                            }
                        }
                        version_cache.insert(key.to_string(), version);
                    }
                    L2Backend::Cluster { version_cache, .. } => {
                        // 使用 LRU 策略：如果缓存超过阈值，移除一批最旧的条目
                        if version_cache.len() > VERSION_CACHE_MAX_SIZE {
                            let keys: Vec<_> = version_cache
                                .iter()
                                .take(VERSION_CACHE_EVICTION_BATCH)
                                .map(|e| e.key().clone())
                                .collect();
                            for key in keys {
                                version_cache.remove(&key);
                            }
                        }
                        version_cache.insert(key.to_string(), version);
                    }
                }
                Ok(Some((v, version)))
            }
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

        // 更新版本缓存（无锁写入）
        match self {
            L2Backend::Standalone { version_cache, .. } => {
                // 使用 LRU 策略：如果缓存超过 10000，移除 1000 个最旧的条目
                if version_cache.len() > 10000 {
                    let mut to_remove = Vec::new();
                    for entry in version_cache.iter() {
                        to_remove.push(entry.key().clone());
                        if to_remove.len() >= 1000 {
                            break;
                        }
                    }
                    for key in to_remove {
                        version_cache.remove(&key);
                    }
                }
                let new_version = version_cache.get(key).map(|v| *v.value() + 1).unwrap_or(1);
                version_cache.insert(key.to_string(), new_version);
            }
            L2Backend::Cluster { version_cache, .. } => {
                // 使用 LRU 策略：如果缓存超过 10000，移除 1000 个最旧的条目
                if version_cache.len() > 10000 {
                    let mut to_remove = Vec::new();
                    for entry in version_cache.iter() {
                        to_remove.push(entry.key().clone());
                        if to_remove.len() >= 1000 {
                            break;
                        }
                    }
                    for key in to_remove {
                        version_cache.remove(&key);
                    }
                }
                let new_version = version_cache.get(key).map(|v| *v.value() + 1).unwrap_or(1);
                version_cache.insert(key.to_string(), new_version);
            }
        }

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
            L2Backend::Standalone {
                manager,
                version_cache,
                ..
            } => {
                let mut conn = manager.clone();
                let _: () = redis::pipe()
                    .del(key)
                    .del(&version_key)
                    .query_async(&mut conn)
                    .await?;
                // 从版本缓存中移除（无锁删除）
                version_cache.remove(key);
            }
            L2Backend::Cluster {
                client,
                version_cache,
                ..
            } => {
                let mut conn = client.get_async_connection().await?;
                let _: () = redis::pipe()
                    .del(key)
                    .del(&version_key)
                    .query_async(&mut conn)
                    .await?;
                // 从版本缓存中移除（无锁删除）
                version_cache.remove(key);
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
            // 验证键安全性，跳过无效键并记录警告
            #[cfg(feature = "l2-redis")]
            if let Err(e) = validate_redis_key(&entry.key) {
                warn!(
                    "Skipping invalid WAL entry key '{}': {}",
                    entry.key, e
                );
                continue;
            }

            match entry.operation {
                crate::recovery::wal::Operation::Set => {
                    if let Some(val) = entry.value {
                        pipe.set(&entry.key, val).ignore();
                        // 验证 TTL 范围，防止命令注入和 panic
                        if let Some(t) = entry.ttl {
                            // 检查 TTL 是否在合理范围内（1秒 - 30天）
                            if !(0..=30 * 24 * 3600).contains(&t) {
                                warn!(
                                    "Invalid TTL value {} in WAL entry for key {}, skipping. TTL must be between 0 and {} seconds.",
                                    t, entry.key, 30 * 24 * 3600
                                );
                                continue;
                            }
                            // 直接设置 TTL，忽略错误
                            pipe.expire(&entry.key, t).ignore();
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
        // 验证缓存键，防止命令注入
        ensure_safe_key(key)?;

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
        // 验证缓存键和值，防止命令注入
        ensure_safe_key(key)?;

        // 验证值中不包含危险字符
        if value.contains('\r') || value.contains('\n') {
            return Err(CacheError::InvalidInput(
                "Redis value contains forbidden characters".to_string(),
            ));
        }

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
        // 验证缓存键，防止命令注入
        ensure_safe_key(key)?;

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
        // 验证缓存键，防止命令注入
        ensure_safe_key(key)?;

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
        // 验证缓存键，防止命令注入
        ensure_safe_key(key)?;

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

    // ============================================================================
    // Redis 原生操作扩展方法
    // ============================================================================

    /// 计数器操作：增加数值（带 TTL）
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `amount` - 增加的数量
    /// * `ttl` - 过期时间（秒）
    ///
    /// # 返回值
    ///
    /// 返回增加后的值
    #[instrument(skip(self), level = "debug")]
    pub async fn incr_by(&self, key: &str, amount: i64, ttl: Option<u64>) -> Result<i64> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: i64 = redis::cmd("INCRBY").arg(key).arg(amount).query_async(&mut conn).await?;

                // 设置 TTL
                if let Some(ttl_secs) = ttl {
                    redis::cmd("EXPIRE").arg(key).arg(ttl_secs).query_async::<()>(&mut conn).await?;
                }

                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: i64 = redis::cmd("INCRBY").arg(key).arg(amount).query_async(&mut conn).await?;

                if let Some(ttl_secs) = ttl {
                    redis::cmd("EXPIRE").arg(key).arg(ttl_secs).query_async::<()>(&mut conn).await?;
                }

                Ok(result)
            }
        }
    }

    /// 计数器操作：减少数值（带 TTL）
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `amount` - 减少的数量
    /// * `ttl` - 过期时间（秒）
    ///
    /// # 返回值
    ///
    /// 返回减少后的值
    #[instrument(skip(self), level = "debug")]
    pub async fn decr_by(&self, key: &str, amount: i64, ttl: Option<u64>) -> Result<i64> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: i64 = redis::cmd("DECRBY").arg(key).arg(amount).query_async(&mut conn).await?;

                if let Some(ttl_secs) = ttl {
                    redis::cmd("EXPIRE").arg(key).arg(ttl_secs).query_async::<()>(&mut conn).await?;
                }

                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: i64 = redis::cmd("DECRBY").arg(key).arg(amount).query_async(&mut conn).await?;

                if let Some(ttl_secs) = ttl {
                    redis::cmd("EXPIRE").arg(key).arg(ttl_secs).query_async::<()>(&mut conn).await?;
                }

                Ok(result)
            }
        }
    }

    /// 计数器操作：获取计数值
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回计数值，如果不存在返回 None
    #[instrument(skip(self), level = "debug")]
    pub async fn get_counter(&self, key: &str) -> Result<Option<i64>> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: Option<i64> = redis::cmd("GET").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: Option<i64> = redis::cmd("GET").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 有序集合操作：添加成员
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `score` - 分数
    /// * `member` - 成员
    /// * `ttl` - 过期时间（秒）
    ///
    /// # 返回值
    ///
    /// 返回添加的成员数量
    #[instrument(skip(self), level = "debug")]
    pub async fn zadd(&self, key: &str, score: f64, member: &str, ttl: Option<u64>) -> Result<u64> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: u64 = redis::cmd("ZADD").arg(key).arg(score).arg(member).query_async(&mut conn).await?;

                if let Some(ttl_secs) = ttl {
                    redis::cmd("EXPIRE").arg(key).arg(ttl_secs).query_async::<()>(&mut conn).await?;
                }

                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: u64 = redis::cmd("ZADD").arg(key).arg(score).arg(member).query_async(&mut conn).await?;

                if let Some(ttl_secs) = ttl {
                    redis::cmd("EXPIRE").arg(key).arg(ttl_secs).query_async::<()>(&mut conn).await?;
                }

                Ok(result)
            }
        }
    }

    /// 有序集合操作：按分数范围获取成员
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `min` - 最小分数
    /// * `max` - 最大分数
    /// * `with_scores` - 是否返回分数
    ///
    /// # 返回值
    ///
    /// 返回成员列表
    #[instrument(skip(self), level = "debug")]
    pub async fn zrange_by_score(
        &self,
        key: &str,
        min: f64,
        max: f64,
        with_scores: bool,
    ) -> Result<Vec<(String, f64)>> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                if with_scores {
                    let result: Vec<(String, f64)> = redis::cmd("ZRANGEBYSCORE")
                        .arg(key)
                        .arg(min)
                        .arg(max)
                        .arg("WITHSCORES")
                        .query_async(&mut conn)
                        .await?;
                    Ok(result)
                } else {
                    let result: Vec<String> = redis::cmd("ZRANGEBYSCORE")
                        .arg(key)
                        .arg(min)
                        .arg(max)
                        .query_async(&mut conn)
                        .await?;
                    Ok(result.into_iter().map(|s| (s, 0.0)).collect())
                }
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                if with_scores {
                    let result: Vec<(String, f64)> = redis::cmd("ZRANGEBYSCORE")
                        .arg(key)
                        .arg(min)
                        .arg(max)
                        .arg("WITHSCORES")
                        .query_async(&mut conn)
                        .await?;
                    Ok(result)
                } else {
                    let result: Vec<String> = redis::cmd("ZRANGEBYSCORE")
                        .arg(key)
                        .arg(min)
                        .arg(max)
                        .query_async(&mut conn)
                        .await?;
                    Ok(result.into_iter().map(|s| (s, 0.0)).collect())
                }
            }
        }
    }

    /// 有序集合操作：获取成员分数
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `member` - 成员
    ///
    /// # 返回值
    ///
    /// 返回成员分数，如果不存在返回 None
    #[instrument(skip(self), level = "debug")]
    pub async fn zscore(&self, key: &str, member: &str) -> Result<Option<f64>> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: Option<f64> = redis::cmd("ZSCORE").arg(key).arg(member).query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: Option<f64> = redis::cmd("ZSCORE").arg(key).arg(member).query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 有序集合操作：删除成员
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `members` - 要删除的成员列表
    ///
    /// # 返回值
    ///
    /// 返回删除的成员数量
    #[instrument(skip(self), level = "debug")]
    pub async fn zrem(&self, key: &str, members: &[&str]) -> Result<u64> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let mut cmd = redis::cmd("ZREM");
                cmd.arg(key);
                for member in members {
                    cmd.arg(member);
                }
                let result: u64 = cmd.query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let mut cmd = redis::cmd("ZREM");
                cmd.arg(key);
                for member in members {
                    cmd.arg(member);
                }
                let result: u64 = cmd.query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 有序集合操作：获取成员数量
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回成员数量
    #[instrument(skip(self), level = "debug")]
    pub async fn zcard(&self, key: &str) -> Result<u64> {
        ensure_safe_key(key)?;

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: u64 = redis::cmd("ZCARD").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: u64 = redis::cmd("ZCARD").arg(key).query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 键扫描：获取匹配的键
    ///
    /// # 参数
    ///
    /// * `pattern` - 匹配模式
    /// * `count` - 每次返回的键数量
    ///
    /// # 返回值
    ///
    /// 返回匹配的键列表
    #[instrument(skip(self), level = "debug")]
    pub async fn scan_keys(&self, pattern: &str, count: usize) -> Result<Vec<String>> {
        // 验证 SCAN 模式安全性
        #[cfg(feature = "l2-redis")]
        validate_scan_pattern(pattern)?;

        // 限制 count 参数到安全范围
        #[cfg(feature = "l2-redis")]
        let count = clamp_scan_count(count);

        #[cfg(not(feature = "l2-redis"))]
        let count = count; // 保持兼容

        // 添加 30 秒超时保护
        let timeout_duration = Duration::from_secs(30);

        let scan_operation = async {
            match self {
                L2Backend::Standalone { manager, .. } => {
                    let mut conn = manager.clone();
                    let mut all_keys = Vec::new();
                    let mut cursor = 0i64;

                    loop {
                        let (next_cursor, keys): (i64, Vec<String>) = redis::cmd("SCAN")
                            .arg(cursor)
                            .arg("MATCH")
                            .arg(pattern)
                            .arg("COUNT")
                            .arg(count)
                            .query_async(&mut conn)
                            .await?;

                        all_keys.extend(keys);

                        cursor = next_cursor;
                        if cursor == 0 {
                            break;
                        }
                    }

                    Ok(all_keys)
                }
                L2Backend::Cluster { client, .. } => {
                    let mut all_keys = Vec::new();
                    let mut cursor = 0i64;

                    loop {
                        let mut conn = client.get_async_connection().await?;
                        let (next_cursor, keys): (i64, Vec<String>) = redis::cmd("SCAN")
                            .arg(cursor)
                            .arg("MATCH")
                            .arg(pattern)
                            .arg("COUNT")
                            .arg(count)
                            .query_async(&mut conn)
                            .await?;

                        all_keys.extend(keys);

                        cursor = next_cursor;
                        if cursor == 0 {
                            break;
                        }
                    }

                    Ok(all_keys)
                }
            }
        };

        tokio::time::timeout(timeout_duration, scan_operation)
            .await
            .map_err(|_| CacheError::Timeout("SCAN operation timed out after 30 seconds".to_string()))?
    }

    /// 批量获取
    ///
    /// # 参数
    ///
    /// * `keys` - 键列表
    ///
    /// # 返回值
    ///
    /// 返回键值对映射
    #[instrument(skip(self), level = "debug")]
    pub async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let mut pipe = redis::pipe();
                for key in keys {
                    pipe.get(key);
                }
                let results: Vec<Option<Vec<u8>>> = pipe.query_async(&mut conn).await?;

                let mut map = HashMap::new();
                for (key, value) in keys.iter().zip(results.into_iter()) {
                    if let Some(v) = value {
                        map.insert(key.to_string(), v);
                    }
                }
                Ok(map)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let mut pipe = redis::pipe();
                for key in keys {
                    pipe.get(key);
                }
                let results: Vec<Option<Vec<u8>>> = pipe.query_async(&mut conn).await?;

                let mut map = HashMap::new();
                for (key, value) in keys.iter().zip(results.into_iter()) {
                    if let Some(v) = value {
                        map.insert(key.to_string(), v);
                    }
                }
                Ok(map)
            }
        }
    }

    /// 批量设置
    ///
    /// # 参数
    ///
    /// * `items` - 键值对映射
    /// * `ttl` - 过期时间（秒）
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug", fields(item_count = items.len()))]
    pub async fn set_many(&self, items: HashMap<&str, &[u8]>, ttl: Option<u64>) -> Result<()> {
        let ttl_secs = ttl.unwrap_or(3600);

        match self {
            L2Backend::Standalone { manager, version_cache, .. } => {
                let mut conn = manager.clone();
                let mut pipe = redis::pipe();

                for (key, value) in items {
                    let ttl_i64 = ttl_secs.try_into().unwrap_or(3600);
                    pipe.set(key, value).arg("EX").arg(ttl_i64).ignore();
                    pipe.incr(format!("{}:version", key), 1).ignore();
                    pipe.expire(format!("{}:version", key), ttl_i64).ignore();

                    // 更新版本缓存
                    let new_version = version_cache.get(key).map(|v| *v.value() + 1).unwrap_or(1);
                    version_cache.insert(key.to_string(), new_version);
                }

                pipe.query_async::<()>(&mut conn).await?;
            }
            L2Backend::Cluster { client, version_cache, .. } => {
                let mut conn = client.get_async_connection().await?;
                let mut pipe = redis::pipe();

                for (key, value) in items {
                    let ttl_i64 = ttl_secs.try_into().unwrap_or(3600);
                    pipe.set(key, value).arg("EX").arg(ttl_i64).ignore();
                    pipe.incr(format!("{}:version", key), 1).ignore();
                    pipe.expire(format!("{}:version", key), ttl_i64).ignore();

                    let new_version = version_cache.get(key).map(|v| *v.value() + 1).unwrap_or(1);
                    version_cache.insert(key.to_string(), new_version);
                }

                pipe.query_async::<()>(&mut conn).await?;
            }
        }

        Ok(())
    }

    /// 删除匹配模式的键
    ///
    /// # 参数
    ///
    /// * `pattern` - 匹配模式
    ///
    /// # 返回值
    ///
    /// 返回删除的键数量
    #[instrument(skip(self), level = "debug")]
    pub async fn del_pattern(&self, pattern: &str) -> Result<u64> {
        let keys = self.scan_keys(pattern, 1000).await?;

        if keys.is_empty() {
            return Ok(0);
        }

        let mut deleted_count = 0u64;
        let mut batch_count = 0usize;

        match self {
            L2Backend::Standalone { manager, version_cache, .. } => {
                let mut conn = manager.clone();
                let mut pipe = redis::pipe();

                for key in &keys {
                    pipe.del(key).ignore();
                    pipe.del(format!("{}:version", key)).ignore();
                    version_cache.remove(key);
                    deleted_count += 1;
                    batch_count += 1;

                    // 分批执行，每批 100 个键
                    if batch_count >= 100 {
                        pipe.query_async::<()>(&mut conn).await?;
                        pipe = redis::pipe();
                        batch_count = 0;
                    }
                }

                if batch_count > 0 {
                    pipe.query_async::<()>(&mut conn).await?;
                }
            }
            L2Backend::Cluster { client, version_cache, .. } => {
                let mut conn = client.get_async_connection().await?;
                let mut pipe = redis::pipe();

                for key in &keys {
                    pipe.del(key).ignore();
                    pipe.del(format!("{}:version", key)).ignore();
                    version_cache.remove(key);
                    deleted_count += 1;
                    batch_count += 1;

                    if batch_count >= 100 {
                        pipe.query_async::<()>(&mut conn).await?;
                        pipe = redis::pipe();
                        batch_count = 0;
                    }
                }

                if batch_count > 0 {
                    pipe.query_async::<()>(&mut conn).await?;
                }
            }
        }

        Ok(deleted_count)
    }

    /// 执行 Lua 脚本
    ///
    /// # 参数
    ///
    /// * `script` - Lua 脚本
    /// * `keys` - 键列表
    /// * `args` - 参数列表
    ///
    /// # 返回值
    ///
    /// 返回脚本执行结果
    #[instrument(skip(self), level = "debug")]
    pub async fn eval(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<String> {
        // 验证 Lua 脚本安全性
        #[cfg(feature = "l2-redis")]
        validate_lua_script(script, keys.len())?;

        let mut cmd = redis::cmd("EVAL");
        cmd.arg(script).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }

        for arg in args {
            cmd.arg(arg);
        }

        // 添加 30 秒超时保护
        let timeout_duration = Duration::from_secs(30);

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result = tokio::time::timeout(timeout_duration, async {
                    cmd.query_async::<String>(&mut conn).await
                })
                .await
                .map_err(|_| CacheError::Timeout("Lua script execution timed out after 30 seconds".to_string()))??;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result = tokio::time::timeout(timeout_duration, async {
                    cmd.query_async::<String>(&mut conn).await
                })
                .await
                .map_err(|_| CacheError::Timeout("Lua script execution timed out after 30 seconds".to_string()))??;
                Ok(result)
            }
        }
    }

    /// 加载 Lua 脚本
    ///
    /// # 参数
    ///
    /// * `script` - Lua 脚本
    ///
    /// # 返回值
    ///
    /// 返回脚本的 SHA
    #[instrument(skip(self), level = "debug")]
    pub async fn script_load(&self, script: &str) -> Result<String> {
        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: String = redis::cmd("SCRIPT").arg("LOAD").arg(script).query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: String = redis::cmd("SCRIPT").arg("LOAD").arg(script).query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 使用 SHA 执行 Lua 脚本
    ///
    /// # 参数
    ///
    /// * `sha` - 脚本 SHA
    /// * `keys` - 键列表
    /// * `args` - 参数列表
    ///
    /// # 返回值
    ///
    /// 返回脚本执行结果
    #[instrument(skip(self), level = "debug")]
    pub async fn evalsha(&self, sha: &str, keys: &[&str], args: &[&str]) -> Result<String> {
        // 验证键数量（即使使用 SHA，也要防止键数量过多）
        #[cfg(feature = "l2-redis")]
        if keys.len() > 100 {
            return Err(CacheError::InvalidInput(format!(
                "Lua script exceeds maximum key count of 100 (got {} keys)",
                keys.len()
            )));
        }

        let mut cmd = redis::cmd("EVALSHA");
        cmd.arg(sha).arg(keys.len());

        for key in keys {
            cmd.arg(key);
        }

        for arg in args {
            cmd.arg(arg);
        }

        // 添加 30 秒超时保护
        let timeout_duration = Duration::from_secs(30);

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result = tokio::time::timeout(timeout_duration, async {
                    cmd.query_async::<String>(&mut conn).await
                })
                .await
                .map_err(|_| CacheError::Timeout("Lua script execution timed out after 30 seconds".to_string()))??;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result = tokio::time::timeout(timeout_duration, async {
                    cmd.query_async::<String>(&mut conn).await
                })
                .await
                .map_err(|_| CacheError::Timeout("Lua script execution timed out after 30 seconds".to_string()))??;
                Ok(result)
            }
        }
    }
}

// ============================================================================
// RedisNativeOps Trait Implementation
// ============================================================================

#[async_trait]
impl RedisNativeOps for L2Backend {
    /// 设置单个键值
    async fn set(&self, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()> {
        self.set_bytes(key, value.to_vec(), ttl).await
    }

    /// 获取字节数组缓存值
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get_bytes(key).await
    }

    /// 计数器操作：增加数值
    async fn increment(&self, key: &str, amount: i64, ttl: Option<u64>) -> Result<i64> {
        self.incr_by(key, amount, ttl).await
    }

    /// 计数器操作：减少数值
    async fn decrement(&self, key: &str, amount: i64, ttl: Option<u64>) -> Result<i64> {
        self.decr_by(key, amount, ttl).await
    }

    /// 计数器操作：获取计数值
    async fn get_counter(&self, key: &str) -> Result<Option<i64>> {
        self.get_counter(key).await
    }

    /// 有序集合操作：添加成员
    async fn zadd(
        &self,
        key: &str,
        score: f64,
        member: &str,
        ttl: Option<u64>,
    ) -> Result<u64> {
        self.zadd(key, score, member, ttl).await
    }

    /// 有序集合操作：按分数范围获取成员
    async fn zrange_by_score(
        &self,
        key: &str,
        min: f64,
        max: f64,
        with_scores: bool,
    ) -> Result<Vec<ZSetMember>> {
        let raw_result: Vec<(String, f64)> = self.zrange_by_score(key, min, max, with_scores).await?;
        Ok(raw_result
            .into_iter()
            .map(|(member, score)| ZSetMember { member, score })
            .collect())
    }

    /// 有序集合操作：获取成员分数
    async fn zscore(&self, key: &str, member: &str) -> Result<Option<f64>> {
        self.zscore(key, member).await
    }

    /// 有序集合操作：删除成员
    async fn zrem(&self, key: &str, members: &[&str]) -> Result<u64> {
        self.zrem(key, members).await
    }

    /// 有序集合操作：获取成员数量
    async fn zcard(&self, key: &str) -> Result<u64> {
        self.zcard(key).await
    }

    /// 键扫描：获取匹配的键
    async fn scan_keys(&self, pattern: &str, count: usize) -> Result<Vec<String>> {
        self.scan_keys(pattern, count).await
    }

    /// 键扫描迭代器
    fn scan_iter(&self, pattern: &str) -> ScanKeyIterator {
        ScanKeyIterator::new(Arc::new(self.clone()), pattern)
    }

    /// 执行 Lua 脚本（只读）
    async fn eval_readonly(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<String> {
        // 尝试只读执行
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(script).arg(keys.len());
        for key in keys {
            cmd.arg(key);
        }
        for arg in args {
            cmd.arg(arg);
        }

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let result: String = cmd.query_async(&mut conn).await?;
                Ok(result)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let result: String = cmd.query_async(&mut conn).await?;
                Ok(result)
            }
        }
    }

    /// 执行 Lua 脚本（写操作）
    async fn eval_write(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<String> {
        // 写操作与只读相同，Redis 不区分
        self.eval_readonly(script, keys, args).await
    }

    /// 使用缓存的脚本 SHA 执行
    async fn evalsha(&self, sha1: &str, keys: &[&str], args: &[&str]) -> Result<String> {
        self.evalsha(sha1, keys, args).await
    }

    /// 加载 Lua 脚本到缓存
    async fn script_load(&self, script: &str) -> Result<String> {
        self.script_load(script).await
    }

    /// 检查脚本是否在缓存中
    async fn script_exists(&self, sha1: &[&str]) -> Result<Vec<bool>> {
        let mut cmd = redis::cmd("SCRIPT");
        cmd.arg("EXISTS");
        for sha in sha1 {
            cmd.arg(sha);
        }

        match self {
            L2Backend::Standalone { manager, .. } => {
                let mut conn = manager.clone();
                let results: Vec<bool> = cmd.query_async(&mut conn).await?;
                Ok(results)
            }
            L2Backend::Cluster { client, .. } => {
                let mut conn = client.get_async_connection().await?;
                let results: Vec<bool> = cmd.query_async(&mut conn).await?;
                Ok(results)
            }
        }
    }

    /// 批量获取
    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        self.get_many(keys).await
    }

    /// 批量设置
    async fn set_many(&self, items: HashMap<&str, &[u8]>, ttl: Option<u64>) -> Result<()> {
        self.set_many(items, ttl).await
    }

    /// 删除匹配模式的键
    async fn del_pattern(&self, pattern: &str) -> Result<u64> {
        self.del_pattern(pattern).await
    }
}
