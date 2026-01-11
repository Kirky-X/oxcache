//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存管理器，负责初始化和管理所有缓存客户端。

use crate::backend::{l1::L1Backend, l2::L2Backend};
use crate::client::{l1::L1Client, l2::L2Client, two_level::TwoLevelClient, CacheOps};
use crate::config::legacy_config::EvictionPolicy;
use crate::config::{
    CacheStrategy, CacheType, DynamicConfig, GlobalConfig, OxcacheConfig, SerializationType,
};
use crate::error::{CacheError, Result};
use crate::serialization::{json::JsonSerializer, SerializerEnum};
use dashmap::DashMap;
use lazy_static::lazy_static;
use std::sync::Arc;

/// 初始化缓存系统
///
/// 这是一个便捷函数，调用 `CacheManager::init`。
#[instrument(skip(config), level = "info", fields(service_count = config.services.len()))]
pub async fn init(config: OxcacheConfig) -> Result<()> {
    CacheManager::init(config).await
}
use tracing::{event, info, instrument, warn, Level};

/// 缓存管理器
///
/// 负责初始化和管理所有缓存客户端
pub struct CacheManager {
    #[allow(dead_code)]
    clients: DashMap<String, Arc<dyn CacheOps>>,
    #[allow(dead_code)]
    config: OxcacheConfig,
    /// 动态配置管理
    dynamic_config: DynamicConfig,
}

lazy_static! {
    pub static ref MANAGER: Arc<DashMap<String, Arc<dyn CacheOps>>> = Arc::new(DashMap::new());
}

impl CacheManager {
    /// 初始化缓存管理器
    ///
    /// 根据配置初始化所有服务的缓存客户端
    ///
    /// # 参数
    ///
    /// * `config` - 缓存系统配置
    ///
    /// # 返回值
    ///
    /// 返回初始化结果，成功时返回Ok(())，失败时返回相应的错误
    pub async fn init(config: OxcacheConfig) -> Result<()> {
        // 验证配置
        if let Err(e) = config.validate() {
            return Err(CacheError::ConfigError(e));
        }

        info!(
            "Initializing CacheManager with {} services",
            config.services.len()
        );
        let manager = MANAGER.clone();

        for (name, service_cfg) in &config.services {
            // 如果服务已经存在，我们跳过或者覆盖？
            // 目前 DashMap 会覆盖，这允许我们在测试中"重新初始化"特定服务
            // 只要我们不依赖 CacheManager 的内部状态（如监控线程），这应该没问题。
            // 但如果 TwoLevelClient 启动了后台任务（如 HealthChecker, BatchWriter），
            // 简单的覆盖不会停止旧的后台任务，可能会导致资源泄漏或竞争。
            //
            // 注意：优雅的 shutdown 机制已通过 shutdown_all() 函数实现。

            let serializer = match service_cfg
                .serialization
                .as_ref()
                .unwrap_or(&config.global.serialization)
            {
                SerializationType::Json => SerializerEnum::Json(JsonSerializer::new()),
                SerializationType::Bincode => {
                    return Err(CacheError::ConfigError(
                        "Bincode serialization is not currently supported.".to_string(),
                    ))
                }
            };

            let client: Arc<dyn CacheOps> =
                match service_cfg.cache_type {
                    CacheType::TwoLevel => {
                        let l1_cfg = service_cfg.l1.as_ref().ok_or_else(|| {
                            CacheError::ConfigError(format!("缺少{}的L1配置", name))
                        })?;
                        let l2_cfg = service_cfg.l2.as_ref().ok_or_else(|| {
                            CacheError::ConfigError(format!("缺少{}的L2配置", name))
                        })?;
                        let two_level_cfg = service_cfg.two_level.as_ref().ok_or_else(|| {
                            CacheError::ConfigError(format!("缺少{}的TwoLevel配置", name))
                        })?;

                        // 使用默认的 TinyLFU 策略
                        let l1 = Arc::new(L1Backend::new(l1_cfg.max_capacity));
                        let l2 = Arc::new(L2Backend::new(l2_cfg).await?);

                        Arc::new(
                            TwoLevelClient::new(
                                name.clone(),
                                two_level_cfg.clone(),
                                l1,
                                l2,
                                serializer,
                            )
                            .await?,
                        )
                    }
                    CacheType::L1 => {
                        let l1_cfg = service_cfg.l1.as_ref().ok_or_else(|| {
                            CacheError::ConfigError(format!("缺少{}的L1配置", name))
                        })?;
                        let l1 = Arc::new(L1Backend::new(l1_cfg.max_capacity));
                        Arc::new(L1Client::new(name.clone(), l1, serializer))
                    }
                    CacheType::L2 => {
                        let l2_cfg = service_cfg.l2.as_ref().ok_or_else(|| {
                            CacheError::ConfigError(format!("缺少{}的L2配置", name))
                        })?;
                        let l2 = Arc::new(L2Backend::new(l2_cfg).await?);
                        Arc::new(L2Client::new(name.clone(), l2, serializer).await?)
                    }
                };

            manager.insert(name.clone(), client);
        }
        Ok(())
    }

