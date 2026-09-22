// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! single-flight flight 信号回归测试（变更 2026-09-23-cache-audit-hardening T001/T002/T003）。
//!
//! 钉住的核心行为：flight 完成信号具备**无丢失唤醒**语义——leader 的完成通知
//! 先于 follower 订阅时，follower 仍必须立即醒来（旧 `Notify` + `notify_waiters`
//! 实现下该场景为永久挂起，见审计 F03）。

use std::sync::Arc;
use std::time::Duration;

use oxcache::macro_support::{SF_SHARDS, shard_index, wait_flight};

#[tokio::test]
async fn flight_signal_subscribe_before_send_wakes_waiter() {
    // 真实流程不变量：get_or/get_or_option 的 follower 在**分片锁内** subscribe，
    // leader 的 remove+send 必须先取得同一把锁——锁序保证 subscribe 恒先于 send。
    // 本测试钉住数据结构性质：subscribe 后的任意时序 send，changed() 都立即返回
    //（旧 Notify 实现 listener 注册在首次 poll，锁外窗口内丢失唤醒）。
    let (tx, _rx_guard) = tokio::sync::watch::channel(());
    let tx = Arc::new(tx);

    let rx = tx.subscribe(); // follower 在锁内先订阅
    let _ = tx.send(()); // leader 后完成

    let waited = tokio::time::timeout(Duration::from_secs(1), wait_flight(rx)).await;
    assert!(waited.is_ok(), "订阅先于 send 时 follower 必须立即返回");
}

#[tokio::test]
async fn flight_signal_closed_channel_wakes_waiter() {
    // leader panic 路径：守卫 Drop → 注册表 remove + Sender 全部释放 → 通道关闭，
    // follower 的 changed() 返回 Err(Closed) 同样必须放行（而非永久等待）。
    let (tx, _rx_guard) = tokio::sync::watch::channel(());
    let tx = Arc::new(tx);
    let rx = tx.subscribe();
    drop(tx); // 模拟 leader 终止并释放 Sender

    let waited = tokio::time::timeout(Duration::from_secs(1), wait_flight(rx)).await;
    assert!(waited.is_ok(), "通道关闭时 follower 必须立即返回");
}

#[test]
fn shard_index_stable_and_in_range() {
    // macro_support 分片索引契约：范围 [0, SF_SHARDS) 且同 key 稳定
    for key in [
        "",
        "a",
        "user:123",
        "很长很长的中文key🎯",
        &"x".repeat(1024),
    ] {
        let idx = shard_index(key);
        assert!(idx < SF_SHARDS, "key={key} shard={idx} out of range");
        assert_eq!(idx, shard_index(key), "同 key 分片必须稳定");
    }
}

#[tokio::test]
async fn get_or_follower_wakes_when_leader_completes_first() {
    // get_or follower 路径回归：leader 在 follower 注册后、
    // follower 进入等待前完成，follower 必须在超时内拿到值。
    let cache: oxcache::Cache<String, String> = oxcache::Cache::builder().build().await.unwrap();
    let cache = Arc::new(cache);

    let (leader_go_tx, leader_go_rx) = tokio::sync::oneshot::channel::<()>();

    // Leader：等 follower 注册后再完成（此时 follower 尚未进入等待/首次 poll）
    let cache_leader = cache.clone();
    let leader = tokio::spawn(async move {
        cache_leader
            .get_or(&"race-key".to_string(), || async {
                let _ = leader_go_rx.await;
                Ok("leader-value".to_string())
            })
            .await
            .unwrap()
    });

    // 给 leader 注册时间，然后放行其完成
    tokio::time::sleep(Duration::from_millis(50)).await;

    let cache_follower = cache.clone();
    let follower = tokio::spawn(async move {
        let key = "race-key".to_string();
        let fut = cache_follower.get_or(&key, || async { Ok("follower-value".to_string()) });
        tokio::time::timeout(Duration::from_secs(2), fut)
            .await
            .expect("follower 不得永久挂起（丢失唤醒竞态）")
            .unwrap()
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = leader_go_tx.send(());

    let leader_val = tokio::time::timeout(Duration::from_secs(2), leader)
        .await
        .expect("leader 不得挂起")
        .unwrap();
    let follower_val = follower.await.unwrap();

    assert_eq!(leader_val, "leader-value");
    assert_eq!(
        follower_val, "leader-value",
        "follower 应共享 leader 写入缓存的结果"
    );
}
