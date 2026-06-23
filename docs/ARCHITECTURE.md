# Architecture Documentation

This document describes the architecture, design decisions, and technical details of the Oxcache library.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Components](#components)
- [Data Flow](#data-flow)
- [Consistency Model](#consistency-model)
- [Failure Handling](#failure-handling)
- [Performance Optimization](#performance-optimization)
- [Security](#security)
- [Scalability](#scalability)

## Overview

Oxcache is a multi-level caching system designed for high-performance, production-ready applications. It combines:

- **L1 Cache**: In-memory cache using Moka (LRU/TinyLFU eviction)
- **L2 Cache**: Distributed cache using Redis
- **Sync Layer**: Pub/Sub-based invalidation for multi-instance consistency
- **Recovery Layer**: Write-ahead log (WAL) for durability and failover

### Design Goals

1. **Performance**: L1 latency 50-100ns, L2 latency 1-5ms (P99, varies by environment)
2. **Reliability**: Automatic failover, data consistency across instances
3. **Usability**: Zero-boilerplate integration via `#[cached]` macro
4. **Observability**: Comprehensive metrics, tracing, and health checks
5. **Security**: Protection against cache penetration and DoS attacks

## Architecture

```mermaid
graph TD
    A[Application<br/>Functions with #[cached]] --> B[Internal Registry<br/>CACHE_REGISTRY]

    B --> C[Cache&lt;K,V&gt;]
    B --> D[Backend Layer]

    C --> E[CacheBuilder]
    D --> F[MemoryBackend]
    D --> G[RedisBackend]
    D --> H[TieredBackend]

    F --> I[L1 Cache<br/>Moka]
    G --> J[L2 Cache<br/>Redis]
    H --> I
    H --> J

    D --> K[Sync Layer<br/>Pub/Sub]
    D --> L[Recovery<br/>WAL]

    K --> M[Pub/Sub Channel]
    L --> N[WAL Storage]

    style A fill:#e1f5fe
    style B fill:#f3e5f5
    style C fill:#e8f5e8
    style D fill:#fff3e0
    style E fill:#f1f8e9
    style F fill:#e8f5e8
    style G fill:#fdf2e9
    style H fill:#fff3e0
    style I fill:#f1f8e9
    style J fill:#fdf2e9
    style K fill:#fff3e0
    style L fill:#fce4ec
    style M fill:#fff3e0
    style N fill:#fce4ec
```

## Components

### 1. Internal Cache Registry (`internal.rs`)

**Responsibility**: Central registry for all cache instances used by the `#[cached]` macro

**Data Structures**:
- `CACHE_REGISTRY: OnceLock<DashMap<String, Arc<dyn CacheOps>>>`: Thread-safe service-to-client mapping

**Key Internal Functions**:
- `__internal_register_cache(service_name, cache)`: Register a cache instance
- `__internal_get_cache(service_name)`: Retrieve cache by service name
- `__internal_remove_cache(service_name)`: Remove a cache registration
- `__internal_clear_all()`: Shutdown and clear all registered caches

**Thread Safety**: Uses `DashMap` for lock-free concurrent access and `OnceLock` for lazy initialization

**Usage Pattern**:
```rust
// Register cache for #[cached] macro
let cache: Cache<String, User> = Cache::memory().await?;
cache.register_for_macro("my_service").await;

// Macro automatically retrieves cache from registry
#[cached(service = "my_service", ttl = 300)]
async fn get_user(id: u64) -> User { ... }
```

### 2. Cache Interface (`cache/`)

**Responsibility**: Unified type-safe cache interface

**Module Structure**:
- `cache/mod.rs` - Module root and re-exports
- `cache/builder/` - CacheBuilder implementation
- `cache/api/` - Cache operation implementations (basic_ops, batch_ops, bytes_ops, macros)
- `cache/chain.rs` - ChainCache for multi-level backends
- `cache/interface.rs` - UnifiedCache trait

**Key Types**:
- `Cache<K, V>`: Main cache type with generic key and value types
- `CacheBuilder`: Builder for creating configured cache instances
- `ChainCache`: Multi-level cache chain
- `ChainLink`: Individual link in a cache chain

**Key Methods**:
- `new()`: Create cache with default memory backend
- `builder()`: Create cache builder for advanced configuration
- `get(key)`: Get value from cache
- `set(key, value)`: Set value in cache
- `get_or(key, fallback)`: Get value or compute using fallback (single-flight)
- `get_bytes(key)`: Get raw bytes from cache
- `set_bytes(key, value)`: Set raw bytes in cache
- `register_for_macro(service_name)`: Register for #[cached] macro
- `shutdown()`: Shutdown cache and release resources

**Thread Safety**: All operations are thread-safe via Arc<dyn CacheBackend>

**Usage Pattern**:
```rust
// Create cache using Builder
use oxcache::Cache;

let cache: Cache<String, User> = Cache::builder()
    .redis("redis://localhost:6379")
    .build()
    .await?;

// Or create tiered cache (L1 + L2)
let cache: Cache<String, User> = Cache::builder()
    .tiered(10000, "redis://localhost:6379")
    .ttl(Duration::from_secs(3600))
    .build()
    .await?;

// Register for macro usage
cache.register_for_macro("my_service").await;

// Use cache
cache.set(&"user:1".to_string(), &user).await?;
let user: Option<User> = cache.get(&"user:1".to_string()).await?;
```

### 3. Backend Layer (`backend/`)

**Responsibility**: Pluggable cache backend implementations

**Module Structure**:
- `backend/mod.rs` - Module root and re-exports
- `backend/interface.rs` - CacheBackend, CacheReader, CacheWriter traits
- `backend/memory/` - Memory backend implementations (Moka, DashMap, Redis)
- `backend/custom_tiered.rs` - Custom tiered backend configuration
- `backend/score.rs` - Backend scoring system

**Backend Types**:
- `MokaMemoryBackend`: In-memory cache using Moka (LRU/TinyLFU eviction)
- `DashMapMemoryBackend`: Pure in-memory concurrent cache using DashMap
- `RedisBackend`: Redis distributed cache (Standalone/Sentinel/Cluster)
- `ChainCache`: Multi-level cache chain

**Backend Traits**:
```rust
#[async_trait]
pub trait CacheReader: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn stats(&self) -> Result<HashMap<String, String>>;
    async fn health_check(&self) -> Result<bool>;
}

#[async_trait]
pub trait CacheWriter: Send + Sync {
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn clear(&self) -> Result<()>;
}
```

**Tiered Backend Read Path**:
```
1. Check L1 cache (MemoryBackend)
2. If hit → Return value
3. If miss → Check L2 cache (RedisBackend)
4. If L2 hit → Populate L1 → Return value
5. If L2 miss → Return None
```

**Tiered Backend Write Path**:
```
1. Write to L1 cache (async, immediate)
2. Write to L2 cache (async, can be batched)
3. Write to WAL for durability
4. Publish invalidation if needed
```

### 4. Features Module (`features/`)

**Responsibility**: Optional capabilities and runtime feature information

**Key Functions**:
- `get_l1_feature_info()`: Get L1 cache feature status
- `get_l2_feature_info()`: Get L2 cache feature status
- `get_all_feature_info()`: Get all feature status
- `is_l1_enabled()`: Check if L1 is enabled
- `is_l2_enabled()`: Check if L2 is enabled

### 5. Infrastructure Module (`infra/`)

**Responsibility**: Metrics, serialization, telemetry, and observability

**Sub-modules**:
- `metrics/` - Cache metrics collection and export
- `serialization/` - Data serialization utilities
- Telemetry integration with OpenTelemetry

**Key Types**:
- `CacheStats`: Enhanced statistics with hit rates
- `MetricsCollector`: Metrics collection
- Export functions for Prometheus and JSON formats

### 6. Security Module (`security/`)

**Responsibility**: Input validation and security measures

**Sub-modules**:
- `validation.rs` - Redis key, Lua script, SCAN pattern validation
- `redaction.rs` - Sensitive data redaction in logs
- `log.rs` - Secure logging utilities
- `regex.rs` - Pattern matching for security checks

**Key Functions**:
- `validate_redis_key(key)`: Validate Redis key format
- `validate_lua_script(script, num_keys)`: Validate Lua scripts
- `validate_scan_pattern(pattern)`: Validate SCAN patterns
- `clamp_scan_count(count)`: Clamp SCAN count to safe range
- `redact_value(value)`: Redact sensitive values in logs

### 7. Key Generator (`utils/`)

**Responsibility**: Cache key generation and management

**Key Types**:
- `KeyGenerator`: Utility for generating cache keys with namespaces and prefixes

**Key Methods**:
- `new()`: Create default key generator
- `with_namespace(ns)`: Set namespace for key isolation
- `with_prefix_str(prefix)`: Set prefix for key organization
- `generate(template, params)`: Generate key from template
- `generate_full(template, params)`: Generate key with namespace and prefix
- `validate_key(key)`: Validate key format

**Usage Pattern**:
```rust
let gen = KeyGenerator::new()
    .with_namespace("myapp")
    .with_prefix_str("cache");

let key = gen.generate_full("user:{id}", &[("id", "123")]);
// Result: "myapp:cache:user:123"
```

### 8. Events Module (`core/events.rs`)

**Responsibility**: Cache event system for monitoring and hooks

**Key Types**:
- `CacheEvent`: Event data structure
- `CacheEventType`: Event type enum (Hit, Miss, Set, Delete, etc.)
- `EventPublisher`: Event publishing interface

**Usage Pattern**:
```rust
let publisher = EventPublisher::new();
publisher.publish(CacheEvent {
    event_type: CacheEventType::Hit,
    key: "user:123".to_string(),
    timestamp: chrono::Utc::now(),
});
```

### 9. Config Module (`config/`)

**Responsibility**: Cache configuration management

**Key Types**:
- `UnifiedConfigBuilder`: Type-safe configuration builder
- `ServiceConfig`: Service-level configuration
- `L1Config`: L1 cache configuration
- `L2Config`: L2 cache configuration

**Configuration Options**:
```rust
let config = UnifiedConfigBuilder::tiered()
    .with_ttl(7200)
    .with_l1_capacity(10000)
    .with_redis_url("redis://localhost:6379")
    .with_redis_mode("standalone")
    .build();
```

### 10. Security Features

#### Input Validation

The security module (`security/`) provides comprehensive input validation:

#### Redis Key Validation
- Empty key rejection
- 512KB size limit
- Dangerous character detection (`\r`, `\n`, `\0`)
- SQL injection pattern detection
- Path traversal pattern detection

#### Lua Script Validation
- 10KB script length limit
- 100 key limit
- Dangerous command blocking: `FLUSHALL`, `FLUSHDB`, `KEYS`, `SHUTDOWN`, `DEBUG`, `CONFIG`, `SAVE`, `BGSAVE`, `MONITOR`
- Comment preprocessing to prevent bypass

#### SCAN Pattern Validation
- 256 character length limit
- 10 wildcard limit
- Count parameter clamping (1-1000)

#### Sensitive Data Redaction
- Connection string password redaction
- Cache key redaction in logs
- Value redaction for sensitive fields

## Data Flow

### #[cached] Macro Workflow

The `#[cached]` macro provides zero-boilerplate caching by automatically handling cache lookup, storage, and serialization:

```mermaid
sequenceDiagram
    participant App as Application
    participant Macro as #[cached] Macro
    participant Registry as CACHE_REGISTRY
    participant Cache as Cache<K,V>
    participant Backend as CacheBackend

    App->>Macro: Call cached function
    Macro->>Macro: Generate cache key
    Macro->>Registry: __internal_get_cache("service")
    Registry-->>Macro: Arc<dyn CacheOps>
    Macro->>Cache: get_bytes(key)
    Cache->>Backend: get(key)
    Backend-->>Cache: Some(Vec<u8>)
    Cache-->>Macro: Some(bytes)
    Macro->>Macro: Deserialize bytes
    Macro-->>App: Return cached value

    Note over App,Backend: Cache Miss Path
    Macro->>Macro: Execute original function
    Macro->>Macro: Serialize result
    Macro->>Cache: set_bytes(key, bytes)
    Cache->>Backend: set(key, bytes)
    Macro-->>App: Return result
```

**Macro Generated Code Structure**:
```rust
#[cached(service = "my_service", ttl = 300)]
async fn get_user(id: u64) -> Result<User> {
    // ... original function body ...
}
```

Expands to approximately:
```rust
async fn get_user(id: u64) -> Result<User> {
    let cache_key = format!("my_service:get_user:{:?}", id);

    // Get cache from registry
    let client = match oxcache::__internal_get_cache("my_service") {
        Some(c) => c,
        None => return { /* original code */ }.await,
    };

    // Try to get from cache
    if let Ok(Some(bytes)) = client.get_bytes(&cache_key).await {
        if let Ok(val) = client.serializer().deserialize::<User>(&bytes) {
            return Ok(val);
        }
    }

    // Execute original function
    let result = { /* original code */ }.await;

    // Cache result if successful
    if let Ok(ref val) = result {
        if let Ok(bytes) = client.serializer().serialize(val) {
            let _ = client.set_bytes(&cache_key, bytes, Some(300)).await;
        }
    }

    result
}
```

### Read Operation (with #[cached] macro)

```mermaid
flowchart TD
    A[Application<br/>#[cached] function] --> B[Generate cache key]
    B --> C[Get cache from<br/>CACHE_REGISTRY]
    C --> D{Cache found?}
    D -->|no| E[Execute function<br/>uncached]
    D -->|yes| F[get_bytes from cache]
    F --> G{Cache hit?}
    G -->|yes| H[Deserialize value]
    G -->|no| E
    H --> I[Return cached value]
    E --> J[Execute original code]
    J --> K{Result Ok?}
    K -->|yes| L[Serialize result]
    L --> M[set_bytes to cache]
    K -->|no| N[Return error]
    M --> O[Return result]

    style A fill:#e1f5fe
    style B fill:#fff3e0
    style C fill:#f3e5f5
    style D fill:#ffeb3b
    style E fill:#fce4ec
    style F fill:#fff3e0
    style G fill:#ffeb3b
    style H fill:#f1f8e9
    style I fill:#e8f5e8
    style J fill:#fce4ec
    style K fill:#ffeb3b
    style L fill:#f1f8e9
    style M fill:#fff3e0
    style N fill:#fce4ec
    style O fill:#e8f5e8
```

### Tiered Backend Read Path

```mermaid
flowchart TD
    A[Cache.get_bytes] --> B[TieredBackend.get]
    B --> C{Check L1<br/>MemoryBackend}
    C -->|hit| D[Return value]
    C -->|miss| E{Check L2<br/>RedisBackend}
    E -->|hit| F[Populate L1]
    F --> D
    E -->|miss| G[Return None]

    style A fill:#e1f5fe
    style B fill:#fff3e0
    style C fill:#fff3e0
    style D fill:#e8f5e8
    style E fill:#fff3e0
    style F fill:#f1f8e9
    style G fill:#fce4ec
```

### Write Operation (with #[cached] macro)

```mermaid
flowchart TD
    A[Application<br/>#[cached] function] --> B[Execute function]
    B --> C[Result Ok?]
    C -->|no| D[Return error]
    C -->|yes| E[Serialize result]
    E --> F[Get cache from<br/>CACHE_REGISTRY]
    F --> G[set_bytes to cache]
    G --> H{Cache type?}
    H -->|l1-only| I[set_l1_bytes]
    H -->|l2-only| J[set_l2_bytes]
    H -->|two-level| K[set_bytes to both]
    I --> L[Return result]
    J --> L
    K --> M[Write to L1<br/>immediate]
    M --> N[Batch write to L2]
    N --> L

    style A fill:#e1f5fe
    style B fill:#fce4ec
    style C fill:#ffeb3b
    style D fill:#fce4ec
    style E fill:#f1f8e9
    style F fill:#f3e5f5
    style G fill:#fff3e0
    style H fill:#ffeb3b
    style I fill:#f1f8e9
    style J fill:#fdf2e9
    style K fill:#fff3e0
    style L fill:#e8f5e8
    style M fill:#f1f8e9
    style N fill:#fdf2e9
```

## Consistency Model

### Eventual Consistency

Oxcache provides **eventual consistency** across instances:

- **Strong consistency within instance**: L1 + L2 are always consistent
- **Eventual consistency across instances**: Propagation delay of < 100ms typically

### Invalidation Propagation

```mermaid
sequenceDiagram
    participant A as Instance A
    participant P as Pub/Sub Channel
    participant B as Instance B

    A->>P: UPDATE key:123
    P->>B: INVALIDATE key:123
    Note over B: Remove from L1 if version < v5
```

### Versioning Scheme

```
Version format: "v{timestamp}_{instance_id}"

Example: "v1704921600_i32"

Compare versions lexicographically:
- v1704921600_i32 < v1704921601_i45  (newer wins)
```

## Failure Handling

### Redis Failure

**Detection**:
- Connection timeout
- Ping failure
- Connection closed by remote

**Recovery**:
```
1. Switch to L1-only mode
2. Log warning
3. Continue serving from L1
4. Reconnect in background
5. Replay WAL on reconnect
6. Resume normal operation
```

### Network Partition

**Behavior**:
- Instances continue operating with local data
- Invalidation messages queued
- On recovery: Reconcile using versioning

### Disk Failure (WAL)

**Degradation**:
- Pause WAL writes
- Log critical error
- Continue operating (less durable)

## Performance Optimization

### Optimization Techniques

1. **Batch Write**: Buffer multiple operations, flush with Redis MSET
2. **Connection Pooling**: Reuse Redis connections
3. **Lock-Free L1**: Moka's concurrent cache design
4. **JSON Serialization**: Human-readable, widely supported
5. **Compression**: Optional flate2 compression for large values

### Performance Tuning

```toml
[optimization]
# L1 cache
l1_max_capacity = 10000
l1_time_to_idle = 600

# L2 cache
l2_batch_size = 100
l2_batch_timeout_ms = 50

# Serialization
serialization_type = "json"
```

### Benchmark Results

> Test environment: M1 Pro, 16GB RAM, macOS, Redis 7.0
>
> **Note**: Performance varies based on hardware, network conditions, and data size.

| Operation | Throughput | Latency (P99) |
|-----------|------------|---------------|
| L1 Read | 5-10M ops/sec | 50-100ns |
| L1 Write | 2-5M ops/sec | 50-200ns |
| L2 Read | 50-100K ops/sec | 1-5ms |
| L2 Write (batch) | 200-500K ops/sec | 1-10ms |

## Security

### Threat Model

1. **Cache Penetration**: Attacker requests non-existent keys
2. **Cache Breakdown**: Hot key expires, many requests hit DB
3. **DoS Attack**: High request rate overwhelms system
4. **SQL Injection**: Malicious patterns in Redis keys
5. **Lua Script Injection**: Dangerous commands in Lua scripts
6. **ReDoS**: Malicious SCAN patterns causing CPU exhaustion

### Defenses

1. **Single-Flight**: Prevent cache breakdown with request deduplication
2. **Input Validation**: Comprehensive validation for keys, Lua scripts, and SCAN patterns
3. **Comment Preprocessing**: Prevent bypass via Lua comments
4. **Sensitive Data Redaction**: Auto-redact in logs
5. **Rate Limiting**: Token bucket algorithm for DoS protection

### Input Validation

The security module (`security/`) provides comprehensive input validation:

#### Redis Key Validation
- Empty key rejection
- 512KB size limit
- Dangerous character detection (`\r`, `\n`, `\0`)
- SQL injection pattern detection
- Path traversal pattern detection

#### Lua Script Validation
- 10KB script length limit
- 100 key limit
- Dangerous command blocking
- Comment preprocessing to prevent bypass

#### SCAN Pattern Validation
- 256 character length limit
- 10 wildcard limit
- Count parameter clamping (1-1000)

### Best Practices

1. **Key Design**: Use stable, predictable keys
2. **TTL Strategy**: Set appropriate TTL based on data volatility
3. **Access Control**: Use Redis AUTH + TLS
4. **Monitoring**: Track metrics for anomalies

## Scalability

### Horizontal Scaling

```mermaid
graph TD
    subgraph "Application Instances"
        I1[Instance 1]
        I2[Instance 2]
        I3[Instance 3]
    end

    subgraph "Redis Cluster"
        R[Redis Cluster]
    end

    I1 --> R
    I2 --> R
    I3 --> R

    style I1 fill:#e1f5fe
    style I2 fill:#e1f5fe
    style I3 fill:#e1f5fe
    style R fill:#f3e5f5
```

### Vertical Scaling

- Increase L1 capacity (more memory)
- Use faster Redis (SSD, dedicated server)
- Enable Redis persistence (AOF + RDB)

### Partitioning

Database partitioning for large datasets:
```rust
PartitionConfig::time_based(TimeUnit::Month)  // By month
PartitionConfig::hash_based(16)                // 16 shards
```

## Future Enhancements

1. **L3 Cache**: Add support for other distributed caches (Memcached, Cassandra)
2. **Adaptive TTL**: Machine learning-based TTL optimization
3. **Geo-Distribution**: Multi-region replication
4. **Cache Warming**: Intelligent warmup strategies
5. **Advanced Compression**: Zstd compression for large values

## References

- [Moka Documentation](https://github.com/moka-rs/moka)
- [Redis Documentation](https://redis.io/documentation)
- [TinyLFU Paper](https://arxiv.org/abs/1512.00757)
- [Bloom Filter](https://en.wikipedia.org/wiki/Bloom_filter)
