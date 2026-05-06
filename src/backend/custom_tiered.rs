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

use crate::backend::config_validation::ConfigValidation;
use crate::backend::memory::moka::MokaMemoryBackend as MemoryBackend;
use crate::backend::memory::moka::MokaMemoryBackendBuilder as MemoryBackendBuilder;
use crate::backend::validation_result::{ConfigValidationResult, FixedConfigResult, Layer};
use crate::backend::CacheBackend;
use crate::core::types::BackendType;
use crate::error::{CacheError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;

/// 后端支持的层级限制
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerRestriction {
    /// 仅支持 L1（本地高速内存缓存）
    L1Only,
    /// 仅支持 L2/L3（分布式或持久化缓存）
    L2AndL3Only,
    /// 支持任意层级
    Any,
}

impl LayerRestriction {
    /// 检查后端类型是否支持指定层级
    pub fn supports(&self, layer: Layer) -> bool {
        match self {
            LayerRestriction::L1Only => layer == Layer::L1,
            LayerRestriction::L2AndL3Only => layer == Layer::L2 || layer == Layer::L3,
            LayerRestriction::Any => true,
        }
    }

    /// 获取友好的描述文本
    pub fn description(&self) -> &'static str {
        match self {
            LayerRestriction::L1Only => "仅支持 L1 层级",
            LayerRestriction::L2AndL3Only => "仅支持 L2/L3 层级",
            LayerRestriction::Any => "支持任意层级",
        }
    }
}

/// BackendType 的扩展方法，用于层级限制和配置验证
impl BackendType {
    /// 获取后端类型的层级限制
    pub fn layer_restriction(&self) -> LayerRestriction {
        match self {
            #[cfg(feature = "moka")]
            BackendType::Moka => LayerRestriction::L1Only,
            #[cfg(feature = "dashmap")]
            BackendType::Dashmap => LayerRestriction::L1Only,
            #[cfg(feature = "redis")]
            BackendType::Redis => LayerRestriction::L2AndL3Only,
            #[cfg(feature = "sqlite")]
            BackendType::Sqlite => LayerRestriction::L2AndL3Only,
            BackendType::Tiered => LayerRestriction::Any,
            BackendType::Custom(_) => LayerRestriction::Any,
        }
    }

    /// 获取后端类型的推荐层级
    pub fn recommended_layer(&self) -> Layer {
        match self {
            #[cfg(feature = "moka")]
            BackendType::Moka => Layer::L1,
            #[cfg(feature = "dashmap")]
            BackendType::Dashmap => Layer::L1,
            #[cfg(feature = "redis")]
            BackendType::Redis => Layer::L2,
            #[cfg(feature = "sqlite")]
            BackendType::Sqlite => Layer::L2,
            BackendType::Tiered => Layer::L1,
            BackendType::Custom(_) => Layer::L1,
        }
    }

    /// 检查后端类型是否支持指定层级
    pub fn supports_layer(&self, layer: Layer) -> bool {
        self.layer_restriction().supports(layer)
    }

    /// 获取可用的后端类型列表（基于启用的 feature）
    pub fn available_backends() -> Vec<BackendType> {
        vec![
            #[cfg(feature = "moka")]
            BackendType::Moka,
            #[cfg(feature = "dashmap")]
            BackendType::Dashmap,
            #[cfg(feature = "redis")]
            BackendType::Redis,
            #[cfg(feature = "sqlite")]
            BackendType::Sqlite,
            BackendType::Tiered,
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
            #[cfg(feature = "moka")]
            "moka" => Ok(BackendType::Moka),

            #[cfg(feature = "dashmap")]
            "dashmap" => Ok(BackendType::Dashmap),

            #[cfg(feature = "redis")]
            "redis" => Ok(BackendType::Redis),

            #[cfg(feature = "sqlite")]
            "sqlite" | "persist" => Ok(BackendType::Sqlite),

            "tiered" | "multi" | "two-level" | "three-level" => Ok(BackendType::Tiered),

            _ => {
                if let Some(custom_name) = s.strip_prefix("custom:") {
                    // 验证自定义名称
                    let validated_name = ConfigValidation::validate_custom_name(custom_name)?;
                    Ok(BackendType::Custom(validated_name))
                } else {
                    // 构建可用后端列表
                    let mut available = vec![
                        #[cfg(feature = "moka")]
                        "moka",
                        #[cfg(feature = "dashmap")]
                        "dashmap",
                        #[cfg(feature = "redis")]
                        "redis",
                        #[cfg(feature = "sqlite")]
                        "sqlite",
                    ];
                    available.extend(["tiered", "custom:<name>"]);

                    Err(CacheError::InvalidInput(format!(
                        "Unknown backend type: '{}'. Available backends: {}",
                        s,
                        available.join(", ")
                    )))
                }
            }
        }
    }
}

/// 单层后端配置
/// 层级后端配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg(any(feature = "serialization", feature = "full"))]
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

#[cfg(any(feature = "serialization", feature = "full"))]
fn default_enabled() -> bool {
    true
}

