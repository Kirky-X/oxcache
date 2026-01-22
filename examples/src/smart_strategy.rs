//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! # 智能策略示例
//!
//! 演示如何使用智能预取和压缩策略。

use oxcache::smart_strategy::{SmartStrategyConfig, SmartStrategyManager};

#[tokio::main]
async fn main() {
    println!("=== 智能策略示例 ===\n");

    // ===========================================================================
    // 1. 基本使用
    // ===========================================================================
    println!("1. 基本使用示例");

    // 使用默认配置创建管理器
    let manager = SmartStrategyManager::new(None);
    println!("   - 创建默认配置管理器");

    // 模拟一些缓存访问
    for _ in 0..80 {
        manager.record_access(true); // 命中
    }
    for _ in 0..20 {
        manager.record_access(false); // 未命中
    }

    // 获取统计信息
    let stats = manager.hit_rate_stats();
    println!("   - 命中率: {:.1}%", stats.hit_rate * 100.0);
    println!("   - 总命中: {}, 总未命中: {}", stats.total_hits, stats.total_misses);
    println!();

    // ===========================================================================
    // 2. 自定义配置
    // ===========================================================================
    println!("2. 自定义配置示例");

    let config = SmartStrategyConfig {
        prefetch_enabled: true,
        prefetch_threshold: 0.7,  // 命中率低于 70% 时触发预取
        prefetch_window_size: 500,
        prefetch_batch_size: 20,
        compression_enabled: true,
        compression_threshold: 1024,  // 超过 1KB 才考虑压缩
        min_compression_ratio: 0.7,
        compression_sample_rate: 0.2,
    };

    let mut manager = SmartStrategyManager::new(Some(config));
    println!("   - 创建自定义配置管理器");
    println!("   - 预取阈值: {}%", manager.config().prefetch_threshold * 100.0);
    println!("   - 压缩阈值: {} bytes", manager.config().compression_threshold);
    println!();

    // ===========================================================================
    // 3. 压缩决策示例
    // ===========================================================================
    println!("3. 压缩决策示例");

    // 高可压缩性数据（重复模式）
    let compressible_data = vec![0x00u8; 2000];
    let should_compress = manager.should_compress(&compressible_data);
    println!("   - 重复零数据 (2000 bytes): 压缩={}", should_compress);

    // 低可压缩性数据（随机数据）
    let incompressible_data: Vec<u8> = (0..2000).map(|_| rand::random()).collect();
    let should_compress = manager.should_compress(&incompressible_data);
    println!("   - 随机数据 (2000 bytes): 压缩={}", should_compress);

    // 小数据不压缩
    let small_data = b"small data";
    let should_compress = manager.should_compress(small_data);
    println!("   - 小数据 (10 bytes): 压缩={}", should_compress);
    println!();

    // ===========================================================================
    // 4. 预取决策示例
    // ===========================================================================
    println!("4. 预取决策示例");

    // 初始状态 - 高命中率不应该触发预取
    for _ in 0..90 {
        manager.record_access(true);
    }
    for _ in 0..10 {
        manager.record_access(false);
    }
    println!("   - 高命中率 (90%): 预取={}", manager.should_prefetch());

    // 重置后模拟低命中率
    let stats = manager.hit_rate_stats();
    println!("   - 重置前统计: 命中={}, 未命中={}", stats.recent_hits, stats.recent_misses);

    // 模拟低命中率场景
    for _ in 0..30 {
        manager.record_access(true);
    }
    for _ in 0..70 {
        manager.record_access(false);
    }
    println!("   - 低命中率 (30%): 预取={}", manager.should_prefetch());
    println!();

    // ===========================================================================
    // 5. 配置更新
    // ===========================================================================
    println!("5. 配置更新示例");

    let new_config = SmartStrategyConfig {
        prefetch_threshold: 0.5,  // 更严格的预取阈值
        compression_threshold: 2048,
        ..Default::default()
    };

    manager.update_config(new_config);
    println!("   - 更新配置: 预取阈值={}%", manager.config().prefetch_threshold * 100.0);
    println!("   - 更新配置: 压缩阈值={} bytes", manager.config().compression_threshold);
    println!();

    println!("=== 智能策略示例完成 ===");
}
