// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 临时执行探针（E2E 场景 188）：`#[cached]` 返回 `Err` 时不回写缓存，
//! 下次同键调用重新执行函数体；`Ok` 分支正常写回并命中。

#![cfg(feature = "macros")]

use oxcache::Cache;
use oxcache::cached;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

static PROBE188_CALLS: AtomicUsize = AtomicUsize::new(0);
static PROBE188_FAIL_MODE: AtomicUsize = AtomicUsize::new(1);

#[cached(service = "probe188_svc")]
async fn probe188_fn(id: u64) -> Result<u64, String> {
    PROBE188_CALLS.fetch_add(1, Ordering::SeqCst);
    if PROBE188_FAIL_MODE.load(Ordering::SeqCst) == 1 {
        Err(format!("boom-{id}"))
    } else {
        Ok(id * 2)
    }
}

#[tokio::test]
#[serial]
async fn probe_188_err_is_not_written_back() {
    let cache: Cache<String, Vec<u8>> = Cache::builder().build().await.unwrap();
    cache.register_for_macro("probe188_svc").await.unwrap();
    PROBE188_CALLS.store(0, Ordering::SeqCst);
    PROBE188_FAIL_MODE.store(1, Ordering::SeqCst);

    let r1 = probe188_fn(7).await;
    assert_eq!(r1, Err("boom-7".to_string()));
    assert_eq!(
        PROBE188_CALLS.load(Ordering::SeqCst),
        1,
        "first call runs body"
    );

    let r2 = probe188_fn(7).await;
    assert_eq!(r2, Err("boom-7".to_string()));
    assert_eq!(
        PROBE188_CALLS.load(Ordering::SeqCst),
        2,
        "Err must not be cached: second call re-executes body"
    );

    PROBE188_FAIL_MODE.store(0, Ordering::SeqCst);
    let r3 = probe188_fn(7).await.unwrap();
    assert_eq!(r3, 14);
    assert_eq!(
        PROBE188_CALLS.load(Ordering::SeqCst),
        3,
        "Ok call runs body"
    );

    let r4 = probe188_fn(7).await.unwrap();
    assert_eq!(r4, 14);
    assert_eq!(
        PROBE188_CALLS.load(Ordering::SeqCst),
        3,
        "Ok result is written back: fourth call must hit cache"
    );
}
