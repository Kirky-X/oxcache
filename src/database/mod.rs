//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 数据库分区管理模块
//!
//! 提供PostgreSQL和MySQL的按月分区功能

use crate::error::{CacheError, Result};
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod common;
mod connection_string;
// mod mysql;
// mod partition;
// mod postgresql;
// mod sqlite;

pub(crate) use connection_string::{
    ensure_database_directory, extract_sqlite_path, get_recommended_connection_string,
    is_test_connection_string, normalize_connection_string, validate_connection_string, DbType,
    ParsedConnectionString, ValidationResult,
};
// 分区管理模块暂时禁用，待后续修复
// pub mod partition;
// pub(crate) use partition::PartitionStrategy;

/// 数据库类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite, // 用于测试和开发
}

impl DatabaseType {
    /// 从URL字符串解析数据库类型
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            DatabaseType::PostgreSQL
        } else if url.starts_with("mysql://") {
            DatabaseType::MySQL
        } else {
            DatabaseType::SQLite
        }
    }
}
