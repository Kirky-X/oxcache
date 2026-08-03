// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 缓存策略模式示例
//!
//! 本示例演示常见的缓存使用策略：
//! - Cache-Aside（缓存旁路）：读时填充，写时失效
//! - Write-Through 模拟：写入时同步更新缓存与数据源
//! - Lazy Loading（懒加载）：使用 get_or 延迟计算
//! - TTL 生命周期管理：不同数据类型使用不同过期策略
//! - 容量规划：根据访问频率分层管理缓存
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_smart_strategy
//! ```

use oxcache::Cache;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Article {
    id: u64,
    title: String,
    content: String,
    view_count: u64,
}

/// 模拟数据源（如数据库）
struct ArticleStore {
    articles: std::sync::Mutex<std::collections::HashMap<u64, Article>>,
    query_count: AtomicU32,
}

impl ArticleStore {
    fn new() -> Self {
        let mut map = std::collections::HashMap::new();
        for i in 1..=20 {
            map.insert(
                i,
                Article {
                    id: i,
                    title: format!("文章 #{}", i),
                    content: format!("这是第 {} 篇文章的内容...", i),
                    view_count: 0,
                },
            );
        }
        Self {
            articles: std::sync::Mutex::new(map),
            query_count: AtomicU32::new(0),
        }
    }

    fn get(&self, id: u64) -> Option<Article> {
        self.query_count.fetch_add(1, Ordering::SeqCst);
        let store = self.articles.lock().unwrap();
        store.get(&id).cloned()
    }

    fn update(&self, article: &Article) {
        let mut store = self.articles.lock().unwrap();
        store.insert(article.id, article.clone());
    }

