// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration structures for the cache library.
//
// This module uses confers derive macros for zero-boilerplate configuration
// management with built-in validation using garde.

use garde::Validate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 配置提供者 Trait（用于依赖注入）
///
/// 此 trait 定义了配置访问的抽象接口，遵循 di.md 架构规范中的
/// Infrastructure Layer 要求。允许不同配置源（文件、环境变量、远程配置等）
/// 的注入，实现配置的统一访问。
///
/// # 设计原则
///
/// - 继承 `Send + Sync`，确保可在 `Arc` 中安全共享
/// - 使用 `&self` 而非 `&mut self`，内部状态变更通过内部可变性实现
/// - 返回值使用 `Option<T>`，不 panic
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use oxcache::config::ConfigProvider;
///
/// // 注入配置提供者
/// let config: Arc<dyn ConfigProvider> = Arc::new(UnifiedConfig::default());
///
/// // 获取配置值
/// let ttl: Option<i64> = config.get_int("global.default_ttl");
/// ```
pub trait ConfigProvider: Send + Sync {
    /// 获取字符串配置值
    ///
    /// # Arguments
    ///
    /// * `key` - 配置键路径，使用点分隔（如 "global.default_ttl"）
    ///
    /// # Returns
    ///
    /// * `Some(value)` - 配置值存在
    /// * `None` - 配置值不存在或路径无效
    fn get_string(&self, key: &str) -> Option<String>;

    /// 获取整数配置值
    ///
    /// # Arguments
    ///
    /// * `key` - 配置键路径
    ///
    /// # Returns
    ///
    /// * `Some(value)` - 配置值存在且可解析为整数
    /// * `None` - 配置值不存在或解析失败
    fn get_int(&self, key: &str) -> Option<i64>;

    /// 获取布尔配置值
    ///
    /// # Arguments
    ///
    /// * `key` - 配置键路径
    ///
    /// # Returns
    ///
    /// * `Some(value)` - 配置值存在且可解析为布尔值
    /// * `None` - 配置值不存在或解析失败
    fn get_bool(&self, key: &str) -> Option<bool>;

    /// 获取 JSON 配置值
    ///
    /// 用于获取复杂配置结构，如对象或数组。
    ///
    /// # Arguments
    ///
    /// * `key` - 配置键路径
    ///
    /// # Returns
    ///
    /// * `Some(value)` - 配置值存在且可解析为 JSON
    /// * `None` - 配置值不存在或解析失败
    fn get_json(&self, key: &str) -> Option<serde_json::Value>;
}

/// 后端类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum BackendType {
    /// 仅内存后端 (L1)
    Memory,
    /// 仅 Redis 后端 (L2)
    Redis,
    /// 分层后端 (L1 + L2)
    Tiered,
}

impl Default for BackendType {
    #[inline]
    fn default() -> Self {
        BackendType::Memory
    }
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::Memory => write!(f, "Memory"),
            BackendType::Redis => write!(f, "Redis"),
            BackendType::Tiered => write!(f, "Tiered"),
        }
    }
}

impl std::str::FromStr for BackendType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Memory" => Ok(BackendType::Memory),
            "Redis" => Ok(BackendType::Redis),
            "Tiered" => Ok(BackendType::Tiered),
            _ => Err(format!("Unknown backend type: {}", s)),
        }
    }
}

/// 缓存类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CacheType {
    /// 仅 L1 (内存) 缓存
    L1,
    /// 仅 L2 (Redis) 缓存
    L2,
    /// 两级缓存 (L1 + L2)
    TwoLevel,
}

impl Default for CacheType {
    #[inline]
    fn default() -> Self {
        CacheType::L1
    }
}

impl std::fmt::Display for CacheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheType::L1 => write!(f, "L1"),
            CacheType::L2 => write!(f, "L2"),
            CacheType::TwoLevel => write!(f, "TwoLevel"),
        }
    }
}

impl std::str::FromStr for CacheType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "L1" => Ok(CacheType::L1),
            "L2" => Ok(CacheType::L2),
            "TwoLevel" => Ok(CacheType::TwoLevel),
            _ => Err(format!("Unknown cache type: {}", s)),
        }
    }
}

/// 全局配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GlobalConfig {
    /// 默认 TTL（秒）
    #[garde(range(max = 31_536_000))]
    pub default_ttl: u64,

    /// 默认 TTI（秒）
    #[garde(range(max = 31_536_000))]
    pub default_tti: u64,

    /// 健康检查间隔（秒）
    #[garde(range(min = 1, max = 3600))]
    pub health_check_interval: u32,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_ttl: 0,
            default_tti: 0,
            health_check_interval: 30,
        }
    }
}

/// 后端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// 后端类型（字符串形式：Memory, Redis, Tiered）
    pub backend_type: String,

    /// L1 缓存类型
    pub l1_type: String,

    /// L1 缓存选项（JSON 格式）
    pub l1_options_json: String,

    /// L2 缓存类型
    pub l2_type: String,

    /// L2 缓存选项（JSON 格式）
    pub l2_options_json: String,

    /// 是否启用 L1
    pub l1_enabled: bool,

    /// 是否启用 L2
    pub l2_enabled: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: "Memory".to_string(),
            l1_type: "moka".to_string(),
            l1_options_json: String::new(),
            l2_type: "redis".to_string(),
            l2_options_json: String::new(),
            l1_enabled: true,
            l2_enabled: false,
        }
    }
}

impl BackendConfig {
    /// 获取后端类型枚举
    pub fn backend_type_enum(&self) -> BackendType {
        self.backend_type.parse().unwrap_or(BackendType::Memory)
    }

    /// 获取 L1 选项
    pub fn l1_options(&self) -> serde_json::Value {
        if self.l1_options_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&self.l1_options_json).unwrap_or(serde_json::Value::Null)
        }
    }

    /// 获取 L2 选项
    pub fn l2_options(&self) -> serde_json::Value {
        if self.l2_options_json.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&self.l2_options_json).unwrap_or(serde_json::Value::Null)
        }
    }
}

/// 服务特定配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ServiceConfig {
    /// 缓存类型（字符串形式：L1, L2, TwoLevel）
    #[garde(skip)]
    pub cache_type: String,

    /// TTL（秒）
    #[garde(range(max = 31_536_000))]
    pub ttl: Option<u64>,

    /// 最大容量
    #[garde(custom(validate_capacity_opt))]
    pub max_capacity: Option<u64>,

    /// 是否启用指标
    #[garde(skip)]
    pub enable_metrics: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            cache_type: "L1".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }
}

impl ServiceConfig {
    /// 获取缓存类型枚举
    pub fn cache_type_enum(&self) -> CacheType {
        self.cache_type.parse().unwrap_or(CacheType::L1)
    }

