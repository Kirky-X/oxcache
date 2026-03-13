// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 智能策略单元测试
//
// 测试智能缓存策略：命中率收集、预取决策、压缩决策等

#[cfg(test)]
mod smart_strategy_tests {
    use oxcache::smart_strategy::{
        CompressibilityChecker, CompressionDecider, HitRateCollector, PrefetchDecider, SmartStrategyConfig,
    };

    /// 测试命中率收集器的基本功能
    #[test]
    fn test_hit_rate_collector_basic() {
        let collector = HitRateCollector::new(200);

        // 初始状态 - 还没有记录，所以命中率应该是1.0（无数据时的默认值）
        assert_eq!(collector.hit_rate(), 1.0);

        // 记录一些命中
        for _ in 0..80 {
            collector.record_hit();
        }

        // 记录一些未命中
        for _ in 0..20 {
            collector.record_miss();
        }

        // 验证命中率
        assert_eq!(collector.hit_rate(), 0.8);
    }

    /// 测试命中率收集器的窗口滑动
    #[test]
    fn test_hit_rate_collector_window_sliding() {
        let collector = HitRateCollector::new(10); // 小窗口

        // 填充窗口
        for _ in 0..10 {
            collector.record_hit();
        }
        assert_eq!(collector.hit_rate(), 1.0);

        // 添加未命中
        collector.record_miss();
        // 命中率应该下降
        let rate = collector.hit_rate();
        assert!(
            (0.0..1.0).contains(&rate),
            "Rate should be between 0 and 1, got {}",
            rate
        );
    }

    /// 测试命中率收集器的重置
    #[test]
    fn test_hit_rate_collector_reset() {
        let collector = HitRateCollector::new(100);

        // 添加一些数据
        for _ in 0..50 {
            collector.record_hit();
        }
        for _ in 0..50 {
            collector.record_miss();
        }

        // 重置
        collector.reset();

        // 验证重置后的状态 - 命中率应该是1.0（无数据时的默认值）
        assert_eq!(collector.hit_rate(), 1.0);
    }

    /// 测试预取决策器 - 高命中率场景
    #[test]
    fn test_prefetch_decider_high_hit_rate() {
        let config = SmartStrategyConfig {
            prefetch_enabled: true,
            prefetch_threshold: 0.7,
            prefetch_window_size: 100,
            ..Default::default()
        };
        let decider = PrefetchDecider::new(config);
        let collector = decider.hit_rate_collector();

        // 高命中率（90%命中）
        for _ in 0..90 {
            collector.record_hit();
        }
        for _ in 0..10 {
            collector.record_miss();
        }

        // 不应该触发预取（命中率已经很高）
        assert!(!decider.should_prefetch());
    }

    /// 测试预取决策器 - 低命中率场景
    ///
    /// 注意：预取决策可能取决于最近窗口的命中率，而不是总命中率
    #[test]
    fn test_prefetch_decider_low_hit_rate() {
        let config = SmartStrategyConfig {
            prefetch_enabled: true,
            prefetch_threshold: 0.95, // 非常高的阈值
            prefetch_window_size: 100,
            ..Default::default()
        };
        let decider = PrefetchDecider::new(config.clone());
        let collector = decider.hit_rate_collector();

        // 记录大量未命中
        for _ in 0..50 {
            collector.record_miss();
        }

        // 应该有非常低的命中率
        let recent_rate = collector.recent_hit_rate();
        assert!(recent_rate < 0.5, "Recent hit rate should be low");

        // 在非常低的命中率下，应该触发预取
        let should_prefetch = decider.should_prefetch();
        assert!(should_prefetch || !config.prefetch_enabled, "Prefetch decision made");
    }

    /// 测试预取决策器 - 禁用场景
    #[test]
    fn test_prefetch_decider_disabled() {
        let config = SmartStrategyConfig {
            prefetch_enabled: false,
            prefetch_threshold: 0.7,
            prefetch_window_size: 100,
            ..Default::default()
        };
        let decider = PrefetchDecider::new(config);

        // 即使命中率很低，也不应该触发预取（因为被禁用）
        let collector = decider.hit_rate_collector();
        for _ in 0..10 {
            collector.record_hit();
        }
        for _ in 0..90 {
            collector.record_miss();
        }

        assert!(!decider.should_prefetch());
    }

