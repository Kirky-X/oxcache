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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_from_url_sqlite() {
        assert_eq!(DatabaseType::from_url("sqlite:./test.db"), DatabaseType::SQLite);
    }

    #[test]
    fn test_database_type_from_url_redis() {
        // Currently always returns SQLite
        assert_eq!(DatabaseType::from_url("redis://localhost"), DatabaseType::SQLite);
    }

    #[test]
    fn test_database_type_from_url_empty() {
        assert_eq!(DatabaseType::from_url(""), DatabaseType::SQLite);
    }

    #[test]
    fn test_database_type_debug() {
        let debug = format!("{:?}", DatabaseType::SQLite);
        assert!(debug.contains("SQLite"));
    }

    #[test]
    fn test_database_type_clone_eq() {
        let t = DatabaseType::SQLite;
        let cloned = t;
        assert_eq!(t, cloned);
    }

    #[test]
    fn test_database_type_serialize() {
        let serialized = serde_json::to_string(&DatabaseType::SQLite).unwrap();
        assert!(serialized.contains("SQLite"));
    }
}
