// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 缓存提升策略示例
//!
//! 本示例演示缓存提升（Cache Promotion）的概念和行为：
//! - 什么是缓存提升：L2 命中时将数据回填到 L1
//! - 使用 Cache-Aside 模式模拟提升行为
//! - 访问频率分析：识别热点数据
//! - 实际生产环境的配置建议
//!
//! 注意：真正的缓存提升需要 L1 (Moka) + L2 (Redis) 分层配置。
//! 本示例使用内存缓存模拟提升逻辑，生产环境请参考 example_redis_modes。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_cache_promotion
//! ```

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use oxcache::i18n::messages::{
    MSG_EXAMPLE_CACHE_PROMOTION_1_SIMULATING_L1_L2,
    MSG_EXAMPLE_CACHE_PROMOTION_2_INITIALIZE_DATA_INTO,
    MSG_EXAMPLE_CACHE_PROMOTION_3_READS_WITH_PROMOTION,
    MSG_EXAMPLE_CACHE_PROMOTION_4_ACCESS_FREQUENCY_ANALYSIS,
    MSG_EXAMPLE_CACHE_PROMOTION_5_PRODUCTION_CONFIGURATION_SUGGESTIONS,
    MSG_EXAMPLE_CACHE_PROMOTION_6_TTL_EFFECTS_ON, MSG_EXAMPLE_CACHE_PROMOTION_A_REAL_TIERED_CACHE,
    MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_FREQUENCY_RANKING,
    MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_SEQUENCE_ACCESSES, MSG_EXAMPLE_CACHE_PROMOTION_ALL_MISS,
    MSG_EXAMPLE_CACHE_PROMOTION_CAPACITY_10_000_L1, MSG_EXAMPLE_CACHE_PROMOTION_EXAMPLE_COMPLETE,
    MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_IS_PROMOTED, MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_SESS_1,
    MSG_EXAMPLE_CACHE_PROMOTION_L1_FINAL_STATE_ENTRIES, MSG_EXAMPLE_CACHE_PROMOTION_L1_HIT,
    MSG_EXAMPLE_CACHE_PROMOTION_L1_IS_SMALL_BUT, MSG_EXAMPLE_CACHE_PROMOTION_L1_MEMORY_CAPACITY_3,
    MSG_EXAMPLE_CACHE_PROMOTION_L1_MOKA_L2_REDIS,
    MSG_EXAMPLE_CACHE_PROMOTION_L2_CACHE_CAPACITY_100,
    MSG_EXAMPLE_CACHE_PROMOTION_L2_HIT_PROMOTED_TO, MSG_EXAMPLE_CACHE_PROMOTION_ON_AN_L2_HIT,
    MSG_EXAMPLE_CACHE_PROMOTION_PROMOTION_STRATEGY_NOTES,
    MSG_EXAMPLE_CACHE_PROMOTION_REMAINING_TTL, MSG_EXAMPLE_CACHE_PROMOTION_SET_A_SESSION_WITH,
    MSG_EXAMPLE_CACHE_PROMOTION_SIZE_L1_SENSIBLY_TO, MSG_EXAMPLE_CACHE_PROMOTION_STRATEGY_EXAMPLE,
    MSG_EXAMPLE_CACHE_PROMOTION_TIMES, MSG_EXAMPLE_CACHE_PROMOTION_TTL_DURATION_FROM_SECS,
    MSG_EXAMPLE_CACHE_PROMOTION_WROTE_SESSIONS_INTO_L2, t,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
    id: String,
    user_id: u64,
    data: String,
    last_accessed: chrono::DateTime<chrono::Utc>,
}

