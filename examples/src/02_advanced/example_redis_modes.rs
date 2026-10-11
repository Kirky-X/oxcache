// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Redis 多模式连接示例
//!
//! 本示例演示 oxcache 支持的三种 Redis 连接模式：
//! - Standalone（单机模式）：最常用的单节点连接
//! - Sentinel（哨兵模式）：高可用自动故障转移
//! - Cluster（集群模式）：水平扩展分片存储
//!
//! 运行方式：
//! ```bash
//! # 本地开发需允许非 TLS 连接（生产请使用 rediss://）：
//! # workspace 根（默认 features 未启用 redis 时仅打印指引后正常退出）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --example example_redis_modes
//! # 完整演示（二选一）：
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run --features redis --example example_redis_modes
//! OXCACHE_ALLOW_INSECURE_REDIS=I_UNDERSTAND_THE_RISKS \
//!   cargo run -p oxcache-examples --example example_redis_modes
//! ```

// 本文件由主包与 oxcache-examples 包共享（单源双注册）：cfg(feature) 判定的
// 是编译方包自身的 features——主包默认不含 redis，降级为指引输出保证
// `cargo run --example` 可解析运行；examples 包 default 含 redis，走完整演示。
#[cfg(feature = "redis")]
use oxcache::backend::{CacheReader, CacheWriter};
#[cfg(feature = "redis")]
use oxcache::backend::{RedisBackend, RedisMode};

#[cfg(feature = "redis")]
use oxcache::i18n::messages::{
    MSG_EXAMPLE_REDIS_MODES_1_STANDALONE_MODE,
    MSG_EXAMPLE_REDIS_MODES_2_EXPLICITLY_SELECTING_STANDALONE,
    MSG_EXAMPLE_REDIS_MODES_3_CLUSTER_MODE, MSG_EXAMPLE_REDIS_MODES_4_SENTINEL_MODE,
    MSG_EXAMPLE_REDIS_MODES_5_THE_REDISMODETYPE_ENUM, MSG_EXAMPLE_REDIS_MODES_BUILDER_CREATED_MODE,
    MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTED_MODE,
    MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTION_FAILED, MSG_EXAMPLE_REDIS_MODES_CONNECTED,
    MSG_EXAMPLE_REDIS_MODES_CONNECTED_2, MSG_EXAMPLE_REDIS_MODES_CONNECTED_MODE,
    MSG_EXAMPLE_REDIS_MODES_EXAMPLE_COMPLETE, MSG_EXAMPLE_REDIS_MODES_MODE_DISPLAY,
    MSG_EXAMPLE_REDIS_MODES_REDIS_CLUSTER_AVAILABLE_IS,
    MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION_2,
    MSG_EXAMPLE_REDIS_MODES_REDIS_SENTINEL_AVAILABLE_IS,
    MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTED_MODE, MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION,
    MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION_FAILED,
    MSG_EXAMPLE_REDIS_MODES_START_CLUSTER_CD_TESTS,
    MSG_EXAMPLE_REDIS_MODES_START_SENTINEL_CD_TESTS, MSG_EXAMPLE_REDIS_MODES_WRITE_READ,
    MSG_EXAMPLE_REDIS_MODES_WRITE_READ_2, MSG_EXAMPLE_REDIS_MODES_WRITE_READ_3, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "redis"))]
    {
        use oxcache::i18n::messages::{
            MSG_EXAMPLE_REDIS_MODES_FEATURE_DISABLED,
            MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION,
            MSG_EXAMPLE_REDIS_MODES_RUN_WITH_EXAMPLES_PACKAGE,
            MSG_EXAMPLE_REDIS_MODES_RUN_WITH_REDIS_FEATURE, t,
        };
        println!(
            "{}\n",
            t(MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION, &[])
        );
        println!("  {}", t(MSG_EXAMPLE_REDIS_MODES_FEATURE_DISABLED, &[]));
        println!(
            "  {}",
            t(MSG_EXAMPLE_REDIS_MODES_RUN_WITH_REDIS_FEATURE, &[])
        );
        println!(
            "       {}",
            t(MSG_EXAMPLE_REDIS_MODES_RUN_WITH_EXAMPLES_PACKAGE, &[])
        );
        return Ok(());
    }

    #[cfg(feature = "redis")]
    run().await
}

