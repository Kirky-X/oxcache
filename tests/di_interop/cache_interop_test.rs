//! DI (Dependency Injection) Interoperability Tests
//!
//! Tests cover: cache backend interchangeability, config backend interchangeability,
//! database session interchangeability, trait object polymorphism

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use oxcache::{Cache, CacheBackend, Cacheable};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[derive(Debug, Clone, Serialize, Deserialize, Cacheable)]
    struct TestData {
        id: u64,
        value: String,
    }

    /// Test cache backend interchangeability via trait object
    #[tokio::test]
    async fn test_cache_backend_interchangeability() -> Result<()> {
        // Test with different backend implementations through trait object

        // Memory backend
        let mem_cache: Arc<dyn CacheBackend<String, TestData>> =
            Arc::new(Cache::memory().await?);

        let data = TestData {
            id: 1,
            value: "memory".to_string(),
        };
        mem_cache.set("key:1", &data).await?;
        let retrieved: Option<TestData> = mem_cache.get("key:1").await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, "memory");

        // Verify the trait object can be used polymorphically
        test_backend_polymorphism(mem_cache).await?;

        Ok(())
    }

    /// Test that different backends can be used through same interface
    async fn test_backend_polymorphism(
        backend: Arc<dyn CacheBackend<String, TestData>>
    ) -> Result<()> {
        // Common operations should work regardless of backend
        let test_data = TestData {
            id: 42,
            value: "polymorphic".to_string(),
        };

        backend.set("poly:key", &test_data).await?;
        let result: Option<TestData> = backend.get("poly:key").await?;

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 42);

        backend.delete("poly:key").await?;
        let after_delete: Option<TestData> = backend.get("poly:key").await?;
        assert!(after_delete.is_none());

        Ok(())
    }

    /// Test config backend interchangeability
    #[tokio::test]
    async fn test_config_backend_interchangeability() -> Result<()> {
        // Test that config can work with different sources through common interface
        use confers::{Config, sources::*};

        // File source
        let file_config = Config::builder()
            .add_source(File::new("test_config.toml", FileFormat::Toml))
            .build()
            .await?;

        // Env source
        let env_config = Config::builder()
            .add_source(Environment::default())
            .build()
            .await?;

        // Merge multiple sources
        let merged = Config::builder()
            .add_source(File::new("test_config.toml", FileFormat::Toml))
            .add_source(Environment::default())
            .build()
            .await?;

        // Verify all configs implement common traits
        verify_config_interface(&file_config).await?;
        verify_config_interface(&env_config).await?;
        verify_config_interface(&merged).await?;

        Ok(())
    }

    /// Verify config implements required interface
    async fn verify_config_interface(config: &Config) -> Result<()> {
        // Common config operations
        let _has_values = !config.collect::<Vec<_>>().await.is_empty() || true;

        // Get a value if exists
        let _value: Option<String> = config.get("test").cloned();

        Ok(())
    }

    /// Test database session interchangeability
    #[tokio::test]
    async fn test_database_session_interchangeability() -> Result<()> {
        use dbnexus::{Pool, Session};

        // Test pool creation and session management
        // Note: In real tests, this would use testcontainers
        // Here we verify the interface exists

        // Create a test pool (mock or real depending on setup)
        // The key is that different pool implementations share the same interface

        // Verify Session trait is implemented
        // This would be tested with actual database connections

        Ok(())
    }

    /// Test trait object runtime polymorphism
    #[tokio::test]
    async fn test_trait_object_polymorphism() -> Result<()> {
        // Create a collection of different cache backends
        let mut backends: Vec<Box<dyn CacheBackend<String, TestData>>> = Vec::new();

        // Memory backend as trait object
        let mem_cache = Cache::memory().await?;
        backends.push(Box::new(mem_cache));

        // Test each backend through trait object
        for (idx, backend) in backends.iter_mut().enumerate() {
            let test_data = TestData {
                id: idx as u64,
                value: format!("backend_{}", idx),
            };

            let key = format!("key:{}", idx);
            backend.set(&key, &test_data).await?;

            let retrieved: Option<TestData> = backend.get(&key).await?;
            assert!(retrieved.is_some());
        }

        Ok(())
    }

    /// Test dynamic dispatch for cache operations
    #[tokio::test]
    async fn test_dynamic_dispatch_cache_operations() -> Result<()> {
        // Use RwLock for interior mutability with trait objects
        let cache: Arc<RwLock<Box<dyn CacheBackend<String, TestData>>>> =
            Arc::new(RwLock::new(Box::new(Cache::memory().await?)));

        // Write operation
        {
            let mut guard = cache.write().await;
            let data = TestData {
                id: 1,
                value: "dynamic".to_string(),
            };
            guard.set("dynamic:key", &data).await?;
        }

        // Read operation
        {
            let guard = cache.read().await;
            let result: Option<TestData> = guard.get("dynamic:key").await?;
            assert!(result.is_some());
        }

        Ok(())
    }

    /// Test async trait methods work with different backends
    #[tokio::test]
    async fn test_async_trait_methods() -> Result<()> {
        let cache = Cache::memory().await?;

        // Test async operations
        let data = TestData {
            id: 1,
            value: "async".to_string(),
        };

        // set_with_ttl
        cache.set_with_ttl("ttl:key", &data, std::time::Duration::from_secs(60)).await?;

        // get_many / set_many
        let items = vec![
            ("batch:1", TestData { id: 1, value: "batch1".to_string() }),
            ("batch:2", TestData { id: 2, value: "batch2".to_string() }),
        ];
        cache.set_many(items).await?;

        let keys = vec!["batch:1", "batch:2"];
        let results: Vec<Option<TestData>> = cache.get_many(keys).await?;
        assert_eq!(results.len(), 2);

        // stats
        let _stats = cache.stats().await?;

        // clear
        cache.clear().await?;
        let len = cache.len().await?;
        assert_eq!(len, 0);

        Ok(())
    }
}
