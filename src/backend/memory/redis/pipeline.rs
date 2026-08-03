// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Redis pipeline batch operations.

use super::client::RedisBackend;
use super::error::map_redis_error;
use crate::error::OxCacheResult;
use crate::security;
use std::time::Duration;

impl RedisBackend {
    /// Batch set multiple key-value pairs using Redis Pipeline.
    ///
    /// Significantly faster than individual SET commands when setting many keys,
    /// as it reduces network round trips from N to 1.
    pub async fn set_many_pipeline(
        &self,
        items: &[(&str, Vec<u8>)],
        ttl: Option<Duration>,
    ) -> OxCacheResult<()> {
        if items.is_empty() {
            return Ok(());
        }

        for (key, _) in items {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for (key, value) in items {
            if let Some(ttl) = ttl {
                pipe.cmd("SETEX")
                    .arg(key)
                    .arg(ttl.as_secs())
                    .arg(value.as_slice());
            } else {
                pipe.cmd("SET").arg(key).arg(value.as_slice());
            }
        }

        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(map_redis_error)?;

        Ok(())
    }

    /// Batch get multiple keys using Redis Pipeline.
    ///
    /// Significantly faster than individual GET commands when fetching many keys.
    pub async fn get_many_pipeline(
        &self,
        keys: &[&str],
    ) -> OxCacheResult<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        for key in keys {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd("GET").arg(key);
        }

        let results: Vec<Option<Vec<u8>>> = pipe
            .query_async(&mut conn)
            .await
            .map_err(map_redis_error)?;

        Ok(results)
    }

    /// Batch delete multiple keys using Redis Pipeline.
    ///
    /// Significantly faster than individual DEL commands when deleting many keys.
    pub async fn delete_many_pipeline(&self, keys: &[&str]) -> OxCacheResult<()> {
        if keys.is_empty() {
            return Ok(());
        }

        for key in keys {
            security::validate_redis_key(key)?;
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for key in keys {
            pipe.cmd("DEL").arg(key);
        }

        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(map_redis_error)?;

        Ok(())
    }
}