    /// 重置缓存管理器（仅用于测试）
    ///
    /// 清除所有已注册的客户端
    /// 重置缓存管理器（仅用于测试）
    ///
    /// 清除所有已注册的客户端。
    /// 注意：此方法仅用于测试目的，不应在生产环境中使用。
    #[doc(hidden)]
    pub fn reset() {
        MANAGER.clear();
    }
}

/// 获取指定服务的缓存客户端
///
/// # 参数
///
/// * `service` - 服务名称
///
/// # 返回值
///
/// 返回对应服务的缓存客户端，如果服务不存在则返回错误
pub fn get_client(service: &str) -> Result<Arc<dyn CacheOps>> {
    MANAGER
        .get(service)
        .map(|r| r.value().clone())
        .ok_or_else(|| CacheError::ConfigError(format!("未找到服务{}", service)))
}

/// 获取指定服务的强类型缓存客户端
///
/// 注意：这将尝试将客户端向下转型为 TwoLevelClient
///
/// # 参数
///
/// * `service` - 服务名称
///
/// # 返回值
///
/// 返回对应服务的缓存客户端，如果服务不存在则返回错误
pub fn get_typed_client(service: &str) -> Result<Arc<TwoLevelClient>> {
    let client = get_client(service)?;

    // 使用 into_any_arc 进行安全的向下转型
    match client.into_any_arc().downcast::<TwoLevelClient>() {
        Ok(typed) => Ok(typed),
        Err(_) => Err(CacheError::NotSupported(format!(
            "服务 {} 不是 TwoLevelClient",
            service
        ))),
    }
}

/// 优雅关闭所有缓存客户端
///
/// 遍历所有已注册的缓存客户端，调用它们的shutdown方法以释放资源
/// 主要用于应用程序关闭时的清理工作
#[instrument(level = "info")]
pub async fn shutdown_all() -> Result<()> {
    info!("开始关闭所有缓存客户端...");

    let mut errors = Vec::new();

    // 遍历所有客户端并关闭它们
    for entry in MANAGER.iter() {
        let service_name = entry.key();
        let client = entry.value();

        info!("正在关闭服务: {}", service_name);

        match client.shutdown().await {
            Ok(_) => {
                info!("服务 {} 已成功关闭", service_name);
            }
            Err(e) => {
                warn!("关闭服务 {} 时出错: {}", service_name, e);
                errors.push(format!("{}: {}", service_name, e));
            }
        }
    }

    // 清空管理器
    MANAGER.clear();

    if errors.is_empty() {
        info!("所有缓存客户端已成功关闭");
        Ok(())
    } else {
        Err(CacheError::ShutdownError(format!(
            "部分客户端关闭失败: {}",
            errors.join(", ")
        )))
    }
}

// ============================================================================
// 动态策略管理 API
// ============================================================================

