// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 安全信号总线：发布 / 订阅 / 检测点钩子

use super::monitor::{SecurityHook, SecurityResponder};
use super::signal::SecuritySignal;
use super::{DEFAULT_CHANNEL, DEFAULT_SIGNAL_TTL};
use crate::error::OxCacheResult;
use crate::features::invalidation::{ListenerHandle, PubSubTransport};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 安全事件总线配置
#[derive(Debug, Clone)]
pub struct SecurityEventConfig {
    /// 本实例 ID（自豁免依据）
    pub instance_id: String,
    /// Pub/Sub 通道名
    pub channel: String,
    /// 信号时效：超过该时长的信号在订阅端丢弃
    pub signal_ttl: Duration,
}

impl SecurityEventConfig {
    /// 以默认通道与时效创建配置
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            channel: DEFAULT_CHANNEL.to_string(),
            signal_ttl: DEFAULT_SIGNAL_TTL,
        }
    }

    /// 自定义通道
    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = channel.into();
        self
    }

    /// 自定义信号时效
    pub fn with_signal_ttl(mut self, ttl: Duration) -> Self {
        self.signal_ttl = ttl;
        self
    }
}

/// 安全信号总线
pub struct SecurityEventBus {
    transport: Arc<dyn PubSubTransport>,
    config: SecurityEventConfig,
}

impl SecurityEventBus {
    /// 创建总线
    pub fn new(transport: Arc<dyn PubSubTransport>, config: SecurityEventConfig) -> Self {
        Self { transport, config }
    }

    /// 当前配置
    pub fn config(&self) -> &SecurityEventConfig {
        &self.config
    }

    /// 发布信号
    pub async fn publish_signal(&self, signal: &SecuritySignal) -> OxCacheResult<()> {
        self.transport
            .publish(&self.config.channel, &signal.encode()?)
            .await
    }

    /// 订阅并投递：自豁免 → 时效过滤 → responder 回调
    pub async fn spawn_listener(
        &self,
        responder: Arc<dyn SecurityResponder>,
    ) -> OxCacheResult<ListenerHandle> {
        let mut rx = self.transport.subscribe(&self.config.channel).await?;
        let instance_id = self.config.instance_id.clone();
        let signal_ttl = self.config.signal_ttl;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();

        let join = tokio::spawn(async move {
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
                let payload =
                    match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                        Ok(Some(payload)) => payload,
                        Ok(None) => break,
                        Err(_) => continue,
                    };
                let signal = match SecuritySignal::decode(&payload) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if signal.instance_id() == instance_id {
                    continue;
                }
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                if signal
                    .raised_at_unix_ms()
                    .saturating_add(signal_ttl.as_millis() as u64)
                    < now_ms
                {
                    continue;
                }
                responder.on_signal(&signal);
            }
        });

        Ok(ListenerHandle::new(join, stop))
    }

    /// 供检测点持有：回调内 spawn 发布，失败仅记日志不 panic
    pub fn hook(self: &Arc<Self>) -> SecurityHook {
        let bus = self.clone();
        let instance_id = self.config.instance_id.clone();
        Arc::new(move |signal: SecuritySignal| {
            let bus = bus.clone();
            let instance_id = instance_id.clone();
            tokio::spawn(async move {
                // 检测点不感知实例 ID；由总线在发布前补齐，供接收端自豁免
                let signal = match signal {
                    SecuritySignal::IntegrityCompromised {
                        key,
                        consecutive_failures,
                        raised_at_unix_ms,
                        ..
                    } => SecuritySignal::IntegrityCompromised {
                        instance_id,
                        key,
                        consecutive_failures,
                        raised_at_unix_ms,
                    },
                    SecuritySignal::FencingAnomaly {
                        lock_key,
                        node_index,
                        observed_token,
                        last_token,
                        raised_at_unix_ms,
                        ..
                    } => SecuritySignal::FencingAnomaly {
                        instance_id,
                        lock_key,
                        node_index,
                        observed_token,
                        last_token,
                        raised_at_unix_ms,
                    },
                };
                if let Err(err) = bus.publish_signal(&signal).await {
                    #[cfg(feature = "telemetry")]
                    tracing::warn!(
                        target: "oxcache::security_events",
                        "publish security signal failed: {err}"
                    );
                    #[cfg(not(feature = "telemetry"))]
                    let _ = err;
                }
            });
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::invalidation::InMemoryPubSubTransport;
    use std::sync::Mutex;

    struct CollectingResponder {
        seen: Arc<Mutex<Vec<SecuritySignal>>>,
    }

    impl SecurityResponder for CollectingResponder {
        fn on_signal(&self, signal: &SecuritySignal) {
            self.seen.lock().unwrap().push(signal.clone());
        }
    }

    fn signal(instance: &str, at_ms: u64) -> SecuritySignal {
        SecuritySignal::IntegrityCompromised {
            instance_id: instance.to_string(),
            key: "k".to_string(),
            consecutive_failures: 3,
            raised_at_unix_ms: at_ms,
        }
    }

    #[test]
    fn config_defaults() {
        let c = SecurityEventConfig::new("node-a");
        assert_eq!(c.channel, "oxcache:sec-events");
        assert_eq!(c.signal_ttl, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn subscriber_receives_foreign_signal() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus = Arc::new(SecurityEventBus::new(
            transport,
            SecurityEventConfig::new("node-b"),
        ));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let responder = Arc::new(CollectingResponder { seen: seen.clone() });
        let handle = bus.spawn_listener(responder).await.unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        bus.publish_signal(&signal("node-a", now)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(seen.lock().unwrap().len(), 1);
        handle.stop();
        handle.join().await;
    }

    #[tokio::test]
    async fn own_signal_is_exempt() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus = Arc::new(SecurityEventBus::new(
            transport,
            SecurityEventConfig::new("node-a"),
        ));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let responder = Arc::new(CollectingResponder { seen: seen.clone() });
        let handle = bus.spawn_listener(responder).await.unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        bus.publish_signal(&signal("node-a", now)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(seen.lock().unwrap().is_empty());
        handle.stop();
        handle.join().await;
    }

    #[tokio::test]
    async fn expired_signal_is_dropped() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus = Arc::new(SecurityEventBus::new(
            transport,
            SecurityEventConfig::new("node-b"),
        ));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let responder = Arc::new(CollectingResponder { seen: seen.clone() });
        let handle = bus.spawn_listener(responder).await.unwrap();
        bus.publish_signal(&signal("node-a", 1)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(seen.lock().unwrap().is_empty());
        handle.stop();
        handle.join().await;
    }

    #[tokio::test]
    async fn stop_terminates_listener() {
        let transport = Arc::new(InMemoryPubSubTransport::new());
        let bus = Arc::new(SecurityEventBus::new(
            transport,
            SecurityEventConfig::new("node-b"),
        ));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let responder = Arc::new(CollectingResponder { seen });
        let handle = bus.spawn_listener(responder).await.unwrap();
        handle.stop();
        tokio::time::timeout(Duration::from_secs(2), handle.join())
            .await
            .expect("listener must exit after stop");
    }
}
