//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了双层缓存客户端的实现，结合L1和L2缓存。

use super::{db_loader::DbFallbackManager, l2::L2Client, CacheOps};
use crate::backend::l1::L1Backend;
use crate::bloom_filter::{BloomFilterManager, BloomFilterOptions, BloomFilterShared};
use crate::config::TwoLevelConfig;
use crate::error::Result;
use crate::metrics::GLOBAL_METRICS;
use crate::recovery::{
    health::{HealthChecker, HealthState},
    wal::{Operation, WalEntry, WalManager},
};
use crate::serialization::{Serializer, SerializerEnum};
use crate::sync::{
    batch_writer::BatchWriter,
    common::BatchWriterConfig,
    invalidation::{InvalidationPublisher, InvalidationSubscriber},
    promotion::PromotionManager,
    warmup::WarmupManager,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, instrument, warn};

/// 双层缓存客户端实现
///
/// 结合L1（内存）和L2（Redis）缓存，提供高性能和高可用性的缓存解决方案
/// 支持数据库回源功能，当缓存未命中时自动从数据库加载数据
pub struct TwoLevelClient {
    /// 服务名称
    service_name: String,
    /// 双层缓存配置
    config: TwoLevelConfig,
    /// L1缓存后端
    l1: Option<Arc<L1Backend>>,
    /// L2缓存客户端
    l2: Option<Arc<L2Client>>,
    /// 序列化器
    serializer: SerializerEnum,
    /// 健康状态
    health_state: Arc<RwLock<HealthState>>,
    /// WAL管理器
    wal: Arc<WalManager>,
    /// 推广管理器
    promotion_mgr: Option<Arc<PromotionManager>>,
    /// 批量写入器
    batch_writer: Option<Arc<BatchWriter>>,
    /// 失效发布器
    publisher: Option<Arc<InvalidationPublisher>>,
    /// 数据库回源管理器
    db_fallback_mgr: Option<Arc<DbFallbackManager>>,
    /// 布隆过滤器
    bloom_filter: Option<BloomFilterShared>,
    /// 布隆过滤器管理器
    bloom_filter_mgr: Option<Arc<BloomFilterManager>>,
    /// 缓存预热管理器
    warmup_mgr: Option<Arc<WarmupManager>>,
    /// 健康检查器任务句柄
    #[allow(dead_code)]
    health_checker_handle: Option<JoinHandle<()>>,
    /// 批处理写入器任务句柄
    #[allow(dead_code)]
    batch_writer_handle: Option<JoinHandle<()>>,
}

impl Clone for TwoLevelClient {
    fn clone(&self) -> Self {
        Self {
            service_name: self.service_name.clone(),
            config: self.config.clone(),
            l1: self.l1.clone(),
            l2: self.l2.clone(),
            serializer: self.serializer.clone(),
            health_state: self.health_state.clone(),
            wal: self.wal.clone(),
            promotion_mgr: self.promotion_mgr.clone(),
            batch_writer: self.batch_writer.clone(),
            publisher: self.publisher.clone(),
            db_fallback_mgr: self.db_fallback_mgr.clone(),
            bloom_filter: self.bloom_filter.clone(),
            bloom_filter_mgr: self.bloom_filter_mgr.clone(),
            warmup_mgr: self.warmup_mgr.clone(),
            health_checker_handle: None,
            batch_writer_handle: None,
        }
    }
}

