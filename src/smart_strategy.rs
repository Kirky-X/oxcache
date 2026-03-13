//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 智能预取/压缩策略模块
//!
//! 提供基于命中率的自动预取决策和启发式可压缩性检查。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// 智能策略配置
#[derive(Debug, Clone)]
pub struct SmartStrategyConfig {
    /// 是否启用自动预取
    pub prefetch_enabled: bool,
    /// 预取触发阈值（命中率低于此值时触发预取）
    pub prefetch_threshold: f64,
    /// 预取窗口大小（用于计算命中率的请求数）
    pub prefetch_window_size: usize,
    /// 预取批量大小
    pub prefetch_batch_size: usize,
    /// 是否启用自动压缩
    pub compression_enabled: bool,
    /// 压缩阈值（字节大小，超过此值才考虑压缩）
    pub compression_threshold: usize,
    /// 最小压缩率（压缩后大小/原始大小 < 此值时才压缩）
    pub min_compression_ratio: f64,
    /// 压缩采样率（用于启发式检查的采样比例）
    pub compression_sample_rate: f64,
}

impl Default for SmartStrategyConfig {
    fn default() -> Self {
        Self {
            prefetch_enabled: true,
            prefetch_threshold: 0.8,
            prefetch_window_size: 1000,
            prefetch_batch_size: 10,
            compression_enabled: true,
            compression_threshold: 1024,
            min_compression_ratio: 0.8,
            compression_sample_rate: 0.1,
        }
    }
}

/// 命中率统计收集器
#[derive(Debug, Clone)]
pub struct HitRateCollector {
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    window_size: usize,
    recent_hits: Arc<AtomicU64>,
    recent_misses: Arc<AtomicU64>,
}

impl HitRateCollector {
    /// 创建新的命中率收集器
    pub fn new(window_size: usize) -> Self {
        Self {
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            window_size,
            recent_hits: Arc::new(AtomicU64::new(0)),
            recent_misses: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 记录命中
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.recent_hits.fetch_add(1, Ordering::Relaxed);
        self.rotate_window_if_needed();
    }

    /// 记录未命中
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.recent_misses.fetch_add(1, Ordering::Relaxed);
        self.rotate_window_if_needed();
    }

    /// 获取当前命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            1.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 获取最近窗口的命中率
    pub fn recent_hit_rate(&self) -> f64 {
        let hits = self.recent_hits.load(Ordering::Relaxed);
        let misses = self.recent_misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            1.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 检查是否需要触发预取
    pub fn should_prefetch(&self, threshold: f64) -> bool {
        self.recent_hit_rate() < threshold
    }

    /// 旋转窗口
    fn rotate_window_if_needed(&self) {
        let total = self.recent_hits.load(Ordering::Relaxed) + self.recent_misses.load(Ordering::Relaxed);
        if total >= self.window_size as u64 {
            self.recent_hits.store(0, Ordering::Relaxed);
            self.recent_misses.store(0, Ordering::Relaxed);
        }
    }

    /// 获取统计数据
    pub fn stats(&self) -> HitRateStats {
        HitRateStats {
            total_hits: self.hits.load(Ordering::Relaxed),
            total_misses: self.misses.load(Ordering::Relaxed),
            recent_hits: self.recent_hits.load(Ordering::Relaxed),
            recent_misses: self.recent_misses.load(Ordering::Relaxed),
            hit_rate: self.hit_rate(),
            recent_hit_rate: self.recent_hit_rate(),
        }
    }

    /// 重置统计
    pub fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.recent_hits.store(0, Ordering::Relaxed);
        self.recent_misses.store(0, Ordering::Relaxed);
    }
}

/// 命中率统计信息
#[derive(Debug, Clone)]
pub struct HitRateStats {
    pub total_hits: u64,
    pub total_misses: u64,
    pub recent_hits: u64,
    pub recent_misses: u64,
    pub hit_rate: f64,
    pub recent_hit_rate: f64,
}

/// 可压缩性检查器
#[derive(Debug, Clone)]
pub struct CompressibilityChecker {
    sample_size: usize,
    entropy_threshold: f64,
}

impl CompressibilityChecker {
    /// 创建新的可压缩性检查器
    pub fn new(sample_size: usize, entropy_threshold: f64) -> Self {
        Self {
            sample_size: sample_size.max(1), // 确保不为0
            entropy_threshold,
        }
    }

    /// 检查数据是否可压缩
    /// 返回 (是否值得压缩, 估计压缩率)
    pub fn check_compressibility(&self, data: &[u8]) -> (bool, f64) {
        if data.len() < self.sample_size || self.sample_size == 0 {
            return (false, 1.0);
        }

        // 采样检查
        let step = (data.len() / self.sample_size).max(1);
        let sample: Vec<u8> = data.iter().step_by(step).copied().collect();

        if sample.is_empty() {
            return (false, 1.0);
        }

        // 计算字节分布熵
        let entropy = self.calculate_entropy(&sample);

        // 高熵数据（接近均匀分布）通常不可压缩
        // 低熵数据（重复模式多）通常可压缩
        let normalized_entropy = entropy / 8.0; // 归一化到 0-1

        // 估计压缩率（基于熵的简化模型）
        let estimated_ratio = 0.3 + 0.7 * normalized_entropy;

        // 如果熵低于阈值，认为可压缩
        let worth_compressing =
            normalized_entropy < self.entropy_threshold && estimated_ratio < 0.8 && data.len() > self.sample_size;

        (worth_compressing, estimated_ratio)
    }

