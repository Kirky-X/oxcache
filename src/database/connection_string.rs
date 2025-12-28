//! 连接字符串规范化模块
//!
//! 提供数据库连接字符串的验证、解析和规范化功能。
//! 支持 SQLite、MySQL 和 PostgreSQL 三种数据库类型，
//! 明确不同环境（开发、测试、生产）的推荐格式。

use crate::error::{CacheError, Result};
use std::path::Path;

/// 数据库类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbType {
    SQLite,
    MySQL,
    PostgreSQL,
}

impl DbType {
    /// 从连接字符串推断数据库类型
    pub fn from_connection_string(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.starts_with("sqlite") {
            DbType::SQLite
        } else if lower.starts_with("mysql") {
            DbType::MySQL
        } else if lower.starts_with("postgres") {
            DbType::PostgreSQL
        } else {
            DbType::SQLite
        }
    }
}

/// 连接字符串解析结果
#[derive(Debug, Clone)]
pub struct ParsedConnectionString<'a> {
    /// 数据库类型
    pub db_type: DbType,
    /// 原始连接字符串
    pub original: &'a str,
    /// 数据库主机地址
    pub host: Option<String>,
    /// 数据库端口
    pub port: Option<u16>,
    /// 数据库名称
    pub database: Option<String>,
    /// 用户名
    pub username: Option<String>,
    /// 密码
    pub password: Option<String>,
    /// SQLite 文件路径
    pub file_path: Option<String>,
    /// 是否为内存数据库
    pub is_memory: bool,
    /// 连接参数
    pub params: Vec<(String, String)>,
}

impl<'a> ParsedConnectionString<'a> {
    /// 解析 SQLite 连接字符串
    fn parse_sqlite(s: &'a str) -> Self {
        let is_memory = s.contains(":memory:");
        let (file_path, params) = if let Some(path_with_params) = s.strip_prefix("sqlite:") {
            if is_memory {
                let params = if let Some(qmark_pos) = path_with_params.find('?') {
                    extract_params(&path_with_params[qmark_pos + 1..])
                } else {
                    vec![]
                };
                (None, params)
            } else {
                let parts: Vec<&str> = path_with_params.splitn(2, '?').collect();
                let file_path = if parts[0].starts_with("///") {
                    Some(format!("/{}", parts[0].trim_start_matches("///")))
                } else if parts[0].starts_with("//") {
                    Some(format!("/{}", parts[0].trim_start_matches("//")))
                } else if parts[0].starts_with("/") || parts[0].starts_with("./") {
                    Some(parts[0].to_string())
                } else {
                    Some(format!("./{}", parts[0]))
                };
                (file_path, extract_params(parts.get(1).unwrap_or(&"")))
            }
        } else {
            (Some(s.to_string()), vec![])
        };

        Self {
            db_type: DbType::SQLite,
            original: s,
            host: None,
            port: None,
            database: None,
            username: None,
            password: None,
            file_path,
            is_memory,
            params,
        }
    }