impl TwoLevelClient {
    /// 创建新的双层缓存客户端
    ///
    /// # 参数
    ///
    /// * `service_name` - 服务名称
    /// * `config` - 双层缓存配置
    /// * `l1` - L1缓存后端
    /// * `l2` - L2缓存后端
    /// * `serializer` - 序列化器
    ///
    /// # 返回值
    ///
    /// 返回新的双层缓存客户端实例或错误
    #[allow(clippy::too_many_arguments)]
    #[instrument(
        skip(config, l1, l2_backend, serializer),
        level = "info",
        name = "init_two_level_client"
    )]
    pub async fn new(
        service_name: String,
        config: TwoLevelConfig,
        l1: Arc<L1Backend>,
        l2_backend: Arc<crate::backend::l2::L2Backend>,
        serializer: SerializerEnum,
    ) -> Result<Self> {
        let health_state = Arc::new(RwLock::new(HealthState::Healthy));
        let wal = Arc::new(WalManager::new(&service_name)?);

        // 创建L2客户端
        let l2 = Arc::new(
            L2Client::new(service_name.clone(), l2_backend.clone(), serializer.clone()).await?,
        );

        // 启动健康检查器 - 使用L2Backend进行健康检查
        let command_timeout_ms = l2_backend.command_timeout_ms();
        let checker = HealthChecker::new(
            l2_backend.clone(),
            health_state.clone(),
            wal.clone(),
            service_name.clone(),
            command_timeout_ms,
        );
        let health_checker_handle = tokio::spawn(async move { checker.start().await });

        // 确定失效频道名称
        let channel_name = Self::resolve_channel_name(&service_name, &config);

        // 启动失效订阅器 - 使用L2Backend的原始客户端
        let sub = InvalidationSubscriber::new(
            l2_backend.get_raw_client()?,
            l1.clone(),
            channel_name.clone(),
            health_state.clone(),
        );
        sub.start().await?;

        let publisher = Arc::new(InvalidationPublisher::new(
            l2_backend
                .get_raw_client()?
                .get_connection_manager()
                .await?,
            channel_name,
        ));

        let promotion_mgr = if config.promote_on_hit {
            Some(Arc::new(PromotionManager::new(
                l1.clone(),
                l2_backend.clone(),
                health_state.clone(),
            )))
        } else {
            None
        };

        let (batch_writer, batch_writer_handle) = if config.enable_batch_write {
            let batch_config = BatchWriterConfig {
                max_batch_size: config.batch_size,
                flush_interval_ms: config.batch_interval_ms,
            };
            let bw = Arc::new(BatchWriter::new(
                service_name.clone(),
                l2_backend.clone(),
                batch_config,
            ));
            let bw_clone = bw.clone();
            let handle = tokio::spawn(async move { bw_clone.start().await });
            (Some(bw), Some(handle))
        } else {
            (None, None)
        };

        let (bloom_filter, bloom_filter_mgr) = if let Some(bloom_config) = &config.bloom_filter {
            let options = BloomFilterOptions::new(
                bloom_config.name.clone(),
                bloom_config.expected_elements,
                bloom_config.false_positive_rate,
            );
            let mgr = Arc::new(BloomFilterManager::new());
            let filter = mgr.get_or_create(options).await;
            (Some(filter), Some(mgr))
        } else {
            (None, None)
        };

        let warmup_mgr = config.warmup.as_ref().map(|warmup_config| {
            Arc::new(WarmupManager::new(
                service_name.clone(),
                warmup_config.clone(),
            ))
        });

        Ok(Self {
            service_name: service_name.to_string(),
            config,
            l1: Some(l1),
            l2: Some(l2),
            serializer,
            health_state,
            wal,
            promotion_mgr,
            batch_writer,
            publisher: Some(publisher),
            db_fallback_mgr: None,
            bloom_filter,
            bloom_filter_mgr,
            warmup_mgr,
            health_checker_handle: Some(health_checker_handle),
            batch_writer_handle,
        })
    }

    /// 处理L2故障
    #[instrument(skip(self), level = "warn")]
    async fn handle_l2_failure(&self) {
        warn!("L2 failure detected for service: {}", self.service_name);

        let mut state_guard = self.health_state.write().await;
        let current_state = *state_guard;

        match current_state {
            HealthState::Healthy => {
                warn!(
                    "Service {} transitioning from Healthy to Degraded",
                    self.service_name
                );
                *state_guard = HealthState::Degraded {
                    since: std::time::Instant::now(),
                    failure_count: 1,
                };
                crate::metrics::GLOBAL_METRICS.set_health(&self.service_name, 0);
            }
            HealthState::Degraded {
                since,
                failure_count,
            } => {
                let new_failure_count = failure_count + 1;
                warn!(
                    "Service {} remains Degraded, failure count increased: {} -> {}",
                    self.service_name, failure_count, new_failure_count
                );
                *state_guard = HealthState::Degraded {
                    since,
                    failure_count: new_failure_count,
                };
            }
            HealthState::Recovering {
                since: _,
                success_count: _,
            } => {
                warn!(
                    "Service {} recovery failed, transitioning back to Degraded from Recovering",
                    self.service_name
                );
                *state_guard = HealthState::Degraded {
                    since: std::time::Instant::now(),
                    failure_count: 1,
                };
                crate::metrics::GLOBAL_METRICS.set_health(&self.service_name, 0);
            }
        }

        info!(
            "Service {} degradation strategy applied, current state: {:?}",
            self.service_name, *state_guard
        );
    }

    /// 获取当前健康状态
    pub async fn get_health_state(&self) -> HealthState {
        *self.health_state.read().await
    }

    /// 解决失效频道名称
    ///
    /// # 参数
    ///
    /// * `service_name` - 服务名称
    /// * `config` - 双层缓存配置
    ///
    /// # 返回值
    ///
    /// 返回解析后的频道名称
    fn resolve_channel_name(service_name: &str, config: &TwoLevelConfig) -> String {
        use crate::config::InvalidationChannelConfig;
        match &config.invalidation_channel {
            Some(InvalidationChannelConfig::Custom(name)) => name.clone(),
            Some(InvalidationChannelConfig::Structured {
                prefix,
                use_service_name,
            }) => {
                let prefix = prefix.as_deref().unwrap_or("cache:invalidate");
                if *use_service_name {
                    format!("{}:{}", prefix, service_name)
                } else {
                    prefix.to_string()
                }
            }
            None => format!("cache:invalidate:{}", service_name),
        }
    }

    #[allow(dead_code)]
    #[instrument(skip(self), level = "debug")]
    async fn get_from_l1(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(l1) = &self.l1 {
            if let Some((val, _ver)) = l1.get_with_metadata(key).await? {
                debug!("L1 hit for key: {}", key);
                return Ok(Some(val));
            }
        }
        debug!("L1 miss for key: {}", key);
        Ok(None)
    }

    #[allow(dead_code)]
    #[instrument(skip(self), level = "debug")]
    async fn get_from_l2(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(l2) = &self.l2 {
            if let Some(val) = l2.get_bytes(key).await? {
                debug!("L2 hit for key: {}", key);
                return Ok(Some(val));
            }
        }
        debug!("L2 miss for key: {}", key);
        Ok(None)
    }

    /// 缓存预热
    ///
    /// 批量从数据源加载数据并写入缓存
    ///
    /// # 参数
    ///
    /// * `keys` - 需要预热的键列表
    /// * `loader` - 数据加载函数，接收键列表，返回 (key, value) 对列表
    /// * `ttl` - 缓存过期时间
    #[instrument(skip(self, loader), level = "info", fields(key_count = keys.len()))]
    pub async fn warmup<T, F, Fut>(
        &self,
        keys: Vec<String>,
        loader: F,
        ttl: Option<u64>,
    ) -> Result<()>
    where
        T: serde::Serialize + Send + Sync,
        F: Fn(Vec<String>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<Vec<(String, T)>>> + Send,
    {
        if keys.is_empty() {
            return Ok(());
        }

        // 1. 调用加载函数获取数据
        let data = loader(keys).await?;

        // 2. 批量写入缓存
        // 由于我们需要序列化，且 set 接口是单个的，我们循环调用 set
        // 对于大量数据，可以使用 pipeline 优化，但 CacheOps 没有 batch_set 接口
        // 如果开启了 batch_writer，set 会自动批处理
        for (key, value) in data {
            // 使用 set_bytes 避免重复序列化逻辑（虽然 set 也会序列化）
            // 这里我们直接用 self.set
            self.set(&key, &value, ttl).await?;
        }

        Ok(())
    }

    /// 异步执行预热
    ///
    /// 使用配置的预热管理器执行预热
    pub async fn run_warmup(&self) -> Result<()> {
        if let Some(warmup_mgr) = &self.warmup_mgr {
            let client: Arc<Self> = Arc::new(self.clone());
            let result = warmup_mgr
                .run_warmup(move |keys: Vec<String>| {
                    let client = Arc::clone(&client);
                    Box::pin(async move {
                        let mut result = HashMap::new();
                        for key in keys {
                            match client.get_bytes(&key).await {
                                Ok(Some(value)) => {
                                    result.insert(key, value);
                                }
                                Ok(None) => {
                                    debug!("Warmup: key not found in L2: {}", key);
                                }
                                Err(e) => {
                                    warn!("Warmup: failed to get key {} from L2: {}", key, e);
                                }
                            }
                        }
                        Ok(result)
                    })
                })
                .await?;
            if result.success {
                info!(
                    "Cache warmup completed successfully, loaded {} items",
                    result.loaded
                );
            } else {
                warn!(
                    "Cache warmup completed with some failures, loaded: {}, failed: {}",
                    result.loaded, result.failed
                );
            }
        }
        Ok(())
    }

    /// 获取预热管理器
    pub fn warmup_manager(&self) -> Option<&Arc<WarmupManager>> {
        self.warmup_mgr.as_ref()
    }

    /// 优雅关闭客户端
    ///
    /// 停止所有后台任务，释放资源
    #[instrument(skip(self), level = "info", fields(service = %self.service_name))]
    pub async fn shutdown(&self) -> Result<()> {
        info!("正在关闭TwoLevelClient...");

        // 停止健康检查器
        if let Some(handle) = &self.health_checker_handle {
            info!("停止健康检查器");
            handle.abort();
        }

        // 停止批处理写入器
        if let Some(handle) = &self.batch_writer_handle {
            info!("停止批处理写入器");
            handle.abort();
        }

        // 关闭L1缓存连接
        if let Some(_l1) = &self.l1 {
            info!("关闭L1缓存");
            // L1缓存通常是内存缓存，不需要特殊关闭操作
        }

        // 关闭L2缓存连接
        if let Some(l2) = &self.l2 {
            info!("关闭L2缓存");
            l2.shutdown().await?;
        }

        // WAL日志在replay_all中已经处理，这里不需要额外操作
        info!("WAL日志已处理");

        info!("TwoLevelClient已关闭");
        Ok(())
    }
}

