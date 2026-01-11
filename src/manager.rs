//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存管理器，负责初始化和管理所有缓存客户端。

use crate::config::legacy_config::EvictionPolicy;
use crate::config::{
    CacheStrategy, CacheType, DynamicConfig, GlobalConfig, OxcacheConfig, SerializationType,
};
use crate::error::{CacheError, Result};
#[cfg(feature = "l1-moka")]
use crate::serialization::{json::JsonSerializer, SerializerEnum};
use crate::CacheOps;

use dashmap::DashMap;
use lazy_static::lazy_static;
use std::sync::Arc;
use tracing::{event, info, instrument, warn, Level};

/// 初始化缓存系统
///
/// 这是一个便捷函数，调用 `CacheManager::init`。
#[instrument(skip(config), level = "info", fields(service_count = config.services.len()))]
pub async fn init(config: OxcacheConfig) -> Result<()> {
    CacheManager::init(config).await
}

// ============================================================================
// Feature-Gated Imports
// ============================================================================

#[cfg(feature = "l1-moka")]
mod l1_backend {
    use crate::backend::l1::L1Backend;
    use crate::config::L1Config;
    use crate::Result;
    use std::sync::Arc;

    /// 创建 L1 后端（需要 l1-moka feature）
    pub async fn create_l1_backend(l1_cfg: &L1Config) -> Result<Arc<L1Backend>> {
        Ok(Arc::new(L1Backend::new(l1_cfg.max_capacity)))
    }

    /// 检查 L1 功能是否可用
    pub fn is_l1_available() -> bool {
        true
    }
}

#[cfg(not(feature = "l1-moka"))]
mod l1_backend {
    use crate::config::L1Config;
    use crate::Result;
    use std::sync::Arc;

    /// 禁用状态下的 L1 后端桩实现
    #[derive(Clone)]
    pub struct DisabledL1Backend;

    impl DisabledL1Backend {
        pub fn new(_capacity: u64) -> Self {
            Self
        }
    }

    /// 创建 L1 后端（当 l1-moka feature 未启用时返回错误）
    pub async fn create_l1_backend(l1_cfg: &L1Config) -> Result<Arc<DisabledL1Backend>> {
        Err(CacheError::ConfigError(format!(
            "L1 cache (Moka) is not available. Please enable the 'l1-moka' feature in your Cargo.toml: \
             oxcache = {{ version = \"0.1\", features = [\"l1-moka\"] }}",
        )))
    }

    /// 检查 L1 功能是否可用
    pub fn is_l1_available() -> bool {
        false
    }
}

#[cfg(feature = "l2-redis")]
mod l2_backend {
    use crate::backend::l2::L2Backend;
    use crate::config::L2Config;
    use crate::Result;
    use std::sync::Arc;

    /// 创建 L2 后端（需要 l2-redis feature）
    pub async fn create_l2_backend(l2_cfg: &L2Config) -> Result<Arc<L2Backend>> {
        Ok(Arc::new(L2Backend::new(l2_cfg).await?))
    }

    /// 检查 L2 功能是否可用
    pub fn is_l2_available() -> bool {
        true
    }
}

#[cfg(not(feature = "l2-redis"))]
mod l2_backend {
    use crate::error::CacheError;
    use crate::Result;
    use std::sync::Arc;

    /// 禁用状态下的 L2 后端桩实现
    #[derive(Clone)]
    pub struct DisabledD2Backend;

    impl DisabledD2Backend {
        pub fn new() -> Self {
            Self
        }
    }

    /// 创建 L2 后端（当 l2-redis feature 未启用时返回错误）
    pub async fn create_l2_backend(_cfg: &dyn std::any::Any) -> Result<Arc<DisabledD2Backend>> {
        Err(CacheError::ConfigError(format!(
            "L2 cache (Redis) is not available. Please enable the 'l2-redis' feature in your Cargo.toml: \
             oxcache = {{ version = \"0.1\", features = [\"l2-redis\"] }}",
        )))
    }

    /// 检查 L2 功能是否可用
    pub fn is_l2_available() -> bool {
        false
    }
}

// ============================================================================
// Feature Information Functions
// ============================================================================

/// 获取 L1 缓存功能状态信息
pub fn get_l1_feature_info() -> &'static str {
    #[cfg(feature = "l1-moka")]
    {
        "L1 Cache (Moka): Enabled"
    }
    #[cfg(not(feature = "l1-moka"))]
    {
        "L1 Cache (Moka): Disabled (enable with 'l1-moka' feature)"
    }
}

