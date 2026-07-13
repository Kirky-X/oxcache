// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
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
/// use oxcache::error::OxCacheConfigError;
///
/// fn validate_config(config: &CacheConfig) -> OxCacheConfigResult<()> {
///     if config.l1.capacity == 0 {
///         return Err(OxCacheConfigError::InvalidValue {
///             field: "capacity".to_string(),
///             reason: "capacity must be greater than 0".to_string(),
///         });
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug, Error)]
pub enum OxCacheConfigError {
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
#[cfg(feature = "redis")]
pub type OxCacheConfigResult<T> = std::result::Result<T, OxCacheConfigError>;

/// 缓存系统错误类型枚举
///
/// 定义了缓存系统中可能发生的各种错误类型。
/// 所有错误都实现了std::error::Error trait，可以使用?操作符传播。
///
/// # 错误分类
///
/// - **序列化错误** ([`OxCacheError::Serialization`]): 数据序列化/反序列化失败
/// - **后端错误** ([`OxCacheError::BackendError`]): L1/L2缓存后端操作失败
/// - **连接错误** ([`OxCacheError::ConnectionError`]): 网络连接问题
/// - **超时错误** ([`OxCacheError::TimeoutError`]): 操作超时
/// - **数据库错误** ([`OxCacheError::DatabaseError`]): 数据库相关错误
/// - **未找到错误** ([`OxCacheError::NotFound`]): 请求的键不存在
/// - **降级错误** ([`OxCacheError::Degraded`]): 缓存处于降级模式
/// - **操作错误** ([`OxCacheError::Operation`]): 一般操作错误
///
/// # 配置阶段错误
///
/// 配置阶段的错误（如缺少必需字段、无效值等）使用 [`OxCacheConfigError`] 类型，
/// 通过 [`OxCacheConfigResult`] 类型别名返回。
///
/// # 示例
///
/// ```rust,ignore
/// use oxcache::error::{OxCacheError, OxCacheConfigError, OxCacheConfigResult};
///
/// // 运行时错误
/// async fn safe_cache_operation() -> OxCacheResult<String, OxCacheError> {
///     let result = cache.get("key").await?;
///     match result {
///         Some(value) => Ok(value),
///         None => Err(OxCacheError::NotFound("Key not found".to_string()))
///     }
/// }
///
/// // 配置阶段错误
/// fn validate_config(config: &CacheConfig) -> OxCacheConfigResult<()> {
///     if config.capacity == 0 {
///         return Err(OxCacheConfigError::InvalidValue {
///             field: "capacity".to_string(),
///             reason: "must be greater than 0".to_string(),
///         });
///     }
///     Ok(())
/// }
/// ```
#[derive(Error, Debug)]
pub enum OxCacheError {
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
pub type OxCacheResult<T> = std::result::Result<T, OxCacheError>;

impl From<std::io::Error> for OxCacheError {
    fn from(e: std::io::Error) -> Self {
        OxCacheError::IoError(e)
    }
}

// serde_json::Error 转换仅在 serialization/full feature 下可用（serde_json 依赖门控）
#[cfg(any(feature = "serialization", feature = "full"))]
impl From<serde_json::Error> for OxCacheError {
    fn from(e: serde_json::Error) -> Self {
        OxCacheError::Serialization(e.to_string())
    }
}

impl OxCacheError {
    /// 获取错误码
    ///
    /// 返回一个唯一的错误码字符串，便于日志记录和错误追踪。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::OxCacheError;
    ///
    /// let err = OxCacheError::NotFound("key".to_string());
    /// assert_eq!(err.code(), "OXCACHE_001");
    ///
    /// let err = OxCacheError::Connection("failed".to_string());
    /// assert_eq!(err.code(), "OXCACHE_002");
    /// ```
    pub fn code(&self) -> &'static str {
        match self {
            OxCacheError::NotFound(_) => "OXCACHE_001",
            OxCacheError::Connection(_) => "OXCACHE_002",
            OxCacheError::Serialization(_) => "OXCACHE_003",
            OxCacheError::Operation(_) => "OXCACHE_004",
            OxCacheError::Degraded(_) => "OXCACHE_005",
            OxCacheError::L1Error(_) => "OXCACHE_006",
            OxCacheError::L2Error(_) => "OXCACHE_007",
            OxCacheError::NotSupported(_) => "OXCACHE_009",
            OxCacheError::WalError(_) => "OXCACHE_010",
            OxCacheError::DatabaseError(_) => "OXCACHE_011",
            OxCacheError::RedisError(_) => "OXCACHE_012",
            OxCacheError::IoError(_) => "OXCACHE_013",
            OxCacheError::BackendError(_) => "OXCACHE_014",
            OxCacheError::Timeout(_) => "OXCACHE_015",
            OxCacheError::ShutdownError(_) => "OXCACHE_016",
            OxCacheError::KeyTooLong(_, _) => "OXCACHE_017",
            OxCacheError::ValueTooLarge(_, _) => "OXCACHE_018",
            OxCacheError::BufferFull(_) => "OXCACHE_019",
            OxCacheError::InvalidInput(_) => "OXCACHE_020",
            OxCacheError::InvalidKey(_) => "OXCACHE_021",
            OxCacheError::LockError(_) => "OXCACHE_022",
            OxCacheError::ServiceNotFound(_) => "OXCACHE_023",
            OxCacheError::Internal(_) => "OXCACHE_024",
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
    /// use oxcache::error::OxCacheError;
    ///
    /// let err = OxCacheError::Connection("failed".to_string());
    /// assert!(err.is_recoverable());
    ///
    /// let err = OxCacheError::NotFound("key".to_string());
    /// assert!(!err.is_recoverable());
    /// ```
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            OxCacheError::Connection(_)
                | OxCacheError::Timeout(_)
                | OxCacheError::L2Error(_)
                | OxCacheError::BackendError(_)
                | OxCacheError::BufferFull(_)
        )
    }

