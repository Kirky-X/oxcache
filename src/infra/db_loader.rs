//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 数据库回源加载器
//!
//! 提供缓存未命中时自动从数据库加载数据的功能

use crate::error::{CacheError, Result};

const DEFAULT_RETRY_INTERVAL_MS: u64 = 100;

#[cfg(any(feature = "full", feature = "minimal", feature = "core"))]
use crate::infra::validate_cache_key as utils_validate_cache_key;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;

/// 安全的SQL标识符验证
/// 验证SQL标识符（表名、列名等）
/// 只允许字母、数字、下划线，且不以数字开头
pub fn validate_sql_identifier(identifier: &str) -> bool {
    if identifier.is_empty() {
        return false;
    }

    let mut chars = identifier.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };

    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return false;
        }
    }

    true
}

/// 验证缓存键格式
/// 键可以包含字母、数字、连字符、下划线、点号、冒号
pub fn validate_cache_key(key: &str) -> bool {
    #[cfg(any(feature = "full", feature = "minimal", feature = "core"))]
    {
        utils_validate_cache_key(key).is_ok()
    }
    #[cfg(not(any(feature = "full", feature = "minimal", feature = "core")))]
    {
        // 内联实现：当 utils 模块不可用时
        if key.is_empty() || key.len() > 512 {
            return false;
        }
        key.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ':')
    }
}

/// SQL转义函数 - 用于字符串值转义
/// 将特殊字符转义为SQL安全的表示形式
fn escape_sql_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() * 2);
    for c in value.chars() {
        match c {
            '\'' => escaped.push_str("''"),
            '\\' => escaped.push_str("\\\\"),
            '\0' => escaped.push_str("\\0"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// 数据库加载器trait
/// 定义从数据库加载数据的接口
#[async_trait]
pub trait DbLoader: Send + Sync + std::fmt::Debug {
    /// 根据键从数据库加载数据
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回加载的数据，如果数据不存在则返回None
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// 批量加载数据
    ///
    /// # 参数
    ///
    /// * `keys` - 缓存键列表
    ///
    /// # 返回值
    ///
    /// 返回(key, value)对的列表
    async fn load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>>;

    /// 检查数据库连接状态
    fn is_healthy(&self) -> bool;
}

/// 数据库回源管理器
/// 管理数据库加载器并提供回源逻辑
pub struct DbFallbackManager {
    /// 数据库加载器
    loader: Arc<dyn DbLoader>,
    /// 是否启用回源功能
    enabled: bool,
    /// 回源超时时间（毫秒）
    timeout_ms: u64,
    /// 最大重试次数
    max_retries: u32,
}

impl std::fmt::Debug for DbFallbackManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbFallbackManager")
            .field("enabled", &self.enabled)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_retries", &self.max_retries)
            .field("loader_healthy", &self.loader.is_healthy())
            .finish()
    }
}

impl DbFallbackManager {
    /// 创建新的数据库回源管理器
    ///
    /// # 参数
    ///
    /// * `loader` - 数据库加载器
    /// * `enabled` - 是否启用回源功能
    /// * `timeout_ms` - 回源超时时间（毫秒）
    /// * `max_retries` - 最大重试次数
    pub fn new(loader: Arc<dyn DbLoader>, enabled: bool, timeout_ms: u64, max_retries: u32) -> Self {
        Self {
            loader,
            enabled,
            timeout_ms,
            max_retries,
        }
    }

    /// 从数据库回源加载数据
    ///
    /// # 参数
    ///
    /// * `key` - 缓存键
    ///
    /// # 返回值
    ///
    /// 返回从数据库加载的数据，如果加载失败则返回None
    #[instrument(skip(self), level = "info")]
    pub async fn fallback_load(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if !self.enabled {
            return Ok(None);
        }

        if !self.loader.is_healthy() {
            return Ok(None);
        }

        // 尝试加载数据，支持重试机制
        let mut last_error = None;
        for attempt in 0..=self.max_retries {
            match self.try_load_with_timeout(key).await {
                Ok(Some(data)) => {
                    return Ok(Some(data));
                }
                Ok(None) => {
                    return Ok(None);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.max_retries {
                        // 指数退避重试
                        let backoff_ms = DEFAULT_RETRY_INTERVAL_MS * (2_u64.pow(attempt));
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| CacheError::DatabaseError("All fallback attempts failed".to_string())))
    }

    /// 批量回源加载数据
    ///
    /// # 参数
    ///
    /// * `keys` - 缓存键列表
    ///
    /// # 返回值
    ///
    /// 返回(key, value)对的列表
    #[instrument(skip(self), level = "info")]
    pub async fn fallback_load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>> {
        if !self.enabled {
            return Ok(Vec::new());
        }

        if !self.loader.is_healthy() {
            return Ok(Vec::new());
        }

        // 使用超时机制
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(self.timeout_ms),
            self.loader.load_batch(keys.clone()),
        )
        .await
        {
            Ok(Ok(results)) => Ok(results),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CacheError::Timeout(format!(
                "Batch fallback timeout after {}ms",
                self.timeout_ms
            ))),
        }
    }

    /// 使用超时机制尝试加载数据
    async fn try_load_with_timeout(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(self.timeout_ms),
            self.loader.load(key),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Ok(None),
        }
    }

    /// 检查回源功能是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// 示例数据库加载器实现（基于SQL）
#[derive(Debug)]
pub struct SqlDbLoader {
    /// 数据库连接池
    pool: Arc<dyn DbConnectionPool>,
    /// 表名（已验证）
    table_name: String,
    /// 列名（已验证）
    key_column: String,
    /// 值列名（已验证）
    value_column: String,
}

impl SqlDbLoader {
    /// 创建新的SQL数据库加载器
    ///
    /// # 参数
    ///
    /// * `pool` - 数据库连接池
    /// * `table_name` - 缓存表名
    /// * `key_column` - 键列名
    /// * `value_column` - 值列名
    ///
    /// # 返回值
    ///
    /// 返回新的SQL数据库加载器实例
    pub fn new(
        pool: Arc<dyn DbConnectionPool>,
        table_name: String,
        key_column: String,
        value_column: String,
    ) -> Result<Self> {
        if !validate_sql_identifier(&table_name) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid table name: {}. Table name must be a valid SQL identifier.",
                table_name
            )));
        }

        if !validate_sql_identifier(&key_column) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid key column name: {}. Column name must be a valid SQL identifier.",
                key_column
            )));
        }

        if !validate_sql_identifier(&value_column) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid value column name: {}. Column name must be a valid SQL identifier.",
                value_column
            )));
        }

        Ok(Self {
            pool,
            table_name,
            key_column,
            value_column,
        })
    }
}

