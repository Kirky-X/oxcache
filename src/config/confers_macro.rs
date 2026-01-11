//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! confers 宏模块
//!
//! 提供基于 confers 库的声明式配置功能。

#[cfg(feature = "confers")]
use super::ConfigSource;
#[cfg(feature = "confers")]
use crate::OxcacheConfig;
#[cfg(feature = "confers")]
use anyhow::{anyhow, Context as AnyhowContext};
#[cfg(feature = "confers")]
use serde::Deserialize;
#[cfg(feature = "confers")]
use std::collections::HashMap;
#[cfg(feature = "confers")]
use std::fs;
#[cfg(feature = "confers")]
use std::path::{Path, PathBuf};
#[cfg(feature = "confers")]
use toml;

/// 完整配置（confers 版本）
///
/// 从 TOML 文件加载的配置结构。
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct OxcacheConfigFile {
    /// 全局配置
    pub global: Option<GlobalConfigConfers>,
    /// 服务配置
    pub services: Option<Vec<ServiceConfigItem>>,
}

/// 全局配置（confers 版本）
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct GlobalConfigConfers {
    /// 默认 TTL（秒）
    pub default_ttl: Option<u64>,
    /// 健康检查间隔（秒）
    pub health_check_interval: Option<u64>,
    /// 序列化类型
    pub serialization: Option<String>,
    /// 是否启用指标收集
    pub enable_metrics: Option<bool>,
}

/// 服务配置项
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfigItem {
    /// 服务名称
    pub name: String,
    /// 缓存类型（l1, l2, two_level）
    pub cache_type: Option<String>,
    /// TTL（秒）
    pub ttl: Option<u64>,
    /// 序列化类型
    pub serialization: Option<String>,
    /// L1 配置
    pub l1: Option<L1ConfigConfers>,
    /// L2 配置
    pub l2: Option<L2ConfigConfers>,
    /// 双层缓存配置
    pub two_level: Option<TwoLevelConfigConfers>,
}

/// L1 配置
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct L1ConfigConfers {
    pub max_capacity: Option<u64>,
    pub max_key_length: Option<usize>,
    pub max_value_size: Option<usize>,
    pub cleanup_interval_secs: Option<u64>,
    pub eviction_policy: Option<String>,
}

/// L2 配置
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct L2ConfigConfers {
    pub mode: Option<String>,
    pub connection_string: Option<String>,
    pub connection_timeout_ms: Option<u64>,
    pub command_timeout_ms: Option<u64>,
    pub password: Option<String>,
    pub enable_tls: Option<bool>,
    pub default_ttl: Option<u64>,
}

/// 双层缓存配置
#[cfg(feature = "confers")]
#[derive(Debug, Clone, Deserialize)]
pub struct TwoLevelConfigConfers {
    pub promote_on_hit: Option<bool>,
    pub enable_batch_write: Option<bool>,
    pub batch_size: Option<usize>,
    pub batch_interval_ms: Option<u64>,
}

#[cfg(feature = "confers")]
impl Default for GlobalConfigConfers {
    fn default() -> Self {
        Self {
            default_ttl: Some(300),
            health_check_interval: Some(60),
            serialization: Some("json".to_string()),
            enable_metrics: Some(true),
        }
    }
}

#[cfg(feature = "confers")]
impl Default for L1ConfigConfers {
    fn default() -> Self {
        Self {
            max_capacity: Some(10000),
            max_key_length: Some(512),
            max_value_size: Some(10485760),
            cleanup_interval_secs: Some(60),
            eviction_policy: Some("lru".to_string()),
        }
    }
}

#[cfg(feature = "confers")]
impl Default for L2ConfigConfers {
    fn default() -> Self {
        Self {
            mode: Some("standalone".to_string()),
            connection_string: None,
            connection_timeout_ms: Some(5000),
            command_timeout_ms: Some(30000),
            password: None,
            enable_tls: Some(false),
            default_ttl: None,
        }
    }
}

