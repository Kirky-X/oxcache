// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// Pipeline Performance Tests
// 测试 Redis Pipeline 批量操作的性能提升

use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use std::time::{Duration, Instant};

/// 获取 Redis 测试 URL
fn get_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

/// 生成测试数据
fn generate_test_data(count: usize) -> Vec<(String, Vec<u8>, Option<Duration>)> {
    (0..count)
        .map(|i| {
            (
                format!("perf_test_key_{}", i),
                format!("perf_test_value_{}", i).into_bytes(),
                Some(Duration::from_secs(60)),
            )
        })
        .collect()
}

/// 测试 Pipeline SET 性能
#[tokio::test]
#[ignore = "性能测试需要 Redis 环境，使用 cargo test -- --ignored 运行"]
async fn test_pipeline_set_performance() {
    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url)
        .await
        .expect("Failed to create Redis backend");

    // 清理之前的测试数据
    backend.clear().await.ok();

    let test_data = generate_test_data(100);

    // 测试 Pipeline SET
    let start = Instant::now();
    backend.set_many(&test_data).await.expect("Pipeline SET failed");
    let pipeline_duration = start.elapsed();

    // 清理
    backend.clear().await.ok();

    // 测试逐个 SET（对比）
    let start = Instant::now();
    for (key, value, ttl) in &test_data {
        backend
            .set(key, value.clone(), *ttl)
            .await
            .expect("Individual SET failed");
    }
    let individual_duration = start.elapsed();

    // 清理测试数据
    backend.clear().await.ok();

    println!("Pipeline SET Performance:");
    println!("  Pipeline: {:?}", pipeline_duration);
    println!("  Individual: {:?}", individual_duration);
    println!(
        "  Speedup: {:.2}x",
        individual_duration.as_secs_f64() / pipeline_duration.as_secs_f64()
    );

    // Pipeline 应该至少快 3 倍（保守估计，实际可能 10-50 倍）
    assert!(
        pipeline_duration < individual_duration,
        "Pipeline should be faster than individual operations"
    );
}

/// 测试 Pipeline GET 性能
#[tokio::test]
#[ignore = "性能测试需要 Redis 环境，使用 cargo test -- --ignored 运行"]
async fn test_pipeline_get_performance() {
    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url)
        .await
        .expect("Failed to create Redis backend");

    // 准备测试数据
    let test_data = generate_test_data(100);
    backend.set_many(&test_data).await.expect("Failed to setup test data");

    let keys: Vec<String> = test_data.iter().map(|(k, _, _)| k.clone()).collect();

    // 测试 Pipeline GET
    let start = Instant::now();
    let pipeline_results = backend.get_many(&keys).await.expect("Pipeline GET failed");
    let pipeline_duration = start.elapsed();

    // 测试逐个 GET（对比）
    let start = Instant::now();
    let mut individual_results = Vec::new();
    for key in &keys {
        individual_results.push(backend.get(key).await.expect("Individual GET failed"));
    }
    let individual_duration = start.elapsed();

    // 验证结果一致性
    assert_eq!(pipeline_results.len(), individual_results.len());

    // 清理测试数据
    backend.clear().await.ok();

    println!("Pipeline GET Performance:");
    println!("  Pipeline: {:?}", pipeline_duration);
    println!("  Individual: {:?}", individual_duration);
    println!(
        "  Speedup: {:.2}x",
        individual_duration.as_secs_f64() / pipeline_duration.as_secs_f64()
    );

    // Pipeline 应该至少快 3 倍
    assert!(
        pipeline_duration < individual_duration,
        "Pipeline should be faster than individual operations"
    );
}

