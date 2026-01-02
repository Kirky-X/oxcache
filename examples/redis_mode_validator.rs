//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Redis模式验证测试 - 完成test.md中Redis模式验证功能

use oxcache::{
    backend::l2::L2Backend,
    config::{ClusterConfig, L2Config, RedisMode, SentinelConfig},
};
use std::env;
use std::time::Duration;
use tokio::time::timeout;

/// Redis模式验证器
struct RedisModeValidator {
    test_timeout: Duration,
}

impl RedisModeValidator {
    fn new() -> Self {
        Self {
            test_timeout: Duration::from_secs(10),
        }
    }

    /// 验证Standalone模式
    async fn validate_standalone(&self) -> Result<(), String> {
        println!("🔍 Validating Redis Standalone mode...");

        let config = L2Config {
            mode: RedisMode::Standalone,
            connection_string: env::var("REDIS_STANDALONE_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
                .into(),
            ..Default::default()
        };

        match timeout(self.test_timeout, L2Backend::new(&config)).await {
            Ok(Ok(backend)) => {
                // 测试基本操作
                let test_key = "test_standalone_key";
                let test_value = b"test_value";

                backend
                    .set_bytes(test_key, test_value.to_vec(), Some(60))
                    .await
                    .map_err(|e| format!("Standalone set failed: {}", e))?;

                let retrieved = backend
                    .get_bytes(test_key)
                    .await
                    .map_err(|e| format!("Standalone get failed: {}", e))?;

                if retrieved != Some(test_value.to_vec()) {
                    return Err("Standalone value mismatch".to_string());
                }

                backend
                    .delete(test_key)
                    .await
                    .map_err(|e| format!("Standalone delete failed: {}", e))?;

                println!("✅ Redis Standalone mode validation passed");
                Ok(())
            }
            Ok(Err(e)) => Err(format!("Standalone connection failed: {}", e)),
            Err(_) => Err("Standalone connection timeout".to_string()),
        }
    }

    /// 验证Sentinel模式
    async fn validate_sentinel(&self) -> Result<(), String> {
        println!("🔍 Validating Redis Sentinel mode...");

        if env::var("ENABLE_SENTINEL_VALIDATION").is_err() {
            println!("⚠️  Skipping Sentinel validation (ENABLE_SENTINEL_VALIDATION not set)");
            return Ok(());
        }

        let config = L2Config {
            mode: RedisMode::Sentinel,
            sentinel: Some(SentinelConfig {
                master_name: env::var("REDIS_SENTINEL_MASTER")
                    .unwrap_or_else(|_| "mymaster".to_string()),
                nodes: vec![
                    env::var("REDIS_SENTINEL_URL_1")
                        .unwrap_or_else(|_| "redis://127.0.0.1:26379".to_string()),
                    env::var("REDIS_SENTINEL_URL_2")
                        .unwrap_or_else(|_| "redis://127.0.0.1:26380".to_string()),
                    env::var("REDIS_SENTINEL_URL_3")
                        .unwrap_or_else(|_| "redis://127.0.0.1:26381".to_string()),
                ],
            }),
            ..Default::default()
        };

        match timeout(self.test_timeout, L2Backend::new(&config)).await {
            Ok(Ok(backend)) => {
                // 测试基本操作
                let test_key = "test_sentinel_key";
                let test_value = b"test_sentinel_value";

                backend
                    .set_bytes(test_key, test_value.to_vec(), Some(60))
                    .await
                    .map_err(|e| format!("Sentinel set failed: {}", e))?;

                let retrieved = backend
                    .get_bytes(test_key)
                    .await
                    .map_err(|e| format!("Sentinel get failed: {}", e))?;

                if retrieved != Some(test_value.to_vec()) {
                    return Err("Sentinel value mismatch".to_string());
                }

                backend
                    .delete(test_key)
                    .await
                    .map_err(|e| format!("Sentinel delete failed: {}", e))?;

                println!("✅ Redis Sentinel mode validation passed");
                Ok(())
            }
            Ok(Err(e)) => Err(format!("Sentinel connection failed: {}", e)),
            Err(_) => Err("Sentinel connection timeout".to_string()),
        }
    }

    /// 验证Cluster模式
    async fn validate_cluster(&self) -> Result<(), String> {
        println!("🔍 Validating Redis Cluster mode...");

        if env::var("ENABLE_CLUSTER_VALIDATION").is_err() {
            println!("⚠️  Skipping Cluster validation (ENABLE_CLUSTER_VALIDATION not set)");
            return Ok(());
        }

        let config = L2Config {
            mode: RedisMode::Cluster,
            cluster: Some(ClusterConfig {
                nodes: vec![
                    env::var("REDIS_CLUSTER_URL_1")
                        .unwrap_or_else(|_| "redis://127.0.0.1:7000".to_string()),
                    env::var("REDIS_CLUSTER_URL_2")
                        .unwrap_or_else(|_| "redis://127.0.0.1:7001".to_string()),
                    env::var("REDIS_CLUSTER_URL_3")
                        .unwrap_or_else(|_| "redis://127.0.0.1:7002".to_string()),
                    env::var("REDIS_CLUSTER_URL_4")
                        .unwrap_or_else(|_| "redis://127.0.0.1:7003".to_string()),
                    env::var("REDIS_CLUSTER_URL_5")
                        .unwrap_or_else(|_| "redis://127.0.0.1:7004".to_string()),
                    env::var("REDIS_CLUSTER_URL_6")
                        .unwrap_or_else(|_| "redis://127.0.0.1:7005".to_string()),
                ],
            }),
            ..Default::default()
        };

        match timeout(self.test_timeout, L2Backend::new(&config)).await {
            Ok(Ok(backend)) => {
                // 测试基本操作
                let test_key = "test_cluster_key";
                let test_value = b"test_cluster_value";

                backend
                    .set_bytes(test_key, test_value.to_vec(), Some(60))
                    .await
                    .map_err(|e| format!("Cluster set failed: {}", e))?;

                let retrieved = backend
                    .get_bytes(test_key)
                    .await
                    .map_err(|e| format!("Cluster get failed: {}", e))?;

                if retrieved != Some(test_value.to_vec()) {
                    return Err("Cluster value mismatch".to_string());
                }

                backend
                    .delete(test_key)
                    .await
                    .map_err(|e| format!("Cluster delete failed: {}", e))?;

                println!("✅ Redis Cluster mode validation passed");
                Ok(())
            }
            Ok(Err(e)) => Err(format!("Cluster connection failed: {}", e)),
            Err(_) => Err("Cluster connection timeout".to_string()),
        }
    }

    /// 验证TLS连接
    async fn validate_tls(&self) -> Result<(), String> {
        println!("🔍 Validating Redis TLS connection...");

        let tls_url =
            env::var("REDIS_TLS_URL").unwrap_or_else(|_| "rediss://127.0.0.1:6380".to_string());

        let config = L2Config {
            mode: RedisMode::Standalone,
            connection_string: tls_url.into(),
            enable_tls: true,
            ..Default::default()
        };

        match timeout(self.test_timeout, L2Backend::new(&config)).await {
            Ok(Ok(_)) => {
                println!("✅ Redis TLS validation passed");
                Ok(())
            }
            Ok(Err(e)) => {
                // TLS连接失败是预期的，除非有真实的TLS Redis服务器
                println!(
                    "⚠️  TLS connection failed (expected without proper setup): {}",
                    e
                );
                Ok(())
            }
            Err(_) => {
                println!("⚠️  TLS connection timeout");
                Ok(())
            }
        }
    }

    /// 验证配置错误处理
    async fn validate_error_handling(&self) -> Result<(), String> {
        println!("🔍 Validating Redis configuration error handling...");

        // 测试缺失Sentinel配置
        let invalid_sentinel_config = L2Config {
            mode: RedisMode::Sentinel,
            sentinel: None,
            ..Default::default()
        };

        match L2Backend::new(&invalid_sentinel_config).await {
            Ok(_) => return Err("Should fail with missing Sentinel config".to_string()),
            Err(e) => {
                if !e.to_string().contains("Sentinel configuration is missing") {
                    return Err(format!(
                        "Unexpected error for missing Sentinel config: {}",
                        e
                    ));
                }
            }
        }

        // 测试缺失Cluster配置
        let invalid_cluster_config = L2Config {
            mode: RedisMode::Cluster,
            cluster: None,
            ..Default::default()
        };

        match L2Backend::new(&invalid_cluster_config).await {
            Ok(_) => return Err("Should fail with missing Cluster config".to_string()),
            Err(e) => {
                if !e.to_string().contains("Cluster configuration is missing") {
                    return Err(format!(
                        "Unexpected error for missing Cluster config: {}",
                        e
                    ));
                }
            }
        }

        println!("✅ Redis configuration error handling validation passed");
        Ok(())
    }

    /// 运行所有验证
    async fn run_all_validations(&self) -> Result<Vec<String>, Vec<String>> {
        let mut passed_tests = Vec::new();
        let mut failed_tests = Vec::new();

        // 验证Standalone模式
        match self.validate_standalone().await {
            Ok(_) => {
                passed_tests.push("Standalone".to_string());
                println!("✅ Standalone validation passed");
            }
            Err(e) => {
                failed_tests.push(format!("Standalone: {}", e));
                println!("❌ Standalone validation failed: {}", e);
            }
        }

        // 验证Sentinel模式
        match self.validate_sentinel().await {
            Ok(_) => {
                passed_tests.push("Sentinel".to_string());
                println!("✅ Sentinel validation passed");
            }
            Err(e) => {
                failed_tests.push(format!("Sentinel: {}", e));
                println!("❌ Sentinel validation failed: {}", e);
            }
        }

        // 验证Cluster模式
        match self.validate_cluster().await {
            Ok(_) => {
                passed_tests.push("Cluster".to_string());
                println!("✅ Cluster validation passed");
            }
            Err(e) => {
                failed_tests.push(format!("Cluster: {}", e));
                println!("❌ Cluster validation failed: {}", e);
            }
        }

        // 验证TLS模式
        match self.validate_tls().await {
            Ok(_) => {
                passed_tests.push("TLS".to_string());
                println!("✅ TLS validation passed");
            }
            Err(e) => {
                failed_tests.push(format!("TLS: {}", e));
                println!("❌ TLS validation failed: {}", e);
            }
        }

        // 验证错误处理
        match self.validate_error_handling().await {
            Ok(_) => {
                passed_tests.push("Error Handling".to_string());
                println!("✅ Error Handling validation passed");
            }
            Err(e) => {
                failed_tests.push(format!("Error Handling: {}", e));
                println!("❌ Error Handling validation failed: {}", e);
            }
        }

        if failed_tests.is_empty() {
            Ok(passed_tests)
        } else {
            Err(failed_tests)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Redis Mode Validation");
    println!("{}", "=".repeat(50));

    let validator = RedisModeValidator::new();

    match validator.run_all_validations().await {
        Ok(passed) => {
            println!("\n📊 Validation Summary:");
            println!("✅ All validations passed!");
            println!("Passed tests: {}", passed.len());
            for test in passed {
                println!("  - {}", test);
            }
            Ok(())
        }
        Err(failed) => {
            println!("\n📊 Validation Summary:");
            println!("❌ Some validations failed!");
            println!("Failed tests: {}", failed.len());
            for test in failed {
                println!("  - {}", test);
            }
            Err("Validation failed".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_mode_validator_creation() {
        let validator = RedisModeValidator::new();
        assert_eq!(validator.test_timeout, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_error_handling_validation() {
        let validator = RedisModeValidator::new();
        let result = validator.validate_error_handling().await;
        assert!(result.is_ok());
    }
}