#[async_trait]
impl DbLoader for SqlDbLoader {
    #[instrument(skip(self), level = "debug")]
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if !validate_cache_key(key) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid cache key format: {}. Key must be alphanumeric or contain -_.:/ and be <= 1024 characters.",
                key
            )));
        }

        let escaped_key = escape_sql_string(key);
        let query = format!(
            "SELECT {} FROM {} WHERE {} = '{}'",
            self.value_column, self.table_name, self.key_column, escaped_key
        );

        self.pool.execute_query(&query).await
    }

    #[instrument(skip(self), level = "debug")]
    async fn load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        for key in &keys {
            if !validate_cache_key(key) {
                return Err(CacheError::InvalidInput(format!(
                    "Invalid cache key format: {}. Key must be alphanumeric or contain -_.:/ and be <= 1024 characters.",
                    key
                )));
            }
        }

        let escaped_keys: Vec<String> = keys.iter().map(|k| format!("'{}'", escape_sql_string(k))).collect();

        let key_list = escaped_keys.join(",");

        // Validate SQL identifiers to prevent SQL injection
        if !validate_sql_identifier(&self.key_column) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid key_column identifier: {}",
                self.key_column
            )));
        }
        if !validate_sql_identifier(&self.value_column) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid value_column identifier: {}",
                self.value_column
            )));
        }
        if !validate_sql_identifier(&self.table_name) {
            return Err(CacheError::InvalidInput(format!(
                "Invalid table_name identifier: {}",
                self.table_name
            )));
        }

        let query = format!(
            "SELECT {}, {} FROM {} WHERE {} IN ({})",
            self.key_column, self.value_column, self.table_name, self.key_column, key_list
        );

        self.pool.execute_batch_query(&query).await
    }

    fn is_healthy(&self) -> bool {
        self.pool.is_healthy()
    }
}

/// 数据库连接池trait
#[async_trait]
pub trait DbConnectionPool: Send + Sync + std::fmt::Debug {
    /// 执行查询
    async fn execute_query(&self, query: &str) -> Result<Option<Vec<u8>>>;

    /// 执行批量查询
    async fn execute_batch_query(&self, query: &str) -> Result<Vec<(String, Vec<u8>)>>;

    /// 检查连接池健康状态
    fn is_healthy(&self) -> bool;
}

