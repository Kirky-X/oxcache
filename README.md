# Oxcache

Oxcache is a high-performance, robust, and easy-to-use two-level caching library for Rust, designed to bridge the gap between local memory speed and distributed cache consistency.

## Features

- **Two-Level Caching**: Combines fast local memory (L1) with distributed Redis (L2).
- **Macro Support**: Easy-to-use `#[cached]` macro for automatic function result caching.
- **Cache Coherence**: Built-in Pub/Sub mechanism to invalidate L1 caches across instances on update.
- **Resilience**: 
  - **Single-Flight**: Prevents cache stampede (dog-piling) by deduplicating concurrent requests.
  - **WAL (Write-Ahead Log)**: Ensures data durability and recovery during partial outages.
  - **Degraded Mode**: Automatically degrades to local-only or limited functionality when dependencies fail.
  - **Graceful Shutdown**: Comprehensive shutdown mechanism for distributed systems with proper resource cleanup.
- **Flexibility**: Supports standalone, sentinel, and cluster Redis modes.
- **Observability**: Integrated metrics and tracing support.
- **Database Fallback**: Automatic database source fallback for enhanced reliability.
- **Configuration Management**: Advanced configuration with environment-based switching and audit logging.

## Quick Start

### 1. Add Dependency

Add `oxcache` to your `Cargo.toml`:

```toml
[dependencies]
oxcache = { path = "crates/infra/oxcache" } # Adjust path or version
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

### 2. Configuration

Create a `config.toml` file:

```toml
[global]
default_ttl = 300 # 5 minutes

[services.my_service]
cache_type = "two-level"
promote_on_hit = true

[services.my_service.l1]
max_capacity = 10000
ttl = 60

[services.my_service.l2]
mode = "standalone"
connection_string = "redis://127.0.0.1:6379"
```

### 3. Usage

#### Using Macros (Recommended)

```rust
use oxcache::macros::cached;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    id: u64,
    name: String,
}

// Automatically caches result with key "user:{id}"
#[cached(service = "my_service", key = "user:{id}", ttl = 300)]
async fn get_user(id: u64) -> Result<User, String> {
    // Simulate DB call
    Ok(User { id, name: "Alice".to_string() })
}

#[tokio::main]
async fn main() {
    // Initialize cache
    oxcache::init("config.toml").await.expect("Failed to init cache");

    // First call: Miss -> DB -> Cache
    let user = get_user(1).await.unwrap();
    
    // Second call: Hit -> Cache
    let user_cached = get_user(1).await.unwrap();
}
```

#### Manual Client Usage

```rust
use oxcache::get_client;

#[tokio::main]
async fn main() {
    oxcache::init("config.toml").await.unwrap();
    
    let client = get_client("my_service").unwrap();
    
    client.set("key", &"value", None).await.unwrap();
    
    let val: Option<String> = client.get("key").await.unwrap();
    assert_eq!(val, Some("value".to_string()));
}
```

## Advanced Features

### Manual Control

You can bypass the two-level logic and interact with specific layers directly:

```rust
client.set_l1_only("local_key", &value, None).await?;
client.set_l2_only("global_key", &value, None).await?;
```

### Stress Testing

Run the included stress test example to verify performance under load:

```bash
cargo run --example stress_test -- --concurrency 50 --duration 10
```

## Observability

Oxcache integrates with `tracing` and `opentelemetry` for comprehensive monitoring.

### Metrics

The following metrics are collected:

- `cache_requests_total`: Total number of cache requests (labels: service, layer, operation, result).
- `cache_operation_duration_seconds`: Histogram of operation duration.
- `cache_l2_health_status`: Current health status of L2 cache (0: Unhealthy, 1: Healthy, 2: Recovering).
- `cache_wal_entries`: Number of pending entries in the Write-Ahead Log.
- `cache_batch_write_buffer_size`: Current size of the batch write buffer.

### Tracing

To enable distributed tracing, initialize the telemetry module at startup:

```rust
use oxcache::telemetry::init_tracing;

init_tracing("my_app", Some("http://localhost:4317"));
```

## High Availability

Oxcache supports robust Redis configurations:

- **Standalone**: Single Redis instance.
- **Sentinel**: Redis Sentinel for automatic failover.
- **Cluster**: Redis Cluster for sharding and high availability.

Configure in `config.toml`:

```toml
[services.my_service.l2]
mode = "sentinel"
# ... sentinel config ...
```

## Graceful Shutdown

Oxcache provides comprehensive graceful shutdown functionality for distributed systems:

```rust
use oxcache::manager::shutdown_all;

#[tokio::main]
async fn main() {
    // Initialize cache
    oxcache::init("config.toml").await.expect("Failed to init cache");
    
    // Your application logic here
    
    // Graceful shutdown when application is terminating
    shutdown_all().await.expect("Failed to shutdown cache clients");
}
```

The shutdown mechanism ensures:
- Proper cleanup of all cache clients
- Resource deallocation
- Background task termination
- Error aggregation and reporting

## License

MIT
