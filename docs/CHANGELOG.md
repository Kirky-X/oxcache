# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.4] - 2026-01-24

### Refactor

- **Examples**: Adapted all examples to the new Cache API (`set()` / `set_with_ttl()` separation)
- **Examples**: Updated type signatures for `get()` and `delete()` to use `&String`
- **Examples**: Added `Serialize/Deserialize` derive for cacheable structs
- **Examples**: Simplified MySQL example, focused on connection demonstration
- **Examples**: Removed obsolete TTL expiration tests (API changed)
- **Examples**: Unified `tokio::time::sleep` and `std::time::Duration` usage
- **Examples**: Fixed trailing newline issues

### Testing

- **All tests passing**: 163 tests (17 unit + 141 integration + 5 doc)
- **CI checks passing**: Format, Clippy, Deny (licenses + sources)

## [Unreleased]

### Security

- **Critical**: Fixed Lua script validation bypass vulnerability by adding detection for:
  - Dynamic command execution (e.g., `redis.call(CMD)`, `redis.call(VAR)`)
  - String concatenation for command execution (e.g., `redis.call('FLU' .. 'SHALL')`)
  - Nested `redis.eval`/`redis.evalsha` calls
  - Potential infinite loop patterns (`WHILE TRUE`, `REPEAT`, `GOTO`)
  - System command execution (`os.execute`, `io.popen`)
  - Dynamic code loading (`loadstring`, `dofile`)
- **High**: Enhanced configuration file loading security by adding:
  - Path traversal detection (prevents `../` and `~` attacks)
  - File extension validation (only allows `.toml` files)
  - File permission checks (Unix: requires 0600 or stricter)
  - File existence validation
- **High**: Replaced empty feature-gated implementations with compile-time panic errors for better error messages:
  - `BatchWriter` (requires `batch-write` feature)
  - `CacheInvalidator` (requires `batch-write` feature)
  - `CachePromoter` (requires `batch-write` feature)
