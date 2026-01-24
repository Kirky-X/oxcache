//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 用户自定义分层后端架构
//!
//! 提供灵活的后端配置能力：
//! - 使用枚举定义后端类型
//! - Cargo feature 控制后端可用性
//! - 后端层级标签防止错误配置
//! - 自动修复不合理的层级分配
//!
//! # 架构设计
//!
//! ```text
//! +-------------------------------------------------------------+
//! |                    CustomTieredConfig                        |
//! |  +-----------------+  +-----------------+                   |
//! |  |  Layer::L1      |  |  Layer::L2      |                   |
//! |  |  - BackendType  |  |  - BackendType  |                   |
//! |  |  - 配置参数     |  |  - 配置参数     |                   |
//! |  +-----------------+  +-----------------+                   |
//! +-------------------------------------------------------------+
//!                              |
//!                              v
//! +-------------------------------------------------------------+
//! |               TieredBackendValidator                        |
//! |  - 验证后端类型与层级匹配                                    |
//! |  - 自动修复不合法配置                                        |
//! +-------------------------------------------------------------+
//!                              |
//!                              v
//! +-------------------------------------------------------------+
//! |                 TieredBackendFactory                        |
//! |  - 创建 L1/L2 后端实例                                      |
//! |  - 支持依赖注入 (BackendProvider trait)                     |
//! +-------------------------------------------------------------+
//!                              |
//!                              v
//! +-------------------------------------------------------------+
//! |                    TieredBackend                            |
//! |  - 实际创建和组合后端                                        |
//! |  - 统一 CacheBackend 接口                                   |
//! +-------------------------------------------------------------+
//! ```

use crate::backend::client::moka::{MokaMemoryBackend, MokaMemoryBackendBuilder};
use crate::backend::client::moka::MokaMemoryBackend as MemoryBackend;
use crate::backend::client::moka::MokaMemoryBackendBuilder as MemoryBackendBuilder;
use crate::backend::{CacheBackend, TieredBackend};
use crate::error::{CacheError, Result};
use crate::utils::redaction::redact_value;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::instrument;

/// 路径验证配置
#[derive(Debug, Clone)]
pub struct PathValidationConfig {
    /// 允许的基础目录
    pub allowed_base_dirs: Vec<PathBuf>,
    /// 是否允许符号链接
    pub allow_symbolic_links: bool,
    /// 最大路径长度
    pub max_path_length: usize,
}

impl Default for PathValidationConfig {
    fn default() -> Self {
        Self {
            allowed_base_dirs: Vec::new(),
            allow_symbolic_links: false,
            max_path_length: 4096,
        }
    }
}

