# 📘 Oxcache API Reference

> **⚠️ API version note**
>
> This document describes the API of **Oxcache v0.5.0**.

This document provides the detailed API reference of the Oxcache library.

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [🎨 Feature Requirements](#-feature-requirements)
- [🪄 Cache Macro](#-cache-macro)
- [🧩 `Cache<K, V>`](#-cachek-v)
- [🧰 CacheBuilder](#-cachebuilder)
- [🔌 Backend Layer](#-backend-layer)
- [📡 RedisBackend](#-redisbackend)
- [🐉 DragonflyBackend](#-dragonflybackend)
- [🌌 AerospikeBackend](#-aerospikebackend)
- [🔗 ChainCache](#-chaincache)
- [📢 Redis Pub/Sub Broadcast Channel](#-redis-pubsub-broadcast-channelpubsub-feature)
- [🔄 Sync API](#-sync-api)
- [🌸 Bloom Filter](#-bloom-filter)
- [🕒 TTL Management](#-ttl-management)
- [🔒 Security Features](#-security-features)
- [📈 Observability](#-observability)
- [🚨 Error Handling](#-error-handling)
- [🏷 Type Aliases](#-type-aliases)
- [🧬 trait-kit Integration](#-trait-kit-integrationtrait-kit-feature)
- [💻 Examples](#-examples)
- [🎨 Feature Flags](#-feature-flags)

</details>

## 🎨 Feature Requirements

Oxcache uses feature gates to control capabilities. The following are the key features and their requirements:

### Tiered feature sets

- **`minimal`**: L1 cache only (memory + metrics + serialization + chrono), **enabled by default**
- **`redis-tier`**: L1 + L2 cache (minimal + redis; the legacy `core` remains as a compat alias)
- **`full`**: the true full set (redis-tier + every named capability feature, including `bloom` / `trait-kit` / `encrypt` / `integrity` / the three backends etc.; the deprecated aliases `core` / `kit` / `red-lock` / `testing` are not members)

### Component features

For the itemized component features (including defaults and whether they are included in `full`), see the [Feature Flags section below](#-feature-flags); opt-in features such as `bloom` / `trait-kit` are now part of `full` and can still be enabled alone (`features = ["bloom"]`); configuration examples are in that section too.

### Feature dependencies

Some features require or imply other features:

| Feature | Required features | Description |
|------|----------|------|
| `minimal` | `memory`, `metrics`, `serialization` | Minimal preset (L1 memory + metrics) |
| `redis-tier` | `minimal`, `redis` | Core L1 + L2 cache (the legacy `core` remains as a compat alias) |
| `full` | `redis-tier` + every named capability feature (including `bloom` / `trait-kit` / `encrypt` / `integrity` / `redlock` / `warmup` etc.; the deprecated aliases `core` / `kit` / `red-lock` / `testing` are not members) | True full preset; the exact membership is in `Cargo.toml` `[features]` |
| `memory` | `serialization` | Compilation baseline of the backend and cache core APIs |
| `redis` | `serialization` | L2 Redis backend |
| `dragonfly` | `redis` | Dragonfly compatibility-mode backend |
| `aerospike` | — | Aerospike backend (self-managed dependencies) |
| `serialization` | — | Serialization facade (JSON baseline) |
| `serde-bincode` | `serialization` | bincode binary format |
| `postcard` | `serialization` | postcard binary format |
| `metrics` | `serialization`, `chrono`, `dashmap` | Built-in metrics |
| `telemetry` | — | tracing facade (log paths such as reconnect/invalidation) |
| `macros` | `minimal` | Attribute macro |
| `batch` | `memory` | BatchWriter (wraps a `CacheBackend`; full buffer flushes automatically by default, `reject_when_full(true)` rejects instead and returns `BufferFull`/`OXCACHE_019`) |
| `lua` | `redis` | Lua script execution |
| `test-util` | — | Testing helpers (`#[cfg(feature)]`-only stubs) |
| `testing` | `test-util` | Test implementations (deprecated alias; use the canonical `test-util`) |
| `bloom` | — | Bloom filter (now part of `full`, can be enabled alone) |
| `trait-kit` / `kit` | `memory` | kit integration (references `crate::backend`; `kit` is a deprecated alias implying `trait-kit`; now part of `full`, can be enabled alone) |
| `lock` | `redis` | Distributed lock |
| `redlock` | `lock` | RedLock implementation |
| `red-lock` | `redlock` | RedLock multi-node majority lock (deprecated alias; use the canonical `redlock`) |
| `compression` | `memory` | Compression decorator (references the backend and serialization surface) |
| `offload` | `dashmap` | In-process background task executor |
| `disk` | `memory`, `serialization` | Embedded redb L3 |
| `stale` | `memory`, `offload`, `serialization` | SWR three-state expiry |
| `invalidation` | `redis`, `serialization` | Cross-instance invalidation broadcast bus |
| `pubsub` | `redis` | Generic Redis Pub/Sub broadcast channel |
| `encrypt` | `memory` | Encryption decorator (references `crate::backend`) |
| `integrity` | `encrypt`, `hmac`, `sha2` | Integrity signing layer (module hosted inside encryption) |
| `config-confers` | `memory`, `serialization` | Configuration hub |
| `degradation` | `memory` | Degradation decorator (references `crate::backend`) |
| `audit` | — | Audit event stream |
| `versioning` | — | Value versioning |
| `hotkey` | `dashmap` | Hot-key sampling observability |
| `byte-weight` | `moka` | Byte-weighted capacity (moka sync surface) |
| `adaptive-ttl` | `memory` | Adaptive TTL decorator (references `crate::backend`) |
| `warmup` | `memory` | Smart warm-up (operates on `ChainCache`) |

The "required features" column above lists only cargo features (`dep:` dependencies are not repeated; words like `dashmap`/`moka`/
`hmac` denote activation of the corresponding optional dependencies, not features of this crate). The `default` preset is
`minimal`.

## 🪄 Cache Macro

### The `#[cached]` attribute macro

A function-caching decorator with zero template code. Requires the `macros` feature.

**Parameters:**

| Parameter | Type | Required | Default | Description |
|------|------|--------|------|------|
| `service` | `&str` | no | `"default"` | Cache service name (used to look up the registered `Cache` instance) |
| `ttl` | `u64` | no | `None` | Time-to-live (seconds); becomes `Some(n)` when set |
| `key` | `&str` | no | auto-generated | Custom cache key template: placeholders reference function parameters / `self` / `self.<field path>` (e.g. `key = "user_{self.tenant_id}_{id}"`); placeholder values are escaped per segment separator before entering the key. Methods with a `self` receiver must carry the ownership dimension via a placeholder-bearing template (constant templates fail at compile time) |
| `key_prefix` | `&str` | no | `""` | Prefix of the auto-generated cache key (static literal, carries no scope) |
| `scope` | expression | no | — | Scope expression (the key's ownership dimension, e.g. `scope = self.tenant_id`); mutually exclusive with the `key` template (compile error). Functions with a `self` receiver must provide `scope` or `key`, otherwise they fail at compile time |
| `sync` | (flag) | no | async | Uses the synchronous code path (`get_bytes_sync`/`set_bytes_sync`); cannot be combined with `async fn` |
| `skip_cache_write` | (flag) | no | `false` | Skips the cache write for `Ok` results (`Err` results are never cached anyway) |
| `single_flight` | (flag) | no | `false` | Concurrent misses on the same key trigger only one fallback |
| `strict` | (flag) | no | `false` | Panics instead of silently passing through when the cache is unregistered |
| `condition` | function path | no | — | Pre-execution predicate; bypasses the cache when it returns false |
| `skip` | parameter name list, e.g. `skip(password)` | no | — | Named parameters are excluded from the default cache key; mutually exclusive with the `key` template (compile error); unknown parameter names fail at compile time |
| `cache_none` | (flag) | no | `false` | Effective when the return type is `Result<Option<T>, E>`: by default only `Ok(Some)` is cached (`Ok(None)` is not written back); when enabled, `Ok(None)` is cached as `null` and restored on read. Has no effect for non-Option return types |

The macro obtains the `Cache` instance from the internal registry via `oxcache::__internal_get_cache(service)`.
If no cache is registered under `service`, the original function executes directly without caching (`strict` mode panics instead).
When the `sync` flag is used, the `Cache` must be built with `sync_mode(true)`.

**Example (async):**

```rust
// Cargo.toml: oxcache = { version = "0.5.0", features = ["macros"] }
use oxcache::cached;

#[cached(service = "default", ttl = 3600)]
async fn fetch_user(user_id: &str) -> Result<User, String> {
    // 函数体
    # Ok(User { /* ... */ })
}
```

**Custom key format:**

```rust
#[cached(service = "default", ttl = 3600, key = "user:{user_id}")]
async fn fetch_user(user_id: &str) -> Result<User, String> {
    // 函数体
    # Ok(User { /* ... */ })
}
```

**Default key generation**: when neither `key` nor `key_prefix` is set:
`{service}[:{scope}]:{fn_name}[:{args_digest}]`.

- The argument segment is a **sha256 digest** (`{args_digest}`), not the `Debug` plaintext of the arguments:
  arguments often contain passwords / session tokens, and plaintext entering the persisted key space (visible
  to KEYS/SCAN, snapshots, MONITOR) equals credentials landing on disk. `skip(...)` parameters do not
  participate in the digest.
- The `scope` segment is the scope expression's value (`:` percent-encoded as `%3A`, and the escape marker `%`
  itself encoded first as `%25`, guaranteeing injective encoding). Methods with a `self` receiver that have
  neither `scope` nor `key` fail at compile time: the default key carries no ownership dimension, and different
  tenants / users would share the same cache entry. A `key` template on a method without placeholders also fails
  at compile time (a constant template is unrelated to `self`; all tenants would share one constant key).

## 🧩 `Cache<K, V>`

The primary type-safe cache type. `K: CacheKey`, `V: Serialize + Deserialize`.

For byte-level operations without type parameters, use the `BytesCache` type alias:

```rust
use oxcache::BytesCache;

let cache: BytesCache = Cache::builder().build().await?; // Cache<String, Vec<u8>>
cache.set_bytes("k", b"raw".to_vec(), None).await?;
let v: Option<Vec<u8>> = cache.get_bytes("k").await?;
```

**Construction:**

```rust
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct User { id: u64, name: String }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 默认 Moka 内存后端（容量 10000）
    // register_for_macro 仅支持 Cache<String, Vec<u8>>（宏内部按字节读写）
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;

    // 注册供 #[cached] 宏使用
    cache.register_for_macro("default").await?;
    Ok(())
}
```

**Constructors:**

| Constructor | Feature | Description |
|----------|------|------|
| `Cache::builder()` | — | Returns a `CacheBuilder<K, V>` |
| `Cache::new()` | `memory` | Default Moka backend (synchronous, no `.await`) |
| `Cache::memory().await` | — | Convenience: default Moka backend |
| `Cache::redis(url).await` | `redis` | Convenience: Redis backend |
| `Cache::with_dependencies(backend)` | — | Constructs from an `Arc<dyn CacheBackend>` |

### Async Operations

#### `get(&self, key: &K) -> OxCacheResult<Option<V>>`

Gets a value from the cache. Returns `Ok(None)` when not found.

```rust
let user: Option<User> = cache.get(&"user:1".to_string()).await?;
```

#### `set(&self, key: &K, value: &V) -> OxCacheResult<()>`

Sets a value without a per-entry TTL (uses the backend's global TTL, if any).

```rust
cache.set(&"user:1".to_string(), &user).await?;
```

#### `set_with_ttl(&self, key: &K, value: &V, ttl: Option<Duration>) -> OxCacheResult<()>`

Sets a value with an optional per-entry TTL. `Some(d)` overrides the backend's global TTL for that entry;
`None` falls back to the global TTL.

```rust
use std::time::Duration;
cache.set_with_ttl(&"user:1".to_string(), &user, Some(Duration::from_secs(3600))).await?;
```

#### `get_by_str(&self, key: &str) -> OxCacheResult<Option<V>>` / `set_by_str(&self, key: &str, value: &V, ttl: Option<Duration>) -> OxCacheResult<()>`

Borrowed-key reads and writes, eliminating `String` allocations on the hot path (the `set` path makes only 1 `Arc<str>` allocation).
For measured gains, see the [Performance Baseline](PERFORMANCE.md).

#### `delete(&self, key: &K) -> OxCacheResult<()>`

Deletes a value from the cache.

#### `exists(&self, key: &K) -> OxCacheResult<bool>`

Checks whether a key exists in the cache.

#### `clear(&self) -> OxCacheResult<()>`

Clears all entries.

#### `get_or<F, Fut>(&self, key: &K, fallback: F) -> OxCacheResult<V>`

Gets a value or computes it via `fallback` (single flight: concurrent calls on the same key share one computation).

```rust
let user = cache.get_or(&"user:1".to_string(), || async {
    fetch_user_from_db(1).await
}).await?;
```

#### `ttl(&self, key: &K) -> OxCacheResult<Option<Duration>>`

Gets the key's remaining time-to-live. Returns `Ok(None)` when the key has no per-entry TTL or does not exist.
Suitable for update flows that preserve the TTL:

```rust
let original_ttl = cache.ttl(&"user:1".to_string()).await?;
cache.set_with_ttl(&"user:1".to_string(), &new_user, original_ttl).await?;
```

#### `expire(&self, key: &K, ttl: Duration) -> OxCacheResult<bool>`

Updates the TTL of an existing key without modifying its value. Returns `Ok(true)` when the TTL update succeeds and `Ok(false)` when the key does not exist.

### Lifecycle and Statistics

| Method | Returns | Description |
|------|--------|------|
| `health_check().await` | `OxCacheResult<()>` | Backend health check |
| `stats().await` | `Result<HashMap<String,String>>` | Backend-specific statistics |
| `len().await` | `OxCacheResult<u64>` | Number of entries |
| `is_empty().await` | `OxCacheResult<bool>` | Whether the cache is empty |
| `capacity().await` | `OxCacheResult<u64>` | Configured capacity (0 for Redis) |
| `shutdown().await` | `()` | Shuts down and releases resources |
| `register_for_macro(service).await` | `OxCacheResult<()>` | Registers for use by the `#[cached]` macro |
| `keys(pattern).await` | `OxCacheResult<Vec<String>>` | Keys matching the pattern; when the matched count exceeds `MAX_KEYS_RETURN` (`oxcache::MAX_KEYS_RETURN`, 10 000) it returns `Err(Operation)` (to avoid materializing the full key set into memory at once) — use a narrower pattern instead. Exactly 10 000 entries are returned normally |

## 🧰 CacheBuilder

`CacheBuilder<K, V>` is the unified builder for `Cache`, obtained via `Cache::builder()`.

**Methods:**

| Method | Signature | Description |
|------|------|------|
| `backend_arc` | `(backend: Arc<dyn CacheBackend>) -> Self` | Adds a pre-built backend (async surface only) |
| `sync_backend_arc` | `(backend: Arc<dyn SyncCacheBackend>) -> Self` | Adds a pre-built **natively sync** backend (together with `sync_mode(true)` to enable the sync API) |
| `ttl` | `(ttl: Duration) -> Self` | Default TTL for cache entries |
| `tti` | `(tti: Duration) -> Self` | Default TTI (time-to-idle) for memory backends |
| `capacity` | `(capacity: u64) -> Self` | Capacity for memory backends (default 10000) |
| `sync_mode` | `(enabled: bool) -> Self` | Enables the sync API (see [Sync API](#-sync-api)) |
| `serialization_format` | `(format: SerializationFormat) -> Self` | Switches the serialization format (JSON/Bincode/Postcard; requires `serde-bincode`/`postcard`) |
| `null_cache_ttl` | `(ttl: Duration) -> Self` | Null-sentinel TTL (penetration guard) |
| `ttl_jitter` | `(factor: f64) -> Self` | TTL jitter factor (guards against mass simultaneous expiry) |
| `metrics` | `(recorder: Arc<dyn MetricsRecorder>) -> Self` | Injects a `MetricsRecorder` (overrides pure-L1 path metrics) |
| `audit_publisher` | `(publisher: Arc<dyn AuditEventPublisher>) -> Self` | Injects an audit event publisher (`audit` feature; structured hit/miss/set/delete events with key redaction); built-in implementations: `NoOpAuditPublisher` / `InMemoryAuditPublisher` (bounded ring) / `TracingAuditPublisher` (`telemetry`). `get_bytes` / `set_bytes` (the `#[cached]` macro path) publish audit events as well |
| `build` | `async (self) -> Result<Cache<K, V>>` | Builds the cache instance (async wrapper, no await inside) |
| `build_sync` | `(self) -> Result<Cache<K, V>>` | Builds the cache instance synchronously (no runtime needed) |

> **Note:** `CacheBuilder` has no `.redis(...)`, `.tiered(...)`, `.with_backend(...)`,
> `.batch_writes(...)` or `.auto_promote(...)` methods. Use
> `.backend_arc(Arc::new(...))` to insert a `RedisBackend` or another backend.

**Default Moka path** (when `backend_arc` is not called): builds a `MokaMemoryBackend` with the given `capacity`/`ttl`/`tti`. For the three-path configuration of `sync_mode(true)`, see "Sync API and explicit backends".

**Example:**

```rust
use oxcache::{Cache, CacheBuilder};
use std::time::Duration;

// 仅 L1 内存缓存
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

// 通过预构建后端实现 L2（Redis）缓存
let redis = RedisBackend::new("rediss://localhost:6379").await?;
let cache: Cache<String, String> = Cache::builder()
    .backend_arc(Arc::new(redis))
    .build()
    .await?;
```

**Sync API and explicit backends:** `sync_mode(true)` supports three configurations — the default Moka backend (natively
sync, runtime-independent); a natively-sync backend injected via `sync_backend_arc(Arc<dyn SyncCacheBackend>)`
(sync API connects directly; the async API is presented through the `SyncBackendAdapter` facade); and
`backend_arc(Arc<dyn CacheBackend>)` (the concrete type's sync implementation is erased; the sync API is bridged through the generic
`AsyncToSyncBridge` — every sync call blocks on the async surface, **requiring the caller to be on a multi-thread
Tokio runtime**; outside a runtime or on a current_thread runtime every call returns
`Err(OxCacheError::NotSupported)`). For memory-backed backends needing a runtime-independent sync API, inject their native sync surface via
`sync_backend_arc` instead.

> **⚠️ Blocking warning:** both paths carry blocking semantics — the `SyncBackendAdapter`'s async methods
> complete synchronously (when wrapping `RedisBackend` and other network-backed sync backends, every async API call blocks an
> executor thread); the `AsyncToSyncBridge`'s sync methods block on the async surface (same for network-backed
> backends). The recommended usage for network-backed backends: inject via `backend_arc(...)` and use the async API; memory-backed
> backends (Moka / DashMap) are unaffected.

## 🔌 Backend Layer

The `backend` module exposes the backend traits and implementations.

### Backend Traits

For the full definition of the backend trait hierarchy (`CacheReader` / `CacheWriter` / `CacheConnector`, blanket-impl-composed into `CacheBackend`; the batch item type `CacheSetItem`; the sync mirror traits and `AtomicCacheWriter`), see the [backend layer chapter of the architecture documentation](ARCHITECTURE.md#3-backend-layer).

`CacheBackend` additionally provides two pattern-level methods (for namespace invalidation and similar scenarios):

| Method | Returns | Description |
|------|--------|------|
| `delete_by_pattern(pattern).await` | `OxCacheResult<u64>` | Deletes all matching keys and returns the deleted count. Executes natively when the backend offers the `CachePatternOps` capability, **unconstrained by the per-enumeration key cap** (namespace invalidation may span tens of thousands of entries); otherwise falls back to "enumerate + delete one by one" and returns `Err(Operation)` when over the cap |
| `count_by_pattern(pattern).await` | `OxCacheResult<u64>` | Counts matching keys, likewise unconstrained by the enumeration cap |

Backend-native capabilities are exposed via `CacheConnector::as_pattern_ops()`; the primitives on the capability trait are named
`CachePatternOps::pattern_delete` / `CachePatternOps::pattern_count` (deliberately different from the table above: a backend type is
an implementation of both traits, and same-named methods would make method-call syntax on the concrete type report E0034).

### Backend Types

For the backend types, module paths and owning features, see the [architecture documentation](ARCHITECTURE.md#3-backend-layer).

### In-memory backend helpers

```rust
use oxcache::backend::{moka_memory, dashmap_memory, default_memory_backend, MokaMemoryBackend};

let moka = MokaMemoryBackend::builder().capacity(10000).build();
```

`MokaMemoryBackend::builder()` exposes `.capacity(u64)`, `.ttl(Duration)`,
`.time_to_idle(Duration)` and `.build()` (synchronous).

### Backend Scores

Each backend reports a `score()` (higher = faster), used by `ChainCache` to order reads and writes:

| Backend | score | `is_persistent()` |
|------|:-----:|:-----------------:|
| Moka | 100 | false |
| DashMap | 90 | false |
| Redis | 50 | true |
| Valkey (via RedisBackend) | 50 | true |
| Disk (redb) | 85 | true |
| Dragonfly | 50 | true |
| Aerospike | 30 | true |

## 📡 RedisBackend

`RedisBackend` is the L2 distributed cache. It uses `redis::aio::ConnectionManager`
for connection-pool management and is re-exported at `oxcache::backend::RedisBackend`.

**Security:** connections must use TLS (`rediss://`). Non-TLS connections are rejected
unless the environment variable `OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS`
(or `=development-only`) is set.

### Constructors

```rust
use oxcache::backend::RedisBackend;

// 从连接字符串构造（推荐使用 TLS）
let backend = RedisBackend::new("rediss://localhost:6379").await?;

// 显式指定连接池大小（当前等价于 new()）
let backend = RedisBackend::with_pool("rediss://localhost:6379", 8).await?;

// 通过 builder 构造
let backend = RedisBackend::builder()
    .connection_string("rediss://localhost:6379")
    .mode(oxcache::backend::RedisMode::Standalone)
    .build()
    .await?;
```

**`RedisBackendBuilder` methods:**

| Method | Description |
|------|------|
| `connection_string(&str)` | Sets the Redis connection string (a comma-separated list of sentinel URLs in Sentinel mode) |
| `mode(RedisMode)` | Sets the Redis mode (`Standalone`/`Sentinel`/`Cluster`/`ValkeyStandalone`) |
| `sentinel_master_name(impl Into<String>)` | The master name monitored in Sentinel mode (default `"mymaster"`) |
| `sentinel_addr_map(impl IntoIterator<Item=(String,String)>)` | NAT address mapping for Sentinel mode (`ip:port` → client-reachable `host:port`); unmatched addresses are used as-is |
| `lua_timeout(Duration)` | Response timeout for Lua script commands (default 5 seconds; `Duration::ZERO` means unbounded), covering `eval_lua`/`eval_sha`/`script_load` |
| `build().await` | Builds the `RedisBackend` (2-second connection timeout) |

**Sentinel mode behavior:** the connection string points to a list of sentinel nodes; the master is discovered one by one via
`SENTINEL get-master-addr-by-name` (the first successful answer wins);
after a failover, when the data connection hits a recoverable error (disconnect/`READONLY`/`MASTERDOWN` etc.) the new master is
re-discovered automatically and the operation retried once — the same backend instance switches over transparently.

### Instance Methods

| Method | Signature | Description |
|------|------|------|
| `ping().await` | `OxCacheResult<String>` | Pings the server (returns `"PONG"`) |
| `mode()` | `RedisMode` | The configured Redis mode |
| `redact_connection_string(s)` | `String` (associated function) | Redacts the password in a connection string |
| `clear_namespace(ns).await` | `OxCacheResult<()>` | Deletes the keys of a single namespace (`ns:*`): a namespace not ending with `:` gets the separator appended automatically — `tenant1` deletes only `tenant1:*`, never `tenant10:*` |
| `clear_prefix(prefix).await` | `OxCacheResult<()>` | Deletes by bare prefix (`prefix*`): **no namespace boundary** — `tenant1` also deletes `tenant10:*`; use only when bare-prefix semantics are truly needed |

### Pipeline Batch Operations

For high-throughput scenarios, using Redis pipeline (single round trip):

```rust
// 批量 SET
backend.set_many_pipeline(&[("k1", v1), ("k2", v2)], Some(Duration::from_secs(60))).await?;

// 批量 GET
let values: Vec<Option<Vec<u8>>> = backend.get_many_pipeline(&["k1", "k2"]).await?;

// 批量 DEL
backend.delete_many_pipeline(&["k1", "k2"]).await?;
```

### Lua Scripts (`lua` feature)

With the `lua` feature enabled, `RedisBackend` implements `LuaExecutor`:

```rust
use oxcache::backend::interface::LuaExecutor;

// EVAL
let val = backend.eval_lua("return redis.call('GET', KEYS[1])", &["k1"], &[]).await?;

// SCRIPT LOAD + EVALSHA
let sha = backend.script_load("return 1 + 1").await?;
let val = backend.eval_sha(&sha, &[], &[]).await?;
```

`eval_lua` and `script_load` validate the script through `validate_lua_script` before executing/caching; `eval_sha` only validates the SHA1 format and key names, without re-validating the script content — the script was already validated at `script_load` time. When `eval_sha` hits NOSCRIPT (script not cached) it does not fall back; it returns an error explicitly, and the caller must re-run `script_load` or use `eval_lua`.

All three methods execute through the circuit breaker: when the breaker is Open, `Degraded` is returned immediately (no script execution), and successes/failures count toward the breaker. EVAL has side-effect semantics and is **not retried** on failure (unlike ordinary commands' retry behavior). The response timeout is controlled by the builder option `lua_timeout` (default 5 seconds, `Duration::ZERO` unbounded); on timeout it returns `OxCacheError::Timeout` and counts as a breaker failure.

## 🐉 DragonflyBackend

`DragonflyBackend` wraps `RedisBackend`, providing cache support for Dragonfly servers.
Dragonfly is Redis-protocol compatible, so it reuses all of Redis's read/write paths.

```rust
use oxcache::backend::DragonflyBackend;
use std::sync::Arc;

// 构造 Dragonfly 后端（需要 dragonfly feature）
// 构造复用 RedisBackend 的 TLS 强制：连接串须为 rediss://；
// 开发环境确需 redis:// 时，须设置 OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS
let backend = DragonflyBackend::new("rediss://localhost:6379", 8).await?;

// 作为 ChainCache L2 使用
use oxcache::backend::MokaMemoryBackend;
use oxcache::cache::chain::{ChainCacheBuilder, ChainLink};
let chain = ChainCacheBuilder::default()
    .link(ChainLink::new(MokaMemoryBackend::new(), 100, false, "moka"))
    .link(ChainLink::new(backend, 50, true, "dragonfly"))
    .build();
```

**Limitations:**
- `as_atomic_writer()` returns `None` (Dragonfly atomic-operation compatibility is unverified)
- `backend_kind()` returns `BackendKind::Dragonfly`

## 🌌 AerospikeBackend

`AerospikeBackend` is enabled via the `aerospike` feature,
providing an Aerospike persistent KV storage backend.

```rust
use oxcache::backend::{AerospikeBackend, AerospikeConfig};

let config = AerospikeConfig {
    seed_nodes: vec!["127.0.0.1:3000".to_string()],
    namespace: "test".to_string(),
    set_name: "cache".to_string(),
    default_ttl: 3600,
    ip_map: None, // Docker/NAT 环境设置 IP 转换表
};
let backend = AerospikeBackend::new(config).await?;
```

**Unsupported operations:** `len()`, `capacity()`, `keys()` and `clear()` return `Err(NotSupported)`.

## 🔗 ChainCache

`ChainCache` manages multiple backends, ordered by descending score. Reads start from the highest-scored backend
(falling through to the next link on `None` or an error); writes **fan out concurrently** to all backends,
tolerating a single-link failure (an error is returned only when *all* backends fail). Backfill optionally **asynchronously** fills
the higher-scored backend on a lower-scored hit.
Health checks run concurrently, 5-second timeout per backend.

```rust
use oxcache::cache::{ChainCache, ChainLink};
use oxcache::backend::{MokaMemoryBackend, RedisBackend};
use std::time::Duration;

let l1 = MokaMemoryBackend::builder().capacity(10000).ttl(Duration::from_secs(300)).build();
let l2 = RedisBackend::new("rediss://localhost:6379").await?;

let chain = ChainCache::builder()
    .link(ChainLink::from_backend(l1))   // L1，分数 100
    .link(ChainLink::from_backend(l2))   // L2，分数 50
    .enable_backfill()
    .default_time_to_live(Duration::from_secs(600))
    .build();  // 同步构建，返回 ChainCache

chain.set("key", b"value".to_vec(), None).await?;
let v = chain.get("key").await?; // Some(Vec<u8>)
```

### `ChainCacheBuilder`

| Method | Description |
|------|------|
| `link(ChainLink)` | Adds a link |
| `links(Vec<ChainLink>)` | Adds multiple links |
| `backend(B)` | Adds a backend (wrapped automatically via `ChainLink::from_backend`) |
| `default_time_to_live(Duration)` | The default TTL used when `set` is called with `ttl=None` |
| `enable_backfill()` / `disable_backfill()` | Toggles backfill (off by default) |
| `enable_race_read()` / `disable_race_read()` | Toggles concurrent full-read taking the highest-scored hit (off by default) |
| `event_publisher(Arc<dyn EventPublisher>)` | Configures the event publisher (emits events when backend operations fail) |
| `with_invalidation(Arc<InvalidationBus>)` | Enables write-path invalidation broadcast (the `invalidation` feature): at build time, links whose persistence layer has `is_persistent == true` are wrapped with `InvalidatingBackend`; after set/delete/clear/set_many/delete_many succeed, the bus broadcasts so other instances invalidate their local caches; expire does not broadcast (the decorator itself can enable expire broadcast via `InvalidatingBackend::with_expire_broadcast`, off by default for the chain integration); the wrapped layer does not support the sync API |
| `build()` | Builds the `ChainCache` (synchronous; orders links by descending score) |

### `ChainLink`

| Constructor | Description |
|----------|------|
| `new(backend, score, is_persistent, name)` | Manual construction |
| `from_backend(B)` | Derives score/persistent/name automatically from `BackendScore` (async API only) |
| `from_sync_backend(B)` | Like `from_backend` but also fills the sync backend handle (requires `SyncCacheBackend`) |

`ChainLink` accessors: `backend()`, `try_as_sync_backend()`, `score()`,
`is_persistent()`, `name()`.

### TTL Behavior

`ChainCache` does not store TTLs itself; it forwards transparently to each link:

- `set(key, value, Some(d))` → all links use the same TTL `d`.
- `set(key, value, None)` → links use `default_ttl` (if set), otherwise each link's own global TTL.
- `ttl(key)` → returns the first `Some(ttl)` found scanning from the highest score.
- `expire(key, d)` → forwarded to all links; returns `Ok(true)` if any link succeeds.

### Batch Reads: `iter_entries`

`iter_entries(&self, keys: &[&str]) -> Vec<(String, Option<Vec<u8>>)>`:
batch-reads keys not yet hit, layer by layer from the highest score down, one `get_many` call per layer
(`RedisBackend`'s `get_many` is a pipeline batch read); a hit key is answered by the highest-scored
layer that has it; output order matches input, and backfill is not triggered. When a layer's batch read fails wholesale,
the error is split per key (one error event + warn per key), and the keys degrade to the next layer;
keys still unmatched after all layers (miss or all-layer failures) end as `None`; the method itself
does not return `Err`.

```rust
// entries: Vec<(String, Option<Vec<u8>>)>，方法本身不返回 Err
let entries = chain.iter_entries(&["k1", "k2", "k3"]).await;
```

### One-Stop Invalidation Broadcast Assembly

`tiered_with_invalidation(l1, l2, bus)` (`memory` + `invalidation` features)
is equivalent to `ChainBuilder::new().l1(l1).l2(l2).with_invalidation(bus)`:
after the persistent layer (L2 needs `.persistent(true)`) writes succeed, invalidation events are broadcast via `bus`,
and listener tasks on other instances invalidate their own local L1; expire does not broadcast.

### ChainCache Sync API

`ChainCache` exposes `get_sync`/`set_sync`/`delete_sync`. These methods require **every**
link to support `SyncCacheBackend` (i.e. built via `from_sync_backend`);
otherwise they return `Err(OxCacheError::NotSupported)`.

## 📢 Redis Pub/Sub Broadcast Channel (`pubsub` feature)

A generic channel-broadcast facility (`oxcache::pubsub::RedisPubSub`); unlike the invalidation broadcast
(the `invalidation` feature's `InvalidationBus`, which carries only cache invalidation events),
it targets arbitrary cross-instance message semantics: distributed session/login-state synchronization (SSO kickout broadcasts),
business event fan-out, etc.

```rust,no_run
# use std::sync::Arc;
# async fn example() -> Result<(), oxcache::OxCacheError> {
let ps = oxcache::pubsub::RedisPubSub::new("redis://127.0.0.1:6379").await?;
ps.subscribe("sso:kickout", Arc::new(|msg| {
    println!("kickout broadcast: {msg}");
})).await?;
let receivers = ps.publish("sso:kickout", "user-42").await?;
ps.shutdown();
# Ok(())
# }
```

### Behavior Contract

- **Fail-fast at construction**: `new` warms up the publishing connection (`ConnectionManager`); an invalid URL
  or unreachable port returns `Err` immediately, leaving no "fake success" instance;
- **Dedicated subscription connections**: every `subscribe` creates a dedicated Pub/Sub connection on the spot
  (`SUBSCRIBE` monopolizes a connection and cannot reuse a multiplexed one); the first successful subscription is
  confirmed via a oneshot, and `subscribe` returning `Ok(())` means the subscription is live on the server;
- **publish returns the receiver count** (0 when there are no subscribers);
- **Handler panic isolation**: callbacks are wrapped in `catch_unwind`; a panic only logs a warn,
  subsequent messages keep being delivered and the subscription is not interrupted;
- **Reconnection**: after the subscription connection drops, it reconnects with linear backoff and re-SUBSCRIBEs (default 5 attempts,
  base 500ms, capped at 5s; `with_reconnect_attempts(0)` disables reconnection, degrading to
  one-shot subscription semantics);
- **Task reclamation**: background subscription tasks are all registered; `shutdown()` / `Drop` aborts them all.

### API Overview

| Method | Signature | Description |
|---|---|---|
| `new` | `new(url: &str) -> OxCacheResult<Self>` | Constructs and warms up the publishing connection (fail-fast) |
| `with_reconnect_attempts` | `with_reconnect_attempts(self, attempts: usize) -> Self` | Reconnect attempts (default 5; 0 = no reconnect) |
| `publish` | `publish(&self, channel: &str, message: &str) -> OxCacheResult<i64>` | Sends a message, returning the receiver count |
| `subscribe` | `subscribe(&self, channel: &str, handler: Arc<dyn Fn(String) + Send + Sync>) -> OxCacheResult<()>` | Subscribes to a channel; the handler receives messages one by one in a background task |
| `shutdown` | `shutdown(&self) -> usize` | Aborts all background subscription tasks, returning the stopped count |

## 🔄 Sync API

The sync API mirrors the async API. Runtime requirements differ by configuration path (consistent with the README "runtime notes", the architecture documentation, and the CacheBuilder chapter below):

- **Default Moka path (including the native sync surface injected via `sync_backend_arc`)**: sync methods connect directly to the natively-sync implementation, runtime-independent — callable directly outside a runtime (driven by a temporary current-thread runtime); on a `multi_thread` runtime the current runtime is reused via `tokio::task::block_in_place`; only **inside the async context** of a current-thread runtime it explicitly returns `Err(NotSupported)` (tokio forbids nested blocking drivers)
- **The `backend_arc(...)` + `sync_mode(true)` bridged path (`AsyncToSyncBridge`)**: every sync call blocks on the async surface via `block_in_place` + `block_on`, requiring the caller to be on a **multi-thread** Tokio runtime (I/O-backed backends need ≥2 workers); outside a runtime or on a current-thread runtime every call returns `Err(NotSupported)`

### The `SyncCacheBackend` Trait Hierarchy

For the full definition of the sync trait hierarchy (`SyncCacheReader` / `SyncCacheWriter` / `SyncCacheConnector` / `SyncCacheBackend`), see the [backend layer chapter of the architecture documentation](ARCHITECTURE.md#3-backend-layer).

Implementors: `MokaMemoryBackend`, `DashMapMemoryBackend`, `RedisBackend`.

### Enabling Sync on `Cache<K, V>`

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await?;

    // 同步方法现在可用
    cache.set_sync(&"k".to_string(), &"v".to_string())?;
    let v = cache.get_sync(&"k".to_string())?; // Some("v")
    Ok(())
}
```

When injecting a custom sync backend, use `sync_backend_arc` (the backend is stored as an `Arc<dyn SyncCacheBackend>`:
the sync API connects directly, and the async API is bridged through the `SyncBackendAdapter` facade):

```rust
use oxcache::backend::{MokaMemoryBackend, SyncCacheBackend};

let moka = MokaMemoryBackend::builder().capacity(10_000).build();
let sync_backend: std::sync::Arc<dyn SyncCacheBackend> = std::sync::Arc::new(moka);
let cache: Cache<String, String> = Cache::builder()
    .sync_backend_arc(sync_backend)
    .sync_mode(true)
    .build_sync()?;
```

### Sync Methods on `Cache<K, V>`

| Method | Description |
|------|------|
| `get_sync(&K)` | `OxCacheResult<Option<V>>` |
| `set_sync(&K, &V)` | `OxCacheResult<()>` (no per-entry TTL) |
| `set_with_ttl_sync(&K, &V, Option<Duration>)` | `OxCacheResult<()>` |
| `delete_sync(&K)` | `OxCacheResult<()>` |
| `exists_sync(&K)` | `OxCacheResult<bool>` |
| `ttl_sync(&K)` | `OxCacheResult<Option<Duration>>` |
| `expire_sync(&K, Duration)` | `OxCacheResult<bool>` |
| `get_or_sync(&K, fallback)` | `OxCacheResult<V>` (single flight, synchronous) |
| `clear_sync()` | `OxCacheResult<()>` |

When `sync_mode` is `false` (the default), all `*_sync` methods return
`Err(OxCacheError::NotSupported)`.

Audit events: `*_sync` paths publish with the same semantics as the async paths (`audit` feature); runtime-independent publishers (`InMemoryAuditPublisher` / `TracingAuditPublisher`) can be used from sync calls outside a runtime.

## 🌸 Bloom Filter

The `bloom` feature (now part of `full`, and still available standalone) provides negative-query filtering.
`BloomFilterBackend` wraps any `CacheBackend`, skipping the inner backend when the bloom filter says the key is absent.

### `BloomFilter`

```rust
use oxcache::features::bloom_filter::BloomFilter;

let bf = BloomFilter::new(100_000, 0.01); // 容量、误判率
bf.insert("key1");
assert!(bf.contains("key1"));     // 已插入的键始终返回 true
assert!(!bf.contains("absent"));  // 通常为 false（可能误判）
```

**Methods:** `insert(&str)`, `contains(&str) -> bool`, `clear()`, `len() -> u64`,
`is_empty() -> bool`, `capacity() -> usize`, `false_positive_rate() -> f64`,
`load_factor() -> f64`, `rebuild(new_capacity)`.

`BloomFilter` is `Clone`, sharing state through `Arc<RwLock<...>>`,
so an insertion on one clone is visible to all clones.

### `BloomFilterBackend`

```rust
use oxcache::backend::MokaMemoryBackend;
use oxcache::features::bloom_filter::{BloomFilterBackend, BloomFilterBackendBuilder};

let inner = MokaMemoryBackend::new();
let backend = BloomFilterBackend::new(inner);  // 装饰器包装内部后端

// 或经 builder 配置容量与误判率
let backend = BloomFilterBackend::builder()
    .capacity(10_000)
    .false_positive_rate(0.01)
    .inner(MokaMemoryBackend::new())
    .build()?;
```

`BloomFilterBackendBuilder` allows configuring the underlying `BloomFilter`
(capacity, false-positive rate). The decorator implements `CacheBackend`, so it can be used anywhere
a `CacheBackend` is needed (including `ChainCache` links and `CacheBuilder::backend_arc`).

## 🕒 TTL Management

All backends (Moka, DashMap, Redis, Valkey (via RedisBackend), Dragonfly, Aerospike, Disk, Mock, Chain, Bloom) support per-entry TTL through
`set(key, value, Some(ttl))`.

- **Moka** uses the `moka::Expiry` trait for true per-entry TTL, overriding the global TTL set on the builder.
- **DashMap** uses lazy expiry (entries expire when accessed).
- **Redis** uses `SET key value PX <ms>` (millisecond precision; `expire` is `PEXPIRE`).
- **Disk (RedbDiskBackend)** uses lazy expiry (lazy read-path determination + physical deletion); `with_default_ttl` provides the backend default TTL, which `set(ttl=None)` follows.

For the full per-backend behavior reference, see the [TTL Behavior Reference of the architecture documentation](ARCHITECTURE.md#️-ttl-behavior-reference).

### TTL Methods

| Method | Async | Sync |
|------|------|------|
| Read remaining TTL | `cache.ttl(&key).await` | `cache.ttl_sync(&key)` |
| Update an existing key's TTL | `cache.expire(&key, d).await` | `cache.expire_sync(&key, d)` |
| Set an explicit TTL | `cache.set_with_ttl(&key, &v, Some(d)).await` | `cache.set_with_ttl_sync(&key, &v, Some(d))` |

## ⏳ SWR Three-State Expiry (`stale`)

- `StaleWhileRevalidateBackend` (`oxcache::features::stale`) — decorates any `CacheBackend`: a dual-timestamp envelope (`expire_at`/`stale_at`), three-state reads of Actual/Stale/Expired; legacy non-envelope data passes through as fresh; physical TTL = `ttl + stale_ttl`; `get_with_state(key)` returns `(payload, StaleState)`
- `StalePolicy` — `Return` (stale fallback, default) / `Revalidate` (synchronous revalidation) / `OffloadRevalidate` (return stale immediately + background refresh); enabled via `CacheBuilder::stale_ttl(Duration)` + `stale_policy(StalePolicy)`
- `Cache::get_or_refresh(key, ttl, fallback)` — the background-refresh variant of `get_or` (effective under `OffloadRevalidate`; the fallback needs `Send + 'static`); `get_or` degrades to Return under that policy
- `Cache::offload_manager()` — accessor for the Offload manager (for `wait_all` before graceful shutdown)

## 📡 Offload Background Tasks (`offload`)

- `OffloadManager::new(max_concurrent)` / `with_policy(max, TimeoutPolicy)` — per-key dedup + semaphore concurrency cap
- `spawn(key, future) -> bool` — `false` means the key is already in flight (dedup) or the permit is full (over-limit drop, no queuing)
- `is_in_flight` / `in_flight_count` / `cancel_all` / `wait_all(timeout) -> usize`
- `TimeoutPolicy` — `None` / `Cancel(Duration)` (cancel on timeout) / `Warn(Duration)` (default `Warn(30s)`, warn only)
- Metrics: `oxcache_offload_{spawned,deduplicated,completed,timeout}_total` + the gauge `oxcache_offload_active`

## 💾 Disk-Persistent Backend (`disk`)

- `RedbDiskBackend` (`oxcache::backend::disk`) — redb 4.3 embedded disk persistence (pure safe Rust, ACID + WAL); `create(path)` / `open(path)` / `with_default_ttl` / `with_max_entries`
- Lazy expiry: lazy read-path determination + physical deletion (same policy as DashMap); the `max_entries` sweep deletes expired entries first, then the oldest by ascending seq
- `BackendKind::Disk`; `BackendScore`: score 85 (`Scores::REDB`) / persistent / name `"disk"`; chained as L3 via `ChainLink::from_arc` or `ChainBuilder::extra_backend`
- `stats()` returns only the database **file name**, not the absolute path (the full path is available via `RedbDiskBackend::path()`)

## 🔗 Chain Read Strategies (`ChainReadStrategy`)

- `Sequential` (default, return on first layer hit) / `Race` (concurrent full read taking the highest-scored hit) / `ParallelFreshest` (concurrent full read, then pick the freshest by remaining TTL, None lowest priority, ties by highest score)
- `ChainCacheBuilder::read_strategy(strategy)`; `enable_race_read()` / `disable_race_read()` are compatibility aliases

## 🔁 Adaptive TTL (`adaptive-ttl`)

- `AdaptiveTtlBackend` (`oxcache::features::adaptive_ttl`) — decorates any `CacheBackend`: adjusts per-entry TTL by access pattern. For hot keys (hit count ≥ `hot_threshold`), `set` multiplies the TTL by `hot_ttl_multiplier`, and on `get`, subject to the `adjust_interval` rate limit, existing entries are `expire`d to `clamp(hot_ttl_multiplier × remaining TTL)` (either direction, bounding write amplification); when a known key's last access is older than `cold_idle_after`, `set` divides the TTL by `cold_ttl_divisor`; when both hot and cold apply, hot wins
- `AdaptiveTtlConfig` — all thresholds are explicit constants (no black-box heuristics): adjustment results are clamped to `[min_ttl, max_ttl]` (default 1s..1h); `None` (never expires) passes through untouched; the tracking table is capped by `max_tracked_keys` (default 65 536; 0 is explicitly rejected by `validate()`), and when full, new keys pass through as ordinary keys; the configuration is validated at build time via `validate()` — `min_ttl > max_ttl`, a non-finite positive `hot_ttl_multiplier` (0/negative/NaN/inf), `cold_ttl_divisor = 0`, and `max_tracked_keys = 0` all explicitly return `Err(InvalidInput)` (rather than a request-path `Duration::clamp` panic or silent distortion via multiply/divide)
- Enabling: `CacheBuilder::adaptive_ttl(AdaptiveTtlConfig)`; combining with `sync_mode(true)` / `stale_ttl` is explicitly rejected at build time (`Err(NotSupported)`)
- Observability: `stats()` / `reset_stats()` expose the tracked-key count and extension/shortening counts; when the get path's proactive adjustment hits a backend `expire` failure it does not block the hit and counts explicitly in `failed_adjustments`; the backend's `stats()` gains `adaptive_tracked_keys` / `adaptive_hot_extensions` / `adaptive_cold_shortenings` / `adaptive_failed_adjustments`

## 🔥 Smart Warm-up (`warmup`)

- `WarmupLoader` (`oxcache::cache::warmup`, re-exported at the crate root) — the warm-up data-source port (object safe, injected as `Arc<dyn WarmupLoader>`): `load_hot_keys()` returns `Vec<WarmupEntry>`; the implementation source is decided by the caller (business forecasts, `HotKeyTracker` snapshots, upstream analytics exports, etc.). A port failure explicitly aborts the round with `Err`; the cache itself is unaffected
- `WarmupEntry` — one hot key: `value = Some(bytes)` is a direct-supply value backfill (source data / snapshot path); `None` is key-only, promoted from lower layers via the `ChainCache::iter_entries` batch read (no value on the chain counts as `missing`)
- `Warmup::builder(chain).concurrency(n).build()` — the backfill write concurrency cap (default 8; 0 is treated as 1, serial); `run(loader)` executes asynchronously, writes go through `ChainCache::set` to all backends, and a Semaphore sliding window keeps in-flight tasks constant (in-flight ≤ the cap at any moment); `.max_value_bytes(n)` caps the size of one direct-supply value (default aligned with the serialization write surface's `MAX_JSON_SIZE`; 0 = unbounded), `.max_entries(n)` caps entries per round (default 100_000; 0 = unbounded, keeping the first N after dedup in loader order)
- `WarmupReport` — full explicit result counts: `loader_entries` (before dedup) / `deduped` (first occurrence wins) / `warmed` (successful direct-supply backfill) / `promoted` (successful lower-layer promotion; the backfill forwards the chain's remaining TTL without resetting the source entry's expiry semantics) / `missing` (no value on the chain) / `failed` + `failures` (per-item write failure details, not interrupting the batch) / `peak_concurrency` (measured concurrency peak, ≤ the configured cap) / `dropped_value_too_large` (explicitly dropped over the single-value cap) / `dropped_over_entry_cap` (explicitly dropped over the entry cap)

## 🔒 Security Features

Security functions are re-exported at the **crate root**, not under `oxcache::security`:

```rust
use oxcache::{
    validate_redis_key, validate_lua_script, validate_scan_pattern, clamp_scan_count,
    redact_cache_key, redact_connection_string, redact_field, redact_value, Redacted,
    // 安全日志辅助（需 `redis` 或 `full` 特性）：
    // log_cache_key, sanitize_message,
};
```

The redaction helpers (`redact_cache_key` / `redact_connection_string` / `redact_field` /
`redact_value` / `Redacted`) do not depend on the redis backend and are available under any feature combination — key
redaction in telemetry spans, audit events and `CacheEvent` also works in builds without redis.
`validate_redis_key` / `validate_lua_script` / `validate_scan_pattern` /
`clamp_scan_count` / `log_cache_key` / `sanitize_message` require `redis` or `full`.

> **Import path note:** use `use oxcache::validate_redis_key` (crate-root re-export),
> not `use oxcache::security::validate_redis_key`. The `security` module itself is
> `pub(crate)` and not directly accessible. For the complete threat model and security design, see the security document.

### Key Validation (validate_redis_key)

Validates Redis key format and content.

```rust
use oxcache::validate_redis_key;

validate_redis_key("user:123").expect("合法的键");
// 空、过长或包含危险字符的键返回 Err(OxCacheError::InvalidInput)
```

The validation rules (512 KB length cap, `\r` / `\n` / `\0` and command-injection characters, control characters, SQL-injection and path-traversal pattern scans) live in the security document.

### Lua Script Validation (validate_lua_script)

Validates the safety of Lua scripts.

```rust
use oxcache::validate_lua_script;

validate_lua_script("return redis.call('GET', KEYS[1])", 1).expect("合法的脚本");
```

The validation rules (10 KB script and 100-key caps, dangerous-command blacklist, sandbox-escape call interception, comment preprocessing against bypass) live in the security document.

Dangerous commands are judged by the **invoked command name** (not literal substring matching): `redis.call ( 'FLUSHALL' )`,
`redis.call('FLUSH'..'ALL')`, `redis["call"]('FLUSHALL')` and
`redis.call('FLUSHALL')` are equivalent and all rejected; when the command name cannot be statically determined (variables,
`unpack(ARGV)` etc.), it is treated as rejected.

### SCAN Validation (validate_scan_pattern and clamp_scan_count)

Validates SCAN patterns to prevent ReDoS attacks.

```rust
use oxcache::validate_scan_pattern;

validate_scan_pattern("user:*").expect("合法的模式");
```

The validation rules (256-character and 10-wildcard caps) and `clamp_scan_count`'s 1–1000 clamp live in the security document.

### Sensitive-Data Redaction

```rust
use oxcache::{redact_connection_string, redact_cache_key, redact_value, Redacted};

let safe = redact_connection_string("redis://:secret@host:6379");
assert!(!safe.contains("secret"));
```

`Redacted` is a wrapper type preventing the inner value from being accidentally logged.
Use `redact_field` / `redact_value` to wrap sensitive data in logs.

## 📈 Observability

### Metrics (`metrics` feature)

Cache statistics and export helper functions are re-exported at the crate root:

```rust
use oxcache::{CacheStats, get_enhanced_stats, export_json_format, export_prometheus_format};

let stats: CacheStats = get_enhanced_stats();
println!("L1 命中: {}", stats.l1_hits);
println!("整体命中率: {:.2}%", stats.overall_hit_rate() * 100.0);

// 导出为 Prometheus / JSON 文本（export_json_format 返回 Result）
let prometheus_text = export_prometheus_format();
let json_text = export_json_format()?;
```

`CacheStats` exposes L1/L2 hits, misses and operation counts as public fields (`l1_hits` / `l1_misses` /
`l2_hits` / `l2_misses` / `total_operations` etc.), plus the hit-rate methods `l1_hit_rate` /
`l2_hit_rate` / `overall_hit_rate` and the instance methods `export_prometheus` / `export_json`.
`MetricsCollector` (in `oxcache::infra::metrics::backend`) provides the low-level
counters (L1/L2 hit/miss, operation counters); standard exposition-format export goes through
`export_prometheus_standard()`.

**Per-service dimension (off by default)**: after an explicit `.service_name("orders")` on the builder, the cache's operation counters gain a `service` label — `export_prometheus_standard()` emits the `oxcache_service_operations_total{service="orders"}` line, and the JSON export
(`export_json_format` / `snapshot()`) gains a `service_operations` section; the label
cardinality cap is `MetricsConfig::max_service_labels` (default 64); over-limit attributions are counted in
`oxcache_service_labels_overflow_total`. When unset, the export is byte-for-byte identical to the existing format;
an explicit `.metrics(...)` injection takes precedence over `service_name`.

### Event Emission (`EventPublisher`)

Oxcache provides a structured event-emission mechanism through the `EventPublisher` trait.
When a backend operation fails, `ChainCache` raises an event through the configured `EventPublisher`,
and the user decides how to handle it (logging, metrics, alerting, or ignoring).

```rust
use oxcache::EventPublisher;  // crate 根重导出（core 模块为私有）
use std::sync::Arc;

// 实现自定义事件发布器
struct MyPublisher;
impl EventPublisher for MyPublisher { /* ... */ }

// 配置到 ChainCache
let chain = ChainCache::builder()
    .link(ChainLink::from_backend(l1))
    .event_publisher(Arc::new(MyPublisher))
    .build();
```

The built-in metrics implementation is available through the `metrics` feature (no external OpenTelemetry dependencies;
OTLP export, if needed, should be handled at the application layer). The `metrics` feature pulls in `serialization`,
`chrono` and `dashmap` for internal statistics collection and JSON export.

`CacheEvent` keys are redacted before publishing (`redact_cache_key`), and the `Debug` output
is likewise redacted with error text truncated — a consumer printing an event directly with `{:?}` will not write the raw key into logs.

### `MetricsConfig` (`metrics` feature)

The collection-surface configuration of `UnifiedMetrics` / `UnifiedMetricsRecorder` (injected via `with_config`):

| Field | Type | Default | Description |
|------|------|--------|------|
| `detailed` | `bool` | `true` | Whether to collect detailed dimensions (including histograms and duration distributions) |
| `histogram_buckets` | `Vec<f64>` | `0.1, 0.5, 1, 2.5, 5, 10, 25, 50, 100, 250, 500, 1000` | Histogram bucket boundaries (milliseconds) |
| `max_dynamic_metrics` | `usize` | 1000 | Cap on dynamic metric names; beyond it no new series are created |
| `max_service_labels` | `usize` | 64 | Cardinality cap for `service` dimension labels: new attributions beyond the cap are dropped and counted in `oxcache_service_labels_overflow_total`, guarding against label explosion |
| `retention_period` | `Option<Duration>` | `Some(3600 seconds)` | **Currently not consumed by any eviction logic** (the field is reserved for snapshot lifecycle management; setting it has no effect) |

## ⚙️ Unified Configuration Hub (CacheConfig)

`oxcache::config::CacheConfig` carries the full set of cache-building parameters in a single structure, supporting three configuration paths:
programmatic builder, `OXCACHE_*` environment variables, and confers config sources (the `config-confers` feature).

```rust
use oxcache::config::CacheConfig;
use std::time::Duration;

// 程序化构建
let config = CacheConfig::builder()
    .capacity(10_000)
    .ttl(Duration::from_secs(60))
    .backend("redis")
    .redis_url("redis://127.0.0.1:6379")
    .build();

// 一致性检查（值域 / 参数组合 / feature 可用性）
config.validate()?;

// 应用到既有构建器
let builder = config.apply_to_cache_builder(oxcache::CacheBuilder::<String, String>::default()).await?;
```

### Environment variable convention (`CacheConfig::try_from_env()`)

| Environment variable | Type | Mapped field | Feature prerequisite |
| --- | --- | --- | --- |
| `OXCACHE_CAPACITY` | u64 | `capacity` | — |
| `OXCACHE_TTL_MS` | u64 (milliseconds) | `ttl` | — |
| `OXCACHE_TTI_MS` | u64 (milliseconds) | `tti` | — |
| `OXCACHE_NULL_CACHE_TTL_MS` | u64 (milliseconds) | `null_cache_ttl` | — |
| `OXCACHE_TTL_JITTER_FACTOR` | f64 | `ttl_jitter_factor` | — |
| `OXCACHE_SYNC_MODE` | bool | `sync_mode` | — |
| `OXCACHE_BACKEND` | enum string (moka/dashmap/redis/...) | `backend` | any of `memory`/`redis`/`disk` |
| `OXCACHE_METRICS` | bool | `metrics_enabled` | `metrics` |
| `OXCACHE_SERIALIZATION_FORMAT` | enum string (json/bincode/postcard) | `serialization_format` | `serialization` (bincode/postcard each need their sub-feature) |
| `OXCACHE_REDIS_URL` | string | `redis_url` | consumed at build time by `redis`/`dragonfly` |
| `OXCACHE_DISK_PATH` | string | `disk_path` | consumed at build time by `disk` |
| `OXCACHE_CONNECTION_POOL_SIZE` | usize | `connection_pool_size` | consumed at build time by `redis`/`dragonfly` (default 8) |
| `OXCACHE_CIRCUIT_BREAKER_FAILURE_THRESHOLD` | u64 | `circuit_breaker_failure_threshold` | consumed at build time by `redis` |
| `OXCACHE_CIRCUIT_BREAKER_RESET_TIMEOUT_MS` | u64 (milliseconds) | `circuit_breaker_reset_timeout` | consumed at build time by `redis` |
| `OXCACHE_SERVICE_NAME` | string | `service_name` | consumed at build time by metrics (service dimension; the empty string is explicitly rejected) |

Unset keys fall back to the underlying builder defaults (zero behavior drift); parse failures and "key set but the
corresponding feature not enabled" both error explicitly (with the variable name and original value), and booleans accept
`true/1/yes/on` and `false/0/no/off`. `redis_url` may carry credentials: its `Debug` output is redacted to `"***"`,
and serialization (`Serialize`) skips the whole field — it must never land on disk in plaintext.

### `OxcacheConfig` (confers snapshot, `config-confers` feature)

| Field | Type | Default | Description |
|------|------|------|------|
| `capacity` | `u64` | 10_000 (built-in) | L1 capacity (entries) — always a set value after the confers path maps it |
| `default_ttl_ms` | `u64` | 60_000 (built-in) | Default TTL (milliseconds) — ditto |
| `tti_ms` | `Option<u64>` | `None` | Default TTI (milliseconds) |
| `null_cache_ttl_ms` | `Option<u64>` | `None` | Penetration-guard null-cache TTL (milliseconds) |
| `ttl_jitter_factor` | `Option<f64>` | `None` (underlying default 0.1) | TTL jitter factor |
| `sync_mode` | `Option<bool>` | `None` | Sync API mode |
| `backend` | `Option<String>` | `None` | Raw backend string (moka/dashmap/redis/…) |
| `metrics_enabled` | `Option<bool>` | `None` | Metrics switch (requires `metrics`) |
| `redis_url` | `Option<String>` | `None` | Redis-compatible backend connection string (`Debug` redacted; `Serialize` skips the field) |
| `circuit_breaker` | `Option<CircuitBreakerSettings>` | `None` | Circuit params: `failure_threshold` / `recovery_timeout_ms` |

Out-of-range circuit-breaker thresholds and pool sizes **error explicitly** on the confers path (no clamping / no dropping); when a hot-reload reload is rejected, the old snapshot is retained and observable (`oxcache_config_reload_rejected_total`).

### `DistributedConfig` (`oxcache::config`)

Retry / circuit-breaker / health-check parameters for distributed backends, built via `DistributedConfig::builder()`:

| Field | Type | Default | Description |
|------|------|--------|------|
| `retry_count` | `u32` | 3 | Max retry attempts for recoverable operations |
| `retry_base_delay` | `Duration` | 100 milliseconds | Base retry delay (doubling each attempt) |
| `circuit_breaker_threshold` | `u32` | 5 | Consecutive failures before the breaker opens |
| `circuit_breaker_reset_timeout` | `Duration` | 30 seconds | Open → HalfOpen wait duration |
| `health_check_interval` | `Duration` | 60 seconds | Periodic health-check interval |

Builder methods share the names (`retry_count(count)` / `retry_base_delay(delay)` / `circuit_breaker_threshold(threshold)` / `circuit_breaker_reset_timeout(timeout)` etc.).

### confers config source (`config-confers` feature)

`CacheConfig::try_from_confers(&OxcacheConfig)` maps a confers snapshot into the unified configuration;
the key conventions are in the `oxcache::features::confers_config` module docs. Note: `OxcacheConfig`'s
`capacity`/`default_ttl_ms` have built-in defaults (10000 / 60000ms), and after the confers path maps them they are
always set values — even when the corresponding keys are not configured, those defaults override the underlying
builder defaults (unlike the env path's "unset = zero behavior drift"). `OXCACHE_SYNC_MODE` also applies to the confers path's
`cache.sync_mode`: it composes with `cache.backend` (the configured backend's sync API is
bridged through `AsyncToSyncBridge`, requiring a multi-thread runtime at call time).

## 🚨 Error Handling

### `OxCacheError`

All cache operations return `Result<T, OxCacheError>` (aliased as `oxcache::OxCacheResult<T>`).

```rust
use oxcache::OxCacheError;

match result {
    Ok(value) => /* ... */,
    Err(OxCacheError::NotFound(key)) => println!("键未找到: {}", key),
    Err(OxCacheError::NotSupported(msg)) => println!("不支持: {}", msg),
    Err(OxCacheError::Connection(msg)) => println!("连接错误: {}", msg),
    Err(OxCacheError::Serialization(msg)) => println!("序列化错误: {}", msg),
    Err(e) => println!("其他错误 ({}): {}", e.code(), e),
}
```

### Error Variants and Error Codes

| Variant | Error code | Recoverable | Description |
|------|--------|--------|------|
| `NotFound(String)` | `OXCACHE_001` | no | Key not found |
| `Connection(String)` | `OXCACHE_002` | yes | Network connection failure |
| `Serialization(String)` | `OXCACHE_003` | no | Serialization/deserialization failure |
| `Operation(String)` | `OXCACHE_004` | no | General operation failure |
| `Degraded(String)` | `OXCACHE_005` | no | Cache is in degraded mode |
| `L1Error(String)` | `OXCACHE_006` | no | L1 cache operation failure |
| `L2Error(String)` | `OXCACHE_007` | yes | L2 cache operation failure |
| `NotSupported(String)` | `OXCACHE_009` | no | Operation not supported (e.g. the sync API without `sync_mode` enabled) |
| `WalError(String)` | `OXCACHE_010` | no | WAL operation failure |
| `DatabaseError(String)` | `OXCACHE_011` | no | Database error |
| `RedisError(...)` | `OXCACHE_012` | yes | Redis error |
| `IoError(io::Error)` | `OXCACHE_013` | no | I/O error |
| `BackendError(String)` | `OXCACHE_014` | yes | Backend error |
| `Timeout(String)` | `OXCACHE_015` | yes | Operation timed out |
| `ShutdownError(String)` | `OXCACHE_016` | no | Shutdown error |
| `KeyTooLong(usize, usize)` | `OXCACHE_017` | no | Key exceeds the maximum length |
| `ValueTooLarge(usize, usize)` | `OXCACHE_018` | no | Value exceeds the maximum size |
| `BufferFull(String)` | `OXCACHE_019` | yes | Batch-write buffer is full |
| `InvalidInput(String)` | `OXCACHE_020` | no | Invalid input |
| `InvalidKey(String)` | `OXCACHE_021` | no | Invalid key |
| `LockError(String)` | `OXCACHE_022` | no | Lock poisoned |
| `ServiceNotFound(String)` | `OXCACHE_023` | no | Service is not in the registry |
| `Internal(String)` | `OXCACHE_024` | no | Internal state corrupted |

### Helper Methods

- `OxCacheError::code() -> &'static str`: stable error code (e.g. `"OXCACHE_009"`).
- `is_recoverable() -> bool`: returns `true` for `Connection`/`Timeout`/`RedisError`/`L2Error`/`BackendError`/`BufferFull`.
- `is_not_found() -> bool`, `is_connection_error() -> bool`, `is_degraded() -> bool`.

### `OxCacheConfigError` (`redis` feature)

Configuration-stage errors (returned by factory functions and builders):

```rust
pub enum OxCacheConfigError {
    MissingField(String),
    InvalidValue { field: String, reason: String },
    UnsupportedBackend(String),
    ConnectionFailed(String),
}
pub type OxCacheConfigResult<T> = std::result::Result<T, OxCacheConfigError>;
```

## 🏷 Type Aliases

```rust
pub type OxCacheResult<T> = std::result::Result<T, OxCacheError>;
pub type RedisMode = RedisModeType; // Standalone | Sentinel | Cluster | ValkeyStandalone
```

## 🧬 trait-kit Integration (trait-kit feature)

With the `trait-kit` feature enabled, oxcache provides the trait-kit `AsyncKit` integration module `oxcache::integrations::kit`:

| Type / Function | Description |
|------------|------|
| `OxcacheModule` | Implements `AsyncAutoBuilder`, building the `Arc<dyn CacheBackend + Send + Sync>` capability |
| `OxcacheConfig` | Module configuration (`backend: BackendType` enum Memory (default)/Redis/Chain, `capacity: u64` (default 10_000, mirroring the Moka builder), `ttl: Option<Duration>`, `tti: Option<Duration>`, `redis: Option<RedisConfig>` (required when `backend` is Redis or the chain contains Redis), `chain: Vec<ChainLinkConfig>` (consumed only by `BackendType::Chain`, ignored otherwise)). **Same name as `oxcache::features::confers_config::OxcacheConfig` but a different type**: this struct is an independent minimal configuration for kit assembly, deliberately not wired into the oxcache config system |
| `OxcacheBuildObserver` | Implements `BuildObserver`; registrable as a build observer via `AsyncKit::with_observer` |
| `register_cache_shutdown` | Maps a `CacheBackend` shutdown onto the `AsyncShutdownCoordinator` three phases |
| `CacheBackendDecorator` | Decorator type alias `Arc<dyn Fn(Arc<dyn CacheBackend>) -> Arc<dyn CacheBackend>>` |
| `register_cache_decorator` | Helper function calling `kit.decorate::<OxcacheModule>(decorator)` |

```rust
use oxcache::integrations::kit::*;
use trait_kit::prelude::*;
use std::sync::Arc;

let mut kit = AsyncKit::new();
kit.set_config(OxcacheConfig::default());
kit.with_observer(Arc::new(OxcacheBuildObserver));
kit.register::<OxcacheModule>().unwrap();

// 注册装饰器
register_cache_decorator(&kit, |backend| backend);

let built = kit.build().await.unwrap();
let backend = built.require::<OxcacheModule>().unwrap();

// 注册三阶段关闭
let coord = AsyncShutdownCoordinator::new();
register_cache_shutdown(&coord, backend.clone()).unwrap();
coord.shutdown().await.unwrap();
```

### `RedisConfig` and `ChainLinkConfig` (kit assembly surface)

`RedisConfig` is applied field by field to the `RedisBackendBuilder` in `apply_redis_config`:

| Field | Type | Default | Description |
|------|------|--------|------|
| `connection_string` | `String` | `""` (required, written in) | Redis connection string (e.g. `redis://127.0.0.1:6379`) |
| `pool_size` | `usize` | 8 | Connection pool size |
| `connection_timeout` | `Duration` | 2 seconds | Connection timeout |
| `retry_count` | `u32` | 3 | Retry attempts for recoverable errors |
| `retry_delay` | `Duration` | 100 milliseconds | Retry interval |
| `circuit_breaker_threshold` | `u32` | 5 | Consecutive-failure threshold for the circuit breaker |
| `circuit_breaker_reset_timeout` | `Duration` | 30 seconds | Circuit breaker Open→HalfOpen reset timeout |

`ChainLinkConfig` describes each layer of the chain: `backend: BackendType`, `score: u8` (higher scores first; defaults memory 100 / redis 50), `redis: Option<RedisConfig>` (required when `backend == Redis`).

## 💻 Examples

See the [examples/](../../examples/) directory for more usage examples:

| Directory | Content |
|------|------|
| [01_basics](../../examples/src/01_basics/) | Basic operations |
| [02_advanced](../../examples/src/02_advanced/) | Advanced features |
| [03_config](../../examples/src/03_config/) | Configuration |
| [05_database](../../examples/src/05_database/) | Database integration |
| [06_features](../../examples/src/06_features/) | Feature demos |

## 🎨 Feature Flags

> This section was migrated from the README feature-flags chapter: the itemized component-feature reference table.

Tier presets (`default = ["minimal"]`, L1 only):

```toml
oxcache = { version = "0.5.0", features = ["minimal"] }   # 仅 L1（默认）
oxcache = { version = "0.5.0", features = ["redis-tier"] } # L1 + L2 Redis（旧名 core 为兼容别名）
oxcache = { version = "0.5.0", features = ["full"] }      # 真全量：全部具名能力特性（含 bloom / kit / encrypt / integrity / 三种后端等）
```

| Flag | Description | Default |
|------|------|:----:|
| `minimal` | Preset: `memory` + `metrics` + `serialization` + `chrono`, L1 only | ✅ |
| `redis-tier` | Preset: `minimal` + `redis`, L1 + L2 (the legacy `core` remains as a compat alias) | ❌ |
| `full` | True full: `redis-tier` + every named capability feature (macros/compression/batch/Lua/locks/RedLock/three backends/bloom/kit/encrypt/integrity/config-confers etc.) | ❌ |
| `memory` | L1 in-memory backends (Moka + DashMap) | ❌ |
| `redis` | L2 distributed cache (Redis / Valkey, Standalone / Sentinel / Cluster) | ❌ |
| `dragonfly` | Dragonfly backend (Redis protocol compatible) | ❌ |
| `aerospike` | Aerospike backend (independent protocol) | ❌ |
| `macros` | `#[cached]` attribute macro (the `oxcache_macros` crate) | ❌ |
| `serialization` | JSON serialization (serde + serde_json + serde_stacker depth guard) | ❌ |
| `metrics` | Built-in metrics: latency histograms, operation counters, JSON / Prometheus export | ❌ |
| `batch` | `BatchWriter` buffered batch writes (capacity / time dual-threshold flush) | ❌ |
| `lua` | Lua script execution (requires `redis`) | ❌ |
| `test-util` | Testing utilities (exposes internal functions); `testing` is a deprecated alias | ❌ |
| `bloom` | Bloom-filter negative-query filtering (`BloomFilter` + `BloomFilterBackend`) | ❌ |
| `lock` | Distributed lock: TTL, watchdog auto-renewal, reentrant (requires `redis`) | ❌ |
| `redlock` | RedLock multi-node majority lock + fencing tokens (requires `lock`); `red-lock` is a deprecated alias | ❌ |
| `compression` | Adaptive compression: threshold-triggered zstd, legacy-gzip compatible reads | ❌ |
| `telemetry` | `tracing` facade: circuit / backfill / macro-passthrough instrumentation, zero overhead when off | ❌ |
| `invalidation` | Cross-instance invalidation bus: Redis Pub/Sub broadcast + keyspace-notification channel | ❌ |
| `encrypt` | Value-level encryption decorator (XChaCha20-Poly1305, key bound as AAD) | ❌ |
| `integrity` | Value integrity decorator (HMAC-SHA256, verification failure counts as a miss) | ❌ |
| `serde-bincode` | bincode 2 binary serialization format | ❌ |
| `postcard` | postcard binary serialization format | ❌ |
| `config-confers` | confers config-driven build + `ConfigBus` watch hot reload | ❌ |
| `degradation` | Auto-degradation and recovery (Active / Degraded / HalfOpen state machine) | ❌ |
| `audit` | Structured audit event stream (NoOp / bounded in-memory ring / tracing) | ❌ |
| `versioning` | Versioned CAS (in-memory + Redis WATCH/MULTI/EXEC implementations) | ❌ |
| `trait-kit` | trait-kit AsyncKit integration (`OxcacheModule` / health check / lifecycle / shutdown / decorators); `kit` is a deprecated alias | ❌ |
| `disk` | Disk-persistent L3 backend (embedded redb, lazy expiry + `max_entries` sweep, `Scores::REDB = 85`) | ❌ |
| `stale` | SWR three-state expiry: `StaleWhileRevalidateBackend` + `StalePolicy` (Return / Revalidate / OffloadRevalidate) (requires `offload`) | ❌ |
| `offload` | Background task subsystem: `OffloadManager` dedup / concurrency limit / timeout policies, `get_or_refresh` background revalidation | ❌ |
| `adaptive-ttl` | Adaptive TTL: `AdaptiveTtlBackend` adjusts per-entry TTL by access pattern (hot extension / cold shortening, explicit constant configuration) | ❌ |
| `warmup` | Smart warm-up: the `WarmupLoader` port pulls the hot-key set and backfills `ChainCache` asynchronously (batch promotion + direct-supply backfill, controllable dedup/concurrency, explicit report counts) | ❌ |
| `byte-weight` | Byte-weighted sync cache (`ByteWeightCache`, tokio-free direct `moka::sync`) | ❌ |
| `pubsub` | Redis Pub/Sub broadcast component | ❌ |
| `hotkey` | Hot-key sampling observability (`HotKeyTracker` sharded counting + snapshot decayed Top-K) | ❌ |

> `bloom` and `trait-kit` are part of `full` now; enabling them alone (`--features bloom`) remains supported.
