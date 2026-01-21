# Oxcache API Migration Guide

This guide helps you migrate from the old API to the modernized cache API introduced in version 0.2.0.

## Overview

The new API provides:
- **Independent cache instances** (no global state)
- **Type-safe** `Cache<K, V>` interface
- **Simplified configuration** with Builder pattern
- **Default trait implementations** for common types
- **Pluggable backends** via Strategy pattern

## Quick Comparison

### Old API (Deprecated)

```rust
use oxcache::{init, get_client, ServiceConfig, oxcache_config};

// Initialize globally
let config = oxcache_config()
    .with_service("default", ServiceConfig::two_level())
    .build();
init(config).await?;

// Get client (dynamic dispatch)
let client = get_client("default")?;

// Use client
let value: Option<User> = client.get("user:1").await?;
client.set("user:1", &user).await?;
```

### New API (Recommended)

```rust
use oxcache::Cache;

// Create independent cache instance
let cache: Cache<String, User> = Cache::new().await?;

// Use cache (type-safe)
let value: Option<User> = cache.get(&"user:1".to_string()).await?;
cache.set(&"user:1".to_string(), &user).await?;
```

## Migration Examples

### 1. Simple Memory Cache

**Old API:**
```rust
use oxcache::{init, get_client, ServiceConfig, oxcache_config, CacheType};

let config = oxcache_config()
    .with_service("my_cache", ServiceConfig {
        cache_type: CacheType::L1,
        ..Default::default()
    })
    .build();
init(config).await?;

let client = get_client("my_cache")?;
```

**New API:**
```rust
use oxcache::Cache;

let cache: Cache<String, MyType> = Cache::new().await?;
// or
let cache: Cache<String, MyType> = Cache::memory().await?;
```

### 2. Redis Cache

**Old API:**
```rust
use oxcache::{init, get_client, ServiceConfig, oxcache_config, CacheType, L2Config, RedisMode};

let config = oxcache_config()
    .with_service("redis_cache", ServiceConfig {
        cache_type: CacheType::L2,
        l2: Some(L2Config {
            mode: RedisMode::Standalone,
            connection_string: "redis://localhost:6379".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    })
    .build();
init(config).await?;

let client = get_client("redis_cache")?;
```

**New API:**
```rust
use oxcache::Cache;

let cache: Cache<String, MyType> = Cache::redis("redis://localhost:6379").await?;
```

### 3. Tiered Cache (L1 + L2)

**Old API:**
```rust
use oxcache::{init, get_client, ServiceConfig, oxcache_config, CacheType, L1Config, L2Config};

let config = oxcache_config()
    .with_service("tiered_cache", ServiceConfig {
        cache_type: CacheType::TwoLevel,
        l1: Some(L1Config {
            max_capacity: 10000,
            ..Default::default()
        }),
        l2: Some(L2Config {
            connection_string: "redis://localhost:6379".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    })
    .build();
init(config).await?;

let client = get_client("tiered_cache")?;
```

**New API:**
```rust
use oxcache::Cache;

let cache: Cache<String, MyType> = Cache::tiered(10000, "redis://localhost:6379").await?;
```

### 4. Advanced Configuration

**Old API:**
```rust
use oxcache::{init, get_client, ServiceConfig, oxcache_config, CacheType};

let config = oxcache_config()
    .with_service("advanced", ServiceConfig {
        cache_type: CacheType::TwoLevel,
        ttl: Some(3600),
        l1: Some(L1Config {
            max_capacity: 10000,
            ..Default::default()
        }),
        l2: Some(L2Config {
            connection_string: "redis://localhost:6379".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    })
    .build();
init(config).await?;

let client = get_client("advanced")?;
```

**New API:**
```rust
use oxcache::{Cache, builder::BackendBuilder};
use std::time::Duration;

let cache: Cache<String, MyType> = Cache::builder()
    .backend(
        BackendBuilder::tiered()
            .l1_capacity(10000)
            .l2_connection_string("redis://localhost:6379")
            .auto_promote(true)
    )
    .build()
    .await?;
```

### 5. Cache-Aside Pattern

**Old API:**
```rust
let client = get_client("users")?;

let user: Option<User> = client.get("user:1").await?;
let user = match user {
    Some(u) => u,
    None => {
        let u = fetch_user_from_db(1).await?;
        client.set("user:1", &u).await?;
        u
    }
};
```

**New API:**
```rust
let cache: Cache<String, User> = Cache::new().await?;

let user: User = cache.get_or(&"user:1".to_string(), || async {
    fetch_user_from_db(1).await
}).await?;
```

### 6. Batch Operations

**Old API:**
```rust
let client = get_client("users")?;

// Manual iteration
for (key, value) in users {
    client.set(&key, &value).await?;
}

for key in keys {
    client.delete(&key).await?;
}
```

