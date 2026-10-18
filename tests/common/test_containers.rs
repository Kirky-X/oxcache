// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Testcontainers 测试工具
//!
//! 使用 testcontainers 0.28+ API 提供容器管理功能

// 多个测试二进制共享本模块，助手函数按二进制各有取舍，统一放行死代码告警。
#![allow(dead_code)]

use std::future::Future;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};

/// 容器门控的跳过计数（含失败短路引发的跳过），随 [TEST-SKIP] 消息输出，
/// 供从测试输出审计「真实执行数 vs 跳过数」。
///
/// 作用域为「每测试文件模块副本」：各测试文件经 `#[path] mod common` 重复包含
/// 本模块（duplicate mod 既有架构），statics 按文件各存一份——计数不跨文件聚合，
/// 失败短路也因此不跨 backend 污染（valkey 镜像缺失不会连带跳过 dragonfly 探测）；
/// 未来若收敛 mod common 为二进制级单实例，需重新评估该耦合。
static CONTAINER_GATE_SKIPS: AtomicUsize = AtomicUsize::new(0);
/// 首次容器启动失败后置位；同模块副本内后续测试直接跳过，不再重复支付启动/拉取代价。
static CONTAINER_GATE_DEAD: AtomicBool = AtomicBool::new(false);

/// OXCACHE_TEST_STRICT 置位（任意值）时容器不可用转为测试失败（fail-closed），
/// 供容器为前置条件的 CI 环境区分「环境不可用」与「静默失去覆盖」。
fn container_gate_strict() -> bool {
    std::env::var("OXCACHE_TEST_STRICT").is_ok()
}

/// 门控预算（秒）：覆盖镜像拉取与容器创建；超时消息内插同一常量防失真
const CONTAINER_GATE_TIMEOUT_SECS: u64 = 90;
/// 首次失败的真实原因，短路跳过时随消息透出，避免 CI 红灯只见短路占位符
static CONTAINER_GATE_FIRST_REASON: OnceLock<String> = OnceLock::new();

/// 基础设施类跳过原语：置失败短路闩并记录首因，打印带序号的 [TEST-SKIP]；
/// `OXCACHE_TEST_STRICT` 置位时改为 panic。容器不可用/启动超时走此入口。
pub fn gate_skip<T>(label: &str, reason: &str) -> Option<T> {
    CONTAINER_GATE_DEAD.store(true, Ordering::Relaxed);
    let first = CONTAINER_GATE_FIRST_REASON.get_or_init(|| reason.to_string());
    emit_skip(label, "container unavailable", reason, Some(first))
}

/// 后端层瞬时失败跳过原语：容器已就绪，连接/健康检查失败多为握手抖动，
/// 不置短路闩、不占用首因槽——避免一次抖动放大为整组覆盖丢失，
/// 也保持首因语义专指基础设施可用性。
pub fn backend_skip<T>(label: &str, reason: &str) -> Option<T> {
    emit_skip(label, "backend unavailable", reason, None)
}

fn emit_skip<T>(label: &str, kind: &str, reason: &str, first: Option<&str>) -> Option<T> {
    let first_note = match first {
        Some(first) if first != reason => format!(" (first failure: {first})"),
        _ => String::new(),
    };
    let n = CONTAINER_GATE_SKIPS.fetch_add(1, Ordering::Relaxed) + 1;
    let message = format!("[TEST-SKIP] {label} {kind} (gate skip #{n}): {reason}{first_note}");
    if container_gate_strict() {
        panic!("{message}; OXCACHE_TEST_STRICT is set, container-backed tests must not skip");
    }
    println!("{message}; rerun with --nocapture to see skip reasons");
    None
}