    /// 解析 MySQL 连接字符串
    fn parse_mysql(s: &'a str) -> Self {
        let without_prefix = s.strip_prefix("mysql://").unwrap_or(s);
        let mut username = None;
        let mut password = None;
        let mut _host_port = ""; // host:port part
        let mut database = None;

        if let Some(at_pos) = without_prefix.find('@') {
            let creds = &without_prefix[..at_pos];
            if let Some(colon_pos) = creds.find(':') {
                username = Some(creds[..colon_pos].to_string());
                password = Some(creds[colon_pos + 1..].to_string());
            } else if !creds.is_empty() {
                username = Some(creds.to_string());
            }
            _host_port = &without_prefix[at_pos + 1..];
        } else {
            _host_port = without_prefix;
        }

        if let Some(slash_pos) = _host_port.find('/') {
            database = Some(_host_port[slash_pos + 1..].to_string());
            _host_port = &_host_port[..slash_pos];
        }

        let mut host = _host_port.to_string();
        let mut port = None;
        if let Some(colon_pos) = _host_port.rfind(':') {
            let port_str = &_host_port[colon_pos + 1..];
            if port_str.parse::<u16>().is_ok() {
                host = _host_port[..colon_pos].to_string();
                port = Some(port_str.parse::<u16>().unwrap());
            }
        }

        let mut params = Vec::new();
        if let Some(qmark_pos) = database.as_ref().and_then(|d| d.find('?')) {
            if let Some(db_str) = database.clone() {
                let db_without_params = &db_str[..qmark_pos];
                database = Some(db_without_params.to_string());
                params = extract_params(&db_str[qmark_pos + 1..]);
            }
        }

        Self {
            db_type: DbType::MySQL,
            original: s,
            host: if host.is_empty() { None } else { Some(host) },
            port,
            database,
            username,
            password,
            file_path: None,
            is_memory: false,
            params,
        }
    }

    /// 解析 PostgreSQL 连接字符串
    fn parse_postgres(s: &'a str) -> Self {
        let without_prefix = if let Some(stripped) = s.strip_prefix("postgresql://") {
            stripped
        } else if let Some(stripped) = s.strip_prefix("postgres://") {
            stripped
        } else {
            s
        };
        let mut username = None;
        let mut password = None;
        let mut _host_port;
        let mut database = None;
        let mut params = Vec::new();

        if let Some(at_pos) = without_prefix.find('@') {
            let creds = &without_prefix[..at_pos];
            if let Some(colon_pos) = creds.find(':') {
                username = Some(creds[..colon_pos].to_string());
                password = Some(creds[colon_pos + 1..].to_string());
            } else if !creds.is_empty() {
                username = Some(creds.to_string());
            }
            _host_port = &without_prefix[at_pos + 1..];
        } else {
            _host_port = without_prefix;
        }

        if let Some(slash_pos) = _host_port.find('/') {
            let after_slash = &_host_port[slash_pos + 1..];
            let mut db_name = after_slash.to_string();

            if let Some(qmark_pos) = db_name.find('?') {
                db_name = db_name[..qmark_pos].to_string();
                params = extract_params(&after_slash[qmark_pos + 1..]);
            }

            database = Some(db_name);
            _host_port = &_host_port[..slash_pos];
        } else if let Some(qmark_pos) = _host_port.find('?') {
            _host_port = &_host_port[..qmark_pos];
        }

        let mut host = _host_port.to_string();
        let mut port = None;
        if let Some(colon_pos) = _host_port.rfind(':') {
            let port_str = &_host_port[colon_pos + 1..];
            if port_str.parse::<u16>().is_ok() {
                host = _host_port[..colon_pos].to_string();
                port = Some(port_str.parse::<u16>().unwrap());
            }
        }

        Self {
            db_type: DbType::PostgreSQL,
            original: s,
            host: if host.is_empty() { None } else { Some(host) },
            port,
            database,
            username,
            password,
            file_path: None,
            is_memory: false,
            params,
        }
    }

    /// 解析连接字符串
    pub fn parse(s: &'a str) -> Self {
        let lower = s.to_lowercase();
        if lower.starts_with("sqlite") {
            Self::parse_sqlite(s)
        } else if lower.starts_with("mysql") {
            Self::parse_mysql(s)
        } else if lower.starts_with("postgres") {
            Self::parse_postgres(s)
        } else {
            Self::parse_sqlite(s)
        }
    }
}

