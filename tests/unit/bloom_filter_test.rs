// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 布隆过滤器单元测试
//
// 测试布隆过滤器核心功能：添加、查询、误报率计算等

#[cfg(test)]
#[cfg(feature = "bloom-filter")]
mod bloom_filter_tests {
    use oxcache::error::CacheError;
    use oxcache::{BloomFilter, BloomFilterOptions};

    /// 测试布隆过滤器的基本添加和查询功能
    #[test]
    fn test_bloom_filter_basic_operations() -> Result<(), CacheError> {
        let options = BloomFilterOptions::default_with_name("test_basic".to_string());
        let mut filter = BloomFilter::new(options);

        // 初始状态：查询不存在的键应返回false
        assert!(!filter.contains(b"hello")?, "New filter should not contain 'hello'");
        assert!(!filter.contains(b"world")?, "New filter should not contain 'world'");

        // 添加元素
        filter.add(b"hello")?;

        // 查询已添加的键应返回true
        assert!(filter.contains(b"hello")?, "Filter should contain 'hello' after add");
        assert!(!filter.contains(b"world")?, "Filter should not contain 'world'");

        // 添加另一个元素
        filter.add(b"world")?;

        // 两个元素都应该能被查询到
        assert!(filter.contains(b"hello")?, "Filter should still contain 'hello'");
        assert!(filter.contains(b"world")?, "Filter should contain 'world' after add");

        Ok(())
    }

    /// 测试 contains_and_add 方法（原子操作）
    #[test]
    fn test_bloom_filter_contains_and_add() -> Result<(), CacheError> {
        let options = BloomFilterOptions::default_with_name("test_contains_and_add".to_string());
        let mut filter = BloomFilter::new(options);

        // 第一次调用：元素不存在，返回false并添加
        assert!(!filter.contains_and_add(b"new_item")?, "First call should return false");
        assert!(filter.contains(b"new_item")?, "Item should be added after first call");

        // 第二次调用：元素已存在，返回true
        assert!(filter.contains_and_add(b"new_item")?, "Second call should return true");

        Ok(())
    }

    /// 测试布隆过滤器的误报率
    #[test]
    fn test_bloom_filter_false_positive_rate() -> Result<(), CacheError> {
        // 使用较高容量和较低误报率配置
        let options = BloomFilterOptions::new("test_fp".to_string(), 10000, 0.01);
        let mut filter = BloomFilter::new(options);

        // 添加1000个不同的元素
        for i in 0..1000 {
            filter.add(format!("item_{}", i).as_bytes())?;
        }

        // 检查1000个未添加的元素（应该是"假阳性"的）
        let mut false_positives = 0;
        for i in 1000..2000 {
            if filter.contains(format!("fake_{}", i).as_bytes())? {
                false_positives += 1;
            }
        }

        // 计算误报率
        let fp_rate = false_positives as f64 / 1000.0;

        // 误报率应该低于配置的0.01（有一定的容差）
        assert!(
            fp_rate < 0.05,
            "False positive rate {} is too high, expected < 0.05",
            fp_rate
        );

        Ok(())
    }

    /// 测试不同配置的布隆过滤器
    #[test]
    fn test_bloom_filter_configurations() {
        // 默认配置
        let options_default = BloomFilterOptions::default_with_name("default".to_string());
        assert!(options_default.optimal_size() > 0);
        assert!(options_default.optimal_num_hashes() > 0);

        // 自定义配置 - 更多的元素和更低的误报率
        let options_custom = BloomFilterOptions::new("custom".to_string(), 50000, 0.001);
        assert_eq!(options_custom.name, "custom");
        // 自定义配置应该有更多的预期元素数量
        assert!(options_custom.expected_elements > 0);
        // 自定义配置应该有更低的误报率
        assert!(options_custom.false_positive_rate < options_default.false_positive_rate);
    }

    /// 测试移除功能（标准布隆过滤器不支持真正移除）
    #[test]
    fn test_bloom_filter_remove() {
        let options = BloomFilterOptions::default_with_name("test_remove".to_string());
        let mut filter = BloomFilter::new(options);

        // 添加元素
        let _ = filter.add(b"test");

        // 标准布隆过滤器的 remove 方法总是返回 false
        let result = filter.remove(b"test");
        assert!(!result, "Bloom filter remove should return false");
    }

    /// 测试布隆过滤器的统计信息
    #[test]
    fn test_bloom_filter_stats() {
        let options = BloomFilterOptions::default_with_name("test_stats".to_string());
        let mut filter = BloomFilter::new(options);

        // 添加一些元素
        for i in 0..100 {
            let _ = filter.add(format!("item_{}", i).as_bytes());
        }

        // 获取统计信息
        let stats = filter.get_stats();

        // 验证统计信息
        assert_eq!(stats.name, "test_stats");
    }

    /// 测试空名称配置
    #[test]
    fn test_bloom_filter_empty_name() {
        let options = BloomFilterOptions::new("".to_string(), 1000, 0.01);
        assert!(options.name.is_empty());
    }
}
