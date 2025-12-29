# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/your-org/oxcache/compare/v0.1.0...HEAD

[0.1.0]: https://github.com/your-org/oxcache/releases/tag/v0.1.0