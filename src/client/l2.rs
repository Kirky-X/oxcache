//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了L2-only缓存客户端的实现。

use super::CacheOps;
use crate::backend::l2::L2Backend;
use crate::config::TwoLevelConfig;
use crate::error::Result;
use crate::metrics::GLOBAL_METRICS;
use crate::recovery::{
    health::{HealthChecker, HealthState},
    wal::{Operation, WalEntry, WalManager},
};
use crate::serialization::SerializerEnum;
use crate::sync::invalidation::InvalidationPublisher;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{instrument, warn};

/// L2-only 缓存客户端实现
///
/// 仅使用分布式缓存（Redis），提供数据持久化和多实例共享
pub struct L2Client {
    /// 服务名称
    service_name: String,
    /// L2缓存后端
    l2: Arc<L2Backend>,
    /// 序列化器
    serializer: SerializerEnum,
    /// 健康状态
    health_state: Arc<RwLock<HealthState>>,
    /// WAL管理器
    wal: Arc<WalManager>,
    /// 失效发布器
    publisher: Option<Arc<InvalidationPublisher>>,
}

impl L2Client {
    /// 创建新的L2-only缓存客户端
    pub async fn new(
        service_name: String,
        l2: Arc<L2Backend>,
        serializer: SerializerEnum,
    ) -> Result<Self> {
        let health_state = Arc::new(RwLock::new(HealthState::Healthy));
        let wal = Arc::new(WalManager::new(&service_name).await?);

        // 启动健康检查器
        let command_timeout_ms = l2.command_timeout_ms();
        let checker = HealthChecker::new(
            l2.clone(),
            health_state.clone(),
            wal.clone(),
            service_name.clone(),
            command_timeout_ms,
        );
        tokio::spawn(async move { checker.start().await });

        // 默认使用 TwoLevelConfig 的默认值来解析频道名称，
        // 虽然这里只有 L2，但为了复用 resolve_channel_name 逻辑（如果需要的话）
        // 或者直接硬编码/使用简单逻辑。
        // 原 TwoLevelClient 使用 resolve_channel_name，这里我们可以简化，
        // 或者为了保持一致性，假设 L2-only 也可能需要 invalidation (虽然 L1 不存在，但其他实例可能有 L1?)
        // 如果其他实例是 TwoLevel，那么 L2-only 的修改也应该通知它们失效。

        let config = TwoLevelConfig::default();
        let channel_name = Self::resolve_channel_name(&service_name, &config);

        let publisher = Arc::new(InvalidationPublisher::new(
            l2.get_raw_client()?.get_connection_manager().await?,
            channel_name,
        ));

        Ok(Self {
            service_name,
            l2,
            serializer,
            health_state,
            wal,
            publisher: Some(publisher),
        })
    }

    /// 解决失效频道名称 (复用逻辑)
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

    /// 处理L2故障
    async fn handle_l2_failure(&self) {
        tracing::warn!("L2 failure detected for service: {}", self.service_name);

        // Update health state to degraded
        let mut state = self.health_state.write().await;
        *state = crate::recovery::health::HealthState::Degraded {
            since: std::time::Instant::now(),
            failure_count: 1,
        };
    }

    /// Ping L2 backend to check connectivity
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    pub async fn ping(&self) -> Result<()> {
        self.l2.ping().await
    }

    /// 清空 L2 缓存
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    pub async fn clear(&self) -> Result<()> {
        self.l2.clear(&self.service_name).await
    }
}

#[async_trait]
impl CacheOps for L2Client {
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

