// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 字节级操作示例
//!
//! 本示例演示 Cache 的字节级操作 API（get_bytes / set_bytes），
//! 适用于需要直接控制序列化的场景。

use oxcache::Cache;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_BYTE_OPS_1_BASIC_BYTE_OPERATIONS, MSG_EXAMPLE_BYTE_OPS_2_BYTE_OPERATIONS_WITHOUT,
    MSG_EXAMPLE_BYTE_OPS_3_BINARY_DATA, MSG_EXAMPLE_BYTE_OPS_4_CACHE_METADATA_OPERATIONS,
    MSG_EXAMPLE_BYTE_OPS_5_SERIALIZER_ACCESS, MSG_EXAMPLE_BYTE_OPS_6_LARGE_DATA_OPERATIONS,
    MSG_EXAMPLE_BYTE_OPS_AFTER_CLEANUP_ENTRIES_IS,
    MSG_EXAMPLE_BYTE_OPS_BYTE_LEVEL_OPERATIONS_EXAMPLE, MSG_EXAMPLE_BYTE_OPS_CACHE_CLOSED,
    MSG_EXAMPLE_BYTE_OPS_CAPACITY, MSG_EXAMPLE_BYTE_OPS_CHECKSUM,
    MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MATCH, MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MISMATCH,
    MSG_EXAMPLE_BYTE_OPS_ENTRIES, MSG_EXAMPLE_BYTE_OPS_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_BYTE_OPS_IS_EMPTY, MSG_EXAMPLE_BYTE_OPS_READ, MSG_EXAMPLE_BYTE_OPS_READ_BYTES,
    MSG_EXAMPLE_BYTE_OPS_READ_BYTES_2, MSG_EXAMPLE_BYTE_OPS_READ_BYTES_3,
    MSG_EXAMPLE_BYTE_OPS_SERIALIZER_INSTANCE_OBTAINED_DYN, MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES,
    MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_2, MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_NO_TTL,
    MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_OF_BINARY, t,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}\n",
        t(MSG_EXAMPLE_BYTE_OPS_BYTE_LEVEL_OPERATIONS_EXAMPLE, &[])
    );

    let cache: Cache<String, Vec<u8>> = Cache::builder().capacity(100).build().await?;

    println!("{}", t(MSG_EXAMPLE_BYTE_OPS_1_BASIC_BYTE_OPERATIONS, &[]));

    let key = "raw_data".to_string();
    let data = b"Hello, oxcache!".to_vec();

    // set_bytes 接受 TTL 参数（秒），None 表示无过期
    cache.set_bytes(&key, data.clone(), Some(60)).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES,
            &[("data", data.len().to_string())]
        )
    );

    let retrieved = cache.get_bytes(&key).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_READ,
            &[(
                "retrieved",
                format!(
                    "{:?}",
                    retrieved.map(|d| String::from_utf8_lossy(&d).to_string())
                )
            )]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_BYTE_OPS_2_BYTE_OPERATIONS_WITHOUT, &[])
    );

    let key2 = "persistent_data".to_string();
    let data2 = b"Persistent data without TTL".to_vec();

    cache.set_bytes(&key2, data2.clone(), None).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_NO_TTL,
            &[("data2", data2.len().to_string())]
        )
    );

    let retrieved2 = cache.get_bytes(&key2).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_READ_BYTES,
            &[(
                "retrieved2",
                (retrieved2.as_ref().map(|d| d.len()).unwrap_or(0)).to_string()
            )]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_BYTE_OPS_3_BINARY_DATA, &[]));

    let key3 = "binary_data".to_string();
    let binary_data: Vec<u8> = (0..=255).collect();

    cache
        .set_bytes(&key3, binary_data.clone(), Some(300))
        .await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_OF_BINARY,
            &[("binary_data", binary_data.len().to_string())]
        )
    );

    let retrieved3 = cache.get_bytes(&key3).await?;
    if let Some(data) = retrieved3 {
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_BYTE_OPS_READ_BYTES_2,
                &[("data", data.len().to_string())]
            )
        );
        println!(
            "  {}",
            t(
                MSG_EXAMPLE_BYTE_OPS_CHECKSUM,
                &[(
                    "result",
                    if data == binary_data {
                        t(MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MATCH, &[])
                    } else {
                        t(MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MISMATCH, &[])
                    }
                )]
            )
        );
    }

    println!(
        "\n{}",
        t(MSG_EXAMPLE_BYTE_OPS_4_CACHE_METADATA_OPERATIONS, &[])
    );

    let count = cache.len().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_ENTRIES,
            &[("count", count.to_string())]
        )
    );

    let empty = cache.is_empty().await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_IS_EMPTY,
            &[("empty", empty.to_string())]
        )
    );

    let cap = cache.capacity().await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_BYTE_OPS_CAPACITY, &[("cap", cap.to_string())])
    );

    println!("\n{}", t(MSG_EXAMPLE_BYTE_OPS_5_SERIALIZER_ACCESS, &[]));

    let _serializer = cache.serializer();
    println!(
        "  {}",
        t(MSG_EXAMPLE_BYTE_OPS_SERIALIZER_INSTANCE_OBTAINED_DYN, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_BYTE_OPS_6_LARGE_DATA_OPERATIONS, &[]));

    let key4 = "large_data".to_string();
    let large_data = vec![0xABu8; 1024]; // 1KB

    cache.set_bytes(&key4, large_data.clone(), Some(60)).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_2,
            &[("large_data", large_data.len().to_string())]
        )
    );

    let retrieved4 = cache.get_bytes(&key4).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_READ_BYTES_3,
            &[(
                "retrieved4",
                (retrieved4.as_ref().map(|d| d.len()).unwrap_or(0)).to_string()
            )]
        )
    );

    cache.clear().await?;

    let count_after = cache.len().await?;
    let empty_after = cache.is_empty().await?;
    println!(
        "\n  {}",
        t(
            MSG_EXAMPLE_BYTE_OPS_AFTER_CLEANUP_ENTRIES_IS,
            &[
                ("count_after", count_after.to_string()),
                ("empty_after", empty_after.to_string())
            ]
        )
    );

    cache.shutdown().await;
    println!("  {}", t(MSG_EXAMPLE_BYTE_OPS_CACHE_CLOSED, &[]));

    println!("\n{}", t(MSG_EXAMPLE_BYTE_OPS_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
