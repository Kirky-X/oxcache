//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的错误类型和处理机制。

use thiserror::Error;

#[cfg(feature = "redis")]
/// Configuration error type for cache initialization
///
/// This error type is used during the configuration phase (factory functions, builders)
/// and is separate from runtime errors. It represents errors that occur when setting up
/// a cache instance, such as invalid configuration values or missing required fields.
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::error::CacheConfigError;
///
/// fn validate_config(config: &CacheConfig) -> ConfigResult<()> {
///     if config.l1.capacity == 0 {
///         return Err(CacheConfigError::InvalidValue {
///             field: "capacity".to_string(),
///             reason: "capacity must be greater than 0".to_string(),
///         });
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug, Error)]
pub enum CacheConfigError {
    /// Missing required configuration field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid value for a configuration field
    #[error("Invalid value for field '{field}': {reason}")]
    InvalidValue { field: String, reason: String },

    /// Unsupported backend combination
    #[error("Unsupported backend combination: {0}")]
    UnsupportedBackend(String),

    /// Connection failed during initialization
    #[error("Connection failed during initialization: {0}")]
    ConnectionFailed(String),
}

/// Result type for configuration operations
pub type ConfigResult<T> = std::result::Result<T, CacheConfigError>;

/// 缓存系统错误类型枚举
///
/// 定义了缓存系统中可能发生的各种错误类型。
/// 所有错误都实现了std::error::Error trait，可以使用?操作符传播。
///
/// # 错误分类
///
/// - **序列化错误** ([`CacheError::Serialization`]): 数据序列化/反序列化失败
/// - **后端错误** ([`CacheError::BackendError`]): L1/L2缓存后端操作失败
/// - **连接错误** ([`CacheError::ConnectionError`]): �络连接问题
/// - **超时错误** ([`CacheError::TimeoutError`]): 操作超时
/// - **数据库错误** ([`CacheError::DatabaseError`]): 数据库相关错误
/// - **未找到错误** ([`CacheError::NotFound`]): 请求的键不存在
/// - **降级错误** ([`CacheError::Degraded`]): 缓存处于降级模式
/// - **操作错误** ([`CacheError::Operation`]): 一般操作错误
///
/// # 配置阶段错误
///
/// 配置阶段的错误（如缺少必需字段、无效值等）使用 [`CacheConfigError`] 类型，
/// 通过 [`ConfigResult`] 类型别名返回。
///
/// # 示例
///
/// ```rust,ignore
/// use oxcache::error::{CacheError, CacheConfigError, ConfigResult};
///
/// // 运行时错误
/// async fn safe_cache_operation() -> Result<String, CacheError> {
///     let result = cache.get("key").await?;
///     match result {
///         Some(value) => Ok(value),
///         None => Err(CacheError::NotFound("Key not found".to_string()))
///     }
/// }
///
/// // 配置阶段错误
/// fn validate_config(config: &CacheConfig) -> ConfigResult<()> {
///     if config.capacity == 0 {
///         return Err(CacheConfigError::InvalidValue {
///             field: "capacity".to_string(),
///             reason: "must be greater than 0".to_string(),
///         });
///     }
///     Ok(())
/// }
/// ```
#[derive(Error, Debug)]
pub enum CacheError {
    /// 序列化错误
    ///
    /// 通常发生在：
    /// - 尝试序列化不支持的数据类型
    /// - 序列化器配置不兼容
    /// - 数据在传输过程中被损坏
    #[error("Serialization error: {0}. Please check the data format and ensure the serializer is compatible.")]
    Serialization(String),

    /// 操作错误
    ///
    /// 一般的缓存操作失败
    #[error("Operation failed: {0}. Please retry or check your request.")]
    Operation(String),

    /// 连接错误
    ///
    /// 网络连接失败或断开
    #[error("Connection error: {0}. Please check network connectivity and server availability.")]
    Connection(String),

    /// 未找到错误
    ///
    /// 请求的键在缓存中不存在
    #[error("Key not found: {0}. The requested key does not exist in the cache.")]
    NotFound(String),

    /// 降级错误
    ///
    /// 缓存处于降级模式，某些功能不可用
    #[error("Cache degraded: {0}. The cache is operating in degraded mode with limited functionality.")]
    Degraded(String),

    /// L1缓存操作失败
    ///
    /// L1缓存是进程内的内存缓存，可能因以下原因失败：
    /// - 内存不足导致缓存被逐出
    /// - 缓存容量达到上限
    /// - 缓存项过期
    #[error("L1 cache operation failed: {0}. This may indicate memory pressure or configuration issues.")]
    L1Error(String),

    /// L2缓存操作失败
    #[error("L2 cache operation failed: {0}. Please check Redis connection and server status.")]
    L2Error(String),

    /// 操作不支持
    #[error("Operation not supported: {0}. This feature may not be available for the current cache type.")]
    NotSupported(String),

    /// WAL（预写日志）操作失败
    #[error("WAL (Write-Ahead Log) operation failed: {0}. Check disk space and file permissions.")]
    WalError(String),

    /// 数据库错误
    #[error("Database error: {0}. Please check database connectivity and query syntax.")]
    DatabaseError(String),