#[async_trait]
impl CacheOps for TwoLevelClient {
    /// 获取序列化器
    fn serializer(&self) -> &SerializerEnum {
        &self.serializer
    }

    /// 将 trait object 转换为 Any
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// 将 `Arc<Trait>` 转换为 `Arc<dyn Any>`
    fn into_any_arc(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }

    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn lock(&self, key: &str, value: &str, ttl: u64) -> Result<bool> {
        debug!(
            "TwoLevelClient lock called: key={}, value={}, ttl={}",
            key, value, ttl
        );
        if let Some(l2) = &self.l2 {
            debug!("L2 backend available, attempting lock acquisition");
            // 使用L2客户端的lock方法，它会处理健康状态检查
            let result = l2.lock(key, value, ttl).await;
            debug!("L2 lock result: {:?}", result);
            return result;
        }
        // 如果 L2 不可用或未配置，无法获取分布式锁
        warn!("Cannot acquire lock, L2 unavailable or not configured");
        Ok(false)
    }

    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn unlock(&self, key: &str, value: &str) -> Result<bool> {
        if let Some(l2) = &self.l2 {
            // 使用L2客户端的unlock方法，它会处理健康状态检查
            return l2.unlock(key, value).await;
        }
        warn!("Cannot release lock, L2 unavailable or not configured");
        Ok(false)
    }