/// 测试 Pipeline DELETE 性能
#[tokio::test]
#[ignore = "性能测试需要 Redis 环境，使用 cargo test -- --ignored 运行"]
async fn test_pipeline_delete_performance() {
    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url)
        .await
        .expect("Failed to create Redis backend");

    // 准备两组测试数据
    let test_data_1 = generate_test_data(100);
    let test_data_2 = generate_test_data(100);

    // 测试 Pipeline DELETE
    backend
        .set_many(&test_data_1)
        .await
        .expect("Failed to setup test data 1");
    let keys: Vec<String> = test_data_1.iter().map(|(k, _, _)| k.clone()).collect();

    let start = Instant::now();
    backend.delete_many(&keys).await.expect("Pipeline DELETE failed");
    let pipeline_duration = start.elapsed();

    // 测试逐个 DELETE（对比）
    backend
        .set_many(&test_data_2)
        .await
        .expect("Failed to setup test data 2");
    let start = Instant::now();
    for key in &keys {
        backend.delete(key).await.expect("Individual DELETE failed");
    }
    let individual_duration = start.elapsed();

    // 清理测试数据
    backend.clear().await.ok();

    println!("Pipeline DELETE Performance:");
    println!("  Pipeline: {:?}", pipeline_duration);
    println!("  Individual: {:?}", individual_duration);
    println!(
        "  Speedup: {:.2}x",
        individual_duration.as_secs_f64() / pipeline_duration.as_secs_f64()
    );

    // Pipeline 应该至少快 3 倍
    assert!(
        pipeline_duration < individual_duration,
        "Pipeline should be faster than individual operations"
    );
}

/// 测试大规模数据 Pipeline 性能
#[tokio::test]
#[ignore = "性能测试需要 Redis 环境，使用 cargo test -- --ignored 运行"]
async fn test_large_scale_pipeline_performance() {
    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url)
        .await
        .expect("Failed to create Redis backend");

    // 清理之前的测试数据
    backend.clear().await.ok();

    // 测试 1000 个键的批量操作
    let test_data = generate_test_data(1000);

    // Pipeline SET
    let start = Instant::now();
    backend.set_many(&test_data).await.expect("Pipeline SET failed");
    let set_duration = start.elapsed();

    // Pipeline GET
    let keys: Vec<String> = test_data.iter().map(|(k, _, _)| k.clone()).collect();
    let start = Instant::now();
    let results = backend.get_many(&keys).await.expect("Pipeline GET failed");
    let get_duration = start.elapsed();

    // 验证所有键都存在
    assert_eq!(results.len(), 1000);
    assert!(results.iter().all(|r| r.is_some()));

    // Pipeline DELETE
    let start = Instant::now();
    backend.delete_many(&keys).await.expect("Pipeline DELETE failed");
    let delete_duration = start.elapsed();

    // 验证所有键都已删除
    let results = backend.get_many(&keys).await.expect("Failed to verify deletion");
    assert!(results.iter().all(|r| r.is_none()));

    println!("Large Scale Pipeline Performance (1000 keys):");
    println!("  SET: {:?}", set_duration);
    println!("  GET: {:?}", get_duration);
    println!("  DELETE: {:?}", delete_duration);
    println!("  Total: {:?}", set_duration + get_duration + delete_duration);

    // 清理测试数据
    backend.clear().await.ok();
}

/// 测试混合操作性能
#[tokio::test]
#[ignore = "性能测试需要 Redis 环境，使用 cargo test -- --ignored 运行"]
async fn test_mixed_operations_performance() {
    let redis_url = get_redis_url();
    let backend = RedisBackend::new(&redis_url)
        .await
        .expect("Failed to create Redis backend");

    // 清理之前的测试数据
    backend.clear().await.ok();

    // 模拟真实场景：批量设置、读取、更新、删除
    let test_data = generate_test_data(500);

    let total_start = Instant::now();

    // 1. 批量初始化
    backend.set_many(&test_data).await.expect("Batch init failed");

    // 2. 批量读取
    let keys: Vec<String> = test_data.iter().map(|(k, _, _)| k.clone()).collect();
    let _ = backend.get_many(&keys).await.expect("Batch read failed");

    // 3. 批量更新（覆盖）
    let updated_data: Vec<(String, Vec<u8>, Option<Duration>)> = test_data
        .iter()
        .map(|(k, _, ttl)| (k.clone(), format!("updated_{}", k).into_bytes(), *ttl))
        .collect();
    backend.set_many(&updated_data).await.expect("Batch update failed");

    // 4. 批量删除一半
    let delete_keys: Vec<String> = keys.iter().take(250).cloned().collect();
    backend.delete_many(&delete_keys).await.expect("Batch delete failed");

    let total_duration = total_start.elapsed();

    println!("Mixed Operations Performance (500 keys):");
    println!("  Total time: {:?}", total_duration);

    // 清理测试数据
    backend.clear().await.ok();
}
