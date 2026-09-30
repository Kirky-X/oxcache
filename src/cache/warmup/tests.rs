// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 预热单元测试：回填命中、批量晋升、去重、并发上界与失败容错

use super::*;
use crate::backend::CacheReader;
use crate::cache::ChainCache;
use crate::error::OxCacheError;
use crate::testing::MockBackend;
use std::sync::Arc;

/// 固定条目 loader（内存向量）
struct VecLoader {
    entries: Vec<WarmupEntry>,
}

#[async_trait::async_trait]
impl WarmupLoader for VecLoader {
    async fn load_hot_keys(&self) -> OxCacheResult<Vec<WarmupEntry>> {
        Ok(self.entries.clone())
    }
}

/// 注错 loader：端口级失败
struct FailingLoader;

#[async_trait::async_trait]
impl WarmupLoader for FailingLoader {
    async fn load_hot_keys(&self) -> OxCacheResult<Vec<WarmupEntry>> {
        Err(OxCacheError::Operation("loader source unavailable".into()))
    }
}

/// 两层链：高分 L1 + 低分 L2，返回链与低层后端句柄（供直塞低层做晋升场景）
fn two_layer_chain() -> Arc<ChainCache> {
    let l1 = MockBackend::new("l1", 100, false);
    let l2 = MockBackend::new("l2", 50, true);
    Arc::new(ChainCache::builder().backend(l1).backend(l2).build())
}

fn entry(key: &str, value: &str) -> WarmupEntry {
    WarmupEntry {
        key: key.to_string(),
        value: Some(value.as_bytes().to_vec()),
        ttl: None,
    }
}

#[tokio::test]
async fn warmup_backfills_loader_values_into_all_links() {
    let chain = two_layer_chain();
    let loader = Arc::new(VecLoader {
        entries: vec![
            entry("hot:1", "v1"),
            entry("hot:2", "v2"),
            WarmupEntry {
                key: "hot:3".into(),
                value: Some(b"v3".to_vec()),
                ttl: Some(Duration::from_secs(60)),
            },
        ],
    });

    let report = Warmup::builder(chain.clone())
        .concurrency(4)
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(
        (report.loader_entries, report.deduped, report.warmed),
        (3, 3, 3)
    );
    assert_eq!(report.failed, 0);
    assert!(report.failures.is_empty());

    // 全链可读（set 落全部后端）
    for (key, value) in [("hot:1", "v1"), ("hot:2", "v2"), ("hot:3", "v3")] {
        let got = chain.get(key).await.expect("get 不应失败");
        assert_eq!(got, Some(value.as_bytes().to_vec()), "{key} 应回填命中");
    }
    // 直供 TTL 透传（hot:3 带 60s TTL，链上应可读到剩余 TTL）
    let ttl = chain.ttl("hot:3").await.expect("ttl 不应失败");
    assert!(ttl.is_some(), "带 TTL 条目应保留过期语义");
    let no_ttl = chain.ttl("hot:1").await.expect("ttl 不应失败");
    assert!(no_ttl.is_none(), "未带 TTL 条目不应凭空获得过期时间");
}

#[tokio::test]
async fn warmup_dedupes_duplicate_keys_first_wins() {
    let chain = two_layer_chain();
    let loader = Arc::new(VecLoader {
        entries: vec![
            entry("dup", "first"),
            entry("dup", "second"),
            entry("other", "v"),
        ],
    });

    let report = Warmup::builder(chain.clone())
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report.loader_entries, 3);
    assert_eq!(report.deduped, 2, "重复 key 应收敛");
    assert_eq!(report.warmed, 2);

    let got = chain.get("dup").await.expect("get 不应失败");
    assert_eq!(got, Some(b"first".to_vec()), "重复 key 以首次出现为准");
}

