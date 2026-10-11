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

use oxcache::i18n::messages::{
    MSG_EXAMPLE_ERROR_HANDLING_1_ERROR_TYPE_CLASSIFICATION,
    MSG_EXAMPLE_ERROR_HANDLING_2_RECOVERABILITY, MSG_EXAMPLE_ERROR_HANDLING_3_ERROR_CODES_AND,
    MSG_EXAMPLE_ERROR_HANDLING_4_RESILIENT_OPERATIONS_WITH,
    MSG_EXAMPLE_ERROR_HANDLING_5_TRIGGERING_RUNTIME_ERRORS,
    MSG_EXAMPLE_ERROR_HANDLING_6_ERROR_PROPAGATION_WITH,
    MSG_EXAMPLE_ERROR_HANDLING_7_CONFIGURATION_TIME_ERRORS,
    MSG_EXAMPLE_ERROR_HANDLING_A_MISSING_KEY_RETURNS,
    MSG_EXAMPLE_ERROR_HANDLING_BACKEND_ERROR_RECOVERABLE,
    MSG_EXAMPLE_ERROR_HANDLING_CACHE_OPERATION_SUCCEEDED,
    MSG_EXAMPLE_ERROR_HANDLING_COL_CONNECTION_ERROR, MSG_EXAMPLE_ERROR_HANDLING_COL_ERROR_TYPE,
    MSG_EXAMPLE_ERROR_HANDLING_COL_RECOVERABLE, MSG_EXAMPLE_ERROR_HANDLING_CONFIG_ERROR_CODE,
    MSG_EXAMPLE_ERROR_HANDLING_CONNECTION_ERROR_RECOVERABLE,
    MSG_EXAMPLE_ERROR_HANDLING_DEGRADED_DEGRADED_MODE, MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE,
    MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE_2, MSG_EXAMPLE_ERROR_HANDLING_ERROR_MESSAGE,
    MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE, MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_ERROR_HANDLING_FAILED_ON_ATTEMPT_RETRYING, MSG_EXAMPLE_ERROR_HANDLING_FINAL_RESULT,
    MSG_EXAMPLE_ERROR_HANDLING_GET_OR_COMPUTED_RESULT, MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK,
    MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK_PASSED,
    MSG_EXAMPLE_ERROR_HANDLING_HIT_AN_UNRECOVERABLE_ERROR,
    MSG_EXAMPLE_ERROR_HANDLING_INTERNAL_ERROR_RECOVERABLE, MSG_EXAMPLE_ERROR_HANDLING_LOAD_FAILED,
    MSG_EXAMPLE_ERROR_HANDLING_NORMAL_READ, MSG_EXAMPLE_ERROR_HANDLING_NOT_FOUND_RECOVERABLE,
    MSG_EXAMPLE_ERROR_HANDLING_REACHED_THE_MAXIMUM_RETRY,
    MSG_EXAMPLE_ERROR_HANDLING_RESULT_RETRY_SKIPPED, MSG_EXAMPLE_ERROR_HANDLING_RESULT_UNEXPECTED,
    MSG_EXAMPLE_ERROR_HANDLING_RETRYABLE, MSG_EXAMPLE_ERROR_HANDLING_SERIALIZATION_RECOVERABLE,
    MSG_EXAMPLE_ERROR_HANDLING_SUCCEEDED_ON_RETRY,
    MSG_EXAMPLE_ERROR_HANDLING_TIMEOUT_ERROR_RECOVERABLE,
    MSG_EXAMPLE_ERROR_HANDLING_UNRECOVERABLE_ERROR_RESULT, MSG_EXAMPLE_ERROR_HANDLING_USER_1_SCORE,
    MSG_EXAMPLE_ERROR_HANDLING_USER_999_LOAD_FAILED, MSG_EXAMPLE_ERROR_HANDLING_USER_999_SCORE, t,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Config {
    name: String,
    value: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE, &[]));

    println!(
        "{}",
        t(MSG_EXAMPLE_ERROR_HANDLING_1_ERROR_TYPE_CLASSIFICATION, &[])
    );
    demonstrate_error_categories();

    println!("\n{}", t(MSG_EXAMPLE_ERROR_HANDLING_2_RECOVERABILITY, &[]));
    demonstrate_recoverability();

    println!("\n{}", t(MSG_EXAMPLE_ERROR_HANDLING_3_ERROR_CODES_AND, &[]));
    demonstrate_error_codes();

    println!(
        "\n{}",
        t(MSG_EXAMPLE_ERROR_HANDLING_4_RESILIENT_OPERATIONS_WITH, &[])
    );
    demonstrate_retry_pattern().await?;

    println!(
        "\n{}",
        t(MSG_EXAMPLE_ERROR_HANDLING_5_TRIGGERING_RUNTIME_ERRORS, &[])
    );
    demonstrate_runtime_errors().await?;

    println!(
        "\n{}",
        t(MSG_EXAMPLE_ERROR_HANDLING_6_ERROR_PROPAGATION_WITH, &[])
    );
    demonstrate_error_propagation().await?;

    println!(
        "\n{}",
        t(MSG_EXAMPLE_ERROR_HANDLING_7_CONFIGURATION_TIME_ERRORS, &[])
    );
    demonstrate_config_errors();

    println!("\n{}", t(MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE_COMPLETE, &[]));
    Ok(())
}

