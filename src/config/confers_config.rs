// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Configuration structures for the cache library.
//
// This module uses confers derive macros for zero-boilerplate configuration
// management with built-in validation using garde.

use confers::Config;
use garde::Validate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
#[derive(Debug, Clone, Serialize, Deserialize, Config, Validate)]
#[config(validate)]
pub struct GlobalConfig {
    /// 默认 TTL（秒）
    #[config(default = 0u64)]
    #[garde(range(max = 31_536_000))]
    pub default_ttl: u64,

    /// 默认 TTI（秒）
    #[config(default = 0u64)]
    #[garde(range(max = 31_536_000))]
    pub default_tti: u64,

    /// 健康检查间隔（秒）
    #[config(default = 30u32)]
    #[garde(range(min = 1, max = 3600))]
    pub health_check_interval: u32,
}

/// 后端配置
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
pub struct BackendConfig {
    /// 后端类型（字符串形式：Memory, Redis, Tiered）
    #[config(default = "Memory".to_string())]
    pub backend_type: String,

    /// L1 缓存类型
    #[config(default = "moka".to_string())]
    pub l1_type: String,

    /// L1 缓存选项（JSON 格式）
    #[config(default = String::new())]
    pub l1_options_json: String,

    /// L2 缓存类型
    #[config(default = "redis".to_string())]
    pub l2_type: String,

    /// L2 缓存选项（JSON 格式）
    #[config(default = String::new())]
    pub l2_options_json: String,

    /// 是否启用 L1
    #[config(default = true)]
    pub l1_enabled: bool,

    /// 是否启用 L2
    #[config(default = false)]
    pub l2_enabled: bool,
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
#[derive(Debug, Clone, Serialize, Deserialize, Config, Validate)]
#[config(validate)]
pub struct ServiceConfig {
    /// 缓存类型（字符串形式：L1, L2, TwoLevel）
    #[config(default = "L1".to_string())]
    #[garde(skip)]
    pub cache_type: String,

    /// TTL（秒）
    #[garde(range(max = 31_536_000))]
    pub ttl: Option<u64>,

    /// 最大容量
    #[garde(custom(validate_capacity_opt))]
    pub max_capacity: Option<u64>,

    /// 是否启用指标
    #[config(default = true)]
    #[garde(skip)]
    pub enable_metrics: bool,
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
#[derive(Debug, Clone, Serialize, Deserialize, Config, Validate)]
#[config(validate)]
pub struct PerformanceConfig {
    /// 最大并发操作数
    #[config(default = 1000usize)]
    #[garde(range(min = 1, max = 100_000))]
    pub max_concurrent_operations: usize,

    /// 命令超时（毫秒）
    #[config(default = 5000u64)]
    #[garde(range(min = 1, max = 300_000))]
    pub command_timeout: u64,

    /// 是否启用预取
    #[config(default = false)]
    #[garde(skip)]
    pub enable_prefetching: bool,
}

/// 安全配置
#[derive(Debug, Clone, Serialize, Deserialize, Config, Validate)]
#[config(validate)]
pub struct SecurityConfig {
    /// 是否隐藏连接字符串
    #[config(default = true)]
    #[garde(skip)]
    pub connection_string_redaction: bool,

    /// 是否启用限流
    #[config(default = 0u64)]
    #[garde(range(max = 1_000_000))]
    pub enable_rate_limiting: u64,

    /// 限流最大请求数
    #[config(default = 1000u64)]
    #[garde(range(min = 1, max = 1_000_000))]
    pub rate_limit_max_requests: u64,

    /// 限流窗口大小（秒）
    #[config(default = 60u64)]
    #[garde(range(min = 1, max = 3600))]
    pub rate_limit_window_size: u64,
}

/// 指标配置
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
pub struct MetricsConfig {
    /// 是否启用
    #[config(default = false)]
    pub enabled: bool,

    /// 是否详细
    #[config(default = false)]
    pub detailed: bool,

    /// 导出格式
    #[config(default = "prometheus".to_string())]
    pub export_format: String,

    /// 导出端点
    pub export_endpoint: Option<String>,
}

/// 恢复配置
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
pub struct RecoveryConfig {
    /// 是否启用 WAL
    #[config(default = false)]
    pub enable_wal: bool,

    /// WAL 目录
    #[config(default = "./wal".to_string())]
    pub wal_directory: String,

    /// 是否启用自动恢复
    #[config(default = true)]
    pub enable_auto_recovery: bool,
}

/// 统一配置
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
pub struct UnifiedConfig {
    /// 全局配置
    #[config(flatten)]
    pub global: GlobalConfig,

    /// 后端配置
    #[config(flatten)]
    pub backend: BackendConfig,

    /// 服务配置（JSON 格式）
    #[config(default = String::new())]
    pub services_json: String,

    /// 性能配置
    #[config(flatten)]
    pub performance: PerformanceConfig,

    /// 指标配置
    #[config(flatten)]
    pub metrics: MetricsConfig,

    /// 恢复配置
    #[config(flatten)]
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
            .map_err(|e| crate::error::CacheError::ConfigError(format!("读取文件 '{}' 失败: {}", path, e)))?;

        let config: Self = toml::from_str(&content)
            .map_err(|e| crate::error::CacheError::ConfigError(format!("解析 TOML '{}' 失败: {}", path, e)))?;

        config.validate_config()?;

        Ok(config)
    }

    /// 从 JSON 文件加载
    pub fn from_json_file(path: &str) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::CacheError::ConfigError(format!("读取文件 '{}' 失败: {}", path, e)))?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| crate::error::CacheError::ConfigError(format!("解析 JSON '{}' 失败: {}", path, e)))?;

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
            _ => Err(crate::error::CacheError::ConfigError(format!(
                "不支持的配置文件格式: '{}'. 支持格式: .toml, .json",
                path
            ))),
        }
    }

    /// 验证配置内容
    pub fn validate_config(&self) -> crate::error::Result<()> {
        self.global
            .validate()
            .map_err(|e| crate::error::CacheError::ConfigError(format!("全局配置验证失败: {}", e)))?;
        self.performance
            .validate()
            .map_err(|e| crate::error::CacheError::ConfigError(format!("性能配置验证失败: {}", e)))?;
        for (name, service) in self.services() {
            service
                .validate()
                .map_err(|e| crate::error::CacheError::ConfigError(format!("服务 '{}' 配置验证失败: {}", name, e)))?;
        }
        Ok(())
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
}
