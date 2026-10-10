// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// Redis版本兼容性测试 - 支持Redis 6.0, 6.2, 7.0, 7.2等多个版本
//
// 版本矩阵只能对"真实存在的对应版本端点"给出结论：
// - `test_redis_6_compatibility` 仅在显式设置 `REDIS_6_URL` 且端点自报
//   redis_version 6.x 时执行断言，否则显性跳过（不再把本机 6379 当作 6.x
//   错标通过）；
// - `test_redis_7_compatibility` 用 `REDIS_7_URL`，未设置时回落到
//   `REDIS_URL`/默认端点，并以 `INFO server` 的 redis_version 校验 7.x；
// - 综合用例仅跑显式配置的端点，摘要按 passed/failed/skipped 如实输出。

#![cfg(feature = "redis")]

use oxcache::backend::memory::RedisBackend;
use oxcache::backend::{CacheReader, CacheWriter};
use std::sync::Arc;

/// 查询端点自报的 `redis_version`（`INFO server`）。
async fn server_version(connection_string: &str) -> Result<String, String> {
    let client = redis::Client::open(connection_string)
        .map_err(|e| format!("invalid connection string {connection_string}: {e}"))?;
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("connect {connection_string} failed: {e}"))?;
    let info: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut conn)
        .await
        .map_err(|e| format!("INFO server on {connection_string} failed: {e}"))?;
    info.lines()
        .find_map(|line| line.strip_prefix("redis_version:"))
        .map(|v| v.trim().to_string())
        .ok_or_else(|| format!("redis_version missing from INFO server on {connection_string}"))
}

/// 测试指定Redis版本的Standalone模式兼容性（先校验端点自报版本，防错标）。
async fn test_redis_version_standalone(
    version: &str,
    connection_string: &str,
) -> Result<(), String> {
    let actual = server_version(connection_string).await?;
    let expected_major = version.split('.').next().unwrap_or(version);
    if !actual.starts_with(expected_major) {
        return Err(format!(
            "server at {connection_string} reports redis_version {actual}, expected {version}"
        ));
    }
    println!(
        "Testing Redis {} Standalone compatibility (server reports {})...",
        version, actual
    );

    let backend: RedisBackend = RedisBackend::new(connection_string)
        .await
        .map_err(|e| format!("Redis {} connection failed: {}", version, e))?;

    // 测试基本操作
    let test_key = format!("test:{}:compatibility", version.replace('.', "_"));
    let test_value = format!("Redis {} compatibility test data", version);

    // SET
    backend
        .set(
            Arc::from(test_key.as_str()),
            Arc::new(test_value.as_bytes().to_vec()),
            Some(std::time::Duration::from_secs(60)),
        )
        .await
        .map_err(|e| format!("Redis {} set failed: {}", version, e))?;

    // GET
    let retrieved = backend
        .get(test_key.as_str())
        .await
        .map_err(|e| format!("Redis {} get failed: {}", version, e))?;

    if retrieved != Some(test_value.as_bytes().to_vec()) {
        return Err(format!("Redis {} value mismatch", version));
    }

    // DELETE
    backend
        .delete(test_key.as_str())
        .await
        .map_err(|e| format!("Redis {} delete failed: {}", version, e))?;

    println!("✅ Redis {} Standalone compatibility passed", version);
    Ok(())
}

/// 一条版本腿的结果。
enum LegOutcome {
    Passed,
    /// 环境不具备该版本端点（未配置/不可用/自报版本不符），如实跳过。
    Skipped(String),
}

/// 端点可用且自报指定 major 版本时运行 `test_redis_version_standalone`，
/// 否则返回跳过原因（不虚报通过）。
async fn run_version_leg(version: &str, url: &str) -> Result<LegOutcome, String> {
    if !crate::common::is_redis_available_url(url).await {
        return Ok(LegOutcome::Skipped(format!("endpoint {url} unavailable")));
    }
    match server_version(url).await {
        Ok(actual) if actual.starts_with(version.split('.').next().unwrap_or(version)) => {
            test_redis_version_standalone(version, url).await?;
            Ok(LegOutcome::Passed)
        }
        Ok(actual) => Ok(LegOutcome::Skipped(format!(
            "endpoint {url} reports {actual}, not {version}"
        ))),
        Err(e) => Ok(LegOutcome::Skipped(e)),
    }
}