    /// 测试可压缩性检查器 - 高可压缩性数据
    #[test]
    fn test_compressibility_checker_highly_compressible() {
        let checker = CompressibilityChecker::default();

        // 重复模式的数据（高可压缩性）
        let compressible = vec![0x00u8; 1000];
        let (worth_compressing, ratio) = checker.check_compressibility(&compressible);

        assert!(worth_compressing, "Repeated zeros should be compressible");
        assert!(
            ratio < 0.5,
            "Compression ratio should be < 0.5 for highly compressible data"
        );
    }

    /// 测试可压缩性检查器 - 低可压缩性数据
    #[test]
    fn test_compressibility_checker_low_compressibility() {
        let checker = CompressibilityChecker::default();

        // 随机数据（低可压缩性）
        let incompressible: Vec<u8> = (0..1000).map(|_| rand::random()).collect();
        let (worth_compressing, _) = checker.check_compressibility(&incompressible);

        assert!(!worth_compressing, "Random data should not be worth compressing");
    }

    /// 测试可压缩性检查器 - 中等可压缩性数据
    #[test]
    fn test_compressibility_checker_mixed() {
        let checker = CompressibilityChecker::default();

        // 部分重复的数据
        let mixed: Vec<u8> = (0..500)
            .map(|i| if i % 10 == 0 { 0xAA } else { rand::random() })
            .collect();

        let (_, ratio) = checker.check_compressibility(&mixed);

        // 随机数据不应该太容易压缩
        assert!(ratio > 0.3, "Mixed data should have moderate compression ratio");
    }

    /// 测试压缩决策器 - 启用场景
    #[test]
    fn test_compression_decider_enabled() {
        let config = SmartStrategyConfig {
            compression_enabled: true,
            compression_threshold: 50,  // 设置较低的阈值
            min_compression_ratio: 0.5, // 允许更激进压缩
            ..Default::default()
        };
        let decider = CompressionDecider::new(config);

        // 大型可压缩数据
        let large_compressible = vec![0x00u8; 200];
        let should_compress = decider.should_compress(&large_compressible);
        // 大型数据应该被压缩
        assert!(should_compress, "Large data should be compressed");

        // 小数据不应该压缩
        let small_data = vec![0x00u8; 10];
        assert!(!decider.should_compress(&small_data));
    }

    /// 测试压缩决策器 - 禁用场景
    #[test]
    fn test_compression_decider_disabled() {
        let config = SmartStrategyConfig {
            compression_enabled: false,
            ..Default::default()
        };
        let decider = CompressionDecider::new(config);

        // 即使是可压缩的大数据，也不应该压缩
        let data = vec![0x00u8; 2000];
        assert!(!decider.should_compress(&data));
    }

    /// 测试命中率收集器的统计
    #[test]
    fn test_hit_rate_collector_stats() {
        let collector = HitRateCollector::new(100);

        // 添加一些数据
        for _ in 0..75 {
            collector.record_hit();
        }
        for _ in 0..25 {
            collector.record_miss();
        }

        // 验证统计数据
        let hit_rate = collector.hit_rate();
        assert!(
            (hit_rate - 0.75).abs() < 0.01,
            "Hit rate should be approximately 0.75, got {}",
            hit_rate
        );
    }

    /// 测试智能策略配置默认值
    #[test]
    fn test_smart_strategy_config_defaults() {
        let config = SmartStrategyConfig::default();

        // 验证配置值 - 基于 Default 实现
        assert!(config.prefetch_enabled); // 默认是 true
        assert_eq!(config.prefetch_threshold, 0.8);
        assert_eq!(config.prefetch_window_size, 1000);
        assert!(config.compression_enabled); // 默认是 true
        assert_eq!(config.compression_threshold, 1024);
    }
}
