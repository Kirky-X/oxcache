// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
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

use crate::backend::ConfigValidation;
use crate::core::BackendType;
use crate::core::CacheLayer as Layer;
use crate::error::{OxCacheError, OxCacheResult};

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
            #[cfg(feature = "memory")]
            BackendType::Moka => LayerRestriction::L1Only,
            #[cfg(feature = "memory")]
            BackendType::Dashmap => LayerRestriction::L1Only,
            #[cfg(feature = "redis")]
            BackendType::Redis => LayerRestriction::L2AndL3Only,
            BackendType::None => LayerRestriction::Any,
            BackendType::Custom(_) => LayerRestriction::Any,
        }
    }

    /// 获取后端类型的推荐层级
    pub fn recommended_layer(&self) -> Layer {
        match self {
            #[cfg(feature = "memory")]
            BackendType::Moka => Layer::L1,
            #[cfg(feature = "memory")]
            BackendType::Dashmap => Layer::L1,
            #[cfg(feature = "redis")]
            BackendType::Redis => Layer::L2,
            BackendType::None => Layer::L1,
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
            #[cfg(feature = "memory")]
            BackendType::Moka,
            #[cfg(feature = "memory")]
            BackendType::Dashmap,
            #[cfg(feature = "redis")]
            BackendType::Redis,
        ]
    }

    /// 从字符串解析后端类型
    ///
    /// # 安全说明
    /// - 验证自定义名称长度（最大 256 字符）
    /// - 验证自定义名称字符（只允许字母、数字、下划线、连字符、点）
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> OxCacheResult<Self> {
        // 避免 to_lowercase() 堆分配：已知后端名用 eq_ignore_ascii_case O(1) 比较
        #[cfg(feature = "memory")]
        if s.eq_ignore_ascii_case("moka") {
            return Ok(BackendType::Moka);
        }
        #[cfg(feature = "memory")]
        if s.eq_ignore_ascii_case("dashmap") {
            return Ok(BackendType::Dashmap);
        }
        #[cfg(feature = "redis")]
        if s.eq_ignore_ascii_case("redis") {
            return Ok(BackendType::Redis);
        }

        if let Some(custom_name) = s.strip_prefix("custom:") {
            // 验证自定义名称
            let validated_name = ConfigValidation::validate_custom_name(custom_name)?;
            Ok(BackendType::Custom(validated_name))
        } else {
            // 构建可用后端列表
            let mut available = vec![
                #[cfg(feature = "memory")]
                "moka",
                #[cfg(feature = "memory")]
                "dashmap",
                #[cfg(feature = "redis")]
                "redis",
            ];
            available.push("custom:<name>");

            Err(OxCacheError::InvalidInput(format!(
                "Unknown backend type: '{}'. Available backends: {}",
                s,
                available.join(", ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // LayerRestriction tests
    // ----------------------------------------------------------------

    #[test]
    fn test_layer_restriction_l1_only() {
        let lr = LayerRestriction::L1Only;
        assert!(lr.supports(Layer::L1));
        assert!(!lr.supports(Layer::L2));
        assert!(!lr.supports(Layer::L3));
    }

    #[test]
    fn test_layer_restriction_l2_and_l3_only() {
        let lr = LayerRestriction::L2AndL3Only;
        assert!(!lr.supports(Layer::L1));
        assert!(lr.supports(Layer::L2));
        assert!(lr.supports(Layer::L3));
    }

    #[test]
    fn test_layer_restriction_any() {
        let lr = LayerRestriction::Any;
        assert!(lr.supports(Layer::L1));
        assert!(lr.supports(Layer::L2));
        assert!(lr.supports(Layer::L3));
    }

    #[test]
    fn test_layer_restriction_description() {
        assert_eq!(LayerRestriction::L1Only.description(), "仅支持 L1 层级");
        assert_eq!(LayerRestriction::L2AndL3Only.description(), "仅支持 L2/L3 层级");
        assert_eq!(LayerRestriction::Any.description(), "支持任意层级");
    }

    // ----------------------------------------------------------------
    // BackendType extension method tests
    // ----------------------------------------------------------------

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_layer_restriction() {
        assert_eq!(BackendType::Moka.layer_restriction(), LayerRestriction::L1Only);
        assert_eq!(BackendType::Dashmap.layer_restriction(), LayerRestriction::L1Only);
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_backend_type_layer_restriction_redis() {
        assert_eq!(BackendType::Redis.layer_restriction(), LayerRestriction::L2AndL3Only);
    }

    #[test]
    fn test_backend_type_layer_restriction_none() {
        assert_eq!(BackendType::None.layer_restriction(), LayerRestriction::Any);
    }

    #[test]
    fn test_backend_type_layer_restriction_custom() {
        assert_eq!(
            BackendType::Custom("test".to_string()).layer_restriction(),
            LayerRestriction::Any
        );
    }

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_recommended_layer() {
        assert_eq!(BackendType::Moka.recommended_layer(), Layer::L1);
        assert_eq!(BackendType::Dashmap.recommended_layer(), Layer::L1);
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_backend_type_recommended_layer_redis() {
        assert_eq!(BackendType::Redis.recommended_layer(), Layer::L2);
    }

    #[test]
    fn test_backend_type_recommended_layer_none() {
        assert_eq!(BackendType::None.recommended_layer(), Layer::L1);
    }

    #[test]
    fn test_backend_type_recommended_layer_custom() {
        assert_eq!(BackendType::Custom("test".to_string()).recommended_layer(), Layer::L1);
    }

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_supports_layer() {
        assert!(BackendType::Moka.supports_layer(Layer::L1));
        assert!(!BackendType::Moka.supports_layer(Layer::L2));
    }

    #[test]
    fn test_backend_type_supports_layer_none() {
        assert!(BackendType::None.supports_layer(Layer::L1));
        assert!(BackendType::None.supports_layer(Layer::L2));
        assert!(BackendType::None.supports_layer(Layer::L3));
    }

    #[test]
    fn test_backend_type_available_backends() {
        let backends = BackendType::available_backends();
        // With "full" feature, should include Moka, Dashmap, Redis
        assert!(!backends.is_empty());
    }

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_from_str_moka() {
        let bt = BackendType::from_str("moka").unwrap();
        assert_eq!(bt, BackendType::Moka);
    }

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_from_str_case_insensitive() {
        let bt = BackendType::from_str("Moka").unwrap();
        assert_eq!(bt, BackendType::Moka);
        let bt = BackendType::from_str("MOKA").unwrap();
        assert_eq!(bt, BackendType::Moka);
    }

    #[test]
    fn test_backend_type_from_str_custom() {
        let bt = BackendType::from_str("custom:my_backend").unwrap();
        assert_eq!(bt, BackendType::Custom("my_backend".to_string()));
    }

    #[test]
    fn test_backend_type_from_str_custom_invalid_name() {
        let result = BackendType::from_str("custom:invalid name!");
        assert!(result.is_err());
    }

    #[test]
    fn test_backend_type_from_str_unknown() {
        let result = BackendType::from_str("unknown");
        assert!(result.is_err());
        match result.unwrap_err() {
            OxCacheError::InvalidInput(msg) => assert!(msg.contains("Unknown backend type")),
            _ => panic!("Expected InvalidInput error"),
        }
    }
}
