// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! inklog 审计日志桥接示例（`inklog` feature）
//!
//! 本示例演示 oxcache 审计事件流经 `InklogAuditPublisher` 直连 inklog
//! 结构化日志 sink：
//! - `ConsoleSink` 承接 `LogRecord`（级别映射：写操作 INFO / 读探测 DEBUG）
//! - `Cache::builder().audit_publisher(...)` 一行接线，get/set/delete 自动发布
//! - 失败显性计数：`dropped_count()` / `write_failure_count()`
//!
//! 运行方式（需 `inklog-bridge` feature，见 examples/Cargo.toml）：
//! ```bash
//! cd examples && cargo run --example example_inklog_audit_bridge --features inklog-bridge
//! ```

use std::sync::Arc;
use std::time::Duration;

use inklog::ConsoleSinkConfig;
use inklog::LogSink;
use inklog::LogTemplate;
use inklog::sink::ConsoleSink;
use oxcache::features::InklogAuditPublisher;
use oxcache::i18n::messages::{
    MSG_EXAMPLE_INKLOG_BRIDGE_DROPPED, MSG_EXAMPLE_INKLOG_BRIDGE_OBSERVABILITY,
    MSG_EXAMPLE_INKLOG_BRIDGE_TITLE, MSG_EXAMPLE_INKLOG_BRIDGE_WRITE_FAILURES, t,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}\n", t(MSG_EXAMPLE_INKLOG_BRIDGE_TITLE, &[]));

    // inklog 侧：控制台 sink + 渲染模板（生产可换 FileSink / 网络sink）
    let sink = Arc::new(ConsoleSink::new(
        ConsoleSinkConfig {
            colored: false,
            ..ConsoleSinkConfig::default()
        },
        LogTemplate::new("[{level}] {target} - {message} {fields}"),
    ));

    // oxcache 侧：审计事件 → inklog LogRecord，一行接线
    let publisher = Arc::new(InklogAuditPublisher::new(sink.clone()));

    let cache = oxcache::Cache::<String, String>::builder()
        .audit_publisher(publisher.clone())
        .build()
        .await?;

    // 业务操作自动产生审计事件：set/delete 为 INFO，get 为 DEBUG
    cache
        .set(&"user:1".to_string(), &"alice".to_string())
        .await?;
    let _: Option<String> = cache.get(&"user:1".to_string()).await?;
    cache.delete(&"user:1".to_string()).await?;

    // sink 写入是异步的：给后台任务一点时间落盘后统一 flush
    tokio::time::sleep(Duration::from_millis(100)).await;
    sink.flush().await?;

    println!("\n{}", t(MSG_EXAMPLE_INKLOG_BRIDGE_OBSERVABILITY, &[]));
    println!(
        "{}",
        t(
            MSG_EXAMPLE_INKLOG_BRIDGE_DROPPED,
            &[("count", publisher.dropped_count().to_string())]
        )
    );
    println!(
        "{}",
        t(
            MSG_EXAMPLE_INKLOG_BRIDGE_WRITE_FAILURES,
            &[("count", publisher.write_failure_count().to_string())]
        )
    );

    Ok(())
}
