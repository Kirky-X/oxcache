// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 失效总线：发布失效事件 + 后台监听失效本地 L1

use super::{InvalidationKind, InvalidationMessage, PubSubTransport, DEFAULT_CHANNEL};
use crate::backend::CacheBackend;
use crate::error::OxCacheResult;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;

/// 失效总线配置
#[derive(Debug, Clone)]
pub struct InvalidationConfig {
    /// Pub/Sub 通道名（默认 `oxcache:invalidate`）
    pub channel: String,
    /// 本实例 ID（用于自失效豁免；默认 uuid v4）
    pub instance_id: String,
}

impl InvalidationConfig {
    /// 以默认通道 + 随机实例 ID 创建配置
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            channel: DEFAULT_CHANNEL.to_string(),
            instance_id: instance_id.into(),
        }
    }

    /// 自定义 Pub/Sub 通道名
    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = channel.into();
        self
    }
}

/// 后台监听任务句柄
pub struct ListenerHandle {
    /// `Some` = 尚未 join；`None` = join 已消费句柄
    join: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl ListenerHandle {
    /// 由子模块构造句柄
    pub(crate) fn new(join: JoinHandle<()>, stop: Arc<AtomicBool>) -> Self {
        Self {
            join: Some(join),
            stop,
        }
    }

    /// 请求停止监听（任务在下一条消息或轮询间隙退出）
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// 等待监听任务退出
    pub async fn join(mut self) {
        if let Some(join) = self.join.take() {
            let _ = join.await;
        }
    }
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        // 句柄被丢弃即请求停止，避免监听任务在无主状态下空转泄漏
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// 跨实例失效总线。
///
/// - [`invalidate_key`](Self::invalidate_key) / [`invalidate_namespace`](Self::invalidate_namespace)：
///   向通道广播失效事件（写路径调用）；
/// - [`spawn_listener`](Self::spawn_listener)：订阅通道并把**其他实例**的
///   失效事件应用到本地 L1 后端（自身消息豁免）。
pub struct InvalidationBus {
    transport: Arc<dyn PubSubTransport>,
    config: InvalidationConfig,
}

impl std::fmt::Debug for InvalidationBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvalidationBus")
            .field("channel", &self.config.channel)
            .field("instance_id", &self.config.instance_id)
            .finish()
    }
}

impl InvalidationBus {
    /// 基于传输层与配置创建总线
    pub fn new(transport: Arc<dyn PubSubTransport>, config: InvalidationConfig) -> Self {
        Self { transport, config }
    }

    /// 本实例 ID
    pub fn instance_id(&self) -> &str {
        &self.config.instance_id
    }

    /// 广播 key 粒度失效事件
    pub async fn invalidate_key(&self, key: &str) -> OxCacheResult<()> {
        let msg = InvalidationMessage::key(self.config.instance_id.clone(), key);
        self.transport
            .publish(&self.config.channel, &msg.encode()?)
            .await
    }

    /// 广播命名空间（前缀）粒度失效事件
    pub async fn invalidate_namespace(&self, namespace: &str) -> OxCacheResult<()> {
        let msg = InvalidationMessage::namespace(self.config.instance_id.clone(), namespace);
        self.transport
            .publish(&self.config.channel, &msg.encode()?)
            .await
    }

    /// 订阅总线并在收到其他实例的失效事件时失效本地 L1。
    ///
    /// 自失效豁免：`instance_id` 与自身相同的消息会被丢弃。
    pub async fn spawn_listener(&self, l1: Arc<dyn CacheBackend>) -> OxCacheResult<ListenerHandle> {
        let mut rx = self.transport.subscribe(&self.config.channel).await?;
        let instance_id = self.config.instance_id.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();

        let join = tokio::spawn(async move {
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
                // 带超时轮询以便响应 stop
                let payload = match tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    rx.recv(),
                )
                .await
                {
                    Ok(Some(payload)) => payload,
                    Ok(None) => break, // 订阅端关闭
                    Err(_) => continue, // 超时：回到 stop 检查
                };
                let msg = match InvalidationMessage::decode(&payload) {
                    Ok(m) => m,
                    Err(_) => continue, // 无法解析的消息跳过（不 panic）
                };
                // 自失效豁免：丢弃自身实例发出的消息
                if msg.is_from(&instance_id) {
                    continue;
                }
                apply_to_l1(l1.as_ref(), &msg).await;
            }
        });

        Ok(ListenerHandle::new(join, stop))
    }
}

