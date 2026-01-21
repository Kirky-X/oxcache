#[path = "integration/batch_write_test.rs"]
mod batch_write_test;
#[path = "integration/chaos_test.rs"]
mod chaos_test;
#[path = "integration/comprehensive_test.rs"]
mod comprehensive_test;
#[path = "integration/degradation_tests.rs"]
mod degradation_tests;
#[path = "integration/invalidation_test.rs"]
mod invalidation_test;
#[path = "integration/lifecycle_test.rs"]
mod lifecycle_test;
#[path = "integration/lock_warmup_test.rs"]
mod lock_warmup_test;
#[path = "integration/manual_control_test.rs"]
mod manual_control_test;
#[path = "integration/metrics_test.rs"]
mod metrics_test;
#[path = "integration/partitioning_tests.rs"]
mod partitioning_tests;
#[path = "integration/performance_test.rs"]
mod performance_test;
#[path = "integration/recovery_test.rs"]
mod recovery_test;
#[path = "integration/redis_native_test.rs"]
mod redis_native_test;
#[path = "integration/redis_test.rs"]
mod redis_test;
#[path = "integration/sea_orm_sqlite_tests.rs"]
mod sea_orm_sqlite_tests;
#[path = "integration/security_test.rs"]
mod security_test;
#[path = "integration/single_flight_test.rs"]
mod single_flight_test;
#[path = "integration/sqlite_partition_tests.rs"]
mod sqlite_partition_tests;
#[path = "integration/tiered_cache_test.rs"]
mod tiered_cache_test;
#[path = "integration/ttl_control_test.rs"]
mod ttl_control_test;
#[path = "integration/two_level_test.rs"]
mod two_level_test;
#[path = "integration/version_test.rs"]
mod version_test;
#[path = "integration/wal_test.rs"]
mod wal_test;

// Common modules shared by integration tests
#[path = "common/mod.rs"]
mod common;
#[path = "database_test_utils.rs"]
mod database_test_utils;
