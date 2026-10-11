// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Redis Pub/Sub 广播设施（跨实例消息通道）。
//!
//! 缓存失效通知（[`crate::features::invalidation`]）之外的第二类跨实例语义：
//! 通用频道广播。典型消费方：分布式会话/登录态跨节点同步（SSO kickout 广播）、
//! 业务事件扇出。
//!
//! ## 能力
//!
//! - **publish**：经多路复用 `ConnectionManager` 发 `PUBLISH`（自动重连）
//! - **subscribe**：专用订阅连接（`SUBSCRIBE` 会独占连接，无法复用多路复用连接），
//!   首次订阅成功后经 oneshot 回传结果（不"假成功"）
//! - **断线重连**：消息流结束（连接断开）后按可配置次数线性退避重连并重新 SUBSCRIBE
//! - **handler panic 隔离**：`catch_unwind` 包裹回调，panic 不中断订阅
//! - **任务回收**：后台订阅任务句柄统一登记，[`RedisPubSub::shutdown`]/Drop 时 abort
//!
//! ## 示例
//!
//! ```rust,no_run
//! # use std::sync::Arc;
//! # async fn example() -> Result<(), oxcache::OxCacheError> {
//! let ps = oxcache::pubsub::RedisPubSub::new("redis://127.0.0.1:6379").await?;
//! ps.subscribe("sso:kickout", Arc::new(|msg| {
//!     tracing::info!("kickout broadcast: {msg}");
//! })).await?;
//! ps.publish("sso:kickout", "user-42").await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use futures::stream::StreamExt;
use tokio::task::JoinHandle;

use crate::error::{OxCacheError, OxCacheResult};
use crate::i18n::messages::{
    MSG_DETAIL_PUBSUB_CLIENT_OPEN_FAILED, MSG_DETAIL_PUBSUB_CONNECT_FAILED,
    MSG_DETAIL_PUBSUB_CONNECTION_FAILED, MSG_DETAIL_PUBSUB_PUBLISH_FAILED,
    MSG_DETAIL_PUBSUB_SUBSCRIBE_FAILED, MSG_DETAIL_PUBSUB_SUBSCRIBE_TASK_ENDED,
    MSG_LOG_PUBSUB_HANDLER_PANICKED, MSG_LOG_PUBSUB_MAX_RECONNECT_ATTEMPTS_REACHED,
    MSG_LOG_PUBSUB_PAYLOAD_PARSE_FAILED, MSG_LOG_PUBSUB_RECONNECT_FAILED,
    MSG_LOG_PUBSUB_RECONNECTING, MSG_LOG_PUBSUB_RESUBSCRIBE_FAILED, t,
};

/// 断线后最大重连尝试次数（默认 5；0 = 不重连，断连即结束订阅）。
const DEFAULT_RECONNECT_ATTEMPTS: usize = 5;
/// 第 `n` 次重连的退避基数（线性增长：500ms、1s、1.5s …）。
const RECONNECT_BACKOFF_BASE_MS: u64 = 500;
/// 退避封顶（5s）。
const RECONNECT_BACKOFF_MAX_MS: u64 = 5000;

/// Redis Pub/Sub 广播通道。
///
/// `publish` 经多路复用连接管理器；每个 `subscribe` 独占一条订阅连接
/// （由 `Self::client` 现场创建）。后台订阅任务统一登记，
/// [`Self::shutdown`]/Drop 时 abort 回收。
pub struct RedisPubSub {
    /// Redis 客户端（为每个订阅创建专用 PubSub 连接）。
    client: redis::Client,
    /// 连接管理器（用于 PUBLISH，自动重连）。
    publish_conn: redis::aio::ConnectionManager,
    /// 后台订阅任务句柄（即弃会导致任务泄漏，shutdown/Drop 统一回收）。
    tasks: std::sync::Mutex<Vec<JoinHandle<()>>>,
    /// 连接断开后最大重连尝试次数（0 = 不重连，一次性订阅语义）。
    max_reconnect_attempts: usize,
}

