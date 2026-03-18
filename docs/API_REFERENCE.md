# API Reference

> **⚠️ API Version Notice**
>
> This document describes **Oxcache v0.1.4** APIs.

This document provides detailed API reference for the Oxcache library.

## Table of Contents

- [Feature Requirements](#feature-requirements)
- [Cache Macro](#cache-macro)
- [Cache Management](#cache-management)
- [Cache Operations](#cache-operations)
- [Configuration](#configuration)
- [Synchronization](#synchronization)
- [Recovery](#recovery)
- [Database Integration](#database-integration)
- [Security Features](#security-features)
- [Observability](#observability)

## Feature Requirements

Oxcache uses feature gates to control functionality. Here are the key features and their requirements:

### Core Features
- **`minimal`**: L1 cache only (Moka)
- **`core`**: L1 + L2 cache (Redis)
- **`full`**: All features enabled

### Component Features
- **`macros`**: Required for `#[cached]` attribute macro
- **`moka`**: L1 cache implementation (Moka)
- **`redis`**: L2 cache implementation (Redis)
- **`confers`**: Unified configuration file support (TOML)
- **`metrics`**: Basic metrics collection
- **`full-metrics`**: OpenTelemetry integration

### Advanced Features
- **`bloom-filter`**: Cache penetration protection
- **`rate-limiting`**: DoS protection
- **`wal-recovery`**: Write-ahead log for durability
- **`batch-write`**: Optimized batch writing
- **`database`**: Database integration
- **`cli`**: Command-line interface

### Example Configurations

```toml
# Full features (recommended)
oxcache = { version = "0.2.0", features = ["full"] }

# Core functionality only
oxcache = { version = "0.2.0", features = ["core"] }

# Minimal - L1 cache only
oxcache = { version = "0.2.0", features = ["minimal"] }

# Custom selection
oxcache = { version = "0.2.0", features = ["core", "macros", "metrics"] }

# Development with specific features
oxcache = { version = "0.2.0", features = [
    "moka",      # L1 cache (Moka)
    "redis",     # L2 cache (Redis)
    "macros",       # #[cached] macro
    "batch-write",  # Optimized batch writing
    "metrics",      # Basic metrics
] }
```

### Feature Dependencies

Some features require other features to be enabled:

| Feature | Required Features | Description |
|---------|-------------------|-------------|
| `bloom-filter` | `moka` | Cache penetration protection |
| `rate-limiting` | `moka` | DoS protection |
| `wal-recovery` | `redis` | Write-ahead log for durability |
| `batch-write` | `redis` | Optimized batch writing |
| `cli` | `confers` | Command-line interface |
| `opentelemetry` | `metrics` | OpenTelemetry integration |
| `database` | `redis` | Database integration |

**Note**: Using the `full` feature automatically enables all dependencies.

## Cache Macro

### `#[cached]` Attribute Macro

Zero-boilerplate caching decorator for async functions.

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `service` | `&str` | Yes | - | Cache service name |
| `ttl` | `u64` | No | `None` | Time-to-live in seconds |
| `key` | `&str` | No | Auto-generated | Custom cache key format |
| `key_prefix` | `&str` | No | `""` | Prefix for cache key |
| `key_generator` | `&str` | No | `"default"` | Key generation strategy: `"default"`, `"simple"`, `"md5"`, `"murmur3"`, `"namespace"` |
| `cache_type` | `&str` | No | `"two-level"` | Cache type: `"two-level"`, `"l1-only"`, `"l2-only"` |

**Example:**

```rust
// Enable macros feature in Cargo.toml
oxcache = { version = "0.2.0", features = ["macros"] }

// In your code
use oxcache::cached;

#[cached(service = "default", ttl = 3600)]
async fn fetch_user(user_id: &str) -> Result<User> {
    // Function body
}
```

**Custom Key Format:**

```rust
#[cached(service = "default", ttl = 3600, key = "user:{user_id}")]
async fn fetch_user(user_id: &str) -> Result<User> {
    // Function body
}
```

## Cache Management

### Initialization

#### `init(config: OxcacheConfig) -> Result<()>`

Initialize cache system with given configuration.

```rust
use oxcache::{init, oxcache_config, ServiceConfig};

let config = oxcache_config()
    .with_service("default", ServiceConfig::two_level())
    .build();

init(config).await?;
```

### Client Management

For direct cache instance management, use the `Cache` and `CacheBuilder` types:

```rust
use oxcache::{Cache, CacheBuilder};
use oxcache::builder::BackendBuilder;

// Create cache directly
let cache: Cache<String, User> = CacheBuilder::new()
    .backend(
        BackendBuilder::tiered()
            .l1_capacity(10000)
            .l2_connection_string("redis://localhost:6379")
    )
    .build()
    .await?;

// Register for macro usage
cache.register_for_macro("my_service").await;
```

## Cache Operations

### Trait `CacheOps`

All cache clients implement this trait.

#### `get<T>(&self, key: &str) -> Result<Option<T>>`

Get a value from the cache.

**Type Parameters:**
- `T`: Value type, must implement `DeserializeOwned`

**Returns:**
- `Ok(Some(T))`: Value found
- `Ok(None)`: Value not found
- `Err(Error)`: Cache error

**Example:**

```rust
let user: Option<User> = client.get("user:123").await?;
```

#### `set<T>(&self, key: &str, value: &T, ttl: Option<u64>) -> Result<()>`

Set a value in the cache.

**Parameters:**
- `key`: Cache key
- `value`: Value to cache, must implement `Serialize`
- `ttl`: Time-to-live in seconds (None = no expiration)

**Example:**

```rust
client.set("user:123", &user, Some(3600)).await?;
```

#### `delete(&self, key: &str) -> Result<()>`

Delete a value from the cache.

```rust
client.delete("user:123").await?;
```

#### `exists(&self, key: &str) -> Result<bool>`

Check if a key exists in the cache.

```rust
let exists = client.exists("user:123").await?;
```

#### `clear(&self) -> Result<()>`

Clear all entries in the cache.

```rust
client.clear().await?;
```

### Batch Operations

Note: Batch operations are currently handled internally by the OptimizedBatchWriter for performance optimization. Direct batch operation APIs are planned for future releases.

#### Optimized Batch Writer

For high-throughput scenarios, use the optimized batch writer:

```rust
use oxcache::sync::OptimizedBatchWriter;

let writer = OptimizedBatchWriter::new(
    client.clone(),
    "default".to_string(),
    100,  // batch_size
    50,   // max_flush_interval_ms
).await?;
```

## Configuration

### Builder Pattern

#### `oxcache_config() -> OxcacheConfigBuilder`

Create a new configuration builder.

```rust
let builder = oxcache_config();
```

#### `OxcacheConfigBuilder::with_service(name: &str, config: ServiceConfig) -> Self`

Add a service configuration.

```rust
let config = oxcache_config()
    .with_service("default", ServiceConfig::two_level())
    .build();
```

#### `OxcacheConfigBuilder::build(self) -> OxcacheConfig`

Build the final configuration.

```rust
let config = oxcache_config().build();
```

### Service Configuration

#### `ServiceConfig::two_level() -> Self`

Create a two-level cache configuration.

```rust
let config = ServiceConfig::two_level();
```

#### `ServiceConfig::l1_only() -> Self`

Create an L1-only cache configuration.

```rust
let config = ServiceConfig::l1_only();
```

#### `ServiceConfig::l2_only() -> Self`

Create an L2-only cache configuration.

```rust
let config = ServiceConfig::l2_only();
```

#### `ServiceConfig::with_ttl(self, ttl: u64) -> Self`

Set the default TTL for the service.

```rust
let config = ServiceConfig::two_level().with_ttl(3600);
```

### L1 Configuration

#### `L1Config::new() -> Self`

Create a new L1 configuration with defaults.

```rust
let l1 = L1Config::new();
```

#### `L1Config::with_max_capacity(self, capacity: u64) -> Self`

Set maximum cache capacity.

```rust
let l1 = L1Config::new().with_max_capacity(10000);
```

#### `L1Config::with_time_to_idle(self, ttl: u64) -> Self`

Set idle expiration time.

```rust
let l1 = L1Config::new().with_time_to_idle(600);
```

### L2 Configuration

#### `L2Config::new() -> Self`

Create a new L2 configuration.

```rust
let l2 = L2Config::new();
```

#### `L2Config::with_mode(self, mode: RedisMode) -> Self`

Set Redis connection mode.

```rust
let l2 = L2Config::new()
    .with_mode(RedisMode::Standalone)
    .with_connection_string("redis://localhost:6379");
```

#### `L2Config::with_connection_string(self, connection_string: &str) -> Self`

Set Redis connection string.

```rust
let l2 = L2Config::new()
    .with_connection_string("redis://localhost:6379");
```

#### `L2Config::with_enable_batch_write(self, enable: bool) -> Self`

Enable batch write optimization.

```rust
let l2 = L2Config::new().with_enable_batch_write(true);
```

## Synchronization

### Batch Writer

#### `OptimizedBatchWriter`

Optimized batch writer for high-throughput scenarios.

```rust
use oxcache::sync::OptimizedBatchWriter;

let writer = OptimizedBatchWriter::new(
    client.clone(),
    "default".to_string(),
    100,  // batch_size
    50,   // max_flush_interval_ms
).await?;
```

### Invalidation

Cache invalidation is handled internally by the `#[cached]` macro and the internal registry. For manual invalidation, use the cache operations:

```rust
// Invalidate a specific key
cache.delete("user:123").await?;

// Clear all entries
cache.clear().await?;
```

### Cache Promotion

Cache promotion is handled automatically by the tiered cache backend. Hot keys are automatically promoted from L2 to L1 based on access patterns.

To configure promotion behavior:

```rust
use oxcache::{Cache, CacheBuilder, BackendBuilder};

let cache: Cache<String, User> = CacheBuilder::new()
    .backend(
        BackendBuilder::tiered()
            .l1_capacity(10000)
            .l2_connection_string("redis://localhost:6379")
            .auto_promote(true)  // Enable automatic promotion
    )
    .build()
    .await?;
```

## Recovery

### Health Check

#### `HealthChecker`

Check cache health status.

```rust
use oxcache::recovery::HealthChecker;

let checker = HealthChecker::new(client.clone()).await?;
let health = checker.check().await?;

if health.is_healthy() {
    println!("Cache is healthy");
}
```

### Write-Ahead Log (WAL)

#### `WALManager`

Manage write-ahead log for durability.

```rust
use oxcache::recovery::WALManager;

let wal = WALManager::new("/path/to/wal").await?;

// Append entry
wal.append_entry("SET user:123 value 3600").await?;

// Replay entries
wal.replay().await?;
```

## Database Integration

### Database Loader

#### `DbLoader`

Load data from database into cache.

```rust
use oxcache::database::DbLoader;
use oxcache::database::DbLoaderConfig;

let config = DbLoaderConfig::new(
    "mysql://user:pass@localhost/db",
    "SELECT id, name FROM users WHERE id = ?",
);

let loader = DbLoader::new(config)?;
let user: Option<User> = loader.load(&["123"]).await?;
```

### Connection String Parser

#### `ConnectionString`

Parse database connection strings.

```rust
use oxcache::database::ConnectionString;

let conn = ConnectionString::parse("mysql://user:pass@localhost:3306/db")?;
println!("Host: {}", conn.host());
println!("Database: {}", conn.database());
```

## Security Features

### Bloom Filter

#### `BloomFilter`

Prevent cache penetration attacks.

```rust
use oxcache::{BloomFilterConfig, BloomFilterOptions};

let options = BloomFilterOptions::new(
    "my_filter".to_string(),
    100000,  // expected elements
    0.01,    // false positive rate
);

let filter = options.build()?;

// Check if key might exist
if filter.might_contain("user:123") {
    // Proceed with cache lookup
} else {
    // Skip cache lookup, go directly to database
}
```

### Rate Limiting

#### `GlobalRateLimiter`

Prevent DoS attacks.

```rust
use oxcache::{RateLimitConfig, GlobalRateLimiter};

let config = RateLimitConfig {
    max_requests_per_second: 1000,
    burst_capacity: 2000,
    block_duration_secs: 10,
};

let limiter = GlobalRateLimiter::new(Some(config));

if limiter.check("user:123").await? {
    // Process request
} else {
    return Err(Error::RateLimitExceeded);
}
```

## Observability

### Metrics

#### `MetricsCollector`

Collect cache metrics.

```rust
use oxcache::MetricsCollector;

let collector = MetricsCollector::new("default".to_string());
let metrics = collector.collect().await?;

println!("Hits: {}", metrics.hits);
println!("Misses: {}", metrics.misses);
println!("Hit Rate: {:.2}%", metrics.hit_rate());
```

### Telemetry

#### `init_tracing(service_name: &str, otlp_endpoint: Option<&str>)`

Initialize distributed tracing.

```rust
use oxcache::telemetry::init_tracing;

init_tracing("my_service", Some("http://localhost:4317"));
```

### Logging

Oxcache uses the `tracing` crate for structured logging.

```rust
use tracing::{info, error, warn};

info!("Cache initialized");
warn!("Redis connection lost, switching to L1-only mode");
error!("Failed to write to cache: {}", err);
```

## Error Handling

### Error Types

```rust
use oxcache::Error;

match result {
    Ok(data) => println!("{:?}", data),
    Err(Error::KeyNotFound) => println!("Key not found"),
    Err(Error::SerializationError(e)) => println!("Serialization error: {}", e),
    Err(Error::ConnectionError(e)) => println!("Connection error: {}", e),
    Err(e) => println!("Other error: {}", e),
}
```

## Type Aliases

```rust
pub type CacheResult<T> = Result<T, Error>;
pub type CacheClient = Arc<dyn CacheOps>;
```

## Examples

See the [examples/](examples/) directory for more usage examples:

- [Basic Operations](examples/01_basics/)
- [Advanced Features](examples/02_advanced/)
- [Performance Testing](examples/03_performance/)
- [Redis Modes](examples/04_redis_modes/)
- [Database Integration](examples/05_database/)
- [Security Features](examples/06_features/)
- [Testing](examples/07_testing/)
- [UAT](examples/08_uat/)