#[cfg(feature = "redis")]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}\n",
        t(MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION_2, &[])
    );

    println!("{}", t(MSG_EXAMPLE_REDIS_MODES_1_STANDALONE_MODE, &[]));
    let standalone_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_MODES_CONNECTED,
            &[("standalone_url", standalone_url.to_string())]
        )
    );

    let standalone = match RedisBackend::new(&standalone_url).await {
        Ok(backend) => backend,
        Err(e) => {
            use oxcache::i18n::messages::{
                MSG_EXAMPLE_REDIS_MODES_DONE_STANDALONE_SKIPPED,
                MSG_EXAMPLE_REDIS_MODES_STANDALONE_CONNECT_FAILED,
                MSG_EXAMPLE_REDIS_MODES_STANDALONE_ENV_REQUIRED, t,
            };
            println!(
                "  {}",
                t(
                    MSG_EXAMPLE_REDIS_MODES_STANDALONE_CONNECT_FAILED,
                    &[("err", e.to_string())]
                )
            );
            println!(
                "    {}",
                t(MSG_EXAMPLE_REDIS_MODES_STANDALONE_ENV_REQUIRED, &[])
            );
            print_mode_enum();
            println!(
                "\n{}",
                t(MSG_EXAMPLE_REDIS_MODES_DONE_STANDALONE_SKIPPED, &[])
            );
            return Ok(());
        }
    };
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_MODES_CONNECTED_MODE,
            &[("mode", standalone.mode().to_string())]
        )
    );

    // 基本操作测试
    standalone
        .set("mode:standalone".into(), b"hello".to_vec().into(), None)
        .await?;
    let value = standalone.get("mode:standalone").await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_MODES_WRITE_READ,
            &[(
                "value",
                format!(
                    "{:?}",
                    value.map(|v| String::from_utf8_lossy(&v).to_string())
                )
            )]
        )
    );
    standalone.delete("mode:standalone").await?;

    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_REDIS_MODES_2_EXPLICITLY_SELECTING_STANDALONE,
            &[]
        )
    );
    let backend = RedisBackend::builder()
        .connection_string(&standalone_url)
        .mode(RedisMode::Standalone)
        .build()
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_REDIS_MODES_BUILDER_CREATED_MODE,
            &[("mode", backend.mode().to_string())]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_REDIS_MODES_3_CLUSTER_MODE, &[]));
    let cluster_available = std::env::var("REDIS_CLUSTER_AVAILABLE").is_ok();

    if cluster_available {
        let cluster_url = "redis://127.0.0.1:7000";
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_REDIS_MODES_CONNECTED_2,
                &[("cluster_url", cluster_url.to_string())]
            )
        );

        match RedisBackend::new(cluster_url).await {
            Ok(cluster) => {
                println!(
                    "  {}",
                    t(
                        MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTED_MODE,
                        &[("mode", cluster.mode().to_string())]
                    )
                );

                // Cluster 下的基本操作
                cluster
                    .set(
                        "mode:cluster".into(),
                        b"cluster_value".to_vec().into(),
                        None,
                    )
                    .await?;
                let value = cluster.get("mode:cluster").await?;
                println!(
                    "  {}",
                    t(
                        MSG_EXAMPLE_REDIS_MODES_WRITE_READ_2,
                        &[(
                            "value",
                            format!(
                                "{:?}",
                                value.map(|v| String::from_utf8_lossy(&v).to_string())
                            )
                        )]
                    )
                );
                cluster.delete("mode:cluster").await?;
            }
            Err(e) => {
                println!(
                    "  {}",
                    t(
                        MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTION_FAILED,
                        &[("e", e.to_string())]
                    )
                );
            }
        }
    } else {
        println!(
            "  {}",
            t(MSG_EXAMPLE_REDIS_MODES_REDIS_CLUSTER_AVAILABLE_IS, &[])
        );
        println!(
            "    {}",
            t(MSG_EXAMPLE_REDIS_MODES_START_CLUSTER_CD_TESTS, &[])
        );
    }

    println!("\n{}", t(MSG_EXAMPLE_REDIS_MODES_4_SENTINEL_MODE, &[]));
    let sentinel_available = std::env::var("REDIS_SENTINEL_AVAILABLE").is_ok();

    if sentinel_available {
        let sentinel_url = "redis://127.0.0.1:26382";
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION,
                &[("sentinel_url", sentinel_url.to_string())]
            )
        );

        // Sentinel 必须用显式模式构建：RedisBackend::new 默认 Standalone，
        // 直连哨兵端口会把哨兵进程当数据节点（SET 被哨兵拒绝）。
        match RedisBackend::builder()
            .connection_string(sentinel_url)
            .mode(RedisMode::Sentinel)
            .build()
            .await
        {
            Ok(sentinel) => {
                println!(
                    "  {}",
                    t(
                        MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTED_MODE,
                        &[("mode", sentinel.mode().to_string())]
                    )
                );

                // Sentinel 下的基本操作
                sentinel
                    .set(
                        "mode:sentinel".into(),
                        b"sentinel_value".to_vec().into(),
                        None,
                    )
                    .await?;
                let value = sentinel.get("mode:sentinel").await?;
                println!(
                    "  {}",
                    t(
                        MSG_EXAMPLE_REDIS_MODES_WRITE_READ_3,
                        &[(
                            "value",
                            format!(
                                "{:?}",
                                value.map(|v| String::from_utf8_lossy(&v).to_string())
                            )
                        )]
                    )
                );
                sentinel.delete("mode:sentinel").await?;
            }
            Err(e) => {
                println!(
                    "  {}",
                    t(
                        MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION_FAILED,
                        &[("e", e.to_string())]
                    )
                );
            }
        }
    } else {
        println!(
            "  {}",
            t(MSG_EXAMPLE_REDIS_MODES_REDIS_SENTINEL_AVAILABLE_IS, &[])
        );
        println!(
            "    {}",
            t(MSG_EXAMPLE_REDIS_MODES_START_SENTINEL_CD_TESTS, &[])
        );
    }

    print_mode_enum();

    println!("\n{}", t(MSG_EXAMPLE_REDIS_MODES_EXAMPLE_COMPLETE, &[]));
    Ok(())
}

/// 枚举展示段（Standalone 连接失败时作为独立降级出口复用）。
#[cfg(feature = "redis")]
fn print_mode_enum() {
    println!(
        "\n{}",
        t(MSG_EXAMPLE_REDIS_MODES_5_THE_REDISMODETYPE_ENUM, &[])
    );
    let modes = [
        RedisMode::Standalone,
        RedisMode::Sentinel,
        RedisMode::Cluster,
    ];
    for mode in &modes {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_REDIS_MODES_MODE_DISPLAY,
                &[("mode", mode.to_string()), ("mode2", mode.to_string())]
            )
        );
    }
}
