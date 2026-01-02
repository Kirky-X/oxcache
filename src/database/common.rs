//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了数据库分区管理的公共工具函数。

use super::{PartitionConfig, PartitionInfo, PartitionManager};
use crate::error::Result;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use futures::Future;
use std::pin::Pin;

/// 数据库分区管理的公共工具函数
pub trait PartitionCommon {
    /// 计算分区保留截止日期
    fn calculate_cutoff_date(&self, retention_months: u32) -> DateTime<Utc> {
        Utc::now() - chrono::Duration::days((retention_months * 30) as i64)
    }

    /// 获取分区的基础表名（移除日期后缀）
    fn extract_base_table(&self, table_name: &str) -> String {
        if table_name.contains("_y") && table_name.contains("m") {
            // Format: table_prefix_y2023m12
            table_name
                .split("_y")
                .next()
                .unwrap_or(table_name)
                .to_string()
        } else if table_name.contains("_") {
            // Format: table_prefix_2023_12
            table_name
                .split("_")
                .take_while(|part| !part.chars().all(|c| c.is_ascii_digit() || c == 'm'))
                .collect::<Vec<_>>()
                .join("_")
        } else {
            // 没有日期格式，使用原始表名
            table_name.to_string()
        }
    }

    /// 生成分区名称
    fn generate_partition_name(&self, date: &DateTime<Utc>, prefix: &str) -> String {
        format!("{}{}_{:02}", prefix, date.year(), date.month())
    }

    /// 生成分区表名
    fn generate_partition_table_name(&self, table_prefix: &str, date: &DateTime<Utc>) -> String {
        format!("{}_y{}m{:02}", table_prefix, date.year(), date.month())
    }

    /// 获取下一个月的第一天
    fn get_next_month_first_day(&self, date: &DateTime<Utc>) -> DateTime<Utc> {
        if date.month() == 12 {
            Utc.with_ymd_and_hms(date.year() + 1, 1, 1, 0, 0, 0)
                .single()
                .expect("January 1st should be a valid date")
        } else {
            Utc.with_ymd_and_hms(date.year(), date.month() + 1, 1, 0, 0, 0)
                .single()
                .expect("First day of month should be a valid date")
        }
    }

    /// 获取当前配置的保留月数
    fn get_retention_months(&self, config: &PartitionConfig, param_retention: u32) -> u32 {
        config.retention_months.unwrap_or(param_retention)
    }

    /// 解析分区表名获取日期信息
    fn parse_partition_date(&self, table_name: &str) -> Option<DateTime<Utc>> {
        if let Some(y_pos) = table_name.rfind("_y") {
            if let Some(m_pos) = table_name[y_pos + 2..].find("m") {
                let year_str = &table_name[y_pos + 2..y_pos + 2 + m_pos];
                let month_str = &table_name[y_pos + 2 + m_pos + 1..];

                if let (Ok(year), Ok(month)) = (year_str.parse::<i32>(), month_str.parse::<u32>()) {
                    return Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).single();
                }
            }
        }
        None
    }
}

/// 默认实现
impl<T> PartitionCommon for T where T: Sized {}

/// 预创建分区的通用实现
pub async fn common_precreate_partitions<'a, M, F>(
    manager: &'a M,
    table_name: &'a str,
    months_ahead: u32,
    _config: &'a PartitionConfig,
    ensure_partition: F,
) -> Result<()>
where
    M: PartitionCommon + ?Sized + 'a,
    F: for<'b> Fn(
        &'b M,
        DateTime<Utc>,
        &'b str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'b>>,
{
    let now = Utc::now();

    // 预创建未来几个月的分区
    for i in 1..=months_ahead {
        let future_date = now + chrono::Duration::days((i * 30) as i64);
        ensure_partition(manager, future_date, table_name).await?;
    }

    Ok(())
}

/// 清理过期分区的通用实现
pub async fn common_cleanup_old_partitions<'a, M, F, G>(
    manager: &'a M,
    table_name: &'a str,
    retention_months: u32,
    config: &'a PartitionConfig,
    get_partitions: F,
    drop_partition: G,
) -> Result<usize>
where
    M: PartitionCommon + ?Sized + 'a,
    F: for<'b> Fn(
        &'b M,
        &'b str,
    )
        -> Pin<Box<dyn futures::Future<Output = Result<Vec<PartitionInfo>>> + Send + 'b>>,
    G: for<'b> Fn(
        &'b M,
        &'b str,
        &'b str,
    ) -> Pin<Box<dyn futures::Future<Output = Result<()>> + Send + 'b>>,
{
    // 如果配置中指定了保留月数，则使用配置的，否则使用参数传入的
    let retention = config.retention_months.unwrap_or(retention_months);

    // 获取所有分区
    let partitions = get_partitions(manager, table_name).await?;
    let cutoff_date = manager.calculate_cutoff_date(retention);

    let mut dropped_count = 0;
    for partition in partitions {
        if partition.end_date < cutoff_date {
            drop_partition(manager, table_name, &partition.name).await?;
            dropped_count += 1;
        }
    }

    Ok(dropped_count)
}

/// 分区管理器扩展trait，用于通用实现
pub trait PartitionManagerExt: PartitionCommon + PartitionManager {
    /// 预创建未来分区（使用通用实现）
    fn precreate_partitions(
        &self,
        table_name: &str,
        months_ahead: u32,
    ) -> impl futures::Future<Output = Result<()>> + Send {
        let manager = self;
        let table_name = table_name.to_string();
        async move {
            common_precreate_partitions(
                manager,
                &table_name,
                months_ahead,
                manager.get_config(),
                |manager, date, table| {
                    Box::pin(PartitionManager::ensure_partition_exists(
                        manager, date, table,
                    ))
                },
            )
            .await
        }
    }

    /// 清理过期分区（使用通用实现）
    fn cleanup_old_partitions(
        &self,
        table_name: &str,
        retention_months: u32,
    ) -> impl futures::Future<Output = Result<usize>> + Send {
        let manager = self;
        let table_name = table_name.to_string();
        async move {
            common_cleanup_old_partitions(
                manager,
                &table_name,
                retention_months,
                manager.get_config(),
                |manager, table| Box::pin(manager.get_partitions(table)),
                |manager, table, partition| Box::pin(manager.drop_partition(table, partition)),
            )
            .await
        }
    }

    /// 确保分区存在
    fn ensure_partition_exists(
        &self,
        date: DateTime<Utc>,
        table_name: &str,
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    /// 获取配置
    fn get_config(&self) -> &PartitionConfig;
}
