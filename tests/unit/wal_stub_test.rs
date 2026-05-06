// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// WAL 空实现测试 - 仅在没有 wal-recovery feature 时运行

#[cfg(not(feature = "wal-recovery"))]
mod tests {
    use oxcache::features::recovery::wal::{Operation, WalEntry, WalManager, WalReplayableBackend};
    use std::sync::Arc;
    use std::time::SystemTime;

    #[test]
    fn test_wal_entry_create() {
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "test_key".to_string(),
            value: Some(b"value".to_vec()),
            ttl: Some(3600),
        };
        assert_eq!(entry.key, "test_key");
    }

    #[test]
    fn test_wal_entry_with_none_values() {
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Delete,
            key: "delete_key".to_string(),
            value: None,
            ttl: None,
        };
        assert_eq!(entry.key, "delete_key");
        assert!(entry.value.is_none());
        assert!(entry.ttl.is_none());
    }

    #[test]
    fn test_operation_variants() {
        let set = Operation::Set;
        let delete = Operation::Delete;

        assert!(matches!(set, Operation::Set));
        assert!(matches!(delete, Operation::Delete));
    }

    #[test]
    fn test_operation_default() {
        let op = Operation::default();
        assert!(matches!(op, Operation::Set));
    }

    #[tokio::test]
    async fn test_wal_manager_new() {
        let wal = WalManager::new("test").await.unwrap();
        let _ = wal;
    }

    #[tokio::test]
    async fn test_wal_manager_add_entry() {
        let wal = WalManager::new("test").await.unwrap();
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "key".to_string(),
            value: None,
            ttl: None,
        };
        wal.add_entry(&entry).await.unwrap();
    }

    #[tokio::test]
    async fn test_wal_manager_append() {
        let wal = WalManager::new("test").await.unwrap();
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "key".to_string(),
            value: None,
            ttl: None,
        };
        wal.append(entry).await.unwrap();
    }

    #[tokio::test]
    async fn test_wal_manager_get_entries() {
        let wal = WalManager::new("test").await.unwrap();
        let entries = wal.get_entries().await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_wal_manager_clear_entries() {
        let wal = WalManager::new("test").await.unwrap();
        wal.clear_entries().await.unwrap();
    }

    #[tokio::test]
    async fn test_wal_manager_flush() {
        let wal = WalManager::new("test").await.unwrap();
        wal.flush().await.unwrap();
    }

    #[tokio::test]
    async fn test_wal_manager_clear() {
        let wal = WalManager::new("test").await.unwrap();
        wal.clear().await.unwrap();
    }

    #[derive(Clone, Default)]
    struct TestBackend;

    #[allow(async_fn_in_trait)]
    impl WalReplayableBackend for TestBackend {
        async fn pipeline_replay(&self, _entries: Vec<WalEntry>) -> oxcache::error::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_wal_manager_replay_all() {
        let wal = WalManager::new("test").await.unwrap();
        let backend = TestBackend;
        let count = wal.replay_all(&backend).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_wal_replayable_backend_arc_impl() {
        let backend = Arc::new(TestBackend);
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "key".to_string(),
            value: None,
            ttl: None,
        };
        backend.pipeline_replay(vec![entry]).await.unwrap();
    }

    #[test]
    fn test_wal_entry_clone() {
        let entry1 = WalEntry {
            timestamp: SystemTime::now(),
            operation: Operation::Set,
            key: "key".to_string(),
            value: Some(b"val".to_vec()),
            ttl: Some(60),
        };
        let entry2 = entry1.clone();
        assert_eq!(entry1.key, entry2.key);
    }

    #[test]
    fn test_wal_manager_debug() {
        let wal = WalManager::default();
        let debug = format!("{:?}", wal);
        assert!(debug.contains("WalManager"));
    }
}
