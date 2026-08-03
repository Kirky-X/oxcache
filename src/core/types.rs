// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 类型安全的枚举定义，用于替代硬编码字符串常量

// serde derive/attrs 仅在 serialization/full feature 下可用（特性隔离）
#[cfg(any(feature = "serialization", feature = "full"))]
use serde::{Deserialize, Serialize};

/// Redis 连接模式
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(any(feature = "serialization", feature = "full"), derive(Serialize, Deserialize))]
#[cfg_attr(any(feature = "serialization", feature = "full"), serde(rename_all = "lowercase"))]
pub enum RedisModeType {
    /// 单机模式
    #[default]
    Standalone,
    /// 哨兵模式
    Sentinel,
    /// 集群模式
    Cluster,
}

impl std::fmt::Display for RedisModeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standalone => write!(f, "standalone"),
            Self::Sentinel => write!(f, "sentinel"),
            Self::Cluster => write!(f, "cluster"),
        }
    }
}

impl std::str::FromStr for RedisModeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 避免 to_lowercase() 堆分配：直接用 eq_ignore_ascii_case
        if s.eq_ignore_ascii_case("standalone") {
            return Ok(Self::Standalone);
        }
        if s.eq_ignore_ascii_case("sentinel") {
            return Ok(Self::Sentinel);
        }
        if s.eq_ignore_ascii_case("cluster") {
            return Ok(Self::Cluster);
        }
        Err(format!(
            "Invalid RedisModeType: '{}'. Expected: standalone, sentinel, or cluster",
            s
        ))
    }
}

/// 缓存后端类型
///
/// 每个后端类型都有其推荐的层级限制：
/// - `Moka` - L1（高性能内存缓存）
/// - `Dashmap` - L1（纯并发HashMap）
/// - `Redis` - L2/L3（分布式缓存）
/// - `Sqlite` - L2/L3（持久化存储）
/// - `None` - 无后端（需通过 ChainCache 或 Custom 显式配置多后端）
/// - `Custom` - 任意层级（自定义后端）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(any(feature = "serialization", feature = "full"), derive(Serialize, Deserialize))]
#[cfg_attr(any(feature = "serialization", feature = "full"), serde(rename_all = "snake_case"))]
pub enum BackendType {
    /// Moka 高性能内存缓存（推荐 L1/L2）
    #[cfg(feature = "memory")]
    Moka,
    /// DashMap 纯并发HashMap（推荐 L1/L2，无驱逐策略）
    #[cfg(feature = "memory")]
    Dashmap,
    /// Redis 分布式缓存（推荐 L2/L3）
    #[cfg(feature = "redis")]
    Redis,
    /// 无后端（需通过 ChainCache 或 Custom 显式配置多后端）
    #[cfg_attr(any(feature = "serialization", feature = "full"), serde(rename = "none"))]
    #[default]
    None,
    /// 自定义后端（任意层级，通过 BackendProvider 注入）
    Custom(String),
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "memory")]
            Self::Moka => write!(f, "moka"),
            #[cfg(feature = "memory")]
            Self::Dashmap => write!(f, "dashmap"),
            #[cfg(feature = "redis")]
            Self::Redis => write!(f, "redis"),
            Self::None => write!(f, "none"),
            Self::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// 缓存层级
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(any(feature = "serialization", feature = "full"), derive(Serialize, Deserialize))]
pub enum CacheLayer {
    /// L1 内存缓存
    #[default]
    L1,
    /// L2 分布式缓存
    L2,
    /// L3 持久化/外部存储
    L3,
}

impl std::fmt::Display for CacheLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::L1 => write!(f, "L1"),
            Self::L2 => write!(f, "L2"),
            Self::L3 => write!(f, "L3"),
        }
    }
}

/// 序列化格式类型
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(any(feature = "serialization", feature = "full"), derive(Serialize, Deserialize))]
#[cfg_attr(any(feature = "serialization", feature = "full"), serde(rename_all = "lowercase"))]
pub enum SerializationType {
    /// JSON 格式
    #[default]
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_mode_type_default() {
        assert_eq!(RedisModeType::default(), RedisModeType::Standalone);
    }