    /// Check if this error is a "not found" error
    ///
    /// Returns true if the error indicates that a requested key was not found.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::OxCacheError;
    ///
    /// let err = OxCacheError::NotFound("key".to_string());
    /// assert!(err.is_not_found());
    ///
    /// let other_err = OxCacheError::Connection("failed".to_string());
    /// assert!(!other_err.is_not_found());
    /// ```
    pub fn is_not_found(&self) -> bool {
        matches!(self, OxCacheError::NotFound(_))
    }

    /// Check if this error is a connection error
    ///
    /// Returns true if the error indicates a connection-related failure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::OxCacheError;
    ///
    /// let err = OxCacheError::Connection("failed".to_string());
    /// assert!(err.is_connection_error());
    ///
    /// let other_err = OxCacheError::NotFound("key".to_string());
    /// assert!(!other_err.is_connection_error());
    /// ```
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            OxCacheError::Connection(_) | OxCacheError::RedisError(_) | OxCacheError::L2Error(_)
        )
    }

    /// Check if this error is a degraded mode error
    ///
    /// Returns true if the error indicates the cache is operating in degraded mode.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::OxCacheError;
    ///
    /// let err = OxCacheError::Degraded("L2 unavailable".to_string());
    /// assert!(err.is_degraded());
    ///
    /// let other_err = OxCacheError::NotFound("key".to_string());
    /// assert!(!other_err.is_degraded());
    /// ```
    pub fn is_degraded(&self) -> bool {
        matches!(self, OxCacheError::Degraded(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // OxCacheConfigError Display tests
    // ============================================================================

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_missing_field_display() {
        let err = OxCacheConfigError::MissingField("host".to_string());
        assert_eq!(err.to_string(), "Missing required field: host");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_invalid_value_display() {
        let err = OxCacheConfigError::InvalidValue {
            field: "capacity".to_string(),
            reason: "must be > 0".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid value for field 'capacity': must be > 0");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_unsupported_backend_display() {
        let err = OxCacheConfigError::UnsupportedBackend("unknown".to_string());
        assert_eq!(err.to_string(), "Unsupported backend combination: unknown");
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_connection_failed_display() {
        let err = OxCacheConfigError::ConnectionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Connection failed during initialization: timeout");
    }

    // ============================================================================
    // OxCacheError Display tests - all variants
    // ============================================================================

    #[test]
    fn test_cache_error_serialization_display() {
        let err = OxCacheError::Serialization("bad data".to_string());
        let s = err.to_string();
        assert!(s.contains("Serialization error: bad data"));
    }

    #[test]
    fn test_cache_error_operation_display() {
        let err = OxCacheError::Operation("fail".to_string());
        let s = err.to_string();
        assert!(s.contains("Operation failed: fail"));
    }

    #[test]
    fn test_cache_error_connection_display() {
        let err = OxCacheError::Connection("refused".to_string());
        let s = err.to_string();
        assert!(s.contains("Connection error: refused"));
    }

    #[test]
    fn test_cache_error_not_found_display() {
        let err = OxCacheError::NotFound("key1".to_string());
        let s = err.to_string();
        assert!(s.contains("Key not found: key1"));
    }

    #[test]
    fn test_cache_error_degraded_display() {
        let err = OxCacheError::Degraded("L2 down".to_string());
        let s = err.to_string();
        assert!(s.contains("Cache degraded: L2 down"));
    }

    #[test]
    fn test_cache_error_l1_error_display() {
        let err = OxCacheError::L1Error("oom".to_string());
        let s = err.to_string();
        assert!(s.contains("L1 cache operation failed: oom"));
    }

    #[test]
    fn test_cache_error_l2_error_display() {
        let err = OxCacheError::L2Error("redis down".to_string());
        let s = err.to_string();
        assert!(s.contains("L2 cache operation failed: redis down"));
    }

    #[test]
    fn test_cache_error_not_supported_display() {
        let err = OxCacheError::NotSupported("scan".to_string());
        let s = err.to_string();
        assert!(s.contains("Operation not supported: scan"));
    }

    #[test]
    fn test_cache_error_wal_error_display() {
        let err = OxCacheError::WalError("disk full".to_string());
        let s = err.to_string();
        assert!(s.contains("WAL (Write-Ahead Log) operation failed: disk full"));
    }

    #[test]
    fn test_cache_error_database_error_display() {
        let err = OxCacheError::DatabaseError("query failed".to_string());
        let s = err.to_string();
        assert!(s.contains("Database error: query failed"));
    }

    #[test]
    fn test_cache_error_redis_error_display() {
        #[cfg(feature = "redis")]
        {
            let err = OxCacheError::RedisError(redis::RedisError::from(std::io::Error::other("auth failed")));
            let s = err.to_string();
            assert!(s.contains("Redis connection failed"));
        }
        #[cfg(not(feature = "redis"))]
        {
            let err = OxCacheError::RedisError("auth failed".to_string());
            let s = err.to_string();
            assert!(s.contains("Redis connection failed: auth failed"));
        }
    }

    #[test]
    fn test_cache_error_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = OxCacheError::IoError(io_err);
        let s = err.to_string();
        assert!(s.contains("I/O error:"));
    }

    #[test]
    fn test_cache_error_backend_error_display() {
        let err = OxCacheError::BackendError("transient".to_string());
        let s = err.to_string();
        assert!(s.contains("Backend error: transient"));
    }

    #[test]
    fn test_cache_error_timeout_display() {
        let err = OxCacheError::Timeout("5s".to_string());
        let s = err.to_string();
        assert!(s.contains("Operation timed out: 5s"));
    }

    #[test]
    fn test_cache_error_shutdown_error_display() {
        let err = OxCacheError::ShutdownError("leak".to_string());
        let s = err.to_string();
        assert!(s.contains("Shutdown error: leak"));
    }

    #[test]
    fn test_cache_error_key_too_long_display() {
        let err = OxCacheError::KeyTooLong(600, 512);
        let s = err.to_string();
        assert!(s.contains("Key too long: 600. Maximum key length is 512 bytes."));
    }

    #[test]
    fn test_cache_error_value_too_large_display() {
        let err = OxCacheError::ValueTooLarge(2048, 1024);
        let s = err.to_string();
        assert!(s.contains("Value too large: 2048. Maximum value size is 1024 bytes."));
    }

    #[test]
    fn test_cache_error_buffer_full_display() {
        let err = OxCacheError::BufferFull("batch".to_string());
        let s = err.to_string();
        assert!(s.contains("Buffer full: batch"));
    }

    #[test]
    fn test_cache_error_invalid_input_display() {
        let err = OxCacheError::InvalidInput("bad".to_string());
        let s = err.to_string();
        assert!(s.contains("Invalid input: bad"));
    }

    #[test]
    fn test_cache_error_invalid_key_display() {
        let err = OxCacheError::InvalidKey("bad key".to_string());
        let s = err.to_string();
        assert!(s.contains("Invalid key: bad key"));
    }

    #[test]
    fn test_cache_error_lock_error_display() {
        let err = OxCacheError::LockError("poisoned".to_string());
        let s = err.to_string();
        assert!(s.contains("Lock error: poisoned"));
    }

    #[test]
    fn test_cache_error_service_not_found_display() {
        let err = OxCacheError::ServiceNotFound("svc".to_string());
        let s = err.to_string();
        assert!(s.contains("Service not found: svc"));
    }

    #[test]
    fn test_cache_error_internal_display() {
        let err = OxCacheError::Internal("boom".to_string());
        let s = err.to_string();
        assert_eq!(s, "Internal error: boom");
    }

    // ============================================================================
    // From conversions
    // ============================================================================

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let cache_err: OxCacheError = io_err.into();
        assert!(matches!(cache_err, OxCacheError::IoError(_)));
    }

    #[cfg(any(feature = "serialization", feature = "full"))]
    #[test]
    fn test_from_serde_json_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let cache_err: OxCacheError = serde_err.into();
        assert!(matches!(cache_err, OxCacheError::Serialization(_)));
    }

    // ============================================================================
    // code() method tests
    // ============================================================================

    #[test]
    fn test_error_code_not_found() {
        assert_eq!(OxCacheError::NotFound("k".to_string()).code(), "OXCACHE_001");
    }

    #[test]
    fn test_error_code_connection() {
        assert_eq!(OxCacheError::Connection("c".to_string()).code(), "OXCACHE_002");
    }

    #[test]
    fn test_error_code_serialization() {
        assert_eq!(OxCacheError::Serialization("s".to_string()).code(), "OXCACHE_003");
    }

    #[test]
    fn test_error_code_operation() {
        assert_eq!(OxCacheError::Operation("o".to_string()).code(), "OXCACHE_004");
    }

    #[test]
    fn test_error_code_degraded() {
        assert_eq!(OxCacheError::Degraded("d".to_string()).code(), "OXCACHE_005");
    }

    #[test]
    fn test_error_code_l1() {
        assert_eq!(OxCacheError::L1Error("l1".to_string()).code(), "OXCACHE_006");
    }

    #[test]
    fn test_error_code_l2() {
        assert_eq!(OxCacheError::L2Error("l2".to_string()).code(), "OXCACHE_007");
    }

    #[test]
    fn test_error_code_not_supported() {
        assert_eq!(OxCacheError::NotSupported("ns".to_string()).code(), "OXCACHE_009");
    }

    #[test]
    fn test_error_code_wal() {
        assert_eq!(OxCacheError::WalError("w".to_string()).code(), "OXCACHE_010");
    }

    #[test]
    fn test_error_code_database() {
        assert_eq!(OxCacheError::DatabaseError("db".to_string()).code(), "OXCACHE_011");
    }

    #[test]
    fn test_error_code_redis() {
        #[cfg(feature = "redis")]
        {
            let err = OxCacheError::RedisError(redis::RedisError::from(std::io::Error::other("r")));
            assert_eq!(err.code(), "OXCACHE_012");
        }
        #[cfg(not(feature = "redis"))]
        {
            assert_eq!(OxCacheError::RedisError("r".to_string()).code(), "OXCACHE_012");
        }
    }

    #[test]
    fn test_error_code_io() {
        let io_err = std::io::Error::other("x");
        assert_eq!(OxCacheError::IoError(io_err).code(), "OXCACHE_013");
    }

    #[test]
    fn test_error_code_backend() {
        assert_eq!(OxCacheError::BackendError("b".to_string()).code(), "OXCACHE_014");
    }

    #[test]
    fn test_error_code_timeout() {
        assert_eq!(OxCacheError::Timeout("t".to_string()).code(), "OXCACHE_015");
    }

    #[test]
    fn test_error_code_shutdown() {
        assert_eq!(OxCacheError::ShutdownError("s".to_string()).code(), "OXCACHE_016");
    }

    #[test]
    fn test_error_code_key_too_long() {
        assert_eq!(OxCacheError::KeyTooLong(1, 2).code(), "OXCACHE_017");
    }

    #[test]
    fn test_error_code_value_too_large() {
        assert_eq!(OxCacheError::ValueTooLarge(1, 2).code(), "OXCACHE_018");
    }

    #[test]
    fn test_error_code_buffer_full() {
        assert_eq!(OxCacheError::BufferFull("b".to_string()).code(), "OXCACHE_019");
    }

    #[test]
    fn test_error_code_invalid_input() {
        assert_eq!(OxCacheError::InvalidInput("i".to_string()).code(), "OXCACHE_020");
    }

    #[test]
    fn test_error_code_invalid_key() {
        assert_eq!(OxCacheError::InvalidKey("k".to_string()).code(), "OXCACHE_021");
    }

    #[test]
    fn test_error_code_lock_error() {
        assert_eq!(OxCacheError::LockError("l".to_string()).code(), "OXCACHE_022");
    }

    #[test]
    fn test_error_code_service_not_found() {
        assert_eq!(OxCacheError::ServiceNotFound("s".to_string()).code(), "OXCACHE_023");
    }

    #[test]
    fn test_error_code_internal() {
        assert_eq!(OxCacheError::Internal("i".to_string()).code(), "OXCACHE_024");
    }

    // ============================================================================
    // is_recoverable() tests
    // ============================================================================

    #[test]
    fn test_is_recoverable_connection() {
        assert!(OxCacheError::Connection("c".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_timeout() {
        assert!(OxCacheError::Timeout("t".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_l2() {
        assert!(OxCacheError::L2Error("l2".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_backend() {
        assert!(OxCacheError::BackendError("b".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_recoverable_buffer_full() {
        assert!(OxCacheError::BufferFull("b".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_not_found() {
        assert!(!OxCacheError::NotFound("k".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_internal() {
        assert!(!OxCacheError::Internal("i".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_not_recoverable_serialization() {
        assert!(!OxCacheError::Serialization("s".to_string()).is_recoverable());
    }

    // ============================================================================
    // is_not_found() tests
    // ============================================================================

    #[test]
    fn test_is_not_found_true() {
        assert!(OxCacheError::NotFound("key".to_string()).is_not_found());
    }

    #[test]
    fn test_is_not_found_false() {
        assert!(!OxCacheError::Connection("c".to_string()).is_not_found());
    }

    // ============================================================================
    // is_connection_error() tests
    // ============================================================================

    #[test]
    fn test_is_connection_error_connection() {
        assert!(OxCacheError::Connection("c".to_string()).is_connection_error());
    }

    #[test]
    fn test_is_connection_error_redis() {
        #[cfg(feature = "redis")]
        {
            let err = OxCacheError::RedisError(redis::RedisError::from(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "r",
            )));
            assert!(err.is_connection_error());
        }
        #[cfg(not(feature = "redis"))]
        {
            assert!(OxCacheError::RedisError("r".to_string()).is_connection_error());
        }
    }

    #[test]
    fn test_is_connection_error_l2() {
        assert!(OxCacheError::L2Error("l2".to_string()).is_connection_error());
    }

    #[test]
    fn test_is_connection_error_false() {
        assert!(!OxCacheError::NotFound("k".to_string()).is_connection_error());
    }

    // ============================================================================
    // is_degraded() tests
    // ============================================================================

    #[test]
    fn test_is_degraded_true() {
        assert!(OxCacheError::Degraded("d".to_string()).is_degraded());
    }

    #[test]
    fn test_is_degraded_false() {
        assert!(!OxCacheError::NotFound("k".to_string()).is_degraded());
    }

    // ============================================================================
    // Debug trait test
    // ============================================================================

    #[test]
    fn test_cache_error_debug() {
        let err = OxCacheError::NotFound("key".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"));
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_debug() {
        let err = OxCacheConfigError::MissingField("f".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("MissingField"));
    }

    // ============================================================================
    // std::error::Error trait test
    // ============================================================================

    #[test]
    fn test_cache_error_is_std_error() {
        let err = OxCacheError::NotFound("key".to_string());
        let _: &dyn std::error::Error = &err;
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_cache_config_error_is_std_error() {
        let err = OxCacheConfigError::MissingField("f".to_string());
        let _: &dyn std::error::Error = &err;
    }
}
