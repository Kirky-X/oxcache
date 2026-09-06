// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! E2E：ChainCache `event_publisher` 后端失败事件集成链（场景 ID：OXE-01/02/03）。
//!
//! 场景来源：`docs/TEST_SCENARIOS.md`「事件系统（EVT）」域。
//!
//! 既有覆盖引用声明：`src/core/events.rs` 内联测试（21 项）覆盖 `CacheEvent`
//! 构建器与 `EventPublisher` 默认方法语义（组件级）；`src/cache/chain.rs` 的
//! `emit_backend_error` 各调用点（get/set/delete/backfill）由单元测试覆盖。
//! **tests 层此前零覆盖**：publisher 配置 → 真实后端失败 → `publish_error`
//! 到达订阅方的完整链路。本文件以停止真实 Redis 容器注入故障（进程内真实
//! 实现 + 真实失败路径，非 mock），补齐三条公开 API 路径：
//!
//! - OXE-01：`set`——L2 失败 → 部分成功语义（`Ok(())`）+ 事件到达；
//! - OXE-02：`get`——L1 命中不触发 L2（对照：无新事件）；L1 miss → L2
//!   失败 → 降级为 `Ok(None)`（部分明确 miss 即不整体失败）+ 事件到达；
//! - OXE-03：`delete`——L2 失败 → 部分成功语义 + 事件到达。
//!
//! 环境依赖：Docker（testcontainers 自起 redis:7-alpine，`stop_with_timeout`
//! 注入故障，测毕自清）；Docker 不可用时静默跳过。

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};

use oxcache::backend::{CacheConnector, CacheReader, CacheWriter, MokaMemoryBackend, RedisBackend};
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::{CacheEvent, EventPublisher, OxCacheError};

/// 录制型事件订阅方：捕获 `publish_error` 到达的事件（仅用于断言）。
struct RecordingPublisher {
    errors: Mutex<Vec<(Option<String>, String)>>,
}

#[async_trait]
impl EventPublisher for RecordingPublisher {
    async fn publish(&self, _event: CacheEvent) -> Result<(), OxCacheError> {
        Ok(())
    }

    fn publish_error(&self, key: Option<String>, error: String) -> Result<(), OxCacheError> {
        self.errors
            .lock()
            .expect("recording publisher lock poisoned")
            .push((key, error));
        Ok(())
    }
}

impl RecordingPublisher {
    /// 快照当前已录制的事件（锁保护访问）。
    fn errors(&self) -> std::sync::MutexGuard<'_, Vec<(Option<String>, String)>> {
        self.errors
            .lock()
            .expect("recording publisher lock poisoned")
    }
}

/// Set up a self-managed Redis container + backend; skip if Docker unavailable.
///
/// 返回 `ContainerAsync` 以便测试内 `stop_with_timeout` 注入真实故障
/// （`RedisContainer` 封装未暴露容器句柄，故此处直接使用 testcontainers API）。
async fn setup() -> Option<(ContainerAsync<GenericImage>, RedisBackend)> {
    // SAFETY: test-only environment variable, idempotent same-value set
    unsafe { std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS") };
    let image = GenericImage::new("redis", "7-alpine")
        .with_exposed_port(6379.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"));
    let container = image.start().await.ok()?;
    let port = container.get_host_port_ipv4(6379).await.ok()?;
    let backend = RedisBackend::new(&format!("redis://127.0.0.1:{port}"))
        .await
        .ok()?;
    backend.health_check().await.ok()?;
    Some((container, backend))
}

/// OXE-01/02/03：L2 停机注入 → set/get/delete 三条公开 API 路径的事件链。
#[tokio::test]
async fn test_chain_publishes_error_events_on_backend_failure() {
    let Some((container, redis)) = setup().await else {
        return;
    };

    let recorder = Arc::new(RecordingPublisher {
        errors: Mutex::new(Vec::new()),
    });
    let chain = ChainCache::builder()
        .event_publisher(recorder.clone())
        .link(ChainLink::from_backend(
            MokaMemoryBackend::builder().capacity(128).build(),
        ))
        .link(ChainLink::from_backend(redis))
        .build();

    // bootstrap：L1(moka, score=100) + L2(redis, score=50) 全链写入成功
    chain
        .set("evt:bootstrap", b"v1".to_vec(), None)
        .await
        .expect("bootstrap set");
    assert!(
        recorder.errors().is_empty(),
        "全链成功不应产生错误事件: {:?}",
        recorder.errors()
    );

    // 故障注入：停止 L2（redis）容器
    container
        .stop_with_timeout(Some(0))
        .await
        .expect("stop redis container");

    // OXE-01：set——L1 成功 L2 失败 → 部分成功语义（Ok），事件到达
    chain
        .set("evt:set-path", b"v2".to_vec(), None)
        .await
        .expect("部分后端失败时 set 仍应成功（部分成功语义）");

    // OXE-02：get——L1 命中不触发 L2（无新事件）；L1 miss → L2 失败时
    // 降级为 Ok(None)（部分明确 miss 即不整体失败），事件仍然到达
    let cached = chain.get("evt:set-path").await.expect("L1 命中读取");
    assert_eq!(cached, Some(b"v2".to_vec()), "L1 数据应仍可读");
    let ghost = chain
        .get("evt:ghost")
        .await
        .expect("部分后端失败时 get 不应整体失败");
    assert_eq!(ghost, None, "L1 miss 且 L2 失败时应降级为 miss");

    // OXE-03：delete——L1 成功 L2 失败 → 部分成功语义，事件到达
    chain
        .delete("evt:bootstrap")
        .await
        .expect("部分后端失败时 delete 仍应成功（部分成功语义）");

    // 事件断言：三条失败路径各产生一条 publish_error，格式
    // "backend {name}: {error}"，key 均为 Some(原始键)
    let errors = recorder.errors();
    assert_eq!(
        errors.len(),
        3,
        "set/get(ghost)/delete 三条失败路径各应产生一条错误事件: {errors:?}"
    );
    for key in ["evt:set-path", "evt:ghost", "evt:bootstrap"] {
        assert!(
            errors
                .iter()
                .any(|(k, e)| k.as_deref() == Some(key) && e.contains("backend redis")),
            "应包含键 {key} 的 'backend redis' 错误事件: {errors:?}"
        );
    }
}