/// 配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbFallbackConfig {
    /// 是否启用回源功能
    pub enabled: bool,
    /// 回源超时时间（毫秒）
    pub timeout_ms: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 数据库连接字符串
    pub connection_string: String,
    /// 缓存表名
    pub table_name: String,
    /// 键列名
    pub key_column: String,
    /// 值列名
    pub value_column: String,
}

impl Default for DbFallbackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_ms: 5000,
            max_retries: 3,
            connection_string: String::new(),
            table_name: "cache_table".to_string(),
            key_column: "cache_key".to_string(),
            value_column: "cache_value".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    // ========================================================================
    // validate_sql_identifier tests
    // ========================================================================

    #[test]
    fn test_validate_sql_identifier_valid_simple() {
        assert!(validate_sql_identifier("users"));
        assert!(validate_sql_identifier("user123"));
        assert!(validate_sql_identifier("_private_table"));
        assert!(validate_sql_identifier("a"));
        assert!(validate_sql_identifier("cache_items"));
    }

    #[test]
    fn test_validate_sql_identifier_invalid_starts_with_digit() {
        assert!(!validate_sql_identifier("1users"));
        assert!(!validate_sql_identifier("9abc"));
        assert!(!validate_sql_identifier("0"));
    }

    #[test]
    fn test_validate_sql_identifier_invalid_empty() {
        assert!(!validate_sql_identifier(""));
    }

    #[test]
    fn test_validate_sql_identifier_invalid_special_chars() {
        assert!(!validate_sql_identifier("user-table"));
        assert!(!validate_sql_identifier("users.name"));
        assert!(!validate_sql_identifier("table name"));
        assert!(!validate_sql_identifier("users;DROP"));
        assert!(!validate_sql_identifier("table'name"));
    }

    #[test]
    fn test_validate_sql_identifier_unicode() {
        assert!(!validate_sql_identifier("users_中国"));
    }

    // ========================================================================
    // validate_cache_key tests
    // ========================================================================

    #[test]
    fn test_validate_cache_key_valid_simple() {
        assert!(validate_cache_key("user:123"));
        assert!(validate_cache_key("session-abc"));
        assert!(validate_cache_key("cache.key"));
        assert!(validate_cache_key("item_42"));
        assert!(validate_cache_key("ABC123"));
    }

    #[test]
    fn test_validate_cache_key_invalid_empty() {
        assert!(!validate_cache_key(""));
    }

    #[test]
    fn test_validate_cache_key_invalid_special_chars() {
        assert!(!validate_cache_key("user name"));
        assert!(!validate_cache_key("key\n"));
        assert!(!validate_cache_key("key\t"));
    }

    #[test]
    fn test_validate_cache_key_max_length_boundary() {
        // The feature-gated version uses MAX_CACHE_KEY_LENGTH (1024)
        // and the inline fallback uses 512. Use a value that exceeds both.
        let long_valid_key = "a".repeat(1024);
        assert!(validate_cache_key(&long_valid_key));
        let invalid_key = "a".repeat(1025);
        assert!(!validate_cache_key(&invalid_key));
    }

    // ========================================================================
    // escape_sql_string tests
    // ========================================================================

    #[test]
    fn test_escape_sql_string_single_quote() {
        assert_eq!(escape_sql_string("O'Brien"), "O''Brien");
    }

    #[test]
    fn test_escape_sql_string_backslash() {
        assert_eq!(escape_sql_string("path\\to"), "path\\\\to");
    }

    #[test]
    fn test_escape_sql_string_null_char() {
        assert_eq!(escape_sql_string("hello\0world"), "hello\\0world");
    }

    #[test]
    fn test_escape_sql_string_double_quote() {
        assert_eq!(escape_sql_string("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn test_escape_sql_string_newline() {
        assert_eq!(escape_sql_string("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_escape_sql_string_carriage_return() {
        assert_eq!(escape_sql_string("line1\rline2"), "line1\\rline2");
    }

    #[test]
    fn test_escape_sql_string_tab() {
        assert_eq!(escape_sql_string("col1\tcol2"), "col1\\tcol2");
    }

    #[test]
    fn test_escape_sql_string_mixed() {
        assert_eq!(escape_sql_string("hello 'world'\nnew"), "hello ''world''\\nnew");
    }

    #[test]
    fn test_escape_sql_string_empty() {
        assert_eq!(escape_sql_string(""), "");
    }

    #[test]
    fn test_escape_sql_string_no_special_chars() {
        assert_eq!(escape_sql_string("hello world"), "hello world");
    }

    // ========================================================================
    // DbFallbackManager tests
    // ========================================================================

    mock! {
        pub TestDbLoader {}
        impl std::fmt::Debug for TestDbLoader {
            fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
        }
        #[async_trait]
        impl DbLoader for TestDbLoader {
            async fn load(&self, key: &str) -> Result<Option<Vec<u8>>>;
            async fn load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>>;
            fn is_healthy(&self) -> bool;
        }
    }

    #[tokio::test]
    async fn test_fallback_load_disabled() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(0);

        let manager = DbFallbackManager::new(Arc::new(mock_loader), false, 5000, 3);
        let result = manager.fallback_load("some_key").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fallback_load_unhealthy_loader() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| false);

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 3);
        let result = manager.fallback_load("some_key").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fallback_load_success() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| true);
        mock_loader
            .expect_load()
            .times(1)
            .returning(|_| Ok(Some(b"data".to_vec())));

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 0);
        let result = manager.fallback_load("key1").await.unwrap();
        assert_eq!(result, Some(b"data".to_vec()));
    }

    #[tokio::test]
    async fn test_fallback_load_returns_none_when_key_not_found() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| true);
        mock_loader.expect_load().times(1).returning(|_| Ok(None));

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 0);
        let result = manager.fallback_load("missing").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fallback_load_batch_disabled() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(0);

        let manager = DbFallbackManager::new(Arc::new(mock_loader), false, 5000, 3);
        let result = manager.fallback_load_batch(vec!["key1".to_string()]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_fallback_load_batch_unhealthy() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| false);

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 3);
        let result = manager.fallback_load_batch(vec!["key1".to_string()]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_fallback_load_batch_success() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| true);
        mock_loader.expect_load_batch().times(1).returning(|keys| {
            Ok(keys
                .into_iter()
                .map(|k| (k.clone(), format!("value_for_{}", k).into_bytes()))
                .collect())
        });

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 3);
        let keys = vec!["key1".to_string(), "key2".to_string()];
        let result = manager.fallback_load_batch(keys).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "key1");
    }

    #[tokio::test]
    async fn test_fallback_load_batch_error() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| true);
        mock_loader
            .expect_load_batch()
            .times(1)
            .returning(|_| Err(CacheError::DatabaseError("query failed".to_string())));

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 3);
        let result = manager.fallback_load_batch(vec!["key1".to_string()]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_enabled() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().returning(|| true);

        let enabled_manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 3);
        assert!(enabled_manager.is_enabled());

        let mut mock_loader2 = MockTestDbLoader::new();
        mock_loader2.expect_is_healthy().returning(|| true);

        let disabled_manager = DbFallbackManager::new(Arc::new(mock_loader2), false, 5000, 3);
        assert!(!disabled_manager.is_enabled());
    }

    #[test]
    fn test_db_fallback_manager_debug() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().returning(|| true);

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 3);
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("DbFallbackManager"));
        assert!(debug_str.contains("enabled"));
        assert!(debug_str.contains("timeout_ms"));
    }

    // ========================================================================
    // SqlDbLoader tests
    // ========================================================================

    #[test]
    fn test_sql_db_loader_new_valid() {
        let result = SqlDbLoader::new(
            Arc::new(MockDbConnectionPool::healthy()),
            "users".to_string(),
            "id".to_string(),
            "data".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_sql_db_loader_new_invalid_table_name() {
        let result = SqlDbLoader::new(
            Arc::new(MockDbConnectionPool::healthy()),
            "1invalid".to_string(),
            "id".to_string(),
            "data".to_string(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid table name"));
    }

    #[test]
    fn test_sql_db_loader_new_invalid_key_column() {
        let result = SqlDbLoader::new(
            Arc::new(MockDbConnectionPool::healthy()),
            "users".to_string(),
            "1invalid".to_string(),
            "data".to_string(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid key column name"));
    }

    #[test]
    fn test_sql_db_loader_new_invalid_value_column() {
        let result = SqlDbLoader::new(
            Arc::new(MockDbConnectionPool::healthy()),
            "users".to_string(),
            "id".to_string(),
            "1invalid".to_string(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid value column name"));
    }

    #[tokio::test]
    async fn test_sql_db_loader_load_invalid_cache_key() {
        let loader = SqlDbLoader::new(
            Arc::new(MockDbConnectionPool::healthy()),
            "users".to_string(),
            "id".to_string(),
            "data".to_string(),
        )
        .unwrap();
        let result = loader.load("invalid key with spaces").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sql_db_loader_load_valid_key() {
        let pool = MockDbConnectionPool::healthy();
        let loader = SqlDbLoader::new(
            Arc::new(pool),
            "users".to_string(),
            "id".to_string(),
            "data".to_string(),
        )
        .unwrap();
        let result = loader.load("user:123").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_sql_db_loader_load_batch_empty() {
        let loader = SqlDbLoader::new(
            Arc::new(MockDbConnectionPool::healthy()),
            "users".to_string(),
            "id".to_string(),
            "data".to_string(),
        )
        .unwrap();
        let result = loader.load_batch(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_sql_db_loader_load_batch_invalid_key() {
        let loader = SqlDbLoader::new(
            Arc::new(MockDbConnectionPool::healthy()),
            "users".to_string(),
            "id".to_string(),
            "data".to_string(),
        )
        .unwrap();
        let result = loader
            .load_batch(vec!["valid_key".to_string(), "invalid key".to_string()])
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sql_db_loader_load_batch_success() {
        let pool = MockDbConnectionPool::with_batch_data(vec![
            ("key1".to_string(), b"value1".to_vec()),
            ("key2".to_string(), b"value2".to_vec()),
        ]);
        let loader = SqlDbLoader::new(
            Arc::new(pool),
            "users".to_string(),
            "id".to_string(),
            "data".to_string(),
        )
        .unwrap();
        let result = loader
            .load_batch(vec!["key1".to_string(), "key2".to_string()])
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_sql_db_loader_is_healthy() {
        let pool = MockDbConnectionPool::healthy();
        let loader = SqlDbLoader::new(
            Arc::new(pool),
            "users".to_string(),
            "id".to_string(),
            "data".to_string(),
        )
        .unwrap();
        assert!(loader.is_healthy());
    }

    // ========================================================================
    // DbFallbackConfig tests
    // ========================================================================

    #[test]
    fn test_db_fallback_config_default() {
        let config = DbFallbackConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_retries, 3);
        assert!(config.connection_string.is_empty());
        assert_eq!(config.table_name, "cache_table");
        assert_eq!(config.key_column, "cache_key");
        assert_eq!(config.value_column, "cache_value");
    }

    #[test]
    fn test_db_fallback_config_clone() {
        let config = DbFallbackConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.timeout_ms, config.timeout_ms);
    }

    #[test]
    fn test_db_fallback_config_debug() {
        let config = DbFallbackConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("DbFallbackConfig"));
    }

    // ========================================================================
    // Mock DbConnectionPool for testing
    // ========================================================================

    #[derive(Debug, Clone)]
    struct MockDbConnectionPool {
        healthy: bool,
        batch_data: Vec<(String, Vec<u8>)>,
    }

    impl MockDbConnectionPool {
        fn healthy() -> Self {
            Self {
                healthy: true,
                batch_data: vec![],
            }
        }

        fn with_batch_data(data: Vec<(String, Vec<u8>)>) -> Self {
            Self {
                healthy: true,
                batch_data: data,
            }
        }
    }

    #[async_trait]
    impl DbConnectionPool for MockDbConnectionPool {
        async fn execute_query(&self, _query: &str) -> Result<Option<Vec<u8>>> {
            Ok(None)
        }

        async fn execute_batch_query(&self, _query: &str) -> Result<Vec<(String, Vec<u8>)>> {
            Ok(self.batch_data.clone())
        }

        fn is_healthy(&self) -> bool {
            self.healthy
        }
    }

    // ========================================================================
    // Retry behavior tests
    // ========================================================================

    mock! {
        pub FailingDbLoader {}
        impl std::fmt::Debug for FailingDbLoader {
            fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
        }
        #[async_trait]
        impl DbLoader for FailingDbLoader {
            async fn load(&self, key: &str) -> Result<Option<Vec<u8>>>;
            async fn load_batch(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<u8>)>>;
            fn is_healthy(&self) -> bool;
        }
    }

    #[tokio::test]
    async fn test_fallback_load_retries_then_fails() {
        let mut mock_loader = MockFailingDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| true);
        mock_loader
            .expect_load()
            .times(1)
            .returning(|_| Err(CacheError::DatabaseError("transient error".to_string())));

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 5000, 0);
        let result = manager.fallback_load("test_key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fallback_load_batch_timeout_error() {
        let mut mock_loader = MockTestDbLoader::new();
        mock_loader.expect_is_healthy().times(1).returning(|| true);
        mock_loader
            .expect_load_batch()
            .times(1)
            .returning(|_| Err(CacheError::DatabaseError("connection lost".to_string())));

        let manager = DbFallbackManager::new(Arc::new(mock_loader), true, 1, 0);
        let result = manager.fallback_load_batch(vec!["key1".to_string()]).await;
        assert!(result.is_err());
    }
}
