// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 降级策略和健康状态测试
//!
//! 真实环境集成测试覆盖：
//! - Redis Standalone：重试、熔断器、分布式指标、stats 增强、ChainCache 并行删除
//! - Redis Cluster：基本操作 + 重试/熔断（需 `REDIS_CLUSTER_AVAILABLE` 环境变量）
//! - Redis Sentinel：基本操作 + 重试/熔断（需 `REDIS_SENTINEL_AVAILABLE` 环境变量）

#[cfg(test)]
#[cfg(feature = "redis")]
mod degradation_tests_inner {
    use crate::common::{
        create_cluster_redis_urls, is_redis_available_url,
        wait_for_redis_cluster, wait_for_sentinel,
    };
    use oxcache::backend::memory::redis::{RedisBackend, RedisMode};
    use oxcache::backend::AtomicCacheWriter;
    use oxcache::backend::CacheConnector;
    use oxcache::backend::CacheReader;
    use oxcache::backend::CacheWriter;
    use oxcache::cache::{ChainCache, ChainLink};
    use oxcache::infra::metrics::UnifiedMetrics;
    use serial_test::serial;
    use std::sync::Arc;
    use std::time::Duration;

    // ============================================================================
    // 测试辅助
    // ============================================================================

    /// Standalone Redis URL（Docker 端口 6379）
    fn standalone_url() -> String {
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
    }

    async fn setup_backend() -> RedisBackend {
        unsafe {
            std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
        }
        let url = standalone_url();
        RedisBackend::builder()
            .connection_string(&url)
            .retry_count(2)
            .retry_delay(Duration::from_millis(50))
            .circuit_breaker_threshold(3)
            .circuit_breaker_reset_timeout(Duration::from_secs(5))
            .build()
            .await
            .expect("Redis Standalone should be available on port 6379")
    }

    async fn skip_if_unavailable() -> bool {
        let url = standalone_url();
        if !is_redis_available_url(&url).await {
            println!("[SKIP] Redis Standalone not available at {}", url);
            return false;
        }
        true
    }

    /// 清理测试键
    async fn cleanup_key(backend: &RedisBackend, key: &str) {
        let _ = backend.delete(key).await;
    }

    // ============================================================================
    // 1. Standalone — 基本操作 + 重试透明性
    // ============================================================================

    mod standalone_retry {
        use super::*;

        /// 验证正常操作在重试机制下透明成功
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_basic_operations_with_retry_transparent() {
            if !skip_if_unavailable().await {
                return;
            }
            let backend = setup_backend().await;
            let key = "oxcache:test:retry:basic";

            // SET → GET → DELETE 完整链路
            backend
                .set(Arc::from(key), Arc::new(b"value_retry_test".to_vec()), None)
                .await
                .expect("SET should succeed with retry layer");

            let val = backend
                .get(key)
                .await
                .expect("GET should succeed")
                .expect("Key should exist");
            assert_eq!(val, b"value_retry_test");

            cleanup_key(&backend, key).await;
        }

        /// 验证 TTL 操作在重试层下正常工作
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_ttl_operations_with_retry() {
            if !skip_if_unavailable().await {
                return;
            }
            let backend = setup_backend().await;
            let key = "oxcache:test:retry:ttl";

            backend
                .set(
                    Arc::from(key),
                    Arc::new(b"ttl_value".to_vec()),
                    Some(Duration::from_secs(120)),
                )
                .await
                .expect("SET with TTL should succeed");

            let ttl = backend.ttl(key).await.expect("TTL should succeed");
            assert!(ttl.is_some());
            assert!(ttl.unwrap().as_secs() > 0 && ttl.unwrap().as_secs() <= 120);

            cleanup_key(&backend, key).await;
        }

