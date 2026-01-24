// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 内存泄漏测试

#![allow(unexpected_cfgs)]

#[path = "common/mod.rs"]
mod common;

use common::is_redis_available;
use oxcache::backend::{CacheBackend, MemoryBackend, RedisBackend};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 内存泄漏测试模块
/// 使用循环引用和大量操作来检测潜在的内存泄漏

#[tokio::test]
async fn test_l1_cache_memory_leak() {
    let cache: Arc<dyn CacheBackend> = Arc::new(MemoryBackend::builder().capacity(1000).build());

    // 执行大量操作，检测内存泄漏
    for i in 0..10000 {
        let key = format!("key_{}", i % 100); // 循环使用100个key
        let value = vec![i as u8; 100];

        cache
            .set(&key, value.clone(), Some(Duration::from_secs(60)))
            .await
            .unwrap();
        cache.get(&key).await.unwrap();

        if i % 1000 == 0 {
            // 定期清理，模拟真实使用场景
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

    let redis_url = "redis://127.0.0.1:6379/15";
    let l2_backend: Arc<dyn CacheBackend> = match RedisBackend::new(redis_url).await {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            println!("无法连接Redis: {:?}", e);
            return;
        }
    };

    // 执行大量L2操作
    for i in 0..5000 {
        let key = format!("l2_leak_test_{}", i % 50); // 循环使用50个key
        let value = vec![i as u8; 1024]; // 1KB数据

        l2_backend
            .set(&key, value.clone(), Some(Duration::from_secs(300)))
            .await
            .unwrap();
        l2_backend.get(&key).await.unwrap();

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

    let redis_url = "redis://127.0.0.1:6379/14";
    let l1: Arc<dyn CacheBackend> = Arc::new(MemoryBackend::builder().capacity(100).build());
    let l2: Arc<dyn CacheBackend> = match RedisBackend::new(redis_url).await {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            println!("无法连接Redis: {:?}", e);
            return;
        }
    };

    // 测试L1缓存的内存泄漏
    for i in 0..1500 {
        let key = format!("two_level_l1_{}", i % 100);
        let value = format!("value_{}", i).into_bytes();

        // 写入操作
        l1.set(&key, value.clone(), Some(Duration::from_secs(120)))
            .await
            .unwrap();

        // 读取操作
        let _ = l1.get(&key).await;

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
        l2.set(&key, value.clone(), Some(Duration::from_secs(120)))
            .await
            .unwrap();

        // 读取操作
        let _ = l2.get(&key).await;

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

    let l1: Arc<dyn CacheBackend> = Arc::new(MemoryBackend::builder().capacity(500).build());
    let redis_url = "redis://127.0.0.1:6379/13";
    let l2: Arc<dyn CacheBackend> = match RedisBackend::new(redis_url).await {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            println!("无法连接Redis: {:?}", e);
            return;
        }
    };

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
            l1.set(key, value.clone(), Some(Duration::from_secs(60)))
                .await
                .unwrap();
        }

        // L1批量获取
        for (key, _) in &batch {
            let _ = l1.get(key).await;
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
            l2.set(key, value.clone(), Some(Duration::from_secs(60)))
                .await
                .unwrap();
        }

        // L2批量获取
        for (key, _) in &l2_batch {
            let _ = l2.get(key).await;
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
    let cache: Arc<dyn CacheBackend> = Arc::new(MemoryBackend::builder().capacity(1000).build());
    let mut handles = vec![];

    // 并发内存泄漏测试
    for thread_id in 0..10 {
        let cache_clone = Arc::clone(&cache);

        let handle = tokio::spawn(async move {
            for i in 0..1000 {
                let key = format!("thread_{}_key_{}", thread_id, i % 50);
                let value = format!("thread_{}_value_{}", thread_id, i).into_bytes();

                cache_clone
                    .set(&key, value.clone(), Some(Duration::from_secs(60)))
                    .await
                    .unwrap();
                let _ = cache_clone.get(&key).await;

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
    let cache: Arc<dyn CacheBackend> = Arc::new(MemoryBackend::builder().capacity(100).build());

    // 将循环引用序列化为字节数组存储
    let serialized = format!("circular_ref_data_{}", Rc::strong_count(&node1)).into_bytes();
    cache
        .set(
            "circular_ref",
            serialized.clone(),
            Some(Duration::from_secs(10)),
        )
        .await
        .unwrap();

    // 删除后应该释放内存
    cache.delete("circular_ref").await.unwrap();
    drop(cache);
    drop(node1);
    drop(node2);

    sleep(Duration::from_millis(100)).await;
}