    fn query_count(&self) -> u32 {
        self.query_count.load(Ordering::SeqCst)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 缓存策略模式示例 ===\n");

    let cache: Cache<String, Article> = Cache::builder().capacity(100).build().await?;
    let store = Arc::new(ArticleStore::new());

    // 1. Cache-Aside 策略
    println!("--- 1. Cache-Aside（缓存旁路）策略 ---");
    println!("  读时填充：缓存未命中时从数据源加载并写入缓存");
    println!("  写时失效：更新数据源后删除缓存条目\n");

    async fn cache_aside_get(
        cache: &Cache<String, Article>,
        store: &ArticleStore,
        id: u64,
    ) -> Result<Option<Article>, Box<dyn std::error::Error>> {
        let key = format!("article:{}", id);
        // 先读缓存
        if let Some(article) = cache.get(&key).await? {
            return Ok(Some(article));
        }
        // 缓存未命中，从数据源加载
        if let Some(article) = store.get(id) {
            cache.set(&key, &article).await?;
            Ok(Some(article))
        } else {
            Ok(None)
        }
    }

    async fn cache_aside_invalidate(cache: &Cache<String, Article>, id: u64) -> Result<(), Box<dyn std::error::Error>> {
        let key = format!("article:{}", id);
        cache.delete(&key).await?;
        Ok(())
    }

    // 首次读取（缓存未命中 → 数据源加载）
    let article = cache_aside_get(&cache, &store, 1).await?;
    println!(
        "  首次读取 #1: {:?} (数据源查询次数: {})",
        article.map(|a| a.title),
        store.query_count()
    );

    // 再次读取（缓存命中）
    let article = cache_aside_get(&cache, &store, 1).await?;
    println!(
        "  再次读取 #1: {:?} (数据源查询次数: {})",
        article.map(|a| a.title),
        store.query_count()
    );

    // 更新后失效缓存
    if let Some(mut a) = store.get(1) {
        a.title = "文章 #1 (已更新)".into();
        store.update(&a);
        cache_aside_invalidate(&cache, 1).await?;
        println!("  更新 #1 并失效缓存");
    }

    // 再次读取（缓存未命中 → 加载更新后的数据）
    let article = cache_aside_get(&cache, &store, 1).await?;
    println!(
        "  读取更新后 #1: {:?} (数据源查询次数: {})",
        article.map(|a| a.title),
        store.query_count()
    );

    // 2. Lazy Loading（懒加载）
    println!("\n--- 2. Lazy Loading（懒加载）策略 ---");
    println!("  使用 get_or 延迟计算：仅在缓存未命中时执行昂贵的计算\n");

    let compute_count = Arc::new(AtomicU32::new(0));

    // 模拟昂贵的计算操作
    let expensive_compute = |id: u64| {
        let counter = compute_count.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Article {
                id,
                title: format!("计算生成的文章 #{}", id),
                content: "动态生成内容...".into(),
                view_count: 0,
            })
        }
    };

    let key = format!("computed:{}", 42);
    // 第一次：缓存未命中 → 执行计算
    let a1 = cache.get_or(&key, || expensive_compute(42)).await?;
    println!(
        "  第一次 get_or #42: {} (计算次数: {})",
        a1.title,
        compute_count.load(Ordering::SeqCst)
    );

    // 第二次：缓存命中 → 不执行计算
    let a2 = cache.get_or(&key, || expensive_compute(42)).await?;
    println!(
        "  第二次 get_or #42: {} (计算次数: {})",
        a2.title,
        compute_count.load(Ordering::SeqCst)
    );

    // 3. TTL 生命周期管理
    println!("\n--- 3. TTL 生命周期管理 ---");
    println!("  不同数据类型使用不同的过期策略\n");

    // 会话数据：短 TTL（30 秒）
    cache
        .set_with_ttl(
            &"session:user1".to_string(),
            &Article {
                id: 100,
                title: "会话".into(),
                content: "短生命周期".into(),
                view_count: 0,
            },
            Some(Duration::from_secs(30)),
        )
        .await?;
    println!("  ✓ 会话数据: TTL=30s");

    // 热点文章：中等 TTL（1 小时）
    cache
        .set_with_ttl(
            &"hot:article:1".to_string(),
            &Article {
                id: 1,
                title: "热门文章".into(),
                content: "中等生命周期".into(),
                view_count: 1000,
            },
            Some(Duration::from_secs(3600)),
        )
        .await?;
    println!("  ✓ 热门文章: TTL=1h");

    // 系统配置：长 TTL（24 小时）
    cache
        .set_with_ttl(
            &"config:app".to_string(),
            &Article {
                id: 0,
                title: "系统配置".into(),
                content: "长生命周期".into(),
                view_count: 0,
            },
            Some(Duration::from_secs(86400)),
        )
        .await?;
    println!("  ✓ 系统配置: TTL=24h");

    // 无 TTL（永不过期）
    cache
        .set(
            &"permanent:terms".to_string(),
            &Article {
                id: 0,
                title: "服务条款".into(),
                content: "永不过期".into(),
                view_count: 0,
            },
        )
        .await?;
    println!("  ✓ 服务条款: 无 TTL（永不过期）");

    // 检查 TTL
    let session_ttl = cache.ttl(&"session:user1".to_string()).await?;
    let config_ttl = cache.ttl(&"config:app".to_string()).await?;
    let permanent_ttl = cache.ttl(&"permanent:terms".to_string()).await?;
    println!("\n  剩余 TTL:");
    println!("    会话:   {:?}", session_ttl);
    println!("    配置:   {:?}", config_ttl);
    println!("    条款:   {:?}", permanent_ttl.unwrap_or(Duration::ZERO));

    // 4. 容量规划与访问频率分层
    println!("\n--- 4. 容量规划与访问频率分层 ---");

    // 预热热点数据
    let hot_ids = [1, 2, 3, 4, 5];
    for id in &hot_ids {
        if let Some(article) = store.get(*id) {
            let key = format!("hot:{}", id);
            cache
                .set_with_ttl(&key, &article, Some(Duration::from_secs(3600)))
                .await?;
        }
    }
    println!("  ✓ 预热 {} 篇热门文章", hot_ids.len());

    // 模拟访问模式
    let access_pattern = [1, 1, 1, 2, 2, 3, 1, 2, 4, 5, 1, 3, 6, 7, 8];
    let mut hits = 0u32;
    let mut misses = 0u32;
    for id in &access_pattern {
        let key = format!("hot:{}", id);
        if cache.get(&key).await?.is_some() {
            hits += 1;
        } else {
            misses += 1;
        }
    }
    let total = access_pattern.len() as u32;
    println!("  模拟 {} 次访问:", total);
    println!("    命中: {} ({:.1}%)", hits, hits as f64 / total as f64 * 100.0);
    println!("    未中: {} ({:.1}%)", misses, misses as f64 / total as f64 * 100.0);

    // 5. 统计信息
    println!("\n--- 5. 缓存统计 ---");
    let stats = cache.stats().await?;
    println!("  条目数: {}", stats.get("entry_count").unwrap_or(&"N/A".to_string()));
    println!("  容量:   {}", stats.get("capacity").unwrap_or(&"N/A".to_string()));

    // 清理
    cache.clear().await?;
    println!("\n✓ 缓存策略示例完成");
    Ok(())
}
