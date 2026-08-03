// Copyright (c) 2025-2026 Kirky.X
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
    println!("=== 缓存提升策略示例 ===\n");

    // 1. 模拟 L1 + L2 分层
    println!("--- 1. 模拟 L1 + L2 分层 ---");
    let l1 = L1Cache::<String, Session>::new(3); // L1 容量小
    let l2: Cache<String, Session> = Cache::builder().capacity(100).build().await?; // L2 容量大
    println!("  L1 (内存): 容量=3 (模拟快速缓存)");
    println!("  L2 (缓存): 容量=100 (模拟慢速/大容量缓存)");

    // 2. 初始化数据到 L2（模拟从数据库或 Redis 加载）
    println!("\n--- 2. 初始化数据到 L2 ---");
    let sessions: Vec<Session> = (1..=10)
        .map(|i| Session {
            id: format!("sess_{}", i),
            user_id: i,
            data: format!("会话数据 {}", i),
            last_accessed: chrono::Utc::now(),
        })
        .collect();

    for session in &sessions {
        l2.set(&session.id, session).await?;
    }
    println!("  ✓ 写入 {} 个会话到 L2", sessions.len());

    // 3. 模拟带提升的读取
    println!("\n--- 3. 带提升的读取（promote_on_hit） ---");

    async fn get_with_promotion(
        l1: &L1Cache<String, Session>,
        l2: &Cache<String, Session>,
        key: &str,
    ) -> Result<Option<Session>, Box<dyn std::error::Error>> {
        // 先查 L1
        if let Some(session) = l1.get(&key.to_string()) {
            println!("    [L1 命中] {}", key);
            return Ok(Some(session));
        }

        // L1 未命中，查 L2
        if let Some(session) = l2.get(&key.to_string()).await? {
            println!("    [L2 命中 → 提升到 L1] {}", key);
            // 提升到 L1
            l1.put(key.to_string(), session.clone());
            return Ok(Some(session));
        }

        println!("    [全部未命中] {}", key);
        Ok(None)
    }

    // 模拟访问序列 — 部分会话被频繁访问
    let access_sequence = vec![
        "sess_1", "sess_2", "sess_1", "sess_3", "sess_1", "sess_2", "sess_4", "sess_1", "sess_5", "sess_2", "sess_1",
        "sess_3",
    ];

    println!("  访问序列 ({} 次):", access_sequence.len());
    for key in &access_sequence {
        get_with_promotion(&l1, &l2, key).await?;
    }

    println!("\n  L1 最终状态: {} 条", l1.len());

    // 4. 访问频率分析
    println!("\n--- 4. 访问频率分析 ---");
    let mut freq: HashMap<&str, u32> = HashMap::new();
    for key in &access_sequence {
        *freq.entry(key).or_default() += 1;
    }
    let mut freq_vec: Vec<_> = freq.into_iter().collect();
    freq_vec.sort_by(|a, b| b.1.cmp(&a.1));

    println!("  访问频率排名:");
    for (key, count) in &freq_vec {
        let bar = "█".repeat(*count as usize);
        println!("    {} {} ({} 次)", key, bar, count);
    }
    println!("  ✓ 热点数据 (sess_1) 自动留在 L1 中");

    // 5. 生产环境配置建议
    println!("\n--- 5. 生产环境配置建议 ---");
    println!("  真正的分层缓存配置:");
    println!("  ```rust");
    println!("  // L1 (Moka) + L2 (Redis) 分层");
    println!("  let cache: Cache<String, Session> = Cache::builder()");
    println!("      .ttl(Duration::from_secs(3600))     // L1 默认 TTL");
    println!("      .capacity(10_000)                    // L1 容量");
    println!("      .build().await?;");
    println!("  ```");
    println!();
    println!("  提升策略要点:");
    println!("  - L1 容量小但速度快，L2 容量大但较慢");
    println!("  - L2 命中时自动回填 L1（promote_on_hit）");
    println!("  - 热点数据自然提升到 L1，冷数据仅在 L2");
    println!("  - 合理设置 L1 容量避免频繁淘汰");

    // 6. TTL 对提升的影响
    println!("\n--- 6. TTL 对提升的影响 ---");
    let short_lived = Session {
        id: "short:1".into(),
        user_id: 99,
        data: "短生命周期".into(),
        last_accessed: chrono::Utc::now(),
    };
    l2.set_with_ttl(&"short:1".into(), &short_lived, Some(Duration::from_secs(5)))
        .await?;
    println!("  ✓ 设置短 TTL (5s) 会话");

    let remaining = l2.ttl(&"short:1".into()).await?;
    println!("  剩余 TTL: {:?}", remaining);

    println!("\n✓ 缓存提升示例完成");
    Ok(())
}
