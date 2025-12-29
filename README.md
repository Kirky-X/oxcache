<div align="center">

# 🚀 Oxcache

[![CI](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml/badge.svg)](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/oxcache.svg)](https://crates.io/crates/oxcache)
[![Documentation](https://docs.rs/oxcache/badge.svg)](https://docs.rs/oxcache)
[![Downloads](https://img.shields.io/crates/d/oxcache.svg)](https://crates.io/crates/oxcache)
[![codecov](https://codecov.io/gh/Kirky-X/oxcache/branch/main/graph/badge.svg)](https://codecov.io/gh/Kirky-X/oxcache)
[![Dependency Status](https://deps.rs/repo/github/Kirky-X/oxcache/status.svg)](https://deps.rs/repo/github/Kirky-X/oxcache)
[![Security Audit](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml/badge.svg?label=security)](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/oxcache.svg)](https://github.com/Kirky-X/oxcache/blob/main/LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)

[English](../README.md) | [简体中文](README_zh.md)

Oxcache is a high-performance, production-grade two-level caching library for Rust, providing L1 (Moka in-memory
cache) + L2 (Redis distributed cache) architecture.

</div>

## ✨ Key Features

<div align="center">

<table>
<tr>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/rocket.png" width="48"><br>
<b>Extreme Performance</b><br>L1 in nanoseconds
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/magic-wand.png" width="48"><br>
<b>Zero-Code Changes</b><br>One-line cache enable
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/cloud.png" width="48"><br>
<b>Auto Recovery</b><br>Redis fault degradation
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/synchronize.png" width="48"><br>
<b>Multi-Instance Sync</b><br>Based on Pub/Sub
</td>
<td width="20%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/lightning.png" width="48"><br>
<b>Batch Optimization</b><br>Smart batch writes
</td>
</tr>
</table>

</div>

- **🚀 Extreme Performance**: L1 nanosecond response (P99 < 100ns), L1 millisecond response (P99 < 5ms)
- **🎯 Zero-Code Changes**: Enable caching with a single `#[cached]` macro
- **🔄 Auto Recovery**: Automatic degradation on Redis failure, WAL replay on recovery
- **🌐 Multi-Instance Sync**: Pub/Sub + version-based invalidation synchronization
- **⚡ Batch Optimization**: Intelligent batch writes for significantly improved throughput
- **🛡️ Production Grade**: Complete observability, health checks, chaos testing verified

## 📦 Quick Start

### 1. Add Dependency

Add `oxcache` to your `Cargo.toml`:

```toml
[dependencies]
oxcache = "0.1"
```

> **Note**: `tokio` and `serde` are already included by default. If you need minimal dependencies, you can use
`oxcache = { version = "0.1", default-features = false }` and add them manually.

### 2. Configuration

Create a `config.toml` file:

```toml
[global]
default_ttl = 3600
health_check_interval = 30
serialization = "json"
enable_metrics = true

[services.user_cache]
cache_type = "two-level"  # "l1" | "l2" | "two-level"
ttl = 600

  [services.user_cache.l1]
  max_capacity = 10000
  ttl = 300  # L1 TTL must be <= L2 TTL
  tti = 180
  initial_capacity = 1000

  [services.user_cache.l2]
  mode = "standalone"  # "standalone" | "sentinel" | "cluster"
  connection_string = "redis://127.0.0.1:6379"

  [services.user_cache.two_level]
  write_through = true
  promote_on_hit = true
  enable_batch_write = true
  batch_size = 100
  batch_interval_ms = 50
```

### 3. Usage

#### Using Macros (Recommended)

```rust
use oxcache::macros::cached;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

// One-line cache enable
#[cached(service = "user_cache", ttl = 600)]
async fn get_user(id: u64) -> Result<User, String> {
    // Simulate slow database query
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok(User {
        id,
        name: format!("User {}", id),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize cache (from config file)
    oxcache::init("config.toml").await?;
    
    // First call: execute function logic + cache result (~100ms)
    let user = get_user(1).await?;
    println!("First call: {:?}", user);
    
    // Second call: return directly from cache (~0.1ms)
    let cached_user = get_user(1).await?;
    println!("Cached call: {:?}", cached_user);
    
    Ok(())
}
```

#### Manual Client Usage

```rust
use oxcache::{get_client, CacheOps};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    oxcache::init("config.toml").await?;
    
    let client = get_client("user_cache")?;
    
    // Standard operation: write to both L1 and L2
    client.set("key", &my_data, Some(300)).await?;
    let data: MyData = client.get("key").await?.unwrap();
    
    // Write to L1 only (temporary data)
    client.set_l1_only("temp_key", &temp_data, Some(60)).await?;
    
    // Write to L2 only (shared data)
    client.set_l2_only("shared_key", &shared_data, Some(3600)).await?;
    
    // Delete
    client.delete("key").await?;
    
    Ok(())
}
```

## 🎨 Use Cases

### Scenario 1: User Information Cache

```rust
#[cached(service = "user_cache", ttl = 600)]
async fn get_user_profile(user_id: u64) -> Result<UserProfile, Error> {
    database::query_user(user_id).await
}
```

### Scenario 2: API Response Cache

```rust
#[cached(
    service = "api_cache",
    ttl = 300,
    key = "api_{endpoint}_{version}"
)]
async fn fetch_api_data(endpoint: String, version: u32) -> Result<ApiResponse, Error> {
    http_client::get(&format!("/api/{}/{}", endpoint, version)).await
}
```

### Scenario 3: L1-Only Hot Data Cache

```rust
#[cached(service = "session_cache", cache_type = "l1", ttl = 60)]
async fn get_user_session(session_id: String) -> Result<Session, Error> {
    session_store::load(session_id).await
}
```

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Application Code                      │
│                  (#[cached] Macro)                       │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ↓
┌─────────────────────────────────────────────────────────┐
│                   CacheManager                           │
│        (Service Registry + Health Monitor)               │
└───┬─────────────────────────────────────────────────┬───┘
    │                                                  │
    ↓                                                  ↓
┌──────────────┐                              ┌──────────────┐
│ TwoLevelClient│                              │ L1OnlyClient │
│               │                              │ L2OnlyClient │
└───┬──────┬───┘                              └──────────────┘
    │      │
    ↓      ↓
┌────────┐ ┌────────────────────────────────────────┐
│  L1    │ │                L2                       │
│ (Moka) │ │              (Redis)                    │
│        │ │                                        │
└────────┘ └────────────────────────────────────────┘
```

**L1**: In-process high-speed cache using LRU/TinyLFU eviction strategy  
**L2**: Distributed shared cache supporting Sentinel/Cluster modes

## 📊 Performance Benchmarks

> Test environment: M1 Pro, 16GB RAM, macOS

```
Single-thread Latency Test (P99):
├── L1 Cache:  ~50ns
├── L2 Cache:  ~1ms
└── Database:   ~10ms

Throughput Test (batch_size=100):
├── Single Write:  ~10K ops/s
└── Batch Write:   ~50K ops/s
```

## 🛡️ Reliability

- ✅ Single-Flight (prevent cache stampede)
- ✅ WAL (Write-Ahead Log) persistence
- ✅ Automatic degradation on Redis failure
- ✅ Graceful shutdown mechanism
- ✅ Health checks and auto-recovery

## 📚 Documentation

- [📖 User Guide](docs/USER_GUIDE.md)
- [📘 API Documentation](https://docs.rs/oxcache)
- [💻 Examples](../examples/)

## 🤝 Contributing

Pull Requests and Issues are welcome!

## 📝 Changelog

See [CHANGELOG.md](../CHANGELOG.md)

## 📄 License

This project is licensed under MIT License. See [LICENSE](../LICENSE) file.

---

<div align="center">

**If this project helps you, please give a ⭐ Star to show support!**

Made with ❤️ by oxcache Team

</div>
