//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 数据库分区管理模块
//!
//! 提供 SQLite 的按月分区功能

use serde::{Deserialize, Serialize};

pub mod common;
pub mod connection_string;
pub mod sqlite;

#[cfg(any(feature = "database", test))]
pub use connection_string::normalize_connection_string_with_redaction;
pub(crate) use connection_string::{is_test_connection_string, normalize_connection_string};
pub mod partition;
pub use partition::{PartitionConfig, PartitionInfo, PartitionManager, PartitionStrategy};

/// 数据库类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseType {
    SQLite,
}

impl DatabaseType {
    /// 从URL字符串解析数据库类型
    pub fn from_url(_url: &str) -> Self {
        DatabaseType::SQLite
    }
}