/// 演示 OxCacheError 各变体的分类
fn demonstrate_error_categories() {
    // 连接类错误 — 通常是暂时性的
    let conn_err = OxCacheError::Connection("Redis connection refused".into());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_CONNECTION_ERROR_RECOVERABLE,
            &[
                ("conn_err", conn_err.to_string()),
                ("is_recoverable", conn_err.is_recoverable().to_string())
            ]
        )
    );

    // 超时类错误 — 可重试
    let timeout_err = OxCacheError::Timeout("no response within 5s".into());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_TIMEOUT_ERROR_RECOVERABLE,
            &[
                ("timeout_err", timeout_err.to_string()),
                ("is_recoverable", timeout_err.is_recoverable().to_string())
            ]
        )
    );

    // 后端错误 — 可能是暂时性的
    let backend_err = OxCacheError::BackendError("Moka out of memory".into());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_BACKEND_ERROR_RECOVERABLE,
            &[
                ("backend_err", backend_err.to_string()),
                ("is_recoverable", backend_err.is_recoverable().to_string())
            ]
        )
    );

    // 未找到 — 不可恢复（需要回源）
    let not_found = OxCacheError::NotFound("user:42".into());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_NOT_FOUND_RECOVERABLE,
            &[
                ("not_found", not_found.to_string()),
                ("is_recoverable", not_found.is_recoverable().to_string())
            ]
        )
    );

    // 序列化错误 — 通常不可恢复
    let ser_err = OxCacheError::Serialization("invalid UTF-8".into());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_SERIALIZATION_RECOVERABLE,
            &[
                ("ser_err", ser_err.to_string()),
                ("is_recoverable", ser_err.is_recoverable().to_string())
            ]
        )
    );

    // 降级模式
    let degraded = OxCacheError::Degraded("L2 Redis unavailable, L1 only".into());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_DEGRADED_DEGRADED_MODE,
            &[
                ("degraded", degraded.to_string()),
                ("is_degraded", degraded.is_degraded().to_string())
            ]
        )
    );

    // 内部错误 — 不可恢复，不应重试
    let internal = OxCacheError::Internal("lock poisoned".into());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_INTERNAL_ERROR_RECOVERABLE,
            &[
                ("internal", internal.to_string()),
                ("is_recoverable", internal.is_recoverable().to_string())
            ]
        )
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
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE,
            &[
                (
                    "type",
                    format!("{:<16}", t(MSG_EXAMPLE_ERROR_HANDLING_COL_ERROR_TYPE, &[]))
                ),
                (
                    "recoverable",
                    format!("{:<10}", t(MSG_EXAMPLE_ERROR_HANDLING_COL_RECOVERABLE, &[]))
                ),
                (
                    "error",
                    format!(
                        "{:<12}",
                        t(MSG_EXAMPLE_ERROR_HANDLING_COL_CONNECTION_ERROR, &[])
                    )
                ),
            ]
        )
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
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_ERROR_MESSAGE,
            &[("err", err.to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE_2,
            &[("code", err.code().to_string())]
        )
    );
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_RETRYABLE,
            &[("is_recoverable", err.is_recoverable().to_string())]
        )
    );

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
                        println!(
                            "  {}",
                            t(
                                MSG_EXAMPLE_ERROR_HANDLING_SUCCEEDED_ON_RETRY,
                                &[
                                    ("operation_name", operation_name.to_string()),
                                    ("attempt", attempt.to_string())
                                ]
                            )
                        );
                    }
                    return Ok(val);
                }
                Err(e) => {
                    if !e.is_recoverable() {
                        println!(
                            "  {}",
                            t(
                                MSG_EXAMPLE_ERROR_HANDLING_HIT_AN_UNRECOVERABLE_ERROR,
                                &[
                                    ("operation_name", operation_name.to_string()),
                                    ("e", e.to_string())
                                ]
                            )
                        );
                        return Err(e);
                    }
                    if attempt < max_retries {
                        let delay = std::time::Duration::from_millis(10 * 2u64.pow(attempt));
                        println!(
                            "  {}",
                            t(
                                MSG_EXAMPLE_ERROR_HANDLING_FAILED_ON_ATTEMPT_RETRYING,
                                &[
                                    ("operation_name", operation_name.to_string()),
                                    ("attempt", (attempt + 1).to_string()),
                                    ("code", e.code().to_string()),
                                    ("as_millis", delay.as_millis().to_string())
                                ]
                            )
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        println!(
                            "  {}",
                            t(
                                MSG_EXAMPLE_ERROR_HANDLING_REACHED_THE_MAXIMUM_RETRY,
                                &[
                                    ("operation_name", operation_name.to_string()),
                                    ("max_retries", max_retries.to_string())
                                ]
                            )
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
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_FINAL_RESULT,
            &[("result", result.to_string())]
        )
    );

    // 演示不可恢复错误不会重试
    let not_found_result = with_retry("lookup cache", 3, || async {
        Err::<String, _>(OxCacheError::NotFound("nonexistent key".into()))
    })
    .await;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_UNRECOVERABLE_ERROR_RESULT,
            &[(
                "result",
                if not_found_result.is_err() {
                    t(MSG_EXAMPLE_ERROR_HANDLING_RESULT_RETRY_SKIPPED, &[])
                } else {
                    t(MSG_EXAMPLE_ERROR_HANDLING_RESULT_UNEXPECTED, &[])
                }
            )]
        )
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
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_CACHE_OPERATION_SUCCEEDED,
            &[("name", format!("{:?}", cached.map(|c| c.name)))]
        )
    );

    Ok(())
}