    /// 创建 L1 服务配置
    #[inline]
    pub fn l1_only() -> Self {
        Self {
            cache_type: "L1".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }

    /// 创建 L2 服务配置
    #[inline]
    pub fn l2_only() -> Self {
        Self {
            cache_type: "L2".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }

    /// 创建两级服务配置
    #[inline]
    pub fn two_level() -> Self {
        Self {
            cache_type: "TwoLevel".to_string(),
            ttl: None,
            max_capacity: None,
            enable_metrics: true,
        }
    }

    /// 设置 TTL
    #[inline]
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

/// 验证容量值
fn validate_capacity_opt(value: &Option<u64>, _ctx: &()) -> garde::Result {
    if let Some(cap) = value {
        if *cap == 0 {
            return Err(garde::Error::new("容量不能为零"));
        }
        if *cap > 100_000_000 {
            return Err(garde::Error::new("容量超过最大值 100,000,000"));
        }
    }
    Ok(())
}

/// 性能配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PerformanceConfig {
    /// 最大并发操作数
    #[garde(range(min = 1, max = 100_000))]
    pub max_concurrent_operations: usize,

    /// 命令超时（毫秒）
    #[garde(range(min = 1, max = 300_000))]
    pub command_timeout: u64,

    /// 是否启用预取
    #[garde(skip)]
    pub enable_prefetching: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_operations: 1000,
            command_timeout: 5000,
            enable_prefetching: false,
        }
    }
}

/// 安全配置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SecurityConfig {
    /// 是否隐藏连接字符串
    #[garde(skip)]
    pub connection_string_redaction: bool,

    /// 是否启用限流
    #[garde(range(max = 1_000_000))]
    pub enable_rate_limiting: u64,

    /// 限流最大请求数
    #[garde(range(min = 1, max = 1_000_000))]
    pub rate_limit_max_requests: u64,

    /// 限流窗口大小（秒）
    #[garde(range(min = 1, max = 3600))]
    pub rate_limit_window_size: u64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            connection_string_redaction: true,
            enable_rate_limiting: 0,
            rate_limit_max_requests: 1000,
            rate_limit_window_size: 60,
        }
    }
}

/// 指标配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// 是否启用
    pub enabled: bool,

    /// 是否详细
    pub detailed: bool,

    /// 导出格式
    pub export_format: String,

    /// 导出端点
    pub export_endpoint: Option<String>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            detailed: false,
            export_format: "prometheus".to_string(),
            export_endpoint: None,
        }
    }
}

/// 恢复配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    /// 是否启用 WAL
    pub enable_wal: bool,

    /// WAL 目录
    pub wal_directory: String,

    /// 是否启用自动恢复
    pub enable_auto_recovery: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            enable_wal: false,
            wal_directory: "./wal".to_string(),
            enable_auto_recovery: true,
        }
    }
}

/// 统一配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedConfig {
    /// 全局配置
    pub global: GlobalConfig,

    /// 后端配置
    pub backend: BackendConfig,

    /// 服务配置（JSON 格式）
    pub services_json: String,

    /// 性能配置
    pub performance: PerformanceConfig,

    /// 指标配置
    pub metrics: MetricsConfig,

    /// 恢复配置
    pub recovery: RecoveryConfig,
}

impl UnifiedConfig {
    /// 获取服务配置映射
    pub fn services(&self) -> HashMap<String, ServiceConfig> {
        if self.services_json.is_empty() {
            HashMap::new()
        } else {
            serde_json::from_str(&self.services_json).unwrap_or_default()
        }
    }

    /// 从 TOML 文件加载
    pub fn from_toml_file(path: &str) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("读取文件 '{}' 失败: {}", path, e)))?;

        let config: Self = toml::from_str(&content)
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("解析 TOML '{}' 失败: {}", path, e)))?;

        config.validate_config()?;

        Ok(config)
    }

    /// 从 JSON 文件加载
    pub fn from_json_file(path: &str) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("读取文件 '{}' 失败: {}", path, e)))?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("解析 JSON '{}' 失败: {}", path, e)))?;

        config.validate_config()?;

        Ok(config)
    }

    /// 自动检测格式并加载
    pub fn from_file_auto(path: &str) -> crate::error::Result<Self> {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "toml" => Self::from_toml_file(path),
            "json" => Self::from_json_file(path),
            _ => Err(crate::error::CacheError::InvalidInput(format!(
                "不支持的配置文件格式: '{}'. 支持格式: .toml, .json",
                path
            ))),
        }
    }

    /// 验证配置内容
    pub fn validate_config(&self) -> crate::error::Result<()> {
        self.global
            .validate()
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("全局配置验证失败: {}", e)))?;
        self.performance
            .validate()
            .map_err(|e| crate::error::CacheError::InvalidInput(format!("性能配置验证失败: {}", e)))?;
        for (name, service) in self.services() {
            service
                .validate()
                .map_err(|e| crate::error::CacheError::InvalidInput(format!("服务 '{}' 配置验证失败: {}", name, e)))?;
        }
        Ok(())
    }
}

