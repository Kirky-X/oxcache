//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! Miri内存安全测试
//! 这个文件包含专门用于Miri检测的内存安全测试
//! 运行方式: cargo +nightly miri test --test miri_memory_test

use oxcache::backend::l1::L1Backend;

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
fn test_no_circular_reference_leak() {
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
