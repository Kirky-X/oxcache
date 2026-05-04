//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! SQL 和命令注入检测

use crate::error::{CacheError, Result};

/// 检查 SQL 注入模式
pub(crate) fn check_sql_injection(key: &str) -> Result<()> {
    const SQL_INJECTION_PATTERNS: &[(&str, &str)] = &[
        ("' OR '", "单引号后跟 OR 模式"),
        ("'--", "SQL 注释模式"),
        ("'; DROP", "SQL DROP 语句"),
        ("'; DELETE", "SQL DELETE 语句"),
        ("'; INSERT", "SQL INSERT 语句"),
        ("UNION SELECT", "SQL UNION 查询"),
        ("xp_cmdshell", "SQL Server 命令执行"),
        ("' OR '1'='1", "经典 SQL 注入永真条件"),
        ("admin'--", "SQL 认证绕过"),
        ("1=1", "SQL 注入永真条件"),
        ("1=2", "SQL 注入永真条件"),
    ];

    let key_upper = key.to_uppercase();
    for (pattern, description) in SQL_INJECTION_PATTERNS {
        if key_upper.contains(&pattern.to_uppercase()) {
            if *pattern == "1=1" || *pattern == "1=2" {
                if key_upper.contains("V1_")
                    || key_upper.contains("_V1")
                    || key_upper.contains("V2_")
                    || key_upper.contains("_V2")
                    || key_upper.contains("KEY_")
                    || key_upper.contains("_KEY")
                    || key_upper.contains("DATA_")
                    || key_upper.contains("_DATA")
                    || key_upper.contains("_STATUS")
                    || key_upper.contains("_ID")
                    || key_upper.contains("_NAME")
                    || key_upper.contains("_TYPE")
                {
                    continue;
                }
            }
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains suspicious SQL injection pattern: {}",
                description
            )));
        }
    }
    Ok(())
}

/// 检查命令注入字符
pub(crate) fn check_command_injection(key: &str) -> Result<()> {
    const COMMAND_INJECTION_CHARS: &[char] = &[';', '|', '&', '`'];

    for c in key.chars() {
        if COMMAND_INJECTION_CHARS.contains(&c) {
            return Err(CacheError::InvalidInput(format!(
                "Redis key contains potential command injection character: {:?}",
                c
            )));
        }
    }
    Ok(())
}
