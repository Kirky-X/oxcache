// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Namespace-scoped key deletion for RedisBackend.

use super::client::RedisBackend;
use super::error::{is_connection_error, map_redis_error};
use crate::core::RedisCommand;
use crate::error::{OxCacheError, OxCacheResult};
use crate::security;

impl RedisBackend {
    /// Delete all keys matching a given prefix using SCAN + pipeline DEL.
    ///
    /// This is a safe alternative to `clear()` that only affects keys with
    /// the specified prefix. Uses incremental SCAN to avoid blocking Redis
    /// and pipeline batch DEL for efficient deletion.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Delete all keys starting with "user:session:"
    /// backend.clear_namespace("user:session:").await?;
    /// ```
    pub async fn clear_namespace(&self, prefix: &str) -> OxCacheResult<()> {
        // Validate that prefix doesn't contain wildcards
        if prefix.contains('*') || prefix.contains('?') {
            return Err(OxCacheError::InvalidInput(
                "Namespace prefix must not contain wildcard characters (* or ?)".to_string(),
            ));
        }

        let pattern = format!("{}*", prefix);
        security::validate_scan_pattern(&pattern)?;

        let mut conn = self.conn();
        let mut cursor = 0i64;

        loop {
            let (new_cursor, keys): (i64, Vec<String>) = redis::cmd(RedisCommand::Scan.as_str())
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
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
                pipe.query_async::<()>(&mut conn).await.map_err(map_redis_error)?;
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(())
    }
}