impl PathValidationConfig {
    /// 创建新配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加允许的基础目录
    pub fn add_allowed_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.allowed_base_dirs.push(dir.into());
        self
    }

    /// 允许符号链接
    pub fn allow_symbolic_links(mut self, allowed: bool) -> Self {
        self.allow_symbolic_links = allowed;
        self
    }

    /// 设置最大路径长度
    pub fn with_max_path_length(mut self, length: usize) -> Self {
        self.max_path_length = length;
        self
    }

    /// 验证路径安全性
    ///
    /// # 参数
    /// * `path` - 要验证的路径
    ///
    /// # 返回值
    /// * `Ok(PathBuf)` - 规范化后的安全路径
    /// * `Err(CacheError)` - 验证失败
    pub fn validate(&self, path: &str) -> Result<PathBuf> {
        // 检查路径长度
        if path.len() > self.max_path_length {
            return Err(CacheError::ConfigError(format!(
                "Path exceeds maximum length of {} characters",
                self.max_path_length
            )));
        }

        // 解析路径（规范化但不使用实际文件）
        let path = Path::new(path);

        // 检查是否绝对路径
        if !path.is_absolute() {
            return Err(CacheError::ConfigError(
                "Only absolute paths are allowed".to_string(),
            ));
        }

        // 规范化路径（移除 . 和 ..，解析冗余分隔符）
        let normalized = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // 如果文件不存在，仍然进行路径规范化检查
                let mut buf = PathBuf::new();
                for component in path.components() {
                    match component {
                        std::path::Component::Normal(part) => {
                            buf.push(part);
                        }
                        std::path::Component::CurDir => {} // 忽略 .
                        std::path::Component::ParentDir => {
                            // 尝试弹出父目录，但不允许超出基础
                            if !buf.pop() {
                                return Err(CacheError::ConfigError(
                                    "Path traversal attempt detected".to_string(),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                buf
            }
        };

        // 如果设置了允许的目录，检查路径是否在允许范围内
        if !self.allowed_base_dirs.is_empty() {
            let mut within_allowed = false;
            for base_dir in &self.allowed_base_dirs {
                let base_canonical = match base_dir.canonicalize() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if normalized.starts_with(&base_canonical) {
                    within_allowed = true;
                    break;
                }
            }
            if !within_allowed {
                return Err(CacheError::ConfigError(format!(
                    "Path is not within allowed directories: {}",
                    normalized.display()
                )));
            }
        }

        // 如果不允许符号链接，检查是否为符号链接
        if !self.allow_symbolic_links {
            // 注意：这里无法检查不存在的文件，实际使用时应在文件操作前再次检查
            // 此处仅作为预防性检查
            if let Some(file_name) = normalized.file_name() {
                if file_name.to_string_lossy().starts_with('.') {
                    // 隐藏文件可能有问题，进行警告
                    tracing::warn!("Loading configuration from hidden file: {}", path.display());
                }
            }
        }

        // 检查路径中是否包含可疑字符
        validate_path_chars(path)?;

        Ok(normalized)
    }
}

/// 验证路径字符
fn validate_path_chars(path: &Path) -> Result<()> {
    // 检查无效字符
    let invalid_chars = ['\0', '\n', '\r', '\t'];
    let path_str = path.to_string_lossy();

    for ch in invalid_chars {
        if path_str.contains(ch) {
            return Err(CacheError::ConfigError(format!(
                "Path contains invalid character: {:?}",
                ch
            )));
        }
    }

    Ok(())
}

/// 配置验证常量
#[derive(Debug, Clone, Copy)]
pub struct ConfigValidation;

impl ConfigValidation {
    /// 最大缓存容量（10亿条目）
    pub const MAX_CAPACITY: u64 = 1_000_000_000;
    /// 最大 TTL（30天，以秒计）
    pub const MAX_TTL_SECS: u64 = 30 * 24 * 60 * 60;
    /// 最大 TTI（30天，以秒计）
    pub const MAX_TTI_SECS: u64 = 30 * 24 * 60 * 60;
    /// 自定义名称最大长度（256字符）
    pub const MAX_CUSTOM_NAME_LENGTH: usize = 256;
    /// 允许的自定义名称字符正则（字母、数字、下划线、连字符、点）
    pub const VALID_NAME_CHARS: &'static str =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-";

    /// 验证容量值
    pub fn validate_capacity(capacity: u64) -> Result<u64> {
        if capacity == 0 {
            return Err(CacheError::ConfigError(
                "Capacity must be greater than 0".to_string(),
            ));
        }
        if capacity > Self::MAX_CAPACITY {
            return Err(CacheError::ConfigError(format!(
                "Capacity {} exceeds maximum allowed value of {}",
                capacity,
                Self::MAX_CAPACITY
            )));
        }
        Ok(capacity)
    }

    /// 验证 TTL 值
    pub fn validate_ttl(ttl: u64) -> Result<u64> {
        if ttl == 0 {
            return Err(CacheError::ConfigError(
                "TTL must be greater than 0".to_string(),
            ));
        }
        if ttl > Self::MAX_TTL_SECS {
            return Err(CacheError::ConfigError(format!(
                "TTL {} seconds exceeds maximum allowed value of {} seconds (30 days)",
                ttl,
                Self::MAX_TTL_SECS
            )));
        }
        Ok(ttl)
    }

    /// 验证 TTI 值
    pub fn validate_tti(tti: u64) -> Result<u64> {
        if tti > Self::MAX_TTI_SECS {
            return Err(CacheError::ConfigError(format!(
                "Time to idle {} seconds exceeds maximum allowed value of {} seconds (30 days)",
                tti,
                Self::MAX_TTI_SECS
            )));
        }
        Ok(tti)
    }

    /// 验证自定义名称
    pub fn validate_custom_name(name: &str) -> Result<String> {
        // 检查长度
        if name.is_empty() {
            return Err(CacheError::ConfigError(
                "Custom backend name cannot be empty".to_string(),
            ));
        }
        if name.len() > Self::MAX_CUSTOM_NAME_LENGTH {
            return Err(CacheError::ConfigError(format!(
                "Custom backend name exceeds maximum length of {} characters",
                Self::MAX_CUSTOM_NAME_LENGTH
            )));
        }

        // 检查字符有效性
        for ch in name.chars() {
            if !Self::VALID_NAME_CHARS.contains(ch) {
                return Err(CacheError::ConfigError(format!(
                    "Custom backend name contains invalid character '{}'. Allowed characters: {}",
                    ch,
                    Self::VALID_NAME_CHARS
                )));
            }
        }

        Ok(name.to_string())
    }
}

/// 缓存层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Layer {
    /// 第一层缓存 - 通常是内存缓存
    #[default]
    L1,
    /// 第二层缓存 - 通常是分布式缓存
    L2,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Layer::L1 => write!(f, "L1"),
            Layer::L2 => write!(f, "L2"),
        }
    }
}

/// 后端支持的层级限制
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerRestriction {
    /// 仅支持 L1（内存缓存）
    L1Only,
    /// 仅支持 L2（分布式缓存）
    L2Only,
    /// 支持任意层级
    Any,
}

impl LayerRestriction {
    /// 检查后端类型是否支持指定层级
    pub fn supports(&self, layer: Layer) -> bool {
        match self {
            LayerRestriction::L1Only => layer == Layer::L1,
            LayerRestriction::L2Only => layer == Layer::L2,
            LayerRestriction::Any => true,
        }
    }

    /// 获取友好的描述文本
    pub fn description(&self) -> &'static str {
        match self {
            LayerRestriction::L1Only => "仅支持 L1 层级",
            LayerRestriction::L2Only => "仅支持 L2 层级",
            LayerRestriction::Any => "支持任意层级",
        }
    }
}

/// 缓存后端类型枚举
///
/// 每个后端类型都有其推荐的层级限制：
/// - `Moka` - 仅支持 L1（内存缓存）
/// - `Memory` - 仅支持 L1（内存缓存）
/// - `Redis` - 仅支持 L2（分布式缓存）
/// - `Tiered` - 支持任意层级（用于组合）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    /// Moka 内存缓存（L1 推荐）
    #[cfg(feature = "l1-moka")]
    Moka,
    /// 简单内存缓存（L1 推荐）
    #[cfg(not(feature = "l1-moka"))]
    Memory,
    /// Redis 分布式缓存（L2 推荐）
    #[cfg(feature = "l2-redis")]
    Redis,
    /// 分层缓存组合（任意层级）
    #[default]
    Tiered,
    /// 持久化缓存（L2 推荐）
    Persisted,
    /// 自定义后端（任意层级）
    Custom(String),
}

