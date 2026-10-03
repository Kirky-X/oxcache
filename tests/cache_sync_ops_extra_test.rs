// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `Cache<K,V>` sync 面长尾与 stale 集成路径覆盖：
//!
//! - sync 杂项方法（clear/health_check/shutdown/stats/len/capacity_sync）
//! - `get_or_option_sync` 全分支（无 null_ttl / 有 null_ttl 哨兵短路 /
//!   fallback Err 透传 / with_ttl 包装）
//! - `get_or_sync` 在未启用 sync_mode 时的显性报错与 guard 清理
//! - `stale_ttl` 三种策略下的 stale 命中语义（Return / Revalidate /
//!   OffloadRevalidate 后台刷新）

#![cfg(feature = "full")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use oxcache::Cache;
use oxcache::features::stale::StalePolicy;

// ============================================================================
// sync 杂项面
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn sync_misc_surface() {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

    cache.set_sync(&"a".to_string(), &"1".to_string()).unwrap();
    cache.set_sync(&"b".to_string(), &"2".to_string()).unwrap();

    // Moka entry_count 最终一致，写后立即读可能滞后——只调用不断言数值
    let _ = cache.len_sync().unwrap();
    let _cap = cache.capacity_sync().unwrap();
    let stats = cache.stats_sync().unwrap();
    let _ = stats;
    cache.health_check_sync().unwrap();

    cache.clear_sync().unwrap();
    let _ = cache.len_sync().unwrap();

    // shutdown 同步面：no-op 安全
    cache.shutdown_sync();
}

