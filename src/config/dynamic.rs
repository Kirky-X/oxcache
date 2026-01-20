//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 配置动态性模块
//!
//! 提供运行时配置热更新和策略切换能力。

use crate::config::{CacheStrategy, LegacyEvictionPolicy as EvictionPolicy, OxcacheConfig};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// 配置变更事件
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub service_name: String,
    pub old_strategy: Option<CacheStrategy>,
    pub new_strategy: CacheStrategy,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 配置动态管理器
#[derive(Debug, Clone)]
pub struct ConfigDynamicManager {
    strategies: Arc<DashMap<String, CacheStrategy>>,
    event_sender: broadcast::Sender<ConfigChangeEvent>,
}

impl Default for ConfigDynamicManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigDynamicManager {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            strategies: Arc::new(DashMap::new()),
            event_sender: sender,
        }
    }

    /// 更新服务策略
    pub fn update_strategy(&self, strategy: CacheStrategy) {
        let old_strategy = self.strategies.get(&strategy.service_name).map(|s| s.clone());
        let service_name = strategy.service_name.clone();
        self.strategies.insert(service_name.clone(), strategy.clone());

        // 发送变更事件
        let _ = self.event_sender.send(ConfigChangeEvent {
            service_name,
            old_strategy,
            new_strategy: strategy,
            timestamp: chrono::Utc::now(),
        });
    }

    /// 获取服务策略
    pub fn get_strategy(&self, service_name: &str) -> Option<CacheStrategy> {
        self.strategies.get(service_name).map(|s| s.clone())
    }

    /// 检查是否存在策略
    pub fn has_strategy(&self, service_name: &str) -> bool {
        self.strategies.contains_key(service_name)
    }

    /// 移除服务策略
    pub fn remove_strategy(&self, service_name: &str) -> bool {
        self.strategies.remove(service_name).is_some()
    }

    /// 切换淘汰策略
    pub fn switch_eviction_policy(&self, service_name: &str, policy: EvictionPolicy) {
        if let Some(mut entry) = self.strategies.get_mut(service_name) {
            entry.l1_eviction_policy = policy;
            entry.updated_at = chrono::Utc::now();
        }
    }

    /// 切换缓存模式（如 promote_on_hit）
    pub fn switch_cache_mode(&self, service_name: &str, promote_on_hit: bool) {
        if let Some(mut entry) = self.strategies.get_mut(service_name) {
            // 注意：这里需要根据实际配置结构调整
            entry.updated_at = chrono::Utc::now();
        }
    }

    /// 订阅配置变更
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.event_sender.subscribe()
    }

    /// 全部清除
    pub fn clear(&self) {
        self.strategies.clear();
    }
}

/// 全局配置动态管理器
pub static GLOBAL_CONFIG_MANAGER: once_cell::sync::Lazy<ConfigDynamicManager> =
    once_cell::sync::Lazy::new(ConfigDynamicManager::new);

/// 运行时配置热更新
#[cfg(feature = "config-dynamic")]
pub async fn hot_reload_config(config_path: &str) -> Result<(), String> {
    use crate::config::confers_macro::confers_load;

    let config = confers_load(config_path).map_err(|e| e.to_string())?;
    apply_runtime_config(&config)?;
    Ok(())
}

/// 应用运行时配置
#[cfg(feature = "config-dynamic")]
fn apply_runtime_config(config: &OxcacheConfig) -> Result<(), String> {
    for (name, service) in &config.services {
        let mut strategy = CacheStrategy::new(name);

        if let Some(ttl) = service.ttl {
            strategy = strategy.with_ttl(ttl);
        }

        GLOBAL_CONFIG_MANAGER.update_strategy(strategy);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dynamic_manager() {
        let manager = ConfigDynamicManager::new();

        // 初始为空
        assert!(!manager.has_strategy("test"));

        // 更新策略
        let strategy = CacheStrategy::new("test").with_ttl(600);
        manager.update_strategy(strategy.clone());

        // 检查存在
        assert!(manager.has_strategy("test"));
        assert_eq!(manager.get_strategy("test"), Some(strategy.clone()));

        // 移除策略
        assert!(manager.remove_strategy("test"));
        assert!(!manager.has_strategy("test"));
    }

    #[test]
    fn test_config_dynamic_manager_switch_policy() {
        let manager = ConfigDynamicManager::new();

        let strategy = CacheStrategy::new("test");
        manager.update_strategy(strategy);

        manager.switch_eviction_policy("test", EvictionPolicy::Lru);

        let updated = manager.get_strategy("test").unwrap();
        assert_eq!(updated.l1_eviction_policy, EvictionPolicy::Lru);
    }

    #[tokio::test]
    async fn test_config_change_event() {
        let manager = ConfigDynamicManager::new();
        let mut receiver = manager.subscribe();

        let strategy = CacheStrategy::new("test");
        manager.update_strategy(strategy.clone());

        // 接收事件
        let event = receiver.recv().await.unwrap();
        assert_eq!(event.service_name, "test");
        assert_eq!(event.new_strategy, strategy);
        assert!(event.old_strategy.is_none());
    }

    #[test]
    fn test_global_config_manager() {
        let manager = &GLOBAL_CONFIG_MANAGER;
        assert!(!manager.has_strategy("global_test"));

        let strategy = CacheStrategy::new("global_test");
        manager.update_strategy(strategy.clone());

        assert!(manager.has_strategy("global_test"));
        assert_eq!(manager.get_strategy("global_test"), Some(strategy));

        manager.remove_strategy("global_test");
    }
}