#[cfg(feature = "confers")]
impl Default for TwoLevelConfigConfers {
    fn default() -> Self {
        Self {
            promote_on_hit: Some(true),
            enable_batch_write: Some(false),
            batch_size: Some(100),
            batch_interval_ms: Some(10),
        }
    }
}

#[cfg(feature = "confers")]
impl Default for ServiceConfigItem {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            cache_type: Some("two_level".to_string()),
            ttl: None,
            serialization: None,
            l1: Some(L1ConfigConfers::default()),
            l2: Some(L2ConfigConfers::default()),
            two_level: Some(TwoLevelConfigConfers::default()),
        }
    }
}

#[cfg(feature = "confers")]
impl OxcacheConfigFile {
    /// 从 TOML 文件加载配置
    ///
    /// # 安全性
    ///
    /// 此方法会规范化路径，防止目录遍历攻击。
    /// 只允许读取绝对路径或相对于当前工作目录的配置文件。
    pub fn from_toml(path: &str) -> anyhow::Result<Self> {
        // 规范化路径，防止目录遍历
        let config_path = Self::validate_and_normalize_path(path)?;

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", path))?;

        toml::from_str(&content).with_context(|| format!("Failed to parse config file: {}", path))
    }

    /// 验证并规范化路径
    ///
    /// 防止目录遍历攻击，确保配置文件在允许的范围内。
    fn validate_and_normalize_path(path: &str) -> anyhow::Result<PathBuf> {
        let path = Path::new(path);

        // 获取绝对路径
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .context("Failed to get current directory")?
                .join(path)
        };

        // 规范化路径（解析 .. 和 .）
        let canonical_path = absolute_path
            .canonicalize()
            .with_context(|| format!("Invalid config path: {}", path.display()))?;

        // 检查路径是否包含父目录引用（防止目录遍历）
        if let Some(parent) = canonical_path.parent() {
            if parent.exists() && !parent.starts_with(std::env::current_dir()?) {
                return Err(anyhow!(
                    "Config path '{}' is outside the allowed directory",
                    path.display()
                ));
            }
        }