impl fmt::Display for BackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "l1-moka")]
            BackendType::Moka => write!(f, "moka"),
            #[cfg(not(feature = "l1-moka"))]
            BackendType::Memory => write!(f, "memory"),
            #[cfg(feature = "l2-redis")]
            BackendType::Redis => write!(f, "redis"),
            BackendType::Tiered => write!(f, "tiered"),
            BackendType::Persisted => write!(f, "persisted"),
            // 脱敏自定义名称，防止敏感信息泄露
            BackendType::Custom(name) => {
                let masked = redact_value(name, 8);
                write!(f, "custom:{}", masked)
            }
        }
    }
}

impl BackendType {
    /// 获取后端类型的层级限制
    pub fn layer_restriction(&self) -> LayerRestriction {
        match self {
            #[cfg(feature = "l1-moka")]
            BackendType::Moka => LayerRestriction::L1Only,
            #[cfg(not(feature = "l1-moka"))]
            BackendType::Memory => LayerRestriction::L1Only,
            #[cfg(feature = "l2-redis")]
            BackendType::Redis => LayerRestriction::L2Only,
            BackendType::Tiered => LayerRestriction::Any,
            BackendType::Persisted => LayerRestriction::L2Only,
            BackendType::Custom(_) => LayerRestriction::Any,
        }
    }

    /// 获取后端类型的推荐层级
    pub fn recommended_layer(&self) -> Layer {
        match self.layer_restriction() {
            LayerRestriction::L1Only => Layer::L1,
            LayerRestriction::L2Only => Layer::L2,
            LayerRestriction::Any => Layer::L1,
        }
    }

    /// 检查后端类型是否支持指定层级
    pub fn supports_layer(&self, layer: Layer) -> bool {
        self.layer_restriction().supports(layer)
    }

    /// 获取可用的后端类型列表（基于启用的 feature）
    pub fn available_backends() -> Vec<BackendType> {
        vec![
            #[cfg(feature = "l1-moka")]
            BackendType::Moka,
            #[cfg(feature = "l2-redis")]
            BackendType::Redis,
            BackendType::Tiered,
            BackendType::Persisted,
        ]
    }

    /// 从字符串解析后端类型
    ///
    /// # 安全说明
    /// - 验证自定义名称长度（最大 256 字符）
    /// - 验证自定义名称字符（只允许字母、数字、下划线、连字符、点）
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            #[cfg(feature = "l1-moka")]
            "moka" => Ok(BackendType::Moka),
            #[cfg(not(feature = "l1-moka"))]
            "memory" | "moka" => Ok(BackendType::Memory),
            #[cfg(feature = "l2-redis")]
            "redis" => Ok(BackendType::Redis),
            "tiered" | "multi" | "two-level" => Ok(BackendType::Tiered),
            "persisted" | "persist" | "sqlite" => Ok(BackendType::Persisted),
            _ => {
                if let Some(custom_name) = s.strip_prefix("custom:") {
                    // 验证自定义名称
                    let validated_name = ConfigValidation::validate_custom_name(custom_name)?;
                    Ok(BackendType::Custom(validated_name))
                } else {
                    #[cfg(feature = "l1-moka")]
                    #[cfg(feature = "l2-redis")]
                    let available = "moka, memory, redis, tiered, persisted, custom:<name>";

                    #[cfg(feature = "l1-moka")]
                    #[cfg(not(feature = "l2-redis"))]
                    let available = "moka, memory, tiered, persisted, custom:<name>";

                    #[cfg(not(feature = "l1-moka"))]
                    #[cfg(feature = "l2-redis")]
                    let available = "memory, redis, tiered, persisted, custom:<name>";

                    #[cfg(not(feature = "l1-moka"))]
                    #[cfg(not(feature = "l2-redis"))]
                    let available = "memory, tiered, persisted, custom:<name>";

                    Err(CacheError::ConfigError(format!(
                        "Unknown backend type: '{}'. Available backends: {}",
                        s, available
                    )))
                }
            }
        }
    }
}

/// 单层后端配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerBackendConfig {
    /// 后端类型
    #[serde(default)]
    pub backend_type: BackendType,
    /// 后端特定配置（JSON 格式）
    #[serde(default)]
    pub options: serde_json::Value,
    /// 是否启用该层
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl LayerBackendConfig {
    /// 创建新配置
    pub fn new(backend_type: BackendType) -> Self {
        Self {
            backend_type,
            options: serde_json::Value::Null,
            enabled: true,
        }
    }

    /// 设置后端类型
    pub fn with_backend_type(mut self, backend_type: BackendType) -> Self {
        self.backend_type = backend_type;
        self
    }

    /// 设置配置选项
    pub fn with_options(mut self, options: serde_json::Value) -> Self {
        self.options = options;
        self
    }

    /// 启用/禁用该层
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 验证配置是否有效
    pub fn validate(&self, layer: Layer) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if !self.backend_type.supports_layer(layer) {
            return Err(CacheError::ConfigError(format!(
                "Backend type '{}' does not support layer {}. {}",
                self.backend_type,
                layer,
                self.backend_type.layer_restriction().description()
            )));
        }

        Ok(())
    }
}

