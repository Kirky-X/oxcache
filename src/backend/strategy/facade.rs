//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! L2 后端门面实现
//!
//! 使用策略模式封装不同的 Redis 部署模式，提供统一的 API。

use crate::backend::client::redis::{DefaultRedisProvider, RedisProvider};
use crate::backend::strategy::standalone::StandaloneStrategy;
use crate::backend::strategy::traits::{HealthStatus, L2BackendStrategy, ScanResult};
use crate::config::legacy_config::{L2Config, RedisMode};
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, instrument};

/// L2 后端门面
///
/// 使用策略模式封装不同的 Redis 部署模式，
/// 提供统一的 API 接口。
#[derive(Clone)]
pub struct L2BackendFacade {
    /// 策略实现
    strategy: Arc<dyn L2BackendStrategy>,
    /// Redis 模式
    mode: RedisMode,
    /// 版本缓存
    version_cache: Arc<dashmap::DashMap<String, u64>>,
}

impl std::fmt::Debug for L2BackendFacade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("L2BackendFacade")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl L2BackendFacade {
    /// 创建新的 L2 后端门面实例
    ///
    /// # 参数
    /// * `config` - L2 缓存配置
    ///
    /// # 返回值
    /// * 新的 L2BackendFacade 实例或错误
    #[instrument(skip(config), level = "info", fields(mode = ?config.mode))]
    pub async fn new(config: &L2Config) -> Result<Self> {
        Self::new_with_provider(config, Arc::new(DefaultRedisProvider)).await
    }

    /// 使用指定的 Redis 提供者创建新的 L2 后端门面实例
    ///
    /// # 参数
    /// * `config` - L2 缓存配置
    /// * `provider` - Redis 提供者
    ///
    /// # 返回值
    /// * 新的 L2BackendFacade 实例或错误
    #[instrument(skip(config, provider), level = "info", fields(mode = ?config.mode))]
    pub async fn new_with_provider(
        config: &L2Config,
        provider: Arc<dyn RedisProvider>,
    ) -> Result<Self> {
        debug!("Initializing L2BackendFacade with mode: {:?}", config.mode);

        let strategy: Arc<dyn L2BackendStrategy> = match config.mode {
            RedisMode::Standalone => {
                let (client, manager) = provider.get_standalone_client(config).await?;
                // 暂时不支持独立从库配置，统一使用主库
                // 后续可以通过配置扩展支持
                Arc::new(StandaloneStrategy::new(config, client, manager, None))
            }
            RedisMode::Sentinel => {
                // 使用哨兵配置获取客户端（包含主库和从库）
                let (client, manager, read_manager) = provider.get_sentinel_client(config).await?;
                Arc::new(StandaloneStrategy::new(
                    config,
                    client,
                    manager,
                    read_manager,
                ))
            }
            RedisMode::Cluster => {
                // 对于集群模式，暂时使用 standalone 策略
                // 完整集群支持需要后续实现 ClusterStrategy
                let (client, manager) = provider.get_standalone_client(config).await?;
                Arc::new(StandaloneStrategy::new(config, client, manager, None))
            }
        };

        Ok(Self {
            strategy,
            mode: config.mode,
            version_cache: Arc::new(dashmap::DashMap::new()),
        })
    }

    /// 获取策略名称
    pub fn strategy_name(&self) -> &str {
        self.strategy.name()
    }

    /// 检查连接状态
    pub fn is_connected(&self) -> bool {
        self.strategy.is_connected()
    }

    /// 获取版本缓存
    pub fn version_cache(&self) -> &Arc<dashmap::DashMap<String, u64>> {
        &self.version_cache
    }

    /// 获取 TTL（支持版本缓存）
    async fn get_with_version(&self, key: &str) -> Result<Option<(Vec<u8>, u64)>> {
        // 首先尝试从版本缓存获取
        if let Some(version) = self.version_cache.get(key) {
            if let Ok(value) = self.strategy.get(key).await {
                return Ok(value.map(|v| (v, *version)));
            }
        }

        // 从 Redis 获取
        self.strategy.get_with_version(key).await
    }
}

#[async_trait]
impl L2BackendStrategy for L2BackendFacade {
    fn name(&self) -> &str {
        "facade"
    }

    fn is_connected(&self) -> bool {
        self.strategy.is_connected()
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.strategy.get(key).await
    }

    async fn set(&self, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()> {
        self.strategy.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        self.strategy.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.strategy.exists(key).await
    }

    async fn expire(&self, key: &str, ttl: u64) -> Result<bool> {
        self.strategy.expire(key, ttl).await
    }

    async fn ttl(&self, key: &str) -> Result<Option<i64>> {
        self.strategy.ttl(key).await
    }

    async fn get_with_version(&self, key: &str) -> Result<Option<(Vec<u8>, u64)>> {
        self.get_with_version(key).await
    }

    async fn compare_and_set(
        &self,
        key: &str,
        value: &[u8],
        expected_version: u64,
        new_version: u64,
        ttl: Option<u64>,
    ) -> Result<bool> {
        self.strategy
            .compare_and_set(key, value, expected_version, new_version, ttl)
            .await
    }

    async fn lock(&self, key: &str, ttl: u64) -> Result<Option<String>> {
        self.strategy.lock(key, ttl).await
    }

    async fn unlock(&self, key: &str, value: &str) -> Result<bool> {
        self.strategy.unlock(key, value).await
    }

    async fn mget(
        &self,
        keys: &[&str],
    ) -> std::result::Result<std::collections::HashMap<String, Vec<u8>>, CacheError> {
        self.strategy.mget(keys).await
    }

    async fn mset(&self, items: &[(&str, &[u8])], ttl: Option<u64>) -> Result<()> {
        self.strategy.mset(items, ttl).await
    }

    async fn scan(&self, pattern: &str, count: usize, cursor: u64) -> Result<ScanResult> {
        self.strategy.scan(pattern, count, cursor).await
    }

    async fn scan_keys(&self, pattern: &str, limit: usize) -> Result<Vec<String>> {
        self.strategy.scan_keys(pattern, limit).await
    }

    async fn ping(&self) -> Result<()> {
        self.strategy.ping().await
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        self.strategy.health_check().await
    }

    fn command_timeout(&self) -> std::time::Duration {
        self.strategy.command_timeout()
    }

    async fn close(&self) -> Result<()> {
        self.strategy.close().await
    }
}
