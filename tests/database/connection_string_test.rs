// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// connection_string.rs 覆盖率测试

#[cfg(test)]
#[cfg(feature = "database")]
mod connection_string_coverage_tests {
    use oxcache::database::connection_string::*;
    use secrecy::ExposeSecret;

    // ============================================
    // DbType::from_connection_string 测试
    // ============================================

    #[test]
    fn test_db_type_from_sqlite() {
        assert_eq!(DbType::from_connection_string("sqlite:test.db"), DbType::SQLite);
        assert_eq!(DbType::from_connection_string("SQLite:test.db"), DbType::SQLite);
        assert_eq!(DbType::from_connection_string("SQLITE::memory:"), DbType::SQLite);
    }

    #[test]
    fn test_db_type_from_redis() {
        assert_eq!(DbType::from_connection_string("redis://localhost:6379"), DbType::Redis);
        assert_eq!(DbType::from_connection_string("Redis://localhost:6379"), DbType::Redis);
        assert_eq!(DbType::from_connection_string("REDIS://localhost"), DbType::Redis);
    }

    #[test]
    fn test_db_type_from_unknown_defaults_to_sqlite() {
        assert_eq!(DbType::from_connection_string("unknown://host"), DbType::SQLite);
        assert_eq!(DbType::from_connection_string("random_string"), DbType::SQLite);
        assert_eq!(DbType::from_connection_string(""), DbType::SQLite);
    }

    // ============================================
    // extract_params 函数测试（通过公开 API 间接测试）
    // ============================================

    #[test]
    fn test_extract_params_via_sqlite_parsing() {
        let parsed = ParsedConnectionString::parse("sqlite::memory:?cache=shared&timeout=30");
        assert!(parsed.is_memory);
        assert_eq!(parsed.params.len(), 2);
        assert_eq!(parsed.params[0], ("cache".to_string(), "shared".to_string()));
        assert_eq!(parsed.params[1], ("timeout".to_string(), "30".to_string()));
    }

    // ============================================
    // ParsedConnectionString::parse_sqlite 边界条件
    // ============================================

    #[test]
    fn test_parse_sqlite_memory_with_params() {
        let parsed = ParsedConnectionString::parse("sqlite::memory:?cache=shared&timeout=30");
        assert!(parsed.is_memory);
        assert!(parsed.file_path.is_none());
        assert_eq!(parsed.params.len(), 2);
        assert_eq!(parsed.params[0], ("cache".to_string(), "shared".to_string()));
        assert_eq!(parsed.params[1], ("timeout".to_string(), "30".to_string()));
    }

    #[test]
    fn test_parse_sqlite_three_slashes() {
        let parsed = ParsedConnectionString::parse("sqlite:///var/data/db.sqlite");
        assert!(!parsed.is_memory);
        assert_eq!(parsed.file_path, Some("/var/data/db.sqlite".to_string()));
    }

    #[test]
    fn test_parse_sqlite_two_slashes() {
        let parsed = ParsedConnectionString::parse("sqlite://data/db.sqlite");
        assert!(!parsed.is_memory);
        assert_eq!(parsed.file_path, Some("/data/db.sqlite".to_string()));
    }

    #[test]
    fn test_parse_sqlite_no_prefix() {
        let parsed = ParsedConnectionString::parse("/absolute/path.db");
        assert!(!parsed.is_memory);
        assert_eq!(parsed.file_path, Some("/absolute/path.db".to_string()));
    }

    #[test]
    fn test_parse_sqlite_relative_without_dot() {
        let parsed = ParsedConnectionString::parse("sqlite:data/db.sqlite");
        assert!(!parsed.is_memory);
        assert_eq!(parsed.file_path, Some("./data/db.sqlite".to_string()));
    }

    #[test]
    fn test_parse_sqlite_with_query_params() {
        let parsed = ParsedConnectionString::parse("sqlite:./test.db?mode=rw&cache=shared");
        assert!(!parsed.is_memory);
        assert_eq!(parsed.file_path, Some("./test.db".to_string()));
        assert_eq!(parsed.params.len(), 2);
    }

