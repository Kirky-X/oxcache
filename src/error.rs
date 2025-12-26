//! Copyright (c) 2025, Kirky.X
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
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// L1缓存操作失败
    #[error("L1 operation failed: {0}")]
    L1Error(String),

    /// L2缓存操作失败
    #[error("L2 operation failed: {0}")]
    L2Error(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// 配置错误（别名，为了兼容）
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// 操作不支持
    #[error("Operation not supported: {0}")]
    NotSupported(String),

    /// WAL（预写日志）操作失败
    #[error("WAL operation failed: {0}")]
    WalError(String),

    /// 数据库错误
    #[error("Database error: {0}")]
    DbError(#[from] rusqlite::Error),

    /// Sea-ORM数据库错误
    #[error("Sea-ORM error: {0}")]
    SeaOrmError(#[from] sea_orm::DbErr),

    /// 数据库连接错误
    #[error("Database connection error: {0}")]
    DatabaseError(String),

    /// Redis错误
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    /// IO错误
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// 后端错误
    #[error("Backend error: {0}")]
    BackendError(String),

    /// 超时错误
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// 关闭错误
    #[error("Shutdown error: {0}")]
    ShutdownError(String),
}

/// 缓存操作结果类型别名
///
/// 简化错误处理，所有缓存操作都返回此类型
pub type Result<T> = std::result::Result<T, CacheError>;
