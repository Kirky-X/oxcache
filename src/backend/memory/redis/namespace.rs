// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Namespace-scoped key deletion for RedisBackend.

use super::client::RedisBackend;
use super::error::{is_connection_error, map_redis_error};
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use crate::security;

/// 命名空间分隔符：命名空间段之间的边界
const NAMESPACE_SEPARATOR: char = ':';

impl RedisBackend {
    /// Delete all keys in a namespace using SCAN + pipeline DEL.
    ///
    /// This is a safe alternative to `clear()` that only affects keys of a
    /// single namespace. The namespace is matched **with the segment
    /// boundary**: a namespace without a trailing `:` is completed with it,
    /// so `"tenant1"` deletes `tenant1:*` and never `tenant10:*`.
    ///
    /// Uses incremental SCAN to avoid blocking Redis and pipeline batch DEL
    /// for efficient deletion.
    ///
    /// Note: SCAN + DEL is not atomic — keys created concurrently may be
    /// missed or deleted after matching. Callers needing a point-in-time
    /// snapshot must coordinate writes externally.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Delete all keys of the "user:session:" namespace
    /// backend.clear_namespace("user:session:").await?;
    /// ```
    pub async fn clear_namespace(&self, namespace: &str) -> OxCacheResult<()> {
        let namespace = Self::validated_prefix(namespace)?;
        // 命名空间边界：未以分隔符结尾时补齐，避免 tenant1 连带扫掉 tenant10
        let prefix = if namespace.ends_with(NAMESPACE_SEPARATOR) {
            namespace.to_string()
        } else {
            format!("{namespace}{NAMESPACE_SEPARATOR}")
        };
        self.clear_by_pattern(&format!("{prefix}*")).await
    }

    /// Delete all keys starting with a **raw** prefix using SCAN + pipeline DEL.
    ///
    /// Unlike [`clear_namespace`](Self::clear_namespace) this matches
    /// `prefix*` literally and therefore has **no namespace boundary**:
    /// `"tenant1"` also matches `tenant10:*` and `tenant1abc:*`. Use it only
    /// when raw-prefix semantics are intended; prefer
    /// [`clear_namespace`](Self::clear_namespace) otherwise.
    pub async fn clear_prefix(&self, prefix: &str) -> OxCacheResult<()> {
        let prefix = Self::validated_prefix(prefix)?;
        self.clear_by_pattern(&format!("{prefix}*")).await
    }

    /// 校验前缀：非空、不含通配符与 glob 元字符
    fn validated_prefix(prefix: &str) -> OxCacheResult<&str> {
        // An empty prefix would match everything ("*"); refuse explicitly
        // instead of wiping the whole database.
        if prefix.is_empty() {
            return Err(OxCacheError::InvalidInput(
                "Namespace prefix must not be empty (empty prefix would delete the entire database)"
                    .to_string(),
            ));
        }

        // Validate that prefix doesn't contain wildcards
        if prefix.contains('*') || prefix.contains('?') {
            return Err(OxCacheError::InvalidInput(
                "Namespace prefix must not contain wildcard characters (* or ?)".to_string(),
            ));
        }

        // Glob character classes and escapes would let the prefix match keys
        // outside the intended namespace.
        if prefix.contains('[') || prefix.contains(']') || prefix.contains('\\') {
            return Err(OxCacheError::InvalidInput(
                "Namespace prefix must not contain glob pattern characters ([, ], \\)".to_string(),
            ));
        }

        Ok(prefix)
    }

    /// 按 SCAN 模式批量删除键
    async fn clear_by_pattern(&self, pattern: &str) -> OxCacheResult<()> {
        security::validate_scan_pattern(pattern)?;

        let mut conn = self.conn();
        let mut cursor = 0i64;

        loop {
            let (new_cursor, keys): (i64, Vec<String>) = redis::cmd(RedisCommand::Scan.as_str())
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    if is_connection_error(&e) {
                        OxCacheError::Connection(e.to_string())
                    } else {
                        OxCacheError::Operation(e.to_string())
                    }
                })?;

            if !keys.is_empty() {
                // Pipeline batch DEL
                let mut pipe = redis::pipe();
                for key in &keys {
                    pipe.cmd(RedisCommand::Del.as_str()).arg(key);
                }
                pipe.query_async::<()>(&mut conn)
                    .await
                    .map_err(map_redis_error)?;
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(())
    }
}