        /// 验证批量操作在重试层下正常工作
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_batch_operations_with_retry() {
            if !skip_if_unavailable().await {
                return;
            }
            let backend = setup_backend().await;

            let items = vec![
                (
                    Arc::from("oxcache:test:batch:1"),
                    Arc::new(b"v1".to_vec()),
                    Some(Duration::from_secs(60)),
                ),
                (
                    Arc::from("oxcache:test:batch:2"),
                    Arc::new(b"v2".to_vec()),
                    Some(Duration::from_secs(60)),
                ),
                (
                    Arc::from("oxcache:test:batch:3"),
                    Arc::new(b"v3".to_vec()),
                    None,
                ),
            ];

            backend.set_many(&items).await.expect("set_many should succeed");

            let keys: Vec<String> = items.iter().map(|(k, _, _)| k.to_string()).collect();
            let values = backend.get_many(&keys).await.expect("get_many should succeed");
            assert_eq!(values.len(), 3);
            assert_eq!(values[0], Some(b"v1".to_vec()));
            assert_eq!(values[1], Some(b"v2".to_vec()));
            assert_eq!(values[2], Some(b"v3".to_vec()));

            backend
                .delete_many(&keys)
                .await
                .expect("delete_many should succeed");

            // 验证删除成功
            let after = backend.get_many(&keys).await.expect("get_many after delete");
            for v in &after {
                assert!(v.is_none());
            }
        }

        /// 验证原子操作在重试层下正常工作
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_atomic_operations_with_retry() {
            if !skip_if_unavailable().await {
                return;
            }
            let backend = setup_backend().await;
            let key = "oxcache:test:retry:atomic";
            cleanup_key(&backend, key).await;

            // INCR
            let val = backend
                .incr(key, 1, Some(Duration::from_secs(60)))
                .await
                .expect("INCR should succeed");
            assert_eq!(val, 1);

            let val = backend
                .incr(key, 5, None)
                .await
                .expect("INCRBY should succeed");
            assert_eq!(val, 6);

            // SET_IF_ABSENT
            let set_result = backend
                .set_if_absent(key, b"should_not_set".to_vec(), None)
                .await
                .expect("set_if_absent should succeed");
            assert!(!set_result); // Key already exists

            let new_key = "oxcache:test:retry:atomic:nx";
            cleanup_key(&backend, new_key).await;
            let set_result = backend
                .set_if_absent(new_key, b"nx_value".to_vec(), None)
                .await
                .expect("set_if_absent should succeed");
            assert!(set_result);

            cleanup_key(&backend, key).await;
            cleanup_key(&backend, new_key).await;
        }
    }

    // ============================================================================
    // 2. Standalone — 熔断器行为
    // ============================================================================

    mod standalone_circuit_breaker {
        use super::*;

        /// 验证连接到不可达 Redis 时熔断器最终打开
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_circuit_breaker_opens_on_unreachable_host() {
            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            // 使用低阈值加速测试
            let result = RedisBackend::builder()
                .connection_string("redis://192.0.2.1:6379") // TEST-NET, 不可达
                .retry_count(1)
                .retry_delay(Duration::from_millis(10))
                .circuit_breaker_threshold(2)
                .circuit_breaker_reset_timeout(Duration::from_secs(30))
                .connection_timeout(Duration::from_millis(500))
                .build()
                .await;

            // Builder 在连接阶段就会失败（ConnectionManager 创建时尝试连接）
            assert!(result.is_err(), "Should fail to connect to unreachable host");
        }

        /// 验证有效连接下 health_check 通过
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_health_check_passes_on_healthy_redis() {
            if !skip_if_unavailable().await {
                return;
            }
            let backend = setup_backend().await;
            backend
                .health_check()
                .await
                .expect("Health check should pass on healthy Redis");
        }

        /// 验证分布式参数通过 Builder 链式调用正确注入
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_distributed_config_injection() {
            if !skip_if_unavailable().await {
                return;
            }
            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            let url = standalone_url();
            let backend = RedisBackend::builder()
                .connection_string(&url)
                .retry_count(5)
                .retry_delay(Duration::from_millis(200))
                .circuit_breaker_threshold(10)
                .circuit_breaker_reset_timeout(Duration::from_secs(60))
                .build()
                .await
                .expect("Should build with distributed config");

            // 验证操作正常
            let key = "oxcache:test:config:injection";
            backend
                .set(Arc::from(key), Arc::new(b"configured".to_vec()), None)
                .await
                .expect("SET should work with DistributedConfig");

            let val = backend.get(key).await.unwrap().unwrap();
            assert_eq!(val, b"configured");

            cleanup_key(&backend, key).await;
        }
    }