    /// 获取缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        GLOBAL_METRICS.record_request(&self.service_name, "L2", "get", "attempt");
        let start = std::time::Instant::now();
        match self.l2.get_with_version(key).await {
            Ok(Some((value, _))) => {
                let duration = start.elapsed().as_secs_f64();
                GLOBAL_METRICS.record_duration(&self.service_name, "L2", "get", duration);
                GLOBAL_METRICS.record_request(&self.service_name, "L2", "get", "hit");
                Ok(Some(value))
            }
            Ok(None) => {
                let duration = start.elapsed().as_secs_f64();
                GLOBAL_METRICS.record_duration(&self.service_name, "L2", "get", duration);
                GLOBAL_METRICS.record_request(&self.service_name, "L2", "get", "miss");
                Ok(None)
            }
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                GLOBAL_METRICS.record_duration(&self.service_name, "L2", "get", duration);
                self.handle_l2_failure().await;
                Err(e)
            }
        }
    }

    /// 设置缓存值（字节）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    async fn set_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        let state = self.health_state.read().await;
        tracing::info!("set_bytes: current health state = {:?}", *state);
        match *state {
            HealthState::Healthy | HealthState::Recovering { .. } => {
                drop(state);

                let start = std::time::Instant::now();

                // 先检查key是否存在，只有更新已存在的key时才发送失效通知
                let key_exists = match self.l2.get_with_version(key).await {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(_) => true, // 如果检查失败，假设key存在，发送失效通知
                };

                match self.l2.set_with_version(key, value.clone(), ttl).await {
                    Ok(_) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(&self.service_name, "L2", "set", duration);
                        // 只有在更新已存在的key时才发送失效通知
                        if key_exists {
                            if let Some(publisher) = &self.publisher {
                                let _ = publisher.publish(key).await;
                            }
                        }
                        Ok(())
                    }
                    Err(e) => {
                        let duration = start.elapsed().as_secs_f64();
                        GLOBAL_METRICS.record_duration(&self.service_name, "L2", "set", duration);
                        tracing::warn!("L2 set failed during set_bytes, writing to WAL: {}", e);
                        self.handle_l2_failure().await;

                        // Write to WAL on failure
                        self.wal
                            .append(WalEntry {
                                timestamp: std::time::SystemTime::now(),
                                operation: Operation::Set,
                                key: key.to_string(),
                                value: Some(value),
                                ttl: ttl.map(|t| t as i64),
                            })
                            .await?;

                        // Return success since operation was written to WAL
                        Ok(())
                    }
                }
            }
            HealthState::Degraded { .. } => {
                tracing::info!("set_bytes: L2 is degraded, writing to WAL and returning success");
                drop(state);
                self.wal
                    .append(WalEntry {
                        timestamp: std::time::SystemTime::now(),
                        operation: Operation::Set,
                        key: key.to_string(),
                        value: Some(value),
                        ttl: ttl.map(|t| t as i64),
                    })
                    .await?;

                // Return success since operation was written to WAL
                Ok(())
            }
            HealthState::WalReplaying { .. } => {
                tracing::info!(
                    "set_bytes: L2 is replaying WAL, writing to WAL and returning success"
                );
                drop(state);
                self.wal
                    .append(WalEntry {
                        timestamp: std::time::SystemTime::now(),
                        operation: Operation::Set,
                        key: key.to_string(),
                        value: Some(value),
                        ttl: ttl.map(|t| t as i64),
                    })
                    .await?;

                // Return success since operation was written to WAL
                Ok(())
            }
        }
    }

    /// 设置 L2 缓存值（字节）
    #[instrument(skip(self, value), level = "debug", fields(service = %self.service_name))]
    async fn set_l2_bytes(&self, key: &str, value: Vec<u8>, ttl: Option<u64>) -> Result<()> {
        self.set_bytes(key, value, ttl).await
    }

    /// 获取 L1 缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l1_bytes(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// 获取 L2 缓存值（字节）
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn get_l2_bytes(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get_bytes(key).await
    }

    /// 删除缓存项
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn delete(&self, key: &str) -> Result<()> {
        let state = self.health_state.read().await;
        match *state {
            HealthState::Healthy | HealthState::Recovering { .. } => {
                drop(state);
                match self.l2.delete(key).await {
                    Ok(_) => {
                        if let Some(publisher) = &self.publisher {
                            let _ = publisher.publish(key).await;
                        }
                        Ok(())
                    }
                    Err(e) => {
                        self.handle_l2_failure().await;
                        Err(e)
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
                    .await
            }
            HealthState::WalReplaying { .. } => {
                drop(state);
                self.wal
                    .append(WalEntry {
                        timestamp: std::time::SystemTime::now(),
                        operation: Operation::Delete,
                        key: key.to_string(),
                        value: None,
                        ttl: None,
                    })
                    .await
            }
        }
    }

    /// 尝试获取分布式锁
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn lock(&self, key: &str, value: &str, ttl: u64) -> Result<bool> {
        let state = self.health_state.read().await;
        match *state {
            HealthState::Healthy | HealthState::Recovering { .. } => {
                drop(state);
                match self.l2.lock(key, value, ttl).await {
                    Ok(result) => {
                        if result {
                            GLOBAL_METRICS.record_request(&self.service_name, "L2", "lock", "hit");
                        } else {
                            GLOBAL_METRICS.record_request(&self.service_name, "L2", "lock", "miss");
                        }
                        Ok(result)
                    }
                    Err(e) => {
                        self.handle_l2_failure().await;
                        Err(e)
                    }
                }
            }
            HealthState::Degraded { .. } => {
                drop(state);
                warn!(
                    "Cannot acquire lock in degraded state, service={}",
                    self.service_name
                );
                Ok(false)
            }
            HealthState::WalReplaying { .. } => {
                drop(state);
                warn!(
                    "Cannot acquire lock during WAL replay, service={}",
                    self.service_name
                );
                Ok(false)
            }
        }
    }

    /// 释放分布式锁
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn unlock(&self, key: &str, value: &str) -> Result<bool> {
        let state = self.health_state.read().await;
        match *state {
            HealthState::Healthy | HealthState::Recovering { .. } => {
                drop(state);
                match self.l2.unlock(key, value).await {
                    Ok(result) => {
                        if result {
                            GLOBAL_METRICS.record_request(
                                &self.service_name,
                                "L2",
                                "unlock",
                                "hit",
                            );
                        } else {
                            GLOBAL_METRICS.record_request(
                                &self.service_name,
                                "L2",
                                "unlock",
                                "miss",
                            );
                        }
                        Ok(result)
                    }
                    Err(e) => {
                        self.handle_l2_failure().await;
                        Err(e)
                    }
                }
            }
            HealthState::Degraded { .. } => {
                drop(state);
                warn!(
                    "Cannot release lock in degraded state, service={}",
                    self.service_name
                );
                Ok(false)
            }
            HealthState::WalReplaying { .. } => {
                drop(state);
                warn!(
                    "Cannot release lock during WAL replay, service={}",
                    self.service_name
                );
                Ok(false)
            }
        }
    }

    /// 清空 L2 缓存
    #[instrument(skip(self), level = "debug", fields(service = %self.service_name))]
    async fn clear_l2(&self) -> Result<()> {
        self.l2.clear(&self.service_name).await?;
        GLOBAL_METRICS.record_request(&self.service_name, "L2", "clear", "success");
        Ok(())
    }
}