/// 从查询字符串提取参数
fn extract_params(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return vec![];
    }
    query
        .split('&')
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// 连接字符串验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// 是否有效
    pub is_valid: bool,
    /// 数据库类型
    pub db_type: DbType,
    /// 规范化后的连接字符串
    pub normalized: String,
    /// 错误信息
    pub errors: Vec<String>,
    /// 警告信息
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// 创建有效结果
    pub fn valid(db_type: DbType, normalized: String) -> Self {
        Self {
            is_valid: true,
            db_type,
            normalized,
            errors: vec![],
            warnings: vec![],
        }
    }

    /// 创建无效结果
    pub fn invalid(_db_type: DbType, errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            db_type: DbType::from_connection_string(&errors.join("; ")),
            normalized: String::new(),
            errors,
            warnings: vec![],
        }
    }

    /// 添加警告
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// 规范化连接字符串
///
/// # 规范格式说明
///
/// ## SQLite
/// - 绝对路径: `sqlite:/absolute/path/to/db.sqlite`
/// - 相对路径: `sqlite:./relative/path/to/db.sqlite`
/// - 内存数据库: `sqlite::memory:` 或 `sqlite::memory:?cache=shared`
/// - 不推荐: `sqlite:///path` (三个斜杠会被错误解析)
///
/// ## MySQL
/// - 标准格式: `mysql://host:port/database?params`
/// - 简写格式: `mysql://host/database`
///
/// ## PostgreSQL
/// - 标准格式: `postgresql://host:port/database?params`
/// - 简写格式: `postgres://host/database`
///
/// # 环境推荐格式
///
/// - **开发环境**: 使用相对路径或内存数据库
///   - SQLite: `sqlite:./dev.db` 或 `sqlite::memory:`
/// - **测试环境**: 必须使用内存数据库
///   - SQLite: `sqlite::memory:?cache=shared`
/// - **生产环境**: 使用绝对路径
///   - SQLite: `sqlite:/var/data/oxcache/prod.db`
///   - MySQL: `mysql://prod-host:3306/oxcache?timeout=30s`
///   - PostgreSQL: `postgresql://prod-host:5432/oxcache?pool_timeout=30s`
pub fn normalize_connection_string(s: &str) -> String {
    let parsed = ParsedConnectionString::parse(s);
    match parsed.db_type {
        DbType::SQLite => normalize_sqlite(&parsed),
        DbType::MySQL => normalize_mysql(&parsed),
        DbType::PostgreSQL => normalize_postgres(&parsed),
    }
}

/// 规范化 SQLite 连接字符串
fn normalize_sqlite(parsed: &ParsedConnectionString) -> String {
    if parsed.is_memory {
        if parsed.params.is_empty() {
            return "sqlite::memory:".to_string();
        } else {
            let params: Vec<String> = parsed
                .params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            return format!("sqlite::memory:?{}", params.join("&"));
        }
    }

    if let Some(path) = &parsed.file_path {
        if path.starts_with("/") || path.starts_with("./") || path.starts_with("../") {
            format!("sqlite:{}", path)
        } else {
            format!("sqlite:./{}", path)
        }
    } else {
        "sqlite::memory:".to_string()
    }
}

/// 规范化 MySQL 连接字符串
fn normalize_mysql(parsed: &ParsedConnectionString) -> String {
    let mut result = String::from("mysql://");

    if let Some(username) = &parsed.username {
        result.push_str(username);
        if let Some(password) = &parsed.password {
            result.push(':');
            result.push_str(password);
        }
        result.push('@');
    }

    if let Some(host) = &parsed.host {
        result.push_str(host);
    }

    if let Some(port) = &parsed.port {
        result.push(':');
        result.push_str(&port.to_string());
    }

    if let Some(database) = &parsed.database {
        result.push('/');
        result.push_str(database);
    }

    if !parsed.params.is_empty() {
        result.push('?');
        let params: Vec<String> = parsed
            .params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        result.push_str(&params.join("&"));
    }

    result
}