/// 容器可用性门控：`setup` 产出就绪容器则原样返回；不可用、超时或同模块副本内
/// 此前已失败时跳过。
///
/// - 预算：`setup` 整体 90 秒（覆盖镜像拉取与容器创建；`wait_ready` 内部另有
///   [`wait_for_redis_ready`] 的 30 秒就绪预算），避免 registry 慢拉取把「不可用即跳过」
///   退化成挂起；
/// - 失败短路：首次失败后同模块副本内后续探测直接跳过；
/// - `OXCACHE_TEST_STRICT` 置位时跳过改为 panic（CI 置位实现 fail-closed；
///   本地默认跳过，跳过原因需 `--nocapture` 查看）；
/// - 残留边界：90 秒超时取消 `setup` 时，若取消点落在容器已创建、
///   [`ContainerAsync`] 句柄未 construct 的窗口，daemon 侧容器无管理者清理
///   （watchdog feature 兜底信号退出，此窗口仍可能残留），长寿命开发机可偶发
///   `docker system prune`。
pub async fn container_or_skip<T>(
    label: &str,
    setup: impl Future<Output = Result<T, String>>,
) -> Option<T> {
    if CONTAINER_GATE_DEAD.load(Ordering::Relaxed) {
        return gate_skip(label, "earlier startup failure short-circuited this probe");
    }
    match tokio::time::timeout(Duration::from_secs(CONTAINER_GATE_TIMEOUT_SECS), setup).await {
        Ok(Ok(ready)) => Some(ready),
        Ok(Err(e)) => gate_skip(label, &e),
        Err(_) => gate_skip(
            label,
            &format!(
                "timed out after {CONTAINER_GATE_TIMEOUT_SECS}s \
                 (image pull or container start stalled)"
            ),
        ),
    }
}

/// testcontainers watchdog 首个注册的串行化门（进程级单例，见下方静态量说明）。
///
/// testcontainers 0.28 的 watchdog 注册表由 conquer-once 0.4 `Lazy` 承载，
/// `AsyncRunner::start` 成功创建容器后经 `watchdog::register` 触发其
/// `get_or_init`；该 Lazy 的并发初始化在上游存在竞态（testcontainers-rs#995）：
/// 初始化落地瞬间，后到线程的 `try_block` 命中 `AlreadyInit` 分支的
/// `unreachable!()`（conquer-once cell.rs:586），测试进程直接 panic。
/// Lazy 就绪后 `get_or_init` 直接读值、不再触达竞态分支，故只需把「首个成功
/// 启动落地」之前的启动串行化，代价至多一次启动的等待。
///
/// 所有 `AsyncRunner::start` 调用点（本文件三个容器包装器、docker_test_utils、
/// e2e 直连 setup）必须经本守卫启动——绕过即重新暴露竞态。
pub async fn start_with_watchdog_gate<T, E>(
    start: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    use std::sync::atomic::Ordering::{Acquire, Release};

    let ready = &crate::common::test_containers::WATCHDOG_FIRST_START_DONE;
    if ready.load(Acquire) {
        return start.await;
    }
    let _guard = crate::common::test_containers::WATCHDOG_FIRST_START_GATE
        .lock()
        .await;
    if ready.load(Acquire) {
        return start.await;
    }
    let result = start.await;
    // 仅成功置位：失败路径可能尚未走到 register，Lazy 仍未初始化（继续串行）
    if result.is_ok() {
        ready.store(true, Release);
    }
    result
}

/// 首个容器启动是否已成功落地（[`start_with_watchdog_gate`] 的快路径判据）。
///
/// 门控状态必须跨本模块副本共享：`#[path] mod common` 按测试文件复制本模块
/// （见文件顶部注释），模块内静态量随副本各存一份、挡不住跨副本并发首启；
/// 故静态量经 `crate::common::test_containers` 绝对路径取「根副本」实例
/// （测试入口文件的 `mod common` 提供），各副本共用同一份。
pub(crate) static WATCHDOG_FIRST_START_DONE: AtomicBool = AtomicBool::new(false);
/// 首个成功启动前的启动互斥（[`start_with_watchdog_gate`]）
pub(crate) static WATCHDOG_FIRST_START_GATE: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

