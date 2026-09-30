// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存审计事件流（`audit` feature）
//!
//! [`AuditEventPublisher`] 端口：get/set/delete 操作发布结构化审计事件
//! （脱敏 key + 操作 + 时间戳 + operator 元数据），满足 Repudiation 维度
//! 的审计留痕。
//!
//! - [`NoOpAuditPublisher`]：默认实现，零开销；
//! - [`InMemoryAuditPublisher`]：有界环形缓冲，测试/调试用；
//! - [`TracingAuditPublisher`]（`telemetry` feature）：桥接到 `tracing`
//!   结构化事件（供 inklog 等订阅端承接，M3 方向）；
//! - [`InklogAuditPublisher`]（`inklog` feature）：审计事件 → `inklog::LogRecord`
//!   直连注入的 `LogSink`（ConsoleSink/FileSink 等）。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::audit::{AuditEvent, AuditAction, NoOpAuditPublisher};
//!
//! let cache = Cache::builder()
//!     .audit_publisher(Arc::new(InMemoryAuditPublisher::new(1024)))
//!     .build().await?;
//! // get/set/delete 自动发布审计事件
//! ```

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 审计动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    /// 读取命中
    Hit,
    /// 读取未命中
    Miss,
    /// 写入
    Set,
    /// 删除
    Delete,
    /// 容量淘汰
    Evict,
    /// 过期移除
    Expired,
    /// 清空
    Clear,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::Hit => "hit",
            AuditAction::Miss => "miss",
            AuditAction::Set => "set",
            AuditAction::Delete => "delete",
            AuditAction::Evict => "evict",
            AuditAction::Expired => "expired",
            AuditAction::Clear => "clear",
        }
    }
}

/// 结构化审计事件
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// 操作类型
    pub action: AuditAction,
    /// 脱敏后的缓存键
    pub key: Option<String>,
    /// 命名空间（可选）
    pub namespace: Option<String>,
    /// 操作者上下文（可选，调用方注入）
    pub operator: Option<String>,
    /// 事件时间戳（毫秒）
    pub timestamp_ms: u64,
    /// 附加元数据
    pub metadata: Vec<(String, String)>,
}

impl AuditEvent {
    /// 创建审计事件
    pub fn new(action: AuditAction) -> Self {
        Self {
            action,
            key: None,
            namespace: None,
            operator: None,
            timestamp_ms: now_ms(),
            metadata: Vec::new(),
        }
    }

    /// 设置脱敏键
    pub fn with_key(mut self, redacted_key: impl Into<String>) -> Self {
        self.key = Some(redacted_key.into());
        self
    }

    /// 设置命名空间
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// 设置操作者上下文
    pub fn with_operator(mut self, operator: impl Into<String>) -> Self {
        self.operator = Some(operator.into());
        self
    }

    /// 附加元数据
    pub fn with_metadata(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.metadata.push((k.into(), v.into()));
        self
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_millis() as u64
}

/// 审计键脱敏：敏感模式掩码 + 长键截断
pub fn redact_key_for_audit(key: &str) -> String {
    const SENSITIVE: [&str; 4] = ["token", "password", "secret", "api_key"];
    const MAX_LEN: usize = 32;
    let lower = key.to_ascii_lowercase();
    if SENSITIVE.iter().any(|p| lower.contains(p)) {
        let suffix = if key.len() > 2 {
            &key[key.len() - 2..]
        } else {
            key
        };
        return format!("<sensitive>…{suffix}");
    }
    if key.len() > MAX_LEN {
        format!("{}…(len={})", &key[..MAX_LEN], key.len())
    } else {
        key.to_string()
    }
}

/// 审计事件发布端口（对象安全，可 `Arc<dyn AuditEventPublisher>` 注入）
///
/// 实现应为**非阻塞**（审计发布失败/慢不得影响主操作语义）。
pub trait AuditEventPublisher: Send + Sync + 'static {
    /// 发布一条审计事件
    fn publish(&self, event: AuditEvent);
}

