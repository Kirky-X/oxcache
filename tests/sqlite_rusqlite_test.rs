use rusqlite::{Connection, Result};
use std::fs;

#[test]
fn test_rusqlite_connection() -> Result<()> {
    let db_name = "test_rusqlite.db";

    // Remove existing file
    let _ = fs::remove_file(db_name);

    println!("Testing rusqlite connection with: {}", db_name);

    // Create connection
    let conn = Connection::open(db_name)?;

    // Create a test table
    conn.execute(
        "CREATE TABLE test (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        )",
        [],
    )?;

    // Insert test data
    conn.execute("INSERT INTO test (name) VALUES (?1)", ["test_name"])?;

    // Query test data
    {
        let mut stmt = conn.prepare("SELECT id, name FROM test")?;
        let test_iter = stmt.query_map([], |row| {
            Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
        })?;

        for test in test_iter {
            let (id, name) = test?;
            println!("Found test: id={}, name={}", id, name);
        }
    }

    // Clean up
    drop(conn);
    let _ = fs::remove_file(db_name);

    println!("✓ rusqlite test completed successfully");
    Ok(())
}

#[test]
fn test_rusqlite_with_path() -> Result<()> {
    let current_dir = std::env::current_dir().unwrap();
    let db_path = current_dir.join("test_rusqlite_path.db");

    // Remove existing file
    let _ = fs::remove_file(&db_path);

    println!(
        "Testing rusqlite connection with absolute path: {:?}",
        db_path
    );

    // Create connection
    let conn = Connection::open(&db_path)?;

    // Create a test table
    conn.execute(
        "CREATE TABLE test (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        )",
        [],
    )?;

    println!("✓ rusqlite with absolute path test completed successfully");

    // Clean up
    drop(conn);
    let _ = fs::remove_file(&db_path);

    Ok(())
}