/// 为 UnifiedConfig 实现 ConfigProvider trait，支持依赖注入
impl ConfigProvider for UnifiedConfig {
    fn get_string(&self, key: &str) -> Option<String> {
        // 通过 key 路径获取配置值
        // 例如 "global.default_ttl" -> self.global.default_ttl
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() < 2 {
            return None;
        }

        match parts[0] {
            "global" => match *parts.get(1)? {
                "default_ttl" => Some(self.global.default_ttl.to_string()),
                "default_tti" => Some(self.global.default_tti.to_string()),
                "health_check_interval" => Some(self.global.health_check_interval.to_string()),
                _ => None,
            },
            "backend" => match *parts.get(1)? {
                "backend_type" => Some(self.backend.backend_type.clone()),
                "l1_type" => Some(self.backend.l1_type.clone()),
                "l1_options_json" => Some(self.backend.l1_options_json.clone()),
                "l2_type" => Some(self.backend.l2_type.clone()),
                "l2_options_json" => Some(self.backend.l2_options_json.clone()),
                "l1_enabled" => Some(self.backend.l1_enabled.to_string()),
                "l2_enabled" => Some(self.backend.l2_enabled.to_string()),
                _ => None,
            },
            "performance" => match *parts.get(1)? {
                "max_concurrent_operations" => Some(self.performance.max_concurrent_operations.to_string()),
                "command_timeout" => Some(self.performance.command_timeout.to_string()),
                "enable_prefetching" => Some(self.performance.enable_prefetching.to_string()),
                _ => None,
            },
            "metrics" => match *parts.get(1)? {
                "enabled" => Some(self.metrics.enabled.to_string()),
                "detailed" => Some(self.metrics.detailed.to_string()),
                "export_format" => Some(self.metrics.export_format.clone()),
                "export_endpoint" => self.metrics.export_endpoint.clone(),
                _ => None,
            },
            "recovery" => match *parts.get(1)? {
                "enable_wal" => Some(self.recovery.enable_wal.to_string()),
                "wal_directory" => Some(self.recovery.wal_directory.clone()),
                "enable_auto_recovery" => Some(self.recovery.enable_auto_recovery.to_string()),
                _ => None,
            },
            "services" => {
                // 对于 services，直接返回 services_json
                if *parts.get(1)? == "json" {
                    Some(self.services_json.clone())
                } else {
                    // 支持通过路径访问特定服务配置
                    let services = self.services();
                    let service_name = *parts.get(1)?;
                    if let Some(service) = services.get(service_name) {
                        match *parts.get(2)? {
                            "cache_type" => Some(service.cache_type.clone()),
                            "ttl" => service.ttl.map(|v| v.to_string()),
                            "max_capacity" => service.max_capacity.map(|v| v.to_string()),
                            "enable_metrics" => Some(service.enable_metrics.to_string()),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
            }
            _ => None,
        }
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.get_string(key).and_then(|v| v.parse().ok())
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_string(key).and_then(|v| v.parse().ok())
    }

    fn get_json(&self, key: &str) -> Option<serde_json::Value> {
        // 对于 JSON 类型字段，直接解析
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() < 2 {
            return None;
        }

        // 特殊处理 JSON 配置字段
        if parts[0] == "backend" {
            match *parts.get(1)? {
                "l1_options" => {
                    if self.backend.l1_options_json.is_empty() {
                        return Some(serde_json::Value::Null);
                    }
                    serde_json::from_str(&self.backend.l1_options_json).ok()
                }
                "l2_options" => {
                    if self.backend.l2_options_json.is_empty() {
                        return Some(serde_json::Value::Null);
                    }
                    serde_json::from_str(&self.backend.l2_options_json).ok()
                }
                _ => {
                    // 尝试将字符串值转换为 JSON
                    let value = self.get_string(key)?;
                    serde_json::from_str(&format!("\"{}\"", value)).ok()
                }
            }
        } else if parts[0] == "services" && *parts.get(1)? == "all" {
            // 返回所有服务配置的 JSON
            serde_json::to_value(self.services()).ok()
        } else {
            // 尝试将字符串值转换为 JSON
            let value = self.get_string(key)?;
            serde_json::from_str(&format!("\"{}\"", value)).ok()
        }
    }
}

/// 配置构建器（使用 confers ConfigBuilder 的便捷包装）
pub struct UnifiedConfigBuilder {
    builder: confers::ConfigBuilder<UnifiedConfig>,
    services: HashMap<String, ServiceConfig>,
}

impl Default for UnifiedConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedConfigBuilder {
    /// 创建新的构建器
    #[inline]
    pub fn new() -> Self {
        let builder = confers::ConfigBuilder::new()
            .default("global.default_ttl".to_string(), confers::ConfigValue::uint(0))
            .default("global.default_tti".to_string(), confers::ConfigValue::uint(0))
            .default(
                "global.health_check_interval".to_string(),
                confers::ConfigValue::uint(30),
            )
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Memory"),
            )
            .default("backend.l1_type".to_string(), confers::ConfigValue::string("moka"))
            .default("backend.l1_options_json".to_string(), confers::ConfigValue::string(""))
            .default("backend.l2_type".to_string(), confers::ConfigValue::string("redis"))
            .default("backend.l2_options_json".to_string(), confers::ConfigValue::string(""))
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(false))
            .default("services_json".to_string(), confers::ConfigValue::string(""))
            .default(
                "performance.max_concurrent_operations".to_string(),
                confers::ConfigValue::uint(1000),
            )
            .default(
                "performance.command_timeout".to_string(),
                confers::ConfigValue::uint(5000),
            )
            .default(
                "performance.enable_prefetching".to_string(),
                confers::ConfigValue::bool(false),
            )
            .default("metrics.enabled".to_string(), confers::ConfigValue::bool(false))
            .default("metrics.detailed".to_string(), confers::ConfigValue::bool(false))
            .default(
                "metrics.export_format".to_string(),
                confers::ConfigValue::string("prometheus"),
            )
            .default("recovery.enable_wal".to_string(), confers::ConfigValue::bool(false))
            .default(
                "recovery.wal_directory".to_string(),
                confers::ConfigValue::string("./wal"),
            )
            .default(
                "recovery.enable_auto_recovery".to_string(),
                confers::ConfigValue::bool(true),
            );
        Self {
            builder,
            services: HashMap::new(),
        }
    }

    /// 创建仅内存缓存配置
    #[inline]
    pub fn memory_only() -> Self {
        let mut builder = Self::new();
        builder.builder = builder
            .builder
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Memory"),
            )
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(false))
            .default("backend.l1_type".to_string(), confers::ConfigValue::string("moka"));
        builder
    }

    /// 创建仅 Redis 缓存配置
    #[inline]
    pub fn redis_only() -> Self {
        let mut builder = Self::new();
        builder.builder = builder
            .builder
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Redis"),
            )
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(false))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_type".to_string(), confers::ConfigValue::string("redis"));
        builder
    }

    /// 创建分层缓存配置
    #[inline]
    pub fn tiered() -> Self {
        let mut builder = Self::new();
        builder.builder = builder
            .builder
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string("Tiered"),
            )
            .default("backend.l1_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l2_enabled".to_string(), confers::ConfigValue::bool(true))
            .default("backend.l1_type".to_string(), confers::ConfigValue::string("moka"))
            .default("backend.l2_type".to_string(), confers::ConfigValue::string("redis"));
        builder
    }

    /// 设置默认 TTL
    #[inline]
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.builder = self
            .builder
            .default("global.default_ttl".to_string(), confers::ConfigValue::uint(ttl));
        self
    }

    /// 设置默认 TTI
    #[inline]
    pub fn with_tti(mut self, tti: u64) -> Self {
        self.builder = self
            .builder
            .default("global.default_tti".to_string(), confers::ConfigValue::uint(tti));
        self
    }

    /// 设置健康检查间隔
    #[inline]
    pub fn with_health_check_interval(mut self, interval: u32) -> Self {
        self.builder = self.builder.default(
            "global.health_check_interval".to_string(),
            confers::ConfigValue::uint(interval as u64),
        );
        self
    }

    /// 设置 L1 容量
    #[inline]
    pub fn with_l1_capacity(mut self, capacity: u64) -> Self {
        let options = serde_json::json!({"max_capacity": capacity}).to_string();
        self.builder = self.builder.default(
            "backend.l1_options_json".to_string(),
            confers::ConfigValue::string(&options),
        );
        self
    }

    /// 设置 Redis URL
    #[inline]
    pub fn with_redis_url(mut self, url: &str) -> Self {
        let options = serde_json::json!({"connection_string": url}).to_string();
        self.builder = self.builder.default(
            "backend.l2_options_json".to_string(),
            confers::ConfigValue::string(&options),
        );
        self
    }

    /// 设置 Redis 模式
    #[inline]
    pub fn with_redis_mode(mut self, mode: &str) -> Self {
        let options = serde_json::json!({"mode": mode}).to_string();
        self.builder = self.builder.default(
            "backend.l2_options_json".to_string(),
            confers::ConfigValue::string(&options),
        );
        self
    }

    /// 设置最大并发操作数
    #[inline]
    pub fn with_max_concurrent_operations(mut self, max_ops: usize) -> Self {
        self.builder = self.builder.default(
            "performance.max_concurrent_operations".to_string(),
            confers::ConfigValue::uint(max_ops as u64),
        );
        self
    }

    /// 设置命令超时
    #[inline]
    pub fn with_command_timeout(mut self, timeout: u64) -> Self {
        self.builder = self.builder.default(
            "performance.command_timeout".to_string(),
            confers::ConfigValue::uint(timeout),
        );
        self
    }

    /// 设置是否启用指标
    #[inline]
    pub fn with_metrics(mut self, enabled: bool) -> Self {
        self.builder = self
            .builder
            .default("metrics.enabled".to_string(), confers::ConfigValue::bool(enabled));
        self
    }

    /// 设置是否启用 WAL
    #[inline]
    pub fn with_wal(mut self, enabled: bool) -> Self {
        self.builder = self
            .builder
            .default("recovery.enable_wal".to_string(), confers::ConfigValue::bool(enabled));
        self
    }

    /// 设置 WAL 目录
    #[inline]
    pub fn with_wal_directory(mut self, directory: &str) -> Self {
        self.builder = self.builder.default(
            "recovery.wal_directory".to_string(),
            confers::ConfigValue::string(directory),
        );
        self
    }

    /// 设置是否启用自动恢复
    #[inline]
    pub fn with_auto_recovery(mut self, enabled: bool) -> Self {
        self.builder = self.builder.default(
            "recovery.enable_auto_recovery".to_string(),
            confers::ConfigValue::bool(enabled),
        );
        self
    }

    /// 添加服务配置
    #[inline]
    pub fn with_service(mut self, name: &str, cache_type: CacheType, ttl: u64) -> Self {
        let service = ServiceConfig {
            cache_type: cache_type.to_string(),
            ttl: if ttl > 0 { Some(ttl) } else { None },
            max_capacity: None,
            enable_metrics: true,
        };
        self.services.insert(name.to_string(), service);
        let services_json = serde_json::to_string(&self.services).unwrap_or_default();
        self.builder = self.builder.default(
            "services_json".to_string(),
            confers::ConfigValue::string(&services_json),
        );
        Self {
            builder: self.builder,
            services: self.services,
        }
    }

    /// 从文件加载
    #[inline]
    pub fn file(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.builder = self.builder.file(path);
        self
    }

    /// 从可选文件加载
    #[inline]
    pub fn file_optional(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.builder = self.builder.file_optional(path);
        self
    }

    /// 添加环境变量源
    #[inline]
    pub fn env(mut self) -> Self {
        self.builder = self.builder.env();
        self
    }

    /// 添加带前缀的环境变量源
    #[inline]
    pub fn env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.builder = self.builder.env_prefix(prefix);
        self
    }

    /// 构建配置
    #[inline]
    pub fn build(self) -> confers::ConfigResult<UnifiedConfig> {
        self.builder.build()
    }

    /// 构建为 JSON Value
    #[inline]
    pub fn build_json(self) -> serde_json::Value {
        self.build()
            .map(|c| serde_json::to_value(c).unwrap_or_default())
            .unwrap_or_default()
    }

    /// 使用预构建的 UnifiedConfig 创建构建器（完全依赖注入模式）
    ///
    /// 此方法遵循 di.md 架构规范中的模式 3（完全注入），允许
    /// 应用层完全控制配置的生命周期和单例共享。
    ///
    /// # Arguments
    ///
    /// * `config` - 预构建的 UnifiedConfig 实例
    ///
    /// # Returns
    ///
    /// 使用预构建配置初始化的 UnifiedConfigBuilder 实例
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    /// use oxcache::config::{ConfigProvider, UnifiedConfig, UnifiedConfigBuilder};
    ///
    /// // 应用层创建并管理配置单例
    /// let config = UnifiedConfig::from_toml_file("config.toml")?;
    ///
    /// // 通过完全注入创建构建器（用于进一步修改或直接使用）
    /// let builder = UnifiedConfigBuilder::with_dependencies(config);
    ///
    /// // 可以在 builder 上继续修改配置
    /// let final_config = builder.with_ttl(7200).build()?;
    /// ```
    pub fn with_dependencies(config: UnifiedConfig) -> Self {
        let builder = confers::ConfigBuilder::new();
        let services = config.services();
        let services_json = serde_json::to_string(&services).unwrap_or_default();

        // 使用预构建的配置值初始化 builder
        let builder = builder
            .default(
                "global.default_ttl".to_string(),
                confers::ConfigValue::uint(config.global.default_ttl),
            )
            .default(
                "global.default_tti".to_string(),
                confers::ConfigValue::uint(config.global.default_tti),
            )
            .default(
                "global.health_check_interval".to_string(),
                confers::ConfigValue::uint(config.global.health_check_interval as u64),
            )
            .default(
                "backend.backend_type".to_string(),
                confers::ConfigValue::string(&config.backend.backend_type),
            )
            .default(
                "backend.l1_type".to_string(),
                confers::ConfigValue::string(&config.backend.l1_type),
            )
            .default(
                "backend.l1_options_json".to_string(),
                confers::ConfigValue::string(&config.backend.l1_options_json),
            )
            .default(
                "backend.l2_type".to_string(),
                confers::ConfigValue::string(&config.backend.l2_type),
            )
            .default(
                "backend.l2_options_json".to_string(),
                confers::ConfigValue::string(&config.backend.l2_options_json),
            )
            .default(
                "backend.l1_enabled".to_string(),
                confers::ConfigValue::bool(config.backend.l1_enabled),
            )
            .default(
                "backend.l2_enabled".to_string(),
                confers::ConfigValue::bool(config.backend.l2_enabled),
            )
            .default(
                "services_json".to_string(),
                confers::ConfigValue::string(&services_json),
            )
            .default(
                "performance.max_concurrent_operations".to_string(),
                confers::ConfigValue::uint(config.performance.max_concurrent_operations as u64),
            )
            .default(
                "performance.command_timeout".to_string(),
                confers::ConfigValue::uint(config.performance.command_timeout),
            )
            .default(
                "performance.enable_prefetching".to_string(),
                confers::ConfigValue::bool(config.performance.enable_prefetching),
            )
            .default(
                "metrics.enabled".to_string(),
                confers::ConfigValue::bool(config.metrics.enabled),
            )
            .default(
                "metrics.detailed".to_string(),
                confers::ConfigValue::bool(config.metrics.detailed),
            )
            .default(
                "metrics.export_format".to_string(),
                confers::ConfigValue::string(&config.metrics.export_format),
            )
            .default(
                "recovery.enable_wal".to_string(),
                confers::ConfigValue::bool(config.recovery.enable_wal),
            )
            .default(
                "recovery.wal_directory".to_string(),
                confers::ConfigValue::string(&config.recovery.wal_directory),
            )
            .default(
                "recovery.enable_auto_recovery".to_string(),
                confers::ConfigValue::bool(config.recovery.enable_auto_recovery),
            );

        // 处理可选的 export_endpoint
        let builder = if let Some(endpoint) = &config.metrics.export_endpoint {
            builder.default(
                "metrics.export_endpoint".to_string(),
                confers::ConfigValue::string(endpoint),
            )
        } else {
            builder
        };

        Self { builder, services }
    }
}

