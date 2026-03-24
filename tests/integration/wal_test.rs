//! WAL (Write-Ahead Log) Recovery Tests
//!
//! 这些测试验证 WAL 恢复机制的完整功能。

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::Result;
    use oxcache::error::{CacheError, Result as OxResult};
    use oxcache::recovery::wal::{Operation, WalEntry, WalManager, WalReplayableBackend};
    use tokio::sync::Mutex;

    /// 用于测试的 MockBackend
    #[derive(Clone, Default)]
    struct MockBackend {
        pub entries: Arc<Mutex<Vec<WalEntry>>>,
        pub should_fail: bool,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                entries: Arc::new(Mutex::new(Vec::new())),
                should_fail: false,
            }
        }

        fn with_failure() -> Self {
            Self {
                entries: Arc::new(Mutex::new(Vec::new())),
                should_fail: true,
            }
        }
    }

    #[allow(async_fn_in_trait)]
    impl WalReplayableBackend for MockBackend {
        async fn pipeline_replay(&self, entries: Vec<WalEntry>) -> OxResult<()> {
            if self.should_fail {
                return Err(CacheError::BackendError("Mock backend failure for testing".to_string()));
            }

            let mut backend_entries = self.entries.lock().await;
            backend_entries.extend(entries);
            Ok(())
        }
    }

    fn create_test_entry(key: &str, operation: Operation) -> WalEntry {
        WalEntry {
            timestamp: SystemTime::now(),
            operation,
            key: key.to_string(),
            value: Some(b"test_value".to_vec()),
            ttl: Some(3600),
        }
    }

    fn is_same_operation(a: &Operation, b: &Operation) -> bool {
        matches!(
            (a, b),
            (Operation::Set, Operation::Set) | (Operation::Delete, Operation::Delete)
        )
    }

    // ========================================================================
    // WalEntry Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_entry_creation() -> Result<()> {
        let entry = create_test_entry("test_key", Operation::Set);

        assert_eq!(entry.key, "test_key");
        assert!(matches!(entry.operation, Operation::Set));
        assert!(entry.value.is_some());
        assert_eq!(entry.value.as_ref().unwrap(), b"test_value");
        assert!(entry.ttl.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_wal_entry_delete_operation() -> Result<()> {
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Delete,
            key: "delete_key".to_string(),
            value: None,
            ttl: None,
        };

        assert_eq!(entry.key, "delete_key");
        assert!(matches!(entry.operation, Operation::Delete));
        assert!(entry.value.is_none());
        assert!(entry.ttl.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_wal_entry_clone() -> Result<()> {
        let entry1 = create_test_entry("clone_key", Operation::Set);
        let entry2 = entry1.clone();

        assert_eq!(entry1.key, entry2.key);
        assert!(is_same_operation(&entry1.operation, &entry2.operation));
        assert_eq!(entry1.value, entry2.value);

        Ok(())
    }

    // ========================================================================
    // WalManager Basic Operations Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_manager_new() -> Result<()> {
        let wal = WalManager::new("test_service").await?;
        assert!(wal.flush().await.is_ok());
        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_append_single_entry() -> Result<()> {
        let wal = WalManager::new("test_append").await?;
        let entry = create_test_entry("append_key", Operation::Set);

        wal.append(entry).await?;

        // Flush to ensure entry is written
        wal.flush().await?;

        // Get entries and verify
        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "append_key");

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_add_entry() -> Result<()> {
        let wal = WalManager::new("test_add").await?;
        let entry = create_test_entry("add_key", Operation::Set);

        wal.add_entry(&entry).await?;
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "add_key");

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_multiple_entries() -> Result<()> {
        let wal = WalManager::new("test_multiple").await?;

        // Add multiple entries
        for i in 0..5 {
            let entry = WalEntry {
                timestamp: SystemTime::now(),
                operation: Operation::Set,
                key: format!("key_{}", i),
                value: Some(format!("value_{}", i).into_bytes()),
                ttl: Some(3600),
            };
            wal.append(entry).await?;
        }

        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 5);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_delete_operation() -> Result<()> {
        let wal = WalManager::new("test_delete").await?;

        // Add a delete entry
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Delete,
            key: "delete_me".to_string(),
            value: None,
            ttl: None,
        };

        wal.append(entry).await?;
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].operation, Operation::Delete));
        assert_eq!(entries[0].key, "delete_me");

        wal.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WalManager Clear and Flush Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_clear_entries() -> Result<()> {
        let wal = WalManager::new("test_clear").await?;

        // Add some entries
        for i in 0..3 {
            let entry = create_test_entry(&format!("clear_key_{}", i), Operation::Set);
            wal.append(entry).await?;
        }
        wal.flush().await?;

        // Verify entries exist
        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 3);

        // Clear entries
        wal.clear_entries().await?;

        // Verify entries are cleared
        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 0);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_clear_alias() -> Result<()> {
        let wal = WalManager::new("test_clear_alias").await?;

        let entry = create_test_entry("clear_alias_key", Operation::Set);
        wal.append(entry).await?;
        wal.flush().await?;

        wal.clear().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 0);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_flush_empty() -> Result<()> {
        let wal = WalManager::new("test_flush_empty").await?;

        // Flush empty WAL should not fail
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 0);

        wal.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WAL Replay Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_replay_all() -> Result<()> {
        let wal = WalManager::new("test_replay").await?;
        let backend = MockBackend::new();

        // Add entries to WAL
        for i in 0..3 {
            let entry = create_test_entry(&format!("replay_key_{}", i), Operation::Set);
            wal.append(entry).await?;
        }
        wal.flush().await?;

        // Replay to backend
        let count = wal.replay_all(&backend).await?;
        assert_eq!(count, 3);

        // Verify backend received entries
        let backend_entries = backend.entries.lock().await;
        assert_eq!(backend_entries.len(), 3);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_replay_empty() -> Result<()> {
        let wal = WalManager::new("test_replay_empty").await?;
        let backend = MockBackend::new();

        // Replay empty WAL
        let count = wal.replay_all(&backend).await?;
        assert_eq!(count, 0);

        // Backend should have no entries
        let backend_entries = backend.entries.lock().await;
        assert_eq!(backend_entries.len(), 0);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_replay_with_delete() -> Result<()> {
        let wal = WalManager::new("test_replay_delete").await?;
        let backend = MockBackend::new();

        // Add set and delete entries
        wal.append(create_test_entry("set_then_delete", Operation::Set)).await?;
        wal.append(WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Delete,
            key: "set_then_delete".to_string(),
            value: None,
            ttl: None,
        })
        .await?;
        wal.flush().await?;

        // Replay
        let count = wal.replay_all(&backend).await?;
        assert_eq!(count, 2);

        let backend_entries = backend.entries.lock().await;
        assert_eq!(backend_entries.len(), 2);

        // Check we have both operations
        let has_set = backend_entries.iter().any(|e| matches!(e.operation, Operation::Set));
        let has_delete = backend_entries.iter().any(|e| matches!(e.operation, Operation::Delete));
        assert!(has_set);
        assert!(has_delete);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_replay_failure_preserves_entries() -> Result<()> {
        let wal = WalManager::new("test_replay_fail").await?;
        let backend = MockBackend::with_failure();

        // Add entries
        for i in 0..2 {
            let entry = create_test_entry(&format!("fail_key_{}", i), Operation::Set);
            wal.append(entry).await?;
        }
        wal.flush().await?;

        // Try to replay (should fail)
        let result = wal.replay_all(&backend).await;
        assert!(result.is_err());

        // Entries should still exist in WAL
        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 2);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_replay_clears_after_success() -> Result<()> {
        let wal = WalManager::new("test_replay_clear").await?;
        let backend = MockBackend::new();

        // Add entries
        let entry = create_test_entry("clear_after_replay", Operation::Set);
        wal.append(entry).await?;
        wal.flush().await?;

        // Verify entries exist
        assert_eq!(wal.get_entries().await?.len(), 1);

        // Replay successfully
        wal.replay_all(&backend).await?;

        // Entries should be cleared
        assert_eq!(wal.get_entries().await?.len(), 0);

        // But backend should have them
        assert_eq!(backend.entries.lock().await.len(), 1);

        wal.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WAL Ordering Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_entries_ordered_by_timestamp() -> Result<()> {
        let wal = WalManager::new("test_order").await?;

        // Add entries with different timestamps (should be ordered by insertion time)
        for i in 0..5 {
            let entry = WalEntry {
                timestamp: SystemTime::now(),
                operation: Operation::Set,
                key: format!("order_key_{}", i),
                value: Some(i.to_string().into_bytes()),
                ttl: None,
            };
            wal.append(entry).await?;

            // Small delay to ensure different timestamps
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 5);

        // Verify ordering (earlier entries should come first)
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.key, format!("order_key_{}", i));
        }

        wal.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WAL with Different Entry Types Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_entry_with_no_ttl() -> Result<()> {
        let wal = WalManager::new("test_no_ttl").await?;

        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "no_ttl_key".to_string(),
            value: Some(b"no_ttl_value".to_vec()),
            ttl: None,
        };

        wal.append(entry).await?;
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].ttl.is_none());

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_entry_with_large_value() -> Result<()> {
        let wal = WalManager::new("test_large_value").await?;

        // Create a large value (1MB)
        let large_value = vec![0u8; 1024 * 1024];

        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "large_value_key".to_string(),
            value: Some(large_value),
            ttl: Some(86400), // 24 hours
        };

        wal.append(entry).await?;
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value.as_ref().unwrap().len(), 1024 * 1024);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_special_characters_in_key() -> Result<()> {
        let wal = WalManager::new("test_special_chars").await?;

        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "key/with/slashes:and:colons".to_string(),
            value: Some(b"special".to_vec()),
            ttl: None,
        };

        wal.append(entry).await?;
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "key/with/slashes:and:colons");

        wal.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WAL Shutdown Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_shutdown() -> Result<()> {
        let wal = WalManager::new("test_shutdown").await?;

        // Add entry
        let entry = create_test_entry("shutdown_key", Operation::Set);
        wal.append(entry).await?;

        // Shutdown should be graceful
        wal.shutdown().await;

        Ok(())
    }

    #[tokio::test]
    async fn test_wal_double_shutdown() -> Result<()> {
        let wal = WalManager::new("test_double_shutdown").await?;

        // First shutdown
        wal.shutdown().await;

        // Second shutdown should also be fine (no-op after first)
        wal.shutdown().await;

        Ok(())
    }

    // ========================================================================
    // WAL Service Name Isolation Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_service_name_isolation() -> Result<()> {
        // Note: When OXCACHE_TEST_USE_MEMORY is set or service_name matches test patterns,
        // all WALs share the same in-memory database. This test verifies that entries
        // are correctly filtered by service_name in the query.

        let wal1 = WalManager::new("test_isolation_a").await?;
        let wal2 = WalManager::new("test_isolation_b").await?;

        // Add entries to different services
        wal1.append(create_test_entry("key_a", Operation::Set)).await?;
        wal2.append(create_test_entry("key_b", Operation::Set)).await?;

        wal1.flush().await?;
        wal2.flush().await?;

        // Verify each WAL only has its own entries (filtered by service_name in query)
        let entries1 = wal1.get_entries().await?;
        let entries2 = wal2.get_entries().await?;

        assert_eq!(entries1.len(), 1);
        assert_eq!(entries2.len(), 1);
        assert_eq!(entries1[0].key, "key_a");
        assert_eq!(entries2[0].key, "key_b");

        wal1.shutdown().await;
        wal2.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WAL Edge Cases Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_very_old_timestamp() -> Result<()> {
        let wal = WalManager::new("test_old_timestamp").await?;

        // Use a very old timestamp (year 2000)
        let old_timestamp = UNIX_EPOCH + std::time::Duration::from_secs(946684800); // 2000-01-01

        let entry = WalEntry {
            timestamp: old_timestamp,
            operation: Operation::Set,
            key: "old_timestamp_key".to_string(),
            value: Some(b"old".to_vec()),
            ttl: None,
        };

        wal.append(entry).await?;
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        // Note: SQLite only stores second-level precision
        // Compare using duration from Unix epoch (seconds only)
        let expected_secs = old_timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let actual_secs = entries[0].timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(expected_secs, actual_secs);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_future_timestamp() -> Result<()> {
        let wal = WalManager::new("test_future_timestamp").await?;

        // Use a future timestamp
        let future_timestamp = SystemTime::now() + std::time::Duration::from_secs(86400 * 365); // 1 year from now

        let entry = WalEntry {
            timestamp: future_timestamp,
            operation: Operation::Set,
            key: "future_timestamp_key".to_string(),
            value: Some(b"future".to_vec()),
            ttl: None,
        };

        wal.append(entry).await?;
        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 1);
        // Note: SQLite only stores second-level precision, so nanoseconds are lost
        // Compare using duration from Unix epoch (seconds only)
        let expected_secs = future_timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let actual_secs = entries[0].timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(expected_secs, actual_secs);

        wal.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_wal_replay_preserves_operation_order() -> Result<()> {
        let wal = WalManager::new("test_operation_order").await?;
        let backend = MockBackend::new();

        // Add set, then delete, then set again for same key
        wal.append(WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "same_key".to_string(),
            value: Some(b"v1".to_vec()),
            ttl: None,
        })
        .await?;

        // Small delay to ensure order
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        wal.append(WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Delete,
            key: "same_key".to_string(),
            value: None,
            ttl: None,
        })
        .await?;

        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        wal.append(WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "same_key".to_string(),
            value: Some(b"v2".to_vec()),
            ttl: None,
        })
        .await?;

        wal.flush().await?;

        // Replay
        wal.replay_all(&backend).await?;

        // Backend should have all 3 operations in order
        let backend_entries = backend.entries.lock().await;
        assert_eq!(backend_entries.len(), 3);
        assert!(matches!(backend_entries[0].operation, Operation::Set));
        assert!(matches!(backend_entries[1].operation, Operation::Delete));
        assert!(matches!(backend_entries[2].operation, Operation::Set));

        wal.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WAL Sequential Operations Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_sequential_appends() -> Result<()> {
        let wal = WalManager::new("test_sequential").await?;

        // Sequentially append entries
        for i in 0..10 {
            let entry = create_test_entry(&format!("seq_key_{}", i), Operation::Set);
            wal.append(entry).await?;
        }

        wal.flush().await?;

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 10);

        // Verify all keys are present
        for i in 0..10 {
            let has_key = entries.iter().any(|e| e.key == format!("seq_key_{}", i));
            assert!(has_key, "Missing key seq_key_{}", i);
        }

        wal.shutdown().await;
        Ok(())
    }

    // ========================================================================
    // WAL Batch Operations Tests
    // ========================================================================

    #[tokio::test]
    async fn test_wal_multiple_flush_cycles() -> Result<()> {
        let wal = WalManager::new("test_flush_cycles").await?;

        // Multiple flush cycles
        for cycle in 0..3 {
            for i in 0..3 {
                let entry = create_test_entry(&format!("cycle_{}_key_{}", cycle, i), Operation::Set);
                wal.append(entry).await?;
            }
            wal.flush().await?;
        }

        let entries = wal.get_entries().await?;
        assert_eq!(entries.len(), 9);

        wal.shutdown().await;
        Ok(())
    }
}
