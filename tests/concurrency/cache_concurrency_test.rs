//! Concurrency tests for oxcache Cache operations
//!
//! Tests cover: concurrent read/write, multi-task testing, timeout protection, deadlock avoidance

#[cfg(test)]
mod tests {
    use oxcache::{Cache, Cacheable};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;
    use tokio::task;

    #[derive(Debug, Clone, Serialize, Deserialize, Cacheable)]
    struct User {
        id: u64,
        name: String,
    }

    /// Test concurrent read/write operations
    #[tokio::test]
    async fn test_concurrent_read_write() -> anyhow::Result<()> {
        let cache: Arc<RwLock<Cache<String, User>>> = Arc::new(
            RwLock::new(Cache::memory().await?)
        );

        let cache_clone = cache.clone();

        // Spawn writer task
        let writer = task::spawn(async move {
            for i in 0..100 {
                let user = User {
                    id: i,
                    name: format!("User{}", i),
                };
                cache_clone.write().await
                    .set(&format!("user:{}", i), &user).await
                    .unwrap();
            }
        });

        let cache_clone2 = cache.clone();

        // Spawn reader task
        let reader = task::spawn(async move {
            for _ in 0..100 {
                let _user: Option<User> = cache_clone2.read().await
                    .get("user:50").await
                    .unwrap();
            }
        });

        // Wait for both tasks
        writer.await?;
        reader.await?;

        // Verify data integrity
        let user: Option<User> = cache.read().await
            .get("user:50").await?;

        assert!(user.is_some());
        assert_eq!(user.unwrap().name, "User50");

        Ok(())
    }

    /// Test multiple concurrent tasks
    #[tokio::test]
    async fn test_multiple_concurrent_tasks() -> anyhow::Result<()> {
        let cache: Arc<Cache<String, User>> = Arc::new(
            Cache::memory().await?
        );

        // Create multiple tasks that all access the cache
        let mut handles = vec![];

        for i in 0..10 {
            let cache = cache.clone();
            let handle = task::spawn(async move {
                let user = User {
                    id: i,
                    name: format!("TaskUser{}", i),
                };
                cache.set(&format!("task:{}", i), &user).await
            });
            handles.push(handle);
        }

        // Wait all tasks
        for handle in handles {
            handle.await?;
        }

        // Verify all writes succeeded
        for i in 0..10 {
            let user: Option<User> = cache.get(&format!("task:{}", i)).await?;
            assert!(user.is_some());
        }

        Ok(())
    }

    /// Test timeout protection
    #[tokio::test]
    async fn test_timeout_protection() -> anyhow::Result<()> {
        let cache: Cache<String, User> = Cache::memory().await?;

        // Set a value
        let user = User {
            id: 1,
            name: "Test".to_string(),
        };
        cache.set("key:1", &user).await?;

        // Use timeout for get operation
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            cache.get("key:1")
        ).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        Ok(())
    }

    /// Test no deadlocks with rapid acquire/release
    #[tokio::test]
    async fn test_no_deadlock_rapid_access() -> anyhow::Result<()> {
        let cache: Arc<Cache<String, User>> = Arc::new(
            Cache::memory().await?
        );

        // Rapidly acquire and release locks
        for iteration in 0..50 {
            // Write
            let user = User {
                id: iteration,
                name: format!("Iteration{}", iteration),
            };
            cache.set(&format!("key:{}", iteration), &user).await?;

            // Read immediately after
            let _result: Option<User> = cache.get(&format!("key:{}", iteration)).await?;

            // Delete
            cache.delete(&format!("key:{}", iteration)).await?;
        }

        Ok(())
    }

    /// Test concurrent set_many and get_many
    #[tokio::test]
    async fn test_concurrent_batch_operations() -> anyhow::Result<()> {
        let cache: Arc<Cache<String, User>> = Arc::new(
            Cache::memory().await?
        );

        let mut handles = vec![];

        // Multiple tasks doing batch operations
        for batch_id in 0..5 {
            let cache = cache.clone();
            let handle = task::spawn(async move {
                let items: Vec<(&str, User)> = (0..20)
                    .map(|i| (
                        format!("batch{}:item{}", batch_id, i).as_str(),
                        User {
                            id: i as u64,
                            name: format!("Batch{}Item{}", batch_id, i),
                        }
                    ))
                    .collect();

                cache.set_many(items).await
            });
            handles.push(handle);
        }

        // Wait all batches
        for handle in handles {
            handle.await?;
        }

        // Verify total count
        let len = cache.len().await?;
        assert_eq!(len, 100); // 5 batches * 20 items

        Ok(())
    }

    /// Test concurrent delete operations
    #[tokio::test]
    async fn test_concurrent_delete() -> anyhow::Result<()> {
        let cache: Arc<Cache<String, User>> = Arc::new(
            Cache::memory().await?
        );

        // Populate cache
        for i in 0..50 {
            let user = User { id: i, name: format!("User{}", i) };
            cache.set(&format!("key:{}", i), &user).await?;
        }

        // Concurrent deletes
        let mut handles = vec![];
        for i in 0..50 {
            let cache = cache.clone();
            let handle = task::spawn(async move {
                cache.delete(&format!("key:{}", i)).await
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await?;
        }

        // Verify empty
        let len = cache.len().await?;
        assert_eq!(len, 0);

        Ok(())
    }

    /// Test get_or with concurrent fallback calls
    #[tokio::test]
    async fn test_concurrent_get_or_fallback() -> anyhow::Result<()> {
        let cache: Arc<Cache<String, User>> = Arc::new(
            Cache::memory().await?
        );

        // Concurrent get_or calls for same key
        let mut handles = vec![];

        for _ in 0..10 {
            let cache = cache.clone();
            let handle = task::spawn(async move {
                cache.get_or("shared:key", || async {
                    Ok(User {
                        id: 999,
                        name: "Fallback".to_string(),
                    })
                }).await
            });
            handles.push(handle);
        }

        let mut results = vec![];
        for handle in handles {
            let result = handle.await?;
            results.push(result);
        }

        // All should return the same value (either all fallback or all cached)
        let first_name = results[0].name.clone();
        for result in &results {
            assert_eq!(result.name, first_name);
        }

        Ok(())
    }
}
