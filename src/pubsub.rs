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
        let client = redis::Client::open(url)
            .map_err(|e| OxCacheError::BackendError(format!("pubsub client open: {e}")))?;
        // 预热发布连接：URL 无效/不可达时在构造期显性失败（fail-fast）
        let publish_conn = redis::aio::ConnectionManager::new(client.clone())
            .await
            .map_err(|e| OxCacheError::BackendError(format!("pubsub connection: {e}")))?;
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
            .map_err(|e| OxCacheError::BackendError(format!("pubsub publish: {e}")))
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
                    let _ = ready_tx.send(Err(OxCacheError::BackendError(format!(
                        "pubsub connect: {e}"
                    ))));
                    return;
                }
            };
            if let Err(e) = pubsub.subscribe(&channel).await {
                let _ = ready_tx.send(Err(OxCacheError::BackendError(format!(
                    "pubsub subscribe: {e}"
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
                                        "pubsub handler panicked: channel={channel}, continue"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "pubsub payload parse failed: channel={channel}, err={e}"
                                );
                            }
                        }
                    }
                }
                // 消息流结束 = 连接断开：按可配置次数线性退避重连并重新 SUBSCRIBE
                if attempt >= max_attempts {
                    tracing::error!(
                        "pubsub stream ended, max reconnect attempts ({max_attempts}) reached: channel={channel}"
                    );
                    return;
                }
                attempt += 1;
                let backoff = std::time::Duration::from_millis(
                    (RECONNECT_BACKOFF_BASE_MS * attempt as u64).min(RECONNECT_BACKOFF_MAX_MS),
                );
                tracing::warn!(
                    "pubsub stream ended (connection lost): channel={channel}, reconnect {attempt}/{max_attempts} in {backoff:?}"
                );
                tokio::time::sleep(backoff).await;
                match client.get_async_pubsub().await {
                    Ok(mut p) => {
                        if let Err(e) = p.subscribe(&channel).await {
                            tracing::error!(
                                "pubsub resubscribe failed: channel={channel}, err={e}"
                            );
                            return;
                        }
                        pubsub = p;
                        // 重连成功：计数清零（连续断连才计入放弃判定）
                        attempt = 0;
                    }
                    Err(e) => {
                        tracing::error!("pubsub reconnect failed: channel={channel}, err={e}");
                        return;
                    }
                }
            }
        });

        let result = match ready_rx.await {
            Ok(r) => r,
            Err(_) => Err(OxCacheError::BackendError(
                "pubsub subscribe task ended before ready".to_string(),
            )),
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

    /// TCP 探活本地 Redis（127.0.0.1:6379）是否可达（约定同 garrison
    /// tests/acceptance/environment.rs：不可达时 eprintln!("[SKIP] …") 并 return）。
    async fn redis_reachable() -> bool {
        tokio::net::TcpStream::connect("127.0.0.1:6379")
            .await
            .is_ok()
    }

    /// 端到端收发测试的前置探测：127.0.0.1:6379 必须是**支持 PUBLISH 的真
    /// Redis**。同进程 `test_support::ensure_server(6379)` 会在真 Redis 缺席时
    /// 架起最小假服务器（无 PUBLISH），仅 TCP 探活无法区分，故以一条探测
    /// PUBLISH 的整数回复为准；非真 Redis 时 SKIP（与不可达同一口径）。
    async fn real_redis_available() -> bool {
        if !redis_reachable().await {
            return false;
        }
        let probe = async {
            let client = redis::Client::open("redis://127.0.0.1:6379").ok()?;
            let mut conn = client.get_multiplexed_async_connection().await.ok()?;
            redis::cmd("PUBLISH")
                .arg("oxcache-pubsub-probe")
                .arg("1")
                .query_async::<i64>(&mut conn)
                .await
                .ok()
        };
        probe.await.is_some()
    }

    /// 构造指向不可达端口的实例（确定性失败路径，无 Redis 依赖）。
    #[tokio::test]
    async fn new_with_unreachable_port_fails_fast() {
        let result = RedisPubSub::new("redis://127.0.0.1:1").await;
        assert!(result.is_err(), "不可达端口构造应 fail-fast 返回 Err");
    }

    /// 非法 URL 在构造期被 redis client 拒绝。
    #[tokio::test]
    async fn new_rejects_malformed_url() {
        assert!(RedisPubSub::new("not-a-redis-url").await.is_err());
    }

    /// subscribe 收到 publish 的消息（端到端收发）。
    #[tokio::test]
    async fn subscribe_receives_published_message() {
        if !real_redis_available().await {
            eprintln!("[SKIP] 127.0.0.1:6379 无支持 PUBLISH 的真 Redis");
            return;
        }
        let ps = RedisPubSub::new("redis://127.0.0.1:6379").await.unwrap();
        // 接收端用 tokio mpsc + timeout 异步等待：std 的 recv_timeout 会阻塞
        // current_thread 运行时的唯一线程，后台订阅任务无从投递；
        // 发送端须 unbounded——tokio 1.53 起有界 Sender::send 为 async，
        // 同步 handler 内无法 await，未 poll 的 future 会导致消息静默丢失
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        ps.subscribe(
            "oxcache-pubsub-test-e2e",
            Arc::new(move |msg| {
                let _ = tx.send(msg);
            }),
        )
        .await
        .expect("subscribe 应成功");
        // 订阅建立存在异步窗口，稍候再发布
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        ps.publish("oxcache-pubsub-test-e2e", "hello-pubsub")
            .await
            .unwrap();
        let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await;
        assert_eq!(
            received.ok().flatten().as_deref(),
            Some("hello-pubsub"),
            "订阅端应收到发布的消息"
        );
        ps.shutdown();
    }

    /// handler panic 被隔离：panic 后续消息仍投递、订阅不中断。
    #[tokio::test]
    async fn subscribe_handler_panic_does_not_interrupt() {
        if !real_redis_available().await {
            eprintln!("[SKIP] 127.0.0.1:6379 无支持 PUBLISH 的真 Redis");
            return;
        }
        let ps = RedisPubSub::new("redis://127.0.0.1:6379").await.unwrap();
        // 同上：unbounded 同步 send + 异步等待投递
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();
        ps.subscribe(
            "oxcache-pubsub-test-panic",
            Arc::new(move |msg| {
                counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if msg == "panic-trigger" {
                    panic!("intentional handler panic");
                }
                let _ = tx.send(msg);
            }),
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        ps.publish("oxcache-pubsub-test-panic", "panic-trigger")
            .await
            .unwrap();
        // 共享 Redis 在并行负载下会抖动：panic 处理期间若恰逢订阅任务断线
        // 重连，该窗口内发布的消息会丢失（重连后恢复订阅）。每轮重发一次覆盖
        // 重连窗口（pub/sub 重复投递无害），钉住「订阅存活 + panic 后恢复
        // 投递」的核心语义。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut received: Option<String> = None;
        while std::time::Instant::now() < deadline {
            ps.publish("oxcache-pubsub-test-panic", "after-panic")
                .await
                .unwrap();
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(msg)) => {
                    received = Some(msg);
                    break;
                }
                // 通道关闭 = 订阅任务提前退出，与静默超时是两种失败，须区分
                Ok(None) => panic!("订阅任务提前退出（通道关闭），panic 隔离失效"),
                Err(_) => {}
            }
        }
        assert_eq!(
            received.as_deref(),
            Some("after-panic"),
            "panic 后续消息应继续投递"
        );
        assert!(
            counter.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "panic 前后的消息都应到达 handler"
        );
        ps.shutdown();
    }

    /// shutdown 停止消息投递：shutdown 后发布的消息不再到达 handler。
    #[tokio::test]
    async fn shutdown_stops_message_delivery() {
        if !real_redis_available().await {
            eprintln!("[SKIP] 127.0.0.1:6379 无支持 PUBLISH 的真 Redis");
            return;
        }
        let ps = RedisPubSub::new("redis://127.0.0.1:6379").await.unwrap();
        // 异步等待窗口：std 阻塞接收会冻结运行时，泄漏消息反而探测不到
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        ps.subscribe(
            "oxcache-pubsub-test-shutdown",
            Arc::new(move |msg| {
                let _ = tx.send(msg);
            }),
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let stopped = ps.shutdown();
        assert_eq!(stopped, 1, "应停止 1 个订阅任务");

        ps.publish("oxcache-pubsub-test-shutdown", "after-shutdown")
            .await
            .unwrap();
        // 留出潜在投递窗口：不应收到任何消息。
        // Ok(None)（channel 关闭：handler 连同被 abort 的任务一起 drop）与
        // Err(Elapsed)（超时无消息）都是"无泄漏"；Ok(Some(_)) 才是泄漏
        let leaked = tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await;
        assert!(
            !matches!(leaked, Ok(Some(_))),
            "shutdown 后不应再收到消息，实际: {leaked:?}"
        );
    }
}
