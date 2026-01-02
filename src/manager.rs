//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存管理器，负责初始化和管理所有缓存客户端。

use crate::backend::{l1::L1Backend, l2::L2Backend};
use crate::client::{l1::L1Client, l2::L2Client, two_level::TwoLevelClient, CacheOps};
use crate::config::{CacheType, Config, SerializationType};
use crate::error::{CacheError, Result};
use crate::serialization::{json::JsonSerializer, SerializerEnum};
use dashmap::DashMap;
use lazy_static::lazy_static;
use std::sync::Arc;
use tracing::{info, instrument, warn};

/// 缓存管理器
///
/// 负责初始化和管理所有缓存客户端
pub struct CacheManager {
    #[allow(dead_code)]
    clients: DashMap<String, Arc<dyn CacheOps>>,
    #[allow(dead_code)]
    config: Config,
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
    #[instrument(skip(config), level = "info", fields(service_count = config.services.len()))]
    pub async fn init(config: Config) -> Result<()> {
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
