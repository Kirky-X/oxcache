//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 自适应预取模块
//!
//! 实现智能缓存预取功能，根据访问模式预测并预加载即将使用的数据。

use crate::backend::strategy::L2BackendStrategy;
use crate::error::Result;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// 访问模式追踪器
///
/// 记录键的访问频率和时间局部性，用于预测即将访问的键。
#[derive(Clone)]
pub struct AccessPatternTracker {
    /// 频率计数器：(key -> 访问次数)
    frequency: Arc<DashMap<String, AtomicU64>>,
    /// 最近访问记录：(key, 时间戳)
    recent_access: Arc<DashMap<String, Instant>>,
    /// 访问历史窗口（用于时间局部性分析）
    access_window: Arc<DashMap<String, VecDeque<Instant>>>,
    /// 全局访问计数
    total_accesses: Arc<AtomicU64>,
    /// 键之间的关联关系（共同出现频率）
    key_correlations: Arc<DashMap<String, DashMap<String, u32>>>,
}

impl AccessPatternTracker {
    /// 创建新的访问模式追踪器
    pub fn new() -> Self {
        Self {
            frequency: Arc::new(DashMap::new()),
            recent_access: Arc::new(DashMap::new()),
            access_window: Arc::new(DashMap::new()),
            total_accesses: Arc::new(AtomicU64::new(0)),
            key_correlations: Arc::new(DashMap::new()),
        }
    }