/// 获取 L2 缓存功能状态信息
pub fn get_l2_feature_info() -> &'static str {
    #[cfg(feature = "l2-redis")]
    {
        "L2 Cache (Redis): Enabled"
    }
    #[cfg(not(feature = "l2-redis"))]
    {
        "L2 Cache (Redis): Disabled (enable with 'l2-redis' feature)"
    }
}

/// 获取所有功能状态信息
pub fn get_all_feature_info() -> Vec<&'static str> {
    vec![get_l1_feature_info(), get_l2_feature_info()]
}

/// 检查 L1 功能是否启用
pub fn is_l1_enabled() -> bool {
    l1_backend::is_l1_available()
}

/// 检查 L2 功能是否启用
pub fn is_l2_enabled() -> bool {
    l2_backend::is_l2_available()
}

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

        // 记录功能状态
        info!("Cache features: {}", get_l1_feature_info());
        info!("Cache features: {}", get_l2_feature_info());

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

            // 初始化客户端（带 feature 检查）
            let client = Self::init_client(name, service_cfg, &serializer).await?;
            manager.insert(name.clone(), client);
        }

        info!("CacheManager initialization completed");
        Ok(())
    }

    /// 初始化单个客户端（带 feature 检查和优雅降级）
    async fn init_client(
        name: &str,
        service_cfg: &crate::config::ServiceConfig,
        serializer: &SerializerEnum,
    ) -> Result<Arc<dyn CacheOps>> {
        match service_cfg.cache_type {
            #[cfg(all(feature = "l1-moka", feature = "l2-redis"))]
            CacheType::TwoLevel => Self::init_two_level_client(name, service_cfg, serializer).await,
            #[cfg(not(all(feature = "l1-moka", feature = "l2-redis")))]
            CacheType::TwoLevel => Err(CacheError::ConfigError(
                "Two-level cache requires both l1-moka and l2-redis features".to_string(),
            )),
            CacheType::L1 => Self::init_l1_only_client(name, service_cfg, serializer).await,
            #[cfg(feature = "l2-redis")]
            CacheType::L2 => Self::init_l2_only_client(name, service_cfg, serializer).await,
            #[cfg(not(feature = "l2-redis"))]
            CacheType::L2 => Err(CacheError::ConfigError(
                "L2-only cache requires l2-redis feature".to_string(),
            )),
        }
    }

    /// 初始化双层缓存客户端（带优雅降级）
    #[cfg(all(feature = "l1-moka", feature = "l2-redis"))]
    async fn init_two_level_client(
        name: &str,
        service_cfg: &crate::config::ServiceConfig,
        serializer: &SerializerEnum,
    ) -> Result<Arc<dyn CacheOps>> {
        #[cfg(feature = "l1-moka")]
        use crate::client::two_level::TwoLevelClient;

        let l1_cfg = service_cfg.l1.as_ref().ok_or_else(|| {
            CacheError::ConfigError(format!("Service '{}' is missing L1 configuration", name))
        })?;

        let l2_cfg = service_cfg.l2.as_ref().ok_or_else(|| {
            CacheError::ConfigError(format!("Service '{}' is missing L2 configuration", name))
        })?;

        let two_level_cfg = service_cfg.two_level.as_ref().ok_or_else(|| {
            CacheError::ConfigError(format!(
                "Service '{}' is missing TwoLevel configuration",
                name
            ))
        })?;

        // 检查 feature 可用性并创建后端
        let l1_available = l1_backend::is_l1_available();
        let l2_available = l2_backend::is_l2_available();

        // 根据功能可用性选择降级策略
        match (l1_available, l2_available) {
            (true, true) => {
                // 完整功能：创建完整的 TwoLevelClient
                let l1 = l1_backend::create_l1_backend(l1_cfg).await?;
                let l2 = l2_backend::create_l2_backend(l2_cfg).await?;

                Ok(Arc::new(
                    TwoLevelClient::new(
                        name.to_string(),
                        two_level_cfg.clone(),
                        l1,
                        l2,
                        serializer.clone(),
                    )
                    .await?,
                ))
            }
            (true, false) => {
                // L1 可用，L2 不可用：降级为 L1 only
                warn!(
                    "Service '{}': L2 (Redis) not available, degrading to L1-only mode",
                    name
                );
                let l1 = l1_backend::create_l1_backend(l1_cfg).await?;
                let l1_client =
                    crate::client::l1::L1Client::new(name.to_string(), l1, serializer.clone());
                Ok(Arc::new(l1_client))
            }
            #[cfg(feature = "l2-redis")]
            (false, true) => {
                // L1 不可用，L2 可用：降级为 L2 only
                warn!(
                    "Service '{}': L1 (Moka) not available, degrading to L2-only mode",
                    name
                );
                let l2 = l2_backend::create_l2_backend(l2_cfg).await?;
                let l2_client =
                    crate::client::l2::L2Client::new(name.to_string(), l2, serializer.clone())
                        .await?;
                Ok(Arc::new(l2_client))
            }
            #[cfg(not(feature = "l2-redis"))]
            (false, _) => {
                // L1 不可用，且 L2 不可用（因为 l2-redis 未启用）
                Err(CacheError::ConfigError(format!(
                    "Service '{}': L1 (Moka) is not available. \
                     Please enable 'l1-moka' feature in Cargo.toml",
                    name
                )))
            }
            #[cfg(feature = "l2-redis")]
            (false, false) => {
                // 两层都不可用：返回错误
                Err(CacheError::ConfigError(format!(
                    "Service '{}': Both L1 and L2 cache backends are unavailable. \
                     Please enable 'l1-moka' and/or 'l2-redis' features in Cargo.toml",
                    name
                )))
            }
        }
    }

    /// 初始化 L1 only 客户端
    async fn init_l1_only_client(
        name: &str,
        service_cfg: &crate::config::ServiceConfig,
        serializer: &SerializerEnum,
    ) -> Result<Arc<dyn CacheOps>> {
        use crate::client::l1::L1Client;

        let l1_cfg = service_cfg.l1.as_ref().ok_or_else(|| {
            CacheError::ConfigError(format!("Service '{}' is missing L1 configuration", name))
        })?;

        if !l1_backend::is_l1_available() {
            return Err(CacheError::ConfigError(format!(
                "Service '{}': L1 cache (Moka) is not available. \
                 Please enable the 'l1-moka' feature in Cargo.toml",
                name
            )));
        }

        let l1 = l1_backend::create_l1_backend(l1_cfg).await?;
        Ok(Arc::new(L1Client::new(
            name.to_string(),
            l1,
            serializer.clone(),
        )))
    }

    /// 初始化 L2 only 客户端
    #[cfg(feature = "l2-redis")]
    async fn init_l2_only_client(
        name: &str,
        service_cfg: &crate::config::ServiceConfig,
        serializer: &SerializerEnum,
    ) -> Result<Arc<dyn CacheOps>> {
        use crate::client::l2::L2Client;

        let l2_cfg = service_cfg.l2.as_ref().ok_or_else(|| {
            CacheError::ConfigError(format!("Service '{}' is missing L2 configuration", name))
        })?;

        if !l2_backend::is_l2_available() {
            return Err(CacheError::ConfigError(format!(
                "Service '{}': L2 cache (Redis) is not available. \
                 Please enable the 'l2-redis' feature in Cargo.toml",
                name
            )));
        }

        let l2 = l2_backend::create_l2_backend(l2_cfg).await?;
        Ok(Arc::new(
            L2Client::new(name.to_string(), l2, serializer.clone()).await?,
        ))
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
#[cfg(all(feature = "l1-moka", feature = "l2-redis"))]
pub fn get_typed_client(service: &str) -> Result<Arc<crate::client::two_level::TwoLevelClient>> {
    let client = get_client(service)?;

    // 使用 into_any_arc 进行安全的向下转型
    match client
        .into_any_arc()
        .downcast::<crate::client::two_level::TwoLevelClient>()
    {
        Ok(typed) => Ok(typed),
        Err(_) => Err(CacheError::NotSupported(format!(
            "服务 {} 不是 TwoLevelClient",
            service
        ))),
    }
}

/// 获取指定服务的强类型缓存客户端（未启用功能时的桩实现）
#[cfg(not(all(feature = "l1-moka", feature = "l2-redis")))]
pub fn get_typed_client(_service: &str) -> Result<Arc<dyn crate::CacheOps>> {
    Err(CacheError::NotSupported(
        "TwoLevelClient is not available. Please enable both 'l1-moka' and 'l2-redis' features."
            .to_string(),
    ))
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
