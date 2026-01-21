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
//! ```
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    CustomTieredConfig                        │
//! │  ┌─────────────────┐  ┌─────────────────┐                   │
//! │  │  Layer::L1      │  │  Layer::L2      │                   │
//! │  │  - BackendType  │  │  - BackendType  │                   │
//! │  │  - 配置参数     │  │  - 配置参数     │                   │
//! │  └─────────────────┘  └─────────────────┘                   │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │               TieredBackendValidator                        │
//! │  - 验证后端类型与层级匹配                                    │
//! │  - 自动修复不合法配置                                        │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    TieredBackend                            │
//! │  - 实际创建和组合后端                                        │
//! │  - 统一 CacheBackend 接口                                   │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use crate::backend::memory::{MemoryBackend, MemoryBackendBuilder};
use crate::backend::{CacheBackend, TieredBackend};
use crate::error::{CacheError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// 缓存层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Layer {
    /// 第一层缓存 - 通常是内存缓存
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

impl Default for Layer {
    fn default() -> Self {
        Layer::L1
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
            BackendType::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

impl Default for BackendType {
    fn default() -> Self {
        #[cfg(feature = "l1-moka")]
        {
            BackendType::Moka
        }
        #[cfg(not(feature = "l1-moka"))]
        {
            BackendType::Memory
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
        let mut backends = Vec::new();

        #[cfg(feature = "l1-moka")]
        backends.push(BackendType::Moka);

        #[cfg(feature = "l2-redis")]
        backends.push(BackendType::Redis);

        backends.push(BackendType::Tiered);

        backends.push(BackendType::Persisted);

        backends
    }

    /// 从字符串解析后端类型
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
                if s.starts_with("custom:") {
                    Ok(BackendType::Custom(s[7..].to_string()))
                } else {
                    Err(CacheError::ConfigError(format!(
                        "Unknown backend type: {}. Available: moka, redis, tiered",
                        s
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
                                            fix.from_backend, fix.to_backend
                                        );
                                    }
                                    fixed.l1.backend_type = fix.to_backend.clone();
                                }
                                Layer::L2 => {
                                    if self.auto_fix.warn_on_fix {
                                        tracing::warn!(
                                            "L2 backend '{}' is not suitable for L2, auto-fixing to '{}'",
                                            fix.from_backend, fix.to_backend
                                        );
                                    }
                                    fixed.l2.backend_type = fix.to_backend.clone();                }
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

    /// 获取后端类型的工厂方法
    pub fn create_l1_backend(&self) -> Result<Arc<dyn CacheBackend>> {
        match &self.l1.backend_type {
            #[cfg(feature = "l1-moka")]
            BackendType::Moka => {
                let mut builder = MemoryBackend::builder();
                builder = self.apply_l1_options(builder);
                let backend = builder.build();
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "l1-moka"))]
            BackendType::Memory => {
                let mut builder = MemoryBackend::builder();
                builder = self.apply_l1_options(builder);
                let backend = builder.build();
                Ok(Arc::new(backend))
            }
            _ => Err(CacheError::ConfigError(format!(
                "Backend type '{}' is not supported for L1",
                self.l1.backend_type
            ))),
        }
    }

    fn apply_l1_options(&self, mut builder: MemoryBackendBuilder) -> MemoryBackendBuilder {
        if let Some(options) = self.l1.options.as_object() {
            if let Some(capacity) = options.get("capacity").and_then(|v| v.as_u64()) {
                builder = builder.capacity(capacity);
            }
            if let Some(ttl) = options.get("ttl").and_then(|v| v.as_u64()) {
                builder = builder.ttl(std::time::Duration::from_secs(ttl));
            }
            if let Some(tti) = options.get("time_to_idle").and_then(|v| v.as_u64()) {
                builder = builder.time_to_idle(std::time::Duration::from_secs(tti));
            }
        }
        builder
    }

    pub async fn create_l2_backend(&self) -> Result<Arc<dyn CacheBackend>> {
        use crate::backend::RedisBackend;

        match &self.l2.backend_type {
            #[cfg(feature = "l2-redis")]
            BackendType::Redis => {
                let connection_string = self
                    .l2
                    .options
                    .get("connection_string")
                    .and_then(|v| v.as_str())
                    .unwrap_or("redis://localhost:6379");
                let backend = RedisBackend::new(connection_string).await?;
                Ok(Arc::new(backend))
            }
            #[cfg(not(feature = "l2-redis"))]
            BackendType::Redis => {
                // 如果没有 l2-redis feature，降级到内存后端
                tracing::warn!("Redis backend not available, falling back to memory backend");
                let mut builder = MemoryBackend::builder();
                builder = self.apply_l2_options(builder);
                let backend = builder.build();
                Ok(Arc::new(backend))
            }
            _ => Err(CacheError::ConfigError(format!(
                "Backend type '{}' is not supported for L2",
                self.l2.backend_type
            ))),
        }
    }

    #[allow(dead_code)]
    fn apply_l2_options(&self, mut builder: MemoryBackendBuilder) -> MemoryBackendBuilder {
        if let Some(options) = self.l2.options.as_object() {
            if let Some(capacity) = options.get("capacity").and_then(|v| v.as_u64()) {
                builder = builder.capacity(capacity);
            }
        }
        builder
    }

    /// 创建分层后端
    pub async fn create_tiered_backend(&self) -> Result<TieredBackend> {
        let l1 = self.create_l1_backend()?;
        let l2 = self.create_l2_backend().await?;

        let backend = TieredBackend::from_arc(l1, l2);
        Ok(backend)
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
        self.invalid_layers.push((layer, backend_type, error.clone()));

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
                report.push_str(&format!(
                    "  - Layer {}: {} - {}\n",
                    layer, backend, error
                ));
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

        let l1_backend = val.valid_layers.iter().find(|(l, _)| *l == Layer::L1).map(|(_, b)| b.clone());
        let l2_backend = val.valid_layers.iter().find(|(l, _)| *l == Layer::L2).map(|(_, b)| b.clone());

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
#[cfg(feature = "confers")]
pub async fn load_from_file(path: &str) -> Result<CustomTieredConfig> {
    use std::fs;
    use toml;

    let content = fs::read_to_string(path)
        .map_err(|e| CacheError::ConfigError(e.to_string()))?;

    let config: CustomTieredConfig = toml::from_str(&content)
        .map_err(|e| CacheError::ConfigError(e.to_string()))?;

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
            tracing::info!("Auto-fixed tiered cache configuration: {:?}", result.warnings);
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
            assert_eq!(BackendType::Moka.layer_restriction(), LayerRestriction::L1Only);
            assert!(BackendType::Moka.supports_layer(Layer::L1));
            assert!(!BackendType::Moka.supports_layer(Layer::L2));
        }

        #[cfg(feature = "l2-redis")]
        {
            assert_eq!(BackendType::Redis.layer_restriction(), LayerRestriction::L2Only);
            assert!(!BackendType::Redis.supports_layer(Layer::L1));
            assert!(BackendType::Redis.supports_layer(Layer::L2));
        }

        assert_eq!(BackendType::Tiered.layer_restriction(), LayerRestriction::Any);
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
}