    /// 计算字节分布熵
    fn calculate_entropy(&self, data: &[u8]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        // 统计字节频率
        let mut counts = [0u64; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }

        // 计算熵
        let total = data.len() as f64;
        let mut entropy = 0.0f64;

        for &count in &counts {
            if count > 0 {
                let p = count as f64 / total;
                entropy -= p * p.log2();
            }
        }

        entropy
    }
}

impl Default for CompressibilityChecker {
    fn default() -> Self {
        Self {
            sample_size: 256,
            entropy_threshold: 0.9,
        }
    }
}

/// 智能预取决策器
#[derive(Debug, Clone)]
pub struct PrefetchDecider {
    config: SmartStrategyConfig,
    hit_rate_collector: Arc<HitRateCollector>,
    last_prefetch_time: Arc<std::sync::atomic::AtomicU64>,
    min_prefetch_interval: Duration,
}

impl PrefetchDecider {
    /// 创建新的预取决策器
    pub fn new(config: SmartStrategyConfig) -> Self {
        Self {
            config: config.clone(),
            hit_rate_collector: Arc::new(HitRateCollector::new(config.prefetch_window_size)),
            last_prefetch_time: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            min_prefetch_interval: Duration::from_secs(5),
        }
    }

    /// 记录访问结果
    pub fn record_access(&self, hit: bool) {
        if hit {
            self.hit_rate_collector.record_hit();
        } else {
            self.hit_rate_collector.record_miss();
        }
    }

    /// 检查是否应该执行预取
    pub fn should_prefetch(&self) -> bool {
        if !self.config.prefetch_enabled {
            return false;
        }

        // 检查时间间隔
        let last = std::sync::atomic::AtomicU64::load(&self.last_prefetch_time, Ordering::Relaxed);
        let last_time = SystemTime::UNIX_EPOCH + Duration::from_secs(last);
        let now = SystemTime::now();

        if let Ok(elapsed) = now.duration_since(last_time) {
            if elapsed < self.min_prefetch_interval {
                return false;
            }
        }

        // 检查命中率阈值
        self.hit_rate_collector.should_prefetch(self.config.prefetch_threshold)
    }

    /// 标记预取已执行
    pub fn mark_prefetched(&self) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        std::sync::atomic::AtomicU64::store(&self.last_prefetch_time, now, Ordering::Relaxed);
    }

    /// 获取配置
    pub fn config(&self) -> &SmartStrategyConfig {
        &self.config
    }

    /// 获取命中率统计
    pub fn hit_rate_stats(&self) -> HitRateStats {
        self.hit_rate_collector.stats()
    }

    /// 获取命中率收集器的引用
    pub fn hit_rate_collector(&self) -> &Arc<HitRateCollector> {
        &self.hit_rate_collector
    }
}

/// 智能压缩决策器
#[derive(Debug, Clone)]
pub struct CompressionDecider {
    config: SmartStrategyConfig,
    checker: CompressibilityChecker,
}

impl CompressionDecider {
    /// 创建新的压缩决策器
    pub fn new(config: SmartStrategyConfig) -> Self {
        // 使用config中的参数创建压缩性检查器
        let sample_size = config.compression_threshold.max(100);
        let entropy_threshold = (1.0 - config.min_compression_ratio).max(0.1);

        Self {
            config: config.clone(),
            checker: CompressibilityChecker::new(sample_size, entropy_threshold),
        }
    }

    /// 检查是否应该压缩数据
    pub fn should_compress(&self, data: &[u8]) -> bool {
        if !self.config.compression_enabled {
            return false;
        }

        // 检查大小阈值
        if data.len() < self.config.compression_threshold {
            return false;
        }

        // 可压缩性检查
        let (worth_compressing, _) = self.checker.check_compressibility(data);
        worth_compressing
    }

    /// 获取配置
    pub fn config(&self) -> &SmartStrategyConfig {
        &self.config
    }
}

/// 智能策略管理器
#[derive(Debug, Clone)]
pub struct SmartStrategyManager {
    prefetch_decider: Arc<PrefetchDecider>,
    compression_decider: Arc<CompressionDecider>,
    config: SmartStrategyConfig,
}

impl SmartStrategyManager {
    /// 创建新的智能策略管理器
    pub fn new(config: Option<SmartStrategyConfig>) -> Self {
        let config = config.unwrap_or_default();
        Self {
            prefetch_decider: Arc::new(PrefetchDecider::new(config.clone())),
            compression_decider: Arc::new(CompressionDecider::new(config.clone())),
            config,
        }
    }

