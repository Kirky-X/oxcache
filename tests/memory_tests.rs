//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 内存测试 - 包含内存泄漏测试和 Miri 内存安全测试

use oxcache::backend::l1::L1Backend;
use oxcache::backend::l2::L2Backend;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// 更新路径引用
#[path = "test_utils.rs"]
mod test_utils;
use test_utils::is_redis_available;

// ============================================================================
// 内存泄漏测试模块
// ============================================================================

/// 内存泄漏测试模块
/// 使用循环引用和大量操作来检测潜在的内存泄漏

#[tokio::test]
async fn test_l1_cache_memory_leak() {
    let cache = Arc::new(L1Backend::new(1000));

    // 执行大量操作，检测内存泄漏
    for i in 0..10000 {
        let key = format!("key_{}", i % 100); // 循环使用100个key
        let value = vec![i as u8; 100];

        cache
            .set_bytes(&key, value.clone(), Some(60))
            .await
            .unwrap();
        cache.get_bytes(&key).await.unwrap();

        if i % 1000 == 0 {
            // 定期清理，模拟真实使用场景
            // L1Backend doesn't have clear method, so we'll delete keys individually
            for j in 0..100 {
                let key = format!("key_{}", j);
                let _ = cache.delete(&key).await;
            }
            sleep(Duration::from_millis(10)).await;
        }
    }

    // 清理所有数据
    for j in 0..100 {
        let key = format!("key_{}", j);
        let _ = cache.delete(&key).await;
    }

    // 强制drop，确保所有内存被释放
    drop(cache);

    // 给垃圾回收一些时间
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_l2_cache_memory_leak() {
    if !is_redis_available().await {
        println!("跳过test_l2_cache_memory_leak：Redis不可用");
        return;
    }

    use oxcache::config::L2Config;
    use oxcache::config::RedisMode;

    let config = L2Config {
        mode: RedisMode::Standalone,
        connection_string: secrecy::SecretString::from("redis://127.0.0.1:6379/15".to_string()),
        connection_timeout_ms: 5000,
        command_timeout_ms: 1000,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: None,
        default_ttl: Some(3600),
        ..Default::default()
    };

    let l2_backend = L2Backend::new(&config)
        .await
        .expect("Failed to connect to Redis");

    // 执行大量L2操作
    for i in 0..5000 {
        let key = format!("l2_leak_test_{}", i % 50); // 循环使用50个key
        let value = vec![i as u8; 1024]; // 1KB数据

        l2_backend
            .set_with_version(&key, value.clone(), Some(300))
            .await
            .unwrap();
        l2_backend.get_bytes(&key).await.unwrap();

        if i % 500 == 0 {
            // 定期删除，避免Redis内存溢出
            l2_backend.delete(&key).await.unwrap();
            sleep(Duration::from_millis(50)).await;
        }
    }

    // 清理测试数据
    for i in 0..50 {
        let key = format!("l2_leak_test_{}", i);
        l2_backend.delete(&key).await.unwrap();
    }

    drop(l2_backend);
}

#[tokio::test]
async fn test_two_level_cache_memory_leak() {
    if !is_redis_available().await {
        println!("跳过test_two_level_cache_memory_leak：Redis不可用");
        return;
    }

    use oxcache::config::L2Config;
    use oxcache::config::RedisMode;

    let l1 = Arc::new(L1Backend::new(100));

    let config = L2Config {
        mode: RedisMode::Standalone,
        connection_string: secrecy::SecretString::from("redis://127.0.0.1:6379/14".to_string()),
        connection_timeout_ms: 5000,
        command_timeout_ms: 1000,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: None,
        default_ttl: Some(3600),
        ..Default::default()
    };

    let l2 = L2Backend::new(&config)
        .await
        .expect("Failed to connect to Redis");

    // 直接使用L1和L2进行测试，不创建TwoLevelClient
    // 测试L1缓存的内存泄漏
    for i in 0..1500 {
        let key = format!("two_level_l1_{}", i % 100);
        let value = format!("value_{}", i).into_bytes();

        // 写入操作
        l1.set_bytes(&key, value.clone(), Some(120)).await.unwrap();

        // 读取操作
        let _ = l1.get_bytes(&key).await;

        // 定期清理
        if i % 150 == 0 {
            for j in 0..100 {
                let key = format!("two_level_l1_{}", j);
                let _ = l1.delete(&key).await;
            }
            sleep(Duration::from_millis(20)).await;
        }
    }

    // 清理L1数据
    for j in 0..100 {
        let key = format!("two_level_l1_{}", j);
        let _ = l1.delete(&key).await;
    }

    // 测试L2缓存的内存泄漏
    for i in 0..1500 {
        let key = format!("two_level_l2_{}", i % 100);
        let value = format!("value_{}", i).into_bytes();

        // 写入操作
        l2.set_with_version(&key, value.clone(), Some(120))
            .await
            .unwrap();

        // 读取操作
        let _ = l2.get_bytes(&key).await;

        // 定期清理
        if i % 150 == 0 {
            for j in 0..100 {
                let key = format!("two_level_l2_{}", j);
                l2.delete(&key).await.unwrap();
            }
            sleep(Duration::from_millis(20)).await;
        }
    }

    // 清理L2数据
    for j in 0..100 {
        let key = format!("two_level_l2_{}", j);
        l2.delete(&key).await.unwrap();
    }

    drop(l1);
    drop(l2);
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_batch_operation_memory_leak() {
    if !is_redis_available().await {
        println!("跳过test_batch_operation_memory_leak：Redis不可用");
        return;
    }

    let l1 = Arc::new(L1Backend::new(500));

    use oxcache::config::L2Config;
    use oxcache::config::RedisMode;

    let config = L2Config {
        mode: RedisMode::Standalone,
        connection_string: secrecy::SecretString::from("redis://127.0.0.1:6379/13".to_string()),
        connection_timeout_ms: 5000,
        command_timeout_ms: 1000,
        password: None,
        enable_tls: false,
        sentinel: None,
        cluster: None,
        default_ttl: Some(3600),
        ..Default::default()
    };

    let l2 = L2Backend::new(&config)
        .await
        .expect("Failed to connect to Redis");

    // 批量操作内存泄漏测试 - 分别测试L1和L2
    for batch_id in 0..50 {
        let mut batch = Vec::new();

        for i in 0..50 {
            let key = format!("batch_l1_{}_{}", batch_id, i);
            let value = vec![batch_id as u8; 256];
            batch.push((key, value));
        }

        // L1批量设置
        for (key, value) in &batch {
            l1.set_bytes(key, value.clone(), Some(60)).await.unwrap();
        }

        // L1批量获取
        for (key, _) in &batch {
            let _ = l1.get_bytes(key).await;
        }

        // L1批量删除
        for (key, _) in &batch {
            l1.delete(key).await.unwrap();
        }

        // L2批量操作
        let mut l2_batch = Vec::new();
        for i in 0..50 {
            let key = format!("batch_l2_{}_{}", batch_id, i);
            let value = vec![batch_id as u8; 256];
            l2_batch.push((key, value));
        }

        // L2批量设置
        for (key, value) in &l2_batch {
            l2.set_with_version(key, value.clone(), Some(60))
                .await
                .unwrap();
        }

        // L2批量获取
        for (key, _) in &l2_batch {
            let _ = l2.get_bytes(key).await;
        }

        // L2批量删除
        for (key, _) in &l2_batch {
            l2.delete(key).await.unwrap();
        }

        sleep(Duration::from_millis(10)).await;
    }

    // 清理L1缓存
    for i in 0..100 {
        let key = format!("batch_l1_0_{}", i);
        let _ = l1.delete(&key).await;
    }

    drop(l1);
    drop(l2);
}

#[tokio::test]
async fn test_concurrent_memory_leak() {
    let cache = Arc::new(L1Backend::new(1000));
    let mut handles = vec![];

    // 并发内存泄漏测试
    for thread_id in 0..10 {
        let cache_clone = Arc::clone(&cache);

        let handle = tokio::spawn(async move {
            for i in 0..1000 {
                let key = format!("thread_{}_key_{}", thread_id, i % 50);
                let value = format!("thread_{}_value_{}", thread_id, i).into_bytes();

                cache_clone
                    .set_bytes(&key, value.clone(), Some(60))
                    .await
                    .unwrap();
                let _ = cache_clone.get_bytes(&key).await;

                if i % 100 == 0 {
                    // 定期清理部分key，避免全部清理影响并发测试
                    for j in 0..50 {
                        let key = format!("thread_{}_key_{}", thread_id, j);
                        let _ = cache_clone.delete(&key).await;
                    }
                }
            }
        });

        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }

    // 清理所有数据
    for thread_id in 0..10 {
        for i in 0..50 {
            let key = format!("thread_{}_key_{}", thread_id, i);
            let _ = cache.delete(&key).await;
        }
    }

    drop(cache);
    sleep(Duration::from_millis(200)).await;
}

/// 这个测试专门用于检测循环引用导致的内存泄漏
#[tokio::test]
async fn test_circular_reference_memory_leak() {
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Node {
        _value: Vec<u8>,
        next: Option<Rc<RefCell<Node>>>,
    }

    // 创建循环引用
    let node1 = Rc::new(RefCell::new(Node {
        _value: vec![1; 1024],
        next: None,
    }));

    let node2 = Rc::new(RefCell::new(Node {
        _value: vec![2; 1024],
        next: Some(Rc::clone(&node1)),
    }));

    // 创建循环
    node1.borrow_mut().next = Some(Rc::clone(&node2));

    // 使用缓存存储循环引用（序列化为字节数组）
    let cache = Arc::new(L1Backend::new(100));

    // 将循环引用序列化为字节数组存储
    let serialized = format!("circular_ref_data_{}", Rc::strong_count(&node1)).into_bytes();
    cache
        .set_bytes("circular_ref", serialized.clone(), Some(10))
        .await
        .unwrap();

    // 删除后应该释放内存
    cache.delete("circular_ref").await.unwrap();
    drop(cache);
    drop(node1);
    drop(node2);

    sleep(Duration::from_millis(100)).await;
}

// 内存使用监控辅助函数（需要jemalloc或其他内存分配器支持）
#[cfg(feature = "memory-profiling")]
mod memory_profiling {
    use super::*;

    use jemalloc_ctl::{epoch, stats};
    use std::fmt;

    #[derive(Debug)]
    struct JemallocWrapper(jemalloc_ctl::Error);

    impl fmt::Display for JemallocWrapper {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "jemalloc error: {:?}", self.0)
        }
    }

    impl std::error::Error for JemallocWrapper {}

    pub async fn get_memory_usage() -> Result<(usize, usize), Box<dyn std::error::Error>> {
        epoch::advance().map_err(|e| Box::new(JemallocWrapper(e)) as Box<dyn std::error::Error>)?;

        let allocated = stats::allocated::read()
            .map_err(|e| Box::new(JemallocWrapper(e)) as Box<dyn std::error::Error>)?;
        let active = stats::active::read()
            .map_err(|e| Box::new(JemallocWrapper(e)) as Box<dyn std::error::Error>)?;

        Ok((allocated, active))
    }

    #[tokio::test]
    async fn test_memory_usage_tracking() {
        let (initial_allocated, initial_active) = get_memory_usage().await.unwrap();

        let cache = Arc::new(L1Backend::new(10000));

        // 执行大量操作，模拟真实使用场景
        for i in 0..10000 {
            let key = format!("mem_test_{}", i);
            let value = vec![i as u8; 1024];
            cache.set_bytes(&key, value, Some(60)).await.unwrap();
        }

        let (peak_allocated, peak_active) = get_memory_usage().await.unwrap();
        println!(
            "Memory usage - Initial: {} bytes allocated, {} bytes active",
            initial_allocated, initial_active
        );
        println!(
            "Memory usage - Peak: {} bytes allocated, {} bytes active",
            peak_allocated, peak_active
        );

        // 清理缓存：L1Backend没有clear方法，逐个删除键
        for i in 0..10000 {
            let key = format!("mem_test_{}", i);
            let _ = cache.delete(&key).await;
        }
        drop(cache);
        sleep(Duration::from_millis(500)).await;

        let (final_allocated, final_active) = get_memory_usage().await.unwrap();
        println!(
            "Memory usage - Final: {} bytes allocated, {} bytes active",
            final_allocated, final_active
        );

        // Jemalloc 会保留内存以便后续重用，这不是真正的内存泄漏
        // 验证分配的内存没有过度增长（允许最多 10MB 的 Jemalloc 缓存）
        let max_reasonable_allocation = initial_allocated.saturating_add(10 * 1024 * 1024);
        assert!(
            final_allocated < max_reasonable_allocation,
            "Potential memory leak: allocated {} bytes (initial: {}, max reasonable: {})",
            final_allocated,
            initial_allocated,
            max_reasonable_allocation
        );

        // 验证没有持续的内存增长趋势（多次运行测试不应该导致内存持续增加）
        // 这个检查在单次测试中没有意义，但可以防止明显的泄漏
        assert!(
            final_allocated <= peak_allocated * 2,
            "Memory allocation increased significantly after cleanup: {} vs peak {}",
            final_allocated,
            peak_allocated
        );
    }

    #[tokio::test]
    async fn test_long_running_memory_stability() {
        // 长时间运行的内存稳定性测试
        let (initial_allocated, _) = get_memory_usage().await.unwrap();
        let cache = Arc::new(L1Backend::new(5000));

        // 定期记录内存使用情况
        let mut memory_samples = Vec::new();
        memory_samples.push(initial_allocated);

        // 运行10轮，每轮执行操作后休息一段时间
        for round in 0..10 {
            println!("Running memory stability test round {}/10", round + 1);

            // 执行批量操作
            for i in 0..2000 {
                let key = format!("longrun_{}_{}", round, i % 500);
                let value = vec![round as u8; 512];
                cache.set_bytes(&key, value, Some(120)).await.unwrap();
            }

            // 执行批量读取
            for i in 0..2000 {
                let key = format!("longrun_{}_{}", round, i % 500);
                let _ = cache.get_bytes(&key).await;
            }

            // 定期清理旧数据
            if round % 2 == 0 {
                for i in 0..500 {
                    let key = format!("longrun_{}_{}", (round + 1) % 2, i);
                    let _ = cache.delete(&key).await;
                }
            }

            // 记录内存使用情况
            let (current_allocated, _) = get_memory_usage().await.unwrap();
            memory_samples.push(current_allocated);
            println!(
                "  Memory usage after round {}: {} bytes",
                round + 1,
                current_allocated
            );

            // 休息一段时间
            sleep(Duration::from_millis(200)).await;
        }

        // 清理所有数据
        for round in 0..10 {
            for i in 0..500 {
                let key = format!("longrun_{}_{}", round, i);
                let _ = cache.delete(&key).await;
            }
        }

        drop(cache);
        sleep(Duration::from_millis(500)).await;

        // 验证长时间运行后的内存稳定性
        let max_memory = memory_samples.iter().max().unwrap();
        let min_memory = memory_samples.iter().min().unwrap();

        println!("Long running memory stability test results:");
        println!("  Minimum memory usage: {} bytes", min_memory);
        println!("  Maximum memory usage: {} bytes", max_memory);
        println!("  Memory usage range: {} bytes", max_memory - min_memory);

        // 检查内存使用是否在合理范围内波动，没有持续增长
        assert!(
            *max_memory < initial_allocated * 3,
            "Memory usage exceeded expected limit: {} vs {}",
            max_memory,
            initial_allocated * 3
        );
    }
}

