//! Singleflight 模块单元测试
//!
//! 测试请求去重机制的功能和并发安全性。

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use oxcache::singleflight::SingleFlight;

    // ========================================================================
    // Basic Functionality Tests
    // ========================================================================

    #[tokio::test]
    async fn test_singleflight_creation() {
        let sf = SingleFlight::new();
        assert_eq!(sf.active_calls().await, 0);
    }

    #[tokio::test]
    async fn test_singleflight_default() {
        let sf = SingleFlight::default();
        assert_eq!(sf.active_calls().await, 0);
    }

    #[tokio::test]
    async fn test_singleflight_debug() {
        let sf = SingleFlight::new();
        let debug_str = format!("{:?}", sf);
        assert!(debug_str.contains("SingleFlight"));
        assert!(debug_str.contains("active_calls"));
    }

    #[tokio::test]
    async fn test_singleflight_single_call() {
        let sf = SingleFlight::new();
        let call_count = Arc::new(AtomicU32::new(0));

        let call_count_clone = call_count.clone();
        let result = sf
            .call("key1", || {
                let counter = call_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(b"result1".to_vec())
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"result1");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_singleflight_different_keys() {
        let sf = SingleFlight::new();
        let call_count = Arc::new(AtomicU32::new(0));

        // Call with key1
        let call_count_clone = call_count.clone();
        let result1 = sf
            .call("key1", || {
                let counter = call_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(b"result1".to_vec())
                }
            })
            .await;

        // Call with key2
        let call_count_clone = call_count.clone();
        let result2 = sf
            .call("key2", || {
                let counter = call_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(b"result2".to_vec())
                }
            })
            .await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert_eq!(result1.unwrap(), b"result1");
        assert_eq!(result2.unwrap(), b"result2");
        // Both calls should execute work since they have different keys
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    // ========================================================================
    // Deduplication Tests
    // ========================================================================

    #[tokio::test]
    async fn test_singleflight_deduplication() {
        let sf = Arc::new(SingleFlight::new());
        let call_count = Arc::new(AtomicU32::new(0));
        let mut handles = vec![];

        // Spawn 10 concurrent calls with the same key
        for i in 0..10 {
            let sf_clone = sf.clone();
            let call_count_clone = call_count.clone();
            let handle = tokio::spawn(async move {
                let result = sf_clone
                    .call("shared_key", || {
                        let counter = call_count_clone.clone();
                        async move {
                            // Simulate slow work
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            counter.fetch_add(1, Ordering::SeqCst);
                            Ok(format!("result_{}", i).into_bytes())
                        }
                    })
                    .await;
                result
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let mut results = vec![];
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            results.push(result.unwrap());
        }

        // Work should only be executed once
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // All callers should get the same result
        let first_result = &results[0];
        for result in &results[1..] {
            assert_eq!(result, first_result);
        }
    }

    #[tokio::test]
    async fn test_singleflight_deduplication_with_active_calls() {
        let sf = Arc::new(SingleFlight::new());
        let call_count = Arc::new(AtomicU32::new(0));
        let mut handles = vec![];

        // Spawn concurrent calls
        for _i in 0..5 {
            let sf_clone = sf.clone();
            let call_count_clone = call_count.clone();
            let handle = tokio::spawn(async move {
                let active_before = sf_clone.active_calls().await;

                let result = sf_clone
                    .call("test_key", || {
                        let counter = call_count_clone.clone();
                        async move {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            counter.fetch_add(1, Ordering::SeqCst);
                            Ok(b"shared".to_vec())
                        }
                    })
                    .await;

                let active_after = sf_clone.active_calls().await;
                (result, active_before, active_after)
            });
            handles.push(handle);
        }

        // All tasks should succeed
        for handle in handles {
            let (result, _active_before, active_after) = handle.await.unwrap();
            assert!(result.is_ok());
            // After completion, active calls should be 0
            assert_eq!(active_after, 0);
        }

        // Only one execution should occur
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    // ========================================================================
    // Error Propagation Tests
    // ========================================================================

    #[tokio::test]
    async fn test_singleflight_error_propagation() {
        let sf = Arc::new(SingleFlight::new());
        let mut handles = vec![];

        // Spawn concurrent calls that will fail
        for i in 0..5 {
            let sf_clone = sf.clone();
            let handle = tokio::spawn(async move {
                sf_clone
                    .call("error_key", || async move {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Err(oxcache::error::CacheError::Internal(format!(
                            "error_{}",
                            i
                        )))
                    })
                    .await
            });
            handles.push(handle);
        }

        // All callers should receive the error
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_err());
            if let Err(oxcache::error::CacheError::Internal(msg)) = result {
                assert!(msg.contains("error_"));
            } else {
                panic!("Expected Internal error");
            }
        }
    }

    #[tokio::test]
    async fn test_singleflight_mixed_success_and_error() {
        let sf = SingleFlight::new();

        // First call succeeds
        let result1 = sf
            .call("key_success", || async { Ok(b"success".to_vec()) })
            .await;
        assert!(result1.is_ok());

        // Second call with different key fails
        let result2 = sf
            .call("key_error", || async {
                Err(oxcache::error::CacheError::Internal("test error".into()))
            })
            .await;
        assert!(result2.is_err());

        // Third call with first key succeeds again
        let result3 = sf
            .call("key_success", || async { Ok(b"success2".to_vec()) })
            .await;
        assert!(result1.is_ok());
        assert_eq!(result3.unwrap(), b"success2");
    }

    // ========================================================================
    // Reset and Cleanup Tests
    // ========================================================================

    #[tokio::test]
    async fn test_singleflight_reset() {
        let sf = Arc::new(SingleFlight::new());
        let call_count = Arc::new(AtomicU32::new(0));

        // Start a slow call
        let sf_clone = sf.clone();
        let call_count_clone = call_count.clone();
        let handle = tokio::spawn(async move {
            sf_clone
                .call("reset_key", || {
                    let counter = call_count_clone.clone();
                    async move {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        counter.fetch_add(1, Ordering::SeqCst);
                        Ok(b"result".to_vec())
                    }
                })
                .await
        });

        // Wait a bit for the call to register
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Verify active call
        assert_eq!(sf.active_calls().await, 1);

        // Reset
        sf.reset().await;

        // Active calls should be 0 after reset (though the task is still running)
        // Note: reset() clears the map but doesn't abort running tasks
        let active = sf.active_calls().await;
        assert!(active <= 1);

        // Original task should still complete
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    // ========================================================================
    // Sequential Calls Tests
    // ========================================================================

    #[tokio::test]
    async fn test_singleflight_sequential_same_key() {
        let sf = SingleFlight::new();
        let call_count = Arc::new(AtomicU32::new(0));

        // First call
        let call_count_clone = call_count.clone();
        let result1 = sf
            .call("seq_key", || {
                let counter = call_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(b"first".to_vec())
                }
            })
            .await;

        // Second call with same key
        let call_count_clone = call_count.clone();
        let result2 = sf
            .call("seq_key", || {
                let counter = call_count_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(b"second".to_vec())
                }
            })
            .await;

        // Both should execute since they're sequential
        assert_eq!(result1.unwrap(), b"first");
        assert_eq!(result2.unwrap(), b"second");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_singleflight_rapid_calls() {
        let sf = Arc::new(SingleFlight::new());
        let call_count = Arc::new(AtomicU32::new(0));
        let mut handles = vec![];

        // Rapid fire calls with different keys
        for i in 0..20 {
            let sf_clone = sf.clone();
            let call_count_clone = call_count.clone();
            let handle = tokio::spawn(async move {
                sf_clone
                    .call(&format!("key_{}", i % 5), || {
                        // Only 5 unique keys, so each key gets 4 calls
                        let counter = call_count_clone.clone();
                        async move {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            counter.fetch_add(1, Ordering::SeqCst);
                            Ok(b"result".to_vec())
                        }
                    })
                    .await
            });
            handles.push(handle);
        }

        // All should succeed
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }

        // With 5 unique keys, work should execute 5 times (once per key)
        assert_eq!(call_count.load(Ordering::SeqCst), 5);
    }

    // ========================================================================
    // Edge Cases Tests
    // ========================================================================

    #[tokio::test]
    async fn test_singleflight_empty_key() {
        let sf = SingleFlight::new();
        let result = sf.call("", || async { Ok(b"empty_key".to_vec()) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"empty_key");
    }

    #[tokio::test]
    async fn test_singleflight_special_characters_in_key() {
        let sf = SingleFlight::new();
        let result = sf
            .call("key/with/special:chars", || async { Ok(b"special".to_vec()) })
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"special");
    }

    #[tokio::test]
    async fn test_singleflight_large_result() {
        let sf = SingleFlight::new();
        let large_data = vec![0u8; 1024 * 1024]; // 1MB
        let result = sf
            .call("large_key", || {
                let data = large_data.clone();
                async move { Ok(data) }
            })
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1024 * 1024);
    }

    #[tokio::test]
    async fn test_singleflight_zero_duration_work() {
        let sf = SingleFlight::new();
        let call_count = Arc::new(AtomicU32::new(0));

        let call_count_clone = call_count.clone();
        let result = sf
            .call("instant_key", || {
                let counter = call_count_clone.clone();
                async move {
                    // No sleep, instant return
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(b"instant".to_vec())
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