    /// 记录一次访问
    ///
    /// # 参数
    /// * `key` - 被访问的键
    /// * `correlated_keys` - 同时访问的相关键（用于关联分析）
    pub fn record_access(&self, key: &str, correlated_keys: &[&str]) {
        // 更新频率
        let freq_entry = self.frequency.entry(key.to_string()).or_default();
        freq_entry.fetch_add(1, Ordering::Relaxed);

        // 更新最近访问时间
        let now = Instant::now();
        self.recent_access.insert(key.to_string(), now);

        // 更新访问窗口
        let window_entry = self.access_window.entry(key.to_string()).or_default();
        window_entry.push_back(now);
        // 只保留最近 N 次访问
        const MAX_WINDOW_SIZE: usize = 100;
        while window_entry.len() > MAX_WINDOW_SIZE {
            window_entry.pop_front();
        }

        // 更新关联关系
        for &corr_key in correlated_keys {
            let corr_map = self.key_correlations
                .entry(key.to_string())
                .or_default();
            let corr_entry = corr_map.entry(corr_key.to_string()).or_default();
            *corr_entry += 1;
        }

        // 增加全局计数
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取键的访问频率得分
    ///
    /// 综合考虑频率和时间局部性。
    pub fn get_access_score(&self, key: &str) -> f64 {
        let freq = self.frequency.get(key).map(|v| v.load(Ordering::Relaxed)).unwrap_or(0) as f64;

        // 时间局部性：最近访问越频繁，得分越高
        let recency_score = if let Some(accesses) = self.access_window.get(key) {
            let now = Instant::now();
            let recent_count = accesses
                .iter()
                .filter(|&&t| now.duration_since(t) < Duration::from_secs(60))
                .count() as f64;
            recent_count / 60.0 // 每秒权重
        } else {
            0.0
        };

        // 综合得分 = 频率得分 * 0.6 + 时间局部性 * 0.4
        (freq * 0.6 + recency_score * 0.4).max(0.0)
    }

    /// 获取最可能预取的键
    ///
    /// 基于关联关系预测即将访问的键。
    pub fn get_predicted_keys(&self, accessed_key: &str, limit: usize) -> Vec<(String, f64)> {
        let mut predictions = Vec::new();

        // 基于关联关系预测
        if let Some(corr_map) = self.key_correlations.get(accessed_key) {
            for entry in corr_map.iter() {
                let key = entry.key().clone();
                let score = *entry.value() as f64;
                let access_score = self.get_access_score(&key);

                // 综合关联得分和访问得分
                let combined_score = score * 0.7 + access_score * 0.3;
                predictions.push((key, combined_score));
            }
        }

        // 按得分排序
        predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        predictions.into_iter().take(limit).collect()
    }

    /// 获取高频率访问的键
    pub fn get_hot_keys(&self, min_frequency: u64, limit: usize) -> Vec<(String, u64)> {
        let mut hot_keys: Vec<_> = self
            .frequency
            .iter()
            .filter(|e| e.value().load(Ordering::Relaxed) >= min_frequency)
            .map(|e| (e.key().clone(), e.value().load(Ordering::Relaxed)))
            .collect();

        hot_keys.sort_by(|a, b| b.1.cmp(&a.1));
        hot_keys.into_iter().take(limit).collect()
    }

    /// 获取总访问次数
    pub fn total_accesses(&self) -> u64 {
        self.total_accesses.load(Ordering::Relaxed)
    }

    /// 清理过期条目
    pub fn cleanup(&self, max_age: Duration) {
        let now = Instant::now();

        // 清理旧的最近访问记录
        self.recent_access
            .retain(|_, &mut t| now.duration_since(t) < max_age * 2);

        // 清理旧的访问窗口
        self.access_window
            .retain(|_, accesses| {
                accesses.retain(|&t| now.duration_since(t) < max_age);
                !accesses.is_empty()
            });

        // 清理低频关联
        self.key_correlations
            .retain(|_, corr_map| {
                corr_map.retain(|_, &mut v| v > 0);
                !corr_map.is_empty()
            });
    }
}

impl Default for AccessPatternTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// 预取请求
#[derive(Debug)]
pub struct PrefetchRequest {
    /// 要预取的键
    pub key: String,
    /// 优先级（数值越高越优先）
    pub priority: u8,
    /// 创建时间
    pub created_at: Instant,
}

impl PrefetchRequest {
    pub fn new(key: String, priority: u8) -> Self {
        Self {
            key,
            priority,
            created_at: Instant::now(),
        }
    }
}

/// 自适应预取器
///
/// 监控访问模式，预测并预取数据。
#[derive(Clone)]
pub struct AdaptivePrefetcher {
    /// 访问模式追踪器
    pub tracker: Arc<AccessPatternTracker>,
    /// 预取请求通道
    prefetch_tx: Arc<mpsc::Sender<PrefetchRequest>>,
    /// 正在预取中的键（防止重复预取）
    in_progress: Arc<DashMap<String, Instant>>,
    /// 预取任务句柄
    _prefetch_task: JoinHandle<()>,
    /// 是否启用预取
    enabled: Arc<AtomicU8>,
    /// 预取批量大小
    batch_size: usize,
    /// 预取间隔（毫秒）
    prefetch_interval_ms: u64,
}

impl AdaptivePrefetcher {
    /// 创建新的自适应预取器
    ///
    /// # 参数
    /// * `l2_backend` - L2 后端（用于预取数据）
    /// * `tracker` - 访问模式追踪器
    /// * `batch_size` - 每次预取的批量大小
    /// * `prefetch_interval_ms` - 预取间隔（毫秒）
    pub fn new(
        l2_backend: Arc<dyn L2BackendStrategy>,
        tracker: Arc<AccessPatternTracker>,
        batch_size: usize,
        prefetch_interval_ms: u64,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        let enabled = Arc::new(AtomicU8::new(1)); // 默认启用
        let in_progress = Arc::new(DashMap::new());

        // 创建预取任务
        let prefetch_task = tokio::spawn(Self::prefetch_worker(
            rx,
            l2_backend,
            tracker.clone(),
            in_progress.clone(),
            Arc::clone(&enabled),
            batch_size,
            prefetch_interval_ms,
        ));

        Self {
            tracker,
            prefetch_tx: Arc::new(tx),
            in_progress,
            _prefetch_task: prefetch_task,
            enabled,
            batch_size,
            prefetch_interval_ms,
        }
    }

