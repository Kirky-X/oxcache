use sea_orm::{ConnectOptions, ConnectionTrait, Database};

#[tokio::test]
async fn test_sqlite_connection_absolute_path() {
    let db_name = "test_absolute.db";
    let current_dir = std::env::current_dir().unwrap();
    let db_path = current_dir.join(db_name);

    // Remove existing file
    let _ = std::fs::remove_file(&db_path);

    println!("Testing with absolute path: {:?}", db_path);

    // Test different connection string formats with absolute path
    let connection_strings = [
        format!("sqlite:{}", db_path.display()),
        format!("sqlite://{}", db_path.display()),
        format!("sqlite:///{}", db_path.display()),
    ];

    for (i, conn_str) in connection_strings.iter().enumerate() {
        println!("Testing connection string {}: {}", i + 1, conn_str);

        let mut opt = ConnectOptions::new(conn_str.clone());
        opt.max_connections(1)
            .min_connections(1)
            .connect_timeout(std::time::Duration::from_secs(5));

        match Database::connect(opt).await {
            Ok(db) => {
                println!("✓ Connection {} succeeded!", i + 1);
                // Test a simple query
                let result = db
                    .execute(sea_orm::Statement::from_string(
                        sea_orm::DatabaseBackend::Sqlite,
                        "SELECT 1 as test".to_string(),
                    ))
                    .await;

                match result {
                    Ok(_) => println!("✓ Query test succeeded for connection {}", i + 1),
                    Err(e) => println!("✗ Query test failed for connection {}: {}", i + 1, e),
                }
            }
            Err(e) => println!("✗ Connection {} failed: {}", i + 1, e),
        }

        // Clean up
        let _ = std::fs::remove_file(&db_path);
    }
}

#[tokio::test]
async fn test_sqlite_connection_relative_path() {
    let db_name = "test_relative.db";

    // Remove existing file
    let _ = std::fs::remove_file(db_name);

    println!("Testing with relative path: {}", db_name);

    // Test different connection string formats with relative path
    let connection_strings = [
        format!("sqlite:{}", db_name),
        format!("sqlite:./{}", db_name),
    ];

    for (i, conn_str) in connection_strings.iter().enumerate() {
        println!("Testing connection string {}: {}", i + 1, conn_str);

        let mut opt = ConnectOptions::new(conn_str.clone());
        opt.max_connections(1)
            .min_connections(1)
            .connect_timeout(std::time::Duration::from_secs(5));

        match Database::connect(opt).await {
            Ok(db) => {
                println!("✓ Connection {} succeeded!", i + 1);
                // Test a simple query
                let result = db
                    .execute(sea_orm::Statement::from_string(
                        sea_orm::DatabaseBackend::Sqlite,
                        "SELECT 1 as test".to_string(),
                    ))
                    .await;

                match result {
                    Ok(_) => println!("✓ Query test succeeded for connection {}", i + 1),
                    Err(e) => println!("✗ Query test failed for connection {}: {}", i + 1, e),
                }
            }
            Err(e) => println!("✗ Connection {} failed: {}", i + 1, e),
        }

        // Clean up
        let _ = std::fs::remove_file(db_name);
    }
}
