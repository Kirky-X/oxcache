#[path = "config/config_test.rs"]
mod config_test;

#[cfg(feature = "confers")]
#[path = "config/config_coverage_test.rs"]
mod config_coverage_test;

#[cfg(feature = "confers")]
#[path = "config/confers_config_test.rs"]
mod confers_config_test;
