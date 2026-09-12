// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Pub/Sub 传输抽象（协议层）
//!
//! [`PubSubTransport`] 定义失效总线的 wire 协议层：`publish` 原始载荷、
//! `subscribe` 返回接收端。两个实现：
//!
//! - [`InMemoryPubSubTransport`]：进程内协议层 mock，广播语义与 Redis
//!   Pub/Sub 一致（订阅后发布、at-most-once、无持久化），供单测使用；
//! - [`RedisPubSubTransport`]：真实 Redis `PUBLISH`/`SUBSCRIBE` 实现。

use crate::error::{OxCacheError, OxCacheResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;

/// 订阅接收端：从通道接收原始载荷（字符串）
pub struct SubscriptionReceiver {
    rx: UnboundedReceiver<String>,
}

impl SubscriptionReceiver {
    pub(crate) fn new(rx: UnboundedReceiver<String>) -> Self {
        Self { rx }
    }

    /// 接收下一条原始载荷；订阅端全部关闭后返回 `None`
    pub async fn recv(&mut self) -> Option<String> {
        self.rx.recv().await
    }

    /// 非阻塞尝试接收
    pub fn try_recv(&mut self) -> Option<String> {
        self.rx.try_recv().ok()
    }
}

/// 失效总线传输抽象（协议层）
///
/// 实现必须保证：消息仅投递给**已订阅**的接收端（Pub/Sub at-most-once
/// 语义，无离线补投；兜底策略是 L1 保留短 TTL）。
#[async_trait]
pub trait PubSubTransport: Send + Sync + 'static {
    /// 向通道发布一条原始载荷。无订阅者时为 no-op。
    async fn publish(&self, channel: &str, payload: &str) -> OxCacheResult<()>;

    /// 订阅通道，返回原始载荷接收端。
    async fn subscribe(&self, channel: &str) -> OxCacheResult<SubscriptionReceiver>;
}

// ============================================================================
// InMemory mock transport（协议层，供单测与进程内组合使用）
// ============================================================================

/// 进程内 Pub/Sub 协议层 mock。
///
/// 语义对齐 Redis Pub/Sub：只有订阅之后发布的消息会被投递；每条消息
/// 广播给该通道的全部订阅者；无持久化。
#[derive(Default)]
pub struct InMemoryPubSubTransport {
    channels: Mutex<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<String>>>>,
}

impl InMemoryPubSubTransport {
    /// 创建空的 mock 传输
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前某通道订阅者数量（测试辅助）
    pub fn subscriber_count(&self, channel: &str) -> usize {
        self.channels
            .lock()
            .map(|map| map.get(channel).map(Vec::len).unwrap_or(0))
            .unwrap_or(0)
    }
}

#[async_trait]
impl PubSubTransport for InMemoryPubSubTransport {
    async fn publish(&self, channel: &str, payload: &str) -> OxCacheResult<()> {
        match self.channels.lock() {
            Ok(mut map) => {
                if let Some(list) = map.get_mut(channel) {
                    let payload = payload.to_string();
                    // 广播给全部订阅者；接收端已断开的发送者当场剔除，
                    // 防止订阅churn 导致死 sender 在 map 中持续累积
                    list.retain(|tx| tx.send(payload.clone()).is_ok());
                }
            }
            // 锁中毒：与 subscribe 的容错口径一致，静默放弃本次投递
            Err(_) => {}
        }
        Ok(())
    }

    async fn subscribe(&self, channel: &str) -> OxCacheResult<SubscriptionReceiver> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        match self.channels.lock() {
            Ok(mut map) => {
                map.entry(channel.to_string()).or_default().push(tx);
            }
            Err(_) => {
                return Err(OxCacheError::Operation(
                    "in-memory pubsub transport lock poisoned".to_string(),
                ))
            }
        }
        Ok(SubscriptionReceiver::new(rx))
    }
}

// ============================================================================
// Redis transport（真实 Pub/Sub）
// ============================================================================

/// Redis Pub/Sub 传输实现。
///
/// - `publish`：经复用的 `ConnectionManager` 执行 `PUBLISH <channel> <payload>`；
/// - `subscribe`：独立 Pub/Sub 连接 `SUBSCRIBE <channel>`，后台任务把
///   消息转发到无界通道。订阅断线时任务以指数退避自动重连（重连窗口
///   内的消息不补投，语义与 Pub/Sub at-most-once 一致）。
pub struct RedisPubSubTransport {
    client: redis::Client,
    /// 复用的发布连接（PUBLISH 走常规连接即可）
    publish_conn: tokio::sync::Mutex<redis::aio::ConnectionManager>,
    connection_string: String,
}

