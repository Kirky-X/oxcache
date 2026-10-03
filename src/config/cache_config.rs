// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 统一缓存配置中枢（`CacheConfig`）
//!
//! 以单一结构承载缓存构建的全量可配置参数，并提供三条配置通路：
//!
//! 1. **程序化构建**：`CacheConfig::builder()` 链式赋值；
//! 2. **环境变量构建**：[`CacheConfig::try_from_env()`] 按下表读取 `OXCACHE_*`，
//!    未设置的键回落 `None`（即底层构建器默认行为，零行为漂移）；
//! 3. **confers 配置源**：`config-confers` feature 下
//!    [`CacheConfig::try_from_confers()`] 从
//!    [`OxcacheConfig`](crate::features::confers_config::OxcacheConfig) 快照映射，
//!    键约定见该模块文档表。
//!
//! # 环境变量约定
//!
//! | 环境变量 | 类型 | 映射字段 |
//! | --- | --- | --- |
//! | `OXCACHE_CAPACITY` | u64 | `capacity` |
//! | `OXCACHE_TTL_MS` | u64（毫秒） | `ttl` |
//! | `OXCACHE_TTI_MS` | u64（毫秒） | `tti` |
//! | `OXCACHE_NULL_CACHE_TTL_MS` | u64（毫秒） | `null_cache_ttl` |
//! | `OXCACHE_TTL_JITTER_FACTOR` | f64 | `ttl_jitter_factor` |
//! | `OXCACHE_SYNC_MODE` | bool | `sync_mode` |
//! | `OXCACHE_BACKEND` | 枚举串 | `backend` |
//! | `OXCACHE_METRICS` | bool | `metrics_enabled` |
//! | `OXCACHE_SERIALIZATION_FORMAT` | 枚举串 | `serialization_format` |
//! | `OXCACHE_REDIS_URL` | 字符串 | `redis_url` |
//! | `OXCACHE_DISK_PATH` | 字符串 | `disk_path` |
//! | `OXCACHE_CONNECTION_POOL_SIZE` | usize | `connection_pool_size`（≥ 1） |
//! | `OXCACHE_CIRCUIT_BREAKER_FAILURE_THRESHOLD` | u64 | `circuit_breaker_failure_threshold` |
//! | `OXCACHE_CIRCUIT_BREAKER_RESET_TIMEOUT_MS` | u64（毫秒） | `circuit_breaker_reset_timeout` |
//!
//! 布尔值接受 `true/1/yes/on` 与 `false/0/no/off`（大小写不敏感），其余值显性报错。
//! 数值解析失败同样显性报错（含变量名与原始值），绝不静默回落默认。
//!
//! `backend` 取值：`moka` / `dashmap` / `redis` / `valkey` / `dragonfly` /
//! `aerospike` / `chain` / `mock` / `disk` / `unknown`（大小写不敏感）。
//! 其中 `valkey`（后端实现缺失）、`chain`（须经
//! [`ChainBuilder`](crate::cache::ChainBuilder) 组装）、`aerospike`（需程序化
//! namespace/set 配置）与 `unknown` 在 [`CacheConfig::validate()`] 中一律拒绝；
//! 其余取值要求对应 feature 已启用（如 `redis` 需 `redis` feature）。

use crate::error::{OxCacheError, OxCacheResult};
#[cfg(not(feature = "metrics"))]
use crate::i18n::messages::MSG_DETAIL_CONFIG_METRICS_FEATURE;
#[cfg(all(
    any(feature = "serialization", feature = "full"),
    not(feature = "serde-bincode")
))]
use crate::i18n::messages::MSG_DETAIL_CONFIG_SERIALIZATION_BINCODE_REQUIRES_FEATURE;
#[cfg(any(feature = "serialization", feature = "full"))]
use crate::i18n::messages::MSG_DETAIL_CONFIG_SERIALIZATION_INVALID_FORMAT;
#[cfg(all(
    any(feature = "serialization", feature = "full"),
    not(feature = "postcard")
))]
use crate::i18n::messages::MSG_DETAIL_CONFIG_SERIALIZATION_POSTCARD_REQUIRES_FEATURE;
#[cfg(not(any(feature = "memory", feature = "redis", feature = "disk")))]
use crate::i18n::messages::{
    MSG_DETAIL_CONFIG_BACKEND_FEATURES, MSG_DETAIL_CONFIG_ENV_BACKEND_FEATURES,
};
use crate::i18n::messages::{
    MSG_DETAIL_CONFIG_CAPACITY_EXCEEDS_USIZE, MSG_DETAIL_CONFIG_CAPACITY_ZERO,
    MSG_DETAIL_CONFIG_CB_THRESHOLD_ZERO, MSG_DETAIL_CONFIG_ENV_INVALID_VALUE,
    MSG_DETAIL_CONFIG_POOL_SIZE_ZERO, MSG_DETAIL_CONFIG_SERVICE_NAME_EMPTY,
    MSG_DETAIL_CONFIG_TTL_ZERO, t,
};
#[cfg(not(any(feature = "serialization", feature = "full")))]
use crate::i18n::messages::{
    MSG_DETAIL_CONFIG_ENV_SERIALIZATION_FEATURE, MSG_DETAIL_CONFIG_SERIALIZATION_FEATURE,
};
use std::time::Duration;

/// 环境变量统一前缀
pub const ENV_PREFIX: &str = "OXCACHE_";

// 约定键名统一由此引用（读取点与测试清单共用）；前缀契约由测试
// `env_keys_share_prefix` 锁定，避免字面量与 `ENV_PREFIX` 漂移
pub(crate) const KEY_CAPACITY: &str = "OXCACHE_CAPACITY";
pub(crate) const KEY_TTL_MS: &str = "OXCACHE_TTL_MS";
pub(crate) const KEY_TTI_MS: &str = "OXCACHE_TTI_MS";
pub(crate) const KEY_NULL_CACHE_TTL_MS: &str = "OXCACHE_NULL_CACHE_TTL_MS";
pub(crate) const KEY_TTL_JITTER_FACTOR: &str = "OXCACHE_TTL_JITTER_FACTOR";
pub(crate) const KEY_SYNC_MODE: &str = "OXCACHE_SYNC_MODE";
pub(crate) const KEY_BACKEND: &str = "OXCACHE_BACKEND";
pub(crate) const KEY_METRICS: &str = "OXCACHE_METRICS";
pub(crate) const KEY_SERIALIZATION_FORMAT: &str = "OXCACHE_SERIALIZATION_FORMAT";
pub(crate) const KEY_REDIS_URL: &str = "OXCACHE_REDIS_URL";
pub(crate) const KEY_DISK_PATH: &str = "OXCACHE_DISK_PATH";
pub(crate) const KEY_CONNECTION_POOL_SIZE: &str = "OXCACHE_CONNECTION_POOL_SIZE";
pub(crate) const KEY_CB_FAILURE_THRESHOLD: &str = "OXCACHE_CIRCUIT_BREAKER_FAILURE_THRESHOLD";
pub(crate) const KEY_CB_RESET_TIMEOUT_MS: &str = "OXCACHE_CIRCUIT_BREAKER_RESET_TIMEOUT_MS";
pub(crate) const KEY_SERVICE_NAME: &str = "OXCACHE_SERVICE_NAME";

/// 统一缓存配置
///
/// 字段全部 `Option`：`None` = 未配置，底层构建器沿用自身默认，
/// 保证「env 未设置时行为不变」。
#[derive(Clone, Default, PartialEq)]
pub struct CacheConfig {
    /// L1 容量（条目数）
    pub capacity: Option<u64>,
    /// 默认 TTL
    pub ttl: Option<Duration>,
    /// 默认 TTI（idle 过期）
    pub tti: Option<Duration>,
    /// 穿透防护空值缓存 TTL
    pub null_cache_ttl: Option<Duration>,
    /// TTL 抖动因子（底层构建器 clamp 到 `0.0..=1.0`，NaN 视为 0）
    pub ttl_jitter_factor: Option<f64>,
    /// 同步 API 模式（与 `backend` 可组合：Moka/DashMap 双面皆原生——async
    /// 面运行时无关、sync 面直连；其余后端 sync 面经 `AsyncToSyncBridge`
    /// 桥出、调用期要求多线程 runtime）
    pub sync_mode: Option<bool>,
    /// 后端类型原始串（moka/dashmap/redis/...；`None` = 未选择，由构建器自定）
    ///
    /// 存原始串而非 [`BackendKind`](crate::backend::BackendKind)：`backend`
    /// 模块整体受 lib.rs 的 cfg(any(memory,redis,disk)) 门控，窄 feature 组合
    /// 下该路径不存在，配置中枢须在任何 feature 组合下可用；解析与可用性
    /// 检查统一延迟到 [`CacheConfig::validate()`] / build 时显性完成。
    pub backend: Option<String>,
    /// 指标开关：`Some(false)` 注入 `NoOpMetricsRecorder` 恢复静默；
    /// `Some(true)` / `None` 沿用默认全局 recorder
    pub metrics_enabled: Option<bool>,
    /// 序列化传输格式原始串（json/bincode/postcard；`None` = 未设置）
    ///
    /// 与 `backend` 同理存原始串，解析延迟到 validate / apply 时显性完成。
    pub serialization_format: Option<String>,
    /// Redis 兼容后端连接串（`backend` ∈ {Redis, Valkey, Dragonfly} 时必填）
    ///
    /// 连接串可含 `redis://用户名:密码@host` 形式的凭证：[`Debug`] 实现对其脱敏
    /// 为 `"***"`，禁止以其他方式明文落日志。
    pub redis_url: Option<String>,
    /// 磁盘后端数据文件路径（`backend == Disk` 时必填）
    pub disk_path: Option<String>,
    /// 连接池大小（Redis / Dragonfly 分布式后端消费，缺省 8）
    pub connection_pool_size: Option<usize>,
    /// 熔断器连续失败阈值（当前由 Redis 后端构建消费）
    pub circuit_breaker_failure_threshold: Option<u32>,
    /// 熔断器 Open → HalfOpen 恢复超时（当前由 Redis 后端构建消费）
    pub circuit_breaker_reset_timeout: Option<Duration>,
    /// R9 service 维度标签（空串在 validate 期显性拒绝；装配依赖 metrics feature）
    pub service_name: Option<String>,
}

