# Oxcache Test Suite Structure

This document describes the organization of the test suite for the Oxcache project.

## Directory Structure

```
tests/
├── README.md                          # This file
├── lib.rs                            # Test library configuration
├── integration.rs                    # Integration tests entry point
├── e2e.rs                            # End-to-end tests entry point
├── unit.rs                           # Unit tests entry point
├── test_utils.rs                     # Common test utilities
├── database_test_utils.rs            # Database testing utilities
├── redis_test_utils.rs               # Redis testing utilities
│
├── integration/                      # Integration tests
│   ├── batch_write_test.rs           # Batch write operations
│   ├── chaos_test.rs                 # Chaos engineering tests
│   ├── cli_test.rs                   # CLI tests
│   ├── comprehensive_test.rs         # Comprehensive feature tests
│   ├── degradation_tests.rs          # Degradation handling tests
│   ├── http_cache_test.rs            # HTTP caching tests
│   ├── invalidation_test.rs          # Cache invalidation tests
│   ├── lifecycle_test.rs             # Lifecycle management tests
│   ├── lock_warmup_test.rs           # Lock warmup tests
│   ├── manual_control_test.rs        # Manual cache control tests
│   ├── metrics_test.rs               # Metrics collection tests
│   ├── partitioning_tests.rs         # Database partitioning tests
│   ├── performance_test.rs           # Performance benchmarks
│   ├── recovery_test.rs              # Recovery/WAL tests
│   ├── redis_native_test.rs          # Native Redis operations
│   ├── redis_test.rs                 # Redis backend tests
│   ├── sea_orm_sqlite_tests.rs       # Sea-ORM integration tests
│   ├── security_test.rs              # Security feature tests
│   ├── single_flight_test.rs         # Single-flight pattern tests
│   ├── sqlite_partition_tests.rs     # SQLite partitioning tests
│   ├── ttl_control_test.rs           # TTL control tests
│   ├── two_level_test.rs             # Two-level cache tests
│   ├── version_test.rs               # Version compatibility tests
│   └── wal_test.rs                   # Write-ahead log tests
│
├── e2e/                              # End-to-end tests
│   └── macro_test.rs                 # Macro-based caching tests
│
├── unit/                             # Unit tests
│   └── serialization_test.rs         # Serialization tests
│
├── common/                           # Shared test utilities
│   ├── mod.rs                        # Common module exports
│   ├── database_test_utils.rs        # Database utilities
│   └── redis_test_utils.rs           # Redis utilities
│
├── config/                           # Configuration tests
│   ├── config_test.rs                # Configuration API tests
│   └── config_coverage_test.rs       # Configuration coverage tests
│
├── feature/                          # Feature flag tests
│   ├── feature_gated_init_test.rs    # Feature-gated initialization
│   ├── features_coverage_test.rs     # Feature coverage tests
│   └── telemetry_test.rs             # Telemetry tests
│
├── backend/                          # Backend-specific tests
│   ├── l2_backend_test.rs            # L2 backend (Redis) tests
│   ├── mock_l2_backend_test.rs       # Mock backend tests
│   └── redis_version_compatibility_test.rs  # Redis version compatibility
│
├── cache/                            # Cache layer tests
│   └── layer_test.rs                 # Layer (L1/L2) cache tests
│
├── database/                         # Database integration tests
│   ├── cross_database_integration_tests.rs  # Cross-database tests
│   ├── database_partitioning_tests.rs       # Partitioning tests
│   ├── debug_mysql_test.rs           # MySQL debugging tests
│   └── sqlite_connection_test.rs     # SQLite connection tests
│
├── security/                         # Security tests
│   ├── security_coverage_test.rs     # Security coverage tests
│   └── security_tests.rs             # Security validation tests
│
└── performance/                      # Performance & memory tests
    ├── memory_leak_test.rs           # Memory leak detection tests
    ├── memory_tests.rs               # Memory usage tests
    └── miri_memory_test.rs           # Miri memory safety tests
```

## Test Categories

### Integration Tests (`integration/`)
Tests that verify the interaction between different components of the cache system. These tests typically:
- Require external services (Redis, databases)
- Test multi-component workflows
- Verify system-level behavior

Run with: `cargo test --test integration`

### End-to-End Tests (`e2e/`)
Tests that verify complete user workflows from start to finish. These tests:
- Test the full request/response cycle
- Verify macro-based caching functionality
- Simulate real-world usage patterns

Run with: `cargo test --test e2e`

### Unit Tests (`unit/`)
Tests that verify individual component functionality in isolation. These tests:
- Test single functions or methods
- Mock external dependencies
- Verify internal logic correctness

Run with: `cargo test --test unit`

### Configuration Tests (`config/`)
Tests for the configuration system:
- Configuration parsing and validation
- Cache type configuration
- Backend configuration

### Feature Tests (`feature/`)
Tests for feature flags and conditional compilation:
- Feature detection
- Feature-gated initialization
- Telemetry functionality

### Backend Tests (`backend/`)
Tests for cache backend implementations:
- Redis backend operations
- Mock backend behavior
- Version compatibility

### Cache Layer Tests (`cache/`)
Tests for cache layer functionality:
- L1 (memory) cache operations
- L2 (Redis) cache operations
- Layer promotion/demotion

### Database Tests (`database/`)
Tests for database integration:
- Partition management
- Cross-database compatibility
- Connection handling

### Security Tests (`security/`)
Tests for security features:
- Input validation
- Lua script validation
- Scan pattern validation

### Performance Tests (`performance/`)
Tests for performance and memory safety:
- Memory leak detection
- Memory usage patterns
- Miri memory safety verification

## Running Tests

### All Tests
```bash
cargo test --all-features --workspace
```

### Specific Test Category
```bash
cargo test --test integration      # Integration tests
cargo test --test e2e              # E2E tests
cargo test --test unit             # Unit tests
```

### Skip Network Tests
```bash
cargo test --all-features -- --skip redis
```

### With Database Tests
```bash
OXCACHE_TEST_DATABASE=1 cargo test
```

### With Redis Tests
```bash
cargo test --features redis
```

## Test Utilities

### `test_utils.rs`
Common utilities for all tests:
- Logging setup
- Cache initialization
- Redis availability checks
- Service cleanup

### `database_test_utils.rs`
Database-specific utilities:
- Test configuration management
- Partition helper functions
- Connection testing

### `redis_test_utils.rs`
Redis-specific utilities:
- Connection testing
- Cluster setup helpers
- Sentinel configuration

## Best Practices

1. **Naming**: Test files should end with `_test.rs`
2. **Organization**: Group related tests by feature or component
3. **Utilities**: Share common code through `test_utils.rs` or `common/`
4. **Isolation**: Tests should be independent and not depend on execution order
5. **Cleanup**: Tests should clean up resources after completion
6. **Skipping**: Use appropriate skip conditions for tests requiring external services