impl RedisPubSub {
    /// 从 Redis URL 创建发布/订阅通道。
    pub async fn new(url: &str) -> OxCacheResult<Self> {
        let client = redis::Client::open(url).map_err(|e| {
            OxCacheError::BackendError(t(
                MSG_DETAIL_PUBSUB_CLIENT_OPEN_FAILED,
                &[("err", e.to_string())],
            ))
        })?;
        // 预热发布连接：URL 无效/不可达时在构造期显性失败（fail-fast）
        let publish_conn = redis::aio::ConnectionManager::new(client.clone())
            .await
            .map_err(|e| {
                OxCacheError::BackendError(t(
                    MSG_DETAIL_PUBSUB_CONNECTION_FAILED,
                    &[("err", e.to_string())],
                ))
            })?;
        Ok(Self {
            client,
            publish_conn,
            tasks: std::sync::Mutex::new(Vec::new()),
            max_reconnect_attempts: DEFAULT_RECONNECT_ATTEMPTS,
        })
    }

    /// 配置断线重连最大尝试次数（默认 5；0 = 不重连）。
    #[must_use]
    pub fn with_reconnect_attempts(mut self, attempts: usize) -> Self {
        self.max_reconnect_attempts = attempts;
        self
    }

    /// 向频道发布消息，返回接收端数量（无订阅者时为 0）。
    pub async fn publish(&self, channel: &str, message: &str) -> OxCacheResult<i64> {
        let mut conn = self.publish_conn.clone();
        redis::cmd("PUBLISH")
            .arg(channel)
            .arg(message)
            .query_async::<i64>(&mut conn)
            .await
            .map_err(|e| {
                OxCacheError::BackendError(t(
                    MSG_DETAIL_PUBSUB_PUBLISH_FAILED,
                    &[("err", e.to_string())],
                ))
            })
    }