#[tokio::test]
async fn warmup_promotes_key_only_entries_via_batch_read() {
    let chain = two_layer_chain();
    // 值仅存在于低层（L2）：模拟冷启动后低层有历史数据、高层为空的场景
    let low = chain.links().last().expect("两层链必有低层").clone();
    low.backend()
        .set(Arc::from("cold:1"), Arc::new(b"lv1".to_vec()), None)
        .await
        .expect("直塞低层");
    low.backend()
        .set(Arc::from("cold:2"), Arc::new(b"lv2".to_vec()), None)
        .await
        .expect("直塞低层");

    let loader = Arc::new(VecLoader {
        entries: vec![
            WarmupEntry {
                key: "cold:1".into(),
                value: None,
                ttl: None,
            },
            WarmupEntry {
                key: "cold:2".into(),
                value: None,
                ttl: None,
            },
            WarmupEntry {
                key: "cold:absent".into(),
                value: None,
                ttl: None,
            },
        ],
    });

    let report = Warmup::builder(chain.clone())
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report.promoted, 2, "低层命中条目应晋升");
    assert_eq!(report.missing, 1, "链上无值条目应计 missing");
    assert_eq!(report.failed, 0);

    // 高层（L1）现可直接命中
    let high = chain.links().first().expect("两层链必有高层").clone();
    let got = high.backend().get("cold:1").await.expect("get 不应失败");
    assert_eq!(got, Some(b"lv1".to_vec()), "晋升后高层应有值");
}

#[tokio::test]
async fn warmup_loader_failure_is_explicit_and_cache_unaffected() {
    let chain = two_layer_chain();

    let result = Warmup::builder(chain.clone())
        .build()
        .run(Arc::new(FailingLoader))
        .await;

    let err = result.expect_err("loader 端口失败必须显性返回 Err");
    assert!(
        err.to_string().contains("loader source unavailable"),
        "错误应保留 loader 根因: {err}"
    );

    // 预热失败不影响缓存本体可用性
    chain
        .set("after-failure", b"ok".to_vec(), None)
        .await
        .expect("缓存仍可写");
}

#[tokio::test]
async fn warmup_write_failures_are_counted_not_fatal() {
    // 双层全部 set 注错：每条回填失败，但整批不中断、不向上抛
    let l1 = MockBackend::new("l1", 100, false).with_fail_set();
    let l2 = MockBackend::new("l2", 50, true).with_fail_set();
    let chain = Arc::new(ChainCache::builder().backend(l1).backend(l2).build());

    let loader = Arc::new(VecLoader {
        entries: vec![entry("k1", "v1"), entry("k2", "v2"), entry("k3", "v3")],
    });

    let report = Warmup::builder(chain)
        .build()
        .run(loader)
        .await
        .expect("写入失败应计入报告而非返回 Err");

    assert_eq!(report.warmed, 0);
    assert_eq!(report.failed, 3, "逐条失败应显性计数");
    assert_eq!(report.failures.len(), 3, "失败明细与计数一一对应");
    assert!(
        report
            .failures
            .iter()
            .all(|(k, _)| ["k1", "k2", "k3"].contains(&k.as_str())),
        "失败明细应携带键名"
    );
}

#[tokio::test]
async fn warmup_respects_concurrency_bound() {
    let chain = two_layer_chain();
    let entries: Vec<WarmupEntry> = (0..32u32).map(|i| entry(&format!("k{i}"), "v")).collect();
    let loader = Arc::new(VecLoader { entries });

    let report = Warmup::builder(chain)
        .concurrency(4)
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report.warmed, 32);
    assert!(
        report.peak_concurrency <= 4,
        "实测并发峰值不得超过配置上界: {}",
        report.peak_concurrency
    );
    assert!(report.peak_concurrency >= 1, "峰值应至少为 1");
}

#[tokio::test]
async fn warmup_zero_concurrency_clamps_to_one() {
    let chain = two_layer_chain();
    let loader = Arc::new(VecLoader {
        entries: vec![entry("k", "v")],
    });

    let report = Warmup::builder(chain)
        .concurrency(0)
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report.warmed, 1);
    assert_eq!(report.peak_concurrency, 1, "0 并发应收敛为串行");
}

