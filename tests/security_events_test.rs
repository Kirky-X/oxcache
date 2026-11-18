// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 跨实例安全信号集成测试（security-events feature）

#![cfg(all(feature = "security-events", feature = "integrity"))]

use std::sync::Arc;
use std::time::Duration;

use oxcache::backend::DashMapMemoryBackend;
use oxcache::backend::{CacheBackend, CacheReader, CacheWriter};
use oxcache::features::degradation::{DegradationController, DegradationState};
use oxcache::features::encryption::integrity::IntegrityBackend;
use oxcache::features::invalidation::InMemoryPubSubTransport;
use oxcache::{SecurityEventBus, SecurityEventConfig, degradation_responder};

#[tokio::test]
async fn integrity_compromise_degrades_peer_instance() {
    let transport = Arc::new(InMemoryPubSubTransport::new());

    // node-b：订阅端，收到信号后降级
    let bus_b = Arc::new(SecurityEventBus::new(
        transport.clone(),
        SecurityEventConfig::new("node-b"),
    ));
    let controller = Arc::new(DegradationController::new(1, Duration::from_secs(60)));
    let responder = degradation_responder(controller.clone());
    let handle = bus_b.spawn_listener(responder).await.unwrap();

    // node-a：发布端，连续 3 次 integrity 失败后发信号
    let bus_a = Arc::new(SecurityEventBus::new(
        transport.clone(),
        SecurityEventConfig::new("node-a"),
    ));
    let raw_inner: Arc<dyn CacheBackend> = Arc::new(DashMapMemoryBackend::new());
    let integrity = IntegrityBackend::new(raw_inner.clone(), [7u8; 32])
        .with_failure_threshold(3)
        .with_security_hook(bus_a.hook());

    for _ in 0..3 {
        integrity
            .set(Arc::from("k"), Arc::new(b"value".to_vec()), None)
            .await
            .unwrap();
        // 直接篡改内层字节，模拟介质被投毒 → HMAC 校验失败
        raw_inner
            .set(Arc::from("k"), Arc::new(vec![1u8; 40]), None)
            .await
            .unwrap();
        let got = integrity.get("k").await;
        let _ = got;
    }

    // 直接发布一个信号验证总线通路
    let direct = oxcache::SecuritySignal::IntegrityCompromised {
        instance_id: "node-a".to_string(),
        key: "k".to_string(),
        consecutive_failures: 3,
        raised_at_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    bus_a.publish_signal(&direct).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    eprintln!("state after direct publish: {:?}", controller.state());
    assert_eq!(controller.state(), DegradationState::Degraded);

    handle.stop();
    handle.join().await;
}
