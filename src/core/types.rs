//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 类型安全的枚举定义，用于替代硬编码字符串常量

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Redis 连接模式
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
        match s.to_lowercase().as_str() {
            "standalone" => Ok(Self::Standalone),
            "sentinel" => Ok(Self::Sentinel),
            "cluster" => Ok(Self::Cluster),
            _ => Err(format!(
                "Invalid RedisModeType: '{}'. Expected: standalone, sentinel, or cluster",
                s
            )),
        }
    }
}

/// 缓存后端类型
///
/// 每个后端类型都有其推荐的层级限制：
/// - `Moka` - L1（高性能内存缓存）
/// - `Dashmap` - L1（纯并发HashMap）
/// - `Redis` - L2/L3（分布式缓存）
/// - `Sqlite` - L2/L3（持久化存储）
/// - `Tiered` - 任意层级（用于组合）
/// - `Custom` - 任意层级（自定义后端）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    /// Moka 高性能内存缓存（推荐 L1/L2）
    #[cfg(feature = "moka")]
    Moka,
    /// DashMap 纯并发HashMap（推荐 L1/L2，无驱逐策略）
    #[cfg(feature = "dashmap")]
    Dashmap,
    /// Redis 分布式缓存（推荐 L2/L3）
    #[cfg(feature = "redis")]
    Redis,
    /// Sqlite 持久化存储（推荐 L2/L3）
    #[cfg(feature = "sqlite")]
    Sqlite,
    /// 分层缓存组合（任意层级）
    #[default]
    Tiered,
    /// 自定义后端（任意层级，通过 BackendProvider 注入）
    Custom(String),
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "moka")]
            Self::Moka => write!(f, "moka"),
            #[cfg(feature = "dashmap")]
            Self::Dashmap => write!(f, "dashmap"),
            #[cfg(feature = "redis")]
            Self::Redis => write!(f, "redis"),
            #[cfg(feature = "sqlite")]
            Self::Sqlite => write!(f, "sqlite"),
            Self::Tiered => write!(f, "tiered"),
            Self::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// 缓存层级
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerializationType {
    /// JSON 格式
    #[default]
    Json,
    /// Bincode 格式
    #[cfg(feature = "bincode")]
    Bincode,
    /// CBOR 格式
    #[cfg(feature = "extra-serialization")]
    Cbor,
    /// MessagePack 格式
    #[cfg(feature = "extra-serialization")]
    Messagepack,
}

impl std::fmt::Display for SerializationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            #[cfg(feature = "bincode")]
            Self::Bincode => write!(f, "bincode"),
            #[cfg(feature = "extra-serialization")]
            Self::Cbor => write!(f, "cbor"),
            #[cfg(feature = "extra-serialization")]
            Self::Messagepack => write!(f, "messagepack"),
        }
    }
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
    fn test_backend_type_serialize() {
        let backend = BackendType::Moka;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, "\"moka\"");
    }

    #[test]
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
}