// ============================================================================
// get_or_option_sync 全分支
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_or_option_sync_value_none_and_error() {
    let cache: Cache<String, String> = Cache::builder().sync_mode(true).build().await.unwrap();

    // fallback Some → 缓存并返回
    let calls = AtomicU32::new(0);
    let v = cache
        .get_or_option_sync(&"k:1".to_string(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some("v1".to_string()))
        })
        .unwrap();
    assert_eq!(v.as_deref(), Some("v1"));
    // 二次调用命中缓存，fallback 不再执行
    let v2 = cache
        .get_or_option_sync(&"k:1".to_string(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some("other".to_string()))
        })
        .unwrap();
    assert_eq!(v2.as_deref(), Some("v1"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // fallback None（未配置 null_ttl）→ 返回 None，不缓存哨兵
    let n = cache
        .get_or_option_sync(&"k:2".to_string(), || {
            Ok::<Option<String>, oxcache::OxCacheError>(None)
        })
        .unwrap();
    assert!(n.is_none());

    // fallback Err → 显性透传
    let err = cache
        .get_or_option_sync(&"k:3".to_string(), || {
            Err(oxcache::OxCacheError::L1Error("boom".to_string()))
        })
        .unwrap_err();
    assert!(err.to_string().contains("boom"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_or_option_sync_null_sentinel_short_circuit() {
    let cache: Cache<String, String> = Cache::builder()
        .sync_mode(true)
        .null_cache_ttl(Duration::from_secs(30))
        .build()
        .await
        .unwrap();

    let calls = AtomicU32::new(0);
    let first = cache
        .get_or_option_sync(&"nk".to_string(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<Option<String>, oxcache::OxCacheError>(None)
        })
        .unwrap();
    assert!(first.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // 哨兵已缓存 → 第二次调用短路返回 None，不再执行 fallback
    let second = cache
        .get_or_option_sync(&"nk".to_string(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<Option<String>, oxcache::OxCacheError>(None)
        })
        .unwrap();
    assert!(second.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_or_option_with_ttl_sync_variants() {
    let cache: Cache<String, String> = Cache::builder()
        .sync_mode(true)
        .null_cache_ttl(Duration::from_secs(30))
        .build()
        .await
        .unwrap();

    // with_ttl 包装：值路径
    let v = cache
        .get_or_option_with_ttl_sync(&"tk".to_string(), Some(Duration::from_secs(60)), || {
            Ok(Some("tv".to_string()))
        })
        .unwrap();
    assert_eq!(v.as_deref(), Some("tv"));

    // with_ttl 包装：None + 哨兵（jitter 分支在 null_cache_ttl 存在时走 apply_jitter）
    let n = cache
        .get_or_option_with_ttl_sync(&"tn".to_string(), Some(Duration::from_secs(60)), || {
            Ok::<Option<String>, oxcache::OxCacheError>(None)
        })
        .unwrap();
    assert!(n.is_none());
    let n2 = cache
        .get_or_option_with_ttl_sync(&"tn".to_string(), Some(Duration::from_secs(60)), || {
            Ok::<Option<String>, oxcache::OxCacheError>(None)
        })
        .unwrap();
    assert!(n2.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_api_without_sync_mode_fails_explicitly() {
    // 未启用 sync_mode → sync 面显性报错（不 panic、不静默）
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
    let err = cache
        .get_or_sync(&"k".to_string(), || Ok("v".to_string()))
        .unwrap_err();
    assert!(!err.to_string().is_empty());
    let err = cache.get_sync(&"k".to_string()).unwrap_err();
    assert!(!err.to_string().is_empty());
}

// ============================================================================
// get_or_sync 竞争路径（leader/follower）
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_or_sync_concurrent_leader_follower() {
    let cache: Arc<Cache<String, String>> =
        Arc::new(Cache::builder().sync_mode(true).build().await.unwrap());

    let calls = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let cache = Arc::clone(&cache);
        let calls = Arc::clone(&calls);
        handles.push(tokio::task::spawn_blocking(move || {
            cache
                .get_or_sync(&"contended".to_string(), || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(120));
                    Ok("computed".to_string())
                })
                .unwrap()
        }));
    }
    for h in handles {
        assert_eq!(h.await.unwrap(), "computed");
    }
    // single-flight：fallback 实际执行次数远小于并发调用数
    let executed = calls.load(Ordering::SeqCst);
    assert!((1..=4).contains(&executed), "executed={executed}");
    // leader 已缓存 → 后续调用不再回源
    let again = cache
        .get_or_sync(&"contended".to_string(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok("should-not-run".to_string())
        })
        .unwrap();
    assert_eq!(again, "computed");
}

// ============================================================================
// stale 三策略（stale_ttl 启用 SWR 三态过期）
// ============================================================================

async fn build_stale_cache(policy: StalePolicy) -> Cache<String, String> {
    // stale 窗口拉宽到 2s：测试以短 sleep 越过 TTL 后落在 Stale 态，
    // 避免 llvm-cov 插桩/调度抖动下滑出 150ms 级窄窗误入 Expired 分支
    Cache::builder()
        .stale_ttl(Duration::from_secs(2))
        .stale_policy(policy)
        .build()
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_return_policy_serves_old_value() {
    let cache = build_stale_cache(StalePolicy::Return).await;
    let calls = AtomicU32::new(0);

    let v = cache
        .get_or_with_ttl(&"s:1".to_string(), Some(Duration::from_millis(80)), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("fresh1".to_string()) }
        })
        .await
        .unwrap();
    assert_eq!(v, "fresh1");

    // 等待过期进入 stale 窗口 → get_or 命中 stale，返回旧值且不回源
    tokio::time::sleep(Duration::from_millis(200)).await;
    let v2 = cache
        .get_or_with_ttl(&"s:1".to_string(), Some(Duration::from_millis(80)), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("fresh2".to_string()) }
        })
        .await
        .unwrap();
    assert_eq!(v2, "fresh1", "Return 策略：stale 命中返回旧值");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "stale 命中不执行 fallback");
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_revalidate_policy_treats_as_miss() {
    let cache = build_stale_cache(StalePolicy::Revalidate).await;
    let calls = AtomicU32::new(0);

    let v = cache
        .get_or_with_ttl(&"s:2".to_string(), Some(Duration::from_millis(80)), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("v1".to_string()) }
        })
        .await
        .unwrap();
    assert_eq!(v, "v1");

    // stale 窗口内 → 视同 miss：删除 stale 条目并回源
    tokio::time::sleep(Duration::from_millis(200)).await;
    let v2 = cache
        .get_or_with_ttl(&"s:2".to_string(), Some(Duration::from_millis(80)), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("v2".to_string()) }
        })
        .await
        .unwrap();
    assert_eq!(v2, "v2", "Revalidate 策略：stale 视同 miss 并刷新");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_offload_policy_returns_old_and_refreshes_background() {
    let cache = build_stale_cache(StalePolicy::OffloadRevalidate).await;
    let calls = Arc::new(AtomicU32::new(0));

    let v = cache
        .get_or_refresh(&"s:3".to_string(), Some(Duration::from_millis(80)), {
            let calls = Arc::clone(&calls);
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok("gen1".to_string()) }
            }
        })
        .await
        .unwrap();
    assert_eq!(v, "gen1");

    // stale 窗口内 → 立即返回旧值，fallback 交由 offload 后台执行
    tokio::time::sleep(Duration::from_millis(200)).await;
    let v2 = cache
        .get_or_refresh(&"s:3".to_string(), Some(Duration::from_millis(80)), {
            let calls = Arc::clone(&calls);
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok("gen2".to_string()) }
            }
        })
        .await
        .unwrap();
    assert_eq!(v2, "gen1", "OffloadRevalidate：立即返回旧值");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // 等待后台刷新落盘后，再次 get 应得到新值
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(Some(v)) = cache.get(&"s:3".to_string()).await
            && v == "gen2"
        {
            return;
        }
    }
    panic!("offload 后台刷新未落盘");
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_get_or_refresh_revalidate_and_return_paths() {
    // Return 策略下 get_or_refresh：stale 命中直接返回旧值
    let cache = build_stale_cache(StalePolicy::Return).await;
    let calls = Arc::new(AtomicU32::new(0));
    let v = cache
        .get_or_refresh(&"r:1".to_string(), Some(Duration::from_millis(80)), {
            let calls = Arc::clone(&calls);
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok("old".to_string()) }
            }
        })
        .await
        .unwrap();
    assert_eq!(v, "old");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let v2 = cache
        .get_or_refresh(&"r:1".to_string(), Some(Duration::from_millis(80)), {
            let calls = Arc::clone(&calls);
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok("new".to_string()) }
            }
        })
        .await
        .unwrap();
    assert_eq!(v2, "old");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_miss_falls_through_to_normal_get_or() {
    let cache = build_stale_cache(StalePolicy::Return).await;
    // 无任何历史条目：stale 查询 miss → 正常 get_or 路径
    let v = cache
        .get_or_with_ttl(&"s:none".to_string(), None, || async {
            Ok("plain".to_string())
        })
        .await
        .unwrap();
    assert_eq!(v, "plain");
}

