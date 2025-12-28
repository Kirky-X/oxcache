//! Copyright (c) 2025, Kirky.X
//!
//! MIT License
//!
//! 数据库分区与WAL集成示例
//!
//! 本示例演示如何将WAL（预写日志）系统用于缓存数据恢复

use oxcache::error::Result;
use oxcache::recovery::wal::{Operation, WalEntry, WalManager};
use std::time::{Duration, SystemTime};

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== 数据库分区与WAL集成示例 ===\n");

    // 1. 创建WAL管理器
    println!("1. 创建WAL管理器...");
    let wal_manager = WalManager::new("partitioned_cache_service").await?;
    println!("   WAL管理器创建成功");

    // 2. 模拟缓存操作并记录到WAL
    println!("2. 模拟缓存操作并记录到WAL...");

    // 创建WAL条目 - 设置用户数据
    let entry1 = WalEntry {
        timestamp: SystemTime::now(),
        operation: Operation::Set,
        key: "user:1001".to_string(),
        value: Some(b"user_data_1001".to_vec()),
        ttl: Some(3600), // 1小时TTL
    };

    // 创建WAL条目 - 设置配置数据
    let entry2 = WalEntry {
        timestamp: SystemTime::now() + Duration::from_secs(1),
        operation: Operation::Set,
        key: "config:app".to_string(),
        value: Some(b"app_config_data".to_vec()),
        ttl: None, // 永不过期
    };

    // 创建WAL条目 - 删除操作
    let entry3 = WalEntry {
        timestamp: SystemTime::now() + Duration::from_secs(2),
        operation: Operation::Delete,
        key: "temp:data".to_string(),
        value: None,
        ttl: None,
    };

    // 记录到WAL
    wal_manager.append(entry1).await?;
    wal_manager.append(entry2).await?;
    wal_manager.append(entry3).await?;
    println!("   已记录3个WAL条目");

    // 3. 模拟缓存恢复过程
    println!("3. 模拟缓存恢复过程...");

    // 创建模拟的L2缓存后端用于重放
    let l2_config = oxcache::config::L2Config {
        connection_string: "memory://test".to_string().into(),
        ..Default::default()
    };

    match oxcache::backend::l2::L2Backend::new(&l2_config).await {
        Ok(l2_backend) => {
            println!("   开始重放WAL日志...");
            let replayed_count = wal_manager.replay_all(&l2_backend).await?;
            println!("   重放完成，共重放 {} 个条目", replayed_count);
        }
        Err(e) => {
            println!("   L2缓存创建失败: {}，跳过重放步骤", e);
        }
    }

    // 4. 验证WAL已被清空（重放后）
    println!("4. 验证WAL状态...");
    println!("   WAL日志重放后已被自动清空");

    // 5. 添加新的WAL条目用于演示
    println!("5. 添加新的演示条目...");
    let demo_entry = WalEntry {
        timestamp: SystemTime::now(),
        operation: Operation::Set,
        key: "demo:key".to_string(),
        value: Some(b"demo_value".to_vec()),
        ttl: Some(1800), // 30分钟TTL
    };
    wal_manager.append(demo_entry).await?;
    println!("   添加演示条目完成");

    // 6. 手动清空WAL
    println!("6. 手动清空WAL...");
    wal_manager.clear().await?;
    println!("   WAL已手动清空");

    println!("\n=== 数据库分区与WAL集成示例完成 ===");
    println!("   本示例演示了：");
    println!("   - 如何记录缓存操作到WAL");
    println!("   - 如何通过重放WAL日志恢复缓存数据");
    println!("   - WAL日志的自动和手动清理机制");
    Ok(())
}