/// 将失效事件应用到本地 L1 后端
async fn apply_to_l1(l1: &dyn CacheBackend, msg: &InvalidationMessage) {
    match msg.kind {
        InvalidationKind::Key => {
            let _ = l1.delete(&msg.target).await;
        }
        InvalidationKind::Namespace => {
            // 命名空间失效：前缀 glob 匹配后逐 key 删除
            let pattern = if msg.target == "*" {
                "*".to_string()
            } else {
                format!("{}*", msg.target)
            };
            if let Ok(keys) = l1.keys(&pattern).await {
                for key in keys {
                    let _ = l1.delete(&key).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;
    use crate::features::invalidation::InMemoryPubSubTransport;

    fn l1() -> Arc<dyn CacheBackend> {
        Arc::new(MockBackend::new("mock", 100, false))
    }

    async fn set_l1(backend: &Arc<dyn CacheBackend>, key: &str, value: &[u8]) {
        backend
            .set(Arc::from(key), Arc::new(value.to_vec()), None)
            .await
            .unwrap();
    }

    /// 双实例（mock 协议层）：A 写失效广播 → B 本地条目失效
    #[tokio::test]
    async fn cross_instance_invalidation_propagates() {
        let transport = Arc::new(InMemoryPubSubTransport::new());

        let bus_a = InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-a").with_channel("test-ch"),
        );
        let bus_b = InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-b").with_channel("test-ch"),
        );

        let l1_b = l1();
        set_l1(&l1_b, "user:1", b"alice").await;
        assert!(l1_b.exists("user:1").await.unwrap());

        // B 订阅失效总线
        let handle_b = bus_b.spawn_listener(l1_b.clone()).await.unwrap();
        // 等订阅建立
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // A 广播 key 失效
        bus_a.invalidate_key("user:1").await.unwrap();

        // B 的 L1 在 RTT 内失效
        for _ in 0..50 {
            if !l1_b.exists("user:1").await.unwrap() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            !l1_b.exists("user:1").await.unwrap(),
            "B 的本地 L1 条目应被 A 的广播失效"
        );

        handle_b.stop();
        handle_b.join().await;
    }

    /// 自失效豁免：A 自己广播的失效事件不重复失效 A 的本地 L1
    #[tokio::test]
    async fn self_invalidation_is_exempt() {
        let transport = Arc::new(InMemoryPubSubTransport::new());

        let bus_a = InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-a").with_channel("test-ch"),
        );

        let l1_a = l1();
        set_l1(&l1_a, "user:1", b"alice").await;

        // A 同时是发布者与订阅者
        let handle_a = bus_a.spawn_listener(l1_a.clone()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // A 广播自己的失效事件
        bus_a.invalidate_key("user:1").await.unwrap();

        // 给监听足够时间收到并（错误地）处理
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert!(
            l1_a.exists("user:1").await.unwrap(),
            "自身广播的失效消息应被豁免，A 的本地条目不应被删除"
        );

        handle_a.stop();
        handle_a.join().await;
    }

    /// 命名空间粒度失效：前缀匹配的条目全部失效
    #[tokio::test]
    async fn namespace_invalidation_removes_all_matching_keys() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus_a = InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-a").with_channel("test-ch"),
        );
        let bus_b = InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-b").with_channel("test-ch"),
        );

        let l1_b = l1();
        set_l1(&l1_b, "users:1", b"a").await;
        set_l1(&l1_b, "users:2", b"b").await;
        set_l1(&l1_b, "orders:1", b"c").await;

        let handle_b = bus_b.spawn_listener(l1_b.clone()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        bus_a.invalidate_namespace("users:").await.unwrap();

        for _ in 0..50 {
            if !l1_b.exists("users:1").await.unwrap() && !l1_b.exists("users:2").await.unwrap() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        assert!(!l1_b.exists("users:1").await.unwrap());
        assert!(!l1_b.exists("users:2").await.unwrap());
        assert!(
            l1_b.exists("orders:1").await.unwrap(),
            "非匹配前缀的条目不应被失效"
        );

        handle_b.stop();
        handle_b.join().await;
    }

    /// 无法解析的载荷不 panic、不失效任何条目
    #[tokio::test]
    async fn malformed_payload_is_ignored() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus = InvalidationBus::new(
            transport.clone(),
            InvalidationConfig::new("instance-a").with_channel("test-ch"),
        );

        let l1 = l1();
        set_l1(&l1, "user:1", b"keep").await;

        let handle = bus.spawn_listener(l1.clone()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // 直接经传输层注入垃圾载荷
        transport.publish("test-ch", "garbage-not-json").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert!(l1.exists("user:1").await.unwrap());

        handle.stop();
        handle.join().await;
    }

    #[test]
    fn config_defaults() {
        let cfg = InvalidationConfig::new("i");
        assert_eq!(cfg.channel, super::super::DEFAULT_CHANNEL);
        assert_eq!(cfg.instance_id, "i");

        let cfg = cfg.with_channel("custom");
        assert_eq!(cfg.channel, "custom");
    }
}