    /// 订阅频道（独占连接 + 后台消息循环 + 断线重连）。
    ///
    /// 返回 `Ok(())` 时表示首次订阅已成功，此后 `handler` 在后台任务中
    /// 逐条接收消息；handler panic 被隔离（warn 日志后继续）。
    pub async fn subscribe(
        &self,
        channel: &str,
        handler: Arc<dyn Fn(String) + Send + Sync>,
    ) -> OxCacheResult<()> {
        let channel = channel.to_string();
        let client = self.client.clone();
        let max_attempts = self.max_reconnect_attempts;

        // oneshot 回传首次连接+订阅结果：失败时调用方拿到 Err（不"假成功"）
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<OxCacheResult<()>>();

        let handle = tokio::spawn(async move {
            let mut pubsub = match client.get_async_pubsub().await {
                Ok(p) => p,
                Err(e) => {
                    let _ = ready_tx.send(Err(OxCacheError::BackendError(t(
                        MSG_DETAIL_PUBSUB_CONNECT_FAILED,
                        &[("err", e.to_string())],
                    ))));
                    return;
                }
            };
            if let Err(e) = pubsub.subscribe(&channel).await {
                let _ = ready_tx.send(Err(OxCacheError::BackendError(t(
                    MSG_DETAIL_PUBSUB_SUBSCRIBE_FAILED,
                    &[("err", e.to_string())],
                ))));
                return;
            }
            // 首次订阅成功，通知调用方
            let _ = ready_tx.send(Ok(()));

            // 消息循环 + 断线重连
            let mut attempt = 0usize;
            loop {
                {
                    let mut msg_stream = pubsub.on_message();
                    while let Some(msg) = msg_stream.next().await {
                        let payload: Result<String, _> = msg.get_payload();
                        match payload {
                            Ok(payload_str) => {
                                // catch_unwind 隔离 handler panic，防止中断订阅
                                let handler_clone = handler.clone();
                                let result = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(move || {
                                        handler_clone(payload_str);
                                    }),
                                );
                                if result.is_err() {
                                    tracing::warn!(
                                        channel = %channel,
                                        "{}",
                                        t(MSG_LOG_PUBSUB_HANDLER_PANICKED, &[])
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    channel = %channel,
                                    err = %e,
                                    "{}",
                                    t(MSG_LOG_PUBSUB_PAYLOAD_PARSE_FAILED, &[])
                                );
                            }
                        }
                    }
                }
                // 消息流结束 = 连接断开：按可配置次数线性退避重连并重新 SUBSCRIBE
                if attempt >= max_attempts {
                    tracing::error!(
                        channel = %channel,
                        attempts = max_attempts,
                        "{}",
                        t(MSG_LOG_PUBSUB_MAX_RECONNECT_ATTEMPTS_REACHED, &[])
                    );
                    return;
                }
                attempt += 1;
                let backoff = std::time::Duration::from_millis(
                    (RECONNECT_BACKOFF_BASE_MS * attempt as u64).min(RECONNECT_BACKOFF_MAX_MS),
                );
                tracing::warn!(
                    channel = %channel,
                    attempt,
                    max_attempts,
                    backoff = ?backoff,
                    "{}",
                    t(MSG_LOG_PUBSUB_RECONNECTING, &[])
                );
                tokio::time::sleep(backoff).await;
                match client.get_async_pubsub().await {
                    Ok(mut p) => {
                        if let Err(e) = p.subscribe(&channel).await {
                            tracing::error!(
                                channel = %channel,
                                err = %e,
                                "{}",
                                t(MSG_LOG_PUBSUB_RESUBSCRIBE_FAILED, &[])
                            );
                            return;
                        }
                        pubsub = p;
                        // 重连成功：计数清零（连续断连才计入放弃判定）
                        attempt = 0;
                    }
                    Err(e) => {
                        tracing::error!(
                            channel = %channel,
                            err = %e,
                            "{}",
                            t(MSG_LOG_PUBSUB_RECONNECT_FAILED, &[])
                        );
                        return;
                    }
                }
            }
        });

        let result = match ready_rx.await {
            Ok(r) => r,
            Err(_) => Err(OxCacheError::BackendError(t(
                MSG_DETAIL_PUBSUB_SUBSCRIBE_TASK_ENDED,
                &[],
            ))),
        };
        // 订阅任务交给结构体登记（成功路径）；失败路径任务已自行退出
        if result.is_ok() {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.retain(|h| !h.is_finished());
            tasks.push(handle);
        }
        result
    }

    /// 停止全部后台订阅任务（abort 并清空句柄），返回被停止的任务数。
    pub fn shutdown(&self) -> usize {
        let mut tasks = self.tasks.lock().unwrap();
        let stopped = tasks.len();
        for handle in tasks.drain(..) {
            handle.abort();
        }
        stopped
    }
}

impl Drop for RedisPubSub {
    fn drop(&mut self) {
        // Drop 时 abort 全部订阅任务，避免任务泄漏
        for handle in self.tasks.lock().unwrap().drain(..) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造指向不可达端口的实例（确定性失败路径，无 Redis 依赖）。
    #[tokio::test]
    async fn new_with_unreachable_port_fails_fast() {
        let result = RedisPubSub::new("redis://127.0.0.1:1").await;
        assert!(
            result.is_err(),
            "unreachable port construction must fail-fast with Err"
        );
    }

    /// 非法 URL 在构造期被 redis client 拒绝。
    #[tokio::test]
    async fn new_rejects_malformed_url() {
        assert!(RedisPubSub::new("not-a-redis-url").await.is_err());
    }

    // 端到端收发 / panic 隔离 / shutdown / 断线重连需真实 Redis 端点，由
    // tests/integration/redis/pubsub_real_redis_test.rs 以容器门控覆盖
    // （进程内假服务器不实现 PUBLISH/SUBSCRIBE，此处无法验证）。
}
