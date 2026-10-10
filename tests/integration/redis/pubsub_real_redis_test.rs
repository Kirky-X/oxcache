// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `RedisPubSub` / `RedisPubSubTransport` 真实 Redis 端点测试（testcontainers 门控）
//!
//! 进程内假服务器不实现 PUBLISH/SUBSCRIBE，pub/sub 的端到端语义（订阅后发布、
//! handler panic 隔离、shutdown/Drop 回收订阅任务、断线重连与一次性订阅）只能
//! 对真实 Redis 验证。Docker 不可用、启动超时或容器无法就绪时跳过
//! （`OXCACHE_TEST_STRICT` 置位时改为失败，语义见 `container_or_skip`）。

#![cfg(all(feature = "redis", feature = "pubsub"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use oxcache::features::invalidation::transport::{PubSubTransport, RedisPubSubTransport};
use oxcache::pubsub::RedisPubSub;

#[path = "../../common/mod.rs"]
mod common;
use common::test_containers::{RedisContainer, container_or_skip, start_redis_container};

/// 就绪的 Redis 容器与连接 URL；不可用时返回 None（调用方跳过测试）
async fn setup() -> Option<(RedisContainer, String)> {
    container_or_skip("Redis(pubsub)", start_redis_container()).await
}

/// 订阅后发布：消息到达 handler，`publish` 返回接收端数量（≥1）。
#[tokio::test]
async fn pubsub_roundtrip_delivers_and_reports_receiver_count() {
    let Some((_container, url)) = setup().await else {
        return;
    };
    let ps = RedisPubSub::new(&url).await.expect("构造 RedisPubSub");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    ps.subscribe(
        "oxcache:it:pubsub:roundtrip",
        Arc::new(move |msg| {
            let _ = tx.send(msg);
        }),
    )
    .await
    .expect("subscribe 应成功");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let receivers = ps
        .publish("oxcache:it:pubsub:roundtrip", "hello-pubsub")
        .await
        .expect("publish 应成功");
    assert!(receivers >= 1, "至少 1 个订阅者，got {receivers}");

    let received = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await;
    assert_eq!(
        received.ok().flatten().as_deref(),
        Some("hello-pubsub"),
        "订阅端应收到发布的消息"
    );
    assert_eq!(ps.shutdown(), 1, "应回收 1 个订阅任务");
}

/// handler panic 被隔离：panic 后续消息仍投递、订阅不中断。
#[tokio::test]
async fn pubsub_handler_panic_is_isolated_and_delivery_resumes() {
    let Some((_container, url)) = setup().await else {
        return;
    };
    let ps = RedisPubSub::new(&url).await.expect("构造 RedisPubSub");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    ps.subscribe(
        "oxcache:it:pubsub:panic",
        Arc::new(move |msg| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            if msg == "panic-trigger" {
                panic!("intentional handler panic");
            }
            let _ = tx.send(msg);
        }),
    )
    .await
    .expect("subscribe 应成功");
    tokio::time::sleep(Duration::from_millis(100)).await;

    ps.publish("oxcache:it:pubsub:panic", "panic-trigger")
        .await
        .expect("publish 应成功");

    // 重连窗口内发布的消息会丢失：循环重发钉住「panic 后订阅仍存活」语义
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut received: Option<String> = None;
    while std::time::Instant::now() < deadline {
        ps.publish("oxcache:it:pubsub:panic", "after-panic")
            .await
            .expect("publish 应成功");
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
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
        counter.load(Ordering::SeqCst) >= 2,
        "panic 前后的消息都应到达 handler"
    );
    ps.shutdown();
}

/// shutdown 停止消息投递：shutdown 后发布的消息不再到达 handler。
#[tokio::test]
async fn pubsub_shutdown_stops_message_delivery() {
    let Some((_container, url)) = setup().await else {
        return;
    };
    let ps = RedisPubSub::new(&url).await.expect("构造 RedisPubSub");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    ps.subscribe(
        "oxcache:it:pubsub:shutdown",
        Arc::new(move |msg| {
            let _ = tx.send(msg);
        }),
    )
    .await
    .expect("subscribe 应成功");
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(ps.shutdown(), 1, "应停止 1 个订阅任务");
    ps.publish("oxcache:it:pubsub:shutdown", "after-shutdown")
        .await
        .expect("publish 应成功");

    // Ok(None)（通道关闭）与 Err(Elapsed)（超时无消息）都是"无泄漏"
    let leaked = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(
        !matches!(leaked, Ok(Some(_))),
        "shutdown 后不应再收到消息，实际: {leaked:?}"
    );
}

