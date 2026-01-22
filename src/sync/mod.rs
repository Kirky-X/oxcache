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
// 当 batch-write 功能禁用时的空实现（使用 panic! 提供清晰的错误消息）
// ============================================================================

/// 批量写入配置（需要 batch-write feature）
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

/// 批量操作类型（需要 batch-write feature）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub enum BatchOperation {
    #[default]
    Set,
    Delete,
}

/// 批量写入器（需要 batch-write feature）
///
/// # Feature Requirement
///
/// 此类型需要 `batch-write` feature 启用。
///
/// 启用方法：
/// ```toml
/// [dependencies]
/// oxcache = { version = "0.2", features = ["batch-write"] }
/// ```
///
/// 或使用 `full` feature 启用所有功能：
/// ```toml
/// [dependencies]
/// oxcache = { version = "0.2", features = ["full"] }
/// ```
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct BatchWriter;

#[cfg(not(feature = "batch-write"))]
impl BatchWriter {
    pub fn new(
        _service_name: String,
        _l2: std::sync::Arc<crate::backend::l2::L2Backend>,
        _config: BatchWriterConfig,
    ) -> Self {
        panic!(
            "BatchWriter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub fn new_with_default_config(
        _service_name: String,
        _l2: std::sync::Arc<crate::backend::l2::L2Backend>,
    ) -> Self {
        panic!(
            "BatchWriter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn shutdown(&self) {
        panic!(
            "BatchWriter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn start(&self) {
        panic!(
            "BatchWriter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn enqueue(&self, _key: String, _value: Vec<u8>, _ttl: Option<u64>) -> Result<()> {
        panic!(
            "BatchWriter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn enqueue_delete(&self, _key: String) -> Result<()> {
        panic!(
            "BatchWriter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn enqueue_operation(&self, _operation: BatchOperation) -> Result<()> {
        panic!(
            "BatchWriter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }
}

/// 缓存失效配置（需要 batch-write feature）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct InvalidationConfig;

/// 缓存失效器（需要 batch-write feature）
///
/// # Feature Requirement
///
/// 此类型需要 `batch-write` feature 启用。
///
/// 启用方法：
/// ```toml
/// [dependencies]
/// oxcache = { version = "0.2", features = ["batch-write"] }
/// ```
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct CacheInvalidator;

#[cfg(not(feature = "batch-write"))]
impl CacheInvalidator {
    pub fn new(_config: InvalidationConfig) -> Self {
        panic!(
            "CacheInvalidator requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn invalidate(&self, _key: &str) -> Result<()> {
        panic!(
            "CacheInvalidator requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn invalidate_pattern(&self, _pattern: &str) -> Result<()> {
        panic!(
            "CacheInvalidator requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn invalidate_all(&self) -> Result<()> {
        panic!(
            "CacheInvalidator requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }
}

/// 缓存提升配置（需要 batch-write feature）
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct PromotionConfig;

/// 缓存提升器（需要 batch-write feature）
///
/// # Feature Requirement
///
/// 此类型需要 `batch-write` feature 启用。
///
/// 启用方法：
/// ```toml
/// [dependencies]
/// oxcache = { version = "0.2", features = ["batch-write"] }
/// ```
#[cfg(not(feature = "batch-write"))]
#[derive(Debug, Clone, Default)]
pub struct CachePromoter;

#[cfg(not(feature = "batch-write"))]
impl CachePromoter {
    pub fn new(_config: PromotionConfig) -> Self {
        panic!(
            "CachePromoter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn promote(&self, _key: &str) -> Result<()> {
        panic!(
            "CachePromoter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }

    pub async fn promote_many(&self, _keys: &[String]) -> Result<()> {
        panic!(
            "CachePromoter requires the 'batch-write' feature to be enabled. \
             \n\n\
             Solution 1: Enable the batch-write feature:\n\
               oxcache = {{ version = \"0.2\", features = [\"batch-write\"] }}\n\
             \n\
             Solution 2: Enable all features:\n\
               oxcache = {{ version = \"0.2\", features = [\"full\"] }}"
        );
    }
}
