//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 原子计数器集合

use std::sync::atomic::AtomicU64;

/// 原子计数器集合
///
/// 用于高频指标的并发安全计数
#[derive(Debug)]
pub struct AtomicCounters {
    /// L1缓存命中次数
    pub l1_get_hits: AtomicU64,
    /// L1缓存未命中次数
    pub l1_get_misses: AtomicU64,
    /// L2缓存命中次数
    pub l2_get_hits: AtomicU64,
    /// L2缓存未命中次数
    pub l2_get_misses: AtomicU64,
    /// L1缓存设置次数
    pub l1_set_total: AtomicU64,
    /// L2缓存设置次数
    pub l2_set_total: AtomicU64,
    /// L1缓存删除次数
    pub l1_delete_total: AtomicU64,
    /// L2缓存删除次数
    pub l2_delete_total: AtomicU64,
    /// 总操作次数
    pub total_operations: AtomicU64,
    /// L1 缓存项数量
    pub l1_items: AtomicU64,
    /// L1 缓存容量使用（字节）
    pub l1_capacity_used: AtomicU64,
    /// 预取操作次数
    pub prefetch_total: AtomicU64,
    /// 压缩操作次数
    pub compression_total: AtomicU64,
    /// 压缩节省的字节数
    pub compression_bytes_saved: AtomicU64,
}

impl Default for AtomicCounters {
    fn default() -> Self {
        Self {
            l1_get_hits: AtomicU64::new(0),
            l1_get_misses: AtomicU64::new(0),
            l2_get_hits: AtomicU64::new(0),
            l2_get_misses: AtomicU64::new(0),
            l1_set_total: AtomicU64::new(0),
            l2_set_total: AtomicU64::new(0),
            l1_delete_total: AtomicU64::new(0),
            l2_delete_total: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
            l1_items: AtomicU64::new(0),
            l1_capacity_used: AtomicU64::new(0),
            prefetch_total: AtomicU64::new(0),
            compression_total: AtomicU64::new(0),
            compression_bytes_saved: AtomicU64::new(0),
        }
    }
}