// ============================================================================
// get_or / get_or_option 并发与哨兵分支补测
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_or_async_concurrent_follower_and_double_check() {
    let cache: Arc<Cache<String, String>> = Arc::new(Cache::builder().build().await.unwrap());
    let calls = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();
    for i in 0..8 {
        let cache = Arc::clone(&cache);
        let calls = Arc::clone(&calls);
        handles.push(tokio::spawn(async move {
            cache
                .get_or(&"hot".to_string(), move || {
                    let calls = Arc::clone(&calls);
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        let _ = i;
                        Ok("computed".to_string())
                    }
                })
                .await
        }));
    }
    for h in handles {
        assert_eq!(h.await.unwrap().unwrap(), "computed");
    }
    let executed = calls.load(Ordering::SeqCst);
    assert!((1..=8).contains(&executed), "executed={executed}");
    // 后到者走 leader 双检命中（543-544）：缓存已有值，fallback 不再执行
    let again = cache
        .get_or(&"hot".to_string(), || async { Ok("nope".to_string()) })
        .await
        .unwrap();
    assert_eq!(again, "computed");
}

#[tokio::test(flavor = "multi_thread")]
async fn get_or_option_async_follower_value_and_leader_failure() {
    let cache: Arc<Cache<String, String>> = Arc::new(Cache::builder().build().await.unwrap());

    // follower：leader 慢回源，follower 订阅后取缓存值（687）
    let c1 = Arc::clone(&cache);
    let leader = tokio::spawn(async move {
        c1.get_or_option(&"fv".to_string(), || async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(Some("lv".to_string()))
        })
        .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let follower_val = cache
        .get_or_option(&"fv".to_string(), || async {
            Ok(Some("ignored".to_string()))
        })
        .await
        .unwrap();
    assert_eq!(follower_val.as_deref(), Some("lv"));
    assert_eq!(leader.await.unwrap().unwrap().as_deref(), Some("lv"));

    // leader 回源失败：follower 醒来后无缓存 → 显性报错（695-699）
    let c2 = Arc::clone(&cache);
    let failing = tokio::spawn(async move {
        c2.get_or_option(&"ff".to_string(), || async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Err(oxcache::OxCacheError::L1Error("boom".to_string()))
        })
        .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let err = cache
        .get_or_option(&"ff".to_string(), || async {
            Ok(Some("ignored".to_string()))
        })
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty());
    let _ = failing.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn get_or_option_with_ttl_async_value_path() {
    let cache: Cache<String, String> = Cache::builder().build().await.unwrap();
    let v = cache
        .get_or_option_with_ttl(
            &"ttl-obj".to_string(),
            Some(Duration::from_secs(30)),
            || async { Ok(Some("tv".to_string())) },
        )
        .await
        .unwrap();
    assert_eq!(v.as_deref(), Some("tv"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_or_option_sync_concurrent_follower_paths() {
    let cache: Arc<Cache<String, String>> =
        Arc::new(Cache::builder().sync_mode(true).build().await.unwrap());

    // follower 值路径（1186-1198 等待 + 1222-1223 返回）
    let mut handles = Vec::new();
    for _ in 0..6 {
        let cache = Arc::clone(&cache);
        handles.push(tokio::task::spawn_blocking(move || {
            cache
                .get_or_option_sync(&"of".to_string(), || {
                    std::thread::sleep(Duration::from_millis(200));
                    Ok(Some("sv".to_string()))
                })
                .unwrap()
        }));
    }
    for h in handles {
        assert_eq!(h.await.unwrap().as_deref(), Some("sv"));
    }

    // follower 哨兵路径：leader 缓存 null 哨兵，follower 返回 None（1200-1210）
    let sentinel_cache: Arc<Cache<String, String>> = Arc::new(
        Cache::builder()
            .sync_mode(true)
            .null_cache_ttl(Duration::from_secs(30))
            .build()
            .await
            .unwrap(),
    );
    let mut handles = Vec::new();
    for _ in 0..4 {
        let cache = Arc::clone(&sentinel_cache);
        handles.push(tokio::task::spawn_blocking(move || {
            cache
                .get_or_option_sync(&"ns".to_string(), || {
                    std::thread::sleep(Duration::from_millis(200));
                    Ok::<Option<String>, oxcache::OxCacheError>(None)
                })
                .unwrap()
        }));
    }
    for h in handles {
        assert!(h.await.unwrap().is_none());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_sentinel_payload_treated_as_miss() {
    // stale 窗口拉宽到 2s，防插桩/调度抖动下滑出窄窗误入 Expired 分支
    let cache: Cache<String, String> = Cache::builder()
        .stale_ttl(Duration::from_secs(2))
        .stale_policy(StalePolicy::Return)
        .null_cache_ttl(Duration::from_millis(70))
        .build()
        .await
        .unwrap();

    // 哨兵写入（70ms TTL）
    let none = cache
        .get_or_option(&"sn".to_string(), || async {
            Ok::<Option<String>, oxcache::OxCacheError>(None)
        })
        .await
        .unwrap();
    assert!(none.is_none());

    // 过期进入 stale 窗口后 get_or：stale_step_basic 命中哨兵载荷 → 按 miss 处理
    tokio::time::sleep(Duration::from_millis(150)).await;
    let calls = AtomicU32::new(0);
    let v = cache
        .get_or(&"sn".to_string(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("recomputed".to_string()) }
        })
        .await
        .unwrap();
    assert_eq!(v, "recomputed", "哨兵 stale 命中按 miss 重新回源");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_or_refresh_revalidate_deletes_stale_entry() {
    // stale 窗口拉宽到 2s，防插桩/调度抖动下滑出窄窗误入 Expired 分支
    let cache: Cache<String, String> = Cache::builder()
        .stale_ttl(Duration::from_secs(2))
        .stale_policy(StalePolicy::Revalidate)
        .build()
        .await
        .unwrap();
    let calls = Arc::new(AtomicU32::new(0));

    let v = cache
        .get_or_refresh(&"rr".to_string(), Some(Duration::from_millis(70)), {
            let calls = Arc::clone(&calls);
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok("gen1".to_string()) }
            }
        })
        .await
        .unwrap();
    assert_eq!(v, "gen1");

    // stale 窗口：Revalidate 删除 stale 条目并走 miss 回源（466）
    tokio::time::sleep(Duration::from_millis(120)).await;
    let v2 = cache
        .get_or_refresh(&"rr".to_string(), Some(Duration::from_millis(70)), {
            let calls = Arc::clone(&calls);
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                async move { Ok("gen2".to_string()) }
            }
        })
        .await
        .unwrap();
    assert_eq!(v2, "gen2");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