/// 空实现（默认）：零开销
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpAuditPublisher;

impl AuditEventPublisher for NoOpAuditPublisher {
    fn publish(&self, _event: AuditEvent) {}
}

/// 有界环形缓冲实现（测试 / 调试快照）
pub struct InMemoryAuditPublisher {
    events: Mutex<VecDeque<AuditEvent>>,
    capacity: usize,
}

impl InMemoryAuditPublisher {
    /// 创建指定容量（上限）的内存发布器
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
            capacity: capacity.max(1),
        }
    }

    /// 当前缓冲事件快照
    pub fn snapshot(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 已缓冲事件数
    pub fn len(&self) -> usize {
        self.events.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AuditEventPublisher for InMemoryAuditPublisher {
    fn publish(&self, event: AuditEvent) {
        if let Ok(mut q) = self.events.lock() {
            if q.len() >= self.capacity {
                q.pop_front();
            }
            q.push_back(event);
        }
    }
}

/// tracing 桥（`telemetry` feature）：审计事件 → 结构化 tracing 事件
#[cfg(feature = "telemetry")]
pub struct TracingAuditPublisher;

#[cfg(feature = "telemetry")]
impl AuditEventPublisher for TracingAuditPublisher {
    fn publish(&self, event: AuditEvent) {
        tracing::info!(
            target: "oxcache::audit",
            action = event.action.as_str(),
            key = event.key.as_deref().unwrap_or(""),
            namespace = event.namespace.as_deref().unwrap_or(""),
            operator = event.operator.as_deref().unwrap_or(""),
            timestamp_ms = event.timestamp_ms,
            "cache audit event"
        );
    }
}

// ========================================================================
// inklog 结构化日志桥接（`inklog` feature）：审计事件 → inklog LogRecord，
// 经有界 channel 交由单一 writer task 顺序写入注入的 `LogSink`
// （ConsoleSink/FileSink 等落盘端）。级别映射为显式常量表：状态变更
// （Set/Delete/Clear）INFO，读探测与容量/过期清理 DEBUG。
// ========================================================================

/// 发布器内部事件通道容量（事件数）：publish 非阻塞 `try_send`，通道写满
/// 即过载丢弃——过载以显性丢弃计数呈现，绝不反压缓存操作
#[cfg(feature = "inklog")]
pub const BRIDGE_CHANNEL_CAPACITY: usize = 1024;

#[cfg(feature = "inklog")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "inklog")]
use std::sync::{Arc, Once};

/// 共享内部态：sink 引用 + 事件通道 + writer 启动控制 + 显性计数。
/// dropped/write_failures 以 `Arc<AtomicU64>` 承载，使 writer task 只持有
/// 计数面而不持有 `Arc<InklogBridgeInner>`（writer 持 Inner 会钉住通道
/// 发送端，publisher 全部释放后 writer 永远看不到通道关闭、无法优雅退出）
#[cfg(feature = "inklog")]
struct InklogBridgeInner {
    sink: Arc<dyn inklog::sink::LogSink>,
    /// 有界事件通道：publish 非阻塞 `try_send`，单 writer task 消费
    tx: tokio::sync::mpsc::Sender<inklog::LogRecord>,
    /// writer task 的接收端：首个 runtime 上下文 publish 时取出生成 writer
    rx: Mutex<Option<tokio::sync::mpsc::Receiver<inklog::LogRecord>>>,
    /// writer task 只启动一次（首个 runtime 上下文的 publish 触发）
    writer_started: Once,
    /// 显性丢弃计数：无 runtime 上下文 / 通道过载 / 通道已关闭（writer
    /// 所属 runtime 关停）/ runtime 关停时通道内滞留事件（接收端守卫清算）
    dropped: Arc<AtomicU64>,
    /// sink 写入失败次数
    write_failures: Arc<AtomicU64>,
}

/// writer task 接收端守卫：writer 所属 runtime 关停会取消 task（future
/// 连同接收端一并丢弃），Drop 中把通道内滞留事件逐条计入 dropped——
/// runtime 关停窗口的事件丢失显性化，不留静默黑洞
#[cfg(feature = "inklog")]
struct WriterRxGuard {
    rx: tokio::sync::mpsc::Receiver<inklog::LogRecord>,
    dropped: Arc<AtomicU64>,
}

#[cfg(feature = "inklog")]
impl Drop for WriterRxGuard {
    fn drop(&mut self) {
        while self.rx.try_recv().is_ok() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// inklog 结构化日志发布器（`inklog` feature）：审计事件 → `inklog::LogRecord`
///
/// 事件经有界 channel（容量 [`BRIDGE_CHANNEL_CAPACITY`]）交由单一 writer
/// task 顺序写入注入的 [`inklog::sink::LogSink`]（单消费者保序）；
/// `publish` 本身非阻塞。失败显性化（除写失败计入 `write_failure_count()`
/// 外，其余全部计入 `dropped_count()`，均不影响主操作语义）：
///
/// - 调用线程无 runtime 上下文（如 sync API 在 runtime 之外调用——该路径
///   事件无法落盘，逐条丢弃计数）；
/// - channel 已满（过载：丢弃而非阻塞调用方）；
/// - channel 已关闭（writer 所属 runtime 已关停后的后续 publish）；
/// - runtime 关停时 channel 内滞留的事件（writer 被取消时由接收端守卫清算）。
#[cfg(feature = "inklog")]
#[derive(Clone)]
pub struct InklogAuditPublisher {
    inner: Arc<InklogBridgeInner>,
}

#[cfg(feature = "inklog")]
impl InklogAuditPublisher {
    /// 创建发布器（sink 由调用方提供：ConsoleSink / FileSink / 自定义实现）。
    /// writer task 在首个 runtime 上下文的 `publish` 时惰性启动
    pub fn new(sink: Arc<dyn inklog::sink::LogSink>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(BRIDGE_CHANNEL_CAPACITY);
        Self {
            inner: Arc::new(InklogBridgeInner {
                sink,
                tx,
                rx: Mutex::new(Some(rx)),
                writer_started: Once::new(),
                dropped: Arc::new(AtomicU64::new(0)),
                write_failures: Arc::new(AtomicU64::new(0)),
            }),
        }
    }

    /// 显性丢弃的事件数（无 runtime / 通道过载 / 通道关闭 / 关停滞留清算）
    pub fn dropped_count(&self) -> u64 {
        self.inner.dropped.load(Ordering::Relaxed)
    }

    /// sink 写入失败次数
    pub fn write_failure_count(&self) -> u64 {
        self.inner.write_failures.load(Ordering::Relaxed)
    }
}

/// 审计事件 → inklog 日志记录的纯映射（级别/字段构造，无 IO）
#[cfg(feature = "inklog")]
fn audit_event_to_log_record(event: &AuditEvent) -> inklog::LogRecord {
    use serde_json::Value;

    let level = match event.action {
        // 状态变更是审计留痕主体
        AuditAction::Set | AuditAction::Delete | AuditAction::Clear => inklog::tracing::Level::INFO,
        // 读探测与容量/过期清理属诊断信息
        AuditAction::Hit | AuditAction::Miss | AuditAction::Evict | AuditAction::Expired => {
            inklog::tracing::Level::DEBUG
        }
    };
    let mut record = inklog::LogRecord::new(
        level,
        "oxcache::audit".to_string(),
        "cache audit event".to_string(),
    );
    record
        .fields
        .insert("action".to_string(), Value::from(event.action.as_str()));
    if let Some(key) = &event.key {
        record
            .fields
            .insert("key".to_string(), Value::from(key.as_str()));
    }
    if let Some(namespace) = &event.namespace {
        record
            .fields
            .insert("namespace".to_string(), Value::from(namespace.as_str()));
    }
    if let Some(operator) = &event.operator {
        record
            .fields
            .insert("operator".to_string(), Value::from(operator.as_str()));
    }
    record
        .fields
        .insert("timestamp_ms".to_string(), Value::from(event.timestamp_ms));
    for (k, v) in &event.metadata {
        record
            .fields
            .insert(format!("meta.{k}"), Value::from(v.as_str()));
    }
    record
}

#[cfg(feature = "inklog")]
impl AuditEventPublisher for InklogAuditPublisher {
    fn publish(&self, event: AuditEvent) {
        let record = audit_event_to_log_record(&event);
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // writer task 只在首个 runtime 上下文 publish 时启动一次；
                // 随其所属 runtime 关停而取消，之后的 publish 走「通道已关闭」
                // 丢弃计数
                self.inner.writer_started.call_once(|| {
                    let rx = self
                        .inner
                        .rx
                        .lock()
                        .expect("writer 接收端锁不会中毒（临界区无 panic 点）")
                        .take()
                        .expect("writer 首次启动时接收端必然在位");
                    let sink = self.inner.sink.clone();
                    let dropped = self.inner.dropped.clone();
                    let write_failures = self.inner.write_failures.clone();
                    handle.spawn(async move {
                        let mut guard = WriterRxGuard { rx, dropped };
                        while let Some(record) = guard.rx.recv().await {
                            if sink.write(&record).await.is_err() {
                                write_failures.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    });
                });
                match self.inner.tx.try_send(record) {
                    Ok(()) => {}
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_))
                    | Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        self.inner.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Err(_) => {
                self.inner.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn noop_publisher_accepts_events() {
        let publisher = NoOpAuditPublisher;
        publisher.publish(AuditEvent::new(AuditAction::Set).with_key("k"));
        publisher.publish(AuditEvent::new(AuditAction::Evict));
    }

    #[test]
    fn in_memory_publisher_ring_buffers() {
        let publisher = InMemoryAuditPublisher::new(3);
        for i in 0..5u32 {
            publisher.publish(AuditEvent::new(AuditAction::Hit).with_key(format!("k{i}")));
        }
        assert_eq!(publisher.len(), 3, "环形缓冲应保持容量上限");
        let events = publisher.snapshot();
        // 最旧的被挤掉
        assert_eq!(events[0].key.as_deref(), Some("k2"));
        assert_eq!(events[2].key.as_deref(), Some("k4"));
    }

    #[test]
    fn events_carry_action_key_timestamp_and_metadata() {
        let event = AuditEvent::new(AuditAction::Expired)
            .with_key("user:1")
            .with_namespace("users")
            .with_operator("svc-cache")
            .with_metadata("reason", "ttl");
        assert_eq!(event.action, AuditAction::Expired);
        assert_eq!(event.key.as_deref(), Some("user:1"));
        assert_eq!(event.namespace.as_deref(), Some("users"));
        assert_eq!(event.operator.as_deref(), Some("svc-cache"));
        assert_eq!(event.metadata[0], ("reason".to_string(), "ttl".to_string()));
        assert!(event.timestamp_ms > 0);
    }

    #[test]
    fn action_display_names() {
        assert_eq!(AuditAction::Hit.as_str(), "hit");
        assert_eq!(AuditAction::Miss.as_str(), "miss");
        assert_eq!(AuditAction::Evict.as_str(), "evict");
        assert_eq!(AuditAction::Expired.as_str(), "expired");
    }

    #[test]
    fn sensitive_keys_are_masked() {
        let redacted = redact_key_for_audit("user:api_key:abcdef");
        assert!(redacted.starts_with("<sensitive>"), "got {redacted}");
        assert!(!redacted.contains("abcdef"), "敏感值不得完整出现");

        let long = "k".repeat(100);
        let redacted = redact_key_for_audit(&long);
        assert!(redacted.contains("len=100"), "长键应截断并带长度标注");
        assert!(redacted.len() < 50);

        let normal = redact_key_for_audit("user:1");
        assert_eq!(normal, "user:1");
    }

    /// 对象安全：Arc<dyn AuditEventPublisher> 注入形态可用
    #[test]
    fn publisher_is_object_safe() {
        let concrete = Arc::new(InMemoryAuditPublisher::new(8));
        let publisher: Arc<dyn AuditEventPublisher> = concrete.clone();
        publisher.publish(AuditEvent::new(AuditAction::Clear));
        // dyn 端口与具体类型共享同一缓冲
        assert_eq!(concrete.len(), 1);
    }

    // ====================================================================
    // inklog 结构化日志桥接（`inklog` feature）
    // ====================================================================
    #[cfg(feature = "inklog")]
    mod inklog_bridge {
        use super::*;
        use crate::features::audit::{InklogAuditPublisher, audit_event_to_log_record};
        use inklog::sink::LogSink;
        use std::collections::HashMap;
        use std::sync::mpsc::Sender;

        /// sink 写入时捕获的字段快照（LogRecord 无 Clone，按需拷贝）
        #[derive(Debug, Clone)]
        struct Captured {
            target: String,
            level: String,
            fields: HashMap<String, serde_json::Value>,
        }

        /// 收集型测试 sink：写入完成经 channel 通知（确定性等待），可注入写失败
        struct CollectingSink {
            captured: Arc<Mutex<Vec<Captured>>>,
            done: Sender<()>,
            fail: bool,
        }

        #[async_trait::async_trait]
        impl LogSink for CollectingSink {
            async fn write(&self, record: &inklog::LogRecord) -> Result<(), inklog::InklogError> {
                if !self.fail {
                    self.captured.lock().unwrap().push(Captured {
                        target: record.target.clone(),
                        level: record.level.clone(),
                        fields: record.fields.clone(),
                    });
                }
                let _ = self.done.send(());
                if self.fail {
                    Err(inklog::InklogError::RuntimeError("sink write fault".into()))
                } else {
                    Ok(())
                }
            }

            async fn flush(&self) -> Result<(), inklog::InklogError> {
                Ok(())
            }

            async fn shutdown(&self) -> Result<(), inklog::InklogError> {
                Ok(())
            }
        }

        fn publisher_with_sink(
            fail: bool,
        ) -> (
            InklogAuditPublisher,
            Arc<Mutex<Vec<Captured>>>,
            std::sync::mpsc::Receiver<()>,
        ) {
            let captured = Arc::new(Mutex::new(Vec::new()));
            let (tx, rx) = std::sync::mpsc::channel();
            let sink = Arc::new(CollectingSink {
                captured: captured.clone(),
                done: tx,
                fail,
            });
            (InklogAuditPublisher::new(sink), captured, rx)
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn publish_forwards_audit_event_as_log_record() {
            let (publisher, captured, rx) = publisher_with_sink(false);
            let dyn_publisher: Arc<dyn AuditEventPublisher> = Arc::new(publisher.clone());

            dyn_publisher.publish(
                AuditEvent::new(AuditAction::Set)
                    .with_key("user:1")
                    .with_namespace("users")
                    .with_operator("svc-a")
                    .with_metadata("reason", "test"),
            );

            rx.recv_timeout(std::time::Duration::from_secs(5))
                .expect("写入完成信号");
            let snapshot = captured.lock().unwrap();
            assert_eq!(snapshot.len(), 1);
            let record = &snapshot[0];
            assert_eq!(record.target, "oxcache::audit");
            assert_eq!(record.level, "INFO", "写操作(Set)应映射 INFO");
            assert_eq!(
                record.fields.get("action").and_then(|v| v.as_str()),
                Some("set")
            );
            assert_eq!(
                record.fields.get("key").and_then(|v| v.as_str()),
                Some("user:1")
            );
            assert_eq!(
                record.fields.get("namespace").and_then(|v| v.as_str()),
                Some("users")
            );
            assert_eq!(
                record.fields.get("operator").and_then(|v| v.as_str()),
                Some("svc-a")
            );
            assert_eq!(
                record.fields.get("meta.reason").and_then(|v| v.as_str()),
                Some("test")
            );
            assert!(record.fields.contains_key("timestamp_ms"));
            // Arc<dyn> 端口与具体类型共享同一失败计数面
            assert_eq!(publisher.write_failure_count(), 0);
            assert_eq!(publisher.dropped_count(), 0);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn read_actions_map_to_debug_level() {
            let (publisher, captured, rx) = publisher_with_sink(false);

            publisher.publish(AuditEvent::new(AuditAction::Hit).with_key("k"));
            publisher.publish(AuditEvent::new(AuditAction::Miss).with_key("k2"));
            publisher.publish(AuditEvent::new(AuditAction::Evict).with_key("k3"));

            for _ in 0..3 {
                rx.recv_timeout(std::time::Duration::from_secs(5))
                    .expect("写入完成信号");
            }
            let snapshot = captured.lock().unwrap();
            assert_eq!(snapshot.len(), 3);
            for record in snapshot.iter() {
                assert_eq!(record.level, "DEBUG", "读探测/容量清理应映射 DEBUG");
            }
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn sink_write_failure_is_counted() {
            let (publisher, captured, rx) = publisher_with_sink(true);

            publisher.publish(AuditEvent::new(AuditAction::Set).with_key("k"));

            rx.recv_timeout(std::time::Duration::from_secs(5))
                .expect("写入尝试完成信号");
            // 失败计数由 writer task 在 sink 返回 Err 之后累加，与完成信号
            // 存在先后：限时轮询等待计数落地（上限 1s）
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
            while publisher.write_failure_count() == 0 && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert!(
                captured.lock().unwrap().is_empty(),
                "失败写入不得留下半条记录"
            );
            assert_eq!(publisher.write_failure_count(), 1, "sink 写失败应显性计数");
        }

        #[test]
        fn publish_without_runtime_is_dropped_and_counted() {
            let (publisher, captured, _rx) = publisher_with_sink(false);
            let moved = publisher.clone();
            std::thread::spawn(move || {
                // 纯 std 线程无 tokio runtime 上下文：事件显性丢弃并计数，
                // 不得 panic 或阻塞
                moved.publish(AuditEvent::new(AuditAction::Set).with_key("no-runtime"));
            })
            .join()
            .expect("无 runtime publish 不得 panic");
            assert_eq!(
                publisher.dropped_count(),
                1,
                "无 runtime 上下文的事件应计入 dropped"
            );
            assert!(captured.lock().unwrap().is_empty());
        }

        /// 闸门 sink：write 先经 `started` 通道确认已消费一条事件，再挂起
        /// 于 `gate` 闸门前——用于把 writer 精确钉在首条写入上，构造确定的
        /// 通道状态（过载灌满 / 关停滞留）
        struct GatedSink {
            started: std::sync::mpsc::Sender<()>,
            gate: Arc<tokio::sync::Notify>,
        }

        #[async_trait::async_trait]
        impl LogSink for GatedSink {
            async fn write(&self, _record: &inklog::LogRecord) -> Result<(), inklog::InklogError> {
                let _ = self.started.send(());
                self.gate.notified().await;
                Ok(())
            }

            async fn flush(&self) -> Result<(), inklog::InklogError> {
                Ok(())
            }

            async fn shutdown(&self) -> Result<(), inklog::InklogError> {
                Ok(())
            }
        }

        fn gated_publisher() -> (
            InklogAuditPublisher,
            std::sync::mpsc::Receiver<()>,
            Arc<tokio::sync::Notify>,
        ) {
            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let gate = Arc::new(tokio::sync::Notify::new());
            (
                InklogAuditPublisher::new(Arc::new(GatedSink {
                    started: started_tx,
                    gate: gate.clone(),
                })),
                started_rx,
                gate,
            )
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn channel_overload_drops_are_counted() {
            let (publisher, started_rx, gate) = gated_publisher();

            // 首条事件由 writer 消费并挂起在 sink 内：确认后再灌满通道，
            // 计数基准确定（writer 占 1 条，通道缓冲余量 = 容量）
            publisher.publish(AuditEvent::new(AuditAction::Set).with_key("first"));
            started_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("writer 应已消费首条事件并被闸门挂起");

            for _ in 0..BRIDGE_CHANNEL_CAPACITY {
                publisher.publish(AuditEvent::new(AuditAction::Miss).with_key("fill"));
            }
            let overflow = 7u64;
            for _ in 0..overflow {
                publisher.publish(AuditEvent::new(AuditAction::Miss).with_key("overflow"));
            }

            assert_eq!(
                publisher.dropped_count(),
                overflow,
                "通道满后的 publish 应显性丢弃计数而非阻塞"
            );
            assert_eq!(publisher.write_failure_count(), 0);

            // 收尾放行（挂起中的 writer 不阻塞测试进程退出）
            gate.notify_waiters();
        }

        // 普通线程测试（自管两个 runtime，不能套 #[tokio::test]）
        #[test]
        fn runtime_shutdown_losses_are_counted() {
            let (publisher, started_rx, _gate) = gated_publisher();

            let rt1 = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            rt1.block_on(async {
                publisher.publish(AuditEvent::new(AuditAction::Set).with_key("in-write"));
            });
            // 确认 writer 已消费首条事件、被钉在 sink 写入内（正在写入中的
            // 这一条随 task 取消丢失，守卫无法清算——文档化的边界）
            started_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("writer 应已被钉在闸门写入内");
            rt1.block_on(async {
                publisher.publish(AuditEvent::new(AuditAction::Set).with_key("buffered-1"));
                publisher.publish(AuditEvent::new(AuditAction::Set).with_key("buffered-2"));
            });
            // runtime 关停取消 writer：通道内 2 条滞留事件经接收端守卫清算
            // 计数，通道随之关闭
            drop(rt1);

            let rt2 = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            rt2.block_on(async {
                publisher.publish(AuditEvent::new(AuditAction::Set).with_key("after-shutdown"));
            });

            assert_eq!(
                publisher.dropped_count(),
                3,
                "滞留事件 2 条 + 通道关闭后 1 条均应显性计数"
            );
            assert_eq!(publisher.write_failure_count(), 0);
        }

        #[test]
        fn level_mapping_is_explicit_and_total() {
            // 纯函数映射全覆盖：7 个动作都有确定级别
            let cases = [
                (AuditAction::Set, "INFO"),
                (AuditAction::Delete, "INFO"),
                (AuditAction::Clear, "INFO"),
                (AuditAction::Hit, "DEBUG"),
                (AuditAction::Miss, "DEBUG"),
                (AuditAction::Evict, "DEBUG"),
                (AuditAction::Expired, "DEBUG"),
            ];
            for (action, expected) in cases {
                let record = audit_event_to_log_record(&AuditEvent::new(action).with_key("k"));
                assert_eq!(record.level, expected, "{action:?} 级别映射");
                assert_eq!(record.target, "oxcache::audit");
            }
        }

        #[test]
        fn optional_fields_omit_empty_values() {
            let record = audit_event_to_log_record(&AuditEvent::new(AuditAction::Miss));
            assert!(!record.fields.contains_key("key"));
            assert!(!record.fields.contains_key("namespace"));
            assert!(!record.fields.contains_key("operator"));
            assert_eq!(
                record.fields.get("action").and_then(|v| v.as_str()),
                Some("miss")
            );
        }
    }
}