// redis_url 可能携带凭证，手动实现 Debug 脱敏，防止连接串明文进日志
impl std::fmt::Debug for CacheConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheConfig")
            .field("capacity", &self.capacity)
            .field("ttl", &self.ttl)
            .field("tti", &self.tti)
            .field("null_cache_ttl", &self.null_cache_ttl)
            .field("ttl_jitter_factor", &self.ttl_jitter_factor)
            .field("sync_mode", &self.sync_mode)
            .field("backend", &self.backend)
            .field("metrics_enabled", &self.metrics_enabled)
            .field("serialization_format", &self.serialization_format)
            .field("redis_url", &redact_url(self.redis_url.as_deref()))
            .field("disk_path", &self.disk_path)
            .field("connection_pool_size", &self.connection_pool_size)
            .field(
                "circuit_breaker_failure_threshold",
                &self.circuit_breaker_failure_threshold,
            )
            .field(
                "circuit_breaker_reset_timeout",
                &self.circuit_breaker_reset_timeout,
            )
            .field("service_name", &self.service_name)
            .finish()
    }
}

/// 连接串脱敏：存在即显示 `"***"`，不泄漏任何片段
fn redact_url(url: Option<&str>) -> Option<&'static str> {
    url.map(|_| "***")
}

impl CacheConfig {
    /// 创建构建器
    pub fn builder() -> CacheConfigBuilder {
        CacheConfigBuilder::default()
    }

