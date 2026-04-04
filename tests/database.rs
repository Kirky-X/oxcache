#[path = "database/cross_database_integration_tests.rs"]
mod cross_database_integration_tests;

#[path = "database/database_partitioning_tests.rs"]
mod database_partitioning_tests;

#[path = "database/debug_mysql_test.rs"]
mod debug_mysql_test;

#[path = "database/partitioning_tests.rs"]
mod partitioning_tests;

#[path = "database/connection_string_test.rs"]
mod connection_string_test;

#[path = "common/database_test_utils.rs"]
pub mod database_test_utils;
