//! 数据库回源加载器
//!
//! 提供缓存未命中时自动从数据库加载数据的功能

use crate::error::{CacheError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

/// 数据库加载器trait
/// 定义从数据库加载数据的接口
#[async_trait]
pub trait DbLoader: Send + Sync + std::fmt::Debug {
    /// 根据键从数据库加载数据
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回加载的数据，如果数据不存在则返回None
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// 批量加载数据
    ///
    /// # 参数
    ///
    /// * `keys` - 缓存键列表
    ///
    /// # 返回值
    ///
    /// 返回(key, value)对的列表
    async fn load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>>;

    /// 检查数据库连接状态
    fn is_healthy(&self) -> bool;
}

/// 数据库回源管理器
/// 管理数据库加载器并提供回源逻辑
pub struct DbFallbackManager {
    /// 数据库加载器
    loader: Arc<dyn DbLoader>,
    /// 是否启用回源功能
    enabled: bool,
    /// 回源超时时间（毫秒）
    timeout_ms: u64,
    /// 最大重试次数
    max_retries: u32,
}

impl std::fmt::Debug for DbFallbackManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbFallbackManager")
            .field("enabled", &self.enabled)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_retries", &self.max_retries)
            .field("loader_healthy", &self.loader.is_healthy())
            .finish()
    }
}

impl DbFallbackManager {
    /// 创建新的数据库回源管理器
    ///
    /// # 参数
    ///
    /// * `loader` - 数据库加载器
    /// * `enabled` - 是否启用回源功能
    /// * `timeout_ms` - 回源超时时间（毫秒）
    /// * `max_retries` - 最大重试次数
    pub fn new(
        loader: Arc<dyn DbLoader>,
        enabled: bool,
        timeout_ms: u64,
        max_retries: u32,
    ) -> Self {
        Self {
            loader,
            enabled,
            timeout_ms,
            max_retries,
        }
    }

    /// 从数据库回源加载数据
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回从数据库加载的数据，如果加载失败则返回None
    #[instrument(skip(self), level = "info")]
    pub async fn fallback_load(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if !self.enabled {
            debug!("Database fallback is disabled");
            return Ok(None);
        }

        if !self.loader.is_healthy() {
            error!("Database loader is not healthy, skipping fallback");
            return Ok(None);
        }

        info!("Attempting database fallback for key: {}", key);

        // 尝试加载数据，支持重试机制
        let mut last_error = None;
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                debug!("Retry attempt {} for key: {}", attempt, key);
            }

            match self.try_load_with_timeout(key).await {
                Ok(Some(data)) => {
                    info!("Successfully loaded data from database for key: {}", key);
                    return Ok(Some(data));
                }
                Ok(None) => {
                    debug!("No data found in database for key: {}", key);
                    return Ok(None);
                }
                Err(e) => {
                    error!("Failed to load data from database for key {}: {}", key, e);
                    last_error = Some(e);
                    if attempt < self.max_retries {
                        // 指数退避重试
                        let backoff_ms = 100 * (2_u64.pow(attempt));
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }

        error!("All retry attempts failed for key: {}", key);
        Err(last_error.unwrap_or_else(|| {
            CacheError::DatabaseError("All fallback attempts failed".to_string())
        }))
    }

    /// 批量回源加载数据
    ///
    /// # 参数
    ///
    /// * `keys` - 缓存键列表
    ///
    /// # 返回值
    ///
    /// 返回(key, value)对的列表
    #[instrument(skip(self), level = "info")]
    pub async fn fallback_load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>> {
        if !self.enabled {
            debug!("Database fallback is disabled");
            return Ok(Vec::new());
        }

        if !self.loader.is_healthy() {
            error!("Database loader is not healthy, skipping batch fallback");
            return Ok(Vec::new());
        }

        info!("Attempting batch database fallback for {} keys", keys.len());

        // 使用超时机制
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(self.timeout_ms),
            self.loader.load_batch(keys.clone()),
        )
        .await
        {
            Ok(Ok(results)) => {
                info!("Successfully loaded {} items from database", results.len());
                Ok(results)
            }
            Ok(Err(e)) => {
                error!("Failed to batch load from database: {}", e);
                Err(e)
            }
            Err(_) => {
                error!(
                    "Batch database fallback timed out after {}ms",
                    self.timeout_ms
                );
                Err(CacheError::Timeout(format!(
                    "Batch fallback timeout after {}ms",
                    self.timeout_ms
                )))
            }
        }
    }

    /// 使用超时机制尝试加载数据
    async fn try_load_with_timeout(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(self.timeout_ms),
            self.loader.load(key),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                debug!(
                    "Database load timed out after {}ms for key: {}",
                    self.timeout_ms, key
                );
                Ok(None)
            }
        }
    }

    /// 检查回源功能是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// 示例数据库加载器实现（基于SQL）
#[derive(Debug)]
pub struct SqlDbLoader {
    /// 数据库连接池
    pool: Arc<dyn DbConnectionPool>,
    /// 查询语句模板
    query_template: String,
}

#[async_trait]
impl DbLoader for SqlDbLoader {
    #[instrument(skip(self), level = "debug")]
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let query = self.query_template.replace("{key}", key);
        debug!("Executing database query: {}", query);

        // 执行查询并返回结果
        self.pool.execute_query(&query).await
    }

    #[instrument(skip(self), level = "debug")]
    async fn load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // 构建IN查询
        let key_list = keys
            .iter()
            .map(|k| format!("'{}'", k))
            .collect::<Vec<_>>()
            .join(",");

        let query = format!(
            "SELECT cache_key, cache_value FROM cache_table WHERE cache_key IN ({})",
            key_list
        );

        debug!("Executing batch database query for {} keys", keys.len());
        self.pool.execute_batch_query(&query).await
    }

    fn is_healthy(&self) -> bool {
        self.pool.is_healthy()
    }
}

/// 数据库连接池trait
#[async_trait]
pub trait DbConnectionPool: Send + Sync + std::fmt::Debug {
    /// 执行查询
    async fn execute_query(&self, query: &str) -> Result<Option<Vec<u8>>>;

    /// 执行批量查询
    async fn execute_batch_query(&self, query: &str) -> Result<Vec<(String, Vec<u8>)>>;

    /// 检查连接池健康状态
    fn is_healthy(&self) -> bool;
}

/// 配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbFallbackConfig {
    /// 是否启用回源功能
    pub enabled: bool,
    /// 回源超时时间（毫秒）
    pub timeout_ms: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 数据库连接字符串
    pub connection_string: String,
    /// 查询语句模板
    pub query_template: String,
}

impl Default for DbFallbackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_ms: 5000,
            max_retries: 3,
            connection_string: String::new(),
            query_template: "SELECT cache_value FROM cache_table WHERE cache_key = '{key}'"
                .to_string(),
        }
    }
}