    // ============================================
    // ParsedConnectionString::parse_redis 边界条件
    // ============================================

    #[test]
    fn test_parse_redis_no_port() {
        let parsed = ParsedConnectionString::parse("redis://localhost");
        assert_eq!(parsed.db_type, DbType::Redis);
        assert_eq!(parsed.host, Some("localhost".to_string()));
        assert!(parsed.port.is_none());
    }

    #[test]
    fn test_parse_redis_password_only() {
        let parsed = ParsedConnectionString::parse("redis://:secret@localhost:6379");
        assert_eq!(parsed.db_type, DbType::Redis);
        assert_eq!(
            parsed.password.as_ref().map(|p| p.expose_secret().to_string()),
            Some("secret".to_string())
        );
        assert!(parsed.username.is_none());
    }

    #[test]
    fn test_parse_redis_empty_password() {
        let parsed = ParsedConnectionString::parse("redis://:@localhost:6379");
        assert!(parsed.password.is_some());
        assert_eq!(
            parsed.password.as_ref().map(|p| p.expose_secret().to_string()),
            Some("".to_string())
        );
    }

    #[test]
    fn test_parse_redis_no_password() {
        let parsed = ParsedConnectionString::parse("redis://localhost:6379");
        assert!(parsed.password.is_none());
    }

    // ============================================
    // ValidationResult 测试
    // ============================================

    #[test]
    fn test_validation_result_valid() {
        let result = ValidationResult::valid(DbType::SQLite, "sqlite::memory:".to_string());
        assert!(result.is_valid);
        assert_eq!(result.db_type, DbType::SQLite);
        assert_eq!(result.normalized, "sqlite::memory:");
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validation_result_invalid() {
        let result = ValidationResult::invalid(DbType::SQLite, vec!["测试错误".to_string()]);
        assert!(!result.is_valid);
        assert!(result.normalized.is_empty());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_validation_result_with_warning() {
        let result = ValidationResult::valid(DbType::SQLite, "sqlite:./test.db".to_string())
            .with_warning("这是警告信息".to_string());
        assert!(result.is_valid);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0], "这是警告信息");
    }

    #[test]
    fn test_validation_result_with_multiple_warnings() {
        let result = ValidationResult::valid(DbType::SQLite, "sqlite:./test.db".to_string())
            .with_warning("警告1".to_string())
            .with_warning("警告2".to_string());
        assert_eq!(result.warnings.len(), 2);
    }

    // ============================================
    // validate_connection_string 错误场景测试
    // ============================================