    /// 从环境变量构建配置
    ///
    /// 仅读取 [`ENV_PREFIX`] 下的约定键（见模块文档表）；未设置的键为 `None`。
    /// 解析失败返回 `OxCacheError::InvalidInput` 并附变量名与原始值。
    pub fn try_from_env() -> OxCacheResult<Self> {
        let mut config = Self::default();

        if let Some(raw) = env_value(KEY_CAPACITY)? {
            config.capacity = Some(
                raw.parse::<u64>()
                    .map_err(invalid_value(KEY_CAPACITY, &raw))?,
            );
        }
        if let Some(raw) = env_value(KEY_TTL_MS)? {
            let ms = raw
                .parse::<u64>()
                .map_err(invalid_value(KEY_TTL_MS, &raw))?;
            config.ttl = Some(Duration::from_millis(ms));
        }
        if let Some(raw) = env_value(KEY_TTI_MS)? {
            let ms = raw
                .parse::<u64>()
                .map_err(invalid_value(KEY_TTI_MS, &raw))?;
            config.tti = Some(Duration::from_millis(ms));
        }
        if let Some(raw) = env_value(KEY_NULL_CACHE_TTL_MS)? {
            let ms = raw
                .parse::<u64>()
                .map_err(invalid_value(KEY_NULL_CACHE_TTL_MS, &raw))?;
            config.null_cache_ttl = Some(Duration::from_millis(ms));
        }
        if let Some(raw) = env_value(KEY_TTL_JITTER_FACTOR)? {
            let factor = raw
                .parse::<f64>()
                .map_err(invalid_value(KEY_TTL_JITTER_FACTOR, &raw))?;
            config.ttl_jitter_factor = Some(factor);
        }
        if let Some(raw) = env_value(KEY_SYNC_MODE)? {
            config.sync_mode = Some(parse_bool_value(KEY_SYNC_MODE, &raw)?);
        }
        if let Some(raw) = env_value(KEY_BACKEND)? {
            #[cfg(not(any(feature = "memory", feature = "redis", feature = "disk")))]
            {
                return Err(OxCacheError::InvalidInput(t(
                    MSG_DETAIL_CONFIG_ENV_BACKEND_FEATURES,
                    &[
                        ("key", KEY_BACKEND.to_string()),
                        ("raw", format!("{raw:?}")),
                    ],
                )));
            }
            #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
            {
                parse_backend_kind(KEY_BACKEND, &raw)?;
                config.backend = Some(raw.to_ascii_lowercase());
            }
        }
        if let Some(raw) = env_value(KEY_METRICS)? {
            config.metrics_enabled = Some(parse_bool_value(KEY_METRICS, &raw)?);
        }
        #[cfg(not(any(feature = "serialization", feature = "full")))]
        if let Some(raw) = env_value(KEY_SERIALIZATION_FORMAT)? {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_CONFIG_ENV_SERIALIZATION_FEATURE,
                &[
                    ("key", KEY_SERIALIZATION_FORMAT.to_string()),
                    ("raw", format!("{raw:?}")),
                ],
            )));
        }
        #[cfg(any(feature = "serialization", feature = "full"))]
        if let Some(raw) = env_value(KEY_SERIALIZATION_FORMAT)? {
            parse_serialization_format(KEY_SERIALIZATION_FORMAT, &raw)?;
            config.serialization_format = Some(raw.to_ascii_lowercase());
        }
        if let Some(raw) = env_value(KEY_REDIS_URL)? {
            config.redis_url = Some(raw);
        }
        if let Some(raw) = env_value(KEY_DISK_PATH)? {
            config.disk_path = Some(raw);
        }
        if let Some(raw) = env_value(KEY_CONNECTION_POOL_SIZE)? {
            let pool = raw
                .parse::<usize>()
                .map_err(invalid_value(KEY_CONNECTION_POOL_SIZE, &raw))?;
            config.connection_pool_size = Some(pool);
        }
        if let Some(raw) = env_value(KEY_CB_FAILURE_THRESHOLD)? {
            let threshold = raw
                .parse::<u32>()
                .map_err(invalid_value(KEY_CB_FAILURE_THRESHOLD, &raw))?;
            config.circuit_breaker_failure_threshold = Some(threshold);
        }
        if let Some(raw) = env_value(KEY_CB_RESET_TIMEOUT_MS)? {
            let ms = raw
                .parse::<u64>()
                .map_err(invalid_value(KEY_CB_RESET_TIMEOUT_MS, &raw))?;
            config.circuit_breaker_reset_timeout = Some(Duration::from_millis(ms));
        }
        if let Some(raw) = env_value(KEY_SERVICE_NAME)? {
            config.service_name = Some(raw);
        }
        Ok(config)
    }

    /// 配置一致性检查
    ///
    /// 覆盖值域约束、参数组合约束与当前构建 feature 可用性三类：
    ///
    /// - 值域：`capacity`/`circuit_breaker_failure_threshold`/`connection_pool_size`
    ///   必须 > 0 且 `capacity` 不得超出目标平台 `usize` 范围（跨平台防静默截断）；
    ///   `ttl`/`tti`/`null_cache_ttl` 不得为 `Duration::ZERO`（永不过期用 `None` 表达）；
    /// - 组合：`backend` ∈ {Redis, Valkey, Dragonfly} 时 `redis_url` 必填非空；
    ///   `backend == Disk` 时 `disk_path` 必填非空；`sync_mode == Some(true)`
    ///   与 `backend` 可组合（Moka/DashMap 经槽位注入走原生同步面，其余后端
    ///   sync API 桥出、调用期要求多线程 runtime，见
    ///   [`CacheBuilder::build`](crate::cache::CacheBuilder)）；
    /// - feature：未启用 `metrics` 时配置 `metrics_enabled`、未启用
    ///   `serialization` 时配置 `serialization_format`、未启用 `memory`/`redis`/
    ///   `disk` 任一时配置 `backend` 均显性报错；
    /// - feature：`Moka`/`DashMap`/`Mock` 需 `memory`，`Redis` 需 `redis`，
    ///   `Dragonfly` 需 `dragonfly`，`Disk` 需 `disk`，`Aerospike` 需 `aerospike`；
    ///   `Valkey`（无后端实现）、`Chain`（经 `ChainBuilder` 组装）、`Unknown`
    ///   一律不可经本配置构建。
    ///
    /// 来源说明：本检查对 env/confers/程序化 builder 三条通路共用，无法得知
    /// 配置实际来源，错误消息只报字段名，不伪装成环境变量名；env 与 confers
    /// 解析层的报错各自携带 `OXCACHE_*` 变量名 / `cache.*` 键名的精确定位。
    pub fn validate(&self) -> OxCacheResult<()> {
        // backend 原始串在此统一解析；kind 级约束与 feature 可用性检查见下。
        // 窄 feature 组合下 `backend` 模块不编译，kind 解析不可用——配置了
        // backend 必须显性失败，而非静默忽略。
        #[cfg(not(any(feature = "memory", feature = "redis", feature = "disk")))]
        {
            if let Some(raw) = self.backend.as_deref() {
                let _ = raw;
                return Err(OxCacheError::InvalidInput(t(
                    MSG_DETAIL_CONFIG_BACKEND_FEATURES,
                    &[("raw", format!("{raw:?}"))],
                )));
            }
        }
        #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
        let parsed_backend = match self.backend.as_deref() {
            None => None,
            Some(raw) => Some(parse_backend_kind("backend", raw)?),
        };
        #[cfg(any(feature = "serialization", feature = "full"))]
        {
            // 序列化格式解析：json 恒可用，bincode/postcard 缺 feature 时显性报错
            if let Some(raw) = self.serialization_format.as_deref() {
                parse_serialization_format("serialization_format", raw)?;
            }
        }
        if let Some(capacity) = self.capacity {
            if capacity == 0 {
                return Err(OxCacheError::InvalidInput(t(
                    MSG_DETAIL_CONFIG_CAPACITY_ZERO,
                    &[],
                )));
            }
            // 32 位目标上 u64 容量会静默截断，超界直接显性拒绝
            if capacity > usize::MAX as u64 {
                return Err(OxCacheError::InvalidInput(t(
                    MSG_DETAIL_CONFIG_CAPACITY_EXCEEDS_USIZE,
                    &[
                        ("capacity", capacity.to_string()),
                        ("max", usize::MAX.to_string()),
                    ],
                )));
            }
        }
        for (name, ttl) in [
            ("ttl", self.ttl),
            ("tti", self.tti),
            ("null_cache_ttl", self.null_cache_ttl),
        ] {
            if ttl == Some(Duration::ZERO) {
                return Err(OxCacheError::InvalidInput(t(
                    MSG_DETAIL_CONFIG_TTL_ZERO,
                    &[("name", name.to_string())],
                )));
            }
        }
        #[cfg(not(feature = "metrics"))]
        if self.metrics_enabled.is_some() {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_CONFIG_METRICS_FEATURE,
                &[],
            )));
        }
        #[cfg(not(any(feature = "serialization", feature = "full")))]
        if self.serialization_format.is_some() {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_CONFIG_SERIALIZATION_FEATURE,
                &[],
            )));
        }
        if self.circuit_breaker_failure_threshold == Some(0) {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_CONFIG_CB_THRESHOLD_ZERO,
                &[],
            )));
        }
        if self.service_name.as_deref() == Some("") {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_CONFIG_SERVICE_NAME_EMPTY,
                &[],
            )));
        }
        if self.connection_pool_size == Some(0) {
            return Err(OxCacheError::InvalidInput(t(
                MSG_DETAIL_CONFIG_POOL_SIZE_ZERO,
                &[],
            )));
        }

        #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
        {
            use crate::backend::BackendKind;
            match parsed_backend {
                None => {}
                Some(BackendKind::Moka) | Some(BackendKind::DashMap) => {
                    #[cfg(not(feature = "memory"))]
                    return Err(OxCacheError::InvalidInput(
                        "backend requires the `memory` feature, which is not enabled in this build"
                            .to_string(),
                    ));
                }
                Some(BackendKind::Mock) => {
                    // MockBackend 仅存在于测试构建（memory 模块 #[cfg(test)]）
                    #[cfg(not(test))]
                    return Err(OxCacheError::InvalidInput(
                        "backend `mock` only exists in test builds and is not available here"
                            .to_string(),
                    ));
                    #[cfg(test)]
                    {
                        #[cfg(not(feature = "memory"))]
                    return Err(OxCacheError::InvalidInput(
                        "backend requires the `memory` feature, which is not enabled in this build"
                            .to_string(),
                    ));
                    }
                }
                Some(BackendKind::Redis) => {
                    #[cfg(not(feature = "redis"))]
                return Err(OxCacheError::InvalidInput(
                    "backend `redis` requires the `redis` feature, which is not enabled in this build"
                        .to_string(),
                ));
                    #[cfg(feature = "redis")]
                    self.require_non_empty("redis_url", self.redis_url.as_deref())?;
                }
                Some(BackendKind::Dragonfly) => {
                    #[cfg(not(feature = "dragonfly"))]
                return Err(OxCacheError::InvalidInput(
                    "backend `dragonfly` requires the `dragonfly` feature, which is not enabled in this build"
                        .to_string(),
                ));
                    #[cfg(feature = "dragonfly")]
                    self.require_non_empty("redis_url", self.redis_url.as_deref())?;
                }
                Some(BackendKind::Disk) => {
                    #[cfg(not(feature = "disk"))]
                return Err(OxCacheError::InvalidInput(
                    "backend `disk` requires the `disk` feature, which is not enabled in this build"
                        .to_string(),
                ));
                    #[cfg(feature = "disk")]
                    self.require_non_empty("disk_path", self.disk_path.as_deref())?;
                }
                Some(BackendKind::Aerospike) => {
                    #[cfg(not(feature = "aerospike"))]
                return Err(OxCacheError::InvalidInput(
                    "backend `aerospike` requires the `aerospike` feature, which is not enabled in this build"
                        .to_string(),
                ));
                    #[cfg(feature = "aerospike")]
                return Err(OxCacheError::InvalidInput(
                    "backend `aerospike` needs namespace/set configuration and must be built programmatically, not via CacheConfig"
                        .to_string(),
                ));
                }
                // Valkey: BackendKind variant exists but no backend implementation ships
                Some(BackendKind::Valkey) => {
                    return Err(OxCacheError::InvalidInput(
                    "backend `valkey` has no implementation; use `redis` (protocol-compatible) or `dragonfly`"
                        .to_string(),
                ));
                }
                Some(BackendKind::Chain) => {
                    return Err(OxCacheError::InvalidInput(
                    "backend `chain` must be assembled via ChainBuilder, not a single CacheConfig backend"
                        .to_string(),
                ));
                }
                Some(kind @ BackendKind::Unknown) => {
                    return Err(OxCacheError::InvalidInput(format!(
                        "backend `{kind:?}` cannot be built from configuration"
                    )));
                }
            }
        }

        // sync_mode 与 backend 可组合：有原生同步面的后端经槽位注入直连，
        // 其余后端 sync 面桥出（多线程 runtime 要求在调用期显性报错）
        Ok(())
    }

    /// 按配置构建后端实例（async 面句柄）
    ///
    /// 返回 `None` 表示未配置 `backend`（调用方自行选择后端）。
    /// 构建前先行 [`CacheConfig::validate()`]；[`apply_to_cache_builder`](Self::apply_to_cache_builder)
    /// 已先验过，此处再次校验是公开入口的独立契约（有意双重校验，二者以
    /// validate 为单一事实源）。
    ///
    /// 注意：`Disk` 分支含阻塞文件 I/O（redb 打开/建库，耗时可达毫秒到秒级），
    /// 仅限启动期调用，不得用于请求路径。
    ///
    /// 有原生同步面的后端（Moka/DashMap）经私有方法 `build_backend_slot`
    /// 保留同步面，且 Dual 槽直接返回原生 async 面（运行时无关，零交接税）；
    /// 仅 sync 一等入口的场景经 `SyncBackendAdapter` 门面呈现 async 面
    ///（门面的 async 方法完成于后端同步面，runtime 要求随后端）。
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    pub async fn build_backend(
        &self,
    ) -> OxCacheResult<Option<std::sync::Arc<dyn crate::backend::CacheBackend>>> {
        Ok(match self.build_backend_slot().await? {
            None => None,
            Some(crate::cache::builder::cache_builder::BackendSlot::Async(backend)) => {
                Some(backend)
            }
            Some(crate::cache::builder::cache_builder::BackendSlot::Sync(sync_backend)) => Some(
                std::sync::Arc::new(crate::backend::SyncBackendAdapter::new(sync_backend)),
            ),
            Some(crate::cache::builder::cache_builder::BackendSlot::Dual {
                async_face, ..
            }) => Some(async_face),
        })
    }

    /// 按配置构建后端槽位（保留原生同步面信息）
    ///
    /// 与 [`Self::build_backend`] 同一构建逻辑，但以
    /// [`BackendSlot`](crate::cache::builder::cache_builder::BackendSlot)
    /// 表达：有原生同步面的后端（Moka/DashMap）产 `Dual` 槽（同一具体
    /// Arc 的两次 coerce，async/sync 两面皆原生，零桥接税）；仅 async 面的
    /// 后端（Redis/Dragonfly/Disk/Mock）产 `Async` 槽，sync 面经桥接呈现。
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    pub(crate) async fn build_backend_slot(
        &self,
    ) -> OxCacheResult<Option<crate::cache::builder::cache_builder::BackendSlot>> {
        use crate::cache::builder::cache_builder::BackendSlot;
        self.validate()?;
        let backend = match self.backend.as_deref() {
            None => return Ok(None),
            Some(raw) => parse_backend_kind("backend", raw)?,
        };
        use crate::backend::BackendKind;
        let slot = match backend {
            BackendKind::Moka => {
                #[cfg(not(feature = "memory"))]
                unreachable!("validate rejects Moka without the memory feature");
                #[cfg(feature = "memory")]
                {
                    let mut builder = crate::backend::MokaMemoryBackend::builder()
                        .capacity(self.capacity.unwrap_or(10_000));
                    if let Some(ttl) = self.ttl {
                        builder = builder.ttl(ttl);
                    }
                    if let Some(tti) = self.tti {
                        builder = builder.time_to_idle(tti);
                    }
                    // 同一具体 Arc 双向 coerce：async 面保持原生 future
                    //（运行时无关），sync 面原生直连——不走任何桥接
                    let moka: std::sync::Arc<crate::backend::MokaMemoryBackend> =
                        std::sync::Arc::new(builder.build());
                    BackendSlot::Dual {
                        async_face: moka.clone(),
                        sync_face: moka,
                    }
                }
            }
            BackendKind::DashMap => {
                #[cfg(not(feature = "memory"))]
                unreachable!("validate rejects DashMap without the memory feature");
                #[cfg(feature = "memory")]
                {
                    let mut builder = crate::backend::DashMapMemoryBackend::builder();
                    if let Some(capacity) = self.capacity {
                        builder = builder.capacity(capacity as usize);
                    }
                    if let Some(ttl) = self.ttl {
                        builder = builder.default_ttl(ttl);
                    }
                    let dashmap: std::sync::Arc<crate::backend::DashMapMemoryBackend> =
                        std::sync::Arc::new(builder.build());
                    BackendSlot::Dual {
                        async_face: dashmap.clone(),
                        sync_face: dashmap,
                    }
                }
            }
            BackendKind::Mock => {
                // validate 已拦截所有非法组合；arm 主体仅测试构建 + memory 下编译，
                // 其余组合由类型为 `!` 的兜底表达式填充（不依赖外部 feature 隐含关系）。
                // MockBackend 无原生 sync 读/写面（仅 SyncAtomicCacheWriter），
                // 进不了 Sync 槽，保持 Async 槽桥接
                #[cfg(all(test, feature = "memory"))]
                {
                    BackendSlot::Async(std::sync::Arc::new(
                        crate::backend::memory::MockBackend::new("cache-config-mock", 100, false),
                    ))
                }
                #[cfg(not(all(test, feature = "memory")))]
                unreachable!("validate rejects Mock outside test builds with memory")
            }
            BackendKind::Redis => {
                #[cfg(not(feature = "redis"))]
                unreachable!("validate rejects Redis without the redis feature");
                #[cfg(feature = "redis")]
                {
                    let url = self.redis_url.as_deref().unwrap_or_default();
                    let mut builder = crate::backend::RedisBackend::builder();
                    builder = builder.connection_string(url);
                    if let Some(pool) = self.connection_pool_size {
                        builder = builder.pool_size(pool);
                    }
                    if let Some(threshold) = self.circuit_breaker_failure_threshold {
                        builder = builder.circuit_breaker_threshold(threshold);
                    }
                    if let Some(timeout) = self.circuit_breaker_reset_timeout {
                        builder = builder.circuit_breaker_reset_timeout(timeout);
                    }
                    // Redis 虽有同步面，但那是 block_in_place 桥接自身 async 面
                    // 的产物：进 Sync 槽会让 async API 变成 async→sync→async
                    // 双重跳，保持 Async 槽（sync 面单次桥接）
                    BackendSlot::Async(std::sync::Arc::new(builder.build().await?))
                }
            }
            BackendKind::Dragonfly => {
                #[cfg(not(feature = "dragonfly"))]
                unreachable!("validate rejects Dragonfly without the dragonfly feature");
                #[cfg(feature = "dragonfly")]
                {
                    let url = self.redis_url.as_deref().unwrap_or_default();
                    let pool = self.connection_pool_size.unwrap_or(8);
                    BackendSlot::Async(std::sync::Arc::new(
                        crate::backend::DragonflyBackend::new(url, pool).await?,
                    ))
                }
            }
            BackendKind::Disk => {
                #[cfg(not(feature = "disk"))]
                unreachable!("validate rejects Disk without the disk feature");
                #[cfg(feature = "disk")]
                {
                    let path = self.disk_path.as_deref().unwrap_or_default();
                    let disk = match crate::backend::disk::RedbDiskBackend::open(path) {
                        Ok(disk) => disk,
                        Err(open_err) => crate::backend::disk::RedbDiskBackend::create(path)
                            .map_err(|create_err| {
                                OxCacheError::Operation(format!(
                                    "disk backend open failed ({open_err}) and create failed ({create_err}): {path}"
                                ))
                            })?,
                    };
                    let disk = match self.ttl {
                        Some(ttl) => disk.with_default_ttl(ttl),
                        None => disk,
                    };
                    BackendSlot::Async(std::sync::Arc::new(disk))
                }
            }
            other => {
                return Err(OxCacheError::NotSupported(format!(
                    "backend `{other:?}` cannot be built via CacheConfig (rejected by validate or requires programmatic assembly)"
                )));
            }
        };
        Ok(Some(slot))
    }

    /// 将配置应用到 [`CacheBuilder`](crate::cache::CacheBuilder)
    ///
    /// 覆盖全部可映射字段；`backend` 已配置时按槽位注入——Moka/DashMap 走
    /// `Dual` 槽（同一具体后端的双面原生 coerce：async 面运行时无关，
    /// `sync_mode(true)` 下 sync API 原生直连），Redis/Dragonfly/Disk/Mock
    /// 走 `backend_arc`（sync 面经 `AsyncToSyncBridge` 桥出，调用期要求
    /// 多线程 runtime）。若调用方已另行注入后端，构建期将因多后端
    /// `Err(NotSupported)` fail-fast，与 [`CacheBuilder`](crate::cache::CacheBuilder) 既有契约一致。
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    pub async fn apply_to_cache_builder<K, V>(
        &self,
        mut builder: crate::cache::CacheBuilder<K, V>,
    ) -> OxCacheResult<crate::cache::CacheBuilder<K, V>>
    where
        K: crate::traits::CacheKey,
        V: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        self.validate()?;
        if let Some(capacity) = self.capacity {
            builder = builder.capacity(capacity);
        }
        if let Some(ttl) = self.ttl {
            builder = builder.ttl(ttl);
        }
        if let Some(tti) = self.tti {
            builder = builder.tti(tti);
        }
        if let Some(null_cache_ttl) = self.null_cache_ttl {
            builder = builder.null_cache_ttl(null_cache_ttl);
        }
        if let Some(factor) = self.ttl_jitter_factor {
            builder = builder.ttl_jitter(factor);
        }
        if let Some(sync_mode) = self.sync_mode {
            builder = builder.sync_mode(sync_mode);
        }
        #[cfg(feature = "metrics")]
        if let Some(service) = self.service_name.clone() {
            builder = builder.service_name(service);
        }
        #[cfg(feature = "metrics")]
        if self.metrics_enabled == Some(false) {
            builder = builder.metrics(std::sync::Arc::new(crate::infra::NoOpMetricsRecorder));
        }
        #[cfg(any(feature = "serialization", feature = "full"))]
        if let Some(raw) = self.serialization_format.as_deref() {
            let format = parse_serialization_format("serialization_format", raw)?;
            builder = builder.serialization_format(format);
        }
        // 按槽位注入：Dual 槽（Moka/DashMap 双面原生 coerce）两面皆不走桥接；
        // Sync 槽 async 面经门面；Async 槽 sync 面经 AsyncToSyncBridge 桥出
        match self.build_backend_slot().await? {
            None => {}
            Some(slot) => {
                builder = builder.backend_slot(slot);
            }
        }
        Ok(builder)
    }
}