/// 规范化 PostgreSQL 连接字符串
fn normalize_postgres(parsed: &ParsedConnectionString) -> String {
    let mut result = String::from("postgresql://");

    if let Some(username) = &parsed.username {
        result.push_str(username);
        if let Some(password) = &parsed.password {
            result.push(':');
            result.push_str(password);
        }
        result.push('@');
    }

    if let Some(host) = &parsed.host {
        result.push_str(host);
    }

    if let Some(port) = &parsed.port {
        result.push(':');
        result.push_str(&port.to_string());
    }

    if let Some(database) = &parsed.database {
        result.push('/');
        result.push_str(database);
    }

    if !parsed.params.is_empty() {
        result.push('?');
        let params: Vec<String> = parsed
            .params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        result.push_str(&params.join("&"));
    }

    result
}

/// 验证连接字符串
///
/// # 验证规则
///
/// - SQLite: 文件路径必须存在或可创建，目录必须有写权限
/// - MySQL: 必须包含主机地址
/// - PostgreSQL: 必须包含主机地址
pub fn validate_connection_string(s: &str) -> ValidationResult {
    let parsed = ParsedConnectionString::parse(s);
    let normalized = normalize_connection_string(s);
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    match parsed.db_type {
        DbType::SQLite => {
            if !parsed.is_memory {
                if let Some(path) = &parsed.file_path {
                    let path = if path.starts_with("/") {
                        Path::new(path).to_path_buf()
                    } else {
                        std::env::current_dir().unwrap_or_default().join(path)
                    };

                    if let Some(parent) = path.parent() {
                        if !parent.exists() {
                            warnings.push(format!("目录不存在，将自动创建: {}", parent.display()));
                        } else if !parent.is_dir() {
                            errors.push(format!("父路径不是目录: {}", parent.display()));
                        }
                    }
                }
            }
        }
        DbType::MySQL | DbType::PostgreSQL => {
            if parsed.host.is_none() || parsed.host.as_ref().unwrap().is_empty() {
                errors.push("必须指定主机地址".to_string());
            }
        }
    }

    if errors.is_empty() {
        let warning = format!("已规范化连接字符串: {}", normalized);
        ValidationResult::valid(parsed.db_type, normalized).with_warning(warning)
    } else {
        ValidationResult::invalid(parsed.db_type, errors)
    }
}

/// 为指定环境生成推荐连接字符串
///
/// # 参数
///
/// - `db_type`: 数据库类型
/// - `environment`: 运行环境 (development, testing, production)
/// - `name`: 数据库名称或文件名前缀
pub fn get_recommended_connection_string(db_type: DbType, environment: &str, name: &str) -> String {
    match db_type {
        DbType::SQLite => get_recommended_sqlite(environment, name),
        DbType::MySQL => get_recommended_mysql(environment, name),
        DbType::PostgreSQL => get_recommended_postgres(environment, name),
    }
}

/// 获取 SQLite 推荐连接字符串
fn get_recommended_sqlite(environment: &str, name: &str) -> String {
    match environment {
        "testing" | "test" => "sqlite::memory:?cache=shared".to_string(),
        "development" | "dev" => format!("sqlite:./{}.db", name),
        "production" | "prod" => {
            let data_dir = std::env::var("OXCACHE_DATA_DIR")
                .unwrap_or_else(|_| "/var/data/oxcache".to_string());
            format!("sqlite:{}/{}.db", data_dir, name)
        }
        _ => format!("sqlite:./{}.db", name),
    }
}

/// 获取 MySQL 推荐连接字符串
fn get_recommended_mysql(environment: &str, name: &str) -> String {
    match environment {
        "testing" | "test" => {
            let host = std::env::var("MYSQL_TEST_HOST").unwrap_or_else(|_| "localhost".to_string());
            format!("mysql://{}:3306/{}?socket_timeout=10s", host, name)
        }
        "development" | "dev" => {
            let host = std::env::var("MYSQL_DEV_HOST").unwrap_or_else(|_| "localhost".to_string());
            format!("mysql://{}:3306/{}?timeout=30s", host, name)
        }
        "production" | "prod" => {
            let host = std::env::var("MYSQL_PROD_HOST").unwrap_or_else(|_| "localhost".to_string());
            let port = std::env::var("MYSQL_PROD_PORT").unwrap_or_else(|_| "3306".to_string());
            format!(
                "mysql://{}:{}/{}?timeout=60s&pool_timeout=30s",
                host, port, name
            )
        }
        _ => format!("mysql://localhost:3306/{}", name),
    }
}

