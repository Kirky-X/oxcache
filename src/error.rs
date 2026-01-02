//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的错误类型和处理机制。

use thiserror::Error;

/// 缓存系统错误类型枚举
///
/// 定义了缓存系统中可能发生的各种错误类型
#[derive(Error, Debug)]
pub enum CacheError {
    /// 序列化错误
    #[error("Serialization error: {0}. Please check the data format and ensure the serializer is compatible."
    )]
    Serialization(String),

    /// L1缓存操作失败
    #[error("L1 cache operation failed: {0}. This may indicate memory pressure or configuration issues."
    )]
    L1Error(String),

    /// L2缓存操作失败
    #[error("L2 cache operation failed: {0}. Please check Redis connection and server status.")]
    L2Error(String),

    /// 配置错误
    #[error("Configuration error: {0}. Please review your configuration file and ensure all required settings are provided."
    )]
    ConfigError(String),

    /// 配置错误（别名，为了兼容）
    #[error("Configuration error: {0}. Please review your configuration file.")]
    Configuration(String),

    /// 操作不支持
    #[error("Operation not supported: {0}. This feature may not be available for the current cache type."
    )]
    NotSupported(String),

    /// WAL（预写日志）操作失败
    #[error("WAL (Write-Ahead Log) operation failed: {0}. Check disk space and file permissions.")]
    WalError(String),

    /// 数据库错误
    #[error("Database error: {0}. Please check database connectivity and query syntax.")]
    DatabaseError(String),

    /// Redis错误
    #[error("Redis connection failed: {0}. Please ensure Redis server is running and the connection string is correct."
    )]
    RedisError(#[from] redis::RedisError),

    /// IO错误
    #[error("I/O error: {0}. Check file permissions and disk space.")]
    IoError(#[from] std::io::Error),

    /// 后端错误
    #[error("Backend error: {0}. This may be a transient issue, please retry.")]
    BackendError(String),

    /// 超时错误
    #[error("Operation timed out: {0}. Consider increasing the timeout value or check system performance."
    )]
    Timeout(String),

    /// 关闭错误
    #[error("Shutdown error: {0}. Some resources may not have been properly released.")]
    ShutdownError(String),

    /// 键过长错误
    #[error("Key too long: {0}. Maximum key length is {1} bytes.")]
    KeyTooLong(usize, usize),

    /// 值过大错误
    #[error("Value too large: {0}. Maximum value size is {1} bytes.")]
    ValueTooLarge(usize, usize),

    /// 缓冲区已满错误
    #[error("Buffer full: {0}. The batch write buffer has reached capacity. Please retry later or increase buffer size."
    )]
    BufferFull(String),

    /// 无效输入错误
    #[error(
        "Invalid input: {0}. The provided input does not meet the required format or constraints."
    )]
    InvalidInput(String),

    /// 无效键错误
    #[error("Invalid key: {0}. The provided key does not meet the required format or contains forbidden characters.")]
    InvalidKey(String),
}

/// 缓存操作结果类型别名
///
/// 简化错误处理，所有缓存操作都返回此类型
pub type Result<T> = std::result::Result<T, CacheError>;

impl From<sea_orm::DbErr> for CacheError {
    fn from(e: sea_orm::DbErr) -> Self {
        CacheError::DatabaseError(e.to_string())
    }
}