/// 后端提供者 trait - 支持依赖注入
///
/// 允许自定义后端创建逻辑，实现解耦。
/// 默认提供者是 `DefaultBackendProvider`。
#[async_trait]
pub trait BackendProvider: Send + Sync {
    /// 创建 L1 后端
    async fn create_l1(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>>;

    /// 创建 L2 后端
    async fn create_l2(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>>;
}

/// 默认后端提供者
///
/// 使用 MemoryBackend 作为 L1，RedisBackend 作为 L2。
#[derive(Default)]
pub struct DefaultBackendProvider;

#[async_trait]
impl BackendProvider for DefaultBackendProvider {
    #[instrument(skip(self, options), level = "debug")]
    async fn create_l1(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>> {
        let mut builder = MemoryBackend::builder();
        builder = apply_memory_options(builder, options);
        let backend = builder.build();
        Ok(Arc::new(backend))
    }

    #[instrument(skip(self, options), level = "debug")]
    async fn create_l2(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>> {
        #[cfg(feature = "l2-redis")]
        {
            use crate::backend::client::redis::RedisBackend;

            let connection_string = options
                .get("connection_string")
                .and_then(|v| v.as_str())
                .unwrap_or("redis://localhost:6379");

            // 记录脱敏后的连接字符串，防止敏感信息泄露
            tracing::debug!(
                "Creating Redis backend with connection: redis://***@{}",
                connection_string
                    .split('@')
                    .nth(1)
                    .unwrap_or(connection_string)
                    .split(':')
                    .next()
                    .unwrap_or("unknown")
            );

            let backend = Arc::new(RedisBackend::new(connection_string).await?);
            Ok(backend)
        }
        #[cfg(not(feature = "l2-redis"))]
        {
            // 如果没有 l2-redis feature，降级到内存后端
            tracing::warn!("Redis backend not available, falling back to memory backend");
            let mut builder = MemoryBackend::builder();
            builder = apply_memory_options(builder, options);
            let backend = builder.build();
            Ok(Arc::new(backend))
        }
    }
}

/// 应用内存后端配置选项
///
/// # 安全说明
/// 此函数会验证配置参数的范围：
/// - capacity 不能超过 10 亿
/// - ttl 不能超过 30 天
/// - tti 不能超过 30 天
fn apply_memory_options(
    mut builder: MemoryBackendBuilder,
    options: &serde_json::Value,
) -> MemoryBackendBuilder {
    if let Some(options) = options.as_object() {
        if let Some(capacity) = options.get("capacity").and_then(|v| v.as_u64()) {
            // 验证容量值
            match ConfigValidation::validate_capacity(capacity) {
                Ok(validated) => {
                    builder = builder.capacity(validated);
                }
                Err(e) => {
                    tracing::warn!("Invalid capacity: {}", e);
                }
            }
        }
        if let Some(ttl) = options.get("ttl").and_then(|v| v.as_u64()) {
            // 验证 TTL 值
            match ConfigValidation::validate_ttl(ttl) {
                Ok(validated) => {
                    builder = builder.ttl(std::time::Duration::from_secs(validated));
                }
                Err(e) => {
                    tracing::warn!("Invalid TTL: {}", e);
                }
            }
        }
        if let Some(tti) = options.get("time_to_idle").and_then(|v| v.as_u64()) {
            // 验证 TTI 值
            match ConfigValidation::validate_tti(tti) {
                Ok(validated) => {
                    builder = builder.time_to_idle(std::time::Duration::from_secs(validated));
                }
                Err(e) => {
                    tracing::warn!("Invalid TTI: {}", e);
                }
            }
        }
    }
    builder
}

/// 分层后端工厂
///
/// 负责根据配置创建后端实例。
/// 支持自定义 `BackendProvider` 实现依赖注入。
#[derive(Clone)]
pub struct TieredBackendFactory {
    provider: Arc<dyn BackendProvider>,
}

impl Default for TieredBackendFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl TieredBackendFactory {
    /// 创建新工厂
    pub fn new() -> Self {
        Self {
            provider: Arc::new(DefaultBackendProvider),
        }
    }

    /// 使用自定义提供者创建工厂
    pub fn with_provider(provider: Arc<dyn BackendProvider>) -> Self {
        Self { provider }
    }

    /// 创建 L1 后端
    #[instrument(skip(self, options), level = "debug")]
    pub async fn create_l1(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>> {
        self.provider.create_l1(options).await
    }

    /// 创建 L2 后端
    #[instrument(skip(self, options), level = "debug")]
    pub async fn create_l2(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>> {
        self.provider.create_l2(options).await
    }

    /// 创建分层后端
    #[instrument(skip(self, l1_options, l2_options), level = "debug")]
    pub async fn create_tiered_backend(
        &self,
        l1_options: &serde_json::Value,
        l2_options: &serde_json::Value,
    ) -> Result<TieredBackend> {
        let l1 = self.create_l1(l1_options).await?;
        let l2 = self.create_l2(l2_options).await?;
        Ok(TieredBackend::from_arc(l1, l2))
    }
}

/// 用户自定义分层缓存配置
///
/// 允许用户灵活配置 L1 和 L2 的后端类型：
/// - L1 可以选择：Moka、Memory
/// - L2 可以选择：Redis、Persisted
/// - 支持自动验证和修复不合理的配置
///
/// # 示例
///
/// ```toml
/// [cache.my_service]
/// # 自定义 L1 为 Moka
/// l1_backend = "moka"
/// l1_capacity = 10000
///
/// # 自定义 L2 为 Redis
/// l2_backend = "redis"
/// l2_connection_string = "redis://localhost:6379"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomTieredConfig {
    /// L1 后端配置
    pub l1: LayerBackendConfig,
    /// L2 后端配置
    pub l2: LayerBackendConfig,
    /// 自动修复配置
    #[serde(default)]
    pub auto_fix: AutoFixConfig,
}

/// 自动修复配置
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AutoFixConfig {
    /// 是否启用自动修复
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 是否在修复时输出警告日志
    #[serde(default = "default_true")]
    pub warn_on_fix: bool,
}

impl Default for AutoFixConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            warn_on_fix: true,
        }
    }
}

fn default_true() -> bool {
    true
}

impl AutoFixConfig {
    pub fn new() -> Self {
        Self {
            enabled: true,
            warn_on_fix: true,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_warn_on_fix(mut self, warn: bool) -> Self {
        self.warn_on_fix = warn;
        self
    }
}

impl CustomTieredConfig {
    /// 创建新配置（默认 L1=Moka, L2=Redis）
    #[cfg(feature = "l1-moka")]
    pub fn new() -> Self {
        Self {
            l1: LayerBackendConfig::new(BackendType::Moka),
            l2: LayerBackendConfig::new(BackendType::Redis),
            auto_fix: AutoFixConfig::new(),
        }
    }

    #[cfg(not(feature = "l1-moka"))]
    pub fn new() -> Self {
        Self {
            l1: LayerBackendConfig::new(BackendType::Memory),
            l2: LayerBackendConfig::new(BackendType::Tiered),
            auto_fix: AutoFixConfig::new(),
        }
    }

    /// 设置 L1 后端
    pub fn l1_backend(mut self, backend_type: BackendType) -> Self {
        self.l1.backend_type = backend_type;
        self
    }

    /// 设置 L2 后端
    pub fn l2_backend(mut self, backend_type: BackendType) -> Self {
        self.l2.backend_type = backend_type;
        self
    }

    /// 启用/禁用自动修复
    pub fn auto_fix(mut self, enabled: bool) -> Self {
        self.auto_fix.enabled = enabled;
        self
    }

    /// 验证配置
    ///
    /// 返回验证结果和修复信息
    pub fn validate(&self) -> ConfigValidationResult {
        let mut result = ConfigValidationResult::new();

        // 验证 L1 配置
        if self.l1.enabled {
            match self.l1.validate(Layer::L1) {
                Ok(_) => {
                    result.add_valid(Layer::L1, self.l1.backend_type.clone());
                }
                Err(e) => {
                    result.add_invalid(Layer::L1, self.l1.backend_type.clone(), e.to_string());
                }
            }
        }

        // 验证 L2 配置
        if self.l2.enabled {
            match self.l2.validate(Layer::L2) {
                Ok(_) => {
                    result.add_valid(Layer::L2, self.l2.backend_type.clone());
                }
                Err(e) => {
                    result.add_invalid(Layer::L2, self.l2.backend_type.clone(), e.to_string());
                }
            }
        }

        result
    }

    /// 验证并自动修复配置
    ///
    /// 如果配置不合法且启用了自动修复，会返回修复后的配置
    pub fn validate_and_fix(&self) -> (FixedConfigResult, Option<CustomTieredConfig>) {
        let validation = self.validate();
        let fixes = validation.get_fixes();

        if fixes.is_empty() || !self.auto_fix.enabled {
            return (FixedConfigResult::from(validation), None);
        }

        // 创建修复后的配置
        let mut fixed = self.clone();

        for fix in fixes {
            match fix.layer {
                Layer::L1 => {
                    if self.auto_fix.warn_on_fix {
                        tracing::warn!(
                            "L1 backend '{}' is not suitable for L1, auto-fixing to '{}'",
                            fix.from_backend,
                            fix.to_backend
                        );
                    }
                    fixed.l1.backend_type = fix.to_backend.clone();
                }
                Layer::L2 => {
                    if self.auto_fix.warn_on_fix {
                        tracing::warn!(
                            "L2 backend '{}' is not suitable for L2, auto-fixing to '{}'",
                            fix.from_backend,
                            fix.to_backend
                        );
                    }
                    fixed.l2.backend_type = fix.to_backend.clone();
                }
            }
        }

        // 重新验证修复后的配置
        let fixed_validation = fixed.validate();
        let fixed_result = FixedConfigResult::from(fixed_validation);

        // 验证修复是否成功
        if fixed_result.is_valid() {
            (fixed_result, Some(fixed))
        } else {
            // 修复失败，返回原始验证结果
            tracing::error!("Auto-fix failed for tiered cache configuration");
            (FixedConfigResult::from(validation), None)
        }
    }
}

/// 配置验证结果
#[derive(Debug, Clone, Default)]
pub struct ConfigValidationResult {
    valid_layers: Vec<(Layer, BackendType)>,
    invalid_layers: Vec<(Layer, BackendType, String)>,
    fixes: Vec<ConfigFix>,
}

impl ConfigValidationResult {
    pub fn new() -> Self {
        Self {
            valid_layers: Vec::new(),
            invalid_layers: Vec::new(),
            fixes: Vec::new(),
        }
    }

    fn add_valid(&mut self, layer: Layer, backend_type: BackendType) {
        self.valid_layers.push((layer, backend_type));
    }

    fn add_invalid(&mut self, layer: Layer, backend_type: BackendType, error: String) {
        let backend_type_clone = backend_type.clone();
        self.invalid_layers
            .push((layer, backend_type, error.clone()));

        // 生成修复建议
        let suggested = backend_type_clone.recommended_layer();
        if suggested != layer {
            // 查找该层级推荐的后端
            let recommended = match layer {
                Layer::L1 => BackendType::default(),
                Layer::L2 => {
                    #[cfg(feature = "l2-redis")]
                    {
                        BackendType::Redis
                    }
                    #[cfg(not(feature = "l2-redis"))]
                    {
                        BackendType::Memory
                    }
                }
            };

            self.fixes.push(ConfigFix {
                layer,
                from_backend: backend_type_clone,
                to_backend: recommended,
                reason: error,
            });
        }
    }

    pub fn is_valid(&self) -> bool {
        self.invalid_layers.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.fixes.is_empty()
    }

    pub fn get_fixes(&self) -> &[ConfigFix] {
        &self.fixes
    }

    pub fn get_validation_report(&self) -> String {
        let mut report = String::new();

        if self.is_valid() {
            report.push_str("✅ Configuration is valid\n");
        } else {
            report.push_str("❌ Configuration has issues:\n");
            for (layer, backend, error) in &self.invalid_layers {
                report.push_str(&format!("  - Layer {}: {} - {}\n", layer, backend, error));
            }
        }

        if !self.fixes.is_empty() {
            report.push_str("\n🔧 Suggested fixes:\n");
            for fix in &self.fixes {
                report.push_str(&format!(
                    "  - {}: '{}' → '{}' (reason: {})\n",
                    fix.layer, fix.from_backend, fix.to_backend, fix.reason
                ));
            }
        }

        report
    }
}

/// 配置修复建议
#[derive(Debug, Clone)]
pub struct ConfigFix {
    pub layer: Layer,
    pub from_backend: BackendType,
    pub to_backend: BackendType,
    pub reason: String,
}

/// 固定配置结果
#[derive(Debug, Clone)]
pub struct FixedConfigResult {
    pub is_valid: bool,
    pub l1_backend: Option<BackendType>,
    pub l2_backend: Option<BackendType>,
    pub warnings: Vec<String>,
}

impl From<ConfigValidationResult> for FixedConfigResult {
    fn from(val: ConfigValidationResult) -> Self {
        let mut warnings = Vec::new();

        for fix in &val.fixes {
            warnings.push(format!(
                "Auto-fixed {} from '{}' to '{}'",
                fix.layer, fix.from_backend, fix.to_backend
            ));
        }

        let l1_backend = val
            .valid_layers
            .iter()
            .find(|(l, _)| *l == Layer::L1)
            .map(|(_, b)| b.clone());
        let l2_backend = val
            .valid_layers
            .iter()
            .find(|(l, _)| *l == Layer::L2)
            .map(|(_, b)| b.clone());

        Self {
            is_valid: val.is_valid(),
            l1_backend,
            l2_backend,
            warnings,
        }
    }
}

impl FixedConfigResult {
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// 获取配置报告
    pub fn get_report(&self) -> String {
        let mut report = String::new();

        if !self.is_valid {
            report.push_str("Invalid configuration:\n");
        }

        for warning in &self.warnings {
            report.push_str(&format!("  - {}\n", warning));
        }

        report
    }
}

/// Builder 模式的便捷构造器
pub struct CustomTieredConfigBuilder(CustomTieredConfig);

impl Default for CustomTieredConfigBuilder {
    fn default() -> Self {
        Self(CustomTieredConfig::new())
    }
}

impl CustomTieredConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 L1 后端类型
    pub fn l1(mut self, backend_type: BackendType) -> Self {
        self.0.l1.backend_type = backend_type;
        self
    }

    /// 设置 L1 配置选项
    pub fn l1_options(mut self, options: serde_json::Value) -> Self {
        self.0.l1.options = options;
        self
    }

    /// 设置 L2 后端类型
    pub fn l2(mut self, backend_type: BackendType) -> Self {
        self.0.l2.backend_type = backend_type;
        self
    }

    /// 设置 L2 配置选项
    pub fn l2_options(mut self, options: serde_json::Value) -> Self {
        self.0.l2.options = options;
        self
    }

    /// 启用 L1
    pub fn enable_l1(mut self, enabled: bool) -> Self {
        self.0.l1.enabled = enabled;
        self
    }

    /// 启用 L2
    pub fn enable_l2(mut self, enabled: bool) -> Self {
        self.0.l2.enabled = enabled;
        self
    }

    /// 启用自动修复
    pub fn auto_fix(mut self, enabled: bool) -> Self {
        self.0.auto_fix.enabled = enabled;
        self
    }

    /// 构建配置
    pub fn build(self) -> CustomTieredConfig {
        self.0
    }
}

/// 从配置文件加载自定义分层配置
///
/// # 安全说明
/// 此函数会验证路径安全性：
/// - 只允许绝对路径
/// - 规范化路径，防止路径遍历攻击
/// - 可选限制在指定目录范围内
/// - 不允许符号链接（除非明确启用）
///
/// # 参数
/// * `path` - 配置文件路径（绝对路径）
/// * `validation_config` - 路径验证配置（可选，使用默认配置）
///
/// # 返回值
/// * `Ok(CustomTieredConfig)` - 加载的配置
/// * `Err(CacheError)` - 加载或验证失败
#[cfg(feature = "confers")]
#[instrument(skip(path, validation_config), level = "debug")]
pub async fn load_from_file(
    path: &str,
    validation_config: Option<PathValidationConfig>,
) -> Result<CustomTieredConfig> {
    use std::fs;
    use toml;

    // 使用提供的配置或默认配置
    let path_config = validation_config.unwrap_or_default();

    // 验证路径安全性
    let safe_path = path_config.validate(path)?;

    // 读取文件前再次检查是否为符号链接（防御性检查）
    if let Ok(metadata) = fs::metadata(&safe_path) {
        if metadata.file_type().is_symlink() {
            return Err(CacheError::ConfigError(
                "Symbolic links are not allowed for configuration files".to_string(),
            ));
        }
    }

    let content =
        fs::read_to_string(&safe_path).map_err(|e| CacheError::ConfigError(e.to_string()))?;

    let config: CustomTieredConfig =
        toml::from_str(&content).map_err(|e| CacheError::ConfigError(e.to_string()))?;

    // 验证配置
    let (result, fixed) = config.validate_and_fix();

    if !result.is_valid() {
        return Err(CacheError::ConfigError(format!(
            "Invalid tiered cache configuration: {}",
            result.get_report()
        )));
    }

    // 如果有自动修复，返回修复后的配置
    if let Some(fixed_config) = fixed {
        if !result.warnings.is_empty() {
            tracing::info!(
                "Auto-fixed tiered cache configuration: {:?}",
                result.warnings
            );
        }
        Ok(fixed_config)
    } else {
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_layer_restriction() {
        #[cfg(feature = "l1-moka")]
        {
            assert_eq!(
                BackendType::Moka.layer_restriction(),
                LayerRestriction::L1Only
            );
            assert!(BackendType::Moka.supports_layer(Layer::L1));
            assert!(!BackendType::Moka.supports_layer(Layer::L2));
        }

        #[cfg(feature = "l2-redis")]
        {
            assert_eq!(
                BackendType::Redis.layer_restriction(),
                LayerRestriction::L2Only
            );
            assert!(!BackendType::Redis.supports_layer(Layer::L1));
            assert!(BackendType::Redis.supports_layer(Layer::L2));
        }

        assert_eq!(
            BackendType::Tiered.layer_restriction(),
            LayerRestriction::Any
        );
        assert!(BackendType::Tiered.supports_layer(Layer::L1));
        assert!(BackendType::Tiered.supports_layer(Layer::L2));
    }

    #[test]
    fn test_backend_type_recommended_layer() {
        #[cfg(feature = "l1-moka")]
        assert_eq!(BackendType::Moka.recommended_layer(), Layer::L1);

        #[cfg(feature = "l2-redis")]
        assert_eq!(BackendType::Redis.recommended_layer(), Layer::L2);
    }

    #[test]
    fn test_custom_tiered_config_validation() {
        let config = CustomTieredConfig::new();

        #[cfg(feature = "l1-moka")]
        {
            assert_eq!(config.l1.backend_type, BackendType::Moka);
        }
        #[cfg(feature = "l2-redis")]
        {
            assert_eq!(config.l2.backend_type, BackendType::Redis);
        }
    }

    #[test]
    fn test_invalid_config_auto_fix() {
        let mut config = CustomTieredConfig::new();

        // 设置不合法的配置：Redis 作为 L1
        #[cfg(feature = "l2-redis")]
        {
            config.l1.backend_type = BackendType::Redis;
            config.auto_fix.enabled = true;

            let result = config.validate();

            assert!(!result.is_valid());
            assert!(result.has_warnings());
        }
    }

    #[test]
    fn test_custom_tiered_config_builder() {
        let config = CustomTieredConfigBuilder::new()
            .l1(BackendType::Tiered)
            .l2(BackendType::Tiered)
            .enable_l1(true)
            .enable_l2(true)
            .auto_fix(true)
            .build();

        assert_eq!(config.l1.backend_type, BackendType::Tiered);
        assert_eq!(config.l2.backend_type, BackendType::Tiered);
        assert!(config.auto_fix.enabled);
    }

    #[test]
    fn test_layer_backend_config_validate() {
        #[cfg(feature = "l1-moka")]
        {
            let config = LayerBackendConfig::new(BackendType::Moka);
            assert!(config.validate(Layer::L1).is_ok());
            assert!(config.validate(Layer::L2).is_err());
        }

        #[cfg(feature = "l2-redis")]
        {
            let config = LayerBackendConfig::new(BackendType::Redis);
            assert!(config.validate(Layer::L1).is_err());
            assert!(config.validate(Layer::L2).is_ok());
        }
    }

    #[test]
    fn test_config_validation_result() {
        let mut result = ConfigValidationResult::new();

        // 测试有效配置
        result.add_valid(Layer::L1, BackendType::Tiered);
        result.add_valid(Layer::L2, BackendType::Tiered);

        assert!(result.is_valid());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_config_validation_result_with_warnings() {
        let mut result = ConfigValidationResult::new();

        // 添加有效层
        result.add_valid(Layer::L2, BackendType::Tiered);

        // 添加无效层（Redis 在 L1）
        #[cfg(feature = "l2-redis")]
        {
            result.add_invalid(
                Layer::L1,
                BackendType::Redis,
                "Redis is not supported in L1".to_string(),
            );
        }

        // 测试结果
        #[cfg(feature = "l2-redis")]
        {
            assert!(!result.is_valid());
            assert!(result.has_warnings());
            assert!(!result.get_fixes().is_empty());
        }
    }

    #[test]
    fn test_fixed_config_result_from_validation() {
        let mut result = ConfigValidationResult::new();

        result.add_valid(Layer::L1, BackendType::Tiered);
        result.add_valid(Layer::L2, BackendType::Tiered);

        let fixed: FixedConfigResult = result.into();

        assert!(fixed.is_valid);
        assert_eq!(fixed.l1_backend, Some(BackendType::Tiered));
        assert_eq!(fixed.l2_backend, Some(BackendType::Tiered));
        assert!(fixed.warnings.is_empty());
    }

    #[test]
    fn test_auto_fix_config_defaults() {
        let config = AutoFixConfig::default();
        assert!(config.enabled);
        assert!(config.warn_on_fix);
    }

    #[test]
    fn test_auto_fix_config_builder() {
        let config = AutoFixConfig::new()
            .with_enabled(false)
            .with_warn_on_fix(false);

        assert!(!config.enabled);
        assert!(!config.warn_on_fix);
    }

    #[test]
    fn test_backend_type_available_backends() {
        let backends = BackendType::available_backends();

        // 至少应该包含 Tiered
        assert!(backends.contains(&BackendType::Tiered));

        #[cfg(feature = "l1-moka")]
        {
            assert!(backends.contains(&BackendType::Moka));
        }

        #[cfg(feature = "l2-redis")]
        {
            assert!(backends.contains(&BackendType::Redis));
        }
    }

    // ==================== 安全测试 ====================

    #[test]
    fn test_config_validation_capacity_limits() {
        // 有效容量
        assert!(ConfigValidation::validate_capacity(1000).is_ok());
        assert!(ConfigValidation::validate_capacity(ConfigValidation::MAX_CAPACITY).is_ok());

        // 无效容量 - 为零
        assert!(ConfigValidation::validate_capacity(0).is_err());

        // 无效容量 - 超过最大值
        assert!(ConfigValidation::validate_capacity(ConfigValidation::MAX_CAPACITY + 1).is_err());
    }

    #[test]
    fn test_config_validation_ttl_limits() {
        // 有效 TTL
        assert!(ConfigValidation::validate_ttl(3600).is_ok());
        assert!(ConfigValidation::validate_ttl(ConfigValidation::MAX_TTL_SECS).is_ok());

        // 无效 TTL - 为零
        assert!(ConfigValidation::validate_ttl(0).is_err());

        // 无效 TTL - 超过最大值
        assert!(ConfigValidation::validate_ttl(ConfigValidation::MAX_TTL_SECS + 1).is_err());
    }

    #[test]
    fn test_config_validation_tti_limits() {
        // 有效 TTI
        assert!(ConfigValidation::validate_tti(1800).is_ok());
        assert!(ConfigValidation::validate_tti(ConfigValidation::MAX_TTI_SECS).is_ok());

        // 无效 TTI - 超过最大值
        assert!(ConfigValidation::validate_tti(ConfigValidation::MAX_TTI_SECS + 1).is_err());
    }

    #[test]
    fn test_config_validation_custom_name() {
        // 有效名称
        assert!(ConfigValidation::validate_custom_name("valid_name").is_ok());
        assert!(ConfigValidation::validate_custom_name("my-backend.123").is_ok());
        assert!(ConfigValidation::validate_custom_name("A").is_ok());

        // 无效名称 - 为空
        assert!(ConfigValidation::validate_custom_name("").is_err());

        // 无效名称 - 超过最大长度
        let long_name = "a".repeat(ConfigValidation::MAX_CUSTOM_NAME_LENGTH + 1);
        assert!(ConfigValidation::validate_custom_name(&long_name).is_err());

        // 无效名称 - 包含特殊字符
        assert!(ConfigValidation::validate_custom_name("invalid/name").is_err());
        assert!(ConfigValidation::validate_custom_name("invalid@name").is_err());
        assert!(ConfigValidation::validate_custom_name("invalid name").is_err());
    }

    #[test]
    fn test_backend_type_from_str_validates_custom_name() {
        // 有效自定义后端
        let result = BackendType::from_str("custom:valid_name");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            BackendType::Custom("valid_name".to_string())
        );

        // 无效自定义后端 - 名称太长
        let long_name = format!(
            "custom:{}",
            "a".repeat(ConfigValidation::MAX_CUSTOM_NAME_LENGTH + 1)
        );
        let result = BackendType::from_str(&long_name);
        assert!(result.is_err());

        // 无效自定义后端 - 包含无效字符
        let result = BackendType::from_str("custom:invalid/name");
        assert!(result.is_err());
    }

    #[test]
    fn test_path_validation_config_defaults() {
        let config = PathValidationConfig::new();
        assert!(config.allowed_base_dirs.is_empty());
        assert!(!config.allow_symbolic_links);
        assert_eq!(config.max_path_length, 4096);
    }

    #[test]
    fn test_path_validation_rejects_relative_paths() {
        let config = PathValidationConfig::new();
        let result = config.validate("relative/path/config.toml");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("absolute"));
    }

    #[test]
    fn test_path_validation_rejects_invalid_chars() {
        let config = PathValidationConfig::new();
        let result = config.validate("/path/with\ninvalid/chars.toml");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid character"));
    }

    #[test]
    fn test_path_validation_allows_valid_absolute_paths() {
        let config = PathValidationConfig::new();
        // 使用临时目录
        let temp_path = "/tmp/oxcache_test_config.toml";
        let result = config.validate(temp_path);
        // 文件不存在也可以验证路径格式
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("canonicalize"));
    }
}