/// 配置文件格式枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML 格式
    Toml,
    /// JSON 格式
    Json,
}

impl ConfigFormat {
    /// 从文件路径检测格式
    pub fn from_path(path: &str) -> Option<Self> {
        use std::path::Path;
        Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext {
                "toml" => Some(ConfigFormat::Toml),
                "json" => Some(ConfigFormat::Json),
                _ => None,
            })
    }

    /// 获取文件扩展名
    #[inline]
    pub fn extension(&self) -> &str {
        match self {
            ConfigFormat::Toml => "toml",
            ConfigFormat::Json => "json",
        }
    }

    /// 获取 MIME 类型
    #[inline]
    pub fn mime_type(&self) -> &str {
        match self {
            ConfigFormat::Toml => "application/toml",
            ConfigFormat::Json => "application/json",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_default() {
        assert_eq!(BackendType::default(), BackendType::Memory);
    }

    #[test]
    fn test_cache_type_default() {
        assert_eq!(CacheType::default(), CacheType::L1);
    }

    #[test]
    fn test_backend_type_from_str() {
        assert_eq!("Memory".parse::<BackendType>().unwrap(), BackendType::Memory);
        assert_eq!("Redis".parse::<BackendType>().unwrap(), BackendType::Redis);
        assert_eq!("Tiered".parse::<BackendType>().unwrap(), BackendType::Tiered);
    }

    #[test]
    fn test_cache_type_from_str() {
        assert_eq!("L1".parse::<CacheType>().unwrap(), CacheType::L1);
        assert_eq!("L2".parse::<CacheType>().unwrap(), CacheType::L2);
        assert_eq!("TwoLevel".parse::<CacheType>().unwrap(), CacheType::TwoLevel);
    }

    #[test]
    fn test_service_config_l1_only() {
        let config = ServiceConfig::l1_only();
        assert_eq!(config.cache_type_enum(), CacheType::L1);
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_service_config_with_ttl() {
        let config = ServiceConfig::l1_only().with_ttl(3600);
        assert_eq!(config.ttl, Some(3600));
    }

    #[test]
    fn test_config_format_from_path() {
        assert_eq!(ConfigFormat::from_path("config.toml"), Some(ConfigFormat::Toml));
        assert_eq!(ConfigFormat::from_path("config.json"), Some(ConfigFormat::Json));
        assert_eq!(ConfigFormat::from_path("config.yaml"), None);
    }

    #[test]
    fn test_config_format_extension() {
        assert_eq!(ConfigFormat::Toml.extension(), "toml");
        assert_eq!(ConfigFormat::Json.extension(), "json");
    }

    #[test]
    fn test_unified_config_builder_memory_only() {
        let builder = UnifiedConfigBuilder::memory_only();
        let config = builder.build().unwrap();
        assert_eq!(config.backend.backend_type_enum(), BackendType::Memory);
        assert!(config.backend.l1_enabled);
        assert!(!config.backend.l2_enabled);
    }

    #[test]
    fn test_unified_config_builder_redis_only() {
        let builder = UnifiedConfigBuilder::redis_only();
        let config = builder.build().unwrap();
        assert_eq!(config.backend.backend_type_enum(), BackendType::Redis);
        assert!(!config.backend.l1_enabled);
        assert!(config.backend.l2_enabled);
    }

    #[test]
    fn test_unified_config_builder_tiered() {
        let builder = UnifiedConfigBuilder::tiered();
        let config = builder.build().unwrap();
        assert_eq!(config.backend.backend_type_enum(), BackendType::Tiered);
        assert!(config.backend.l1_enabled);
        assert!(config.backend.l2_enabled);
    }

    #[test]
    fn test_unified_config_builder_with_ttl() {
        let config = UnifiedConfigBuilder::memory_only().with_ttl(3600).build().unwrap();
        assert_eq!(config.global.default_ttl, 3600);
    }

    #[test]
    fn test_unified_config_builder_with_l1_capacity() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_l1_capacity(10000)
            .build()
            .unwrap();
        let options = config.backend.l1_options();
        let capacity = options.get("max_capacity").unwrap().as_u64().unwrap();
        assert_eq!(capacity, 10000);
    }

    #[test]
    fn test_unified_config_builder_with_redis_url() {
        let config = UnifiedConfigBuilder::redis_only()
            .with_redis_url("redis://localhost:6379")
            .build()
            .unwrap();
        let options = config.backend.l2_options();
        let url = options.get("connection_string").unwrap().as_str().unwrap();
        assert_eq!(url, "redis://localhost:6379");
    }

    #[test]
    fn test_validate_capacity_opt() {
        assert!(validate_capacity_opt(&None, &()).is_ok());
        assert!(validate_capacity_opt(&Some(100), &()).is_ok());
        assert!(validate_capacity_opt(&Some(0), &()).is_err());
        assert!(validate_capacity_opt(&Some(100_000_001), &()).is_err());
    }

    #[test]
    fn test_global_config_validation() {
        let config = GlobalConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_performance_config_validation() {
        let config = PerformanceConfig::default();
        assert!(config.validate().is_ok());
    }

    // ========================================================================
    // BackendType: Display and error paths
    // ========================================================================

    #[test]
    fn test_backend_type_display() {
        assert_eq!(format!("{}", BackendType::Memory), "Memory");
        assert_eq!(format!("{}", BackendType::Redis), "Redis");
        assert_eq!(format!("{}", BackendType::Tiered), "Tiered");
    }

    #[test]
    fn test_backend_type_from_str_invalid() {
        let result = "Unknown".parse::<BackendType>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown backend type"));
    }

    #[test]
    fn test_backend_type_from_str_case_sensitive() {
        // Only exact match works
        assert!("memory".parse::<BackendType>().is_err());
        assert!("REDIS".parse::<BackendType>().is_err());
    }

    // ========================================================================
    // CacheType: Display and error paths
    // ========================================================================

    #[test]
    fn test_cache_type_display() {
        assert_eq!(format!("{}", CacheType::L1), "L1");
        assert_eq!(format!("{}", CacheType::L2), "L2");
        assert_eq!(format!("{}", CacheType::TwoLevel), "TwoLevel");
    }

    #[test]
    fn test_cache_type_from_str_invalid() {
        let result = "Unknown".parse::<CacheType>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown cache type"));
    }

    #[test]
    fn test_cache_type_from_str_case_sensitive() {
        assert!("l1".parse::<CacheType>().is_err());
        assert!("L2_ONLY".parse::<CacheType>().is_err());
    }

    // ========================================================================
    // BackendConfig: comprehensive
    // ========================================================================

    #[test]
    fn test_backend_config_default() {
        let config = BackendConfig::default();
        assert_eq!(config.backend_type, "Memory");
        assert_eq!(config.l1_type, "moka");
        assert_eq!(config.l2_type, "redis");
        assert!(config.l1_enabled);
        assert!(!config.l2_enabled);
        assert!(config.l1_options_json.is_empty());
        assert!(config.l2_options_json.is_empty());
    }

    #[test]
    fn test_backend_config_backend_type_enum_valid() {
        let mut config = BackendConfig::default();
        assert_eq!(config.backend_type_enum(), BackendType::Memory);

        config.backend_type = "Redis".to_string();
        assert_eq!(config.backend_type_enum(), BackendType::Redis);

        config.backend_type = "Tiered".to_string();
        assert_eq!(config.backend_type_enum(), BackendType::Tiered);
    }

    #[test]
    fn test_backend_config_backend_type_enum_invalid() {
        let mut config = BackendConfig::default();
        config.backend_type = "Invalid".to_string();
        assert_eq!(config.backend_type_enum(), BackendType::Memory); // fallback
    }

    #[test]
    fn test_backend_config_l1_options_empty() {
        let config = BackendConfig::default();
        assert_eq!(config.l1_options(), serde_json::Value::Null);
    }

    #[test]
    fn test_backend_config_l1_options_valid_json() {
        let mut config = BackendConfig::default();
        config.l1_options_json = r#"{"max_capacity": 5000}"#.to_string();
        let options = config.l1_options();
        assert_eq!(options["max_capacity"].as_u64().unwrap(), 5000);
    }

    #[test]
    fn test_backend_config_l1_options_invalid_json() {
        let mut config = BackendConfig::default();
        config.l1_options_json = "not valid json".to_string();
        assert_eq!(config.l1_options(), serde_json::Value::Null);
    }

    #[test]
    fn test_backend_config_l2_options_empty() {
        let config = BackendConfig::default();
        assert_eq!(config.l2_options(), serde_json::Value::Null);
    }

    #[test]
    fn test_backend_config_l2_options_valid_json() {
        let mut config = BackendConfig::default();
        config.l2_options_json = r#"{"connection_string": "redis://localhost"}"#.to_string();
        let options = config.l2_options();
        assert_eq!(options["connection_string"].as_str().unwrap(), "redis://localhost");
    }

    #[test]
    fn test_backend_config_l2_options_invalid_json() {
        let mut config = BackendConfig::default();
        config.l2_options_json = "{broken".to_string();
        assert_eq!(config.l2_options(), serde_json::Value::Null);
    }

    // ========================================================================
    // ServiceConfig: comprehensive
    // ========================================================================

    #[test]
    fn test_service_config_default() {
        let config = ServiceConfig::default();
        assert_eq!(config.cache_type, "L1");
        assert!(config.ttl.is_none());
        assert!(config.max_capacity.is_none());
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_service_config_l2_only() {
        let config = ServiceConfig::l2_only();
        assert_eq!(config.cache_type_enum(), CacheType::L2);
    }

    #[test]
    fn test_service_config_two_level() {
        let config = ServiceConfig::two_level();
        assert_eq!(config.cache_type_enum(), CacheType::TwoLevel);
    }

    #[test]
    fn test_service_config_cache_type_enum_invalid() {
        let mut config = ServiceConfig::default();
        config.cache_type = "Invalid".to_string();
        assert_eq!(config.cache_type_enum(), CacheType::L1); // fallback
    }

    #[test]
    fn test_service_config_with_ttl_chain() {
        let config = ServiceConfig::l1_only().with_ttl(7200);
        assert_eq!(config.ttl, Some(7200));
    }

    #[test]
    fn test_service_config_validation_valid() {
        let config = ServiceConfig::l1_only();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_service_config_validation_invalid_capacity() {
        let mut config = ServiceConfig::l1_only();
        config.max_capacity = Some(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_service_config_validation_capacity_too_large() {
        let mut config = ServiceConfig::l1_only();
        config.max_capacity = Some(100_000_001);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_service_config_validation_ttl_too_large() {
        let mut config = ServiceConfig::l1_only().with_ttl(31_536_001);
        assert!(config.validate().is_err());
    }

    // ========================================================================
    // GlobalConfig: validation boundaries
    // ========================================================================

    #[test]
    fn test_global_config_validation_ttl_too_large() {
        let config = GlobalConfig {
            default_ttl: 31_536_001,
            default_tti: 0,
            health_check_interval: 30,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_global_config_validation_tti_too_large() {
        let config = GlobalConfig {
            default_ttl: 0,
            default_tti: 31_536_001,
            health_check_interval: 30,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_global_config_validation_health_check_too_low() {
        let config = GlobalConfig {
            default_ttl: 0,
            default_tti: 0,
            health_check_interval: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_global_config_validation_health_check_too_high() {
        let config = GlobalConfig {
            default_ttl: 0,
            default_tti: 0,
            health_check_interval: 3601,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_global_config_validation_boundary_max_ttl() {
        let config = GlobalConfig {
            default_ttl: 31_536_000,
            default_tti: 0,
            health_check_interval: 30,
        };
        assert!(config.validate().is_ok());
    }

    // ========================================================================
    // PerformanceConfig: validation boundaries
    // ========================================================================

    #[test]
    fn test_performance_config_validation_max_concurrent_too_low() {
        let config = PerformanceConfig {
            max_concurrent_operations: 0,
            command_timeout: 5000,
            enable_prefetching: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_config_validation_command_timeout_too_low() {
        let config = PerformanceConfig {
            max_concurrent_operations: 1000,
            command_timeout: 0,
            enable_prefetching: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_config_validation_command_timeout_too_high() {
        let config = PerformanceConfig {
            max_concurrent_operations: 1000,
            command_timeout: 300_001,
            enable_prefetching: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_config_validation_boundary_max() {
        let config = PerformanceConfig {
            max_concurrent_operations: 100_000,
            command_timeout: 300_000,
            enable_prefetching: true,
        };
        assert!(config.validate().is_ok());
    }

    // ========================================================================
    // SecurityConfig
    // ========================================================================

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.connection_string_redaction);
        assert_eq!(config.enable_rate_limiting, 0);
        assert_eq!(config.rate_limit_max_requests, 1000);
        assert_eq!(config.rate_limit_window_size, 60);
    }

    #[test]
    fn test_security_config_validation_rate_limit_too_high() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1_000_001,
            rate_limit_max_requests: 1000,
            rate_limit_window_size: 60,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_security_config_validation_max_requests_too_low() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1,
            rate_limit_max_requests: 0,
            rate_limit_window_size: 60,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_security_config_validation_window_too_low() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1,
            rate_limit_max_requests: 100,
            rate_limit_window_size: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_security_config_validation_window_too_high() {
        let config = SecurityConfig {
            connection_string_redaction: true,
            enable_rate_limiting: 1,
            rate_limit_max_requests: 100,
            rate_limit_window_size: 3601,
        };
        assert!(config.validate().is_err());
    }

    // ========================================================================
    // MetricsConfig & RecoveryConfig
    // ========================================================================

    #[test]
    fn test_metrics_config_default() {
        let config = MetricsConfig::default();
        assert!(!config.enabled);
        assert!(!config.detailed);
        assert_eq!(config.export_format, "prometheus");
        assert!(config.export_endpoint.is_none());
    }

    #[test]
    fn test_recovery_config_default() {
        let config = RecoveryConfig::default();
        assert!(!config.enable_wal);
        assert_eq!(config.wal_directory, "./wal");
        assert!(config.enable_auto_recovery);
    }

    // ========================================================================
    // UnifiedConfig: comprehensive
    // ========================================================================

    #[test]
    fn test_unified_config_default() {
        let config = UnifiedConfig::default();
        assert_eq!(config.global.default_ttl, 0);
        assert_eq!(config.backend.backend_type, "Memory");
        assert!(config.services_json.is_empty());
        assert_eq!(config.performance.max_concurrent_operations, 1000);
        assert!(!config.metrics.enabled);
        assert!(!config.recovery.enable_wal);
    }

    #[test]
    fn test_unified_config_services_empty() {
        let config = UnifiedConfig::default();
        let services = config.services();
        assert!(services.is_empty());
    }

    #[test]
    fn test_unified_config_services_valid_json() {
        let mut config = UnifiedConfig::default();
        config.services_json =
            r#"{"auth": {"cache_type": "L1", "ttl": null, "max_capacity": null, "enable_metrics": true}}"#.to_string();
        let services = config.services();
        assert!(services.contains_key("auth"));
        assert_eq!(services["auth"].cache_type, "L1");
    }

    #[test]
    fn test_unified_config_services_invalid_json() {
        let mut config = UnifiedConfig::default();
        config.services_json = "{invalid".to_string();
        let services = config.services();
        assert!(services.is_empty()); // falls back to default
    }

    #[test]
    fn test_unified_config_validate_config_valid() {
        let config = UnifiedConfig::default();
        assert!(config.validate_config().is_ok());
    }

    #[test]
    fn test_unified_config_validate_config_invalid_global() {
        let mut config = UnifiedConfig::default();
        config.global.default_ttl = 31_536_001;
        assert!(config.validate_config().is_err());
    }

    #[test]
    fn test_unified_config_validate_config_invalid_service() {
        let mut config = UnifiedConfig::default();
        config.services_json =
            r#"{"bad": {"cache_type": "L1", "ttl": null, "max_capacity": 0, "enable_metrics": true}}"#.to_string();
        assert!(config.validate_config().is_err());
    }

    #[test]
    fn test_unified_config_validate_config_invalid_performance() {
        let mut config = UnifiedConfig::default();
        config.performance.max_concurrent_operations = 0;
        assert!(config.validate_config().is_err());
    }

    // ========================================================================
    // UnifiedConfig: file loading (error paths only; file I/O blocked in sandbox)
    // ========================================================================

    #[test]
    fn test_unified_config_from_toml_file_not_found() {
        let result = UnifiedConfig::from_toml_file("/nonexistent/path/config.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_unified_config_from_json_file_not_found() {
        let result = UnifiedConfig::from_json_file("/nonexistent/config.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_unified_config_from_file_auto_unsupported_format() {
        let result = UnifiedConfig::from_file_auto("/some/config.yaml");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("不支持的配置文件格式"));
    }

    #[test]
    fn test_unified_config_from_file_auto_no_extension() {
        let result = UnifiedConfig::from_file_auto("/some/config");
        assert!(result.is_err());
    }

    #[test]
    fn test_unified_config_toml_deserialize() {
        // Test TOML deserialization via confers ConfigBuilder (the standard path)
        let builder = UnifiedConfigBuilder::new()
            .with_ttl(3600)
            .with_tti(1800)
            .with_health_check_interval(60);
        let config = builder.build().unwrap();
        assert_eq!(config.global.default_ttl, 3600);
        assert_eq!(config.global.default_tti, 1800);
        assert_eq!(config.global.health_check_interval, 60);
    }

    #[test]
    fn test_unified_config_json_deserialize() {
        let json_str = serde_json::json!({
            "global": {"default_ttl": 7200, "default_tti": 3600, "health_check_interval": 30},
            "backend": {
                "backend_type": "Memory", "l1_type": "moka", "l1_options_json": "",
                "l2_type": "redis", "l2_options_json": "", "l1_enabled": true, "l2_enabled": false
            },
            "services_json": "",
            "performance": {"max_concurrent_operations": 1000, "command_timeout": 5000, "enable_prefetching": false},
            "metrics": {"enabled": false, "detailed": false, "export_format": "prometheus"},
            "recovery": {"enable_wal": false, "wal_directory": "./wal", "enable_auto_recovery": true}
        });
        let config: UnifiedConfig = serde_json::from_value(json_str).unwrap();
        assert_eq!(config.global.default_ttl, 7200);
        assert_eq!(config.performance.max_concurrent_operations, 1000);
    }

    #[test]
    fn test_unified_config_invalid_toml_parse() {
        let result: Result<UnifiedConfig, _> = toml::from_str("this is not toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_unified_config_invalid_json_parse() {
        let result = serde_json::from_str::<UnifiedConfig>("{invalid json}");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_format_from_path_edge_cases() {
        assert_eq!(ConfigFormat::from_path("config.TOML"), None); // case sensitive
        assert_eq!(ConfigFormat::from_path("config.json"), Some(ConfigFormat::Json));
        assert_eq!(ConfigFormat::from_path("dir/sub/config.toml"), Some(ConfigFormat::Toml));
        assert_eq!(ConfigFormat::from_path(""), None);
    }

    // ========================================================================
    // ConfigProvider trait on UnifiedConfig
    // ========================================================================

    #[test]
    fn test_config_provider_get_string_global() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_string("global.default_ttl"), Some("0".to_string()));
        assert_eq!(
            config.get_string("global.health_check_interval"),
            Some("30".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_backend() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_string("backend.backend_type"), Some("Memory".to_string()));
        assert_eq!(config.get_string("backend.l1_type"), Some("moka".to_string()));
        assert_eq!(config.get_string("backend.l1_enabled"), Some("true".to_string()));
        assert_eq!(config.get_string("backend.l2_enabled"), Some("false".to_string()));
    }

    #[test]
    fn test_config_provider_get_string_performance() {
        let config = UnifiedConfig::default();
        assert_eq!(
            config.get_string("performance.max_concurrent_operations"),
            Some("1000".to_string())
        );
        assert_eq!(
            config.get_string("performance.enable_prefetching"),
            Some("false".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_metrics() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_string("metrics.enabled"), Some("false".to_string()));
        assert_eq!(
            config.get_string("metrics.export_format"),
            Some("prometheus".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_recovery() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_string("recovery.enable_wal"), Some("false".to_string()));
        assert_eq!(config.get_string("recovery.wal_directory"), Some("./wal".to_string()));
    }

    #[test]
    fn test_config_provider_get_string_invalid_key() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_string("invalid"), None);
        assert_eq!(config.get_string("global"), None);
        assert_eq!(config.get_string("global.nonexistent"), None);
        assert_eq!(config.get_string("unknown.field"), None);
    }

    #[test]
    fn test_config_provider_get_string_services() {
        let mut config = UnifiedConfig::default();
        config.services_json =
            r#"{"auth": {"cache_type": "L1", "ttl": 3600, "max_capacity": 1000, "enable_metrics": true}}"#.to_string();
        assert_eq!(config.get_string("services.json"), Some(config.services_json.clone()));
        assert_eq!(config.get_string("services.auth.cache_type"), Some("L1".to_string()));
        assert_eq!(config.get_string("services.auth.ttl"), Some("3600".to_string()));
        assert_eq!(
            config.get_string("services.auth.max_capacity"),
            Some("1000".to_string())
        );
    }

    #[test]
    fn test_config_provider_get_string_services_nonexistent() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_string("services.nonexistent.cache_type"), None);
    }

    #[test]
    fn test_config_provider_get_int() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_int("global.default_ttl"), Some(0));
        assert_eq!(config.get_int("performance.max_concurrent_operations"), Some(1000));
        assert_eq!(config.get_int("global.nonexistent"), None);
    }

    #[test]
    fn test_config_provider_get_bool() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_bool("backend.l1_enabled"), Some(true));
        assert_eq!(config.get_bool("backend.l2_enabled"), Some(false));
        assert_eq!(config.get_bool("metrics.enabled"), Some(false));
        assert_eq!(config.get_bool("invalid.key"), None);
    }

    #[test]
    fn test_config_provider_get_json_backend_options_empty() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_json("backend.l1_options"), Some(serde_json::Value::Null));
        assert_eq!(config.get_json("backend.l2_options"), Some(serde_json::Value::Null));
    }

    #[test]
    fn test_config_provider_get_json_backend_options_valid() {
        let mut config = UnifiedConfig::default();
        config.backend.l1_options_json = r#"{"max_capacity": 10000}"#.to_string();
        let json = config.get_json("backend.l1_options").unwrap();
        assert_eq!(json["max_capacity"], 10000);
    }

    #[test]
    fn test_config_provider_get_json_services_all() {
        let mut config = UnifiedConfig::default();
        config.services_json =
            r#"{"auth": {"cache_type": "L1", "ttl": null, "max_capacity": null, "enable_metrics": true}}"#.to_string();
        let json = config.get_json("services.all").unwrap();
        assert!(json.is_object());
        assert!(json.as_object().unwrap().contains_key("auth"));
    }

    #[test]
    fn test_config_provider_get_json_fallback() {
        let config = UnifiedConfig::default();
        // Falls back to wrapping string value in JSON
        let result = config.get_json("backend.backend_type");
        assert!(result.is_some());
    }

    #[test]
    fn test_config_provider_get_json_invalid_key() {
        let config = UnifiedConfig::default();
        assert_eq!(config.get_json("invalid"), None);
    }

    // ========================================================================
    // UnifiedConfigBuilder: all builder methods
    // ========================================================================

    #[test]
    fn test_builder_default() {
        let builder = UnifiedConfigBuilder::new();
        let config = builder.build().unwrap();
        assert_eq!(config.global.default_ttl, 0);
        assert_eq!(config.global.default_tti, 0);
    }

    #[test]
    fn test_builder_with_tti() {
        let config = UnifiedConfigBuilder::memory_only().with_tti(1800).build().unwrap();
        assert_eq!(config.global.default_tti, 1800);
    }

    #[test]
    fn test_builder_with_health_check_interval() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_health_check_interval(120)
            .build()
            .unwrap();
        assert_eq!(config.global.health_check_interval, 120);
    }

    #[test]
    fn test_builder_with_redis_mode() {
        let config = UnifiedConfigBuilder::redis_only()
            .with_redis_mode("cluster")
            .build()
            .unwrap();
        let options = config.backend.l2_options();
        assert_eq!(options["mode"].as_str().unwrap(), "cluster");
    }

    #[test]
    fn test_builder_with_max_concurrent_operations() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_max_concurrent_operations(5000)
            .build()
            .unwrap();
        assert_eq!(config.performance.max_concurrent_operations, 5000);
    }

    #[test]
    fn test_builder_with_command_timeout() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_command_timeout(10000)
            .build()
            .unwrap();
        assert_eq!(config.performance.command_timeout, 10000);
    }

    #[test]
    fn test_builder_with_metrics() {
        let config = UnifiedConfigBuilder::memory_only().with_metrics(true).build().unwrap();
        assert!(config.metrics.enabled);
    }

    #[test]
    fn test_builder_with_wal() {
        let config = UnifiedConfigBuilder::memory_only().with_wal(true).build().unwrap();
        assert!(config.recovery.enable_wal);
    }

    #[test]
    fn test_builder_with_wal_directory() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_wal_directory("/var/wal")
            .build()
            .unwrap();
        assert_eq!(config.recovery.wal_directory, "/var/wal");
    }

    #[test]
    fn test_builder_with_auto_recovery() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_auto_recovery(false)
            .build()
            .unwrap();
        assert!(!config.recovery.enable_auto_recovery);
    }

    #[test]
    fn test_builder_with_service() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_service("auth", CacheType::L1, 3600)
            .build()
            .unwrap();
        let services = config.services();
        assert!(services.contains_key("auth"));
        assert_eq!(services["auth"].cache_type, "L1");
        assert_eq!(services["auth"].ttl, Some(3600));
    }

    #[test]
    fn test_builder_with_service_zero_ttl() {
        let config = UnifiedConfigBuilder::memory_only()
            .with_service("auth", CacheType::L1, 0)
            .build()
            .unwrap();
        let services = config.services();
        assert!(services["auth"].ttl.is_none()); // 0 -> None
    }

    #[test]
    fn test_builder_build_json() {
        let json = UnifiedConfigBuilder::memory_only().with_ttl(3600).build_json();
        assert!(json.is_object());
        assert_eq!(json["global"]["default_ttl"], 3600);
    }

    #[test]
    fn test_builder_build_json_default() {
        let json = UnifiedConfigBuilder::default().build_json();
        assert!(json.is_object());
    }

    // ========================================================================
    // UnifiedConfigBuilder::with_dependencies
    // ========================================================================

    #[test]
    fn test_builder_with_dependencies() {
        let config = UnifiedConfig::default();
        let builder = UnifiedConfigBuilder::with_dependencies(config);
        let result = builder.build().unwrap();
        assert_eq!(result.global.default_ttl, 0);
        assert_eq!(result.backend.backend_type, "Memory");
    }

    #[test]
    fn test_builder_with_dependencies_custom() {
        let config = UnifiedConfig {
            global: GlobalConfig {
                default_ttl: 7200,
                default_tti: 3600,
                health_check_interval: 60,
            },
            backend: BackendConfig {
                backend_type: "Tiered".to_string(),
                l1_type: "moka".to_string(),
                l1_options_json: r#"{"max_capacity": 5000}"#.to_string(),
                l2_type: "redis".to_string(),
                l2_options_json: r#"{"connection_string": "redis://localhost"}"#.to_string(),
                l1_enabled: true,
                l2_enabled: true,
            },
            services_json: String::new(),
            performance: PerformanceConfig {
                max_concurrent_operations: 2000,
                command_timeout: 10000,
                enable_prefetching: true,
            },
            metrics: MetricsConfig {
                enabled: true,
                detailed: true,
                export_format: "opentelemetry".to_string(),
                export_endpoint: Some("http://localhost:4318".to_string()),
            },
            recovery: RecoveryConfig {
                enable_wal: true,
                wal_directory: "/var/wal".to_string(),
                enable_auto_recovery: false,
            },
        };
        let builder = UnifiedConfigBuilder::with_dependencies(config);
        let result = builder.build().unwrap();
        assert_eq!(result.global.default_ttl, 7200);
        assert_eq!(result.global.default_tti, 3600);
        assert_eq!(result.global.health_check_interval, 60);
        assert_eq!(result.backend.backend_type, "Tiered");
        assert!(result.backend.l1_enabled);
        assert!(result.backend.l2_enabled);
        assert_eq!(result.performance.max_concurrent_operations, 2000);
        assert!(result.metrics.enabled);
        assert!(result.metrics.detailed);
        assert_eq!(result.metrics.export_format, "opentelemetry");
        assert_eq!(
            result.metrics.export_endpoint,
            Some("http://localhost:4318".to_string())
        );
        assert!(result.recovery.enable_wal);
        assert_eq!(result.recovery.wal_directory, "/var/wal");
        assert!(!result.recovery.enable_auto_recovery);
    }

    // ========================================================================
    // ConfigFormat: mime_type
    // ========================================================================

    #[test]
    fn test_config_format_mime_type() {
        assert_eq!(ConfigFormat::Toml.mime_type(), "application/toml");
        assert_eq!(ConfigFormat::Json.mime_type(), "application/json");
    }

    // ========================================================================
    // Serialization round-trip
    // ========================================================================

    #[test]
    fn test_unified_config_serialize_deserialize() {
        let config = UnifiedConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: UnifiedConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.global.default_ttl, config.global.default_ttl);
        assert_eq!(deserialized.backend.backend_type, config.backend.backend_type);
    }

    #[test]
    fn test_service_config_serialize_deserialize() {
        let config = ServiceConfig::l1_only().with_ttl(3600);
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ServiceConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.cache_type, config.cache_type);
        assert_eq!(deserialized.ttl, config.ttl);
    }

    #[test]
    fn test_backend_config_serialize_deserialize() {
        let config = BackendConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: BackendConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.backend_type, config.backend_type);
    }
}