/// Drop 回收后台订阅任务：订阅者被 drop 后其连接不再接收消息。
#[tokio::test]
async fn pubsub_drop_stops_background_subscription() {
    let Some((_container, url)) = setup().await else {
        return;
    };
    let subscriber = RedisPubSub::new(&url).await.expect("构造订阅端");
    let publisher = RedisPubSub::new(&url).await.expect("构造发布端");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    subscriber
        .subscribe(
            "oxcache:it:pubsub:drop",
            Arc::new(move |msg| {
                let _ = tx.send(msg);
            }),
        )
        .await
        .expect("subscribe 应成功");
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(subscriber);

    publisher
        .publish("oxcache:it:pubsub:drop", "after-drop")
        .await
        .expect("publish 应成功");
    let leaked = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(
        !matches!(leaked, Ok(Some(_))),
        "Drop 后订阅任务应被 abort，不再投递，实际: {leaked:?}"
    );
}

/// 断开订阅连接：`with_reconnect_attempts(0)` 的一次性订阅语义——任务退出，
/// 不再重连（消息不补投）。
#[tokio::test]
async fn pubsub_zero_reconnect_attempts_ends_subscription_on_disconnect() {
    let Some((_container, url)) = setup().await else {
        return;
    };
    let ps = RedisPubSub::new(&url)
        .await
        .expect("构造 RedisPubSub")
        .with_reconnect_attempts(0);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    ps.subscribe(
        "oxcache:it:pubsub:no-reconnect",
        Arc::new(move |msg| {
            let _ = tx.send(msg);
        }),
    )
    .await
    .expect("subscribe 应成功");
    tokio::time::sleep(Duration::from_millis(100)).await;

    kill_pubsub_connections(&url).await;
    // 连接已断：重连尝试上限为 0，任务直接结束
    tokio::time::sleep(Duration::from_millis(300)).await;
    ps.publish("oxcache:it:pubsub:no-reconnect", "after-kill")
        .await
        .expect("publish 应成功");

    let leaked = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(
        !matches!(leaked, Ok(Some(_))),
        "一次性订阅在断连后不应再投递，实际: {leaked:?}"
    );
}

/// 断线重连：订阅连接被服务端切断后，后台任务重新 SUBSCRIBE 并恢复投递。
#[tokio::test]
async fn pubsub_reconnects_and_resubscribes_after_disconnect() {
    let Some((_container, url)) = setup().await else {
        return;
    };
    let ps = RedisPubSub::new(&url)
        .await
        .expect("构造 RedisPubSub")
        .with_reconnect_attempts(5);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    ps.subscribe(
        "oxcache:it:pubsub:reconnect",
        Arc::new(move |msg| {
            let _ = tx.send(msg);
        }),
    )
    .await
    .expect("subscribe 应成功");
    tokio::time::sleep(Duration::from_millis(100)).await;

    kill_pubsub_connections(&url).await;
    // 重连退避 500ms 起：循环发布覆盖重连窗口（Pub/Sub 无补投）
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    let mut received: Option<String> = None;
    while std::time::Instant::now() < deadline {
        ps.publish("oxcache:it:pubsub:reconnect", "after-reconnect")
            .await
            .expect("publish 应成功");
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Some(msg)) => {
                received = Some(msg);
                break;
            }
            Ok(None) => panic!("订阅任务提前退出（通道关闭）"),
            Err(_) => {}
        }
    }
    assert_eq!(
        received.as_deref(),
        Some("after-reconnect"),
        "重连后应恢复投递"
    );
    ps.shutdown();
}

/// 服务端切断全部 pub/sub 连接（不影响普通连接）
async fn kill_pubsub_connections(url: &str) {
    let client = oxcache::redis::Client::open(url).expect("open redis client");
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .expect("connect for CLIENT KILL");
    let killed: i64 = oxcache::redis::cmd("CLIENT")
        .arg("KILL")
        .arg("TYPE")
        .arg("pubsub")
        .query_async(&mut conn)
        .await
        .expect("CLIENT KILL TYPE pubsub");
    assert!(killed >= 1, "应至少切断 1 条订阅连接，got {killed}");
}

/// 失效总线协议层：Redis 传输的 publish/subscribe 端到端投递与连接串回读。
#[tokio::test]
async fn redis_transport_publishes_and_subscribes() {
    let Some((_container, url)) = setup().await else {
        return;
    };
    let transport = RedisPubSubTransport::new(&url)
        .await
        .expect("构造 Redis 传输层");
    assert_eq!(transport.connection_string(), url);

    let mut rx = transport
        .subscribe("oxcache:it:transport")
        .await
        .expect("subscribe 应成功");
    tokio::time::sleep(Duration::from_millis(100)).await;
    transport
        .publish("oxcache:it:transport", "payload")
        .await
        .expect("publish 应成功");

    let received = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await;
    assert_eq!(
        received.ok().flatten().as_deref(),
        Some("payload"),
        "协议层应投递原始载荷"
    );
}