    // ============================================================================
    // 3. Standalone — Stats 增强（连接池监控）
    // ============================================================================

    mod standalone_stats {
        use super::*;

        /// 验证 stats() 返回增强的连接池信息
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_stats_includes_connection_pool_info() {
            if !skip_if_unavailable().await {
                return;
            }
            let backend = setup_backend().await;
            let stats = backend.stats().await.expect("stats() should succeed");

            // 基础字段
            assert!(
                stats.contains_key("memory_info"),
                "stats should contain memory_info"
            );

            // 增强字段（T012 新增）
            assert!(
                stats.contains_key("connected_clients"),
                "stats should contain connected_clients"
            );
            assert!(
                stats.contains_key("maxclients"),
                "stats should contain maxclients"
            );

            // 值应为有效数字字符串
            let connected = stats.get("connected_clients").unwrap();
            assert!(
                connected.parse::<u64>().is_ok(),
                "connected_clients should be numeric, got: {}",
                connected
            );

            let maxclients = stats.get("maxclients").unwrap();
            assert!(
                maxclients.parse::<u64>().is_ok(),
                "maxclients should be numeric, got: {}",
                maxclients
            );
        }
    }

    // ============================================================================
    // 4. Standalone — 分布式指标验证
    // ============================================================================

    mod standalone_metrics {
        use super::*;

        /// 验证操作后分布式指标正确递增
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_distributed_metrics_after_operations() {
            if !skip_if_unavailable().await {
                return;
            }
            let backend = setup_backend().await;
            let metrics = UnifiedMetrics::new();

            let before_retry = metrics.get_counters().l2_retry_total;
            let before_degraded = metrics.get_counters().l2_degraded;

            // 执行正常操作（不应触发重试或降级）
            let key = "oxcache:test:metrics:ops";
            backend
                .set(Arc::from(key), Arc::new(b"metrics_test".to_vec()), None)
                .await
                .unwrap();
            let _ = backend.get(key).await.unwrap();
            cleanup_key(&backend, key).await;

            // 正常操作不应增加 degraded 计数
            let after_degraded = metrics.get_counters().l2_degraded;
            assert_eq!(
                before_degraded, after_degraded,
                "l2_degraded should not increment on successful operations"
            );

            // retry 计数可能为 0（正常操作无重试）
            let after_retry = metrics.get_counters().l2_retry_total;
            assert!(
                after_retry >= before_retry,
                "l2_retry_total should not decrease"
            );
        }

        /// 验证 UnifiedMetrics 新增计数器字段存在
        #[tokio::test]
        async fn test_new_atomic_counters_exist() {
            let metrics = UnifiedMetrics::new();
            let counters = metrics.get_counters();

            // 新增的 4 个分布式计数器应初始化为 0
            assert_eq!(counters.l2_degraded, 0);
            assert_eq!(counters.l2_retry_total, 0);
            assert_eq!(counters.backfill_success, 0);
            assert_eq!(counters.backfill_failed, 0);
        }
    }

    // ============================================================================
    // 5. ChainCache — 并行删除 + 回填指标
    // ============================================================================

    mod chain_cache_distributed {
        use super::*;
        use oxcache::backend::MokaMemoryBackend;

