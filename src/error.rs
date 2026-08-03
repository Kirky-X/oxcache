// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 该模块定义了缓存系统的错误类型和处理机制。

use std::fmt;

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
#[derive(Debug)]
pub enum OxCacheConfigError {
    /// Missing required configuration field
    MissingField(String),

    /// Invalid value for a configuration field
    InvalidValue { field: String, reason: String },

    /// Unsupported backend combination
    UnsupportedBackend(String),

    /// Connection failed during initialization
    ConnectionFailed(String),
}

#[cfg(feature = "redis")]
impl fmt::Display for OxCacheConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let locale = crate::i18n::get_default_locale();
        f.write_str(&self.localized_message(&locale))
    }
}

#[cfg(feature = "redis")]
impl std::error::Error for OxCacheConfigError {}

#[cfg(feature = "redis")]
impl OxCacheConfigError {
    /// Return the i18n message ID for this config error variant.
    pub fn message_id(&self) -> &'static str {
        match self {
            OxCacheConfigError::MissingField(_) => crate::i18n::messages::MSG_CFG_MISSING_FIELD,
            OxCacheConfigError::InvalidValue { .. } => crate::i18n::messages::MSG_CFG_INVALID_VALUE,
            OxCacheConfigError::UnsupportedBackend(_) => crate::i18n::messages::MSG_CFG_UNSUPPORTED_BACKEND,
            OxCacheConfigError::ConnectionFailed(_) => crate::i18n::messages::MSG_CFG_CONNECTION_FAILED,
        }
    }

    /// Render a locale-aware config error message.
    pub fn localized_message(&self, locale: &str) -> String {
        let params: Vec<(&str, String)> = match self {
            OxCacheConfigError::MissingField(f) => vec![("field", f.clone())],
            OxCacheConfigError::InvalidValue { field, reason } => vec![
                ("field", field.clone()),
                ("reason", reason.clone()),
            ],
            OxCacheConfigError::UnsupportedBackend(d) => vec![("detail", d.clone())],
            OxCacheConfigError::ConnectionFailed(d) => vec![("detail", d.clone())],
        };
        let template = crate::i18n::messages::lookup(locale, self.message_id())
            .unwrap_or(self.message_id());
        let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        crate::i18n::messages::format_template(template, &borrowed)
    }
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
/// - **连接错误** ([`OxCacheError::Connection`]): 网络连接问题
/// - **超时错误** ([`OxCacheError::Timeout`]): 操作超时
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
#[derive(Debug)]
pub enum OxCacheError {
    /// 序列化错误
    ///
    /// 通常发生在：
    /// - 尝试序列化不支持的数据类型
    /// - 序列化器配置不兼容
    /// - 数据在传输过程中被损坏
    Serialization(String),

    /// 操作错误
    ///
    /// 一般的缓存操作失败
    Operation(String),

    /// 连接错误
    ///
    /// 网络连接失败或断开
    Connection(String),

    /// 未找到错误
    ///
    /// 请求的键在缓存中不存在
    NotFound(String),

    /// 降级错误
    ///
    /// 缓存处于降级模式，某些功能不可用
    Degraded(String),

    /// L1缓存操作失败
    ///
    /// L1缓存是进程内的内存缓存，可能因以下原因失败：
    /// - 内存不足导致缓存被逐出
    /// - 缓存容量达到上限
    /// - 缓存项过期
    L1Error(String),

    /// L2缓存操作失败
    L2Error(String),

    /// 操作不支持
    NotSupported(String),

    /// WAL（预写日志）操作失败
    WalError(String),

    /// 数据库错误
    DatabaseError(String),

    /// Redis错误
    #[cfg(feature = "redis")]
    RedisError(redis::RedisError),

    /// Redis错误（占位符，当 redis feature 禁用时）
    #[cfg(not(feature = "redis"))]
    RedisError(String),

    /// IO错误
    IoError(std::io::Error),

    /// 后端错误
    BackendError(String),

    /// 超时错误
    Timeout(String),

    /// 关闭错误
    ShutdownError(String),

    /// 键过长错误
    KeyTooLong(usize, usize),

    /// 值过大错误
    ValueTooLarge(usize, usize),

    /// 缓冲区已满错误
    BufferFull(String),

    /// 无效输入错误
    InvalidInput(String),

    /// 无效键错误
    InvalidKey(String),

    /// 锁错误
    ///
    /// 互斥锁获取失败，通常发生在锁被毒害（之前的持有者 panicked）
    LockError(String),

    /// 服务配置未找到错误
    ///
    /// 请求的服务配置在 UnifiedConfig 中不存在
    ServiceNotFound(String),

    /// 内部错误
    ///
    /// 内部组件错误，通常表示不可恢复的内部状态异常
    Internal(String),
}

impl fmt::Display for OxCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let locale = crate::i18n::get_default_locale();
        f.write_str(&self.localized_message(&locale))
    }
}

impl std::error::Error for OxCacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(feature = "redis")]
            OxCacheError::RedisError(e) => Some(e),
            OxCacheError::IoError(e) => Some(e),
            _ => None,
        }
    }
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