/// [`CacheConfig`] 链式构建器
#[derive(Debug, Default)]
pub struct CacheConfigBuilder {
    config: CacheConfig,
}

impl CacheConfigBuilder {
    /// L1 容量（条目数）
    pub fn capacity(mut self, capacity: u64) -> Self {
        self.config.capacity = Some(capacity);
        self
    }

    /// 默认 TTL
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.config.ttl = Some(ttl);
        self
    }

    /// 默认 TTI
    pub fn tti(mut self, tti: Duration) -> Self {
        self.config.tti = Some(tti);
        self
    }

    /// 穿透防护空值缓存 TTL
    pub fn null_cache_ttl(mut self, ttl: Duration) -> Self {
        self.config.null_cache_ttl = Some(ttl);
        self
    }

    /// TTL 抖动因子
    pub fn ttl_jitter_factor(mut self, factor: f64) -> Self {
        self.config.ttl_jitter_factor = Some(factor);
        self
    }

    /// 同步 API 模式
    pub fn sync_mode(mut self, enabled: bool) -> Self {
        self.config.sync_mode = Some(enabled);
        self
    }

    /// 后端类型原始串
    pub fn backend(mut self, backend: impl Into<String>) -> Self {
        self.config.backend = Some(backend.into());
        self
    }

    /// 后端类型（类型安全入口；`backend` 模块可用时提供）
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    pub fn backend_kind(mut self, backend: crate::backend::BackendKind) -> Self {
        use crate::backend::BackendKind;
        let raw = match backend {
            BackendKind::Moka => "moka",
            BackendKind::DashMap => "dashmap",
            BackendKind::Redis => "redis",
            BackendKind::Valkey => "valkey",
            BackendKind::Dragonfly => "dragonfly",
            BackendKind::Aerospike => "aerospike",
            BackendKind::Chain => "chain",
            BackendKind::Mock => "mock",
            BackendKind::Disk => "disk",
            BackendKind::Unknown => "unknown",
        };
        self.config.backend = Some(raw.to_string());
        self
    }

    /// 指标开关
    pub fn metrics_enabled(mut self, enabled: bool) -> Self {
        self.config.metrics_enabled = Some(enabled);
        self
    }

    /// 序列化传输格式原始串
    pub fn serialization_format(mut self, format: impl Into<String>) -> Self {
        self.config.serialization_format = Some(format.into());
        self
    }

    /// Redis 兼容后端连接串
    pub fn redis_url(mut self, url: impl Into<String>) -> Self {
        self.config.redis_url = Some(url.into());
        self
    }

    /// 磁盘后端数据文件路径
    pub fn disk_path(mut self, path: impl Into<String>) -> Self {
        self.config.disk_path = Some(path.into());
        self
    }

    /// 连接池大小（Redis / Dragonfly 后端消费，缺省 8）
    pub fn connection_pool_size(mut self, size: usize) -> Self {
        self.config.connection_pool_size = Some(size);
        self
    }

    /// 熔断器连续失败阈值
    pub fn circuit_breaker_failure_threshold(mut self, threshold: u32) -> Self {
        self.config.circuit_breaker_failure_threshold = Some(threshold);
        self
    }

    /// 熔断器 Open → HalfOpen 恢复超时
    pub fn circuit_breaker_reset_timeout(mut self, timeout: Duration) -> Self {
        self.config.circuit_breaker_reset_timeout = Some(timeout);
        self
    }

    /// R9 service 维度标签（空串在 validate 期显性拒绝；装配依赖 metrics feature）
    pub fn service_name(mut self, service: impl Into<String>) -> Self {
        self.config.service_name = Some(service.into());
        self
    }

    /// 构建配置
    pub fn build(self) -> CacheConfig {
        self.config
    }
}