/// 获取动态配置管理器实例
pub fn get_dynamic_config() -> &'static DynamicConfig {
    lazy_static! {
        static ref DYNAMIC_CONFIG: DynamicConfig = DynamicConfig::new();
    }
    &DYNAMIC_CONFIG
}

/// 更新服务的缓存策略
///
/// 此方法允许在运行时动态调整缓存策略，包括 TTL、容量、淘汰策略等。
/// 策略变更会触发事件通知。
///
/// # 参数
///
/// * `service_name` - 服务名称
/// * `ttl` - 新的 TTL（秒），0 表示不修改
/// * `l1_max_capacity` - 新的 L1 最大容量，0 表示不修改
/// * `eviction_policy` - 新的淘汰策略，None 表示不修改
///
/// # 返回值
///
/// 成功返回 Ok(())，服务不存在返回错误
pub fn update_strategy(
    service_name: &str,
    ttl: Option<u64>,
    l1_max_capacity: Option<u64>,
    eviction_policy: Option<EvictionPolicy>,
) -> Result<()> {
    let client = get_client(service_name)?;

    // 获取或创建当前策略
    let dynamic_config = get_dynamic_config();
    let mut strategy = dynamic_config
        .get_strategy(service_name)
        .unwrap_or_else(|| CacheStrategy::new(service_name));

    // 更新策略
    if let Some(new_ttl) = ttl {
        if new_ttl > 0 {
            strategy = strategy.with_ttl(new_ttl);
        }
    }

    if let Some(new_capacity) = l1_max_capacity {
        if new_capacity > 0 {
            strategy = strategy.with_l1_max_capacity(new_capacity);
        }
    }

    if let Some(new_policy) = eviction_policy {
        strategy = strategy.with_l1_eviction_policy(new_policy);
    }

    // 保存新策略
    dynamic_config.update_strategy(strategy.clone());

    // 发出策略变更事件
    event!(
        Level::INFO,
        service = service_name,
        ttl = strategy.ttl,
        l1_max_capacity = strategy.l1_max_capacity,
        eviction_policy = ?strategy.l1_eviction_policy,
        "Cache strategy updated"
    );

    Ok(())
}

/// 获取服务的当前缓存策略
///
/// # 参数
///
/// * `service_name` - 服务名称
///
/// # 返回值
///
/// 返回服务的当前策略配置，如果服务没有动态策略则返回 None
pub fn get_strategy(service_name: &str) -> Option<CacheStrategy> {
    let dynamic_config = get_dynamic_config();
    dynamic_config.get_strategy(service_name)
}

/// 更新 TTL
///
/// 便捷方法：仅更新服务的 TTL
pub fn update_ttl(service_name: &str, ttl: u64) -> Result<()> {
    update_strategy(service_name, Some(ttl), None, None)
}

/// 更新 L1 容量
///
/// 便捷方法：仅更新服务的 L1 最大容量
pub fn update_l1_capacity(service_name: &str, capacity: u64) -> Result<()> {
    update_strategy(service_name, None, Some(capacity), None)
}

/// 更新淘汰策略
///
/// 便捷方法：仅更新服务的 L1 淘汰策略
pub fn update_eviction_policy(service_name: &str, policy: EvictionPolicy) -> Result<()> {
    update_strategy(service_name, None, None, Some(policy))
}

/// 删除服务的动态策略配置
///
/// 删除后，服务将回退到使用静态配置
pub fn reset_strategy(service_name: &str) {
    let dynamic_config = get_dynamic_config();
    dynamic_config.remove_strategy(service_name);

    event!(
        Level::INFO,
        service = service_name,
        "Cache strategy reset to static config"
    );
}

/// 获取所有已配置动态策略的服务名称
pub fn list_strategies() -> Vec<String> {
    let dynamic_config = get_dynamic_config();
    dynamic_config.service_names()
}

/// 清空所有动态策略配置
pub fn clear_all_strategies() {
    let dynamic_config = get_dynamic_config();
    dynamic_config.clear();

    event!(Level::INFO, "All cache strategies cleared");
}