#[cfg(any(feature = "serialization", feature = "full"))]
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
            return Err(CacheError::InvalidInput(format!(
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
#[cfg(any(feature = "serialization", feature = "full"))]
pub trait BackendProvider: Send + Sync {
    /// 创建 L1 后端
    async fn create_l1(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>>;

    /// 创建 L2 后端
    async fn create_l2(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>>;

    /// 创建 L3 后端
    async fn create_l3(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>>;
}

/// 默认后端提供者
///
/// 使用 MemoryBackend 作为 L1，RedisBackend 作为 L2。
#[derive(Default)]
#[cfg(any(feature = "serialization", feature = "full"))]
pub struct DefaultBackendProvider;

#[async_trait]
#[cfg(any(feature = "serialization", feature = "full"))]
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
        #[cfg(feature = "redis")]
        {
            use crate::backend::memory::redis::RedisBackend;

            let connection_string = options
                .get("connection_string")
                .and_then(|v| v.as_str())
                .unwrap_or(crate::core::constants::DEFAULT_REDIS_URL);

            let backend = Arc::new(RedisBackend::new(connection_string).await?);
            Ok(backend)
        }
        #[cfg(not(feature = "redis"))]
        {
            // 如果没有 redis feature，降级到内存后端
            let mut builder = MemoryBackend::builder();
            builder = apply_memory_options(builder, options);
            let backend = builder.build();
            Ok(Arc::new(backend))
        }
    }

    #[allow(unused)]
    #[instrument(skip(self, options), level = "debug")]
    async fn create_l3(&self, options: &serde_json::Value) -> Result<Arc<dyn CacheBackend>> {
        #[cfg(feature = "redis")]
        {
            // L3 默认使用 Redis
            use crate::backend::memory::redis::RedisBackend;

            let connection_string = options
                .get("connection_string")
                .and_then(|v| v.as_str())
                .unwrap_or(crate::core::constants::DEFAULT_REDIS_URL);

            let backend = Arc::new(RedisBackend::new(connection_string).await?);
            Ok(backend)
        }
        #[cfg(not(feature = "redis"))]
        {
            // 如果没有 redis feature，L3 不可用
            Err(CacheError::InvalidInput(
                "L3 backend requires Redis feature to be enabled".to_string(),
            ))
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
#[cfg(any(feature = "serialization", feature = "full"))]
fn apply_memory_options(mut builder: MemoryBackendBuilder, options: &serde_json::Value) -> MemoryBackendBuilder {
    if let Some(options) = options.as_object() {
        if let Some(capacity) = options.get("capacity").and_then(|v| v.as_u64()) {
            // 验证容量值
            match ConfigValidation::validate_capacity(capacity) {
                Ok(validated) => {
                    builder = builder.capacity(validated);
                }
                Err(_) => {
                    // 无效容量值，忽略
                }
            }
        }
        if let Some(ttl) = options.get("ttl").and_then(|v| v.as_u64()) {
            // 验证 TTL 值
            match ConfigValidation::validate_ttl(ttl) {
                Ok(validated) => {
                    builder = builder.ttl(std::time::Duration::from_secs(validated));
                }
                Err(_) => {
                    // 无效 TTL 值，忽略
                }
            }
        }
        if let Some(tti) = options.get("time_to_idle").and_then(|v| v.as_u64()) {
            // 验证 TTI 值
            match ConfigValidation::validate_tti(tti) {
                Ok(validated) => {
                    builder = builder.time_to_idle(std::time::Duration::from_secs(validated));
                }
                Err(_) => {
                    // 无效 TTI 值，忽略
                }
            }
        }
    }
    builder
}

/// 分层后端工厂
///
/// 用户自定义分层缓存配置
///
/// 允许用户灵活配置 L1、L2 和 L3 的后端类型：
/// - L1 可以选择：Moka、DashMap（内存缓存，高速访问）
/// - L2 可以选择：Moka、DashMap、Redis、Sqlite（持久化或分布式）
/// - L3 可以选择：Redis、Sqlite（分布式或持久化存储）
/// - 支持自动验证和修复不合理的配置
///
/// # 示例（三层缓存）
///
/// ```toml
/// [cache.my_service]
/// # 三层缓存配置
/// l1_backend = "moka"
/// l1_capacity = 10000
///
/// l2_backend = "dashmap"
/// l2_capacity = 50000
///
/// l3_backend = "redis"
/// l3_connection_string = "redis://localhost:6379"
/// ```
///
/// # 示例（两层 Redis）
///
/// ```toml
/// [cache.my_service]
/// l1_backend = "moka"
/// l2_backend = "redis"
/// ```
///
/// 自定义分层缓存配置
///
/// 支持灵活的多层缓存配置，每层可以独立配置不同的后端类型。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomTieredConfig {
    /// L1 后端配置（本地高速内存缓存）
    pub l1: LayerBackendConfig,
    /// L2 后端配置（本地持久化或分布式缓存）
    pub l2: LayerBackendConfig,
    /// L3 后端配置（分布式存储或外部服务）
    #[serde(default)]
    pub l3: LayerBackendConfig,
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
    /// 创建新配置（默认 L1=Moka, L2=Redis, L3=禁用）
    #[cfg(all(feature = "moka", feature = "redis", feature = "serialization"))]
    pub fn new() -> Self {
        Self::moka_redis()
    }

    // ============ 便捷构造方法 ============

    /// 仅使用 Redis（单层）
    #[cfg(feature = "redis")]
    pub fn redis_only(connection_string: &str) -> Self {
        let mut options = serde_json::Value::Null;
        if !connection_string.is_empty() {
            options = serde_json::json!({
                "connection_string": connection_string
            });
        }
        Self {
            l1: LayerBackendConfig {
                backend_type: BackendType::Tiered,
                options: serde_json::Value::Null,
                enabled: false,
            },
            l2: LayerBackendConfig {
                backend_type: BackendType::Redis,
                options,
                enabled: true,
            },
            l3: LayerBackendConfig {
                backend_type: BackendType::Tiered,
                options: serde_json::Value::Null,
                enabled: false,
            },
            auto_fix: AutoFixConfig::new(),
        }
    }

    /// Moka + Redis（双层）
    #[cfg(all(feature = "moka", feature = "redis"))]
    pub fn moka_redis() -> Self {
        Self {
            l1: LayerBackendConfig::new(BackendType::Moka),
            l2: LayerBackendConfig::new(BackendType::Redis),
            l3: LayerBackendConfig {
                backend_type: BackendType::Tiered,
                options: serde_json::Value::Null,
                enabled: false,
            },
            auto_fix: AutoFixConfig::new(),
        }
    }

    /// DashMap + Redis（双层）
    #[cfg(all(feature = "dashmap", feature = "redis"))]
    pub fn dashmap_redis() -> Self {
        Self {
            l1: LayerBackendConfig::new(BackendType::Dashmap),
            l2: LayerBackendConfig::new(BackendType::Redis),
            l3: LayerBackendConfig {
                backend_type: BackendType::Tiered,
                options: serde_json::Value::Null,
                enabled: false,
            },
            auto_fix: AutoFixConfig::new(),
        }
    }

    /// Moka + DashMap + Redis（三层）
    #[cfg(all(feature = "moka", feature = "dashmap", feature = "redis"))]
    pub fn moka_dashmap_redis() -> Self {
        Self {
            l1: LayerBackendConfig::new(BackendType::Moka),
            l2: LayerBackendConfig::new(BackendType::Dashmap),
            l3: LayerBackendConfig::new(BackendType::Redis),
            auto_fix: AutoFixConfig::new(),
        }
    }

    /// Moka + Redis + Sqlite（三层持久化）
    #[cfg(all(feature = "moka", feature = "redis", feature = "sqlite"))]
    pub fn moka_redis_sqlite() -> Self {
        Self {
            l1: LayerBackendConfig::new(BackendType::Moka),
            l2: LayerBackendConfig::new(BackendType::Redis),
            l3: LayerBackendConfig::new(BackendType::Sqlite),
            auto_fix: AutoFixConfig::new(),
        }
    }

    /// DashMap + Sqlite（双层持久化）
    #[cfg(all(feature = "dashmap", feature = "sqlite"))]
    pub fn dashmap_sqlite() -> Self {
        Self {
            l1: LayerBackendConfig::new(BackendType::Dashmap),
            l2: LayerBackendConfig::new(BackendType::Sqlite),
            l3: LayerBackendConfig {
                backend_type: BackendType::Tiered,
                options: serde_json::Value::Null,
                enabled: false,
            },
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

        // 验证 L3 配置
        if self.l3.enabled {
            match self.l3.validate(Layer::L3) {
                Ok(_) => {
                    result.add_valid(Layer::L3, self.l3.backend_type.clone());
                }
                Err(e) => {
                    result.add_invalid(Layer::L3, self.l3.backend_type.clone(), e.to_string());
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
                    fixed.l1.backend_type = fix.to_backend.clone();
                }
                Layer::L2 => {
                    fixed.l2.backend_type = fix.to_backend.clone();
                }
                Layer::L3 => {
                    fixed.l3.backend_type = fix.to_backend.clone();
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
            (FixedConfigResult::from(validation), None)
        }
    }
}

/// Builder 模式的便捷构造器
#[derive(Default)]
pub struct CustomTieredConfigBuilder(CustomTieredConfig);

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
    #[cfg(any(feature = "serialization", feature = "full"))]
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
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn l2_options(mut self, options: serde_json::Value) -> Self {
        self.0.l2.options = options;
        self
    }

    /// 设置 L3 后端类型
    pub fn l3(mut self, backend_type: BackendType) -> Self {
        self.0.l3.backend_type = backend_type;
        self
    }

    /// 设置 L3 配置选项
    #[cfg(any(feature = "serialization", feature = "full"))]
    pub fn l3_options(mut self, options: serde_json::Value) -> Self {
        self.0.l3.options = options;
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

    /// 启用 L3
    pub fn enable_l3(mut self, enabled: bool) -> Self {
        self.0.l3.enabled = enabled;
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