/// 读取环境变量；未设置为 `None`
fn env_value(var: &str) -> OxCacheResult<Option<String>> {
    match std::env::var(var) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(raw)) => Err(OxCacheError::InvalidInput(format!(
            "environment variable {var} is not valid unicode: {raw:?}"
        ))),
    }
}

/// 数值解析失败的显性错误（附变量名与原始值）
fn invalid_value<E: std::fmt::Display>(var: &'static str, raw: &str) -> impl Fn(E) -> OxCacheError {
    let raw = raw.to_string();
    move |err| {
        OxCacheError::InvalidInput(t(
            MSG_DETAIL_CONFIG_ENV_INVALID_VALUE,
            &[
                ("key", var.to_string()),
                ("raw", format!("{raw:?}")),
                ("err", err.to_string()),
            ],
        ))
    }
}

/// 解析布尔环境值：`true/1/yes/on` 与 `false/0/no/off`（大小写不敏感）
fn parse_bool_value(var: &str, raw: &str) -> OxCacheResult<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(OxCacheError::InvalidInput(format!(
            "invalid bool value for {var}: {raw:?} (expected true/1/yes/on or false/0/no/off)"
        ))),
    }
}

/// 解析后端类型字符串（大小写不敏感）
///
/// 无法识别的值显性报错；env 解析与 validate 一致性检查共用同一映射。
/// `var` 为定位标签：env 通路传 `OXCACHE_BACKEND`，validate 无法区分配置
/// 来源（env/confers/builder），只传字段名 `backend`，不伪装来源。
/// 定义随 `backend` 模块门控：lib.rs 以 cfg(any(memory,redis,disk)) 门控整个
/// `backend` 模块（含 [`BackendKind`](crate::backend::BackendKind)），窄
/// feature 组合下该路径不存在，本函数与 `backend_kind` 入口同步门控。
#[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
pub(crate) fn parse_backend_kind(
    var: &str,
    raw: &str,
) -> OxCacheResult<crate::backend::BackendKind> {
    let kind = match raw.to_ascii_lowercase().as_str() {
        "moka" => crate::backend::BackendKind::Moka,
        "dashmap" => crate::backend::BackendKind::DashMap,
        "redis" => crate::backend::BackendKind::Redis,
        "valkey" => crate::backend::BackendKind::Valkey,
        "dragonfly" => crate::backend::BackendKind::Dragonfly,
        "aerospike" => crate::backend::BackendKind::Aerospike,
        "chain" => crate::backend::BackendKind::Chain,
        "mock" => crate::backend::BackendKind::Mock,
        "disk" => crate::backend::BackendKind::Disk,
        _ => {
            return Err(OxCacheError::InvalidInput(format!(
                "invalid value for {var}: {raw:?} (expected one of moka/dashmap/redis/valkey/dragonfly/aerospike/chain/mock/disk)"
            )));
        }
    };
    Ok(kind)
}

/// 解析序列化格式字符串（大小写不敏感；feature 缺失时显性报错）
///
/// `var` 为定位标签，语义同 [`parse_backend_kind`]：env 传变量名，
/// validate 传字段名（来源中立）。
#[cfg(any(feature = "serialization", feature = "full"))]
pub(crate) fn parse_serialization_format(
    var: &str,
    raw: &str,
) -> OxCacheResult<crate::infra::serialization::SerializationFormat> {
    use crate::infra::serialization::SerializationFormat;
    match raw.to_ascii_lowercase().as_str() {
        "json" => Ok(SerializationFormat::Json),
        #[cfg(feature = "serde-bincode")]
        "bincode" => Ok(SerializationFormat::Bincode),
        #[cfg(not(feature = "serde-bincode"))]
        "bincode" => Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_CONFIG_SERIALIZATION_BINCODE_REQUIRES_FEATURE,
            &[("field", var.to_string())],
        ))),
        #[cfg(feature = "postcard")]
        "postcard" => Ok(SerializationFormat::Postcard),
        #[cfg(not(feature = "postcard"))]
        "postcard" => Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_CONFIG_SERIALIZATION_POSTCARD_REQUIRES_FEATURE,
            &[("field", var.to_string())],
        ))),
        _ => Err(OxCacheError::InvalidInput(t(
            MSG_DETAIL_CONFIG_SERIALIZATION_INVALID_FORMAT,
            &[("field", var.to_string()), ("raw", format!("{raw:?}"))],
        ))),
    }
}