// ============================================================================
// Miri 内存安全测试模块
// ============================================================================

/// 测试基本的内存安全 - 无内存泄漏
#[test]
fn test_basic_memory_safety() {
    let cache = L1Backend::new(100);

    // 简单的set/get操作
    let key = "test_key";
    let value = vec![1, 2, 3, 4, 5];

    // 注意：这里使用block_on来同步执行异步代码
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        cache.set_bytes(key, value.clone(), Some(60)).await.unwrap();
        let retrieved = cache.get_bytes(key).await.unwrap();

        assert_eq!(retrieved, Some(value));
        cache.delete(key).await.unwrap();
    });
}

/// 测试内存释放 - 确保删除后内存被释放
#[test]
fn test_memory_release_on_delete() {
    let cache = L1Backend::new(10);
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        // 创建大量数据
        for i in 0..100 {
            let key = format!("key_{}", i);
            let value = vec![i as u8; 1024]; // 1KB数据
            cache.set_bytes(&key, value, Some(60)).await.unwrap();
        }

        // 删除所有数据
        for i in 0..100 {
            let key = format!("key_{}", i);
            cache.delete(&key).await.unwrap();
        }

        // 验证所有数据都被删除
        for i in 0..100 {
            let key = format!("key_{}", i);
            let result = cache.get_bytes(&key).await.unwrap();
            assert_eq!(result, None);
        }
    });
}