impl RedisPubSubTransport {
    /// 连接 Redis 并创建传输
    pub async fn new(connection_string: &str) -> OxCacheResult<Self> {
        let client = redis::Client::open(connection_string)
            .map_err(|e| OxCacheError::Connection(format!("invalid redis url: {e}")))?;
        let publish_conn = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.get_connection_manager(),
        )
        .await
        .map_err(|_| OxCacheError::Connection("redis pubsub connect timeout".to_string()))?
        .map_err(|e| OxCacheError::Connection(format!("redis pubsub connect failed: {e}")))?;
        Ok(Self {
            client,
            publish_conn: tokio::sync::Mutex::new(publish_conn),
            connection_string: connection_string.to_string(),
        })
    }

    /// 连接串（脱敏场景由上层处理）
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }

    /// 后台订阅任务：订阅 + 转发 + 断线指数退避重连
    async fn run_subscription(
        client: redis::Client,
        channel: String,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) {
        use futures::stream::StreamExt;
        let mut backoff_ms: u64 = 200;
        loop {
            let subscribed = async {
                let mut pubsub = client.get_async_pubsub().await?;
                pubsub.subscribe(channel.as_str()).await?;
                Ok::<_, redis::RedisError>(pubsub)
            };
            let mut pubsub = match subscribed.await {
                Ok(ps) => {
                    backoff_ms = 200; // 成功后重置退避
                    ps
                }
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(10_000);
                    continue;
                }
            };
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                let payload: Option<String> = msg.get_payload().ok();
                if let Some(payload) = payload
                    && tx.send(payload).is_err()
                {
                    // 接收端已关闭：停止订阅
                    return;
                }
            }
            // 流结束（连接断开）：退避后重连
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms.saturating_mul(2)).min(10_000);
        }
    }
}

#[async_trait]
impl PubSubTransport for RedisPubSubTransport {
    async fn publish(&self, channel: &str, payload: &str) -> OxCacheResult<()> {
        let mut conn = self.publish_conn.lock().await;
        let _: i64 = redis::cmd("PUBLISH")
            .arg(channel)
            .arg(payload)
            .query_async(&mut *conn)
            .await
            .map_err(|e| OxCacheError::Connection(format!("redis PUBLISH failed: {e}")))?;
        Ok(())
    }

    async fn subscribe(&self, channel: &str) -> OxCacheResult<SubscriptionReceiver> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(Self::run_subscription(
            self.client.clone(),
            channel.to_string(),
            tx,
        ));
        Ok(SubscriptionReceiver::new(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn in_memory_subscribe_then_publish_delivers() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let mut rx = transport.subscribe("ch").await.unwrap();
        transport.publish("ch", "hello").await.unwrap();
        assert_eq!(rx.recv().await, Some("hello".to_string()));
    }

    #[tokio::test]
    async fn in_memory_publish_before_subscribe_is_dropped() {
        // Pub/Sub at-most-once：订阅前发布不补投
        let transport = Arc::new(InMemoryPubSubTransport::new());
        transport.publish("ch", "early").await.unwrap();
        let mut rx = transport.subscribe("ch").await.unwrap();
        transport.publish("ch", "late").await.unwrap();
        assert_eq!(rx.recv().await, Some("late".to_string()));
        // "early" 不会被收到
        assert!(rx.try_recv().is_none());
    }

    #[tokio::test]
    async fn in_memory_broadcast_to_all_subscribers() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let mut rx1 = transport.subscribe("ch").await.unwrap();
        let mut rx2 = transport.subscribe("ch").await.unwrap();
        assert_eq!(transport.subscriber_count("ch"), 2);
        transport.publish("ch", "fanout").await.unwrap();
        assert_eq!(rx1.recv().await, Some("fanout".to_string()));
        assert_eq!(rx2.recv().await, Some("fanout".to_string()));
    }

    #[tokio::test]
    async fn in_memory_disconnected_senders_are_pruned() {
        // 接收端 dropped 后，死 sender 应在下一次 publish 时被剔除，
        // 不在通道 map 中无限累积
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let rx = transport.subscribe("ch").await.unwrap();
        assert_eq!(transport.subscriber_count("ch"), 1);

        drop(rx);
        transport.publish("ch", "after-drop").await.unwrap();
        assert_eq!(transport.subscriber_count("ch"), 0, "死 sender 应被剔除");
    }

    #[tokio::test]
    async fn in_memory_channels_are_isolated() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let mut rx_a = transport.subscribe("a").await.unwrap();
        let mut rx_b = transport.subscribe("b").await.unwrap();
        transport.publish("a", "msg-a").await.unwrap();
        assert_eq!(rx_a.recv().await, Some("msg-a".to_string()));
        assert!(rx_b.try_recv().is_none());
    }

    #[test]
    fn in_memory_publish_without_subscribers_is_noop() {
        let transport = InMemoryPubSubTransport::new();
        // 无订阅者时 publish 不报错（fire-and-forget）
        futures::executor::block_on(async {
            transport.publish("ghost", "x").await.unwrap();
        });
    }
}
