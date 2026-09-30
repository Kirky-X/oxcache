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

/// Generic poll-until-ready helper for Redis-protocol containers.
async fn wait_for_redis_ready(url: &str, label: &str) -> Result<(), String> {
    let client =
        redis::Client::open(url).map_err(|e| format!("创建 {} 客户端失败: {}", label, e))?;

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(30);

    while start.elapsed() < timeout {
        match client.get_multiplexed_async_connection().await {
            Ok(_) => return Ok(()),
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    Err(format!("等待 {} 就绪超时", label))
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

        let redis = GenericImage::new("redis", "7-alpine")
            .with_exposed_port(6379.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
            .start()
            .await
            .map_err(|e| format!("启动 Redis 容器失败: {}", e))?;

        let port = redis
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("获取端口失败: {}", e))?;

        Ok(Self {
            container: redis,
            port,
        })
    }

    /// 获取 Redis 连接 URL
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// 获取端口
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 等待 Redis 就绪
    pub async fn wait_ready(&self) -> Result<(), String> {
        wait_for_redis_ready(&self.url(), "Redis").await
    }
}

/// Redis Cluster 容器管理器
pub struct RedisClusterManager {
    nodes: Vec<ContainerAsync<GenericImage>>,
    ports: Vec<u16>,
}

impl RedisClusterManager {
    /// 启动 Redis Cluster (6 个节点)
    pub async fn start_cluster() -> Result<Self, String> {
        use testcontainers::core::IntoContainerPort;

        let mut nodes = Vec::new();
        let mut ports = Vec::new();

        for i in 0..6 {
            let redis = GenericImage::new("redis", "7-alpine")
                .with_exposed_port(6379.tcp())
                .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
                .start()
                .await
                .map_err(|e| format!("启动 Redis Cluster 节点 {} 失败: {}", i, e))?;

            let port = redis
                .get_host_port_ipv4(6379)
                .await
                .map_err(|e| format!("获取端口失败: {}", e))?;

            nodes.push(redis);
            ports.push(port);
        }

        Ok(Self { nodes, ports })
    }

    /// 获取所有节点的 URL
    pub fn urls(&self) -> Vec<String> {
        self.ports
            .iter()
            .map(|p| format!("redis://127.0.0.1:{}", p))
            .collect()
    }

    /// 获取端口列表
    pub fn ports(&self) -> &[u16] {
        &self.ports
    }
}

/// 测试环境管理器
pub struct TestEnvironment {
    redis: Option<RedisContainer>,
}

impl TestEnvironment {
    /// 创建新的测试环境
    pub fn new() -> Self {
        Self { redis: None }
    }

    /// 启动 Redis 容器
    pub async fn start_redis(&mut self) -> Result<&RedisContainer, String> {
        if self.redis.is_none() {
            let container = RedisContainer::start().await?;
            container.wait_ready().await?;
            self.redis = Some(container);
        }
        Ok(self.redis.as_ref().unwrap())
    }

    /// 获取 Redis URL
    pub fn redis_url(&self) -> Option<String> {
        self.redis.as_ref().map(|r| r.url())
    }
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// 容器便捷函数的宏收敛：三个 `start_*_container` 均为
/// `start → wait_ready → 组 URL` 的同构样板（仅容器类型不同），
/// 以声明式宏消除三份克隆（diting 复查 LOW-002 的结构收敛项）。
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

/// 便捷函数：检查 Redis 是否可用
pub async fn is_redis_available(url: &str) -> bool {
    let client = match redis::Client::open(url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    matches!(
        tokio::time::timeout(
            Duration::from_secs(2),
            client.get_multiplexed_async_connection()
        )
        .await,
        Ok(Ok(_))
    )
}

/// Valkey 容器包装器（使用 GenericImage）
pub struct ValkeyContainer {
    container: ContainerAsync<GenericImage>,
    port: u16,
}

impl ValkeyContainer {
    /// 启动一个 Valkey 容器
    pub async fn start() -> Result<Self, String> {
        use testcontainers::core::IntoContainerPort;

        let container = GenericImage::new("valkey/valkey", "7.2")
            .with_exposed_port(6379.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
            .start()
            .await
            .map_err(|e| format!("启动 Valkey 容器失败: {}", e))?;

        let port = container
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("获取端口失败: {}", e))?;

        Ok(Self { container, port })
    }

    /// 获取 Valkey 连接 URL（redis:// 协议，Valkey 兼容）
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// 获取端口
    pub fn port(&self) -> u16 {
        self.port
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

        let container = GenericImage::new("dragonflydb/dragonfly", "v1.27.1")
            .with_exposed_port(6379.tcp())
            .with_wait_for(WaitFor::message_on_stderr("listening on port 6379"))
            .start()
            .await
            .map_err(|e| format!("启动 Dragonfly 容器失败: {}", e))?;

        let port = container
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| format!("获取端口失败: {}", e))?;

        Ok(Self { container, port })
    }

    /// 获取 Dragonfly 连接 URL
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// 获取端口
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 等待 Dragonfly 就绪
    pub async fn wait_ready(&self) -> Result<(), String> {
        wait_for_redis_ready(&self.url(), "Dragonfly").await
    }
}
