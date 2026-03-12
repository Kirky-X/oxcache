//! Docker test utilities using testcontainers
//!
//! This module provides helper functions for setting up Docker-based test environments
//! using testcontainers for Redis and PostgreSQL.

use testcontainers::core::WaitFor;
use testcontainers::images::redis::Redis;
use testcontainers::images::postgres::Postgres;
use testcontainers::Container;

/// Creates a Redis testcontainer for testing
///
/// # Example
/// ```ignore
/// #[tokio::test]
/// async fn test_with_redis() {
///     let (_container, redis_url) = setup_redis_container().await;
///     // use redis_url to connect
/// }
/// ```
pub async fn setup_redis_container() -> (Container<'static, Redis>, String) {
    let redis = testcontainers::run("redis:7-alpine")
        .expect("Failed to start Redis container");

    let host_port = redis.get_host_port_ipv4(6379);
    let redis_url = format!("redis://127.0.0.1:{}", host_port);

    (redis, redis_url)
}

/// Creates a PostgreSQL testcontainer for testing
///
/// # Example
/// ```ignore
/// #[tokio::test]
/// async fn test_with_postgres() {
///     let (_container, connection_string) = setup_postgres_container().await;
///     // use connection_string to connect
/// }
/// ```
pub async fn setup_postgres_container() -> (Container<'static, Postgres>, String) {
    let pg = testcontainers::run("postgres:15-alpine")
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "test")
        .expect("Failed to start PostgreSQL container");

    let host_port = pg.get_host_port_ipv4(5432);
    let connection_string = format!(
        "postgres://test:test@127.0.0.1:{}/test",
        host_port
    );

    (pg, connection_string)
}

/// Creates both Redis and PostgreSQL testcontainers
pub async fn setup_redis_and_postgres() -> (
    Container<'static, Redis>,
    String,
    Container<'static, Postgres>,
    String,
) {
    let (redis_container, redis_url) = setup_redis_container().await;
    let (pg_container, pg_url) = setup_postgres_container().await;

    (redis_container, redis_url, pg_container, pg_url)
}
