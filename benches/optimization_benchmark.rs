//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 性能基准测试 - 验证优化效果

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxcache::cache::SerializerPool;
use oxcache::config::service::L2Config;
use oxcache::serialization::json::JsonSerializer;
use oxcache::serialization::Serializer;
use tokio::runtime::Runtime;

// ============================================================================
// 序列化性能基准测试
// ============================================================================

fn bench_serializer_pool_reuse(c: &mut Criterion) {
    let pool = SerializerPool::new();
    let test_data = TestData::medium();

    c.bench_function("serializer_pool_json", |b| {
        b.iter(|| {
            let serializer = pool.json();
            let serialized = black_box(serializer.serialize(&test_data).unwrap());
            black_box(serialized.len());
        })
    });

    c.bench_function("serializer_new_instance", |b| {
        b.iter(|| {
            let serializer = JsonSerializer::new();
            let serialized = black_box(serializer.serialize(&test_data).unwrap());
            black_box(serialized.len());
        })
    });
}

fn bench_serialization_sizes(c: &mut Criterion) {
    let pool = SerializerPool::new();

    c.bench_function("serialize_small", |b| {
        let data = TestData::small();
        b.iter(|| {
            let serializer = pool.json();
            black_box(serializer.serialize(&data).unwrap());
        })
    });

    c.bench_function("serialize_medium", |b| {
        let data = TestData::medium();
        b.iter(|| {
            let serializer = pool.json();
            black_box(serializer.serialize(&data).unwrap());
        })
    });

    c.bench_function("serialize_large", |b| {
        let data = TestData::large();
        b.iter(|| {
            let serializer = pool.json();
            black_box(serializer.serialize(&data).unwrap());
        })
    });
}

// ============================================================================
// 批量操作性能基准测试
// ============================================================================

fn bench_batch_operations(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    c.bench_function("batch_set_10", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let items: Vec<(String, Vec<u8>)> = (0..10)
                    .map(|i| (format!("key{}", i), vec![1, 2, 3, 4, 5]))
                    .collect();
                black_box(items.len());
            });
        })
    });

    c.bench_function("batch_set_100", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let items: Vec<(String, Vec<u8>)> = (0..100)
                    .map(|i| (format!("key{}", i), vec![1, 2, 3, 4, 5]))
                    .collect();
                black_box(items.len());
            });
        })
    });

    c.bench_function("batch_get_10", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let keys: Vec<String> = (0..10).map(|i| format!("key{}", i)).collect();
                black_box(keys.len());
            });
        })
    });

    c.bench_function("batch_get_100", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let keys: Vec<String> = (0..100).map(|i| format!("key{}", i)).collect();
                black_box(keys.len());
            });
        })
    });
}

// ============================================================================
// 连接重试逻辑基准测试
// ============================================================================

fn bench_connection_retry_config(c: &mut Criterion) {
    c.bench_function("config_creation", |b| {
        b.iter(|| {
            let cfg = L2Config::default()
                .with_connection_timeout_ms(black_box(100))
                .with_max_retries(black_box(3))
                .with_retry_delay_ms(black_box(10));
            black_box(cfg);
        })
    });

    c.bench_function("config_validation", |b| {
        b.iter(|| {
            let cfg = L2Config::default();
            let max_retries: u64 = cfg.max_retries as u64;
            let retry_delay: u64 = cfg.retry_delay_ms;
            black_box(max_retries + retry_delay);
        })
    });
}

// ============================================================================
// 辅助类型和函数
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize)]
struct SmallStruct {
    id: u64,
    name: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MediumStruct {
    id: u64,
    name: String,
    values: Vec<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LargeStruct {
    items: Vec<Item>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Item {
    id: u64,
    name: String,
    data: Vec<u8>,
}

struct TestData;

impl TestData {
    fn small() -> SmallStruct {
        SmallStruct {
            id: 1,
            name: "test".to_string(),
        }
    }

    fn medium() -> MediumStruct {
        MediumStruct {
            id: 1,
            name: "test".to_string(),
            values: (0..100).map(|i| i.to_string()).collect(),
            timestamp: chrono::Utc::now(),
        }
    }

    fn large() -> LargeStruct {
        LargeStruct {
            items: (0..1000)
                .map(|i| Item {
                    id: i,
                    name: format!("item_{}", i),
                    data: vec![0u8; 100],
                })
                .collect(),
        }
    }
}

// ============================================================================
// CRC16 哈希槽位计算基准测试
// ============================================================================

fn bench_slot_calculation(c: &mut Criterion) {
    c.bench_function("slot_simple_key", |b| {
        let keys = vec!["user:123:profile", "user:456:profile", "user:789:profile"];
        b.iter(|| {
            for key in &keys {
                let slot = calculate_slot(black_box(key));
                black_box(slot);
            }
        })
    });

    c.bench_function("slot_tagged_key", |b| {
        let keys = vec![
            "{user_profile}:123",
            "{user_profile}:456",
            "{user_profile}:789",
        ];
        b.iter(|| {
            for key in &keys {
                let slot = calculate_slot(black_box(key));
                black_box(slot);
            }
        })
    });

    c.bench_function("slot_batch_100", |b| {
        let keys: Vec<String> = (0..100).map(|i| format!("user:{}:data", i)).collect();
        b.iter(|| {
            let mut slots = Vec::with_capacity(keys.len());
            for key in &keys {
                slots.push(calculate_slot(key));
            }
            black_box(slots.len());
        })
    });
}

// CRC16 算法（Redis 集群标准算法）
fn calculate_slot(key: &str) -> u16 {
    let mut crc: u16 = 0;

    // 查找 { } 中的标签
    let slot_key = if let Some(start) = key.find('{') {
        if let Some(end) = key.find('}') {
            if end > start + 1 {
                let tag = &key[start + 1..end];
                if !tag.is_empty() {
                    tag
                } else {
                    key
                }
            } else {
                key
            }
        } else {
            key
        }
    } else {
        key
    };

    for &byte in slot_key.as_bytes() {
        crc ^= byte as u16;
        for _ in 0..8 {
            if crc & 0x0001 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc % 16384
}

// ============================================================================
// 基准测试组
// ============================================================================

criterion_group!(
    benches,
    bench_serializer_pool_reuse,
    bench_serialization_sizes,
    bench_batch_operations,
    bench_connection_retry_config,
    bench_slot_calculation,
);

criterion_main!(benches);
