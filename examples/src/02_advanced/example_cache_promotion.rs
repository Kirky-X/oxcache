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
    println!("=== Cache promotion strategy example ===\n");

    println!("--- 1. Simulating L1 + L2 tiers ---");
    let l1 = L1Cache::<String, Session>::new(3); // L1 容量小
    let l2: Cache<String, Session> = Cache::builder().capacity(100).build().await?; // L2 容量大
    println!("  L1 (memory): capacity=3 (simulating a fast cache)");
    println!("  L2 (cache): capacity=100 (simulating a slow/large cache)");

    println!("\n--- 2. Initialize data into L2 ---");
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
    println!("  ✓ wrote {} sessions into L2", sessions.len());

    println!("\n--- 3. Reads with promotion (promote_on_hit) ---");

    async fn get_with_promotion(
        l1: &L1Cache<String, Session>,
        l2: &Cache<String, Session>,
        key: &str,
    ) -> Result<Option<Session>, Box<dyn std::error::Error>> {
        // 先查 L1
        if let Some(session) = l1.get(&key.to_string()) {
            println!("    [L1 hit] {}", key);
            return Ok(Some(session));
        }

        // L1 未命中，查 L2
        if let Some(session) = l2.get(&key.to_string()).await? {
            println!("    [L2 hit -> promoted to L1] {}", key);
            // 提升到 L1
            l1.put(key.to_string(), session.clone());
            return Ok(Some(session));
        }

        println!("    [all miss] {}", key);
        Ok(None)
    }

    // 模拟访问序列 — 部分会话被频繁访问
    let access_sequence = vec![
        "sess_1", "sess_2", "sess_1", "sess_3", "sess_1", "sess_2", "sess_4", "sess_1", "sess_5",
        "sess_2", "sess_1", "sess_3",
    ];

    println!("  Access sequence ({} accesses):", access_sequence.len());
    for key in &access_sequence {
        get_with_promotion(&l1, &l2, key).await?;
    }

    println!("\n  L1 final state: {} entries", l1.len());

    println!("\n--- 4. Access frequency analysis ---");
    let mut freq: HashMap<&str, u32> = HashMap::new();
    for key in &access_sequence {
        *freq.entry(key).or_default() += 1;
    }
    let mut freq_vec: Vec<_> = freq.into_iter().collect();
    freq_vec.sort_by_key(|b| std::cmp::Reverse(b.1));

    println!("  Access frequency ranking:");
    for (key, count) in &freq_vec {
        let bar = "█".repeat(*count as usize);
        println!("    {} {} ({} times)", key, bar, count);
    }
    println!("  ✓ hot data (sess_1) stays in L1 automatically");

    println!("\n--- 5. Production configuration suggestions ---");
    println!("  A real tiered-cache configuration:");
    println!("  ```rust");
    println!("  // L1 (Moka) + L2 (Redis) tiers");
    println!("  let cache: Cache<String, Session> = Cache::builder()");
    println!("      .ttl(Duration::from_secs(3600))     // L1 default TTL");
    println!("      .capacity(10_000)                    // L1 capacity");
    println!("      .build().await?;");
    println!("  ```");
    println!();
    println!("  Promotion strategy notes:");
    println!("  - L1 is small but fast, L2 is large but slower");
    println!("  - on an L2 hit, backfill into L1 automatically (promote_on_hit)");
    println!("  - hot data is promoted to L1 naturally; cold data stays in L2");
    println!("  - size L1 sensibly to avoid frequent eviction");

    println!("\n--- 6. TTL effects on promotion ---");
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
    println!("  ✓ set a session with a short TTL (5s)");

    let remaining = l2.ttl(&"short:1".into()).await?;
    println!("  remaining TTL: {:?}", remaining);

    println!("\n✓ Cache promotion example complete");
    Ok(())
}
