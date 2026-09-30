// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存智能预热（`warmup` feature，默认关闭）
//!
//! 从调用方提供的 [`WarmupLoader`] 端口拉取热 key 集合，异步回填到
//! [`ChainCache`](crate::cache::ChainCache)：复用链式批量读
//! ([`ChainCache::iter_entries`](crate::cache::ChainCache::iter_entries))
//! 从低层晋升仅持 key 的条目，复用链式写入把值落到全部后端。
//!
//! - **去重**：loader 返回的重复 key 以首次出现为准（顺序即优先级）；
//! - **限额**：直供值单值大小上限（默认与序列化写入面 `MAX_JSON_SIZE`
//!   同口径）与单轮条目数上限（默认 100_000）防止 loader 无界集合放大
//!   回填规模，超限条目显性丢弃并计入报告（均可经 builder 调整，置 0
//!   解除）；
//! - **并发**：回填写入按 `concurrency` 有界并发（Semaphore 滑窗恒定在飞
//!   数），报告携带实测峰值便于验证边界；
//! - **晋升 TTL 透传**：仅 key 条目晋升时查链上剩余 TTL 随回填写入，
//!   不把带过期语义的源条目重置为链默认 TTL；
//! - **容错**：loader 端口失败显性返回 `Err`；单条回填失败不中断整批，
//!   以 [`WarmupReport`] 的计数与明细显性呈现。
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use oxcache::cache::{ChainCache, warmup::{Warmup, WarmupEntry, WarmupLoader}};
//! use oxcache::error::OxCacheResult;
//!
//! struct ForecastLoader;
//!
//! #[async_trait::async_trait]
//! impl WarmupLoader for ForecastLoader {
//!     async fn load_hot_keys(&self) -> OxCacheResult<Vec<WarmupEntry>> {
//!         Ok(vec![WarmupEntry {
//!             key: "product:42".into(),
//!             value: Some(b"payload".to_vec()), // 直供值；None = 仅 key，从链低层晋升
//!             ttl: None,
//!         }])
//!     }
//! }
//!
//! # async fn demo() -> OxCacheResult<()> {
//! let chain = Arc::new(
//!     ChainCache::builder()
//!         .backend(oxcache::backend::MokaMemoryBackend::new())
//!         .build(),
//! );
//! let warmup = Warmup::builder(chain).concurrency(8).build();
//! let report = warmup.run(Arc::new(ForecastLoader)).await?;
//! assert_eq!(report.failed, 0);
//! # Ok(())
//! # }
//! # tokio::runtime::Runtime::new().unwrap().block_on(demo()).unwrap();
//! ```

// 具体实现拆分至独立文件（mod.rs 仅承载端口与类型面）
mod runner;

#[cfg(test)]
mod tests;

pub use self::runner::{Warmup, WarmupBuilder};

use crate::error::OxCacheResult;
use async_trait::async_trait;
use std::time::Duration;

/// 预热条目：loader 提供的单条热 key
#[derive(Debug, Clone)]
pub struct WarmupEntry {
    /// 缓存键
    pub key: String,
    /// 预热值：
    /// - `Some(bytes)`：loader 直供值（源数据/快照预热路径）；
    /// - `None`：仅声明热 key，值经链式批量读从低层后端晋升
    ///   （分层晋升路径，低层也无值时计为 missing）。
    pub value: Option<Vec<u8>>,
    /// 条目 TTL（`None` 走链默认 TTL）
    pub ttl: Option<Duration>,
}

/// 预热 loader 端口（对象安全，可 `Arc<dyn WarmupLoader>` 注入）
///
/// 实现来源由调用方决定：业务预测、`HotKeyTracker` 快照、上游分析系统
/// 导出等。返回错误会显性中止本轮预热（缓存本体不受影响）。
#[async_trait]
pub trait WarmupLoader: Send + Sync + 'static {
    /// 返回本轮预热的热 key 集合（重复 key 由预热器去重）
    async fn load_hot_keys(&self) -> OxCacheResult<Vec<WarmupEntry>>;
}

/// 预热执行报告：成功/失败全部以计数与明细显性呈现，不吞任何结果
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WarmupReport {
    /// loader 返回条目数（去重前）
    pub loader_entries: usize,
    /// 去重后条目数
    pub deduped: usize,
    /// 直供值条目成功写入链的数量
    pub warmed: usize,
    /// 仅 key 条目经批量读从低层命中并回填的数量
    pub promoted: usize,
    /// 仅 key 条目链上无值（低层也未命中）的数量
    pub missing: usize,
    /// 回填写入失败的数量（明细见 [`WarmupReport::failures`]）
    pub failed: usize,
    /// 写入失败明细 `(key, 错误信息)`，与 `failed` 一一对应
    pub failures: Vec<(String, String)>,
    /// 回填写入的实测并发峰值（≤ 配置的 `concurrency`）
    pub peak_concurrency: usize,
    /// 直供值超过单值大小上限而显性丢弃的数量（上限 0 = 不限，不产生丢弃）
    pub dropped_value_too_large: usize,
    /// 去重后条目数超过单轮条目数上限而显性丢弃的数量（上限 0 = 不限）
    pub dropped_over_entry_cap: usize,
}
