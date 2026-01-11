//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 分区管理器trait定义
//!

use crate::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

pub use super::{PartitionConfig, PartitionInfo};

/// 分区策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// 按月分区
    Monthly,
    /// 按范围分区（自定义）
    Range,
}

/// 分区管理器trait
#[async_trait]
pub trait PartitionManager: Send + Sync {
    /// 初始化分区表
    async fn initialize_table(&self, table_name: &str, schema: &str) -> Result<()>;

    /// 创建分区
    async fn create_partition(&self, partition: &PartitionInfo) -> Result<()>;

    /// 获取所有分区
    async fn get_partitions(&self, table_name: &str) -> Result<Vec<PartitionInfo>>;

    /// 删除分区
    async fn drop_partition(&self, table_name: &str, partition_name: &str) -> Result<()>;

    /// 确保分区存在
    async fn ensure_partition_exists(&self, date: DateTime<Utc>, table_name: &str) -> Result<()>;

    /// 预创建未来分区
    async fn precreate_partitions(&self, table_name: &str, months_ahead: u32) -> Result<()>;

    /// 清理过期分区
    async fn cleanup_old_partitions(
        &self,
        table_name: &str,
        cutoff_date: DateTime<Utc>,
    ) -> Result<u32>;

    /// 获取配置
    fn get_config(&self) -> PartitionConfig;
}
