//! 分区管理器trait定义

use crate::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::PartitionInfo;

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

    /// 确保分区存在（如果不存在则创建）
    async fn ensure_partition_exists(
        &self,
        date: DateTime<Utc>,
        table_name: &str,
    ) -> Result<String>;

    /// 预创建未来分区
    async fn precreate_partitions(&self, table_name: &str, months_ahead: u32) -> Result<()>;

    /// 清理过期分区
    async fn cleanup_old_partitions(
        &self,
        table_name: &str,
        retention_months: u32,
    ) -> Result<usize> {
        let partitions = self.get_partitions(table_name).await?;
        let cutoff_date = Utc::now() - chrono::Duration::days((retention_months * 30) as i64);

        println!(
            "DEBUG: cleanup_old_partitions - cutoff_date: {}",
            cutoff_date
        );
        println!("DEBUG: found {} partitions", partitions.len());
        for (i, partition) in partitions.iter().enumerate() {
            println!(
                "DEBUG: partition[{}] - name: {}, end_date: {}, will_delete: {}",
                i,
                partition.name,
                partition.end_date,
                partition.end_date < cutoff_date
            );
        }

        let mut dropped_count = 0;
        for partition in partitions {
            if partition.end_date < cutoff_date {
                println!("DEBUG: dropping partition: {}", partition.name);
                self.drop_partition(table_name, &partition.name).await?;
                dropped_count += 1;
            }
        }

        Ok(dropped_count)
    }
}