#[tokio::test]
async fn warmup_drops_entries_over_value_size_limit() {
    let chain = two_layer_chain();
    let loader = Arc::new(VecLoader {
        entries: vec![
            entry("ok:1", "v1"),
            WarmupEntry {
                key: "too:big".into(),
                value: Some(vec![0u8; 64]),
                ttl: None,
            },
            entry("ok:2", "v2"),
        ],
    });

    let report = Warmup::builder(chain.clone())
        .max_value_bytes(8)
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report.deduped, 3);
    assert_eq!(report.warmed, 2, "限额内条目正常回填");
    assert_eq!(
        report.dropped_value_too_large, 1,
        "超单值上限条目应显性丢弃计数"
    );
    assert_eq!(report.failed, 0);
    assert!(report.failures.is_empty());

    let dropped = chain.get("too:big").await.expect("get 不应失败");
    assert!(dropped.is_none(), "超限条目不得写入链");
}

#[tokio::test]
async fn warmup_drops_entries_over_entry_cap() {
    let chain = two_layer_chain();
    let entries: Vec<WarmupEntry> = (0..5u32).map(|i| entry(&format!("cap{i}"), "v")).collect();
    let loader = Arc::new(VecLoader { entries });

    let report = Warmup::builder(chain.clone())
        .max_entries(2)
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report.deduped, 5);
    assert_eq!(report.warmed, 2, "条目数上限内按 loader 顺序保留");
    assert_eq!(
        report.dropped_over_entry_cap, 3,
        "超出条目数上限部分应显性丢弃计数"
    );

    // 顺序即优先级：保留的是前 2 条（cap0/cap1），后 3 条未入链
    for i in 0..5u32 {
        let got = chain.get(&format!("cap{i}")).await.expect("get 不应失败");
        assert_eq!(got.is_some(), i < 2, "cap{i} 入链状态应符合保留顺序");
    }
}

#[tokio::test]
async fn warmup_promotion_preserves_source_ttl() {
    let chain = two_layer_chain();
    // 值仅存在于低层：带 TTL 与不带 TTL 各一条，晋升后源过期语义应保留
    let low = chain.links().last().expect("两层链必有低层").clone();
    low.backend()
        .set(
            Arc::from("src:ttl"),
            Arc::new(b"tv".to_vec()),
            Some(Duration::from_secs(60)),
        )
        .await
        .expect("直塞低层（带 TTL）");
    low.backend()
        .set(Arc::from("src:no-ttl"), Arc::new(b"nv".to_vec()), None)
        .await
        .expect("直塞低层（无 TTL）");

    let loader = Arc::new(VecLoader {
        entries: vec![
            WarmupEntry {
                key: "src:ttl".into(),
                value: None,
                ttl: None,
            },
            WarmupEntry {
                key: "src:no-ttl".into(),
                value: None,
                ttl: None,
            },
        ],
    });

    let report = Warmup::builder(chain.clone())
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report.promoted, 2);

    let promoted_ttl = chain.ttl("src:ttl").await.expect("ttl 不应失败");
    assert!(
        promoted_ttl.is_some_and(|t| t > Duration::ZERO && t <= Duration::from_secs(60)),
        "晋升应透传源条目的剩余 TTL，不得重置为链默认/永不过期: {promoted_ttl:?}"
    );
    let no_ttl = chain.ttl("src:no-ttl").await.expect("ttl 不应失败");
    assert!(no_ttl.is_none(), "源条目无 TTL 时晋升不应凭空获得过期时间");
}

#[tokio::test]
async fn warmup_empty_loader_is_zero_report() {
    let chain = two_layer_chain();
    let loader = Arc::new(VecLoader {
        entries: Vec::new(),
    });

    let report = Warmup::builder(chain)
        .build()
        .run(loader)
        .await
        .expect("预热应成功");

    assert_eq!(report, WarmupReport::default(), "空 loader 应产出全零报告");
}
