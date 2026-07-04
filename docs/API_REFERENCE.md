# API Reference

> **⚠️ API Version Notice**
>
> This document describes **Oxcache v0.3.3** APIs.

This document provides detailed API reference for the Oxcache library.

## Table of Contents

- [Feature Requirements](#feature-requirements)
- [Cache Macro](#cache-macro)
- [Cache<K, V>](#cachek-v)
- [CacheBuilder](#cachebuilder)
- [Backend Layer](#backend-layer)
- [RedisBackend](#redisbackend)
- [ChainCache](#chaincache)
- [Synchronous API](#synchronous-api)
- [Bloom Filter](#bloom-filter)
- [TTL Management](#ttl-management)
- [Security Features](#security-features)
- [Observability](#observability)
- [Error Handling](#error-handling)

## Feature Requirements

Oxcache uses feature gates to control functionality. Here are the key features and their requirements:

### Tiered Feature Sets

- **`minimal`**: L1 cache only (memory + tracing + metrics + serialization + chrono)
- **`core`**: L1 + L2 cache (minimal + redis + futures)
- **`full`**: All features enabled (default)

### Component Features

- **`memory`**: L1 cache backends (Moka + DashMap)
- **`redis`**: L2 cache implementation (Redis + regex)
- **`macros`**: Required for `#[cached]` attribute macro
- **`serialization`**: JSON serialization only (serde + serde_json)
- **`compression`**: Data compression (flate2)
- **`tracing`**: Structured logging support
- **`metrics`**: OpenTelemetry metrics and observability
- **`batch-write`**: Buffered L2 writes (tokio-util)
- **`lua-script`**: Lua script execution support (requires `redis`)
- **`cli`**: Command-line interface (clap)
- **`testing`**: Testing support utilities
- **`bloom-filter`**: Negative-query filtering (NOT included in `full`)

### Example Configurations

```toml
# Full features (recommended, default)
oxcache = { version = "0.3.3", features = ["full"] }

# Core functionality only (L1 + L2)
oxcache = { version = "0.3.3", features = ["core"] }

# Minimal - L1 cache only
oxcache = { version = "0.3.3", features = ["minimal"] }

# Custom selection (e.g. add bloom-filter on top of core)
oxcache = { version = "0.3.3", features = ["core", "macros", "bloom-filter"] }
```

### Feature Dependencies

Some features require or imply other features:

| Feature | Required Features | Description |
|---------|-------------------|-------------|
| `lua-script` | `redis` | Lua script execution |
| `cli` | `metrics`, `dashmap`, `tracing` | Command-line interface |
| `core` | `minimal`, `redis`, `futures` | Core L1 + L2 cache |
| `full` | `core`, `macros`, `compression`, `batch-write`, `lua-script`, `cli`, `testing` | All features (note: `bloom-filter` is NOT in `full`) |

## Cache Macro

### `#[cached]` Attribute Macro

Zero-boilerplate caching decorator for functions. Requires the `macros` feature.

**Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `service` | `&str` | No | `"default"` | Cache service name (used to look up a registered `Cache` instance) |
| `ttl` | `u64` | No | `None` | Time-to-live in seconds (`Some(n)` if set) |
| `key` | `&str` | No | Auto-generated | Custom cache key format |
| `key_prefix` | `&str` | No | `""` | Prefix for the auto-generated cache key |
| `sync` | (flag) | No | async | Use the synchronous code path (`get_bytes_sync`/`set_bytes_sync`); cannot be combined with `async fn` |

The macro retrieves a `Cache` instance from the internal registry via
`oxcache::__internal_get_cache(service)`. If no cache is registered under
`service`, the original function runs uncached. The `Cache` must be built with
`sync_mode(true)` when using the `sync` flag.

**Example (async):**

```rust
// Cargo.toml: oxcache = { version = "0.3.3", features = ["macros"] }
use oxcache::cached;

#[cached(service = "default", ttl = 3600)]
async fn fetch_user(user_id: &str) -> Result<User, String> {
    // Function body
    # Ok(User { /* ... */ })
}
```

**Custom Key Format:**

```rust
#[cached(service = "default", ttl = 3600, key = "user:{user_id}")]
async fn fetch_user(user_id: &str) -> Result<User, String> {
    // Function body
    # Ok(User { /* ... */ })
}
```

**Default key generation** when neither `key` nor `key_prefix` is set:
`{service}:{fn_name}:{arg1:arg2:...}`.

## Cache<K, V>

The main type-safe cache type. `K: CacheKey`, `V: Serialize + Deserialize`.

**Construction:**

```rust
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct User { id: u64, name: String }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Default Moka memory backend (capacity 10000)
    let cache: Cache<String, User> = Cache::builder().build().await?;

    // Register for #[cached] macro usage
    cache.register_for_macro("default").await?;
    Ok(())
}
```

**Constructors:**

| Constructor | Feature | Description |
|-------------|---------|-------------|
| `Cache::builder()` | — | Returns a `CacheBuilder<K, V>` |
| `Cache::new()` | `memory` | Default Moka backend (sync, no `.await`) |
| `Cache::memory().await` | — | Convenience: default Moka backend |
| `Cache::redis(url).await` | `redis` | Convenience: Redis backend |
| `Cache::with_dependencies(backend)` | — | Construct from `Arc<dyn CacheBackend>` |

### Async Operations

#### `get(&self, key: &K) -> Result<Option<V>>`

Get a value from the cache. Returns `Ok(None)` if not found.

```rust
let user: Option<User> = cache.get(&"user:1".to_string()).await?;
```

#### `set(&self, key: &K, value: &V) -> Result<()>`

Set a value with no per-entry TTL (uses the backend's global TTL if any).

```rust
cache.set(&"user:1".to_string(), &user).await?;
```

#### `set_with_ttl(&self, key: &K, value: &V, ttl: Option<Duration>) -> Result<()>`

Set a value with an optional per-entry TTL. `Some(d)` overrides the backend's
global TTL for this entry; `None` falls back to the global TTL.

```rust
use std::time::Duration;
cache.set_with_ttl(&"user:1".to_string(), &user, Some(Duration::from_secs(3600))).await?;
```

#### `delete(&self, key: &K) -> Result<()>`

Delete a value from the cache.

#### `exists(&self, key: &K) -> Result<bool>`

Check if a key exists in the cache.

#### `clear(&self) -> Result<()>`

Clear all entries.

#### `get_or<F, Fut>(&self, key: &K, fallback: F) -> Result<V>`

Get a value or compute it with `fallback` (single-flight: concurrent callers
for the same key share one computation).

```rust
let user = cache.get_or(&"user:1".to_string(), || async {
    fetch_user_from_db(1).await
}).await?;
```

#### `ttl(&self, key: &K) -> Result<Option<Duration>>`

Get the remaining time-to-live for a key. Returns `Ok(None)` if the key has no
per-entry TTL or does not exist. Useful for update-with-preserving-TTL flows:

```rust
let original_ttl = cache.ttl(&"user:1".to_string()).await?;
cache.set_with_ttl(&"user:1".to_string(), &new_user, original_ttl).await?;
```

#### `expire(&self, key: &K, ttl: Duration) -> Result<bool>`

Update the TTL of an existing key without touching its value. Returns `Ok(true)`
if the TTL was updated, `Ok(false)` if the key does not exist.

### Lifecycle and Stats

| Method | Returns | Description |
|--------|---------|-------------|
| `health_check().await` | `Result<()>` | Health check for the backend |
| `stats().await` | `Result<HashMap<String,String>>` | Backend-specific statistics |
| `len().await` | `Result<u64>` | Number of entries |
| `is_empty().await` | `Result<bool>` | Whether the cache is empty |
| `capacity().await` | `Result<u64>` | Configured capacity (0 for Redis) |
| `shutdown().await` | `()` | Shutdown and release resources |
| `register_for_macro(service).await` | `Result<()>` | Register for `#[cached]` macro |

## CacheBuilder

`CacheBuilder<K, V>` is the unified builder for `Cache`. Obtain one via
`Cache::builder()`.

**Methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `backend_arc` | `(backend: Arc<dyn CacheBackend>) -> Self` | Add a pre-built backend |
| `ttl` | `(ttl: Duration) -> Self` | Default TTL for cache entries |
| `tti` | `(tti: Duration) -> Self` | Default TTI (time-to-idle) for memory backends |
| `capacity` | `(capacity: u64) -> Self` | Capacity for memory-based backends (default 10000) |
| `sync_mode` | `(enabled: bool) -> Self` | Enable the synchronous API (see [Synchronous API](#synchronous-api)) |
| `build` | `async (self) -> Result<Cache<K, V>>` | Build the cache instance |

> **Note:** There is no `.redis(...)`, `.tiered(...)`, `.with_backend(...)`,
> `.batch_writes(...)`, or `.auto_promote(...)` method on `CacheBuilder`. Use
> `.backend_arc(Arc::new(...))` to plug in a `RedisBackend` or other backend.

**Default Moka path** (when `backend_arc` is not called): builds a
`MokaMemoryBackend` with the given `capacity`/`ttl`/`tti`. This is the only
configuration that supports `sync_mode(true)`.

**Examples:**

```rust
use oxcache::{Cache, CacheBuilder};
use std::time::Duration;

// L1-only memory cache
let cache: Cache<String, String> = Cache::builder()
    .capacity(10000)
    .ttl(Duration::from_secs(3600))
    .tti(Duration::from_secs(600))
    .build()
    .await?;
```

```rust
use oxcache::{Cache, backend::RedisBackend};
use std::sync::Arc;

// L2 (Redis) cache via a pre-built backend
let redis = RedisBackend::new("rediss://localhost:6379").await?;
let cache: Cache<String, String> = Cache::builder()
    .backend_arc(Arc::new(redis))
    .build()
    .await?;
```

**Sync API limitation:** `sync_mode(true)` combined with `backend_arc(...)`
returns `Err(CacheError::NotSupported)` because `Arc<dyn CacheBackend>` cannot
be upcast to `Arc<dyn SyncCacheBackend>` on stable Rust (no `trait_upcasting`).
Use the default Moka backend with `sync_mode`, or wire up the sync backend
manually via `Cache::with_dependencies` + `set_sync_backend`.

## Backend Layer

The `backend` module exposes the backend traits and implementations.

### Backend Traits

```rust
#[async_trait]
pub trait CacheReader: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn ttl(&self, key: &str) -> Result<Option<Duration>>;
    async fn len(&self) -> Result<u64>;
    async fn is_empty(&self) -> Result<bool>;
    async fn capacity(&self) -> Result<u64>;
    async fn stats(&self) -> Result<HashMap<String, String>>;
    async fn get_many(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>>;
}

#[async_trait]
pub trait CacheWriter: Send + Sync {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn clear(&self) -> Result<()>;
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool>;
    async fn set_many(&self, items: &[(String, Vec<u8>, Option<Duration>)]) -> Result<()>;
    async fn delete_many(&self, keys: &[String]) -> Result<()>;
}

#[async_trait]
pub trait CacheConnector: Send + Sync {
    async fn health_check(&self) -> Result<()>;
    async fn shutdown(&self);
    fn backend_kind(&self) -> BackendKind;
}

// Blanket impl: any type implementing all three is a CacheBackend.
pub trait CacheBackend: CacheReader + CacheWriter + CacheConnector {}
```

### Backend Types

| Type | Feature | Description |
|------|---------|-------------|
| `MokaMemoryBackend` | `memory` | In-memory cache using Moka (LRU/TinyLFU eviction). Supports per-entry TTL via `moka::Expiry`. |
| `DashMapMemoryBackend` | `memory` | Pure in-memory concurrent cache using DashMap (lazy TTL expiration). |
| `RedisBackend` | `redis` | Distributed cache using Redis (Standalone/Sentinel/Cluster). |
| `ChainCache` | — | Multi-level cache chain (see [ChainCache](#chaincache)). |
| `BloomFilterBackend` | `bloom-filter` | Decorator that skips the inner backend on Bloom-filter miss. |

### Memory Backend Helpers

```rust
use oxcache::backend::{moka_memory, dashmap_memory, default_memory_backend, MokaMemoryBackend};

let moka = MokaMemoryBackend::builder().capacity(10000).build();
```

`MokaMemoryBackend::builder()` exposes `.capacity(u64)`, `.ttl(Duration)`,
`.time_to_idle(Duration)`, and `.build()` (sync).

### Backend Scores

Each backend reports a `score()` (higher = faster) used by `ChainCache` to
order reads/writes: Moka=100, DashMap=90, Redis=50. `is_persistent()` is
`true` for Redis.

## RedisBackend

`RedisBackend` is the L2 distributed cache. It uses `redis::aio::ConnectionManager`
for connection pooling and is re-exported at `oxcache::backend::RedisBackend`.

**Security:** Connections must use TLS (`rediss://`). Non-TLS connections are
rejected unless the environment variable
`OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS` (or `=development-only`)
is set.

### Constructors

```rust
use oxcache::backend::RedisBackend;

// From a connection string (TLS recommended)
let backend = RedisBackend::new("rediss://localhost:6379").await?;

// With an explicit pool size (currently equivalent to new())
let backend = RedisBackend::with_pool("rediss://localhost:6379", 8).await?;

// Via the builder
let backend = RedisBackend::builder()
    .connection_string("rediss://localhost:6379")
    .mode(oxcache::backend::RedisMode::Standalone)
    .build()
    .await?;
```

**`RedisBackendBuilder` methods:**

| Method | Description |
|--------|-------------|
| `connection_string(&str)` | Set the Redis connection string |
| `mode(RedisMode)` | Set the Redis mode (`Standalone`/`Sentinel`/`Cluster`) |
| `build().await` | Build the `RedisBackend` (2s connection timeout) |

### Instance Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `ping().await` | `Result<String>` | Ping the server (returns `"PONG"`) |
| `mode()` | `RedisMode` | Configured Redis mode |
| `client()` | `&Client` | Underlying `redis::Client` |
| `redact_connection_string(s)` | `String` (assoc) | Redact the password in a connection string |

### Pipeline Batch Operations

For high-throughput scenarios, use Redis pipelines (single round-trip):

```rust
// Batch SET
backend.set_many_pipeline(&[("k1", v1), ("k2", v2)], Some(Duration::from_secs(60))).await?;

// Batch GET
let values: Vec<Option<Vec<u8>>> = backend.get_many_pipeline(&["k1", "k2"]).await?;

// Batch DEL
backend.delete_many_pipeline(&["k1", "k2"]).await?;
```

### Lua Scripting (`lua-script` feature)

When the `lua-script` feature is enabled, `RedisBackend` implements `LuaExecutor`:

```rust
use oxcache::backend::interface::LuaExecutor;

// EVAL
let val = backend.eval_lua("return redis.call('GET', KEYS[1])", &["k1"], &[]).await?;

// SCRIPT LOAD + EVALSHA
let sha = backend.script_load("return 1 + 1").await?;
let val = backend.eval_sha(&sha, &[], &[]).await?;
```

All Lua scripts are validated via `validate_lua_script` before execution.

## ChainCache

`ChainCache` manages multiple backends ordered by score (descending). Reads
scan from the highest-score backend; writes fan out to all backends. Backfill
optionally populates higher-score backends on a lower-score hit.

```rust
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::backend::{MokaMemoryBackend, RedisBackend};
use std::time::Duration;

let l1 = MokaMemoryBackend::builder().capacity(10000).ttl(Duration::from_secs(300)).build();
let l2 = RedisBackend::new("rediss://localhost:6379").await?;

let chain = ChainCache::builder()
    .link(ChainLink::from_backend(l1))   // L1, score 100
    .link(ChainLink::from_backend(l2))   // L2, score 50
    .enable_backfill()
    .default_time_to_live(Duration::from_secs(600))
    .build();  // sync build, returns ChainCache

chain.set("key", b"value".to_vec(), None).await?;
let v = chain.get("key").await?; // Some(Vec<u8>)
```

### `ChainCacheBuilder`

| Method | Description |
|--------|-------------|
| `link(ChainLink)` | Add one link |
| `links(Vec<ChainLink>)` | Add multiple links |
| `backend(B)` | Add a backend (auto-wraps via `ChainLink::from_backend`) |
| `default_time_to_live(Duration)` | Default TTL used when `set` is called with `ttl=None` |
| `enable_backfill()` / `disable_backfill()` | Toggle backfill (off by default) |
| `build()` | Build the `ChainCache` (sync; sorts links by score descending) |

### `ChainLink`

| Constructor | Description |
|-------------|-------------|
| `new(backend, score, is_persistent, name)` | Manual construction |
| `from_backend(B)` | Auto-derive score/persistent/name from `BackendScore` (async API only) |
| `from_sync_backend(B)` | Like `from_backend` but also fills the sync backend handle (requires `SyncCacheBackend`) |

`ChainLink` accessors: `backend()`, `try_as_sync_backend()`, `score()`,
`is_persistent()`, `name()`.

### TTL Behavior

`ChainCache` does not store TTL itself; it transparently forwards to links:

- `set(key, value, Some(d))` → all links use the same TTL `d`.
- `set(key, value, None)` → links use `default_ttl` if set, else each link's own global TTL.
- `ttl(key)` → returns the first `Some(ttl)` found scanning highest-score-first.
- `expire(key, d)` → forwards to all links; returns `Ok(true)` if any link succeeds.

### Sync API

`ChainCache` exposes `get_sync`/`set_sync`/`delete_sync`. These require **every**
link to support `SyncCacheBackend` (i.e. be built via `from_sync_backend`);
otherwise they return `Err(CacheError::NotSupported)`.

## Synchronous API

The synchronous API mirrors the async API but blocks via
`tokio::task::block_in_place`. It requires a **multi-thread** Tokio runtime
(calling it from a current-thread runtime returns `Err(NotSupported)`).

### `SyncCacheBackend` Trait Hierarchy

```rust
pub trait SyncCacheReader { fn get(&self, key: &str) -> Result<Option<Vec<u8>>>; /* ... */ }
pub trait SyncCacheWriter { fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>; /* ... */ }
pub trait SyncCacheConnector { fn health_check(&self) -> Result<()>; /* ... */ }
pub trait SyncCacheBackend: SyncCacheReader + SyncCacheWriter + SyncCacheConnector {}
```

Implemented by: `MokaMemoryBackend`, `DashMapMemoryBackend`, `RedisBackend`.

### Enabling Sync on `Cache<K, V>`

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await?;

    // Synchronous methods are now available
    cache.set_sync(&"k".to_string(), &"v".to_string())?;
    let v = cache.get_sync(&"k".to_string())?; // Some("v")
    Ok(())
}
```

### Sync Methods on `Cache<K, V>`

| Method | Description |
|--------|-------------|
| `get_sync(&K)` | `Result<Option<V>>` |
| `set_sync(&K, &V)` | `Result<()>` (no per-entry TTL) |
| `set_with_ttl_sync(&K, &V, Option<Duration>)` | `Result<()>` |
| `delete_sync(&K)` | `Result<()>` |
| `exists_sync(&K)` | `Result<bool>` |
| `ttl_sync(&K)` | `Result<Option<Duration>>` |
| `expire_sync(&K, Duration)` | `Result<bool>` |
| `get_or_sync(&K, fallback)` | `Result<V>` (single-flight, sync) |
| `clear_sync()` | `Result<()>` |

When `sync_mode` is `false` (the default), all `*_sync` methods return
`Err(CacheError::NotSupported)`.

## Bloom Filter

The `bloom-filter` feature (NOT included in `full`) provides negative-query
filtering. `BloomFilterBackend` wraps any `CacheBackend` and skips the inner
backend when the Bloom filter reports a key as absent.

### `BloomFilter`

```rust
use oxcache::features::bloom_filter::BloomFilter;

let bf = BloomFilter::new(100_000, 0.01); // capacity, false-positive rate
bf.insert("key1");
assert!(bf.contains("key1"));     // inserted keys always return true
assert!(!bf.contains("absent"));  // usually false (may false-positive)
```

**Methods:** `insert(&str)`, `contains(&str) -> bool`, `clear()`, `len() -> u64`,
`is_empty() -> bool`, `capacity() -> usize`, `false_positive_rate() -> f64`,
`load_factor() -> f64`, `rebuild(new_capacity)`.

`BloomFilter` is `Clone` and shares state via `Arc<RwLock<...>>`, so inserts on
one clone are visible to all clones.

### `BloomFilterBackend`

```rust
use oxcache::backend::MokaMemoryBackend;
use oxcache::features::bloom_filter::{BloomFilterBackend, BloomFilterBackendBuilder};

let inner = MokaMemoryBackend::new();
let backend = BloomFilterBackend::new(inner);  // decorator over the inner backend
```

`BloomFilterBackendBuilder` allows configuring the underlying `BloomFilter`
(capacity, false-positive rate). The decorator implements `CacheBackend`, so it
can be used anywhere a `CacheBackend` is expected (including `ChainCache` links
and `CacheBuilder::backend_arc`).

## TTL Management

All backends (Moka, DashMap, Redis, Mock, Chain, Bloom) honor per-entry TTL via
`set(key, value, Some(ttl))`.

- **Moka** uses the `moka::Expiry` trait for real per-entry TTL, overriding the
  global TTL set on the builder.
- **DashMap** uses lazy expiration (entries expire on access).
- **Redis** uses `SETEX`.

### TTL Methods

| Method | Async | Sync |
|--------|-------|------|
| Read remaining TTL | `cache.ttl(&key).await` | `cache.ttl_sync(&key)` |
| Update TTL of existing key | `cache.expire(&key, d).await` | `cache.expire_sync(&key, d)` |
| Set with explicit TTL | `cache.set_with_ttl(&key, &v, Some(d)).await` | `cache.set_with_ttl_sync(&key, &v, Some(d))` |

## Security Features

Security functions are re-exported at the **crate root** (when the `redis` or
`full` feature is enabled), not under `oxcache::security`:

```rust
use oxcache::{
    validate_redis_key, validate_lua_script, validate_scan_pattern, clamp_scan_count,
    redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted,
    // secure logging helpers:
    // log_cache_key, sanitize_message,
};
```

> **Import path note:** Use `use oxcache::validate_redis_key` (the crate-root
> re-export), not `use oxcache::security::validate_redis_key`. The `security`
> module itself is `pub(crate)` and not directly accessible.

### `validate_redis_key(key: &str) -> Result<()>`

Validate Redis key format and content.

```rust
use oxcache::validate_redis_key;

validate_redis_key("user:123").expect("Valid key");
// Empty, too long, or dangerous keys return Err(CacheError::InvalidInput)
```

**Validation rules:**
- Key cannot be empty
- Key cannot exceed 512KB
- Key cannot contain dangerous characters (`\r`, `\n`, `\0`, `;`, `|`)
- Key is scanned for SQL injection and path traversal patterns (`../`, `etc/passwd`)

### `validate_lua_script(script: &str, num_keys: usize) -> Result<()>`

Validate a Lua script for security issues.

```rust
use oxcache::validate_lua_script;

validate_lua_script("return redis.call('GET', KEYS[1])", 1).expect("Valid script");
```

**Validation rules:**
- Script length cannot exceed 10KB
- Number of keys cannot exceed 100
- Dangerous commands are blocked: `FLUSHALL`, `FLUSHDB`, `KEYS`, `SHUTDOWN`, `DEBUG`, `CONFIG`, `SAVE`, `BGSAVE`, `MONITOR`
- Comment preprocessing prevents bypass via comments

### `validate_scan_pattern(pattern: &str) -> Result<()>`

Validate a SCAN pattern to prevent ReDoS attacks.

```rust
use oxcache::validate_scan_pattern;

validate_scan_pattern("user:*").expect("Valid pattern");
```

**Validation rules:**
- Pattern length cannot exceed 256 characters
- Maximum of 10 wildcard (`*`) characters

### `clamp_scan_count(count: u64) -> u64`

Clamp a SCAN `COUNT` parameter to the safe range (1–1000).

### Sensitive Data Redaction

```rust
use oxcache::{redact_connection_string, redact_cache_key, redact_value, Redacted};

let safe = redact_connection_string("redis://:secret@host:6379");
assert!(!safe.contains("secret"));
```

`Redacted` is a wrapper type that prevents accidental logging of the inner
value. Use `redact_field` / `redact_value` to wrap sensitive data in logs.

## Observability

### Metrics (`metrics` feature)

Cache statistics and export helpers are re-exported at the crate root:

```rust
use oxcache::{CacheStats, get_enhanced_stats, export_json_format, export_prometheus_format};

let stats: CacheStats = get_enhanced_stats();
println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);

let prometheus_text = export_prometheus_format();
let json_text = export_json_format();
```

`CacheStats` exposes hit/miss counts, hit rate, and per-operation latency.
`MetricsCollector` (in `oxcache::infra::metrics::backend`) provides low-level
counters (L1/L2 hits/misses, operation counters).

### Tracing (`tracing` feature)

Oxcache uses the `tracing` crate for structured, instrumentation-based logging.
Spans are attached to `get`/`set`/`delete`/`ttl`/`expire` operations when the
`tracing` feature is enabled.

```rust
use tracing::{info, warn, error};

info!("Cache initialized");
warn!("Redis connection lost, operating in L1-only mode");
error!("Failed to write to cache: {}", err);
```

OpenTelemetry integration (OTLP export) is available via the `metrics` feature,
which pulls in `opentelemetry`, `tracing-opentelemetry`, and `opentelemetry-otlp`.

## Error Handling

### `CacheError`

All cache operations return `Result<T, CacheError>` (aliased as `oxcache::Result<T>`).

```rust
use oxcache::CacheError;

match result {
    Ok(value) => /* ... */,
    Err(CacheError::NotFound(key)) => println!("Key not found: {}", key),
    Err(CacheError::NotSupported(msg)) => println!("Not supported: {}", msg),
    Err(CacheError::Connection(msg)) => println!("Connection error: {}", msg),
    Err(CacheError::Serialization(msg)) => println!("Serialization error: {}", msg),
    Err(e) => println!("Other error ({}): {}", e.code(), e),
}
```

### Error Variants and Codes

| Variant | Code | Recoverable | Description |
|---------|------|-------------|-------------|
| `NotFound(String)` | `CACHE_001` | no | Key not found |
| `Connection(String)` | `CACHE_002` | yes | Network connection failure |
| `Serialization(String)` | `CACHE_003` | no | Serialization/deserialization failure |
| `Operation(String)` | `CACHE_004` | no | General operation failure |
| `Degraded(String)` | `CACHE_005` | no | Cache in degraded mode |
| `L1Error(String)` | `CACHE_006` | no | L1 cache operation failed |
| `L2Error(String)` | `CACHE_007` | yes | L2 cache operation failed |
| `NotSupported(String)` | `CACHE_009` | no | Operation not supported (e.g. sync API without `sync_mode`) |
| `WalError(String)` | `CACHE_010` | no | WAL operation failed |
| `DatabaseError(String)` | `CACHE_011` | no | Database error |
| `RedisError(...)` | `CACHE_012` | yes | Redis error |
| `IoError(io::Error)` | `CACHE_013` | no | I/O error |
| `BackendError(String)` | `CACHE_014` | yes | Backend error |
| `Timeout(String)` | `CACHE_015` | yes | Operation timed out |
| `ShutdownError(String)` | `CACHE_016` | no | Shutdown error |
| `KeyTooLong(usize, usize)` | `CACHE_017` | no | Key exceeds max length |
| `ValueTooLarge(usize, usize)` | `CACHE_018` | no | Value exceeds max size |
| `BufferFull(String)` | `CACHE_019` | yes | Batch write buffer full |
| `InvalidInput(String)` | `CACHE_020` | no | Invalid input |
| `InvalidKey(String)` | `CACHE_021` | no | Invalid key |
| `LockError(String)` | `CACHE_022` | no | Lock poisoned |
| `ServiceNotFound(String)` | `CACHE_023` | no | Service not in registry |
| `Internal(String)` | `CACHE_024` | no | Internal state corruption |

### Helper Methods

- `CacheError::code() -> &'static str`: stable error code (e.g. `"CACHE_009"`).
- `is_recoverable() -> bool`: `true` for `Connection`/`Timeout`/`L2Error`/`BackendError`/`BufferFull`.
- `is_not_found() -> bool`, `is_connection_error() -> bool`, `is_degraded() -> bool`.

### `CacheConfigError` (`redis` feature)

Configuration-phase errors (returned by factory functions and builders):

```rust
pub enum CacheConfigError {
    MissingField(String),
    InvalidValue { field: String, reason: String },
    UnsupportedBackend(String),
    ConnectionFailed(String),
}
pub type ConfigResult<T> = std::result::Result<T, CacheConfigError>;
```

## Type Aliases

```rust
pub type Result<T> = std::result::Result<T, CacheError>;
pub type RedisMode = RedisModeType; // Standalone | Sentinel | Cluster
```

## Examples

See the [examples/](../examples/) directory for more usage examples:

- [Basic Operations](../examples/src/01_basics/)
- [Advanced Features](../examples/src/02_advanced/)
- [Configuration](../examples/src/03_config/)
- [Database Integration](../examples/src/05_database/)
- [Feature Demos](../examples/src/06_features/)