impl CacheConfig {
    /// 必填字符串字段非空检查
    fn require_non_empty(&self, field: &str, value: Option<&str>) -> OxCacheResult<()> {
        match value {
            Some(v) if !v.trim().is_empty() => Ok(()),
            Some(_) => Err(OxCacheError::InvalidInput(format!(
                "{field} must not be blank for the configured backend"
            ))),
            None => Err(OxCacheError::InvalidInput(format!(
                "{field} is required for the configured backend"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// try_from_env 读取的全部环境变量键（测试前置清理，防环境污染）
    const ALL_ENV_KEYS: &[&str] = &[
        KEY_CAPACITY,
        KEY_TTL_MS,
        KEY_TTI_MS,
        KEY_NULL_CACHE_TTL_MS,
        KEY_TTL_JITTER_FACTOR,
        KEY_SYNC_MODE,
        KEY_BACKEND,
        KEY_METRICS,
        KEY_SERIALIZATION_FORMAT,
        KEY_REDIS_URL,
        KEY_DISK_PATH,
        KEY_CONNECTION_POOL_SIZE,
        KEY_CB_FAILURE_THRESHOLD,
        KEY_CB_RESET_TIMEOUT_MS,
        KEY_SERVICE_NAME,
    ];

    #[test]
    #[serial]
    fn try_from_env_parses_remaining_keys() {
        clear_all_env_keys();
        set_env(KEY_DISK_PATH, "/tmp/ox-env-disk.redb");
        set_env(KEY_REDIS_URL, "redis://127.0.0.1:6379");
        set_env(KEY_CONNECTION_POOL_SIZE, "4");
        set_env(KEY_CB_FAILURE_THRESHOLD, "7");
        set_env(KEY_CB_RESET_TIMEOUT_MS, "2500");
        let cfg = CacheConfig::try_from_env().expect("parse env");
        assert_eq!(cfg.disk_path.as_deref(), Some("/tmp/ox-env-disk.redb"));
        assert_eq!(cfg.redis_url.as_deref(), Some("redis://127.0.0.1:6379"));
        assert_eq!(cfg.connection_pool_size, Some(4));
        assert_eq!(cfg.circuit_breaker_failure_threshold, Some(7));
        assert_eq!(
            cfg.circuit_breaker_reset_timeout,
            Some(std::time::Duration::from_millis(2500))
        );
        clear_all_env_keys();
    }

    #[test]
    fn validate_rejects_capacity_beyond_usize() {
        // 32 位目标上 u64 容量会静默截断——超界显性拒绝；64 位 usize 与 u64
        // 同宽，不存在可构造的超界值，边界值 u64::MAX 必须合法通过
        #[cfg(target_pointer_width = "32")]
        {
            let err = CacheConfig::builder()
                .capacity(u64::MAX)
                .build()
                .validate()
                .unwrap_err();
            assert!(!err.to_string().is_empty());
        }
        #[cfg(target_pointer_width = "64")]
        {
            CacheConfig::builder()
                .capacity(u64::MAX)
                .build()
                .validate()
                .expect("u64::MAX == usize::MAX on 64-bit is a legal capacity");
        }
    }

    #[cfg(feature = "dragonfly")]
    #[tokio::test]
    #[serial]
    async fn build_backend_dragonfly_with_fake_endpoint() {
        // 进程内假服务器拉起后走 Dragonfly 槽完整构建
        if !crate::test_support::ensure_server(6380) {
            return;
        }
        let config = CacheConfig::builder()
            .backend("dragonfly")
            .redis_url("redis://127.0.0.1:6380")
            .connection_pool_size(2)
            .build();
        let backend = config.build_backend().await.expect("build dragonfly");
        assert!(backend.is_some());
    }

    #[test]
    #[serial]
    fn env_invalid_numeric_values_rejected() {
        clear_all_env_keys();
        set_env(KEY_CONNECTION_POOL_SIZE, "not-a-number");
        let err = CacheConfig::try_from_env().unwrap_err();
        assert!(err.to_string().contains("CONNECTION_POOL_SIZE") || !err.to_string().is_empty());
        clear_all_env_keys();
        set_env(KEY_CB_FAILURE_THRESHOLD, "-3");
        let err = CacheConfig::try_from_env().unwrap_err();
        assert!(!err.to_string().is_empty());
        clear_all_env_keys();
    }

    #[test]
    fn env_keys_share_prefix() {
        // 键名常量与 ENV_PREFIX 的 DRY 契约：漂移在此显性失败
        for key in ALL_ENV_KEYS {
            assert!(key.starts_with(ENV_PREFIX), "{key} lacks {ENV_PREFIX}");
        }
    }

    // SAFETY: edition 2024 set_var 为 unsafe；env 测试以 #[serial] 串行，进程级 env 无并发读写
    #[allow(unsafe_code)]
    fn set_env(key: &str, value: &str) {
        unsafe { std::env::set_var(key, value) };
    }

    // SAFETY: edition 2024 remove_var 为 unsafe；env 测试以 #[serial] 串行，进程级 env 无并发读写
    #[allow(unsafe_code)]
    fn remove_env(key: &str) {
        unsafe { std::env::remove_var(key) };
    }

    fn clear_all_env_keys() {
        for key in ALL_ENV_KEYS {
            remove_env(key);
        }
    }

    #[test]
    fn default_is_all_none() {
        let config = CacheConfig::default();
        assert_eq!(
            config,
            CacheConfig {
                capacity: None,
                ttl: None,
                tti: None,
                null_cache_ttl: None,
                ttl_jitter_factor: None,
                sync_mode: None,
                backend: None,
                metrics_enabled: None,
                serialization_format: None,
                redis_url: None,
                disk_path: None,
                connection_pool_size: None,
                circuit_breaker_failure_threshold: None,
                circuit_breaker_reset_timeout: None,
                service_name: None,
            }
        );
    }

    #[test]
    #[serial]
    fn env_unset_yields_all_none_behavior_unchanged() {
        clear_all_env_keys();
        let config = CacheConfig::try_from_env().unwrap();
        assert_eq!(config, CacheConfig::default());
    }

    #[test]
    #[serial]
    fn env_full_parse() {
        clear_all_env_keys();
        set_env("OXCACHE_CAPACITY", "2048");
        set_env("OXCACHE_TTL_MS", "30000");
        set_env("OXCACHE_TTI_MS", "15000");
        set_env("OXCACHE_NULL_CACHE_TTL_MS", "500");
        set_env("OXCACHE_TTL_JITTER_FACTOR", "0.25");
        set_env("OXCACHE_SYNC_MODE", "true");
        // 窄组合下 env 层显性拒绝 backend 键（见 env_backend_requires_backend_feature）
        #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
        set_env("OXCACHE_BACKEND", "redis");
        set_env("OXCACHE_METRICS", "false");
        set_env(KEY_CONNECTION_POOL_SIZE, "16");
        set_env(KEY_REDIS_URL, "redis://127.0.0.1:6379");
        set_env("OXCACHE_CIRCUIT_BREAKER_FAILURE_THRESHOLD", "7");
        set_env("OXCACHE_CIRCUIT_BREAKER_RESET_TIMEOUT_MS", "45000");
        set_env("OXCACHE_SERVICE_NAME", "r9-env-svc");
        #[cfg(any(feature = "serialization", feature = "full"))]
        set_env("OXCACHE_SERIALIZATION_FORMAT", "json");

        let config = CacheConfig::try_from_env().unwrap();
        assert_eq!(config.capacity, Some(2048));
        assert_eq!(config.ttl, Some(Duration::from_millis(30_000)));
        assert_eq!(config.tti, Some(Duration::from_millis(15_000)));
        assert_eq!(config.null_cache_ttl, Some(Duration::from_millis(500)));
        assert_eq!(config.ttl_jitter_factor, Some(0.25));
        assert_eq!(config.sync_mode, Some(true));
        #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
        assert_eq!(config.backend, Some("redis".to_string()));
        assert_eq!(config.metrics_enabled, Some(false));
        assert_eq!(config.redis_url, Some("redis://127.0.0.1:6379".to_string()));
        assert_eq!(config.circuit_breaker_failure_threshold, Some(7));
        assert_eq!(
            config.circuit_breaker_reset_timeout,
            Some(Duration::from_millis(45_000))
        );
        assert_eq!(config.service_name.as_deref(), Some("r9-env-svc"));
        #[cfg(any(feature = "serialization", feature = "full"))]
        assert_eq!(config.serialization_format, Some("json".to_string()));
    }

    #[test]
    #[serial]
    fn env_invalid_values_fail_loudly() {
        let cases: &[(&str, &str, &str)] = &[
            ("OXCACHE_CAPACITY", "abc", "capacity"),
            ("OXCACHE_TTL_MS", "-5", "ttl"),
            ("OXCACHE_TTL_JITTER_FACTOR", "fast", "jitter"),
            ("OXCACHE_SYNC_MODE", "maybe", "sync"),
            ("OXCACHE_BACKEND", "memcache", "backend"),
            (
                "OXCACHE_CIRCUIT_BREAKER_FAILURE_THRESHOLD",
                "3.5",
                "threshold",
            ),
            ("OXCACHE_CIRCUIT_BREAKER_RESET_TIMEOUT_MS", "soon", "reset"),
        ];
        for (key, value, label) in cases {
            clear_all_env_keys();
            set_env(key, value);
            let err = CacheConfig::try_from_env().expect_err(label);
            let msg = match err {
                OxCacheError::InvalidInput(m) => m,
                other => panic!("{label}: expected InvalidInput, got {other:?}"),
            };
            // 报错必须携带变量名与原始值，显性化到可直接定位
            assert!(msg.contains(key), "{label}: message lacks var name: {msg}");
            assert!(
                msg.contains(value),
                "{label}: message lacks raw value: {msg}"
            );
        }
    }

    #[test]
    #[serial]
    fn env_bool_variants() {
        for (raw, expected) in [
            ("1", true),
            ("TRUE", true),
            ("yes", true),
            ("On", true),
            ("0", false),
            ("false", false),
            ("No", false),
            ("off", false),
        ] {
            clear_all_env_keys();
            set_env("OXCACHE_SYNC_MODE", raw);
            let config = CacheConfig::try_from_env().unwrap();
            assert_eq!(config.sync_mode, Some(expected), "raw = {raw}");
        }
    }

    // 大小写不敏感解析仅在后端 feature 在场时可达（窄组合下 env 层先行拒绝）
    #[test]
    #[serial]
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    fn env_backend_case_insensitive() {
        clear_all_env_keys();
        set_env("OXCACHE_BACKEND", "DASHMAP");
        let config = CacheConfig::try_from_env().unwrap();
        assert_eq!(config.backend, Some("dashmap".to_string()));
    }

    // 以下两条仅在窄 feature 组合下运行：对应 feature 在场时 env 层不做拦截
    #[test]
    #[serial]
    #[cfg(not(any(feature = "serialization", feature = "full")))]
    fn env_serialization_format_requires_feature() {
        clear_all_env_keys();
        set_env(KEY_SERIALIZATION_FORMAT, "json");
        let err = CacheConfig::try_from_env().unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("serialization")));
    }

    #[test]
    #[serial]
    #[cfg(not(any(feature = "memory", feature = "redis", feature = "disk")))]
    fn env_backend_requires_backend_feature() {
        clear_all_env_keys();
        set_env(KEY_BACKEND, "moka");
        let err = CacheConfig::try_from_env().unwrap_err();
        assert!(
            matches!(&err, OxCacheError::InvalidInput(m) if m.contains("feature")),
            "{err:?}"
        );
    }

    #[test]
    fn validate_rejects_zero_capacity() {
        let err = CacheConfig::builder()
            .capacity(0)
            .build()
            .validate()
            .unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("capacity")));
    }

    #[test]
    fn validate_rejects_zero_ttl() {
        let err = CacheConfig::builder()
            .ttl(Duration::ZERO)
            .build()
            .validate()
            .unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("ttl")));