/// 模拟 L1 缓存（快速、容量小）
struct L1Cache<K: std::hash::Hash + Eq, V> {
    data: std::sync::Mutex<HashMap<K, V>>,
    capacity: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> L1Cache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            data: std::sync::Mutex::new(HashMap::new()),
            capacity,
        }
    }

    fn get(&self, key: &K) -> Option<V> {
        self.data.lock().unwrap().get(key).cloned()
    }

    fn put(&self, key: K, value: V) {
        let mut data = self.data.lock().unwrap();
        if data.len() >= self.capacity {
            // 简单淘汰：随机移除一个条目
            if let Some(first_key) = data.keys().next().cloned() {
                data.remove(&first_key);
            }
        }
        data.insert(key, value);
    }

    fn len(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}\n", t(MSG_EXAMPLE_CACHE_PROMOTION_STRATEGY_EXAMPLE, &[]));

    println!("{}", t(MSG_EXAMPLE_CACHE_PROMOTION_1_SIMULATING_L1_L2, &[]));
    let l1 = L1Cache::<String, Session>::new(3); // L1 容量小
    let l2: Cache<String, Session> = Cache::builder().capacity(100).build().await?; // L2 容量大
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_L1_MEMORY_CAPACITY_3, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_L2_CACHE_CAPACITY_100, &[])
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_2_INITIALIZE_DATA_INTO, &[])
    );
    let sessions: Vec<Session> = (1..=10)
        .map(|i| Session {
            id: format!("sess_{}", i),
            user_id: i,
            data: format!("session data {}", i),
            last_accessed: chrono::Utc::now(),
        })
        .collect();

    for session in &sessions {
        l2.set(&session.id, session).await?;
    }
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_PROMOTION_WROTE_SESSIONS_INTO_L2,
            &[("sessions", sessions.len().to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_3_READS_WITH_PROMOTION, &[])
    );

    async fn get_with_promotion(
        l1: &L1Cache<String, Session>,
        l2: &Cache<String, Session>,
        key: &str,
    ) -> Result<Option<Session>, Box<dyn std::error::Error>> {
        // 先查 L1
        if let Some(session) = l1.get(&key.to_string()) {
            println!(
                "    {}",
                t(
                    MSG_EXAMPLE_CACHE_PROMOTION_L1_HIT,
                    &[("key", key.to_string())]
                )
            );
            return Ok(Some(session));
        }

        // L1 未命中，查 L2
        if let Some(session) = l2.get(&key.to_string()).await? {
            println!(
                "    {}",
                t(
                    MSG_EXAMPLE_CACHE_PROMOTION_L2_HIT_PROMOTED_TO,
                    &[("key", key.to_string())]
                )
            );
            // 提升到 L1
            l1.put(key.to_string(), session.clone());
            return Ok(Some(session));
        }

        println!(
            "    {}",
            t(
                MSG_EXAMPLE_CACHE_PROMOTION_ALL_MISS,
                &[("key", key.to_string())]
            )
        );
        Ok(None)
    }

    // 模拟访问序列 — 部分会话被频繁访问
    let access_sequence = vec![
        "sess_1", "sess_2", "sess_1", "sess_3", "sess_1", "sess_2", "sess_4", "sess_1", "sess_5",
        "sess_2", "sess_1", "sess_3",
    ];

    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_SEQUENCE_ACCESSES,
            &[("access_sequence", access_sequence.len().to_string())]
        )
    );
    for key in &access_sequence {
        get_with_promotion(&l1, &l2, key).await?;
    }

    println!(
        "\n  {}",
        t(
            MSG_EXAMPLE_CACHE_PROMOTION_L1_FINAL_STATE_ENTRIES,
            &[("l1", l1.len().to_string())]
        )
    );

    println!(
        "\n{}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_4_ACCESS_FREQUENCY_ANALYSIS, &[])
    );
    let mut freq: HashMap<&str, u32> = HashMap::new();
    for key in &access_sequence {
        *freq.entry(key).or_default() += 1;
    }
    let mut freq_vec: Vec<_> = freq.into_iter().collect();
    freq_vec.sort_by_key(|b| std::cmp::Reverse(b.1));

    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_FREQUENCY_RANKING, &[])
    );
    for (key, count) in &freq_vec {
        let bar = "█".repeat(*count as usize);
        println!(
            "    {}",
            t(
                MSG_EXAMPLE_CACHE_PROMOTION_TIMES,
                &[
                    ("key", key.to_string()),
                    ("bar", bar.to_string()),
                    ("count", count.to_string())
                ]
            )
        );
    }
    println!("  {}", t(MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_SESS_1, &[]));

    println!(
        "\n{}",
        t(
            MSG_EXAMPLE_CACHE_PROMOTION_5_PRODUCTION_CONFIGURATION_SUGGESTIONS,
            &[]
        )
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_A_REAL_TIERED_CACHE, &[])
    );
    println!("  ```rust");
    println!("  {}", t(MSG_EXAMPLE_CACHE_PROMOTION_L1_MOKA_L2_REDIS, &[]));
    println!("  let cache: Cache<String, Session> = Cache::builder()");
    println!(
        "      {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_TTL_DURATION_FROM_SECS, &[])
    );
    println!(
        "      {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_CAPACITY_10_000_L1, &[])
    );
    println!("      .build().await?;");
    println!("  ```");
    println!();
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_PROMOTION_STRATEGY_NOTES, &[])
    );
    println!("  {}", t(MSG_EXAMPLE_CACHE_PROMOTION_L1_IS_SMALL_BUT, &[]));
    println!("  {}", t(MSG_EXAMPLE_CACHE_PROMOTION_ON_AN_L2_HIT, &[]));
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_IS_PROMOTED, &[])
    );
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_SIZE_L1_SENSIBLY_TO, &[])
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_PROMOTION_6_TTL_EFFECTS_ON, &[]));
    let short_lived = Session {
        id: "short:1".into(),
        user_id: 99,
        data: "short-lived".into(),
        last_accessed: chrono::Utc::now(),
    };
    l2.set_with_ttl(
        &"short:1".into(),
        &short_lived,
        Some(Duration::from_secs(5)),
    )
    .await?;
    println!(
        "  {}",
        t(MSG_EXAMPLE_CACHE_PROMOTION_SET_A_SESSION_WITH, &[])
    );

    let remaining = l2.ttl(&"short:1".into()).await?;
    println!(
        "  {}",
        t(
            MSG_EXAMPLE_CACHE_PROMOTION_REMAINING_TTL,
            &[("remaining", format!("{:?}", remaining))]
        )
    );

    println!("\n{}", t(MSG_EXAMPLE_CACHE_PROMOTION_EXAMPLE_COMPLETE, &[]));
    Ok(())
}