- **Medium**: Enhanced Redis key validation by adding:
  - Unicode control character detection
  - SQL injection pattern detection (e.g., `' OR '1'='1'`, `'; DROP TABLE`)
  - Path traversal pattern detection (e.g., `../`, URL-encoded forms)
  - Command injection character detection (e.g., `;`, `|`, `&`, `` ` ``)
- **Medium**: Enhanced production environment validation by adding:
  - More precise private network detection (172.16.0.0/12, .local, .dev, .test, etc.)
  - Password complexity requirements (16+ chars, uppercase, lowercase, digit, special char)
  - Common weak password blacklist
  - TLS certificate verification checks
- **Low**: Unified `security` module visibility (now always public, not feature-gated)
- **Low**: Enhanced deprecation warnings for `manager` module (will be removed in v0.3.0)

### Testing

- **Added**: Comprehensive feature combination testing matrix covering minimal, core, and full feature sets
- **Added**: Automated feature combination validation script (`scripts/validate-feature-combinations.sh`)
- **Added**: CI jobs for testing individual feature sets (minimal, core, full)
- **Added**: CI matrix for testing critical feature combinations
- **Fixed**: Removed `skip_broken` feature flag from database partitioning tests
- **Fixed**: Database tests now use environment variable `OXCACHE_TEST_DATABASE` for conditional execution
- **Improved**: CI cache strategy for better build performance
- **Added**: Feature analysis documentation (`FEATURE_ANALYSIS.md`)
- **Added**: Feature test matrix documentation (`FEATURE_TEST_MATRIX.md`)

### Documentation

- Improved configuration examples with L1-only and L2-only cache types
- Added comprehensive feature dependency documentation
- Enhanced performance data with test environment details
- Fixed broken documentation links and version inconsistencies
- Added documentation validation script and maintenance guidelines
- **Fixed version inconsistencies**: Updated all documentation from 0.1.2 to 0.1.3
- **Corrected API usage**: Updated examples to use `init_from_file()` instead of `init()` for config file initialization
- **Added feature requirements**: Clarified that `config-toml` and `confers` features are required for file-based initialization
- **Fixed macro parameter documentation**: Removed non-existent `strategy` parameter, added `key_prefix` and `key_generator` parameters
- **Fixed L1Config documentation**: Updated structure fields to match actual implementation
- **Added deprecation warnings**: Noted `init_from_file` is deprecated in user guide

### Features

- **Added**: L1 TTL support with query and refresh capabilities
- **Added**: Complete Redis Cluster strategy with automatic slot allocation
- **Added**: Complete Redis Sentinel strategy with automatic failover
- **Implemented**: L1 backend TTL query and refresh methods
- **Implemented**: Redis Cluster connection management with topology awareness
- **Implemented**: Redis Sentinel connection management with master discovery

### Bug Fixes

- **Fixed**: Intermittent failures in tiered cache tests by adding `#[serial]` attributes
- **Fixed**: Type inference issues in `#[cached]` macro for `Result<T, E>` return types
- **Fixed**: Clippy warnings including unused mut variables, unnecessary map_or, and deprecated API usage
- **Fixed**: Code formatting issues with `cargo fmt`

### Testing

- **Added**: CLI tools integration tests (8 tests)
- **Added**: HTTP cache tests (5 tests)
- **Added**: Builder pattern unit tests (17 tests)
- **Added**: New API usage examples
- **Recovered**: E2E macro tests (3 tests)
- **Improved**: Test coverage from 85% to 91%
- **All tests passing**: 363 tests (194 lib + 149 integration + 17 unit + 3 e2e)

### Performance

- **Optimized**: Redis command chaining in Cluster and Sentinel strategies
- **Improved**: Batch write performance with optimized implementations

### Code Quality

- **Refactored**: Removed unused mut variables
- **Refactored**: Used `is_some_and` instead of `map_or` for better readability
- **Refactored**: Used `as_deref()` instead of `.as_ref().map(|s| s.as_str())`
- **Added**: `#[allow(deprecated)]` attributes for deprecated APIs
- **Added**: `#[allow(unexpected_cfgs)]` for test code with custom features
- **Removed**: Unused imports and variables

## [0.1.2] - 2026-01-02

### Security

- **Critical**: Fixed PostgreSQL SQL injection vulnerability by adding identifier validation
- Fixed sensitive information leakage in logs by replacing `println!` with `tracing` (24 locations)
- Fixed Redis command injection risk by validating TTL range (0-30 days)

### Performance

- Optimized L2 version cache by replacing `RwLock<HashMap>` with `DashMap` for lock-free concurrent access (30-50% performance improvement)
- Optimized bloom filter by using `Arc<Vec<u8>>` to reduce memory allocation (40-60% memory reduction)
- Optimized batch writer by using ownership transfer to reduce cloning operations (20-40% throughput improvement)
- Optimized metrics collection by replacing `Mutex<HashMap>` with `DashMap` (2-3x concurrent performance improvement)

### Code Quality

- Fixed excessive `unwrap()` usage by using `expect()` or `ok_or_else()` for proper error handling
- Fixed code formatting issues with `cargo fmt`
- Fixed Clippy warnings and added necessary `#[allow(clippy::type_complexity)]` attributes
- Added missing imports and fixed compilation errors

### CI/CD

- Fixed release workflow duplicate definition issue
- Simplified release workflow from 5 complex jobs to 1 linear job
- Removed automatic version update to avoid modifying code during tag push
- Improved changelog generation using temporary files

### Testing

- All tests passing (48/48)
- Pre-commit checks passing (7/7)
- Performance benchmarks validated optimization results

### Impact

- 98 files changed, 1815 insertions(+), 431 deletions(-)
- Fixed 3 high-priority security vulnerabilities
- Implemented 4 major performance optimizations

## [0.1.1] - 2025-12-31

### Performance

#### Compiler Optimization Enhancements

This release introduces comprehensive compiler optimizations to significantly improve runtime performance and reduce binary size.

##### Optimization Options Applied

| Option | Value | Description |
|--------|-------|-------------|
| `opt-level` | `3` | Maximum optimization level (O3) for aggressive performance tuning |
| `lto` | `fat` | Full Link-Time Optimization enabling cross-crate inlining and dead code elimination |
| `codegen-units` | `1` | Single codegen unit for maximum optimization opportunities |
| `strip` | `true` | Strip debug symbols from release binary to reduce size |
| `panic` | `abort` | Abort on panic instead of unwinding for smaller binary and faster panic handling |
| `overflow-checks` | `false` | Disable runtime overflow checks in release builds for performance |

##### Optimization Objectives

1. **Runtime Performance**: Achieve maximum throughput for high-concurrency cache operations
   - L1 cache (Moka) operations targeting sub-microsecond latency
   - L2 cache (Redis) operations optimized for network efficiency
   - Batch write operations minimized overhead

2. **Binary Size Reduction**: Minimize deployment footprint
   - Symbol stripping removes debug information
   - LTO eliminates unused code across crate boundaries
   - Abort panic strategy reduces runtime overhead

3. **Startup Time**: Improve application initialization performance
   - Single codegen unit enables better whole-program analysis
   - Aggressive inlining reduces function call overhead

##### Implementation Steps

1. Updated `[profile.release]` section in `Cargo.toml`:
   ```toml
   [profile.release]
   opt-level = 3          # O3 optimization
   lto = "fat"            # Full LTO for cross-crate optimization
   codegen-units = 1      # Single codegen unit for max optimization
   strip = true           # Strip debug symbols
   panic = "abort"        # Smaller binary, faster panic
   overflow-checks = false # Disable overflow checks in release
   ```

2. Added benchmark-specific profile for consistent benchmarking:
   ```toml
   [profile.bench]
   opt-level = 3
   lto = "fat"
   codegen-units = 1
   ```

3. Preserved development and test configurations for fast iteration:
   ```toml
   [profile.dev]
   debug = true
   
   [profile.test]
   opt-level = 0
   ```

##### Expected Effects

- **Cache Operations**: 10-30% improvement in L1 cache hit latency
- **Memory Usage**: 15-25% reduction in release binary size
- **Throughput**: Enhanced batch operation performance for high-throughput scenarios
- **Cold Start**: Faster application initialization due to optimized code generation

##### Compatibility Notes

- These optimizations are applied to release builds only
- Development builds retain full debug information for debugging
- Benchmark profiles ensure consistent measurement conditions

### Build System

- Updated version from 0.1.0 to 0.1.1
- Enhanced release profile with comprehensive optimization flags
- Configured benchmark profile for performance testing consistency

## [0.1.0] - 2025-12-23

### Added

- **Graceful Shutdown Mechanism**: Implemented comprehensive graceful shutdown functionality for distributed systems
    - Added `sync_tasks` field to Application for tracking synchronous tasks
    - Implemented `shutdown` method with cleanup of all active tasks
    - Integrated GracefulShutdownManager with Application shutdown logic
    - Simplified MCP adapter shutdown implementation
    - Added `shutdown_all()` function in cache manager for system-wide client shutdown
    - Created comprehensive test suite for shutdown functionality

- **Degradation Strategy System**: Implemented automatic fallback mechanism for L2 cache failures
    - Added `handle_l2_failure` method for L2 cache failure handling
    - Implemented health state monitoring (Healthy → Degraded → Recovering → Healthy)
    - Added failure counter and state transition logic
    - Integrated metrics monitoring indicators
    - Created complete test suite for degradation logic validation
    - Added health state transition tests and mock backend tests

- **Database Fallback Functionality**: Implemented automatic database source fallback
    - Added `DbLoader` trait and `DbFallbackManager` for modular database loading
    - Integrated database fallback mechanism in TwoLevelClient
    - Support for timeout control, retry strategy, and batch data loading
    - Fixed cargo clippy warnings for large enum variants and file opening behavior

- **HTTP Metrics Endpoints**: Implemented HTTP metrics and health endpoints
    - Added Axum-based `/metrics` and `/health` endpoints in manager.rs
    - Enhanced disk space monitoring with automatic cleanup logic
    - Fixed archive service database archiving and Parquet conversion logic
    - Optimized `get_encryption_key` for base64 exception handling

- **Redis Version Compatibility**: Added comprehensive Redis version compatibility testing
    - Support for Redis 6.x/7.x versions, cluster mode, and sentinel mode
    - Added compatibility tests for different Redis configurations
    - Fixed L2Backend method calls to use correct `set_bytes()`/`get_bytes()`

- **Configuration Management Enhancements**:
    - Implemented RUN_ENV based automatic config switching
    - Added priority-based config loading (base → env-specific)
    - Fixed compilation errors in generated code by fully qualifying Clap trait methods
    - Added `new_loader` method to Config derive macro for flexible configuration
    - Resolved feature-gating issues for ConfigWatcher

- **Audit Logging Improvements**:
    - Added config source metadata to AuditLogger
    - Implemented `log_to_file_with_source()` method for AuditLogger
    - Added config source field to track which configuration sources were used
    - Support for tracking defaults, explicit files, remote config, environment variables, and CLI arguments

- **TypeScript Schema Generation**: Completed TypeScript schema generation implementation
    - Fixed ConfigType enum empty interface issues through oneOf structure detection
    - Improved `generate_interface` method to support complex enum structure TypeScript mapping
    - Fixed multi-value enum handling logic in `get_typescript_type`
    - Implemented custom validate support through macro code generation

### Fixed

- **Memory Management**: Fixed memory leak issues and improved memory usage patterns
- **Error Handling**: Enhanced error propagation and custom error types using `thiserror`
- **Code Quality**: Resolved various compilation warnings and clippy lints
- **Test Stability**: Fixed test compilation errors and improved test reliability
- **Documentation**: Updated PRD documentation to mark implemented features as completed

### Security

- **Encryption**: Enhanced encryption key handling and fallback mechanisms
- **Input Validation**: Improved input validation for configuration and user data
- **Audit Trail**: Strengthened audit logging with comprehensive metadata tracking

### Performance

- **Caching**: Optimized cache performance with improved degradation strategies
- **Database**: Enhanced database connection pooling and query optimization
- **Monitoring**: Added comprehensive metrics and monitoring capabilities

### Testing

- **Test Coverage**: Achieved high test coverage including unit tests, integration tests, and property-based tests
- **Mock Testing**: Implemented comprehensive mock testing for Redis and database backends
- **Stress Testing**: Added UAT stress tests and comprehensive stress test suites
- **Compatibility Testing**: Implemented Redis version compatibility and database partitioning tests

[Unreleased]: https://github.com/Kirky-X/oxcache/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/Kirky-X/oxcache/releases/tag/v0.1.1
[0.1.0]: https://github.com/Kirky-X/oxcache/releases/tag/v0.1.0