    /// 预取工作协程
    async fn prefetch_worker(
        mut rx: mpsc::Receiver<PrefetchRequest>,
        l2_backend: Arc<dyn L2BackendStrategy>,
        tracker: Arc<AccessPatternTracker>,
        in_progress: Arc<DashMap<String, Instant>>,
        enabled: Arc<AtomicU8>,
        _batch_size: usize,
        interval_ms: u64,
    ) {
        let interval = Duration::from_millis(interval_ms);

        while let Some(request) = rx.recv().await {
            if enabled.load(Ordering::Relaxed) == 0 {
                continue;
            }

            // 检查是否已在预取中
            if in_progress.contains_key(&request.key) {
                continue;
            }

            // 标记开始预取
            in_progress.insert(request.key.clone(), Instant::now());

            // 执行预取
            match l2_backend.get(&request.key).await {
                Ok(Some(_)) => {
                    debug!("Prefetched key: {}", request.key);
                }
                Ok(None) => {
                    // 键不存在于 L2，不需要预取
                }
                Err(e) => {
                    warn!("Prefetch failed for key {}: {}", request.key, e);
                }
            }

            // 移除预取标记
            in_progress.remove(&request.key);

            // 等待间隔
            tokio::time::sleep(interval).await;
        }
    }

    /// 记录访问并触发预取
    ///
    /// # 参数
    /// * `key` - 被访问的键
    /// * `correlated_keys` - 同时访问的相关键
    pub async fn record_and_prefetch(&self, key: &str, correlated_keys: &[&str]) {
        // 记录访问
        self.tracker.record_access(key, correlated_keys);

        if self.enabled.load(Ordering::Relaxed) == 0 {
            return;
        }

        // 预测并发送预取请求
        let predictions = self.tracker.get_predicted_keys(key, self.batch_size);

        for (pred_key, score) in predictions {
            // 只预取得分高于阈值的键
            if score > 1.0 {
                // 检查是否已在预取中或 L1 中
                if !self.in_progress.contains_key(&pred_key) {
                    let request = PrefetchRequest::new(pred_key, (score.min(255.0) as u8));
                    if let Err(e) = self.prefetch_tx.send(request).await {
                        warn!("Failed to send prefetch request: {}", e);
                    }
                }
            }
        }
    }

    /// 预取指定键
    pub async fn prefetch(&self, key: &str) -> Result<()> {
        if self.enabled.load(Ordering::Relaxed) == 0 {
            return Ok(());
        }

        // 检查是否已在预取中
        if self.in_progress.contains_key(key) {
            return Ok(());
        }

        let request = PrefetchRequest::new(key.to_string(), 128);
        self.prefetch_tx.send(request).await.map_err(|e| {
            crate::error::CacheError::L2Error(format!("Failed to send prefetch request: {}", e))
        })?;

        Ok(())
    }

    /// 批量预取
    pub async fn prefetch_batch(&self, keys: &[&str]) {
        for &key in keys {
            if let Err(e) = self.prefetch(key).await {
                warn!("Batch prefetch failed for key {}: {}", key, e);
            }
        }
    }

    /// 启用/禁用预取
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(if enabled { 1 } else { 0 }, Ordering::Relaxed);
        info!("Adaptive prefetch {}", if enabled { "enabled" } else { "disabled" });
    }

    /// 获取统计信息
    pub fn stats(&self) -> PrefetchStats {
        PrefetchStats {
            total_accesses: self.tracker.total_accesses(),
            hot_keys_count: self.frequency_count(),
            in_progress_count: self.in_progress.len(),
            enabled: self.enabled.load(Ordering::Relaxed) == 1,
        }
    }

    fn frequency_count(&self) -> usize {
        self.tracker.frequency.len()
    }
}

/// 预取统计信息
#[derive(Debug, Clone)]
pub struct PrefetchStats {
    /// 总访问次数
    pub total_accesses: u64,
    /// 热键数量
    pub hot_keys_count: usize,
    /// 正在预取的数量
    pub in_progress_count: usize,
    /// 是否启用
    pub enabled: bool,
}