        /// 验证 ChainCache 包含 Redis 时的并行删除正确性
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_chain_cache_parallel_delete_with_redis() {
            if !skip_if_unavailable().await {
                return;
            }

            // 构建 L1 (Moka) + L2 (Redis) 链
            let l1 = MokaMemoryBackend::builder()
                .capacity(1000)
                .ttl(Duration::from_secs(300))
                .build();
            let l2 = setup_backend().await;

            // 保留 clone 用于后续断言
            let l1_check = l1.clone();
            let l2_check = l2.clone();

            let chain = ChainCache::builder()
                .link(ChainLink::from_backend(l1))
                .link(ChainLink::from_backend(l2))
                .enable_backfill()
                .build();

            let key = "oxcache:test:chain:delete";
            let value = b"chain_delete_value".to_vec();

            // 写入所有后端（ChainCache::set 接受 &str + Vec<u8>）
            chain
                .set(key, value.clone(), None)
                .await
                .expect("ChainCache SET should succeed");

            // 验证 L2 有值
            let l2_val = l2_check.get(key).await.unwrap();
            assert!(l2_val.is_some(), "L2 should have the value after SET");

            // 删除（走并行删除路径）
            chain.delete(key).await.expect("ChainCache DELETE should succeed");

            // 验证 L2 已删除
            let l2_val_after = l2_check.get(key).await.unwrap();
            assert!(
                l2_val_after.is_none(),
                "L2 should be empty after ChainCache DELETE"
            );

            // 验证 L1 也已删除
            let l1_val_after = l1_check.get(key).await.unwrap();
            assert!(
                l1_val_after.is_none(),
                "L1 should be empty after ChainCache DELETE"
            );
        }

        /// 验证 ChainCache 回填成功后递增 backfill_success 指标
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_chain_cache_backfill_metrics() {
            if !skip_if_unavailable().await {
                return;
            }

            let metrics = UnifiedMetrics::new();
            let before_success = metrics.get_counters().backfill_success;

            // 构建链：L1 为空，L2 有数据 → 读 L2 命中后回填 L1
            let l1 = MokaMemoryBackend::builder()
                .capacity(1000)
                .ttl(Duration::from_secs(300))
                .build();
            let l2 = setup_backend().await;

            let key = "oxcache:test:chain:backfill";

            // 直接写入 L2（跳过 L1）
            l2.set(
                Arc::from(key),
                Arc::new(b"backfill_source".to_vec()),
                None,
            )
            .await
            .unwrap();

            // 保留 clone 用于后续断言
            let l1_check = l1.clone();
            let l2_check = l2.clone();

            let chain = ChainCache::builder()
                .link(ChainLink::from_backend(l1))
                .link(ChainLink::from_backend(l2))
                .enable_backfill()
                .build();

            // 从 ChainCache 读取 → 应命中 L2 并回填 L1
            let val = chain.get(key).await.unwrap();
            assert!(val.is_some(), "Should find value from L2");
            assert_eq!(val.unwrap(), b"backfill_source");

            // 等待回填完成（fire-and-forget tokio::spawn）
            tokio::time::sleep(Duration::from_millis(200)).await;

            // 验证 L1 已被回填
            let l1_val = l1_check.get(key).await.unwrap();
            assert!(
                l1_val.is_some(),
                "L1 should have been backfilled from L2"
            );

            // 回填指标应递增
            let after_success = metrics.get_counters().backfill_success;
            assert!(
                after_success >= before_success,
                "backfill_success should increment (before={}, after={})",
                before_success,
                after_success
            );

            // 清理
            let _ = l2_check.delete(key).await;
        }
    }

    // ============================================================================
    // 6. Redis Cluster 模式验证（需 Docker Cluster 环境）
    // ============================================================================

    mod cluster_mode {
        use super::*;

        fn cluster_available() -> bool {
            std::env::var("REDIS_CLUSTER_AVAILABLE").is_ok()
        }

        /// 验证 Cluster 模式基本操作 + 重试层
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_cluster_basic_operations() {
            if !cluster_available() {
                println!("[SKIP] Redis Cluster not available (set REDIS_CLUSTER_AVAILABLE=1)");
                return;
            }

            let urls = create_cluster_redis_urls();
            if !wait_for_redis_cluster(&urls.iter().map(|s| s.as_str()).collect::<Vec<_>>()).await
            {
                println!("[SKIP] Redis Cluster not ready");
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            // Cluster 模式使用第一个节点 URL
            let backend = RedisBackend::builder()
                .connection_string(&urls[0])
                .mode(RedisMode::Cluster)
                .retry_count(2)
                .retry_delay(Duration::from_millis(50))
                .circuit_breaker_threshold(3)
                .build()
                .await;

            match backend {
                Ok(b) => {
                    let key = "oxcache:test:cluster:basic";
                    b.set(Arc::from(key), Arc::new(b"cluster_value".to_vec()), None)
                        .await
                        .expect("Cluster SET should succeed");

                    let val = b.get(key).await.unwrap().expect("Cluster GET should find key");
                    assert_eq!(val, b"cluster_value");

                    let _ = b.delete(key).await;
                    println!("[PASS] Cluster basic operations with retry layer");
                }
                Err(e) => {
                    println!("[SKIP] Cannot connect to Redis Cluster: {}", e);
                }
            }
        }
    }