#[cfg(feature = "redis")]
impl From<redis::RedisError> for OxCacheError {
    fn from(e: redis::RedisError) -> Self {
        OxCacheError::RedisError(e)
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
                | OxCacheError::RedisError(_)
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

    /// Return the i18n message ID for this error variant.
    ///
    /// Message IDs are stable string keys used to look up localized templates
    /// in the message catalog. They follow the pattern `"error.<variant>"`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use oxcache::error::OxCacheError;
    ///
    /// let err = OxCacheError::NotFound("key".to_string());
    /// assert_eq!(err.message_id(), "error.not_found");
    /// ```
    pub fn message_id(&self) -> &'static str {
        match self {
            OxCacheError::Serialization(_) => crate::i18n::messages::MSG_ERR_SERIALIZATION,
            OxCacheError::Operation(_) => crate::i18n::messages::MSG_ERR_OPERATION,
            OxCacheError::Connection(_) => crate::i18n::messages::MSG_ERR_CONNECTION,
            OxCacheError::NotFound(_) => crate::i18n::messages::MSG_ERR_NOT_FOUND,
            OxCacheError::Degraded(_) => crate::i18n::messages::MSG_ERR_DEGRADED,
            OxCacheError::L1Error(_) => crate::i18n::messages::MSG_ERR_L1,
            OxCacheError::L2Error(_) => crate::i18n::messages::MSG_ERR_L2,
            OxCacheError::NotSupported(_) => crate::i18n::messages::MSG_ERR_NOT_SUPPORTED,
            OxCacheError::WalError(_) => crate::i18n::messages::MSG_ERR_WAL,
            OxCacheError::DatabaseError(_) => crate::i18n::messages::MSG_ERR_DATABASE,
            OxCacheError::RedisError(_) => crate::i18n::messages::MSG_ERR_REDIS,
            OxCacheError::IoError(_) => crate::i18n::messages::MSG_ERR_IO,
            OxCacheError::BackendError(_) => crate::i18n::messages::MSG_ERR_BACKEND,
            OxCacheError::Timeout(_) => crate::i18n::messages::MSG_ERR_TIMEOUT,
            OxCacheError::ShutdownError(_) => crate::i18n::messages::MSG_ERR_SHUTDOWN,
            OxCacheError::KeyTooLong(_, _) => crate::i18n::messages::MSG_ERR_KEY_TOO_LONG,
            OxCacheError::ValueTooLarge(_, _) => crate::i18n::messages::MSG_ERR_VALUE_TOO_LARGE,
            OxCacheError::BufferFull(_) => crate::i18n::messages::MSG_ERR_BUFFER_FULL,
            OxCacheError::InvalidInput(_) => crate::i18n::messages::MSG_ERR_INVALID_INPUT,
            OxCacheError::InvalidKey(_) => crate::i18n::messages::MSG_ERR_INVALID_KEY,
            OxCacheError::LockError(_) => crate::i18n::messages::MSG_ERR_LOCK,
            OxCacheError::ServiceNotFound(_) => crate::i18n::messages::MSG_ERR_SERVICE_NOT_FOUND,
            OxCacheError::Internal(_) => crate::i18n::messages::MSG_ERR_INTERNAL,
        }
    }

    /// Render a locale-aware error message using the ICU4X message catalog.
    ///
    /// `Display` uses the global default locale (set via
    /// [`set_default_locale`](crate::i18n::set_default_locale)); this method
    /// lets you render in an explicit `locale` without changing the global.
    ///
    /// Falls back to English for unsupported locales, and to the raw message ID
    /// if the catalog has no entry for this error.
    pub fn localized_message(&self, locale: &str) -> String {
        let params = self.message_params();
        let template = crate::i18n::messages::lookup(locale, self.message_id())
            .unwrap_or(self.message_id());
        let borrowed: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        crate::i18n::messages::format_template(template, &borrowed)
    }

    /// Extract `(key, value)` parameters for message template substitution.
    fn message_params(&self) -> Vec<(&str, String)> {
        match self {
            OxCacheError::KeyTooLong(actual, max) => vec![
                ("actual", actual.to_string()),
                ("max", max.to_string()),
            ],
            OxCacheError::ValueTooLarge(actual, max) => vec![
                ("actual", actual.to_string()),
                ("max", max.to_string()),
            ],
            OxCacheError::Serialization(d)
            | OxCacheError::Operation(d)
            | OxCacheError::Connection(d)
            | OxCacheError::NotFound(d)
            | OxCacheError::Degraded(d)
            | OxCacheError::L1Error(d)
            | OxCacheError::L2Error(d)
            | OxCacheError::NotSupported(d)
            | OxCacheError::WalError(d)
            | OxCacheError::DatabaseError(d)
            | OxCacheError::BackendError(d)
            | OxCacheError::Timeout(d)
            | OxCacheError::ShutdownError(d)
            | OxCacheError::BufferFull(d)
            | OxCacheError::InvalidInput(d)
            | OxCacheError::InvalidKey(d)
            | OxCacheError::LockError(d)
            | OxCacheError::ServiceNotFound(d)
            | OxCacheError::Internal(d) => vec![("detail", d.clone())],
            OxCacheError::RedisError(e) => vec![("detail", e.to_string())],
            OxCacheError::IoError(e) => vec![("detail", e.to_string())],
        }
    }
}

#[cfg(test)]
mod tests;
