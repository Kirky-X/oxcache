//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! # 配置动态性示例
//!
//! 演示如何使用运行时配置热更新和策略切换。
//!
//! 需要启用 `config-dynamic` 特性。

use oxcache::config::{CacheStrategy, LegacyEvictionPolicy as EvictionPolicy, ServiceConfig};
use oxcache::config_dynamic::{ConfigChangeEvent, ConfigDynamicManager, GLOBAL_CONFIG_MANAGER};

#[tokio::main]
async fn main() {
    println!("=== 配置动态性示例 ===\n");

    // ===========================================================================
    // 1. 基本使用
    // ===========================================================================
    println!("1. 基本使用示例");

    let manager = ConfigDynamicManager::new();
    println!("   - 创建配置动态管理器");

    // 检查初始状态
    let has_strategy = manager.has_strategy("test_service");
    println!("   - 初始是否有 test_service 策略: {}", has_strategy);
    println!();

    // ===========================================================================
    // 2. 更新服务策略
    // ===========================================================================
    println!("2. 更新服务策略");

    let strategy = CacheStrategy::new("user_service")
        .with_ttl(3600)
        .with_l1_eviction_policy(EvictionPolicy::Lru);

    manager.update_strategy(strategy.clone());
    println!("   - 更新 user_service 策略");
    println!("     - TTL: {} 秒", strategy.ttl());
    println!("     - 淘汰策略: {:?}", strategy.l1_eviction_policy);

    // 验证
    let retrieved = manager.get_strategy("user_service");
    println!("   - 获取策略: {:?}", retrieved.is_some());
    println!();

    // ===========================================================================
    // 3. 切换淘汰策略
    // ===========================================================================
    println!("3. 切换淘汰策略");

    // 初始策略是 Lru
    let initial = manager.get_strategy("user_service").unwrap();
    println!("   - 当前淘汰策略: {:?}", initial.l1_eviction_policy);

    // 切换到 LFU
    manager.switch_eviction_policy("user_service", EvictionPolicy::Lfu);
    let updated = manager.get_strategy("user_service").unwrap();
    println!("   - 更新后淘汰策略: {:?}", updated.l1_eviction_policy);
    println!();

    // ===========================================================================
    // 4. 配置变更订阅
    // ===========================================================================
    println!("4. 配置变更订阅");

    let mut receiver = manager.subscribe();

    // 触发配置变更
    let new_strategy = CacheStrategy::new("user_service")
        .with_ttl(7200)  // 增加到 2 小时
        .with_l1_eviction_policy(EvictionPolicy::TinyLfu);

    manager.update_strategy(new_strategy);

    // 接收变更事件
    tokio::spawn(async move {
        if let Ok(event) = receiver.recv().await {
            println!("   - 收到配置变更事件:");
            println!("     - 服务: {}", event.service_name);
            println!("     - 旧策略 TTL: {:?}", event.old_strategy.as_ref().map(|s| s.ttl()));
            println!("     - 新策略 TTL: {:?}", event.new_strategy.ttl());
            println!("     - 时间: {}", event.timestamp);
        }
    });

    // 等待一小段时间让异步任务执行
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    println!();

    // ===========================================================================
    // 5. 全局配置管理器
    // ===========================================================================
    println!("5. 全局配置管理器");

    let global_manager = &GLOBAL_CONFIG_MANAGER;

    // 设置全局服务策略
    let global_strategy = CacheStrategy::new("global_api")
        .with_ttl(1800)
        .with_l1_eviction_policy(EvictionPolicy::Lru);

    global_manager.update_strategy(global_strategy);
    println!("   - 设置全局 API 服务策略");

    // 验证
    let has = global_manager.has_strategy("global_api");
    println!("   - 全局 API 策略存在: {}", has);

    let retrieved = global_manager.get_strategy("global_api");
    if let Some(s) = retrieved {
        println!("   - 全局 API TTL: {} 秒", s.ttl());
    }
    println!();

    // ===========================================================================
    // 6. 策略移除
    // ===========================================================================
    println!("6. 策略移除");

    let has_before = manager.has_strategy("user_service");
    println!("   - 移除前 user_service 策略存在: {}", has_before);

    let removed = manager.remove_strategy("user_service");
    println!("   - 移除结果: {}", removed);

    let has_after = manager.has_strategy("user_service");
    println!("   - 移除后 user_service 策略存在: {}", has_after);
    println!();

    // ===========================================================================
    // 7. 完整清除
    // ===========================================================================
    println!("7. 完整清除");

    // 添加一些策略
    manager.update_strategy(CacheStrategy::new("service1"));
    manager.update_strategy(CacheStrategy::new("service2"));
    manager.update_strategy(CacheStrategy::new("service3"));
    println!("   - 添加 3 个服务策略");

    let count = manager.strategies.len();
    println!("   - 清除前策略数量: {}", count);

    manager.clear();
    println!("   - 清除后策略数量: {}", manager.strategies.len());
    println!();

    // ===========================================================================
    // 8. 运行时配置热更新流程
    // ===========================================================================
    println!("8. 运行时配置热更新流程");

    println!("   // 1. 监听配置文件变化");
    println!("   let mut file_watcher = watch_config_file(\"config.toml\").await;");
    println!();
    println!("   // 2. 配置文件变化时加载新配置");
    println!("   if let Ok(new_config) = load_config(\"config.toml\").await {");
    println!("       // 3. 应用新配置");
    println!("       apply_runtime_config(&new_config).await;");
    println!("   }");
    println!();
    println!("   // 4. 策略变更会自动通知订阅者");
    println!("   for event in subscriber.into_iter() {");
    println!("       println!(\"策略变更: {} - TTL: {}\", event.service_name, event.new_strategy.ttl());");
    println!("   }");
    println!();

    println!("=== 配置动态性示例完成 ===");
}