/// 演示运行时错误触发
async fn demonstrate_runtime_errors() -> OxCacheResult<()> {
    let cache: Cache<String, String> = Cache::builder().capacity(2).build().await?;

    // 正常操作
    cache.set(&"k1".to_string(), &"v1".to_string()).await?;
    let val = cache.get(&"k1".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_NORMAL_READ,
            &[("val", format!("{:?}", val))]
        )
    );

    // 读取不存在的键 — 返回 None 而非错误
    let missing = cache.get(&"nonexistent".to_string()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_A_MISSING_KEY_RETURNS,
            &[("missing", format!("{:?}", missing))]
        )
    );

    // 使用 get_or 处理缓存未命中
    let value = cache
        .get_or(&"computed".to_string(), || async {
            Ok("computed result".to_string())
        })
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_ERROR_HANDLING_GET_OR_COMPUTED_RESULT,
            &[("value", value.to_string())]
        )
    );

    // 演示 health_check
    match cache.health_check().await {
        Ok(()) => println!(
            "  {}",
            t(MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK_PASSED, &[])
        ),
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK,
                &[("e", e.to_string()), ("code", e.code().to_string())]
            )
        ),
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

    cache.set(&"score:1".to_string(), &100).await?;

    // 存在的用户
    match load_user_score(&cache, 1).await {
        Ok(score) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_ERROR_HANDLING_USER_1_SCORE,
                &[("score", score.to_string())]
            )
        ),
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_ERROR_HANDLING_LOAD_FAILED,
                &[("e", e.to_string())]
            )
        ),
    }

    // 不存在的用户
    match load_user_score(&cache, 999).await {
        Ok(score) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_ERROR_HANDLING_USER_999_SCORE,
                &[("score", score.to_string())]
            )
        ),
        Err(e) => println!(
            "  {}",
            t(
                MSG_EXAMPLE_ERROR_HANDLING_USER_999_LOAD_FAILED,
                &[("e", e.to_string()), ("code", e.code().to_string())]
            )
        ),
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
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_ERROR_HANDLING_CONFIG_ERROR_CODE,
                &[("err", err.to_string()), ("code", err.code().to_string())]
            )
        );
    }
}