/// Generic poll-until-ready helper for Redis-protocol containers.
async fn wait_for_redis_ready(url: &str, label: &str) -> Result<(), String> {
    let client = redis::Client::open(url)
        .map_err(|e| format!("failed to create the {} client: {}", label, e))?;

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(30);

    while start.elapsed() < timeout {
        match client.get_multiplexed_async_connection().await {
            Ok(_) => return Ok(()),
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    Err(format!("timed out waiting for {} to be ready", label))
}

/// Redis 容器包装器（使用 GenericImage）
pub struct RedisContainer {
    container: ContainerAsync<GenericImage>,
    port: u16,
}

impl RedisContainer {
    /// 启动一个新的 Redis 容器
    pub async fn start() -> Result<Self, String> {
        use testcontainers::core::IntoContainerPort;

        let redis = start_with_watchdog_gate(
            GenericImage::new("redis", "7-alpine")
                .with_exposed_port(6379.tcp())
                .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
                .start(),
        )
        .await
        .map_err(|e| format!("failed to start the Redis container: {}", e))?;

        let port = redis
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("failed to get the port: {}", e))?;

        Ok(Self {
            container: redis,
            port,
        })
    }

    /// 获取 Redis 连接 URL
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// 等待 Redis 就绪
    pub async fn wait_ready(&self) -> Result<(), String> {
        wait_for_redis_ready(&self.url(), "Redis").await
    }
}

/// 容器便捷函数的宏收敛：三个 `start_*_container` 均为
/// `start → wait_ready → 组 URL` 的同构样板（仅容器类型不同），
/// 以声明式宏消除三份克隆。
macro_rules! container_start_fn {
    ($fn_name:ident, $container_ty:ident) => {
        /// 便捷函数：启动容器并返回 `(容器句柄, 连接 URL)`
        pub async fn $fn_name() -> Result<($container_ty, String), String> {
            let container = $container_ty::start().await?;
            container.wait_ready().await?;
            let url = container.url();
            Ok((container, url))
        }
    };
}

container_start_fn!(start_redis_container, RedisContainer);
container_start_fn!(start_valkey_container, ValkeyContainer);
container_start_fn!(start_dragonfly_container, DragonflyContainer);

/// Valkey 容器包装器（使用 GenericImage）
pub struct ValkeyContainer {
    container: ContainerAsync<GenericImage>,
    port: u16,
}

impl ValkeyContainer {
    /// 启动一个 Valkey 容器
    pub async fn start() -> Result<Self, String> {
        use testcontainers::core::IntoContainerPort;

        let container = start_with_watchdog_gate(
            GenericImage::new("valkey/valkey", "7.2")
                .with_exposed_port(6379.tcp())
                .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
                .start(),
        )
        .await
        .map_err(|e| format!("failed to start the Valkey container: {}", e))?;

        let port = container
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("failed to get the port: {}", e))?;

        Ok(Self { container, port })
    }

    /// 获取 Valkey 连接 URL（redis:// 协议，Valkey 兼容）
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// 等待 Valkey 就绪
    pub async fn wait_ready(&self) -> Result<(), String> {
        wait_for_redis_ready(&self.url(), "Valkey").await
    }
}

/// Dragonfly 容器包装器（使用 GenericImage）
pub struct DragonflyContainer {
    container: ContainerAsync<GenericImage>,
    port: u16,
}

impl DragonflyContainer {
    /// 启动一个 Dragonfly 容器
    pub async fn start() -> Result<Self, String> {
        use testcontainers::core::IntoContainerPort;

        let container = start_with_watchdog_gate(
            GenericImage::new("dragonflydb/dragonfly", "v1.27.1")
                .with_exposed_port(6379.tcp())
                .with_wait_for(WaitFor::message_on_stderr("listening on port 6379"))
                .start(),
        )
        .await
        .map_err(|e| format!("failed to start the Dragonfly container: {}", e))?;

        let port = container
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("failed to get the port: {}", e))?;

        Ok(Self { container, port })
    }

    /// 获取 Dragonfly 连接 URL
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// 等待 Dragonfly 就绪
    pub async fn wait_ready(&self) -> Result<(), String> {
        wait_for_redis_ready(&self.url(), "Dragonfly").await
    }
}
