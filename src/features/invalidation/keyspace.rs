// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Redis 键空间通知监听（`invalidation` feature）
//!
//! 失效总线的**第二通道**：订阅 Redis keyspace notifications，把
//! **库外变更**（redis-cli DEL / TTL 过期）投影为本地 L1 失效——补齐
//! "外部工具改 Redis → 应用感知" 的场景（总线只管库内写传播）。
//!
//! # 频道模式（keyevent）
//!
//! 订阅 `__keyevent@<db>__:<event>` 频道（普通 SUBSCRIBE 即可，无需
//! PSUBSCRIBE），载荷 = 受影响的 key：
//!
//! - 默认频道：`__keyevent@0__:del`、`__keyevent@0__:expired`
//! - Redis 端需开启：`CONFIG SET notify-keyspace-events "Egx"`
//!   （E=keyevent，g=DEL 等通用命令，x=expired）
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::invalidation::{KeyspaceNotificationConfig, KeyspaceNotificationListener};
//!
//! KeyspaceNotificationListener::spawn(transport, KeyspaceNotificationConfig::default(), l1).await?;
//! // redis-cli DEL user:1 → 本实例 L1 的 user:1 被失效
//! ```

use super::{ListenerHandle, PubSubTransport};
use crate::backend::CacheBackend;
use crate::error::OxCacheResult;
use std::sync::Arc;
use std::time::Duration;

/// keyspace 通知监听配置
#[derive(Debug, Clone)]
pub struct KeyspaceNotificationConfig {
    /// 订阅的 keyevent 频道（载荷 = key）
    pub channels: Vec<String>,
}

impl Default for KeyspaceNotificationConfig {
    fn default() -> Self {
        Self {
            channels: vec![
                "__keyevent@0__:del".to_string(),
                "__keyevent@0__:expired".to_string(),
            ],
        }
    }
}

impl KeyspaceNotificationConfig {
    /// 自定义频道集合（如 `__keyevent@0__:hdel`）
    pub fn with_channels(mut self, channels: Vec<String>) -> Self {
        self.channels = channels;
        self
    }

    /// db index 生成默认频道（del + expired）
    pub fn for_db(db: i32) -> Self {
        Self {
            channels: vec![
                format!("__keyevent@{db}__:del"),
                format!("__keyevent@{db}__:expired"),
            ],
        }
    }
}

/// 键空间通知监听器
///
/// 收到频道消息（载荷 = key）即删除本地 L1 对应条目。所有 keyspace
/// 消息视为**库外变更**（无实例归属），不做失效总线式自失效豁免；与本实例
/// 经总线传播的失效天然幂等（重复 delete 为 no-op）。
pub struct KeyspaceNotificationListener;

impl KeyspaceNotificationListener {
    /// 订阅 keyevent 频道并失效本地 L1
    pub async fn spawn(
        transport: Arc<dyn PubSubTransport>,
        config: KeyspaceNotificationConfig,
        l1: Arc<dyn CacheBackend>,
    ) -> OxCacheResult<ListenerHandle> {
        // 逐频道订阅（频道间消息合并到同一接收循环）
        let mut merged: Vec<_> = Vec::new();
        for channel in &config.channels {
            let rx = transport.subscribe(channel).await?;
            merged.push(rx);
        }

        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_flag = stop.clone();

        let join = tokio::spawn(async move {
            loop {
                if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                // 带超时轮询：100ms 内任意频道有消息即处理
                let mut payload: Option<String> = None;
                let mut closed: Vec<usize> = Vec::new();
                for (idx, rx) in merged.iter_mut().enumerate() {
                    match tokio::time::timeout(Duration::from_millis(1), rx.recv()).await {
                        Ok(Some(msg)) => {
                            payload = Some(msg);
                            break;
                        }
                        // 单个频道订阅端关闭：仅摘除该频道，其余继续
                        Ok(None) => closed.push(idx),
                        Err(_) => continue,
                    }
                }
                for idx in closed.into_iter().rev() {
                    merged.remove(idx);
                }
                if merged.is_empty() {
                    // 全部频道关闭：监听结束
                    return;
                }
                let Some(key) = payload else {
                    // 无消息：稍作让步避免热循环
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    continue;
                };
                let _ = l1.delete(&key).await;
            }
        });

        Ok(ListenerHandle::new(join, stop))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;
    use crate::features::invalidation::InMemoryPubSubTransport;
    use std::sync::Arc;

    fn l1() -> Arc<dyn CacheBackend> {
        Arc::new(MockBackend::new("mock", 100, false))
    }

    async fn set_entry(l1: &Arc<dyn CacheBackend>, key: &str) {
        l1.set(Arc::from(key), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
    }

    /// 外部 DEL 事件 → 本地 L1 失效
    #[tokio::test]
    async fn external_del_event_invalidates_local_l1() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let l1 = l1();
        set_entry(&l1, "user:1").await;

        let handle = KeyspaceNotificationListener::spawn(
            transport.clone(),
            KeyspaceNotificationConfig::default(),
            l1.clone(),
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 模拟 redis-cli DEL 触发的 keyevent 消息（载荷 = key）
        transport
            .publish("__keyevent@0__:del", "user:1")
            .await
            .unwrap();

        for _ in 0..50 {
            if !l1.exists("user:1").await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!l1.exists("user:1").await.unwrap(), "外部 DEL 库失效本地条目");

        handle.stop();
        handle.join().await;
    }

    /// 过期事件同样投影为本地失效
    #[tokio::test]
    async fn expired_event_invalidates_local_l1() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let l1 = l1();
        set_entry(&l1, "session:42").await;

        let handle = KeyspaceNotificationListener::spawn(
            transport.clone(),
            KeyspaceNotificationConfig::default(),
            l1.clone(),
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        transport
            .publish("__keyevent@0__:expired", "session:42")
            .await
            .unwrap();

        for _ in 0..50 {
            if !l1.exists("session:42").await.unwrap() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!l1.exists("session:42").await.unwrap());

        handle.stop();
        handle.join().await;
    }

    /// 未订阅频道的事件不失效任何条目
    #[tokio::test]
    async fn unsubscribed_channels_are_ignored() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let l1 = l1();
        set_entry(&l1, "keep:1").await;

        // 只订阅 db0 的 del；publish 到 db1 与 set 频道
        let handle = KeyspaceNotificationListener::spawn(
            transport.clone(),
            KeyspaceNotificationConfig::for_db(0),
            l1.clone(),
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        transport.publish("__keyevent@1__:del", "keep:1").await.unwrap();
        transport.publish("__keyevent@0__:hdel", "keep:1").await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;

        assert!(l1.exists("keep:1").await.unwrap(), "未订阅频道不得失效");

        handle.stop();
        handle.join().await;
    }

    #[test]
    fn default_channels_cover_del_and_expired() {
        let cfg = KeyspaceNotificationConfig::default();
        assert_eq!(cfg.channels.len(), 2);
        assert!(cfg.channels[0].ends_with(":del"));
        assert!(cfg.channels[1].ends_with(":expired"));

        let cfg = KeyspaceNotificationConfig::for_db(3);
        assert!(cfg.channels[0].contains("@3__:"), "got {:?}", cfg.channels[0]);
    }
}
