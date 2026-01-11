//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的同步机制，包括批量写入、失效和提升功能。
//! 通过 `batch-write` feature 控制启用/禁用

#[cfg(feature = "batch-write")]
pub mod batch_writer;
#[cfg(feature = "batch-write")]
pub mod common;
#[cfg(feature = "batch-write")]
pub mod invalidation;
#[cfg(feature = "batch-write")]
pub mod optimized_batch_writer;
#[cfg(feature = "batch-write")]
pub mod promotion;
#[cfg(feature = "batch-write")]
pub mod warmup;

#[cfg(not(feature = "batch-write"))]
pub(crate) mod batch_writer;
#[cfg(not(feature = "batch-write"))]
pub(crate) mod common;
#[cfg(not(feature = "batch-write"))]
pub(crate) mod invalidation;
#[cfg(not(feature = "batch-write"))]
pub(crate) mod optimized_batch_writer;
#[cfg(not(feature = "batch-write"))]
pub(crate) mod promotion;

// warmup 模块始终可用（不依赖 batch-write）

// ============================================================================
// 当 batch-write 功能禁用时的空实现
// ============================================================================

#[cfg(not(feature = "batch-write"))]
use crate::error::Result;
#[cfg(not(feature = "batch-write"))]
use std::sync::Arc;

/// 批量写入配置（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct BatchWriterConfig {
    pub max_buffer_size: usize,
    pub max_batch_size: usize,
    pub flush_interval_ms: u64,
}

#[cfg(not(feature = "batch-write"))]
impl BatchWriterConfig {
    pub fn new(max_buffer_size: usize, max_batch_size: usize, flush_interval_ms: u64) -> Self {
        Self {
            max_buffer_size,
            max_batch_size,
            flush_interval_ms,
        }
    }
}

/// 批量操作类型（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone)]
pub enum BatchOperation {
    Set {
        key: String,
        value: Vec<u8>,
        ttl: Option<u64>,
    },
    Delete {
        key: String,
    },
}

/// 批量写入器（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone)]
pub struct BatchWriter;

#[cfg(not(feature = "batch-write"))]
impl BatchWriter {
    pub fn new(_service_name: String, _l2: Arc<L2Backend>, _config: BatchWriterConfig) -> Self {
        Self
    }

    pub fn new_with_default_config(_service_name: String, _l2: Arc<L2Backend>) -> Self {
        Self
    }

    pub async fn shutdown(&self) {}

    pub async fn start(&self) {}

    pub async fn enqueue(&self, _key: String, _value: Vec<u8>, _ttl: Option<u64>) -> Result<()> {
        Ok(())
    }

    pub async fn enqueue_delete(&self, _key: String) -> Result<()> {
        Ok(())
    }

    pub async fn enqueue_operation(&self, _operation: BatchOperation) -> Result<()> {
        Ok(())
    }
}

/// 缓存失效配置（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct InvalidationConfig;

/// 缓存失效器（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct CacheInvalidator;

#[cfg(not(feature = "batch-write"))]
impl CacheInvalidator {
    pub fn new(_config: InvalidationConfig) -> Self {
        Self
    }

    pub async fn invalidate(&self, _key: &str) -> Result<()> {
        Ok(())
    }

    pub async fn invalidate_pattern(&self, _pattern: &str) -> Result<()> {
        Ok(())
    }

    pub async fn invalidate_all(&self) -> Result<()> {
        Ok(())
    }
}

/// 缓存提升配置（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct PromotionConfig;

/// 缓存提升器（空实现）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct CachePromoter;

#[cfg(not(feature = "batch-write"))]
impl CachePromoter {
    pub fn new(_config: PromotionConfig) -> Self {
        Self
    }

    pub async fn promote(&self, _key: &str) -> Result<()> {
        Ok(())
    }

    pub async fn promote_many(&self, _keys: &[String]) -> Result<()> {
        Ok(())
    }
}

// 重新导出需要的类型（当功能禁用时提供类型定义）
#[cfg(not(feature = "batch-write"))]
use crate::backend::l2::L2Backend;