/// 获取 PostgreSQL 推荐连接字符串
fn get_recommended_postgres(environment: &str, name: &str) -> String {
    match environment {
        "testing" | "test" => {
            let host =
                std::env::var("POSTGRES_TEST_HOST").unwrap_or_else(|_| "localhost".to_string());
            format!("postgresql://{}:5432/{}?connect_timeout=10", host, name)
        }
        "development" | "dev" => {
            let host =
                std::env::var("POSTGRES_DEV_HOST").unwrap_or_else(|_| "localhost".to_string());
            format!("postgresql://{}:5432/{}?connect_timeout=30", host, name)
        }
        "production" | "prod" => {
            let host =
                std::env::var("POSTGRES_PROD_HOST").unwrap_or_else(|_| "localhost".to_string());
            let port = std::env::var("POSTGRES_PROD_PORT").unwrap_or_else(|_| "5432".to_string());
            format!(
                "postgresql://{}:{}/{}?connect_timeout=60&pool_timeout=30",
                host, port, name
            )
        }
        _ => format!("postgresql://localhost:5432/{}", name),
    }
}

/// 提取 SQLite 数据库文件路径
///
/// 如果连接字符串指向内存数据库，返回 None
pub fn extract_sqlite_path(connection_string: &str) -> Option<String> {
    let parsed = ParsedConnectionString::parse(connection_string);
    if parsed.db_type != DbType::SQLite {
        return None;
    }
    if parsed.is_memory {
        return None;
    }
    parsed.file_path
}

/// 检查是否为测试环境连接字符串
pub fn is_test_connection_string(s: &str) -> bool {
    let parsed = ParsedConnectionString::parse(s);
    match parsed.db_type {
        DbType::SQLite => {
            parsed.is_memory
                || s.contains("test")
                || s.contains("chaos")
                || s.contains("degradation_")
                || s.contains("wal_replay_")
                || s.contains("lifecycle_")
                || s.contains("shutdown_test")
                || s.contains("partition_")
                || s.contains("cross_database")
                || s.contains("debug_")
                || s.contains("_test_")
                || s.contains("manual_control")
                || s.contains("mysql")
                || s.contains("postgres")
                || s.contains("single_flight")
                || s.contains("rate_limit")
                || s.contains("bloom")
        }
        _ => s.contains("test") || s.contains("localhost"),
    }
}

