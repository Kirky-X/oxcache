# Project Context

## Purpose

**Oxcache** is a high-performance, production-grade two-level caching library for Rust. It provides a seamless L1 (Moka in-memory cache) + L2 (Redis distributed cache) architecture that enables developers to add caching to their applications with minimal code changes through a `#[cached]` attribute macro.

**Key Goals:**
- **Extreme Performance**: L1 nanosecond response (P99 < 100ns), L2 millisecond response (P99 < 5ms)
- **Zero-Code Changes**: Enable caching with a single `#[cached]` macro annotation
- **Automatic Failover**: Graceful degradation on Redis failure with WAL-based recovery
- **Multi-Instance Consistency**: Pub/Sub + version-based cache invalidation synchronization
- **Batch Optimization**: Intelligent batch writes for significantly improved throughput
- **Production Ready**: Complete observability, health checks, and chaos testing verified

## Tech Stack

- **Language**: Rust 1.75+
- **Runtime**: tokio 1.42 (async/await support)
- **L1 Cache**: Moka 0.12 (LRU/TinyLFU eviction)
- **L2 Cache**: Redis 0.27 (Standalone/Sentinel/Cluster modes)
- **Serialization**: serde 1.0, serde_json, flate2 (compression)
- **Database**: sea-orm 1.0 (PostgreSQL, MySQL, SQLite support)
- **Observability**: opentelemetry 0.22, tracing 0.1
- **Process Macros**: async-trait 0.1
- **Concurrency**: dashmap 6.0 (concurrent hash map)
- **CLI**: clap 4.4

### Dev Dependencies

- `tempfile`, `serial_test`, `rand`, `criterion`, `ctor`, `mockall`

## Project Conventions

### Code Style

- **Formatting**: Run `cargo fmt` before committing
- **Linting**: Follow `cargo clippy` suggestions (avoiding warnings)
- **Documentation**: All public APIs must have doc comments with examples
- **Error Handling**: Use `thiserror` for error type definitions
- **Logging**: Use `tracing` for structured logging throughout

### Architecture Patterns

- **Two-Level Caching**: L1 (in-process memory) + L2 (distributed Redis)
- **Service Registry**: `CacheManager` using `DashMap` for concurrent client storage
- **Write-Through/Write-Behind**: Configurable write strategies
- **Write-Ahead Log (WAL)**: Persistence for L2 cache recovery
- **Single-Flight**: Cache stampede prevention using locks
- **Batch Operations**: Accumulate writes for efficiency
- **Async Trait Pattern**: All cache operations use `#[async_trait]`

### Module Structure

```
src/
├── backend/          # L1/L2 cache backend implementations
├── client/           # Cache client wrappers (L1, L2, TwoLevel)
├── database/         # Database integration (MySQL, PostgreSQL, SQLite)
├── recovery/         # Health checks and WAL recovery
├── serialization/    # JSON/Bincode serialization
├── sync/             # Batch writer, invalidation, warmup
└── cli/              # CLI tools for management
```

### Testing Strategy

- **Unit Tests**: Inline in source files (`#[cfg(test)]` modules)
- **Integration Tests**: `tests/integration/` directory
- **Chaos Testing**: `tests/integration/chaos_test.rs` for failure scenarios
- **Concurrency Tests**: Use `serial_test` for test isolation
- **Real Redis Tests**: Scripts in `scripts/real_redis_test.sh`

### Git Workflow

- **Branch Naming**: Feature branches `feature/xxx`, bug fixes `fix/xxx`
- **Commit Messages**: Use Chinese, follow Conventional Commits format:
  - `<type>(<scope>): <subject>`
  - Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`
- **PRs**: Require CI passing before merge

## Domain Context

### Cache Concepts

- **L1 Cache**: In-process memory cache using Moka with LRU/TinyLFU eviction
- **L2 Cache**: Distributed Redis cache supporting multiple connection modes
- **TTL (Time To Live)**: Cache expiration time in seconds
- **TTI (Time To Idle)**: Time after last access before eviction
- **Promotion**: Automatic upgrade of L2 hits to L1 cache
- **Batch Writing**: Accumulating writes for improved throughput
- **WAL (Write-Ahead Log)**: Persistence for recovery

### Supported Configurations

- **Cache Types**: `two-level`, `l1-only`, `l2-only`
- **Redis Modes**: `standalone`, `sentinel`, `cluster`
- **Serialization**: `json` (default), optional `bincode`, compression with `flate2`

## Important Constraints

- **Rust Version**: 1.75+ required
- ** tokio Runtime**: Must be initialized before using cache
- **Shutdown Gracefully**: Call `shutdown_all()` for proper cleanup
- **L1 TTL ≤ L2 TTL**: L1 TTL must be less than or equal to L2 TTL
- **Send + Sync**: All cache clients must be `Send + Sync`

## External Dependencies

- **Redis Server**: For L2 cache (version 6.0+ recommended)
- **Database Servers**: Optional - PostgreSQL, MySQL, or SQLite for database integration features
- **OTLP Endpoint**: Optional - for OpenTelemetry tracing export

## Configuration

Uses TOML configuration files with hierarchical structure:

```toml
[global]
default_ttl = 3600
serialization = "json"

[services.<service_name>]
cache_type = "two-level"

  [services.<service_name>.l1]
  max_capacity = 10000

  [services.<service_name>.l2]
  mode = "standalone"
  connection_string = "redis://127.0.0.1:6379"
```

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo test --test '*'    # Run integration tests
cargo bench              # Run benchmarks
cargo clippy             # Lint
cargo doc --no-deps      # Generate documentation
```