        Ok(canonical_path)
    }

    /// 转换为内部 OxcacheConfig
    pub fn to_oxcache_config(self) -> super::OxcacheConfig {
        use super::{CacheType, GlobalConfig, ServiceConfig};

        let global = self.global.unwrap_or_default();
        let services = self.services.unwrap_or_default();

        // 转换全局配置
        let global = GlobalConfig {
            default_ttl: global.default_ttl.unwrap_or(300),
            health_check_interval: global.health_check_interval.unwrap_or(60),
            serialization: match global
                .serialization
                .unwrap_or_else(|| "json".to_string())
                .as_str()
            {
                "bincode" => super::SerializationType::Bincode,
                _ => super::SerializationType::Json,
            },
            enable_metrics: global.enable_metrics.unwrap_or(true),
        };

        // 转换服务配置
        let mut services_map = HashMap::new();
        for svc in services {
            let cache_type = match svc
                .cache_type
                .unwrap_or_else(|| "two_level".to_string())
                .as_str()
            {
                "l1" | "L1" => CacheType::L1,
                "l2" | "L2" => CacheType::L2,
                _ => CacheType::TwoLevel,
            };

            let l1_config = svc.l1.unwrap_or_default();
            let l2_config = svc.l2.unwrap_or_default();
            let two_level_config = svc.two_level.unwrap_or_default();

            let service = ServiceConfig {
                cache_type,
                ttl: svc.ttl,
                serialization: svc.serialization.map(|s| match s.as_str() {
                    "bincode" => super::SerializationType::Bincode,
                    _ => super::SerializationType::Json,
                }),
                l1: Some(super::L1Config {
                    max_capacity: l1_config.max_capacity.unwrap_or(10000),
                    max_key_length: l1_config.max_key_length.unwrap_or(512),
                    max_value_size: l1_config.max_value_size.unwrap_or(10485760),
                    cleanup_interval_secs: l1_config.cleanup_interval_secs.unwrap_or(60),
                }),
                l2: Some({
                    use secrecy::SecretString;
                    super::L2Config {
                        mode: match l2_config
                            .mode
                            .unwrap_or_else(|| "standalone".to_string())
                            .as_str()
                        {
                            "sentinel" => super::RedisMode::Sentinel,
                            "cluster" => super::RedisMode::Cluster,
                            _ => super::RedisMode::Standalone,
                        },
                        connection_string: SecretString::new(
                            l2_config.connection_string.unwrap_or_default().into(),
                        ),
                        connection_timeout_ms: l2_config.connection_timeout_ms.unwrap_or(5000),
                        command_timeout_ms: l2_config.command_timeout_ms.unwrap_or(30000),
                        password: l2_config
                            .password
                            .map(|p| SecretString::new(p.into_boxed_str())),
                        enable_tls: l2_config.enable_tls.unwrap_or(false),
                        sentinel: None,
                        cluster: None,
                        default_ttl: l2_config.default_ttl,
                        max_key_length: 512,
                        max_value_size: 10 * 1024 * 1024,
                    }
                }),
                two_level: Some(super::TwoLevelConfig {
                    promote_on_hit: two_level_config.promote_on_hit.unwrap_or(true),
                    enable_batch_write: two_level_config.enable_batch_write.unwrap_or(false),
                    batch_size: two_level_config.batch_size.unwrap_or(100),
                    batch_interval_ms: two_level_config.batch_interval_ms.unwrap_or(10),
                    max_key_length: None,
                    max_value_size: None,
                    bloom_filter: None,
                    invalidation_channel: None,
                    warmup: None,
                }),
            };

            services_map.insert(svc.name, service);
        }

        super::OxcacheConfig {
            global,
            services: services_map,
            layer: None,
            #[cfg(feature = "confers")]
            extensions: HashMap::new(),
            #[cfg(feature = "confers")]
            source: Some(super::ConfigSource::File("confers".to_string())),
        }
    }
}

/// 从 TOML 文件加载配置并初始化缓存
///
/// # 示例
///
/// ```rust,ignore
/// #[cfg(feature = "confers")]
/// async fn init_from_confers() -> anyhow::Result<()> {
///     let config = confers_load("config.toml")?;
///     oxcache::init(config).await?;
///     Ok(())
/// }
/// ```
#[cfg(feature = "confers")]
pub fn confers_load(path: &str) -> anyhow::Result<super::OxcacheConfig> {
    let config = OxcacheConfigFile::from_toml(path)?;
    Ok(config.to_oxcache_config())
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "confers")]
    mod confers_tests {
        use super::super::*;
        use std::fs;

        #[test]
        fn test_config_from_toml() {
            let toml_content = r#"
[global]
default_ttl = 600
health_check_interval = 30
serialization = "json"
enable_metrics = true

[[services]]
            name = "default"
cache_type = "two_level"
            ttl = 3600

[services.l1]
max_capacity = 5000

[services.l2]
mode = "standalone"
connection_string = "redis://localhost:6379"
connection_timeout_ms = 5000
"#;
            // 写入临时文件到当前目录
            let temp_file = "oxcache_test_config.toml";
            fs::write(temp_file, toml_content).unwrap();

            // 加载配置
            let config = confers_load(temp_file).unwrap();

            // 验证
            assert_eq!(config.global.default_ttl, 600);
            assert!(config.services.contains_key("default"));

            // 清理
            let _ = fs::remove_file(temp_file);
        }
        #[test]
        fn test_config_source_code() {
            let mut config = super::super::OxcacheConfig::new();
            config.set_source(super::super::ConfigSource::Code);
            assert_eq!(config.source, Some(super::super::ConfigSource::Code));
        }
    }
}