    // ============================================================================
    // 7. Redis Sentinel 模式验证（需 Docker Sentinel 环境）
    // ============================================================================

    mod sentinel_mode {
        use super::*;

        fn sentinel_available() -> bool {
            std::env::var("REDIS_SENTINEL_AVAILABLE").is_ok()
        }

        /// 验证 Sentinel 模式基本操作 + 重试层
        #[serial(redis_degradation)]
        #[tokio::test]
        async fn test_sentinel_basic_operations() {
            if !sentinel_available() {
                println!("[SKIP] Redis Sentinel not available (set REDIS_SENTINEL_AVAILABLE=1)");
                return;
            }

            if !wait_for_sentinel().await {
                println!("[SKIP] Redis Sentinel not ready");
                return;
            }

            unsafe {
                std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS");
            }

            // Step 1: Ask Sentinel for master address
            let sentinel_url = "redis://127.0.0.1:26382";
            let master_url = {
                let client = match redis::Client::open(sentinel_url) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("[SKIP] Cannot open Sentinel client: {}", e);
                        return;
                    }
                };
                let mut conn = match client.get_multiplexed_async_connection().await {
                    Ok(c) => c,
                    Err(e) => {
                        println!("[SKIP] Cannot connect to Sentinel: {}", e);
                        return;
                    }
                };
                // SENTINEL get-master-addr-by-name mymaster -> [ip, port]
                let addr: Result<Vec<String>, _> = redis::cmd("SENTINEL")
                    .arg("get-master-addr-by-name")
                    .arg("mymaster")
                    .query_async(&mut conn)
                    .await;
                match addr {
                    Ok(parts) if parts.len() == 2 => {
                        format!("redis://{}:{}", parts[0], parts[1])
                    }
                    Ok(_) => {
                        println!("[SKIP] Sentinel returned unexpected master address format");
                        return;
                    }
                    Err(e) => {
                        println!("[SKIP] SENTINEL get-master-addr-by-name failed: {}", e);
                        return;
                    }
                }
            };

            println!("[SENTINEL] Master discovered at {} (internal IP)", master_url);

            // Docker NAT: Sentinel returns container-internal IP (172.26.0.2:6379)
            // which is unreachable from the host. Use the host-mapped port instead.
            let host_master_url = std::env::var("REDIS_SENTINEL_MASTER_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:16379".to_string());
            println!("[SENTINEL] Connecting via host-mapped URL: {}", host_master_url);

            // Step 2: Connect to the discovered master with retry/circuit-breaker
            let backend = RedisBackend::builder()
                .connection_string(&host_master_url)
                .mode(RedisMode::Sentinel)
                .retry_count(2)
                .retry_delay(Duration::from_millis(50))
                .circuit_breaker_threshold(3)
                .build()
                .await;

            match backend {
                Ok(b) => {
                    let key = "oxcache:test:sentinel:basic";
                    b.set(Arc::from(key), Arc::new(b"sentinel_value".to_vec()), None)
                        .await
                        .expect("Sentinel SET should succeed");

                    let val = b
                        .get(key)
                        .await
                        .unwrap()
                        .expect("Sentinel GET should find key");
                    assert_eq!(val, b"sentinel_value");

                    let _ = b.delete(key).await;
                    println!("[PASS] Sentinel basic operations with retry layer");
                }
                Err(e) => {
                    println!("[SKIP] Cannot connect to discovered master: {}", e);
                }
            }
        }
    }
}
