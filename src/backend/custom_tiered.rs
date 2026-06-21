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
use crate::core::types::BackendType;
use crate::core::types::CacheLayer as Layer;
use crate::error::{CacheError, Result};

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