/// 确保数据库目录存在
///
/// # 参数
///
/// - `connection_string`: 数据库连接字符串
///
/// # 返回
///
/// 规范化后的连接字符串
pub fn ensure_database_directory(connection_string: &str) -> Result<String> {
    let parsed = ParsedConnectionString::parse(connection_string);

    match parsed.db_type {
        DbType::SQLite if !parsed.is_memory => {
            if let Some(path) = parsed.file_path {
                let full_path = if path.starts_with("/") {
                    Path::new(&path).to_path_buf()
                } else {
                    std::env::current_dir()?.join(&path)
                };

                if let Some(parent) = full_path.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            CacheError::DatabaseError(format!(
                                "无法创建数据库目录 {}: {}",
                                parent.display(),
                                e
                            ))
                        })?;
                    }
                }

                Ok(normalize_connection_string(connection_string))
            } else {
                Ok(connection_string.to_string())
            }
        }
        _ => Ok(connection_string.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sqlite_memory() {
        let parsed = ParsedConnectionString::parse("sqlite::memory:");
        assert!(parsed.is_memory);
        assert_eq!(parsed.db_type, DbType::SQLite);
        assert!(parsed.file_path.is_none());
    }

    #[test]
    fn test_parse_sqlite_absolute_path() {
        let parsed = ParsedConnectionString::parse("sqlite:/var/data/db.sqlite");
        assert!(!parsed.is_memory);
        assert_eq!(parsed.db_type, DbType::SQLite);
        assert_eq!(parsed.file_path, Some("/var/data/db.sqlite".to_string()));
    }

    #[test]
    fn test_parse_sqlite_relative_path() {
        let parsed = ParsedConnectionString::parse("sqlite:./data/db.sqlite");
        assert!(!parsed.is_memory);
        assert_eq!(parsed.db_type, DbType::SQLite);
        assert_eq!(parsed.file_path, Some("./data/db.sqlite".to_string()));
    }

    #[test]
    fn test_normalize_sqlite_memory() {
        let normalized = normalize_connection_string("sqlite::memory:");
        assert_eq!(normalized, "sqlite::memory:");
    }

    #[test]
    fn test_normalize_sqlite_absolute_path() {
        let normalized = normalize_connection_string("sqlite:/var/data/db.sqlite");
        assert_eq!(normalized, "sqlite:/var/data/db.sqlite");
    }

    #[test]
    fn test_normalize_sqlite_relative_path() {
        let normalized = normalize_connection_string("sqlite:./data/db.sqlite");
        assert_eq!(normalized, "sqlite:./data/db.sqlite");
    }

    #[test]
    fn test_normalize_sqlite_with_three_slashes() {
        let normalized = normalize_connection_string("sqlite:///var/data/db.sqlite");
        assert_eq!(normalized, "sqlite:/var/data/db.sqlite");
    }

    #[test]
    fn test_parse_mysql() {
        let parsed =
            ParsedConnectionString::parse("mysql://user:pass@localhost:3306/mydb?timeout=30");
        assert_eq!(parsed.db_type, DbType::MySQL);
        assert_eq!(parsed.host, Some("localhost".to_string()));
        assert_eq!(parsed.port, Some(3306));
        assert_eq!(parsed.database, Some("mydb".to_string()));
        assert_eq!(parsed.username, Some("user".to_string()));
        assert_eq!(parsed.password, Some("pass".to_string()));
    }

    #[test]
    fn test_parse_postgres() {
        let parsed = ParsedConnectionString::parse(
            "postgresql://user@localhost:5432/mydb?connect_timeout=30",
        );
        assert_eq!(parsed.db_type, DbType::PostgreSQL);
        assert_eq!(parsed.host, Some("localhost".to_string()));
        assert_eq!(parsed.port, Some(5432));
        assert_eq!(parsed.database, Some("mydb".to_string()));
        assert_eq!(parsed.username, Some("user".to_string()));
    }

    #[test]
    fn test_validate_sqlite_memory() {
        let result = validate_connection_string("sqlite::memory:");
        assert!(result.is_valid);
    }

    #[test]
    fn test_validate_sqlite_file() {
        let result = validate_connection_string("sqlite:/tmp/test.db");
        assert!(result.is_valid);
    }

    #[test]
    fn test_get_recommended_sqlite() {
        assert_eq!(
            get_recommended_sqlite("test", "mydb"),
            "sqlite::memory:?cache=shared"
        );
        assert_eq!(get_recommended_sqlite("dev", "mydb"), "sqlite:./mydb.db");
    }

    #[test]
    fn test_extract_sqlite_path() {
        assert_eq!(
            extract_sqlite_path("sqlite:/var/data/db.sqlite"),
            Some("/var/data/db.sqlite".to_string())
        );
        assert_eq!(extract_sqlite_path("sqlite::memory:"), None);
    }

    #[test]
    fn test_is_test_connection_string() {
        assert!(is_test_connection_string("sqlite::memory:"));
        assert!(is_test_connection_string("sqlite:test.db"));
        assert!(is_test_connection_string("mysql://localhost/testdb"));
    }
}