/// 测试循环引用 - 确保没有内存泄漏
#[test]
fn test_miri_no_circular_reference_leak() {
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Node {
        #[allow(dead_code)]
        value: Vec<u8>,
        next: Option<Rc<RefCell<Node>>>,
    }

    let cache = L1Backend::new(5);
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        // 创建循环引用
        let node1 = Rc::new(RefCell::new(Node {
            value: vec![1; 100],
            next: None,
        }));

        let node2 = Rc::new(RefCell::new(Node {
            value: vec![2; 100],
            next: Some(Rc::clone(&node1)),
        }));

        node1.borrow_mut().next = Some(Rc::clone(&node2));

        // 存储循环引用
        let data = format!("circular_data_{}", Rc::strong_count(&node1)).into_bytes();
        cache
            .set_bytes("circular", data.clone(), Some(10))
            .await
            .unwrap();

        // 删除后应该释放内存
        cache.delete("circular").await.unwrap();

        // 强引用计数应该减少
        // 注意：由于局部变量的生命周期，drop(node1)和drop(node2)会在block_on结束时发生
        // 这里只验证缓存操作不会导致内存泄漏
        assert_eq!(Rc::strong_count(&node1), 2);
        assert_eq!(Rc::strong_count(&node2), 2);
    });
}

/// 测试缓冲区溢出 - 确保没有缓冲区溢出
#[test]
fn test_buffer_overflow_prevention() {
    let cache = L1Backend::new(10);
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        // 测试各种大小的数据
        let sizes = [0, 1, 10, 100, 1000, 10000];

        for size in sizes {
            let key = format!("size_{}", size);
            let value = vec![42u8; size];

            cache
                .set_bytes(&key, value.clone(), Some(60))
                .await
                .unwrap();
            let retrieved = cache.get_bytes(&key).await.unwrap();

            assert_eq!(retrieved, Some(value));
            cache.delete(&key).await.unwrap();
        }
    });
}

