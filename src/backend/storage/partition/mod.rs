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
        use crate::error::CacheError;
        use chrono::{Datelike, TimeZone};
        let year = date.year();
        let month = date.month();

        // Start of month
        let start_date = Utc
            .with_ymd_and_hms(year, month, 1, 0, 0, 0)
            .single()
            .ok_or_else(|| CacheError::DatabaseError(format!("Invalid start date for {}-{}", year, month)))?;

        // End of month (start of next month)
        let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
        let end_date = Utc
            .with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
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
            }
            // 如果日期构造失败，跳过该分区
        }
        Ok(())
    }

    /// 清理过期分区
    async fn cleanup_old_partitions(&self, table_name: &str, cutoff_date: DateTime<Utc>) -> Result<u32> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike};

    struct TestManager;
    #[async_trait]
    impl PartitionManager for TestManager {
        async fn initialize_table(&self, _table_name: &str, _schema: &str) -> Result<()> {
            Ok(())
        }
        async fn create_partition(&self, _partition: &PartitionInfo) -> Result<()> {
            Ok(())
        }
        async fn get_partitions(&self, _table_name: &str) -> Result<Vec<PartitionInfo>> {
            Ok(vec![])
        }
        async fn drop_partition(&self, _table_name: &str, _partition_name: &str) -> Result<()> {
            Ok(())
        }
        async fn ensure_partition_exists(&self, _date: DateTime<Utc>, _table_name: &str) -> Result<String> {
            Ok(String::new())
        }
        fn get_config(&self) -> PartitionConfig {
            PartitionConfig::default()
        }
    }

    #[test]
    fn test_partition_config_default() {
        let config = PartitionConfig::default();
        assert!(config.enabled);
        assert_eq!(config.strategy, PartitionStrategy::Monthly);
        assert_eq!(config.retention_months, 12);
        assert_eq!(config.precreate_months, 3);
    }

    #[test]
    fn test_partition_config_custom() {
        let config = PartitionConfig {
            enabled: false,
            strategy: PartitionStrategy::Range,
            retention_months: 6,
            precreate_months: 1,
        };
        assert!(!config.enabled);
        assert_eq!(config.strategy, PartitionStrategy::Range);
    }

    #[test]
    fn test_partition_config_clone_debug() {
        let config = PartitionConfig::default();
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
        let debug = format!("{:?}", config);
        assert!(debug.contains("PartitionConfig"));
    }

    #[test]
    fn test_partition_strategy_equality() {
        assert_eq!(PartitionStrategy::Monthly, PartitionStrategy::Monthly);
        assert_ne!(PartitionStrategy::Monthly, PartitionStrategy::Range);
    }

    #[test]
    fn test_partition_strategy_debug() {
        assert_eq!(format!("{:?}", PartitionStrategy::Monthly), "Monthly");
        assert_eq!(format!("{:?}", PartitionStrategy::Range), "Range");
    }

    #[test]
    fn test_partition_strategy_serialize() {
        let s = PartitionStrategy::Monthly;
        let serialized = serde_json::to_string(&s).unwrap();
        assert!(serialized.contains("Monthly"));
    }

    #[test]
    fn test_partition_info_new() {
        let date = Utc.with_ymd_and_hms(2024, 3, 15, 10, 30, 0).single().unwrap();
        let info = PartitionInfo::new(date, "cache").unwrap();
        assert_eq!(info.name, "cache_2024_03");
        assert_eq!(info.table_name, "cache");
        assert_eq!(info.start_date.year(), 2024);
        assert_eq!(info.start_date.month(), 3);
        assert_eq!(info.start_date.day(), 1);
        assert_eq!(info.start_date.hour(), 0);
        assert!(!info.created);
    }

    #[test]
    fn test_partition_info_new_december() {
        let date = Utc.with_ymd_and_hms(2024, 12, 25, 0, 0, 0).single().unwrap();
        let info = PartitionInfo::new(date, "data").unwrap();
        assert_eq!(info.name, "data_2024_12");
        assert_eq!(info.end_date.year(), 2025);
        assert_eq!(info.end_date.month(), 1);
    }

    #[test]
    fn test_partition_info_new_january() {
        let date = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).single().unwrap();
        let info = PartitionInfo::new(date, "logs").unwrap();
        assert_eq!(info.start_date.month(), 1);
        assert_eq!(info.end_date.month(), 2);
    }

    #[test]
    fn test_partition_info_clone_debug() {
        let date = Utc.with_ymd_and_hms(2024, 5, 10, 0, 0, 0).single().unwrap();
        let info = PartitionInfo::new(date, "test").unwrap();
        let cloned = info.clone();
        assert_eq!(cloned.name, info.name);
        let debug = format!("{:?}", info);
        assert!(debug.contains("PartitionInfo"));
    }

    #[test]
    fn test_partition_info_serialize_deserialize() {
        let date = Utc.with_ymd_and_hms(2024, 7, 20, 0, 0, 0).single().unwrap();
        let info = PartitionInfo::new(date, "cache").unwrap();
        let serialized = serde_json::to_string(&info).unwrap();
        let deserialized: PartitionInfo = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, info.name);
    }

    #[test]
    fn test_extract_base_table_partition_format() {
        let manager = TestManager;
        assert_eq!(manager.extract_base_table("cache_2024_03"), "cache");
        assert_eq!(manager.extract_base_table("my_table_2023_12"), "my_table");
        assert_eq!(manager.extract_base_table("data_2025_01"), "data");
    }

    #[test]
    fn test_extract_base_table_non_partition() {
        let manager = TestManager;
        assert_eq!(manager.extract_base_table("simple_table"), "simple_table");
        assert_eq!(manager.extract_base_table("cache"), "cache");
    }

    #[test]
    fn test_extract_base_table_multi_part_name() {
        let manager = TestManager;
        assert_eq!(manager.extract_base_table("my_app_cache_2024_06"), "my_app_cache");
    }

    #[test]
    fn test_generate_partition_name() {
        let manager = TestManager;
        let date = Utc.with_ymd_and_hms(2024, 3, 15, 0, 0, 0).single().unwrap();
        assert_eq!(manager.generate_partition_name(&date, "cache"), "cache_2024_03");
        assert_eq!(manager.generate_partition_name(&date, "data"), "data_2024_03");
    }

    #[test]
    fn test_generate_partition_name_single_digit_month() {
        let manager = TestManager;
        let date = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).single().unwrap();
        assert_eq!(manager.generate_partition_name(&date, "cache"), "cache_2024_01");
    }

    #[test]
    fn test_generate_partition_table_name() {
        let manager = TestManager;
        let date = Utc.with_ymd_and_hms(2024, 11, 10, 0, 0, 0).single().unwrap();
        assert_eq!(manager.generate_partition_table_name("cache", &date), "cache_2024_11");
    }

    #[test]
    fn test_parse_partition_date_valid() {
        let manager = TestManager;
        let result = manager.parse_partition_date("cache_2024_03");
        assert!(result.is_some());
        let date = result.unwrap();
        assert_eq!(date.year(), 2024);
        assert_eq!(date.month(), 3);
    }

    #[test]
    fn test_parse_partition_date_invalid() {
        let manager = TestManager;
        assert!(manager.parse_partition_date("simple").is_none());
        assert!(manager.parse_partition_date("table_abc").is_none());
        assert!(manager.parse_partition_date("cache_2024").is_none());
    }

    #[test]
    fn test_get_next_month_first_day_normal() {
        let manager = TestManager;
        let date = Utc.with_ymd_and_hms(2024, 3, 15, 10, 30, 0).single().unwrap();
        let next = manager.get_next_month_first_day(&date);
        assert_eq!(next.year(), 2024);
        assert_eq!(next.month(), 4);
        assert_eq!(next.day(), 1);
    }

    #[test]
    fn test_get_next_month_first_day_december() {
        let manager = TestManager;
        let date = Utc.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).single().unwrap();
        let next = manager.get_next_month_first_day(&date);
        assert_eq!(next.year(), 2025);
        assert_eq!(next.month(), 1);
    }

    #[test]
    fn test_get_config_returns_config() {
        let manager = TestManager;
        let config = manager.get_config();
        assert!(config.enabled);
        assert_eq!(config.retention_months, 12);
    }

    #[tokio::test]
    async fn test_default_methods_initialize_table() {
        let manager = TestManager;
        assert!(manager
            .initialize_table("cache", "CREATE TABLE IF NOT EXISTS test (id INTEGER)")
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_default_methods_create_partition() {
        let manager = TestManager;
        let date = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).single().unwrap();
        let info = PartitionInfo::new(date, "cache").unwrap();
        assert!(manager.create_partition(&info).await.is_ok());
    }

    #[tokio::test]
    async fn test_default_methods_get_partitions() {
        let manager = TestManager;
        let partitions = manager.get_partitions("cache").await.unwrap();
        assert!(partitions.is_empty());
    }

    #[tokio::test]
    async fn test_default_methods_drop_partition() {
        let manager = TestManager;
        assert!(manager.drop_partition("cache", "cache_2024_03").await.is_ok());
    }

    #[tokio::test]
    async fn test_default_methods_ensure_partition_exists() {
        let manager = TestManager;
        let date = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).single().unwrap();
        let result = manager.ensure_partition_exists(date, "cache").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_precreate_partitions_zero() {
        let manager = TestManager;
        let result = manager.precreate_partitions("cache", 0).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_precreate_partitions_multiple() {
        let manager = TestManager;
        let result = manager.precreate_partitions("events", 2).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_old_partitions_empty() {
        let manager = TestManager;
        let cutoff = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).single().unwrap();
        let dropped = manager.cleanup_old_partitions("cache", cutoff).await.unwrap();
        assert_eq!(dropped, 0);
    }
}
