//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存失效机制，用于处理跨实例的缓存失效。

use crate::backend::l1::L1Backend;
use crate::error::Result;
use crate::recovery::health::HealthState;
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, instrument};

/// 缓存失效订阅者
///
/// 负责订阅Redis频道并处理缓存失效消息
pub struct InvalidationSubscriber {
    /// Redis客户端
    client: redis::Client,
    /// L1缓存后端
    l1: Arc<L1Backend>,
    /// 频道名称
    channel: String,
    /// 健康状态
    health_state: Arc<RwLock<HealthState>>,
}

impl InvalidationSubscriber {
    /// 创建新的失效订阅者
    ///
    /// # 参数
    ///
    /// * `client` - Redis客户端
    /// * `l1` - L1缓存后端
    /// * `channel` - 频道名称
    /// * `health_state` - 健康状态
    ///
    /// # 返回值
    ///
    /// 返回新的失效订阅者实例
    pub fn new(
        client: redis::Client,
        l1: Arc<L1Backend>,
        channel: String,
        health_state: Arc<RwLock<HealthState>>,
    ) -> Self {
        Self {
            client,
            l1,
            channel,
            health_state,
        }
    }

    /// 启动订阅者
    ///
    /// 开始监听频道中的失效消息并处理
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub async fn start(self) -> Result<()> {
        #[allow(deprecated)]
        let conn = self.client.get_async_connection().await?;
        let mut pubsub = conn.into_pubsub();
        pubsub.subscribe(&self.channel).await?;

        let _l1 = self.l1.clone();
        let _health_state = self.health_state.clone();
        debug!("InvalidationSubscriber: 启动订阅者，频道={}", self.channel);
        tokio::spawn(async move {
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                debug!("InvalidationSubscriber: 收到消息");
                // 检查健康状态，只在Redis健康时处理失效消息
                let state = _health_state.read().await;
                debug!("InvalidationSubscriber: 当前健康状态={:?}", *state);
                match *state {
                    HealthState::Healthy => {
                        drop(state);
                        let payload: String = match msg.get_payload() {
                            Ok(payload) => payload,
                            Err(e) => {
                                debug!("InvalidationSubscriber: 解析消息失败: {}", e);
                                continue;
                            }
                        };
                        debug!("InvalidationSubscriber: 处理失效消息，key={}", payload);
                        // 只有在Redis健康时才处理失效消息
                        let _ = _l1.delete(&payload).await;
                        debug!("L1键已失效: {}", payload);
                    }
                    HealthState::Degraded { .. } | HealthState::Recovering { .. } => {
                        drop(state);
                        debug!("Skipping invalidation during Redis outage");
                    }
                    HealthState::WalReplaying { .. } => {
                        drop(state);
                        debug!("Skipping invalidation during WAL replay");
                    }
                }
            }
        });

        Ok(())
    }
}

/// 缓存失效发布者
///
/// 负责向Redis频道发布缓存失效消息
pub struct InvalidationPublisher {
    /// 连接管理器
    manager: redis::aio::ConnectionManager,
    /// 频道名称
    channel: String,
}

impl InvalidationPublisher {
    /// 创建新的失效发布者
    ///
    /// # 参数
    ///
    /// * `manager` - 连接管理器
    /// * `channel` - 频道名称
    ///
    /// # 返回值
    ///
    /// 返回新的失效发布者实例
    pub fn new(manager: redis::aio::ConnectionManager, channel: String) -> Self {
        Self { manager, channel }
    }

    /// 发布失效消息
    ///
    /// # 参数
    ///
    /// * `key` - 失效的键
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    #[instrument(skip(self), level = "debug")]
    pub async fn publish(&self, key: &str) -> Result<()> {
        debug!("InvalidationPublisher: 发布失效消息，key={}", key);
        let mut conn = self.manager.clone();
        let _: i32 = redis::cmd("PUBLISH")
            .arg(&self.channel)
            .arg(key)
            .query_async(&mut conn)
            .await?;
        debug!("InvalidationPublisher: 失效消息发布成功，key={}", key);
        Ok(())
    }
}