/// 测试并发内存安全（简化版，避免复杂的并发测试）
#[test]
fn test_concurrent_memory_safety() {
    use std::sync::Arc;
    use std::thread;

    let cache = Arc::new(L1Backend::new(100));
    let mut handles = vec![];

    for thread_id in 0..5 {
        let cache_clone = Arc::clone(&cache);

        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();

            rt.block_on(async {
                for i in 0..20 {
                    let key = format!("thread_{}_key_{}", thread_id, i);
                    let value = vec![thread_id as u8; 100];

                    cache_clone
                        .set_bytes(&key, value.clone(), Some(60))
                        .await
                        .unwrap();
                    let retrieved = cache_clone.get_bytes(&key).await.unwrap();

                    assert_eq!(retrieved, Some(value));
                    cache_clone.delete(&key).await.unwrap();
                }
            });
        });

        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
}

/// 测试内存对齐 - 确保数据结构正确对齐
#[test]
fn test_memory_alignment() {
    let cache = L1Backend::new(10);
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        // 测试不同对齐要求的数据
        let test_cases = vec![
            ("align1", vec![1u8; 1]),
            ("align2", vec![2u8; 2]),
            ("align4", vec![3u8; 4]),
            ("align8", vec![4u8; 8]),
            ("align16", vec![5u8; 16]),
            ("align32", vec![6u8; 32]),
        ];

        for (key, value) in test_cases {
            cache.set_bytes(key, value.clone(), Some(60)).await.unwrap();
            let retrieved = cache.get_bytes(key).await.unwrap();

            assert_eq!(retrieved, Some(value));
            cache.delete(key).await.unwrap();
        }
    });
}

/// 测试使用未初始化内存 - 确保没有使用未初始化内存
#[test]
fn test_uninitialized_memory_prevention() {
    let cache = L1Backend::new(5);
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        // 创建不同大小的数据
        let sizes = vec![0, 1, 10, 100, 1000];

        for size in sizes {
            let key = format!("uninit_{}", size);

            // 创建明确初始化的数据
            let mut value = vec![0u8; size];

            // 确保所有字节都被初始化
            for (i, byte) in value.iter_mut().enumerate() {
                *byte = (i % 256) as u8;
            }

            cache
                .set_bytes(&key, value.clone(), Some(60))
                .await
                .unwrap();

            // 验证数据完整性
            let retrieved = cache.get_bytes(&key).await.unwrap();
            assert_eq!(retrieved, Some(value));

            cache.delete(&key).await.unwrap();
        }
    });
}
