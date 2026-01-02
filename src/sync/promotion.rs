//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存提升机制，负责将L2缓存数据推广到L1缓存。

use crate::backend::{l1::L1Backend, l2::L2Backend};
use crate::error::Result;
use crate::recovery::health::HealthState;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};

/// 推广管理器
///
/// 负责将L2缓存中的数据推广到L1缓存
pub struct PromotionManager {
    /// 正在处理的推广任务
    in_flight: DashMap<String, Arc<Notify>>,
    /// L1缓存后端
    l1: Arc<L1Backend>,
    /// L2缓存后端
    l2: Arc<L2Backend>,
    /// 健康状态
    #[allow(dead_code)]
    health_state: Arc<RwLock<HealthState>>,
}

impl PromotionManager {
    /// 创建新的推广管理器
    ///
    /// # 参数
    ///
    /// * `l1` - L1缓存后端
    /// * `l2` - L2缓存后端
    /// * `health_state` - 健康状态
    ///
    /// # 返回值
    ///
    /// 返回新的推广管理器实例
    pub fn new(
        l1: Arc<L1Backend>,
        l2: Arc<L2Backend>,
        health_state: Arc<RwLock<HealthState>>,
    ) -> Self {
        Self {
            in_flight: DashMap::new(),
            l1,
            l2,
            health_state,
        }
    }

    /// 推广缓存项
    ///
    /// 将L2缓存中的数据推广到L1缓存
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    /// * `value` - 缓存值
    /// * `version` - 版本号
    ///
    /// # 返回值
    ///
    /// 返回操作结果
    pub async fn promote(&self, key: String, value: Vec<u8>, version: u64) -> Result<()> {
        let notify = self.in_flight.get(&key).map(|r| r.value().clone());
        if let Some(notify) = notify {
            notify.notified().await;
            return Ok(());
        }

        let notify = Arc::new(Notify::new());
        self.in_flight.insert(key.clone(), notify.clone());

        let result = async {
            let l2_ttl = self.l2.ttl(&key).await?;
            let l1_default_ttl = 300;

            let actual_ttl = match l2_ttl {
                Some(ttl) if ttl > 5 => ttl.min(l1_default_ttl),
                _ => return Ok(()),
            };

            self.l1
                .set_with_metadata(&key, value, actual_ttl, version)
                .await
        }
        .await;

        if let Some((_, n)) = self.in_flight.remove(&key) {
            n.notify_waiters();
        }

        result
    }
}
