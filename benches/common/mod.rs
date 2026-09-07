// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Bench 公共辅助：后端服务可用性探测与优雅跳过。
//!
//! 基准测试依赖外部服务（Redis/Dragonfly）。服务不可用时 bench 应打印
//! 清晰的跳过原因并正常退出，而非带连接超时堆栈 panic——`cargo test
//! --all-targets` 会把 bench 当作测试目标执行，硬 panic 会让无服务环境
//! （CI/新克隆机器）的测试矩阵出现假红。

use std::time::Duration;
use tokio::runtime::Runtime;

/// 从 `redis://[user:pass@]host:port[/db]` 提取 `host:port`。
fn endpoint_of(url: &str) -> String {
    let rest = url.strip_prefix("redis://").unwrap_or(url);
    let rest = rest.split('/').next().unwrap_or(rest);
    match rest.rsplit_once('@') {
        Some((_, host)) => host.to_string(),
        None => rest.to_string(),
    }
}

/// TCP 探测服务是否可达（1 秒超时）。
fn service_available(rt: &Runtime, url: &str) -> bool {
    let endpoint = endpoint_of(url);
    rt.block_on(async move {
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::net::TcpStream::connect(&endpoint).await.is_ok()
        })
        .await
        .unwrap_or(false)
    })
}

/// 逐一探测 `(服务名, URL)`；全部可达返回 true。
/// 任一不可达时打印一次跳过原因并返回 false，bench 函数应直接 `return`。
pub fn services_ready(rt: &Runtime, services: &[(&str, &str)]) -> bool {
    let mut all_ready = true;
    let mut missing = Vec::new();
    for (name, url) in services {
        if !service_available(rt, url) {
            all_ready = false;
            missing.push(format!("{name} ({url})"));
        }
    }
    if !all_ready {
        eprintln!(
            "[bench-skip] 依赖服务不可达: {} — 按 bench 模块文档启动服务后重跑（例如 docker run -d -p 6379:6379 redis:7-alpine）",
            missing.join(", ")
        );
    }
    all_ready
}
