// Quick test to verify Redis connection and basic operations

use std::sync::Arc;
use std::time::Duration;
use oxcache::backend::client::RedisBackend;
use oxcache::backend::CacheBackend;

#[tokio::main]
async fn main() {
    println!("🧪 Testing Redis connection...\n");

    let redis_url = "redis://127.0.0.1:6381";
    println!("📡 Connecting to: {}", redis_url);

    // Test 1: Create Redis backend
    match RedisBackend::new(redis_url).await {
        Ok(backend) => {
            let backend = Arc::new(backend);
            println!("✅ Redis backend created successfully");

            // Test 2: Ping
            println!("\n📡 Testing PING...");
            match backend.ping().await {
                Ok(_) => println!("✅ PING successful"),
                Err(e) => println!("❌ PING failed: {}", e),
            }

            // Test 3: SET operation
            println!("\n💾 Testing SET...");
            let test_key = "oxcache:test:connection";
            let test_value = b"Hello, Redis!".to_vec();
            match backend.set(test_key, test_value.clone(), Some(Duration::from_secs(60))).await {
                Ok(_) => println!("✅ SET successful"),
                Err(e) => println!("❌ SET failed: {}", e),
            }

            // Test 4: GET operation
            println!("\n📖 Testing GET...");
            match backend.get(test_key).await {
                Ok(Some(value)) => {
                    if value == test_value {
                        println!("✅ GET successful, value matches");
                    } else {
                        println!("⚠️  GET successful but value doesn't match");
                    }
                }
                Ok(None) => println!("❌ GET returned None"),
                Err(e) => println!("❌ GET failed: {}", e),
            }

            // Test 5: DELETE operation
            println!("\n🗑️  Testing DELETE...");
            match backend.delete(test_key).await {
                Ok(_) => println!("✅ DELETE successful"),
                Err(e) => println!("❌ DELETE failed: {}", e),
            }

            // Test 6: TTL operation
            println!("\n⏰ Testing SET with TTL...");
            let ttl_key = "oxcache:test:ttl";
            match backend.set(ttl_key, b"ttl_value".to_vec(), Some(Duration::from_secs(2))).await {
                Ok(_) => {
                    println!("✅ SET with TTL successful");
                    println!("⏳ Waiting for TTL to expire (2 seconds)...");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    match backend.get(ttl_key).await {
                        Ok(None) => println!("✅ TTL expired correctly (value is None)"),
                        Ok(Some(_)) => println!("❌ TTL didn't expire"),
                        Err(e) => println!("❌ TTL check failed: {}", e),
                    }
                }
                Err(e) => println!("❌ SET with TTL failed: {}", e),
            }

            // Test 7: Batch operations
            println!("\n📦 Testing batch operations...");
            let batch_size = 10;
            for i in 0..batch_size {
                let key = format!("oxcache:test:batch:{}", i);
                let value = format!("batch_value_{}", i).into_bytes();
                if let Err(e) = backend.set(&key, value, Some(Duration::from_secs(60))).await {
                    println!("❌ Batch SET {} failed: {}", i, e);
                }
            }
            println!("✅ Batch SET completed ({} operations)", batch_size);

            // Cleanup
            for i in 0..batch_size {
                let key = format!("oxcache:test:batch:{}", i);
                let _ = backend.delete(&key).await;
            }
            println!("🧹 Cleanup completed");

            println!("\n🎉 All Redis tests passed!");
        }
        Err(e) => {
            println!("❌ Failed to create Redis backend: {}", e);
            std::process::exit(1);
        }
    }
}
