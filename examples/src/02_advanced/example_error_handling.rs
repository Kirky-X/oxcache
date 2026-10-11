// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 错误处理示例
//!
//! 本示例演示 oxcache 的完整错误处理机制：
//! - OxCacheError 各变体的触发与分类
//! - 可恢复 vs 不可恢复错误的判断
//! - 带重试的弹性操作
//! - 错误码获取与日志记录
//! - 配置阶段错误（OxCacheConfigError）
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_error_handling
//! ```

use oxcache::Cache;
use oxcache::error::{OxCacheError, OxCacheResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Config {
    name: String,
    value: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Error handling example ===\n");

    println!("--- 1. Error type classification ---");
    demonstrate_error_categories();

    println!("\n--- 2. Recoverability ---");
    demonstrate_recoverability();

    println!("\n--- 3. Error codes and logging ---");
    demonstrate_error_codes();

    println!("\n--- 4. Resilient operations with retry ---");
    demonstrate_retry_pattern().await?;

    println!("\n--- 5. Triggering runtime errors ---");
    demonstrate_runtime_errors().await?;

    println!("\n--- 6. Error propagation with the ? operator ---");
    demonstrate_error_propagation().await?;

    println!("\n--- 7. Configuration-time errors ---");
    demonstrate_config_errors();

    println!("\n✓ Error handling example complete");
    Ok(())
}

/// 演示 OxCacheError 各变体的分类
fn demonstrate_error_categories() {
    // 连接类错误 — 通常是暂时性的
    let conn_err = OxCacheError::Connection("Redis connection refused".into());
    println!(
        "  connection error: {} (recoverable: {})",
        conn_err,
        conn_err.is_recoverable()
    );

    // 超时类错误 — 可重试
    let timeout_err = OxCacheError::Timeout("no response within 5s".into());
    println!(
        "  timeout error: {} (recoverable: {})",
        timeout_err,
        timeout_err.is_recoverable()
    );

    // 后端错误 — 可能是暂时性的
    let backend_err = OxCacheError::BackendError("Moka out of memory".into());
    println!(
        "  backend error: {} (recoverable: {})",
        backend_err,
        backend_err.is_recoverable()
    );

    // 未找到 — 不可恢复（需要回源）
    let not_found = OxCacheError::NotFound("user:42".into());
    println!(
        "  not found:   {} (recoverable: {})",
        not_found,
        not_found.is_recoverable()
    );

    // 序列化错误 — 通常不可恢复
    let ser_err = OxCacheError::Serialization("invalid UTF-8".into());
    println!(
        "  serialization:   {} (recoverable: {})",
        ser_err,
        ser_err.is_recoverable()
    );

    // 降级模式
    let degraded = OxCacheError::Degraded("L2 Redis unavailable, L1 only".into());
    println!(
        "  degraded:     {} (degraded mode: {})",
        degraded,
        degraded.is_degraded()
    );

    // 内部错误 — 不可恢复，不应重试
    let internal = OxCacheError::Internal("lock poisoned".into());
    println!(
        "  internal error: {} (recoverable: {})",
        internal,
        internal.is_recoverable()
    );
}

/// 演示错误的可恢复性判断
fn demonstrate_recoverability() {
    let errors: Vec<(&str, OxCacheError)> = vec![
        ("Connection", OxCacheError::Connection("refused".into())),
        ("Timeout", OxCacheError::Timeout("30s".into())),
        (
            "BackendError",
            OxCacheError::BackendError("transient".into()),
        ),
        (
            "BufferFull",
            OxCacheError::BufferFull("batch buffer full".into()),
        ),
        ("NotFound", OxCacheError::NotFound("key".into())),
        (
            "Serialization",
            OxCacheError::Serialization("bad data".into()),
        ),
        ("Internal", OxCacheError::Internal("lock poisoned".into())),
        ("KeyTooLong", OxCacheError::KeyTooLong(1024, 512)),
    ];

    println!(
        "  {:<16} {:<10} {:<12} error code",
        "error type", "recoverable?", "connection error?"
    );
    println!("  {}", "-".repeat(50));
    for (name, err) in &errors {
        println!(
            "  {:<16} {:<10} {:<12} {}",
            name,
            err.is_recoverable(),
            err.is_connection_error(),
            err.code(),
        );
    }
}

/// 演示错误码获取
fn demonstrate_error_codes() {
    let err = OxCacheError::Connection("failed to connect to redis://localhost:6379".into());
    println!("  error message: {}", err);
    println!("  error code:   {}", err.code());
    println!("  retryable:   {}", err.is_recoverable());

    // 模拟结构化日志
    println!("\n  [LOG] code={} level=ERROR msg=\"{}\"", err.code(), err);
}

/// 演示带重试的弹性操作模式
async fn demonstrate_retry_pattern() -> OxCacheResult<()> {
    let cache: Cache<String, Config> = Cache::builder().build().await?;

    /// 带指数退避的重试包装器
    async fn with_retry<F, Fut, T>(
        operation_name: &str,
        max_retries: u32,
        mut operation: F,
    ) -> OxCacheResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = OxCacheResult<T>>,
    {
        for attempt in 0..=max_retries {
            match operation().await {
                Ok(val) => {
                    if attempt > 0 {
                        println!("  ✓ {} succeeded on retry {}", operation_name, attempt);
                    }
                    return Ok(val);
                }
                Err(e) => {
                    if !e.is_recoverable() {
                        println!("  ✗ {} hit an unrecoverable error: {}", operation_name, e);
                        return Err(e);
                    }
                    if attempt < max_retries {
                        let delay = std::time::Duration::from_millis(10 * 2u64.pow(attempt));
                        println!(
                            "  ⚠ {} failed on attempt {} ({}), retrying in {}ms...",
                            operation_name,
                            attempt + 1,
                            e.code(),
                            delay.as_millis(),
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        println!(
                            "  ✗ {} reached the maximum retry count {}",
                            operation_name, max_retries
                        );
                        return Err(e);
                    }
                }
            }
        }
        unreachable!()
    }

    // 模拟一个可能暂时失败的操作
    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter = attempt_count.clone();

    let result = with_retry("read config", 3, || {
        let counter = counter.clone();
        async move {
            let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < 2 {
                // 前两次模拟暂时性失败
                Err(OxCacheError::Connection(
                    "simulate an unstable connection".into(),
                ))
            } else {
                Ok("config data".to_string())
            }
        }
    })
    .await?;
    println!("  final result: {}", result);

    // 演示不可恢复错误不会重试
    let not_found_result = with_retry("lookup cache", 3, || async {
        Err::<String, _>(OxCacheError::NotFound("nonexistent key".into()))
    })
    .await;
    println!(
        "  unrecoverable error result: {}",
        if not_found_result.is_err() {
            "correctly skipped the retry"
        } else {
            "unexpected"
        }
    );

    // 实际缓存操作
    let key = "retry_config".to_string();
    cache
        .set(
            &key,
            &Config {
                name: "timeout".into(),
                value: 30,
            },
        )
        .await?;
    let cached = cache.get(&key).await?;
    println!("  cache operation succeeded: {:?}", cached.map(|c| c.name));

    Ok(())
}

/// 演示运行时错误触发
async fn demonstrate_runtime_errors() -> OxCacheResult<()> {
    let cache: Cache<String, String> = Cache::builder().capacity(2).build().await?;

    // 正常操作
    cache.set(&"k1".to_string(), &"v1".to_string()).await?;
    let val = cache.get(&"k1".to_string()).await?;
    println!("  normal read: {:?}", val);

    // 读取不存在的键 — 返回 None 而非错误
    let missing = cache.get(&"nonexistent".to_string()).await?;
    println!("  a missing key returns: {:?} (not an error)", missing);

    // 使用 get_or 处理缓存未命中
    let value = cache
        .get_or(&"computed".to_string(), || async {
            Ok("computed result".to_string())
        })
        .await?;
    println!("  get_or computed result: {}", value);

    // 演示 health_check
    match cache.health_check().await {
        Ok(()) => println!("  health check: ✓ passed"),
        Err(e) => println!("  health check: ✗ {} ({})", e, e.code()),
    }

    Ok(())
}

/// 演示错误传播与 ? 操作符
async fn demonstrate_error_propagation() -> OxCacheResult<()> {
    let cache: Cache<String, i32> = Cache::builder().build().await?;

    /// 业务函数：将错误向上传播
    async fn load_user_score(cache: &Cache<String, i32>, user_id: u64) -> OxCacheResult<i32> {
        let key = format!("score:{}", user_id);
        cache.get(&key).await?.ok_or_else(|| {
            OxCacheError::NotFound(format!("the score for user {} does not exist", user_id))
        })
    }

    // 先设置数据
    cache.set(&"score:1".to_string(), &100).await?;

    // 存在的用户
    match load_user_score(&cache, 1).await {
        Ok(score) => println!("  user 1 score: {}", score),
        Err(e) => println!("  load failed: {}", e),
    }

    // 不存在的用户
    match load_user_score(&cache, 999).await {
        Ok(score) => println!("  user 999 score: {}", score),
        Err(e) => println!("  user 999 load failed: {} (code={})", e, e.code()),
    }

    Ok(())
}

/// 演示配置阶段错误
fn demonstrate_config_errors() {
    // 配置错误示例（使用 OxCacheError 模拟，因为 OxCacheConfigError 需要 redis feature）
    let errors = vec![
        OxCacheError::InvalidInput("capacity must not be 0".into()),
        OxCacheError::InvalidKey("key must not contain forbidden character \\0".into()),
        OxCacheError::KeyTooLong(1024, 512),
        OxCacheError::ValueTooLarge(2048, 1024),
    ];

    for err in &errors {
        println!("  config error: {} [code={}]", err, err.code());
    }
}
