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

// pub use super::{PartitionConfig, PartitionInfo};

/// 分区配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionConfig {
    /// 是否启用分区
    pub enabled: bool,
    /// 分区策略
    pub strategy: PartitionStrategy,
    /// 保留月数
    pub retention_months: u32,
    /// 预创建月数
    pub precreate_months: u32,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: PartitionStrategy::Monthly,
            retention_months: 12,
            precreate_months: 3,
        }
    }
}

/// 分区信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    /// 分区名称
    pub name: String,
    /// 表名
    pub table_name: String,
    /// 开始日期
    pub start_date: DateTime<Utc>,
    /// 结束日期
    pub end_date: DateTime<Utc>,
    /// 是否已创建
    pub created: bool,
}

impl PartitionInfo {
    /// 创建新的分区信息（默认按月）
    pub fn new(date: DateTime<Utc>, table_name: &str) -> Result<Self> {
        use chrono::{Datelike, TimeZone};
        use crate::error::CacheError;
        let year = date.year();
        let month = date.month();
        
        // Start of month
        let start_date = Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0)
            .single()
            .ok_or_else(|| CacheError::DatabaseError(format!("Invalid start date for {}-{}", year, month)))?;
        
        // End of month (start of next month)
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let end_date = Utc.with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
            .single()
            .ok_or_else(|| CacheError::DatabaseError(format!("Invalid end date for {}-{}", next_year, next_month)))?;
        
        let name = format!("{}_{:04}_{:02}", table_name, year, month);
        
        Ok(Self {
            name,
            table_name: table_name.to_string(),
            start_date,
            end_date,
            created: false,
        })
    }
}

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
    async fn ensure_partition_exists(&self, date: DateTime<Utc>, table_name: &str) -> Result<String>;

    /// 预创建未来分区
    async fn precreate_partitions(&self, table_name: &str, months_ahead: u32) -> Result<()> {
        use chrono::{Datelike, TimeZone};
        let now = Utc::now();
        
        for i in 0..=months_ahead {
            let mut target_month = now.month() + i;
            let mut target_year = now.year();
            
            while target_month > 12 {
                target_month -= 12;
                target_year += 1;
            }
            
            // Construct date for 1st of the target month
            // We use single_res to handle potential ambiguity (though unlikely for 1st of month in UTC)
            if let Some(target_date) = Utc.with_ymd_and_hms(target_year, target_month, 1, 0, 0, 0).single() {
                self.ensure_partition_exists(target_date, table_name).await?;
            } else {
                tracing::warn!("Failed to construct date for partition: {}-{}", target_year, target_month);
            }
        }
        Ok(())
    }

    /// 清理过期分区
    async fn cleanup_old_partitions(
        &self,
        table_name: &str,
        cutoff_date: DateTime<Utc>,
    ) -> Result<u32> {
        let partitions = self.get_partitions(table_name).await?;
        let mut dropped_count = 0;
        
        for partition in partitions {
            // If partition end date is before cutoff date, it's expired
            if partition.end_date < cutoff_date {
                self.drop_partition(table_name, &partition.name).await?;
                dropped_count += 1;
            }
        }
        Ok(dropped_count)
    }

    /// 获取配置
    fn get_config(&self) -> PartitionConfig;

    /// 提取基础表名（去除分区后缀）
    fn extract_base_table(&self, table_name: &str) -> String {
        let parts: Vec<&str> = table_name.split('_').collect();
        if parts.len() >= 3 {
            // 检查最后两部分是否为年份和月份
            let year = parts[parts.len() - 2].parse::<i32>();
            let month = parts[parts.len() - 1].parse::<u32>();
            
            if year.is_ok() && month.is_ok() {
                // 是分区表，移除后缀
                return parts[..parts.len() - 2].join("_");
            }
        }
        table_name.to_string()
    }

    /// 生成分区名称
    fn generate_partition_name(&self, date: &DateTime<Utc>, prefix: &str) -> String {
        use chrono::Datelike;
        format!("{}_{:04}_{:02}", prefix, date.year(), date.month())
    }

    /// 生成分区表名
    fn generate_partition_table_name(&self, table_name: &str, date: &DateTime<Utc>) -> String {
        self.generate_partition_name(date, table_name)
    }

    /// 解析分区日期
    fn parse_partition_date(&self, table_name: &str) -> Option<DateTime<Utc>> {
        use chrono::{TimeZone, Utc};
        let parts: Vec<&str> = table_name.split('_').collect();
        if parts.len() >= 3 {
            let year_str = parts[parts.len() - 2];
            let month_str = parts[parts.len() - 1];
            
            if let (Ok(year), Ok(month)) = (year_str.parse::<i32>(), month_str.parse::<u32>()) {
                return Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).single();
            }
        }
        None
    }

    /// 获取下个月的第一天
    fn get_next_month_first_day(&self, date: &DateTime<Utc>) -> DateTime<Utc> {
        use chrono::{Datelike, TimeZone, Utc};
        let (year, month) = if date.month() == 12 {
            (date.year() + 1, 1)
        } else {
            (date.year(), date.month() + 1)
        };
        Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0)
            .single()
            .unwrap_or_else(Utc::now) // Fallback to now if invalid
    }
}