    /// 获取缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // Two-level mode
        if let (Some(l1), Some(l2)) = (&self.l1, &self.l2) {
            // 布隆过滤器检查 - 防止缓存穿透
            if let Some(bloom_filter) = &self.bloom_filter {
                let key_bytes = key.as_bytes();
                if !bloom_filter.contains(key_bytes).await {
                    GLOBAL_METRICS.record_request(&self.service_name, "BloomFilter", "get", "miss");
                    return Ok(None);
                }
                GLOBAL_METRICS.record_request(&self.service_name, "BloomFilter", "get", "hit");
            }

            GLOBAL_METRICS.record_request(&self.service_name, "L1", "get", "attempt");

            let start = std::time::Instant::now();
            // 1. 尝试L1
            if let Some((bytes, _)) = l1.get_with_metadata(key).await? {
                let duration = start.elapsed().as_secs_f64();
                GLOBAL_METRICS.record_duration(&self.service_name, "L1", "get", duration);
                GLOBAL_METRICS.record_request(&self.service_name, "L1", "get", "hit");
                return Ok(Some(bytes));
            }
            let duration = start.elapsed().as_secs_f64();
            GLOBAL_METRICS.record_duration(&self.service_name, "L1", "get", duration);
            GLOBAL_METRICS.record_request(&self.service_name, "L1", "get", "miss");

            // 2. 检查健康状态 - 如果L2降级，仍然尝试L1，但跳过L2
            let state = self.health_state.read().await;
            let is_degraded = matches!(*state, HealthState::Degraded { .. });
            drop(state);

            // 3. 尝试L2（仅当L2健康时）
            if !is_degraded {
                GLOBAL_METRICS.record_request(&self.service_name, "L2", "get", "attempt");
                let start = std::time::Instant::now();
                match l2.get_bytes(key).await {
                    Ok(Some(value)) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(&self.service_name, "L2", "get", duration);
                        GLOBAL_METRICS.record_request(&self.service_name, "L2", "get", "hit");

                        // 注意：L2Client的get_bytes不返回版本信息，所以promotion逻辑需要调整
                        // 如果需要版本信息，我们需要在L2Client中暴露get_with_version方法
                        if self.config.promote_on_hit {
                            if let Some(promotion_mgr) = &self.promotion_mgr {
                                let promo = promotion_mgr.clone();
                                let k = key.to_string();
                                let v = value.clone();
                                // 使用版本0作为默认值，因为get_bytes不返回版本
                                tokio::spawn(async move {
                                    let _ = promo.promote(k, v, 0).await;
                                });
                            }
                        }

                        return Ok(Some(value));
                    }
                    Ok(None) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(&self.service_name, "L2", "get", duration);
                        GLOBAL_METRICS.record_request(&self.service_name, "L2", "get", "miss");
                        // L2未命中，继续尝试数据库回源
                    }
                    Err(_e) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(&self.service_name, "L2", "get", duration);
                        self.handle_l2_failure().await;
                        // L2失败时继续尝试数据库回源
                    }
                }
            }

            // 4. 数据库回源（当L1和L2都未命中时）
            if let Some(db_fallback_mgr) = &self.db_fallback_mgr {
                GLOBAL_METRICS.record_request(&self.service_name, "DB", "fallback", "attempt");
                let start = std::time::Instant::now();

                match db_fallback_mgr.fallback_load(key).await {
                    Ok(Some(data)) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(
                            &self.service_name,
                            "DB",
                            "fallback",
                            duration,
                        );
                        GLOBAL_METRICS.record_request(&self.service_name, "DB", "fallback", "hit");

                        // 将数据回写到L1和L2缓存
                        if let Err(e) = self.set_bytes(key, data.clone(), None).await {
                            warn!("Failed to write fallback data to cache: {}", e);
                        }

                        return Ok(Some(data));
                    }
                    Ok(None) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(
                            &self.service_name,
                            "DB",
                            "fallback",
                            duration,
                        );
                        GLOBAL_METRICS.record_request(&self.service_name, "DB", "fallback", "miss");
                        debug!("Database fallback miss for key: {}", key);
                    }
                    Err(e) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(
                            &self.service_name,
                            "DB",
                            "fallback",
                            duration,
                        );
                        warn!("Database fallback failed for key {}: {}", key, e);
                    }
                }
            }
        }

        Ok(None)
    }

    /// 设置缓存值（字节）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let bytes = value;

        // 自动将键添加到布隆过滤器
        if let Some(bloom_filter) = &self.bloom_filter {
            let key_bytes = key.as_bytes().to_vec();
            bloom_filter.add(&key_bytes).await;
            GLOBAL_METRICS.record_request(&self.service_name, "BloomFilter", "set", "add");
        }

        // Two-level mode
        if let (Some(l1), Some(l2)) = (&self.l1, &self.l2) {
            // 1. 写入L1
            let start = std::time::Instant::now();
            debug!("Writing to L1: key={}", key);
            l1.set_bytes(key, bytes.clone(), ttl).await?;
            let duration = start.elapsed().as_secs_f64();
            GLOBAL_METRICS.record_duration(&self.service_name, "L1", "set", duration);
            debug!("L1 write successful: key={}", key);

            // 2. 检查L2健康状态
            let state = self.health_state.read().await;
            let current_state = *state;
            debug!("Current health state: {:?}", current_state);
            match current_state {
                HealthState::Healthy | HealthState::Recovering { .. } => {
                    drop(state);
                    if self.config.enable_batch_write {
                        if let Some(batch_writer) = &self.batch_writer {
                            batch_writer.enqueue(key.to_string(), bytes, ttl).await?;
                        }
                    } else {
                        // 使用L2客户端的set_bytes方法，它会处理健康状态检查
                        l2.set_bytes(key, bytes, ttl).await?;
                    }
                }
                HealthState::Degraded { .. } => {
                    drop(state);
                    debug!("L2 is degraded, writing to WAL: key={}", key);
                    self.wal
                        .append(WalEntry {
                            timestamp: std::time::SystemTime::now(),
                            operation: Operation::Set,
                            key: key.to_string(),
                            value: Some(bytes),
                            ttl: ttl.map(|t| t as i64),
                        })
                        .await?;
                    debug!("WAL write successful: key={}", key);
                }
            }
        }

        Ok(())
    }

    /// 设置 L1 缓存值（字节）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    async fn set_l1_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        if let Some(l1) = &self.l1 {
            let start = std::time::Instant::now();
            l1.set_bytes(key, value, ttl).await?;
            let duration = start.elapsed().as_secs_f64();
            GLOBAL_METRICS.record_duration(&self.service_name, "L1", "set", duration);
        }
        Ok(())
    }

    /// 设置 L2 缓存值（字节）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    async fn set_l2_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        if let Some(l2) = &self.l2 {
            // 检查L2健康状态
            let state = self.health_state.read().await;
            match *state {
                HealthState::Healthy | HealthState::Recovering { .. } => {
                    drop(state);
                    // 使用L2客户端的set_bytes方法，它会处理健康状态检查
                    l2.set_bytes(key, value, ttl).await?;
                }
                HealthState::Degraded { .. } => {
                    // 降级时不支持直接写入 L2，或者我们可以选择写入 WAL？
                    // set_l2_only 通常意味着强制写入 L2。如果 L2 不可用，应该报错或者写 WAL。
                    // 这里我们选择报错，因为用户明确要求 L2。
                    return Err(crate::error::CacheError::L2Error(
                        "L2 is degraded".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// 获取 L1 缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l1_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(l1) = &self.l1 {
            let start = std::time::Instant::now();
            let result = l1.get_bytes(key).await?;
            let duration = start.elapsed().as_secs_f64();
            GLOBAL_METRICS.record_duration(&self.service_name, "L1", "get", duration);
            Ok(result)
        } else {
            Ok(None)
        }
    }

    /// 获取 L2 缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l2_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(l2) = &self.l2 {
            let start = std::time::Instant::now();
            let result = l2.get_bytes(key).await?;
            let duration = start.elapsed().as_secs_f64();
            GLOBAL_METRICS.record_duration(&self.service_name, "L2", "get", duration);
            Ok(result)
        } else {
            Ok(None)
        }
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
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn delete(&self, key: &str) -> Result<()> {
        // Two-level mode
        if let (Some(l1), Some(l2)) = (&self.l1, &self.l2) {
            // 1. 删除L1
            l1.delete(key).await?;

            // 2. 检查L2健康状态
            let state = self.health_state.read().await;
            match *state {
                HealthState::Healthy | HealthState::Recovering { .. } => {
                    drop(state);
                    match l2.delete(key).await {
                        Ok(_) => {
                            if let Some(publisher) = &self.publisher {
                                let _ = publisher.publish(key).await;
                            }
                        }
                        Err(e) => {
                            self.handle_l2_failure().await;
                            return Err(e);
                        }
                    }
                }
                HealthState::Degraded { .. } => {
                    drop(state);
                    self.wal
                        .append(WalEntry {
                            timestamp: std::time::SystemTime::now(),
                            operation: Operation::Delete,
                            key: key.to_string(),
                            value: None,
                            ttl: None,
                        })
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// 清空 L1 缓存
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn clear_l1(&self) -> Result<()> {
        if let Some(l1) = &self.l1 {
            l1.clear()?;
            GLOBAL_METRICS.record_request(&self.service_name, "L1", "clear", "success");
        }
        Ok(())
    }

    /// 清空 L2 缓存
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn clear_l2(&self) -> Result<()> {
        if let Some(l2) = &self.l2 {
            l2.clear().await?;
            GLOBAL_METRICS.record_request(&self.service_name, "L2", "clear", "success");
        }
        Ok(())
    }

    /// 清空 WAL 日志
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn clear_wal(&self) -> Result<()> {
        self.wal.clear().await?;
        GLOBAL_METRICS.record_request(&self.service_name, "WAL", "clear", "success");
        Ok(())
    }
}

impl TwoLevelClient {
    /// 获取缓存值（带反序列化）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    pub async fn get<T: serde::de::DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        if let Some(bytes) = self.get_bytes(key).await? {
            return Ok(Some(self.serializer.deserialize(&bytes)?));
        }
        Ok(None)
    }

    /// 设置缓存值（带序列化）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    pub async fn set<T: serde::Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<u64>,
    ) -> Result<()> {
        let bytes = self.serializer.serialize(value)?;
        self.set_bytes(key, bytes, ttl).await
    }

    /// 仅设置L1缓存（手动控制）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    pub async fn set_l1_only<T: serde::Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<u64>,
    ) -> Result<()> {
        let bytes = self.serializer.serialize(value)?;
        CacheOps::set_l1_bytes(self, key, bytes, ttl).await
    }

    /// 仅设置L2缓存（手动控制）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    pub async fn set_l2_only<T: serde::Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<u64>,
    ) -> Result<()> {
        let bytes = self.serializer.serialize(value)?;
        CacheOps::set_l2_bytes(self, key, bytes, ttl).await
    }

    /// 仅获取L1缓存（手动控制）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    pub async fn get_l1_only<T: serde::de::DeserializeOwned + Send>(
        &self,
        key: &str,
    ) -> Result<Option<T>> {
        if let Some(bytes) = CacheOps::get_l1_bytes(self, key).await? {
            return Ok(Some(self.serializer.deserialize(&bytes)?));
        }
        Ok(None)
    }

    /// 仅获取L2缓存（手动控制）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    pub async fn get_l2_only<T: serde::de::DeserializeOwned + Send>(
        &self,
        key: &str,
    ) -> Result<Option<T>> {
        if let Some(bytes) = CacheOps::get_l2_bytes(self, key).await? {
            return Ok(Some(self.serializer.deserialize(&bytes)?));
        }
        Ok(None)
    }

    /// Ping L2 backend to check connectivity
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    pub async fn ping_l2(&self) -> Result<()> {
        if let Some(l2) = &self.l2 {
            l2.ping().await
        } else {
            Err(crate::error::CacheError::L2Error(
                "L2 client not available".to_string(),
            ))
        }
    }

    /// 设置数据库回源管理器
    ///
    /// # 参数
    ///
    /// * `db_fallback_mgr` - 数据库回源管理器
    #[instrument(skip(self), level = "info", fields(service = %self.service_name))]
    pub fn set_db_fallback_manager(&mut self, db_fallback_mgr: Arc<DbFallbackManager>) {
        info!(
            "Setting database fallback manager for service: {}",
            self.service_name
        );
        self.db_fallback_mgr = Some(db_fallback_mgr);
    }

    /// 获取数据库回源管理器
    pub fn get_db_fallback_manager(&self) -> Option<Arc<DbFallbackManager>> {
        self.db_fallback_mgr.clone()
    }
}
