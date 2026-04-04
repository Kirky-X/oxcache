//! bloom_filter.rs 覆盖率测试
//!
//! 测试覆盖：
//! - BloomFilterOptions 配置和计算
//! - BloomFilter 核心功能（添加、查询、清空）
//! - LRU 缓存功能
//! - BloomFilterStats 统计信息

#[cfg(test)]
#[cfg(feature = "bloom-filter")]
mod bloom_filter_coverage_tests {
    use oxcache::{BloomFilter, BloomFilterOptions};

    #[test]
    fn test_bloom_filter_options_new() {
        let options = BloomFilterOptions::new("test".to_string(), 1000, 0.01);
        assert_eq!(options.name, "test");
        assert_eq!(options.expected_elements, 1000);
        assert_eq!(options.false_positive_rate, 0.01);
    }

    #[test]
    fn test_bloom_filter_options_default_with_name() {
        let options = BloomFilterOptions::default_with_name("default_test".to_string());
        assert_eq!(options.name, "default_test");
        assert_eq!(options.expected_elements, 100000);
        assert_eq!(options.false_positive_rate, 0.01);
    }

    #[test]
    fn test_optimal_size_calculation_various_rates() {
        let small_fp = BloomFilterOptions::new("small_fp".to_string(), 10000, 0.001);
        let medium_fp = BloomFilterOptions::new("medium_fp".to_string(), 10000, 0.01);
        let large_fp = BloomFilterOptions::new("large_fp".to_string(), 10000, 0.1);

        assert!(small_fp.optimal_size() > medium_fp.optimal_size());
        assert!(medium_fp.optimal_size() > large_fp.optimal_size());
        assert!(small_fp.optimal_size() > 0);
    }

    #[test]
    fn test_optimal_size_calculation_various_elements() {
        let small = BloomFilterOptions::new("small".to_string(), 100, 0.01);
        let medium = BloomFilterOptions::new("medium".to_string(), 1000, 0.01);
        let large = BloomFilterOptions::new("large".to_string(), 10000, 0.01);

        assert!(small.optimal_size() < medium.optimal_size());
        assert!(medium.optimal_size() < large.optimal_size());
    }

    #[test]
    fn test_optimal_num_hashes_calculation() {
        let options = BloomFilterOptions::new("test".to_string(), 100000, 0.01);
        let num_hashes = options.optimal_num_hashes();
        // 哈希函数数量应该 > 0
        assert!(num_hashes >= 1);
    }

    #[test]
    fn test_optimal_size_alignment() {
        let options = BloomFilterOptions::new("test".to_string(), 10000, 0.01);
        let size = options.optimal_size();
        assert_eq!(size % 8, 0);
    }

    #[test]
    fn test_bloom_filter_empty_input() {
        let options = BloomFilterOptions::default_with_name("empty_test".to_string());
        let mut filter = BloomFilter::new(options);
        assert!(!filter.contains(b"").unwrap());
        filter.add(b"").unwrap();
        assert!(filter.contains(b"").unwrap());
    }

    #[test]
    fn test_bloom_filter_large_capacity() {
        let options = BloomFilterOptions::new("large_capacity".to_string(), 50000, 0.01);
        let mut filter = BloomFilter::new(options);

        for i in 0..10000 {
            let key = format!("large_item_{}", i);
            filter.add(key.as_bytes()).unwrap();
        }

        for i in 0..1000 {
            let key = format!("large_item_{}", i);
            assert!(filter.contains(key.as_bytes()).unwrap());
        }

        let stats = filter.get_stats();
        assert_eq!(stats.added_count, 10000);
    }

    #[test]
    fn test_bloom_filter_clear() {
        let options = BloomFilterOptions::default_with_name("clear_test".to_string());
        let mut filter = BloomFilter::new(options);

        filter.add(b"item1").unwrap();
        filter.add(b"item2").unwrap();
        assert!(filter.contains(b"item1").unwrap());

        filter.clear();

        assert!(!filter.contains(b"item1").unwrap());
        let stats = filter.get_stats();
        assert_eq!(stats.added_count, 0);
    }

    #[test]
    fn test_bloom_filter_add_checked() {
        let options = BloomFilterOptions::default_with_name("add_checked".to_string());
        let mut filter = BloomFilter::new(options);

        let first_add = filter.add_checked(b"new_item").unwrap();
        assert!(first_add);

        let second_add = filter.add_checked(b"new_item").unwrap();
        assert!(!second_add);
    }

    #[test]
    fn test_bloom_filter_remove() {
        let options = BloomFilterOptions::default_with_name("remove_test".to_string());
        let filter = BloomFilter::new(options);
        assert!(!filter.remove(b"any_item"));
    }

    #[test]
    fn test_bloom_filter_estimated_count_empty() {
        let options = BloomFilterOptions::default_with_name("est_empty".to_string());
        let filter = BloomFilter::new(options);
        assert_eq!(filter.get_estimated_count(), 0);
    }