    /// 记录缓存访问
    pub fn record_access(&self, hit: bool) {
        self.prefetch_decider.record_access(hit);
    }

    /// 检查是否应该执行预取
    pub fn should_prefetch(&self) -> bool {
        self.prefetch_decider.should_prefetch()
    }

    /// 标记预取已执行
    pub fn mark_prefetched(&self) {
        self.prefetch_decider.mark_prefetched();
    }

    /// 检查是否应该压缩数据
    pub fn should_compress(&self, data: &[u8]) -> bool {
        self.compression_decider.should_compress(data)
    }

    /// 获取配置
    pub fn config(&self) -> &SmartStrategyConfig {
        &self.config
    }

    /// 获取预取决策器
    pub fn prefetch_decider(&self) -> &Arc<PrefetchDecider> {
        &self.prefetch_decider
    }

    /// 获取压缩决策器
    pub fn compression_decider(&self) -> &Arc<CompressionDecider> {
        &self.compression_decider
    }

    /// 更新配置
    pub fn update_config(&mut self, config: SmartStrategyConfig) {
        self.config = config.clone();
        // 重新创建决策器以应用新配置
        self.prefetch_decider = Arc::new(PrefetchDecider::new(config.clone()));
        self.compression_decider = Arc::new(CompressionDecider::new(config));
    }

    /// 获取命中率统计
    pub fn hit_rate_stats(&self) -> HitRateStats {
        self.prefetch_decider.hit_rate_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hit_rate_collector() {
        let collector = HitRateCollector::new(200); // 窗口大小大于记录数量

        // 记录一些命中和未命中
        for _ in 0..80 {
            collector.record_hit();
        }
        for _ in 0..20 {
            collector.record_miss();
        }

        assert_eq!(collector.hit_rate(), 0.8);
        assert_eq!(collector.recent_hit_rate(), 0.8);
    }

    #[test]
    fn test_should_prefetch() {
        let decider = PrefetchDecider::new(SmartStrategyConfig {
            prefetch_enabled: true,
            prefetch_threshold: 0.8,
            prefetch_window_size: 200, // 窗口大小大于记录数量
            ..Default::default()
        });

        // 使用内部的收集器引用
        let collector_ref = decider.hit_rate_collector();

        // 高命中率，不应该触发预取
        for _ in 0..90 {
            collector_ref.record_hit();
        }
        for _ in 0..10 {
            collector_ref.record_miss();
        }
        assert!(!decider.should_prefetch());

        // 重置并制造低命中率
        collector_ref.reset();
        for _ in 0..30 {
            collector_ref.record_hit();
        }
        for _ in 0..70 {
            collector_ref.record_miss();
        }
        assert!(decider.should_prefetch());
    }

    #[test]
    fn test_compressibility_checker() {
        let checker = CompressibilityChecker::default();

        // 高可压缩性数据（重复模式）
        let compressible = vec![0x00u8; 1000];
        let (worth_compressing, ratio) = checker.check_compressibility(&compressible);
        assert!(worth_compressing);
        assert!(ratio < 0.5);

        // 低可压缩性数据（随机数据）
        let incompressible: Vec<u8> = (0..1000).map(|_| rand::random()).collect();
        let (worth_compressing, _) = checker.check_compressibility(&incompressible);
        assert!(!worth_compressing);
    }

    #[test]
    fn test_compression_decider_disabled() {
        let config = SmartStrategyConfig {
            compression_enabled: false,
            ..Default::default()
        };
        let decider = CompressionDecider::new(config);

        let data = vec![0x00u8; 2000];
        assert!(!decider.should_compress(&data));
    }

    #[test]
    fn test_compression_decider_size_threshold() {
        let config = SmartStrategyConfig {
            compression_enabled: true,
            compression_threshold: 1024,
            ..Default::default()
        };
        let decider = CompressionDecider::new(config);

        // 小于阈值的数据不应该压缩
        let small_data = vec![0x00u8; 500];
        assert!(!decider.should_compress(&small_data));

        // 大于阈值的数据会进行可压缩性检查
        let large_data = vec![0x00u8; 2000];
        assert!(decider.should_compress(&large_data));
    }

    #[test]
    fn test_smart_strategy_manager() {
        let manager = SmartStrategyManager::new(None);

        // 记录访问
        for _ in 0..80 {
            manager.record_access(true);
        }
        for _ in 0..20 {
            manager.record_access(false);
        }

        // 检查命中率统计
        let stats = manager.hit_rate_stats();
        assert_eq!(stats.hit_rate, 0.8);
        assert_eq!(stats.total_hits, 80);
        assert_eq!(stats.total_misses, 20);
    }

    #[test]
    fn test_smart_strategy_manager_update_config() {
        let mut manager = SmartStrategyManager::new(None);

        let new_config = SmartStrategyConfig {
            prefetch_threshold: 0.5,
            compression_threshold: 2048,
            ..Default::default()
        };

        manager.update_config(new_config.clone());

        assert_eq!(manager.config().prefetch_threshold, 0.5);
        assert_eq!(manager.config().compression_threshold, 2048);
    }
}
