<div align="center">

# 📖 Oxcache User Guide

### Complete usage guide for the high-performance Rust multi-tier caching library (L1+L2+optional L3)

[🏠 Home](../../README.md) • [📚 Documentation Hub](../../README.md#-documentation) • [🎯 Examples](../../examples/) • [📘 API Reference](API_REFERENCE.md)

---

</div>

> **⚠️ Version note**: this document was written against **Oxcache v0.5.1**.

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [🧭 Introduction](#-introduction)
- [🚀 Getting Started](#-getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [First Steps](#first-steps)
- [🧱 Core Concepts](#-core-concepts)
- [📖 Basic Usage](#-basic-usage)
  - [Using the Cache Macro](#using-the-cache-macro)
  - [Manual Cache Control](#manual-cache-control)
  - [Serialization](#serialization)
- [⚡ Advanced Usage](#-advanced-usage)
  - [Chained Multi-Tier Cache (ChainCache)](#chained-multi-tier-cache-chaincache)
  - [Enabling the Sync API](#enabling-the-sync-api)
  - [Bloom Filter](#bloom-filter)
  - [TTL Management](#ttl-management)
  - [Redis Mode Configuration](#redis-mode-configuration)
  - [Metrics](#metrics)
  - [Event Emission (EventPublisher)](#event-emission-eventpublisher)
  - [Graceful Shutdown](#graceful-shutdown)
  - [Valkey Backend](#valkey-backend)
  - [Dragonfly Backend](#dragonfly-backend)
  - [Aerospike Backend](#aerospike-backend)
- [🌟 Best Practices](#-best-practices)
- [🔧 Troubleshooting](#-troubleshooting)
- [🎯 Next Steps](#-next-steps)
- [💻 Examples Overview](#-examples-overview)
- [🧩 wasm32 Support (experimental)](#-wasm32-support-experimental)

</details>

---

## 🧭 Introduction

**oxcache** is a high-performance, production-grade Rust caching library providing an L1 (in-process memory, Moka/DashMap) + L2 (distributed Redis/Valkey/Dragonfly/Aerospike) + optional L3 disk (redb) multi-tier architecture. It achieves
zero-boilerplate caching through the `#[cached]` macro, and supports the sync API, bloom filters and chained multi-tier backends.

| Learning track | Content | Outcome |
|---------|------|------|
| 🚀 Getting Started | Environment setup and your first cache | Integrated within 5 minutes |
| 🧱 Core Concepts | Multi-tier architecture, chained cache, sync API | Understand the design model |
| 📖 Basic Usage | Macro and manual control, serialization | Day-to-day read/write fluency |
| ⚡ Advanced Usage | ChainCache, bloom, TTL, metrics | Production-grade capability mastery |

Key features include:

- **🚀 Extreme performance**: nanosecond L1 responses (P99 50-200ns), millisecond L2 responses (P99 1-10ms, environment-dependent)
- **🔗 Chained multi-tier cache**: `ChainCache` orders reads and writes by backend score, with backfill support
- **⚡ Sync API**: `sync_mode(true)` enables `get_sync`/`set_sync` and other sync methods
- **🛡️ Built-in security**: key/Lua/SCAN validation, TLS enforcement, sensitive-information redaction
- **📊 Observability**: built-in metrics, EventPublisher event emission

> 💡 **Tip**: this guide assumes basic Rust knowledge. If you are new to Rust,
> consider reading the [official Rust book](https://doc.rust-lang.org/book/) first.

---

## 🚀 Getting Started

### Prerequisites

Before starting, make sure you have installed the following tools:

| Category | Tools |
|------|------|
| **Required** | ✅ Rust 1.97.1+ (stable)　✅ Cargo (installed with Rust)　✅ Git |
| **Optional** | 🔧 A Rust-capable IDE (e.g. VS Code + rust-analyzer)　🔧 Docker (for containerized deployment)　🔧 Redis 6.0+ (for L2 cache testing) |

<details>
<summary><b>🔍 Verify the installation</b></summary>

```bash
# Check the Rust version
rustc --version
# Expected: rustc 1.97.1 (or newer)

# Check the Cargo version
cargo --version
# Expected: cargo 1.97.1 (or newer)
```

</details>

### Installation

Add `oxcache` to your `Cargo.toml`:

```toml
[dependencies]
oxcache = "0.5.1"
```

> **Note**: `default = ["minimal"]`; by default only the L1 in-memory cache is included. To use the full feature set, explicitly enable `features = ["full"]`.

> **Features**: to use the `#[cached]` macro, enable the `macros` feature: `oxcache = { version = "0.5.1", features = ["macros"] }` (already included in `full`).

#### Feature tiers and dependencies

For the tiered presets (`minimal` / `redis-tier` / `full`) and the complete feature list, see the [Feature Flags table of the API Reference](API_REFERENCE.md#-feature-flags); for feature prerequisites (e.g. `lua` requires `redis`, `red-lock` requires `lock`; opt-in features such as `bloom` / `trait-kit` are now part of `full` and can still be enabled alone), see the [Feature Requirements of the API Reference](API_REFERENCE.md#-feature-requirements).

If you need minimal dependencies or custom features:

```toml
[dependencies]
oxcache = { version = "0.5.1", default-features = false, features = ["redis-tier"] }
```

Or use the command line:

```bash
cargo add oxcache
```

### First Steps

Let's verify the installation with a simple example. We'll create a cache using `Cache::builder()`:

#### Option 1: L1 in-memory cache (recommended for getting started)

```rust
use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an L1 in-memory cache with Cache::builder() (default Moka backend)
    let cache: Cache<String, User> = Cache::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    // First call: writes to the cache
    let user = User { id: 1, name: "User 1".to_string() };
    cache.set(&"user:1".to_string(), &user).await?;
    println!("First call: {:?}", user);

    // Second call: reads from the cache
    let cached_user: Option<User> = cache.get(&"user:1".to_string()).await?;
    println!("Cached call: {:?}", cached_user);

    Ok(())
}
```

#### Option 2: using the #[cached] macro

```rust
use oxcache::cached;  // re-exported at the crate root
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
    id: u64,
    name: String,
}

// Enable caching in one line via the #[cached] macro
// Parameters: service (registered name), key (key template), ttl (seconds)
#[cached(service = "user_cache", key = "user:{id}", ttl = 600)]
async fn get_user(id: u64) -> Result<User, String> {
    // Simulate a slow database query
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("User {}", id),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the cache (default Moka backend; register_for_macro only supports Cache<String, Vec<u8>>)
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

    // Register the cache instance in the global manager (the macro looks it up by service name)
    cache.register_for_macro("user_cache").await?;

    // First call: runs the function logic + caches the result (~100ms)
    let user = get_user(1).await?;
    println!("First call: {:?}", user);

    // Second call: returned directly from the cache (~0.1ms)
    let cached_user = get_user(1).await?;
    println!("Cached call: {:?}", cached_user);

    Ok(())
}
```

#### Option 3: L2 Redis cache

```rust
use oxcache::Cache;
use oxcache::backend::RedisBackend;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the Redis backend (use a rediss:// TLS connection in production)
    // For development, set: OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
    let redis = RedisBackend::new("rediss://127.0.0.1:6379").await?;

    // Inject the backend via backend_arc
    let cache: Cache<String, String> = Cache::builder()
        .backend_arc(Arc::new(redis))
        .build()
        .await?;

    cache.set(&"key".to_string(), &"value".to_string()).await?;

    let val: Option<String> = cache.get(&"key".to_string()).await?;
    println!("Value: {:?}", val);

    Ok(())
}
```

> **Note**: `Cache::builder()` has no `.redis(...)` or `.tiered(...)` method. To use Redis, construct the backend with `RedisBackend::new(url).await?` and inject it via `.backend_arc(Arc::new(redis))`. For L1+L2 tiering, use the [ChainCache](#chained-multi-tier-cache-chaincache).

---

## 🧱 Core Concepts

Understanding these core concepts will help you use `oxcache` more effectively.

### Multi-tier cache architecture (L1+L2+optional L3)

The core of `oxcache` is a two-tier (plus optional third tier) cache architecture: L1 (Moka) + L2 (Redis). L1 is local in-memory caching with extremely fast access; L2 is a distributed cache shared across instances.

- **L1 (Moka)**: in-process high-speed cache using LRU/TinyLFU eviction, with per-entry TTL support (via `moka::Expiry`)
- **L2 (Redis)**: distributed shared cache supporting Standalone/Sentinel/Cluster modes

### Chained tiers and backfill

`ChainCache` orders multiple backends by score (from high to low): Moka=100, DashMap=90, Redis=50.
Reads start from the highest-scored backend; writes go to all backends. When `enable_backfill()` is enabled, a hit on a lower-scored backend backfills the higher-scored one, reducing later access to the low-scored backend.

### Sync API

Enabled via `Cache::builder().sync_mode(true)`, providing a synchronous mirror of the async API. For the three-path semantics and runtime requirements (default Moka native sync, the `sync_backend_arc(...)` native sync surface, and `backend_arc(...)` bridged through `AsyncToSyncBridge` — the bridged surface requires a multi_thread runtime), see [Enabling the Sync API](#enabling-the-sync-api) and the [API Reference](API_REFERENCE.md#-sync-api).

### Fault tolerance and single flight

- **Single-Flight**: `get_or` / `get_or_sync` deduplicate concurrent requests for the same key, executing the computation only once
- **Fault-tolerant degradation**: when L2 is unavailable, chained-cache reads skip the failing backend and continue fetching from the others

### Universal per-entry TTL

All backends (Moka / DashMap / Redis / Valkey (via RedisBackend) / Dragonfly / Aerospike / Disk / Mock / Chain / Bloom) support `set(key, value, Some(ttl))` for per-entry TTLs; for the set, read and modify methods, see [TTL Management](#ttl-management).

---

## 📖 Basic Usage

### Using the Cache Macro

Use the `#[cached]` macro to add caching to functions (requires the `macros` feature):

```rust
use oxcache::cached;  // re-exported at the crate root
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    id: u64,
    name: String,
}

// Cache results automatically, with cache key "user:{id}"
// Parameters: service (registered name), key (key template), ttl (seconds)
#[cached(service = "user_cache", key = "user:{id}", ttl = 300)]
async fn get_user(id: u64) -> Result<User, String> {
    // Your business logic goes here, e.g. a database query
    database::query_user(id).await
}
```

The macro looks up the `Cache` instance by `service` name from the internal registry. If not registered, the original function runs normally (no caching).

For the full explanation of macro parameters (`service` / `ttl` / `key` / `key_prefix` / `scope` / `sync` / `skip_cache_write` / `single_flight` / `strict` / `condition` / `skip` / `cache_none`), see the [API Reference](API_REFERENCE.md#-cache-macro).

Methods with a `self` receiver (`&self` / `&mut self`) must provide an ownership dimension, otherwise they fail at compile time:

- `scope = self.tenant_id` (recommended): the scope expression becomes a segment of the key;
- `key = "user_{self.tenant_id}_{id}"`: the key template references parameters or `self` field paths.
  Constant templates (`key = "user_profile"`) fail at compile time on methods — they are unrelated to `self`,
  and all tenants would share the same cache entry.

### Manual Cache Control

You can also bypass the macro and use a `Cache` instance directly:

```rust
use oxcache::Cache;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the cache instance (default Moka backend)
    let cache: Cache<String, String> = Cache::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    // Standard operations
    cache.set(&"key".to_string(), &"value".to_string()).await?;

    let val: Option<String> = cache.get(&"key".to_string()).await?;
    assert_eq!(val, Some("value".to_string()));

    // Write with a per-entry TTL
    cache.set_with_ttl(&"key2".to_string(), &"value2".to_string(), Some(Duration::from_secs(60))).await?;

    // Delete from the cache
    cache.delete(&"key".to_string()).await?;

    // Check whether a key exists
    let exists = cache.exists(&"key".to_string()).await?;
    println!("Key exists: {}", exists);

    // Clear the cache
    cache.clear().await?;

    Ok(())
}
```

#### Operating backends directly

For finer control, construct and operate the underlying backends directly:

```rust
use oxcache::backend::{CacheWriter, MokaMemoryBackend, RedisBackend};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // L1: Moka in-memory backend
    let l1 = MokaMemoryBackend::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(60))
        .build();

    // L2: Redis backend
    let l2 = RedisBackend::new("rediss://127.0.0.1:6379").await?;

    // Write directly to L1 (temporary data)
    l1.set(Arc::from("temp_key"), Arc::new(b"temp_data".to_vec()), Some(Duration::from_secs(60))).await?;

    // Write directly to L2 (shared data)
    l2.set(Arc::from("shared_key"), Arc::new(b"shared_data".to_vec()), Some(Duration::from_secs(3600))).await?;

    Ok(())
}
```

### Serialization

The typed `Cache<K, V>`'s `get`/`set`/`set_with_ttl` all go through the serialization layer: the value type `V` must implement
`serde::Serialize + serde::Deserialize<'de>`, and the key type `K` must implement `oxcache::CacheKey`.

| Format | Feature | Description |
|------|------|------|
| **JSON** | default (`serialization`) | Readability and cross-language interop; `serde_stacker` depth guard against nested-JSON DoS |
| **bincode 2** | `serde-bincode` | Compact binary format, switched via `CacheBuilder::serialization_format()` |
| **postcard** | `postcard` | varint encoding; about 46% of JSON's size for mixed payloads |

```rust
use oxcache::infra::serialization::SerializationFormat;

// Requires the serde-bincode / postcard features (serialization_format needs serialization or full)
let cache: Cache<String, User> = Cache::builder()
    .serialization_format(SerializationFormat::Postcard)
    .build()
    .await?;
```

> **Note**: binary formats have no self-describing header; **formats must not be mixed under the same key prefix**. For measured L2 transfer sizes of each format, see the [Performance Baseline](PERFORMANCE.md).

---

## ⚡ Advanced Usage

### Chained Multi-Tier Cache (ChainCache)

`ChainCache` combines multiple backends into a chain, reading and writing in score order:

```rust
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::backend::{MokaMemoryBackend, RedisBackend};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let l1 = MokaMemoryBackend::builder().capacity(10000).ttl(Duration::from_secs(300)).build();
    let l2 = RedisBackend::new("rediss://127.0.0.1:6379").await?;

    let chain = ChainCache::builder()
        .link(ChainLink::from_backend(l1))   // L1, score 100
        .link(ChainLink::from_backend(l2))   // L2, score 50
        .enable_backfill()                   // backfills L1 on an L2 hit
        .default_time_to_live(Duration::from_secs(600))
        .build();  // synchronous build

    // Write to all backends (TTL passed through)
    chain.set("key", b"value".to_vec(), Some(Duration::from_secs(60))).await?;

    // Read: starts from L1, falls through to L2 on a miss, backfills L1 on a hit (when enabled)
    let v = chain.get("key").await?;
    println!("Value: {:?}", v);

    // Delete from all backends
    chain.delete("key").await?;

    Ok(())
}
```

**ChainCacheBuilder methods**: `link(ChainLink)`, `links(Vec<ChainLink>)`, `backend(B)`,
`default_time_to_live(Duration)`, `enable_backfill()` / `disable_backfill()`, `build()` (synchronous).

### Enabling the Sync API

Under a multi_thread runtime, enable `sync_mode(true)` to use the sync methods:

```rust
use oxcache::Cache;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await?;

    // Sync methods (no .await needed)
    cache.set_sync(&"k".to_string(), &"v".to_string())?;
    let v = cache.get_sync(&"k".to_string())?;
    assert_eq!(v, Some("v".to_string()));

    cache.delete_sync(&"k".to_string())?;
    Ok(())
}
```

For the full sync-method table and the `NotSupported` behavior when `sync_mode` is not enabled, see the [API Reference](API_REFERENCE.md#-sync-api).

Audit behavior: the sync path publishes audit events with the same semantics as the async path (`audit` feature).

### Bloom Filter

Enable the `bloom` feature (now part of `full`, and still available standalone) for negative-query filtering. The `BloomFilterBackend`
decorates any `CacheBackend`, skipping the inner backend when the bloom filter says the key is absent:

```rust
use oxcache::backend::MokaMemoryBackend;
use oxcache::features::bloom_filter::{BloomFilterBackend, BloomFilter};

let bf = BloomFilter::new(100_000, 0.01); // capacity 100k, false-positive rate 1%
let inner = MokaMemoryBackend::new();
let backend = BloomFilterBackend::new(inner);  // usable as a CacheBackend
```

For the full method list, builder usage and semantics of `BloomFilter` and `BloomFilterBackend`, see the [API Reference](API_REFERENCE.md#-bloom-filter).

### TTL Management

```rust
use std::time::Duration;

// Set a per-entry TTL
cache.set_with_ttl(&"k".to_string(), &v, Some(Duration::from_secs(60))).await?;

// Read the remaining TTL (None means no per-entry TTL or the key does not exist)
let ttl = cache.ttl(&"k".to_string()).await?;

// Modify an existing key's TTL (value untouched); returns true on success
let ok = cache.expire(&"k".to_string(), Duration::from_secs(120)).await?;

// Update the value but keep the original TTL
let original = cache.ttl(&"k".to_string()).await?;
cache.set_with_ttl(&"k".to_string(), &new_value, original).await?;
```

For the per-backend TTL behavior reference, see the [TTL Behavior Reference of the architecture documentation](ARCHITECTURE.md#-ttl-behavior-reference).

### Redis Mode Configuration

oxcache supports several Redis deployment modes. TLS (`rediss://`) is recommended for all modes:

#### Standalone mode

```rust
use oxcache::backend::RedisBackend;

let backend = RedisBackend::new("rediss://127.0.0.1:6379").await?;
```

#### Specifying the mode via the builder

```rust
use oxcache::backend::{RedisBackend, RedisMode};

let backend = RedisBackend::builder()
    .connection_string("rediss://127.0.0.1:6379")
    .mode(RedisMode::Standalone)  // or Sentinel / Cluster / ValkeyStandalone
    .build()
    .await?;
```

> **Security**: non-TLS connections are rejected unless the environment variable
> `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS` is set (development environments only).

### Metrics

With the `metrics` feature enabled, the crate-root re-exported `get_enhanced_stats` / `export_prometheus_format` / `export_json_format` read `CacheStats` (hit rates etc.) and export Prometheus / JSON text; the lower-level `MetricsCollector` (in `oxcache::infra::metrics::backend`) provides L1/L2 hit/miss counters and per-operation latency histograms. For a usage example, see the [observability chapter of the API Reference](API_REFERENCE.md#-observability).

### Event Emission (EventPublisher)

oxcache provides a structured event-emission mechanism through the `EventPublisher` trait (re-exported at the crate root; the `core` module is private): when a `ChainCache` backend operation fails, an event is raised through the configured `EventPublisher`, and the user decides how to handle it (logging, metrics, alerting, or ignoring). For implementation and configuration examples, see the [observability chapter of the API Reference](API_REFERENCE.md#-observability).

Enabling the `metrics` feature brings in the built-in metrics implementation (`serialization` + `chrono` + `dashmap`),
with no dependency on external OpenTelemetry crates. OTLP export, if needed, is handled uniformly at the application layer.

### Graceful Shutdown

```rust
use oxcache::Cache;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, String> = Cache::builder()
        .capacity(10000)
        .ttl(Duration::from_secs(3600))
        .build()
        .await?;

    // Your application logic
    cache.set(&"key".to_string(), &"value".to_string()).await?;

    // Graceful shutdown (releases backend resources)
    cache.shutdown().await;

    Ok(())
}
```

The shutdown mechanism calls the backend's `shutdown()`, cleaning up connections and other resources.

### Valkey Backend

[Valkey](https://valkey.io/) is the open-source fork of Redis, fully compatible with the Redis protocol.
`detect_valkey()` auto-detects whether the connection target is Redis or Valkey:

```rust
use oxcache::backend::{RedisBackend, RedisMode};

// Valkey uses the same API as Redis, identified via RedisMode::ValkeyStandalone
// TLS enforcement applies here as well: rediss:// is required (for the development escape hatch see the "Redis Mode Configuration" section above)
let backend = RedisBackend::builder()
    .connection_string("rediss://127.0.0.1:6379")
    .mode(RedisMode::ValkeyStandalone)
    .build()
    .await?;
```

### Dragonfly Backend

[Dragonfly](https://www.dragonflydb.io/) is a high-performance Redis/Memcached alternative.
Enable with the `dragonfly` feature:

```toml
# Cargo.toml
[dependencies]
oxcache = { version = "0.5.1", features = ["dragonfly"] }
```

```rust
use oxcache::backend::DragonflyBackend;

// Construct the Dragonfly backend (reuses RedisBackend's TLS enforcement: rediss:// required;
// or set OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS in development)
let dragonfly = DragonflyBackend::new("rediss://127.0.0.1:6379", 8).await?;
```

Composing it with Moka into a ChainCache is recommended (composition example and limitations in the [API Reference](API_REFERENCE.md#-dragonflybackend)).

### Aerospike Backend

[Aerospike](https://aerospike.com/) is a high-performance distributed KV store.
Enable with the `aerospike` feature:

```toml
# Cargo.toml
[dependencies]
oxcache = { version = "0.5.1", features = ["aerospike"] }
```

The backend is constructed via `AerospikeBackend::new(AerospikeConfig)`; for the configuration fields (`seed_nodes` / `namespace` / `set_name` / `default_ttl` / `ip_map`), see the [API Reference](API_REFERENCE.md#-aerospikebackend).

> **Docker environment note**: the Aerospike container's internal IP differs from the host's; configure an IP translation table via `ip_map`, or use `access-address` to configure the address the server advertises.

---

## 🌟 Best Practices

### ✅ Recommended

- **Set TTLs sensibly**: set expiry based on data update frequency to avoid inconsistency.
- **Use ChainCache for tiering**: hot data in L1 (Moka), shared data in L2 (Redis), with backfill enabled.
- **Monitor the hit rate**: check hit rates periodically and adjust capacity and TTL in time.
- **Use single flight against stampedes**: use `get_or` / `get_or_sync` for hot keys to avoid repeated fallbacks.
- **Separate cold and hot data**: L1 caches hot data, L2 caches shared data.

### ❌ Avoid

- **Caching overly large data**: avoid caching large objects; prefer metadata and IDs.
- **Ignoring expiry policy**: set TTLs sensibly to avoid stale data.
- **Single points of failure**: always use Sentinel or Cluster modes in production.
- **Ignoring monitoring**: enable metrics collection and monitoring to catch problems early.

### 🔒 Security Best Practices

oxcache ships multiple layers of built-in security protection; the following security best practices are recommended in production:

#### 1. Sensitive-information protection

- **Store passwords in environment variables**: never hard-code passwords in configuration files

  ```toml
  # ✅ correct practice
  connection_string = "rediss://:${REDIS_PASSWORD}@localhost:6379/0"

  # ❌ wrong practice
  connection_string = "rediss://:mypassword123@localhost:6379/0"
  ```

- **Enable log redaction**: oxcache automatically redacts sensitive information in logs, ensuring production logs contain no passwords

#### 2. Connection security

- **Use TLS encryption**: production must use `rediss://`, otherwise oxcache refuses the connection

  ```rust
  // ✅ production
  let backend = RedisBackend::new("rediss://:${REDIS_PASSWORD}@localhost:6380").await?;

  // ❌ development requires explicit risk acknowledgement
  // OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
  let backend = RedisBackend::new("redis://127.0.0.1:6379").await?;
  ```

- **Use strong passwords**: Redis passwords should be at least 32 characters, with upper/lowercase letters, digits and special characters

#### 3. Access control

- **Configure network isolation**: restrict Redis access sources with a firewall
- **Use ACL**: Redis ACL is recommended in production to limit user permissions
- **Rotate passwords periodically**: change Redis passwords regularly, every 90 days recommended

#### 4. Auditing and monitoring

- **Enable security logging**: monitor authentication failures and abnormal access
- **Configure alerts**: set alerts for connection and authentication failures
- **Audit periodically**: review logs and security configuration regularly

#### 5. Input validation

- **Key validation**: oxcache validates Redis keys automatically (rejecting empty, over-long keys and keys with `\r` / `\n` / `\0` and command-injection characters, scanning for SQL-injection and path-traversal patterns)
- **Lua script validation**: dangerous commands such as `FLUSHALL`/`FLUSHDB`/`KEYS`/`SHUTDOWN` are intercepted automatically before execution
- **SCAN pattern validation**: wildcard count and length are limited automatically to prevent ReDoS

For the full mechanism, see the 🔒 Security document.

---

## 🔧 Troubleshooting

<details>
<summary><b>❓ Issue: low cache hit rate</b></summary>

**Solution**:

1. Check whether the TTL is set too short
2. Confirm whether the data is updated frequently
3. Enable `enable_backfill()` on `ChainCache` so L2 hits backfill L1
4. Adjust the L1 cache capacity

</details>

<details>
<summary><b>❓ Issue: Redis connection failure</b></summary>

**Solution**:

1. Check that the connection string starts with `rediss://` (TLS)
2. If the development environment must use non-TLS, set `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`
3. Confirm the Redis server is running
4. Check network connectivity and firewall settings
5. Verify the username and password

</details>

<details>
<summary><b>❓ Issue: sync API returns NotSupported</b></summary>

**Solution**:

1. Confirm `Cache::builder().sync_mode(true)` is enabled
2. The default Moka path (including the native sync surface injected via `sync_backend_arc`) is runtime-independent: callable directly outside a runtime, reusing the current runtime via `block_in_place` on a `multi_thread` runtime; only **inside the async context of a current-thread runtime** it returns `Err(NotSupported)` (tokio forbids nested blocking drivers) — call outside a runtime or switch to a `multi_thread` runtime
3. Backends injected via `backend_arc(...)` go through the `AsyncToSyncBridge` bridged surface — it requires
   a `multi_thread` runtime (I/O-backed backends also need ≥2 workers; a single worker risks hangs),
   and outside a runtime or on a current-thread runtime every call returns `Err(NotSupported)`;
   for a runtime-independent sync API, inject a natively-sync backend via `sync_backend_arc(...)`
   (Moka / DashMap) or use the default Moka path

</details>

<details>
<summary><b>❓ Issue: performance degradation</b></summary>

**Solution**:

1. Check for memory leaks
2. Check whether the L1 cache capacity is reasonable
3. Use Redis pipelines for bulk writes (`set_many_pipeline` etc.)
4. Analyze slow-query logs

</details>

<div align="center">

**💬 Still need help?** [Submit an Issue](https://github.com/Kirky-X/oxcache/issues) or [visit the documentation hub](https://docs.rs/oxcache)

</div>

---

## 🎯 Next Steps

| Destination | Content |
|------|------|
| [📘 API Reference](API_REFERENCE.md) | Detailed interface descriptions and the error-code table |
| [🏗️ Architecture](ARCHITECTURE.md) | Dive into internal mechanisms and data flow |
| [🧪 Test Scenario Matrix](TEST_SCENARIOS.md) | Mapping from feature domains to test targets, with execution policy |
| [💻 Example code](../../examples/) | Code samples for real scenarios |
| [📊 Performance Baseline](PERFORMANCE.md) | Serialization sizes and hot-path benchmarks |
| 🔒 Security document | Threat model and security configuration |

<div align="center">

**[📖 API docs](https://docs.rs/oxcache)** • **[❓ FAQ](https://github.com/Kirky-X/oxcache/wiki)** • **[🐛 Report an issue](https://github.com/Kirky-X/oxcache/issues)**

Made with ❤️ by Kirky.X

[⬆ Back to top](#-oxcache-user-guide)

</div>

---

## 💻 Examples Overview

> This section was migrated from the README examples chapter, serving as the grouped list of all runnable examples.

The `examples/` directory (workspace member `oxcache-examples`, set as `publish = false`) contains **37 examples** (all runnable with default features):

```bash
# Run a single example (from the examples/ directory)
cd examples && cargo run --example example_basic_operations

# List all available examples
cd examples && ls src/*/*.rs
```

**Basics (`examples/src/01_basics`)**

| Example | Description |
|------|------|
| `example_basic_operations` | Basic CRUD operations (`get` / `set` / `delete` / `exists`) |
| `example_new_api` | Modern API introduction (`Cache::builder()` / `Cache::memory()`) |
| `example_cache_builder` | CacheBuilder configuration (`capacity` / `ttl` / `tti` / `sync_mode`) |
| `example_serialization` | JSON serialization |
| `example_cache_key` | Custom cache keys (`CacheKey` trait) |
| `example_cached_macro` | `#[cached]` macro (`service` / `ttl` / `key_prefix`) |
| `example_explicit_init` | Explicit initialization (`Cache::new()` / global cache) |
| `example_get_or` | Compute on cache miss (`get_or`, single-flight) |
| `example_sync_api` | Sync API (`get_sync` / `set_sync` / `clear_sync` / `len_sync`) |
| `example_byte_ops` | Byte-level operations (`get_bytes` / `set_bytes` / `len` / `capacity` / `shutdown`) |
| `example_comprehensive_usage` | Comprehensive usage (overview of all features) |

**Advanced (`examples/src/02_advanced`)**

| Example | Description |
|------|------|
| `example_batch_write` | Batch operations (`set_many` / `get_many` / `delete_many`) |
| `example_chain_cache` | Chained cache (`ChainCache` / `ChainLink`) |
| `example_invalidation` | Cache invalidation strategies (TTL / TTI / manual invalidation) |
| `example_warmup` | Cache warm-up (batch preloading) |
| `example_smart_strategy` | Caching strategy patterns (Cache-Aside / Lazy Loading / tiered TTL) |
| `example_cache_promotion` | Cache promotion (L2→L1 promotion / hot-key analysis) |
| `example_error_handling` | Error handling (`OxCacheError` / retry / recoverability) |
| `example_custom_backend` | Custom backends (`CacheReader` / `CacheWriter` / `CacheConnector`) |
| `example_dashmap_backend` | DashMap backend (`DashMapMemoryBackend`) |
| `example_moka_ttl` | Moka per-entry TTL (`Expiry` trait) |
| `example_redis_native` | Native Redis operations (`RedisBackend`, requires Redis) |
| `example_redis_modes` | Redis deployment modes (Standalone / Cluster / Sentinel, requires Redis) |
| `example_redis_pipeline` | Pipeline batching (`set_many_pipeline` / `get_many_pipeline`, requires Redis) |
| `example_lua_script` | Lua script execution (`eval_lua` / `script_load` / `eval_sha`, requires Redis) |

**Configuration (`examples/src/03_config`)**

| Example | Description |
|------|------|
| `example_dynamic_config` | Dynamic configuration (runtime config changes) |
| `example_key_generator` | Key generator (`KeyGenerator`) |

**Database Integration (`examples/src/05_database`)**

| Example | Description |
|------|------|
| `example_database_integration` | Database integration (Cache-Aside pattern) |

**Feature Showcase (`examples/src/06_features`)**

| Example | Description |
|------|------|
| `example_metrics` | Metrics export (`export_json_format` / `export_prometheus_format`) |
| `example_compression` | Data compression (`JsonSerializer::with_compression()`) |
| `example_security` | Data redaction (`redact_value` / `redact_connection_string`) |
| `example_security_validation` | Security validation (`validate_redis_key` / `validate_lua_script`) |
| `security_doctor` | Security self-audit (`audit()` / `SecurityReport`; run with `cargo run --example security_doctor --features doctor`) |
| `example_bloom_filter` | Bloom filter (`BloomFilter` / `BloomFilterBackend`) |
| `example_i18n` | Internationalization (`CacheI18nFormatter`) |
| `example_events` | Event system (`CacheEvent` / `CacheEventType`) |
| `example_cli_usage` | CLI scenarios (obtaining cache status and metrics from code) |
| `example_kit_integration` | trait-kit AsyncKit integration (`OxcacheModule` / health check / lifecycle / three-phase shutdown / decorators) |

> Examples marked "requires Redis" need a running Redis 6.0+ server; all other examples use in-memory backends and run standalone.

---

## 🧩 wasm32 Support (experimental)

> This section was migrated from the README quick-start chapter.

On wasm32-unknown-unknown only the minimal `byte-weight` + `serialization` combination is supported (no async/memory backends):

```bash
cargo build --lib --target wasm32-unknown-unknown --no-default-features --features byte-weight,serialization
```

Limits: the `minimal`/`memory`/async/redis/batch backends rely on multi-thread runtime semantics (`block_in_place`) and are out of scope for wasm; CI only gates the build and clippy for the minimal combination above, with no runtime behavior tests.
