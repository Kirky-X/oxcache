//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! SeaORM SQLite测试

use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use std::fs::File;

fn create_test_connect_options(db_path: &str) -> ConnectOptions {
    let mut opt = ConnectOptions::new(db_path.to_string());
    opt.max_connections(1)
        .min_connections(0)
        .connect_timeout(std::time::Duration::from_secs(10))
        .sqlx_logging(true);
    opt
}

async fn test_basic_connection(db_path: &str) -> bool {
    let opt = create_test_connect_options(db_path);

    match Database::connect(opt).await {
        Ok(db) => {
            println!("✓ Connection succeeded: {}", db_path);

            let result = db
                .execute(sea_orm::Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "SELECT 1 as test".to_string(),
                ))
                .await;

            match result {
                Ok(_) => {
                    println!("✓ Query test succeeded");
                    true
                }
                Err(e) => {
                    println!("✗ Query test failed: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            println!("✗ Connection failed: {} - {}", db_path, e);
            false
        }
    }
}

mod minimal_config_tests {
    use super::*;

    #[tokio::test]
    async fn test_sea_orm_sqlite_minimal() {
        let db_name = "test_sea_orm_minimal.db";
        let _ = std::fs::remove_file(db_name);

        println!(
            "Testing sea-orm SQLite with minimal configuration: {}",
            db_name
        );
        test_basic_connection(&format!("sqlite:{}", db_name)).await;

        let _ = std::fs::remove_file(db_name);
    }

    #[tokio::test]
    async fn test_sea_orm_sqlite_with_logging() {
        let db_name = "test_sea_orm_logging.db";
        let _ = std::fs::remove_file(db_name);

        println!("Testing sea-orm SQLite with detailed logging: {}", db_name);
        test_basic_connection(&format!("sqlite:{}", db_name)).await;

        let _ = std::fs::remove_file(db_name);
    }
}

mod file_creation_tests {
    use super::*;

    #[tokio::test]
    async fn test_sea_orm_sqlite_create_file_first() {
        let db_path = "/tmp/test_sea_orm_created.db";
        let _ = std::fs::remove_file(db_path);

        match File::create(db_path) {
            Ok(_) => println!("✓ Database file created successfully: {}", db_path),
            Err(e) => println!("✗ Failed to create database file: {}", e),
        }

        println!("Testing sea-orm SQLite with pre-created file: {}", db_path);
        test_basic_connection(&format!("sqlite:{}", db_path)).await;

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn test_sea_orm_sqlite_memory() {
        println!("Testing sea-orm SQLite with in-memory database");
        test_basic_connection("sqlite::memory:").await;
    }
}

mod path_tests {
    use super::*;

    #[tokio::test]
    async fn test_sea_orm_sqlite_absolute_path() {
        let db_path = "/home/project/aybss/crates/infra/oxcache/test_sea_orm_absolute.db";
        let _ = std::fs::remove_file(db_path);

        println!("Testing sea-orm SQLite with absolute path: {}", db_path);
        test_basic_connection(&format!("sqlite:{}", db_path)).await;

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn test_sea_orm_sqlite_with_uri_format() {
        let db_path = "/home/project/aybss/crates/infra/oxcache/test_sea_orm_uri.db";
        let _ = std::fs::remove_file(db_path);

        println!("Testing sea-orm SQLite with URI format: {}", db_path);

        let connection_string = format!("sqlite://{}", db_path);
        println!("Connection string: {}", connection_string);

        test_basic_connection(&connection_string).await;

        let _ = std::fs::remove_file(db_path);
    }
}