        let err = CacheConfig::builder()
            .tti(Duration::ZERO)
            .build()
            .validate()
            .unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("tti")));

        let err = CacheConfig::builder()
            .null_cache_ttl(Duration::ZERO)
            .build()
            .validate()
            .unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("null_cache_ttl")));
    }

    #[test]
    fn validate_rejects_zero_circuit_breaker_threshold() {
        let err = CacheConfig::builder()
            .circuit_breaker_failure_threshold(0)
            .build()
            .validate()
            .unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("circuit")));
    }

    #[test]
    fn validate_rejects_zero_pool_size() {
        // 与 capacity/threshold 同位次左移：底层 Redis/Dragonfly builder 亦拒 0，
        // 在此提前拦截使三条配置通路的失败时点一致
        let err = CacheConfig::builder()
            .connection_pool_size(0)
            .build()
            .validate()
            .unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("connection_pool_size")));
    }

    #[test]
    fn validate_redis_backend_requires_url() {
        // redis feature 缺席时 feature 前提先行拦截，必填检查仅在可用构建下可达
        #[cfg(not(feature = "redis"))]
        {
            let err = CacheConfig::builder()
                .backend("redis")
                .build()
                .validate()
                .unwrap_err();
            assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("feature")));
        }
        #[cfg(feature = "redis")]
        let err = CacheConfig::builder()
            .backend("redis")
            .build()
            .validate()
            .unwrap_err();
        #[cfg(feature = "redis")]
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("redis_url")));

        #[cfg(feature = "redis")]
        {
            let err = CacheConfig::builder()
                .backend("redis")
                .redis_url("   ")
                .build()
                .validate()
                .unwrap_err();
            assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("redis_url")));
        }
    }

    #[test]
    fn validate_disk_backend_requires_path() {
        #[cfg(feature = "disk")]
        {
            let err = CacheConfig::builder()
                .backend("disk")
                .build()
                .validate()
                .unwrap_err();
            assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("disk_path")));
        }
        #[cfg(not(feature = "disk"))]
        {
            let err = CacheConfig::builder()
                .backend("disk")
                .disk_path("/tmp/oxcache.redb")
                .build()
                .validate()
                .unwrap_err();
            assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("feature")));
        }
    }

    #[test]
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    fn validate_sync_mode_combines_with_backend() {
        // sync_mode 与显式 backend 合法：Moka/DashMap 走原生同步面（槽位
        // 注入），其余后端 sync 面桥出（runtime 要求属调用期契约，不在配置层拦截）
        for raw in ["moka", "dashmap"] {
            CacheConfig::builder()
                .sync_mode(true)
                .backend(raw)
                .build()
                .validate()
                .unwrap_or_else(|e| panic!("{raw}: {e}"));
        }
        #[cfg(feature = "redis")]
        CacheConfig::builder()
            .sync_mode(true)
            .backend("redis")
            .redis_url("redis://127.0.0.1:6379")
            .build()
            .validate()
            .unwrap();
        // 未配置 backend 时 sync 合法（默认构建路径，原生同步）
        CacheConfig::builder()
            .sync_mode(true)
            .build()
            .validate()
            .unwrap();
    }

    #[test]
    fn validate_rejects_empty_service_name() {
        let err = CacheConfig::builder()
            .service_name("")
            .build()
            .validate()
            .unwrap_err();
        assert!(
            matches!(&err, OxCacheError::InvalidInput(m) if m.contains("service_name")),
            "{err:?}"
        );
        // 非空合法
        CacheConfig::builder()
            .service_name("orders")
            .build()
            .validate()
            .unwrap();
    }

    #[test]
    #[cfg(feature = "memory")]
    fn apply_sync_mode_without_backend_builds_sync_cache() {
        use crate::cache::CacheBuilder;

        let config = CacheConfig::builder().capacity(32).sync_mode(true).build();
        // apply 在独立 runtime 内完成；sync 读写刻意置于 runtime 外，
        // 覆盖 SyncCacheBackend 无运行时路径（Waker::noop 轮询）
        let builder = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                config
                    .apply_to_cache_builder(CacheBuilder::<String, String>::default())
                    .await
                    .unwrap()
            });
        let cache = builder.build_sync().unwrap();
        cache.set_sync(&"k".to_string(), &"v".to_string()).unwrap();
        assert_eq!(
            cache.get_sync(&"k".to_string()).unwrap(),
            Some("v".to_string())
        );
    }

    #[test]
    #[cfg(feature = "memory")]
    fn apply_sync_mode_with_backend_uses_native_sync_surface() {
        use crate::cache::CacheBuilder;

        // sync_mode × backend 组合经 CacheConfig 通路解锁，且 Moka 双面原生：
        // Dual 槽注入后 sync API 在 runtime 之外可用（桥接面在此处显性
        // NotSupported）、async API 在 current_thread runtime 内可用（门面
        // 在此处显性拒绝）——两个判别性证据共同锁定「原生双面零桥接」契约
        let config = CacheConfig::builder()
            .capacity(32)
            .sync_mode(true)
            .backend("moka")
            .build();
        let builder = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .build()
            .unwrap()
            .block_on(async {
                config
                    .apply_to_cache_builder(CacheBuilder::<String, String>::default())
                    .await
                    .unwrap()
            });
        let cache = builder.build_sync().unwrap();
        // 判别一：sync 面运行时无关（runtime 之外直连）
        cache
            .set_sync(&"cfg-native".to_string(), &"v1".to_string())
            .unwrap();
        assert_eq!(
            cache.get_sync(&"cfg-native".to_string()).unwrap(),
            Some("v1".to_string())
        );
        // 判别二：async 面在 current_thread runtime 内可用（未降级为门面）
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                cache
                    .set(&"cfg-native".to_string(), &"v2".to_string())
                    .await
                    .unwrap();
                assert_eq!(
                    cache.get(&"cfg-native".to_string()).await.unwrap(),
                    Some("v2".to_string())
                );
            });
    }

    #[test]
    #[cfg(all(feature = "metrics", feature = "memory"))]
    fn apply_metrics_disabled_builds_functional_cache() {
        use crate::cache::CacheBuilder;

        // metrics_enabled=Some(false) 必须真的走到 recorder 注入分支：
        // Debug 暴露 metrics_injected（dyn recorder 无类型名，判存在即锁定
        // 分支被触发）；全局指标行为断言受并行测试共享状态干扰，不在此展开
        let config = CacheConfig::builder()
            .metrics_enabled(false)
            .sync_mode(true)
            .build();
        let builder = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                config
                    .apply_to_cache_builder(CacheBuilder::<String, String>::default())
                    .await
                    .unwrap()
            });
        let debug = format!("{builder:?}");
        assert!(debug.contains("metrics_injected: true"), "{debug}");
        // build_sync 入口要求 sync_mode(true)，与 metrics 开关互不相干
        let cache = builder.build_sync().unwrap();
        cache.set_sync(&"m".to_string(), &"n".to_string()).unwrap();
        assert_eq!(
            cache.get_sync(&"m".to_string()).unwrap(),
            Some("n".to_string())
        );
        // 默认构建不注入显式 recorder（沿用全局默认）
        assert!(
            !format!("{:?}", CacheBuilder::<String, String>::default())
                .contains("metrics_injected: true")
        );
    }

    #[test]
    fn validate_rejects_unbuildable_backends() {
        // Valkey 无后端实现；Chain 须经 ChainBuilder 组装；Unknown 不可构建
        for raw in ["valkey", "chain", "unknown"] {
            let config = CacheConfig::builder().backend(raw).build();
            let err = config.validate().unwrap_err();
            assert!(matches!(err, OxCacheError::InvalidInput(_)), "raw = {raw}");
        }
    }

    #[test]
    #[cfg(feature = "memory")]
    fn validate_accepts_memory_backends_without_extra_config() {
        for raw in ["moka", "dashmap", "mock"] {
            let config = CacheConfig::builder().backend(raw).build();
            config.validate().unwrap_or_else(|e| panic!("{raw}: {e}"));
        }
    }

    // Dual 槽：build_backend 公开入口返回原生 moka async 面——current_thread
    // runtime 可用是「未降级为门面」的判别性证据（回归锁）
    #[tokio::test]
    #[cfg(feature = "memory")]
    async fn build_backend_moka_applies_capacity_and_ttl() {
        use std::sync::Arc;

        let config = CacheConfig::builder()
            .capacity(123)
            .ttl(Duration::from_millis(45_000))
            .backend_kind(crate::backend::BackendKind::Moka)
            .build();
        config.validate().unwrap();
        let backend = config.build_backend().await.unwrap().unwrap();
        let key: Arc<str> = Arc::from("config:probe");
        backend
            .set(key.clone(), Arc::new(b"v".to_vec()), None)
            .await
            .unwrap();
        assert_eq!(backend.get(&key).await.unwrap().as_deref(), Some(&b"v"[..]));
    }

    #[test]
    #[cfg(feature = "memory")]
    fn validate_feature_gated_backend_rejected_when_missing() {
        // 在无 redis feature 的构建下，配置 Redis 后端必须显性失败
        #[cfg(not(feature = "redis"))]
        {
            let config = CacheConfig::builder()
                .backend("redis")
                .redis_url("redis://127.0.0.1:6379")
                .build();
            let err = config.validate().unwrap_err();
            assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("feature")));
        }
        // redis feature 启用时同配置合法
        #[cfg(feature = "redis")]
        {
            CacheConfig::builder()
                .backend("redis")
                .redis_url("redis://127.0.0.1:6379")
                .build()
                .validate()
                .unwrap();
        }
    }

    #[tokio::test]
    #[cfg(feature = "memory")]
    async fn apply_to_cache_builder_lands_all_fields() {
        use crate::cache::CacheBuilder;
        use std::marker::PhantomData;

        let config = CacheConfig::builder()
            .capacity(4096)
            .ttl(Duration::from_millis(60_000))
            .ttl_jitter_factor(0.2)
            .null_cache_ttl(Duration::from_millis(300))
            .sync_mode(false)
            .build();
        let builder = CacheBuilder::<String, String>::default();
        let builder = config.apply_to_cache_builder(builder).await.unwrap();
        let debug = format!("{:?}", builder);
        assert!(debug.contains("capacity: Some(4096)"), "{debug}");
        assert!(
            debug.contains("ttl: Some(60s)"),
            "ttl debug mismatch: {debug}"
        );
        assert!(debug.contains("ttl_jitter_factor: 0.2"), "{debug}");
        assert!(debug.contains("null_cache_ttl: Some(300ms)"), "{debug}");
        let _ = PhantomData::<String>;
    }

    #[tokio::test]
    #[cfg(feature = "memory")]
    async fn apply_with_backend_injects_backend_arc() {
        use crate::cache::CacheBuilder;

        let config = CacheConfig::builder().capacity(64).backend("moka").build();
        let builder = CacheBuilder::<String, String>::default();
        let builder = config.apply_to_cache_builder(builder).await.unwrap();
        let debug = format!("{:?}", builder);
        assert!(debug.contains("backends_count: 1"), "{debug}");
    }

    #[tokio::test]
    // 随 build_backend 方法的 backend 模块门控，窄组合下方法与测试同步缺席
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    async fn build_backend_unset_returns_none() {
        let config = CacheConfig::default();
        config.validate().unwrap();
        assert!(config.build_backend().await.unwrap().is_none());
    }

    #[tokio::test]
    #[cfg(any(feature = "memory", feature = "redis", feature = "disk"))]
    async fn build_backend_chain_rejected() {
        let config = CacheConfig::builder().backend("chain").build();
        match config.build_backend().await {
            Ok(_) => panic!("Chain must not build a backend"),
            Err(OxCacheError::InvalidInput(msg)) => assert!(
                msg.to_ascii_lowercase().contains("chain"),
                "message should name the backend: {msg}"
            ),
            Err(other) => panic!("expected InvalidInput from validate, got {other:?}"),
        }
    }

    #[test]
    fn debug_redacts_redis_url() {
        let config = CacheConfig::builder()
            .redis_url("redis://admin:s3cret@host:6379") // pragma: allowlist secret — 虚构凭证,本测试断言 Debug 脱敏
            .build();
        let debug = format!("{config:?}");
        assert!(!debug.contains("s3cret"), "credentials leaked: {debug}");
        assert!(
            debug.contains("\"***\""),
            "redaction marker missing: {debug}"
        );
        // None 语义保留
        let absent = format!("{:?}", CacheConfig::default());
        assert!(absent.contains("redis_url: None"), "{absent}");
    }

    #[test]
    fn validate_capacity_beyond_usize_rejected() {
        let config = CacheConfig::builder().capacity(u64::MAX).build();
        if u64::MAX > usize::MAX as u64 {
            assert!(config.validate().is_err());
        } else {
            // 64 位平台 u64::MAX 即 usize::MAX，上界不可达，仅校验通过
            config.validate().unwrap();
        }
    }

    #[test]
    fn validate_metrics_gating_matches_feature() {
        let config = CacheConfig::builder().metrics_enabled(false).build();
        #[cfg(not(feature = "metrics"))]
        {
            let err = config.validate().unwrap_err();
            assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("metrics")));
        }
        #[cfg(feature = "metrics")]
        config.validate().unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "serialization", feature = "full")))]
    fn validate_serialization_gating() {
        let config = CacheConfig::builder().serialization_format("json").build();
        let err = config.validate().unwrap_err();
        assert!(matches!(err, OxCacheError::InvalidInput(m) if m.contains("serialization")));
    }

    #[test]
    fn builder_chain_sets_all_fields() {
        let config = CacheConfig::builder()
            .capacity(100)
            .ttl(Duration::from_secs(1))
            .tti(Duration::from_secs(2))
            .null_cache_ttl(Duration::from_secs(3))
            .ttl_jitter_factor(0.5)
            .sync_mode(true)
            .backend("moka")
            .metrics_enabled(true)
            .redis_url("redis://localhost")
            .disk_path("/tmp/oxcache.redb")
            .circuit_breaker_failure_threshold(9)
            .circuit_breaker_reset_timeout(Duration::from_secs(10))
            .build();
        assert_eq!(config.capacity, Some(100));
        assert_eq!(config.ttl, Some(Duration::from_secs(1)));
        assert_eq!(config.tti, Some(Duration::from_secs(2)));
        assert_eq!(config.null_cache_ttl, Some(Duration::from_secs(3)));
        assert_eq!(config.ttl_jitter_factor, Some(0.5));
        assert_eq!(config.sync_mode, Some(true));
        assert_eq!(config.backend, Some("moka".to_string()));
        assert_eq!(config.metrics_enabled, Some(true));
        assert_eq!(config.redis_url, Some("redis://localhost".to_string()));
        assert_eq!(config.disk_path, Some("/tmp/oxcache.redb".to_string()));
        assert_eq!(config.circuit_breaker_failure_threshold, Some(9));
        assert_eq!(
            config.circuit_breaker_reset_timeout,
            Some(Duration::from_secs(10))
        );
    }
}
