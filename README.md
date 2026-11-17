English | [中文](README_CN.md)

<div align="center">

<img src="docs/assets/oxcache.png" alt="Oxcache Logo" width="240">

[![Rust 2024](https://img.shields.io/badge/Rust-2024-edded?logo=rust&style=for-the-badge)](https://www.rust-lang.org/) [![Rustc 1.97.1+](https://img.shields.io/badge/Rustc-1.97.1+-orange.svg?style=for-the-badge)](https://www.rust-lang.org/) [![crates.io](https://img.shields.io/crates/v/oxcache?style=for-the-badge)](https://crates.io/crates/oxcache) [![docs.rs](https://img.shields.io/docsrs/oxcache?style=for-the-badge)](https://docs.rs/oxcache) [![downloads](https://img.shields.io/crates/d/oxcache?style=for-the-badge)](https://crates.io/crates/oxcache) [![CI](https://img.shields.io/github/actions/workflow/status/Kirky-X/oxcache/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/Kirky-X/oxcache/actions/workflows/ci.yml) [![coverage](https://img.shields.io/codecov/c/github/Kirky-X/oxcache?style=for-the-badge)](https://codecov.io/gh/Kirky-X/oxcache) [![license](https://img.shields.io/crates/l/oxcache?style=for-the-badge)](LICENSE)

**Multi-tier caching for Rust: L1 in-memory + L2 distributed + optional L3 disk (redb)**

[✨ Features](#-features) • [🚀 Quick Start](#-quick-start) • [📚 Documentation](#-documentation) • [💻 Examples](#-examples) • [🤝 Contributing](#-contributing)

</div>

---

## ✨ Features

<table style="width:100%; border-collapse: collapse">
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🚀 <b>Multi-tier caching</b><br><span style="color:#64748B">L1 (Moka / DashMap) and L2 (Redis / Valkey / Dragonfly / Aerospike) chained by score via <code>ChainCache</code>, with async backfill on non-top hits</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">⚡ <b>Zero-boilerplate macro</b><br><span style="color:#64748B">One-line <code>#[cached]</code> integration supporting <code>service</code> / <code>ttl</code> / <code>key</code> / <code>key_prefix</code> / <code>scope</code> / <code>sync</code> / <code>single_flight</code> / <code>strict</code> / <code>condition</code> / <code>skip</code></span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🔄 <b>Sync API</b><br><span style="color:#64748B">With <code>sync_mode(true)</code>, <code>get_sync</code> / <code>set_sync</code> / <code>get_or_sync</code> coexist with the async API on the same <code>Cache&lt;K, V&gt;</code></span></td>
<td width="50%" style="vertical-align:top; padding: 12px">⏱️ <b>Universal per-entry TTL</b><br><span style="color:#64748B"><code>ttl</code> / <code>expire</code> behave consistently across all backend kinds: Moka / DashMap / Redis / Valkey (via RedisBackend) / Dragonfly / Aerospike / Disk / Mock / Chain / Bloom</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🌸 <b>Penetration guard</b><br><span style="color:#64748B">Single-flight dedup (64 shards), null sentinel, TTL jitter, bloom-filter negative-query short-circuit</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🔐 <b>Built-in security</b><br><span style="color:#64748B">Key / Lua / SCAN input validation, connection-string redaction, value-level encryption and integrity decorators</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">📈 <b>Observability</b><br><span style="color:#64748B">Latency histograms and operation counters, Prometheus / JSON export, <code>telemetry</code> tracing events, audit event stream</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧬 <b>Pluggable serialization</b><br><span style="color:#64748B">JSON by default, opt-in <code>serde-bincode</code> / <code>postcard</code> binary formats, depth limiting against nested-JSON DoS</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🗜️ <b>Adaptive compression</b><br><span style="color:#64748B"><code>CompressingBackend</code> applies zstd above a size threshold; reads auto-detect by magic bytes and stay compatible with legacy gzip</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🔑 <b>Distributed coordination</b><br><span style="color:#64748B">Redis distributed lock (watchdog renewal / reentrant), RedLock multi-node majority lock, cross-instance invalidation bus</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🧯 <b>Fault resilience</b><br><span style="color:#64748B">ChainCache per-link fault tolerance, <code>degradation</code> three-state auto-degradation and recovery, health checks, graceful shutdown</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧪 <b>Engineering quality</b><br><span style="color:#64748B">2000+ test functions (as of 0.5.1), chaos and security tests, three-platform CI matrix, coverage gate</span></td>
</tr>
</table>

> For the itemized advanced capabilities such as `lock` / `red-lock` / `invalidation` / `encrypt` / `integrity` / `versioning` / `config-confers` / `degradation` / `audit` / `stale` / `offload` / `disk` / `adaptive-ttl` / `warmup`, see the [API Reference](docs/en/API_REFERENCE.md).

---

## 🚀 Quick Start

### 📦 Installation

```bash
cargo add oxcache            # default minimal: L1 in-memory cache
cargo add oxcache --features full   # full: L1 + L2 + macro + compression + batch + Lua + lock
```

**Requirements**: Rust 1.97.1+ (edition 2024). The default `minimal` feature enables the L1 in-memory cache only; using L2 (Redis / Valkey / Dragonfly / Aerospike) requires a reachable server, and Redis-related tests depend on Docker (containers are started automatically via testcontainers).

### 💡 Minimal Runnable Example

```rust
use oxcache::Cache;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Default Moka L1 in-memory backend
    let cache: Cache<String, User> = Cache::builder().build().await?;

    // Write
    cache
        .set(&"user:1".to_string(), &User { id: 1, name: "Alice".into() })
        .await?;

    // Read
    if let Some(user) = cache.get(&"user:1".to_string()).await? {
        println!("命中: {}", user.name);
    }

    // Delete
    cache.delete(&"user:1".to_string()).await?;
    assert!(cache.get(&"user:1".to_string()).await?.is_none());
    Ok(())
}
```

- For the `#[cached]` macro and core concepts, see the [User Guide](docs/en/USER_GUIDE.md); for more installation options, see the [installation chapter of the User Guide](docs/en/USER_GUIDE.md#installation); for experimental wasm32 support, see the [wasm32 chapter of the User Guide](docs/en/USER_GUIDE.md#-wasm32-support-experimental); for the full example list, see the [examples overview of the User Guide](docs/en/USER_GUIDE.md#-examples-overview).

---

## 🎨 Feature Flags

Tier presets (`default = ["minimal"]`, L1 only):

```toml
oxcache = { version = "0.5.1", features = ["minimal"] }   # L1 only (default)
oxcache = { version = "0.5.1", features = ["redis-tier"] } # L1 + L2 Redis (legacy `core` remains as a compat alias)
oxcache = { version = "0.5.1", features = ["full"] }      # true full: every named capability feature
```

For the complete feature-flag table (itemized descriptions and defaults), see the [API Reference](docs/en/API_REFERENCE.md#-feature-flags).

---

## 📚 Documentation

| Document | Description |
|------|------|
| [📖 User Guide](docs/en/USER_GUIDE.md) | Complete tutorial from installation to advanced usage |
| [📘 API Reference](docs/en/API_REFERENCE.md) | Detailed description of all public APIs and the error-code table |
| [🏗️ Architecture](docs/en/ARCHITECTURE.md) | Design philosophy, module responsibilities, data flow and failure handling |
| [📊 Performance Baseline](docs/en/PERFORMANCE.md) | Serialization sizes, compression ratios, hot-path benchmarks and reproduction commands |
| [🧪 Test Scenario Matrix](docs/en/TEST_SCENARIOS.md) | Mapping from feature domains to test targets, with execution policy |
| [🤝 Contributing Guide](docs/en/CONTRIBUTING.md) | Development environment, TDD workflow and PR process |
| [📋 Changelog](docs/en/CHANGELOG.md) | Change log for every release |
| [📦 Online API Docs](https://docs.rs/oxcache) | Latest auto-generated documentation on docs.rs |
| [📦 crates.io](https://crates.io/crates/oxcache) | Published crate page |

---

## 💻 Examples

The `examples/` directory contains **37 examples** (all runnable with default features); for the grouped list and run commands, see the [examples overview of the User Guide](docs/en/USER_GUIDE.md#-examples-overview). For the full sync-method table and runtime requirements, see the [API Reference](docs/en/API_REFERENCE.md#-sync-api).

---

## 🏗️ Architecture

Oxcache follows a layered design of unified interface plus pluggable backends: applications face a single type-safe entry point, `Cache<K, V>`, and L1/L2 backends can be chained by score via `ChainCache` with on-demand backfill. For the layered architecture diagram, module responsibilities, data flow, the TTL behavior reference and the bloom-filter penetration guard, see the [Architecture documentation](docs/en/ARCHITECTURE.md).

---

## 🧪 Testing

2000+ test functions (as of 0.5.1). For the test layers, function counts and common commands, see the [Test Scenario Matrix](docs/en/TEST_SCENARIOS.md#7-test-layers-and-common-commands); Redis-related tests depend on a local Docker environment (containers are started automatically via testcontainers).

---

## 📊 Performance

The borrowed-key hot-path APIs (`get_by_str` / `set_by_str`) measure **-6.7%** for get and **-12.7%** for set; postcard's mixed-payload transfer size is about **46%** of JSON. For benchmarks and reproduction commands, see the [Performance Baseline](docs/en/PERFORMANCE.md).

---

## 🔒 Security

Oxcache ships multiple layers of defense inside the library; for the defense overview, see the [Security document](docs/en/SECURITY.md#-in-library-defense-overview). **Reporting vulnerabilities**: please do not report security vulnerabilities through public GitHub Issues; use the private [GitHub Security Advisories](https://github.com/Kirky-X/oxcache/security/policy) disclosure channel; the full policy lives in the [Security document](docs/en/SECURITY.md).

---

## 🗺️ Roadmap

The current roadmap lives in the [development roadmap of UNRELEASED.md](docs/en/UNRELEASED.md#development-roadmap).

---

## 🤝 Contributing

Pull Requests and Issues are welcome! For the development environment, the TDD workflow, Git hooks and the PR process, see the [Contributing Guide](docs/en/CONTRIBUTING.md).

---

## 📋 Changelog

See [CHANGELOG.md](docs/en/CHANGELOG.md) for the complete version history and [UNRELEASED.md](docs/en/UNRELEASED.md) for unreleased changes.

---

## 📄 License

This project is licensed under the [MIT](LICENSE) License.

Copyright (c) 2025-2026 Kirky.X🌠

---

## 🙏 Acknowledgments

Oxcache is built on top of many great open-source projects:

- [Moka](https://github.com/moka-rs/moka) and [DashMap](https://github.com/xacrimon/dashmap) — L1 in-memory cache backends
- [redis-rs](https://github.com/redis-rs/redis-rs) — Redis / Valkey / Dragonfly / Sentinel / Cluster client
- [aerospike-client-rust](https://github.com/aerospike/aerospike-client-rust) — Aerospike backend
- [Tokio](https://github.com/tokio-rs/tokio) and [Serde](https://github.com/serde-rs/serde) — async runtime and serialization ecosystem
- [trait-kit](https://github.com/Kirky-X/trait-kit) — AsyncKit integration (lifecycle / health check / shutdown coordination)
- [testcontainers-rs](https://github.com/testcontainers/testcontainers-rs) and [Criterion](https://github.com/bheisler/criterion.rs) — integration testing and benchmarking infrastructure

---

## 📞 Contact & Support

- **Issue tracker**: [GitHub Issues](https://github.com/Kirky-X/oxcache/issues) (templates available for bug reports / feature requests / questions)
- **Security vulnerabilities**: please do not report security vulnerabilities through public GitHub Issues; use the private GitHub Security Advisories disclosure channel
- **Maintainer**: Kirky.X

---

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/oxcache&type=Date)](https://star-history.com/#Kirky-X/oxcache&Date)

### 💝 Support This Project

If you find this project useful, please consider giving it a ⭐️!

**Made with love by Kirky.X**