    /// Redis错误
    #[cfg(feature = "redis")]
    #[error("Redis connection failed: {0}")]
    RedisError(#[from] redis::RedisError),

    /// Redis错误（占位符，当 redis feature 禁用时）
    #[cfg(not(feature = "redis"))]
    #[error("Redis connection failed: {0}. Please enable redis feature and ensure Redis server is running.")]
    RedisError(String),

    /// IO错误
    #[error("I/O error: {0}. Check file permissions and disk space.")]
    IoError(std::io::Error),

    /// 后端错误
    #[error("Backend error: {0}. This may be a transient issue, please retry.")]
    BackendError(String),

    /// 超时错误
    #[error("Operation timed out: {0}. Consider increasing the timeout value or check system performance.")]
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
    #[error(
        "Buffer full: {0}. The batch write buffer has reached capacity. Please retry later or increase buffer size."
    )]
    BufferFull(String),

    /// 无效输入错误
    #[error("Invalid input: {0}. The provided input does not meet the required format or constraints.")]
    InvalidInput(String),

    /// 无效键错误
    #[error("Invalid key: {0}. The provided key does not meet the required format or contains forbidden characters.")]
    InvalidKey(String),

    /// 锁错误
    ///
    /// 互斥锁获取失败，通常发生在锁被毒害（之前的持有者 panicked）
    #[error("Lock error: {0}. The lock may have been poisoned by a previous panic.")]
    LockError(String),

    /// 服务配置未找到错误
    ///
    /// 请求的服务配置在 UnifiedConfig 中不存在
    #[error("Service not found: {0}. The requested service configuration does not exist in the UnifiedConfig.")]
    ServiceNotFound(String),

    /// 内部错误
    ///
    /// 内部组件错误，通常表示不可恢复的内部状态异常
    #[error("Internal error: {0}")]
    Internal(String),
}

/// 缓存操作结果类型别名
///
/// 简化错误处理，所有缓存操作都返回此类型
pub type Result<T> = std::result::Result<T, CacheError>;

#[cfg(feature = "database")]
impl From<sea_orm::DbErr> for CacheError {
    fn from(e: sea_orm::DbErr) -> Self {
        CacheError::DatabaseError(e.to_string())
    }
}

impl From<std::io::Error> for CacheError {
    fn from(e: std::io::Error) -> Self {
        CacheError::IoError(e)
    }
}

impl CacheError {
    /// 获取错误码
    ///
    /// 返回一个唯一的错误码字符串，便于日志记录和错误追踪。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::NotFound("key".to_string());
    /// assert_eq!(err.code(), "CACHE_001");
    ///
    /// let err = CacheError::Connection("failed".to_string());
    /// assert_eq!(err.code(), "CACHE_002");
    /// ```
    pub fn code(&self) -> &'static str {
        match self {
            CacheError::NotFound(_) => "CACHE_001",
            CacheError::Connection(_) => "CACHE_002",
            CacheError::Serialization(_) => "CACHE_003",
            CacheError::Operation(_) => "CACHE_004",
            CacheError::Degraded(_) => "CACHE_005",
            CacheError::L1Error(_) => "CACHE_006",
            CacheError::L2Error(_) => "CACHE_007",
            CacheError::NotSupported(_) => "CACHE_009",
            CacheError::WalError(_) => "CACHE_010",
            CacheError::DatabaseError(_) => "CACHE_011",
            CacheError::RedisError(_) => "CACHE_012",
            CacheError::IoError(_) => "CACHE_013",
            CacheError::BackendError(_) => "CACHE_014",
            CacheError::Timeout(_) => "CACHE_015",
            CacheError::ShutdownError(_) => "CACHE_016",
            CacheError::KeyTooLong(_, _) => "CACHE_017",
            CacheError::ValueTooLarge(_, _) => "CACHE_018",
            CacheError::BufferFull(_) => "CACHE_019",
            CacheError::InvalidInput(_) => "CACHE_020",
            CacheError::InvalidKey(_) => "CACHE_021",
            CacheError::LockError(_) => "CACHE_022",
            CacheError::ServiceNotFound(_) => "CACHE_023",
            CacheError::Internal(_) => "CACHE_024",
        }
    }

    /// 检查错误是否可恢复
    ///
    /// 返回 true 表示错误可能是暂时的，可以重试。
    ///
    /// `Internal` 错误代表内部状态损坏（如锁中毒、数据不一致），不可恢复。
    /// 调用者不应重试此类错误，而应停止操作或重新初始化缓存实例。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::Connection("failed".to_string());
    /// assert!(err.is_recoverable());
    ///
    /// let err = CacheError::NotFound("key".to_string());
    /// assert!(!err.is_recoverable());
    /// ```
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CacheError::Connection(_)
                | CacheError::Timeout(_)
                | CacheError::L2Error(_)
                | CacheError::BackendError(_)
                | CacheError::BufferFull(_)
        )
    }

    /// Check if this error is a "not found" error
    ///
    /// Returns true if the error indicates that a requested key was not found.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::NotFound("key".to_string());
    /// assert!(err.is_not_found());
    ///
    /// let other_err = CacheError::Connection("failed".to_string());
    /// assert!(!other_err.is_not_found());
    /// ```
    pub fn is_not_found(&self) -> bool {
        matches!(self, CacheError::NotFound(_))
    }

    /// Check if this error is a connection error
    ///
    /// Returns true if the error indicates a connection-related failure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::Connection("failed".to_string());
    /// assert!(err.is_connection_error());
    ///
    /// let other_err = CacheError::NotFound("key".to_string());
    /// assert!(!other_err.is_connection_error());
    /// ```
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            CacheError::Connection(_) | CacheError::RedisError(_) | CacheError::L2Error(_)
        )
    }

    /// Check if this error is a degraded mode error
    ///
    /// Returns true if the error indicates the cache is operating in degraded mode.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::CacheError;
    ///
    /// let err = CacheError::Degraded("L2 unavailable".to_string());
    /// assert!(err.is_degraded());
    ///
    /// let other_err = CacheError::NotFound("key".to_string());
    /// assert!(!other_err.is_degraded());
    /// ```
    pub fn is_degraded(&self) -> bool {
        matches!(self, CacheError::Degraded(_))
    }
}