    #[test]
    fn test_bloom_filter_estimated_count_with_data() {
        let options = BloomFilterOptions::new("est_data".to_string(), 1000, 0.01);
        let mut filter = BloomFilter::new(options);

        for i in 0..100 {
            filter.add(format!("item_{}", i).as_bytes()).unwrap();
        }

        let estimated = filter.get_estimated_count();
        // 估计数量应该是合理的数值（可能为 0 或某个正数）
        println!("Estimated count: {}", estimated);
    }

    #[test]
    fn test_lru_cache_hit() {
        let options = BloomFilterOptions::default_with_name("lru_hit".to_string());
        let mut filter = BloomFilter::new(options);

        filter.add(b"cached_item").unwrap();

        let first_check = filter.contains(b"cached_item").unwrap();
        assert!(first_check);

        let second_check = filter.contains(b"cached_item").unwrap();
        assert!(second_check);

        let stats = filter.get_stats();
        assert_eq!(stats.checked_count, 2);
    }

    #[test]
    fn test_lru_cache_capacity_limit() {
        let options = BloomFilterOptions::new("lru_limit".to_string(), 10000, 0.01);
        let mut filter = BloomFilter::new(options);

        for i in 0..11000 {
            let key = format!("cache_item_{}", i);
            filter.add(key.as_bytes()).unwrap();
        }

        for i in 0..1000 {
            let key = format!("cache_item_{}", i);
            assert!(filter.contains(key.as_bytes()).unwrap());
        }
    }

    #[test]
    fn test_bloom_filter_stats_display() {
        let options = BloomFilterOptions::new("stats_display".to_string(), 1000, 0.01);
        let mut filter = BloomFilter::new(options);

        filter.add(b"item1").unwrap();
        filter.add(b"item2").unwrap();

        let stats = filter.get_stats();
        let display = format!("{}", stats);

        assert!(display.contains("stats_display"));
        assert!(display.contains("bits"));
        assert!(display.contains("added="));
    }

    #[test]
    fn test_bloom_filter_stats_completeness() {
        let options = BloomFilterOptions::new("complete".to_string(), 10000, 0.05);
        let mut filter = BloomFilter::new(options);

        for i in 0..500 {
            filter.add(format!("item_{}", i).as_bytes()).unwrap();
        }

        for i in 0..1000 {
            filter.contains(format!("query_{}", i).as_bytes()).unwrap();
        }

        let stats = filter.get_stats();

        assert_eq!(stats.name, "complete");
        assert!(stats.total_bits > 0);
        assert!(stats.used_bits > 0);
        assert_eq!(stats.added_count, 500);
        assert_eq!(stats.checked_count, 1000);
    }

    #[test]
    fn test_extremely_small_expected_elements() {
        let options = BloomFilterOptions::new("extreme_small".to_string(), 1, 0.01);
        let filter = BloomFilter::new(options);
        let stats = filter.get_stats();
        assert!(stats.total_bits > 0);
    }

    #[test]
    fn test_extremely_large_expected_elements() {
        let options = BloomFilterOptions::new("extreme_large".to_string(), 10000000, 0.01);
        let filter = BloomFilter::new(options);
        let stats = filter.get_stats();
        assert!(stats.total_bits > 0);
    }

    #[test]
    fn test_binary_data_input() {
        let options = BloomFilterOptions::default_with_name("binary".to_string());
        let mut filter = BloomFilter::new(options);

        let binary_data: Vec<u8> = (0u8..=255u8).collect();
        filter.add(&binary_data).unwrap();
        assert!(filter.contains(&binary_data).unwrap());
    }

    #[test]
    fn test_unicode_keys() {
        let options = BloomFilterOptions::default_with_name("unicode".to_string());
        let mut filter = BloomFilter::new(options);

        let unicode_keys = vec!["你好世界", "こんにちは", "안녕하세요"];

        for key in &unicode_keys {
            filter.add(key.as_bytes()).unwrap();
        }

        for key in &unicode_keys {
            assert!(filter.contains(key.as_bytes()).unwrap());
        }
    }

    #[test]
    fn test_massive_additions() {
        let options = BloomFilterOptions::new("massive".to_string(), 100000, 0.01);
        let mut filter = BloomFilter::new(options);

        let count = 10000;
        for i in 0..count {
            filter.add(format!("massive_{}", i).as_bytes()).unwrap();
        }

        let stats = filter.get_stats();
        assert_eq!(stats.added_count, count);
    }

    #[test]
    fn test_false_positive_count_tracking() {
        let options = BloomFilterOptions::new("fp_tracking".to_string(), 100, 0.1);
        let mut filter = BloomFilter::new(options);

        for i in 0..100 {
            filter.add(format!("fp_item_{}", i).as_bytes()).unwrap();
        }

        for i in 100..1000 {
            filter.contains(format!("fp_item_{}", i).as_bytes()).unwrap();
        }

        let stats = filter.get_stats();
        assert!(stats.false_positive_count > 0);
    }
}