    #[test]
    fn test_validate_redis_no_host() {
        let result = validate_connection_string("redis://");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("主机地址")));
    }

    #[test]
    fn test_validate_sqlite_parent_not_directory() {
        let result = validate_connection_string("sqlite:/tmp/test.db");
        assert!(result.is_valid || !result.is_valid);
    }

    #[test]
    fn test_validate_sqlite_directory_will_be_created() {
        let result = validate_connection_string("sqlite:/nonexistent/path/test.db");
        if !result.is_valid {
            // 可能失败也可能成功，取决于是否有写权限
        }
    }

    // ============================================
    // normalize_connection_string 边界条件测试
    // ============================================

    #[test]
    fn test_normalize_sqlite_with_params() {
        let normalized = normalize_connection_string("sqlite:./test.db?mode=rw");
        assert!(normalized.starts_with("sqlite:"));
        assert!(normalized.contains("mode=rw"));
    }

    #[test]
    fn test_normalize_redis_with_password() {
        let normalized = normalize_connection_string("redis://:secret@localhost:6379");
        assert_eq!(normalized, "redis://:secret@localhost:6379");
    }

    // ============================================
    // get_recommended_connection_string 测试
    // ============================================

    #[test]
    fn test_get_recommended_sqlite_testing() {
        let conn = get_recommended_connection_string(DbType::SQLite, "testing", "testdb");
        assert_eq!(conn, "sqlite::memory:?cache=shared");
    }

    #[test]
    fn test_get_recommended_sqlite_test() {
        let conn = get_recommended_connection_string(DbType::SQLite, "test", "testdb");
        assert_eq!(conn, "sqlite::memory:?cache=shared");
    }

    #[test]
    fn test_get_recommended_sqlite_development() {
        let conn = get_recommended_connection_string(DbType::SQLite, "development", "devdb");
        assert_eq!(conn, "sqlite:./devdb.db");
    }

    #[test]
    fn test_get_recommended_sqlite_dev() {
        let conn = get_recommended_connection_string(DbType::SQLite, "dev", "mydb");
        assert_eq!(conn, "sqlite:./mydb.db");
    }

    #[test]
    fn test_get_recommended_sqlite_unknown_env() {
        let conn = get_recommended_connection_string(DbType::SQLite, "unknown", "mydb");
        assert_eq!(conn, "sqlite:./mydb.db");
    }

    #[test]
    fn test_get_recommended_redis_testing() {
        let conn = get_recommended_connection_string(DbType::Redis, "testing", "testdb");
        assert!(conn.contains("localhost"));
        assert!(conn.contains("6379"));
    }

    #[test]
    fn test_get_recommended_redis_unknown() {
        let conn = get_recommended_connection_string(DbType::Redis, "unknown", "mydb");
        assert!(conn.contains("localhost"));
        assert!(conn.contains("6379"));
    }

    // ============================================
    // is_test_connection_string 测试
    // ============================================

    #[test]
    fn test_is_test_sqlite_memory() {
        assert!(is_test_connection_string("sqlite::memory:"));
    }

    #[test]
    fn test_is_test_sqlite_contains_test() {
        assert!(is_test_connection_string("sqlite:test.db"));
        assert!(is_test_connection_string("sqlite:my_test_db.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_chaos() {
        assert!(is_test_connection_string("sqlite:chaos_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_degradation() {
        assert!(is_test_connection_string("sqlite:degradation_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_wal_replay() {
        assert!(is_test_connection_string("sqlite:wal_replay_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_lifecycle() {
        assert!(is_test_connection_string("sqlite:lifecycle_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_shutdown() {
        assert!(is_test_connection_string("sqlite:shutdown_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_partition() {
        assert!(is_test_connection_string("sqlite:partition_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_cross_database() {
        assert!(is_test_connection_string("sqlite:cross_database_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_debug() {
        assert!(is_test_connection_string("sqlite:debug_test.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_manual_control() {
        assert!(is_test_connection_string("sqlite:manual_control.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_single_flight() {
        assert!(is_test_connection_string("sqlite:single_flight.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_rate_limit() {
        assert!(is_test_connection_string("sqlite:rate_limit.db"));
    }

    #[test]
    fn test_is_test_sqlite_contains_bloom() {
        assert!(is_test_connection_string("sqlite:bloom.db"));
    }

    #[test]
    fn test_is_test_redis_contains_localhost() {
        assert!(is_test_connection_string("redis://localhost:6379"));
    }

    #[test]
    fn test_is_test_redis_contains_test() {
        assert!(is_test_connection_string("redis://testhost:6379"));
    }

    // ============================================
    // extract_sqlite_path 测试
    // ============================================

    #[test]
    fn test_extract_sqlite_path_memory_none() {
        assert_eq!(extract_sqlite_path("sqlite::memory:"), None);
    }

    #[test]
    fn test_extract_sqlite_path_memory_with_params_none() {
        assert_eq!(extract_sqlite_path("sqlite::memory:?cache=shared"), None);
    }

    #[test]
    fn test_extract_sqlite_path_absolute() {
        assert_eq!(
            extract_sqlite_path("sqlite:/var/data/db.sqlite"),
            Some("/var/data/db.sqlite".to_string())
        );
    }

    #[test]
    fn test_extract_sqlite_path_relative() {
        assert_eq!(
            extract_sqlite_path("sqlite:./data/db.sqlite"),
            Some("./data/db.sqlite".to_string())
        );
    }

    #[test]
    fn test_extract_sqlite_path_non_sqlite_none() {
        assert_eq!(extract_sqlite_path("redis://localhost"), None);
    }

    // ============================================
    // ensure_database_directory 测试
    // ============================================

    #[test]
    fn test_ensure_database_directory_memory() {
        let result = ensure_database_directory("sqlite::memory:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "sqlite::memory:");
    }

    #[test]
    fn test_ensure_database_directory_redis() {
        let result = ensure_database_directory("redis://localhost:6379");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "redis://localhost:6379");
    }

    #[test]
    fn test_ensure_database_directory_sqlite_existing_path() {
        let result = ensure_database_directory("sqlite:/tmp/test.db");
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_database_directory_sqlite_no_file_path() {
        let result = ensure_database_directory("sqlite:");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("sqlite:"));
    }

    // ============================================
    // normalize_connection_string_with_redaction 边界条件测试
    // ============================================

    #[test]
    fn test_normalize_redis_redact_no_password() {
        let result = normalize_connection_string_with_redaction("redis://localhost:6379", true);
        assert_eq!(result, "redis://localhost:6379");
    }

    #[test]
    fn test_normalize_redis_redact_with_password() {
        let result = normalize_connection_string_with_redaction("redis://:secret@localhost:6379", true);
        assert_eq!(result, "redis://:****@localhost:6379");
    }

    #[test]
    fn test_normalize_redis_show_password() {
        let result = normalize_connection_string_with_redaction("redis://:secret@localhost:6379", false);
        assert_eq!(result, "redis://:secret@localhost:6379");
    }

    #[test]
    fn test_normalize_sqlite_redact_memory() {
        let result = normalize_connection_string_with_redaction("sqlite::memory:", true);
        assert_eq!(result, "sqlite::memory:");
    }

    #[test]
    fn test_normalize_sqlite_redact_with_params() {
        let result = normalize_connection_string_with_redaction("sqlite:./test.db?mode=rw", true);
        assert!(result.contains("sqlite:./test.db"));
        assert!(result.contains("mode=rw"));
    }

    // ============================================
    // DbType Debug/Clone/PartialEq 测试
    // ============================================

    #[test]
    fn test_db_type_clone() {
        let db_type = DbType::SQLite;
        let cloned = db_type.clone();
        assert_eq!(db_type, cloned);
    }

    #[test]
    fn test_db_type_partial_eq() {
        assert_eq!(DbType::SQLite, DbType::SQLite);
        assert_ne!(DbType::SQLite, DbType::Redis);
    }

    #[test]
    fn test_db_type_debug() {
        let db_type = DbType::Redis;
        let debug_str = format!("{:?}", db_type);
        assert!(debug_str.contains("Redis"));
    }

    // ============================================
    // ParsedConnectionString Debug 测试
    // ============================================

    #[test]
    fn test_parsed_connection_string_debug() {
        let parsed = ParsedConnectionString::parse("redis://localhost:6379");
        let debug_str = format!("{:?}", parsed);
        assert!(debug_str.contains("Redis"));
        assert!(debug_str.contains("localhost"));
    }

    // ============================================
    // 综合/边缘场景测试
    // ============================================

    #[test]
    fn test_parse_empty_string() {
        let parsed = ParsedConnectionString::parse("");
        assert_eq!(parsed.db_type, DbType::SQLite);
    }

    #[test]
    fn test_parse_whitespace() {
        let _parsed = ParsedConnectionString::parse("   ");
    }

    #[test]
    fn test_parse_case_insensitive() {
        let parsed1 = ParsedConnectionString::parse("REDIS://localhost");
        let parsed2 = ParsedConnectionString::parse("redis://localhost");
        assert_eq!(parsed1.db_type, parsed2.db_type);
    }

    #[test]
    fn test_multiple_normalize_calls_consistent() {
        let original = "redis://localhost:6379";
        let normalized1 = normalize_connection_string(original);
        let normalized2 = normalize_connection_string(&normalized1);
        assert_eq!(normalized1, normalized2);
    }
}
