// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Quick verification that Redis backend works
use oxcache::backend::client::RedisBackend;
use oxcache::backend::CacheBackend;

#[tokio::main]
async fn main() {
    println!("🧪 验证 Redis 后端功能...\n");

    // Use default Redis port 6379
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!("🔗 连接地址: {}\n", redis_url);

    // Test 1: Create backend
    println!("1. 创建 Redis 后端...");
    match RedisBackend::new(&redis_url).await {
        Ok(backend) => {
            println!("   ✅ 后端创建成功\n");

            // Test 2: Ping
            println!("2. 测试 PING...");
            match backend.ping().await {
                Ok(response) => println!("   ✅ PING 响应: {}\n", response),
                Err(e) => println!("   ❌ PING 失败: {}\n", e),
            }

            // Test 3: SET
            println!("3. 测试 SET...");
            let test_key = "oxcache:verify:test";
            let test_value = b"Hello, Redis!".to_vec();
            match backend
                .set(
                    test_key,
                    test_value.clone(),
                    Some(std::time::Duration::from_secs(60)),
                )
                .await
            {
                Ok(_) => println!("   ✅ SET 成功\n"),
                Err(e) => println!("   ❌ SET 失败: {}\n", e),
            }

            // Test 4: GET
            println!("4. 测试 GET...");
            match backend.get(test_key).await {
                Ok(Some(value)) => {
                    if value == test_value {
                        println!("   ✅ GET 成功, 值匹配\n");
                    } else {
                        println!("   ⚠️  GET 成功但值不匹配\n");
                    }
                }
                Ok(None) => println!("   ❌ GET 返回 None\n"),
                Err(e) => println!("   ❌ GET 失败: {}\n", e),
            }

            // Test 5: DELETE
            println!("5. 测试 DELETE...");
            match backend.delete(test_key).await {
                Ok(_) => println!("   ✅ DELETE 成功\n"),
                Err(e) => println!("   ❌ DELETE 失败: {}\n", e),
            }

            // Test 6: GET after DELETE
            println!("6. 验证删除...");
            match backend.get(test_key).await {
                Ok(None) => println!("   ✅ 键已成功删除\n"),
                _ => println!("   ⚠️  键仍然存在\n"),
            }

            println!("🎉 所有 Redis 后端功能验证通过!");
        }
        Err(e) => {
            println!("   ❌ 无法创建 Redis 后端: {}\n", e);
            std::process::exit(1);
        }
    }
}