    #[test]
    fn test_redis_mode_type_serialize() {
        let mode = RedisModeType::Sentinel;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"sentinel\"");
    }

    #[test]
    fn test_redis_mode_type_deserialize() {
        let mode: RedisModeType = serde_json::from_str("\"cluster\"").unwrap();
        assert_eq!(mode, RedisModeType::Cluster);
    }

    #[test]
    fn test_redis_mode_type_display() {
        assert_eq!(format!("{}", RedisModeType::Standalone), "standalone");
        assert_eq!(format!("{}", RedisModeType::Sentinel), "sentinel");
        assert_eq!(format!("{}", RedisModeType::Cluster), "cluster");
    }

    #[test]
    fn test_redis_mode_type_from_str() {
        assert_eq!(
            "standalone".parse::<RedisModeType>().unwrap(),
            RedisModeType::Standalone
        );
        assert_eq!("SENTINEL".parse::<RedisModeType>().unwrap(), RedisModeType::Sentinel);
        assert_eq!("Cluster".parse::<RedisModeType>().unwrap(), RedisModeType::Cluster);
        assert!("invalid".parse::<RedisModeType>().is_err());
    }

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_serialize() {
        let backend = BackendType::Moka;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, "\"moka\"");
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_backend_type_deserialize() {
        let backend: BackendType = serde_json::from_str("\"redis\"").unwrap();
        assert_eq!(backend, BackendType::Redis);
    }

    #[test]
    fn test_cache_layer_serialize() {
        let layer = CacheLayer::L1;
        let json = serde_json::to_string(&layer).unwrap();
        assert_eq!(json, "\"L1\"");
    }

    #[test]
    fn test_cache_layer_deserialize() {
        let layer: CacheLayer = serde_json::from_str("\"L2\"").unwrap();
        assert_eq!(layer, CacheLayer::L2);
    }

    #[test]
    fn test_cache_layer_l3() {
        let layer = CacheLayer::L3;
        assert_eq!(format!("{}", layer), "L3");
    }

    #[test]
    fn test_serialization_type_default() {
        assert_eq!(SerializationType::default(), SerializationType::Json);
    }

    // ============================================================================
    // BackendType Display 测试 (lines 78-87)
    // ============================================================================

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_moka_display() {
        assert_eq!(format!("{}", BackendType::Moka), "moka");
    }

    #[test]
    #[cfg(feature = "memory")]
    fn test_backend_type_dashmap_display() {
        assert_eq!(format!("{}", BackendType::Dashmap), "dashmap");
    }

    #[test]
    #[cfg(feature = "redis")]
    fn test_backend_type_redis_display() {
        assert_eq!(format!("{}", BackendType::Redis), "redis");
    }

    #[test]
    fn test_backend_type_none_display() {
        assert_eq!(format!("{}", BackendType::None), "none");
    }

    #[test]
    fn test_backend_type_custom_display() {
        let backend = BackendType::Custom("my_backend".to_string());
        assert_eq!(format!("{}", backend), "custom:my_backend");
    }

    #[test]
    fn test_backend_type_custom_empty_display() {
        let backend = BackendType::Custom(String::new());
        assert_eq!(format!("{}", backend), "custom:");
    }

    #[test]
    fn test_backend_type_default_is_none() {
        assert_eq!(BackendType::default(), BackendType::None);
    }

    // ============================================================================
    // BackendType 序列化/反序列化测试
    // ============================================================================

    #[test]
    fn test_backend_type_none_serialize() {
        let backend = BackendType::None;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, "\"none\"");
    }

    #[test]
    fn test_backend_type_none_deserialize() {
        let backend: BackendType = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(backend, BackendType::None);
    }

    #[test]
    fn test_backend_type_custom_serialize() {
        let backend = BackendType::Custom("test".to_string());
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, "{\"custom\":\"test\"}");
    }

    #[test]
    fn test_backend_type_custom_deserialize() {
        let backend: BackendType = serde_json::from_str("{\"custom\":\"test\"}").unwrap();
        assert_eq!(backend, BackendType::Custom("test".to_string()));
    }

    // ============================================================================
    // BackendType Debug 和 PartialEq 测试
    // ============================================================================

    #[test]
    fn test_backend_type_debug() {
        let backend = BackendType::None;
        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("None"));
    }

    #[test]
    fn test_backend_type_equality() {
        assert_eq!(BackendType::None, BackendType::None);
        assert_ne!(
            BackendType::Custom("a".to_string()),
            BackendType::Custom("b".to_string())
        );
        assert_eq!(
            BackendType::Custom("a".to_string()),
            BackendType::Custom("a".to_string())
        );
    }

    // ============================================================================
    // CacheLayer Display 测试
    // ============================================================================

    #[test]
    fn test_cache_layer_display() {
        assert_eq!(format!("{}", CacheLayer::L1), "L1");
        assert_eq!(format!("{}", CacheLayer::L2), "L2");
        assert_eq!(format!("{}", CacheLayer::L3), "L3");
    }

    #[test]
    fn test_cache_layer_default() {
        assert_eq!(CacheLayer::default(), CacheLayer::L1);
    }

    // ============================================================================
    // SerializationType 测试
    // ============================================================================

    #[test]
    fn test_serialization_type_debug() {
        let debug_str = format!("{:?}", SerializationType::Json);
        assert!(debug_str.contains("Json"));
    }
}