/// 测试Redis 6.x版本兼容性（需显式 `REDIS_6_URL`，版本自报校验）。
#[tokio::test]
async fn test_redis_6_compatibility() {
    let Ok(connection_string) = std::env::var("REDIS_6_URL") else {
        println!(
            "[TEST-SKIP] Redis 6.x compatibility test: REDIS_6_URL not set; \
             this leg requires a real Redis 6.x endpoint"
        );
        return;
    };

    match run_version_leg("6", &connection_string).await {
        Ok(LegOutcome::Passed) => println!("Redis 6.x compatibility test passed"),
        Ok(LegOutcome::Skipped(reason)) => {
            println!("[TEST-SKIP] Redis 6.x compatibility test: {reason}")
        }
        Err(e) => panic!("Redis 6.x compatibility failed: {e}"),
    }
}

/// 测试Redis 7.x版本兼容性（`REDIS_7_URL`，未设置时回落 `REDIS_URL`/默认端点）。
#[tokio::test]
async fn test_redis_7_compatibility() {
    let connection_string =
        std::env::var("REDIS_7_URL").unwrap_or_else(|_| crate::common::get_redis_url_insecure());

    match run_version_leg("7", &connection_string).await {
        Ok(LegOutcome::Passed) => println!("Redis 7.x compatibility test passed"),
        Ok(LegOutcome::Skipped(reason)) => {
            println!("[TEST-SKIP] Redis 7.x compatibility test: {reason}")
        }
        Err(e) => panic!("Redis 7.x compatibility failed: {e}"),
    }
}

/// 全面的Redis多版本兼容性测试
///
/// 仅测试显式配置的版本端点（`REDIS_6_0_URL`/`REDIS_7_0_URL`），
/// 摘要如实区分 passed / failed / skipped，不再把未配置的端点当作通过。
#[tokio::test]
async fn test_comprehensive_redis_version_compatibility() {
    println!("Starting comprehensive Redis version compatibility tests...");

    if std::env::var("REDIS_VERSION_TEST_ENABLED").is_err() {
        println!("Redis version compatibility tests are disabled.");
        println!("Set REDIS_VERSION_TEST_ENABLED=1 to enable these tests.");
        return;
    }

    // (version, URL)：6.0 仅显式端点；7.0 显式端点或回落默认端点（版本自报校验）
    let configs = vec![
        (
            "6.0",
            std::env::var("REDIS_6_0_URL")
                .ok()
                .or_else(|| std::env::var("REDIS_6_URL").ok()),
        ),
        (
            "7.0",
            std::env::var("REDIS_7_0_URL")
                .ok()
                .or_else(|| std::env::var("REDIS_7_URL").ok())
                .or_else(|| Some(crate::common::get_redis_url_insecure())),
        ),
    ];

    let mut passed_tests = Vec::new();
    let mut failed_tests = Vec::new();
    let mut skipped_tests = Vec::new();

    for (version, url) in configs {
        println!("\nTesting Redis {}...", version);

        let Some(url) = url else {
            let reason = format!("no {version} endpoint configured (env not set)");
            skipped_tests.push(format!("{} Standalone: {}", version, reason));
            println!("  ⚠️  Standalone mode skipped: {}", reason);
            continue;
        };

        match run_version_leg(version, &url).await {
            Ok(LegOutcome::Passed) => {
                passed_tests.push(format!("{} Standalone", version));
                println!("  ✅ Standalone mode passed");
            }
            Ok(LegOutcome::Skipped(reason)) => {
                skipped_tests.push(format!("{} Standalone: {}", version, reason));
                println!("  ⚠️  Standalone mode skipped: {}", reason);
            }
            Err(e) => {
                failed_tests.push(format!("{} Standalone: {}", version, e));
                println!("  ❌ Standalone mode failed: {}", e);
            }
        }
    }

    println!("\nRedis Version Compatibility Test Summary:");
    println!("  ✅ Passed: {}", passed_tests.len());
    println!("  ❌ Failed: {}", failed_tests.len());
    println!("  ⚠️  Skipped: {}", skipped_tests.len());

    if !failed_tests.is_empty() {
        panic!(
            "Redis version compatibility tests failed: {:?}",
            failed_tests
        );
    }

    println!("\n🎉 All configured Redis version compatibility tests completed!");
}