**New API:**
```rust
let cache: Cache<String, User> = Cache::new().await?;

// Batch set
cache.set_many(users.iter().map(|(k, v)| (k, v))).await?;

// Batch delete
cache.delete_many(keys.iter()).await?;

// Batch get
let results: HashMap<String, User> = cache.get_many(keys.iter()).await?;
```

## Key Differences

### 1. Global State vs. Independent Instances

**Old API:**
- Uses global `MANAGER` state
- Requires `init()` before use
- Shared state across application
- Difficult to test (state pollution)

**New API:**
- Each `Cache` instance is independent
- No initialization required
- Easy to create multiple isolated caches
- Test-friendly (no shared state)

### 2. Type Safety

**Old API:**
```rust
let client = get_client("my_cache")?;
let value: Option<User> = client.get("user:1").await?;  // Manual type annotation
```

**New API:**
```rust
let cache: Cache<String, User> = Cache::new().await?;
let value: Option<User> = cache.get(&"user:1".to_string()).await?;  // Type-safe
```

### 3. Key Types

**Old API:**
- Always uses `&str` for keys
- No type safety for keys

**New API:**
- Supports any type implementing `CacheKey`
- Built-in support for `String`, `&str`, `u64`, `i64`, etc.
- Custom key types with `CacheKey` trait

```rust
// String keys
let cache: Cache<String, User> = Cache::new().await?;

// u64 keys
let cache: Cache<u64, User> = Cache::new().await?;

// Custom key type
impl CacheKey for UserId {
    fn to_key_string(&self) -> String {
        format!("user:{}", self.0)
    }
}

let cache: Cache<UserId, User> = Cache::new().await?;
```

### 4. Error Handling

**Old API:**
```rust
match client.get("key").await {
    Ok(Some(value)) => Ok(value),
    Ok(None) => Err(MyError::NotFound),
    Err(e) => Err(MyError::Cache(e)),
}
```

**New API:**
```rust
match cache.get(&key).await {
    Ok(Some(value)) => Ok(value),
    Ok(None) => Err(MyError::NotFound),
    Err(e) if e.is_not_found() => Err(MyError::NotFound),
    Err(e) if e.is_connection_error() => Err(MyError::Connection),
    Err(e) => Err(MyError::Cache(e)),
}
```

## Configuration Migration

### Old Configuration Format

```toml
[services.my_service]
cache_type = "two-level"
ttl = 3600

[services.my_service.l1]
max_capacity = 10000

[services.my_service.l2]
mode = "standalone"
connection_string = "redis://localhost:6379"
```

### New Configuration (Code-based)

```rust
use oxcache::{Cache, builder::BackendBuilder};
use std::time::Duration;

let cache: Cache<String, MyType> = Cache::builder()
    .backend(
        BackendBuilder::tiered()
            .l1_capacity(10000)
            .l2_connection_string("redis://localhost:6379")
    )
    .ttl(Duration::from_secs(3600))
    .build()
    .await?;
```

## Testing Migration

### Old API (Test Setup)

```rust
#[tokio::test]
async fn test_old_api() {
    // Need to reset global state
    oxcache::manager::CacheManager::reset();

    let config = oxcache_config()
        .with_service("test", ServiceConfig::l1_only())
        .build();
    init(config).await.unwrap();

    let client = get_client("test").unwrap();
    // ... test code

    // Need to reset after test
    oxcache::manager::CacheManager::reset();
}
```

### New API (Test Setup)

```rust
#[tokio::test]
async fn test_new_api() {
    // No global state, just create cache
    let cache: Cache<String, MyType> = Cache::new().await.unwrap();

    // ... test code

    // No cleanup needed
}
```

## Deprecation Timeline

- **v0.2.0**: New API introduced, old API marked as deprecated
- **v0.3.0**: Old API still functional but warnings in logs
- **v0.4.0**: Old API removed (breaking change)

## Common Issues

### Issue: `init()` function not found

**Solution:**
```rust
// Old (deprecated)
oxcache::init(config).await?;

// New
let cache: Cache<String, MyType> = Cache::new().await?;
```

### Issue: `get_client()` returns wrong type

**Solution:**
```rust
// Old (deprecated)
let client = get_client("my_cache")?;
let value: Option<User> = client.get("key").await?;

// New
let cache: Cache<String, User> = Cache::new().await?;
let value: Option<User> = cache.get(&"key".to_string()).await?;
```

### Issue: Type inference not working

**Solution:**
```rust
// Explicit type annotation
let cache: Cache<String, User> = Cache::new().await?;

// Or use turbofish
let cache = Cache::<String, User>::new().await?;
```

## Additional Resources

- [API Documentation](https://docs.rs/oxcache)
- [Examples](../examples/)
- [GitHub Repository](https://github.com/Kirky-X/oxcache)

## Support

If you need help with migration, please:
1. Check this guide for common patterns
2. Review the examples in the `examples/` directory
3. Open an issue on GitHub with your specific use case