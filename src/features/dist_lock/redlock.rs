// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! RedLock 多节点多数派锁（`red-lock` feature）
//!
//! [`RedLock`] 在 N 个独立锁节点上执行 RedLock 风格的多数派获取：
//! 逐节点 `SET NX PX`，成功节点数 ≥ `N/2 + 1` 才视为获取成功；
//! 释放时向**全部**节点广播释放（owner 校验，非本人释放为 no-op）。
//!
//! 节点抽象 [`LockNode`] 提供协议层接口：[`InMemoryLockNode`] 为协议层
//! mock（单测用），[`RedisLockNode`] 适配真实 Redis（`lock` feature 路径）。
//!
//! # 已知限制（MVP 口径）
//!
//! 未做时钟漂移补偿（RedLock 论文的 validity time 计算）：持有期由 TTL
//! 直接约束；跨节点 fencing token 取成功节点的**最大值**。
//!
//! # Example
//!
//! ```rust,ignore
//! use oxcache::features::dist_lock::redlock::{InMemoryLockNode, RedLock};
//!
//! let nodes = (0..5).map(|_| Arc::new(InMemoryLockNode::healthy())).collect();
//! let mut lock = RedLock::new(nodes, "job:singleton".into());
//! if lock.try_acquire().await? { /* critical */ lock.release().await?; }
//! ```

use crate::error::{OxCacheError, OxCacheResult};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 单个锁节点的协议层接口
#[async_trait]
pub trait LockNode: Send + Sync {
    /// 尝试获取（SET NX PX 语义）：true = 本节点获取成功
    async fn try_acquire(&self, key: &str, owner: &str, ttl: Duration) -> OxCacheResult<bool>;

    /// 释放（owner 校验；非本人 no-op）
    async fn release(&self, key: &str, owner: &str) -> OxCacheResult<()>;

    /// `INCR <key>:fence` 单调 fencing token
    async fn incr_fence(&self, key: &str) -> OxCacheResult<u64>;
}

/// 进程内多节点协议层 mock（单测与进程内组合用）
#[derive(Default)]
pub struct InMemoryLockNode {
    available: bool,
    locks: Mutex<std::collections::HashMap<String, (String, Instant)>>,
    fences: Mutex<std::collections::HashMap<String, u64>>,
}

impl InMemoryLockNode {
    /// 健康节点
    pub fn healthy() -> Self {
        Self {
            available: true,
            ..Default::default()
        }
    }

    /// 故障节点（模拟宕机：所有操作返回连接错误）
    pub fn down() -> Self {
        Self {
            available: false,
            ..Default::default()
        }
    }

    fn ensure_available(&self) -> OxCacheResult<()> {
        if self.available {
            Ok(())
        } else {
            Err(OxCacheError::Connection("lock node is down".to_string()))
        }
    }
}

#[async_trait]
impl LockNode for InMemoryLockNode {
    async fn try_acquire(&self, key: &str, owner: &str, ttl: Duration) -> OxCacheResult<bool> {
        self.ensure_available()?;
        let mut locks = self.locks.lock().unwrap();
        if let Some((holder, expires_at)) = locks.get(key)
            && *expires_at > Instant::now()
        {
            return Ok(holder == owner);
            // 已过期：落到下方重新获取
        }
        locks.insert(key.to_string(), (owner.to_string(), Instant::now() + ttl));
        Ok(true)
    }

    async fn release(&self, key: &str, owner: &str) -> OxCacheResult<()> {
        self.ensure_available()?;
        let mut locks = self.locks.lock().unwrap();
        if let Some((holder, _)) = locks.get(key)
            && holder == owner
        {
            locks.remove(key);
        }
        Ok(())
    }

    async fn incr_fence(&self, key: &str) -> OxCacheResult<u64> {
        self.ensure_available()?;
        let mut fences = self.fences.lock().unwrap();
        let next = fences.entry(key.to_string()).or_insert(0);
        *next += 1;
        Ok(*next)
    }
}

/// Redis 锁节点适配器（真实部署路径）
pub struct RedisLockNode {
    backend: Arc<crate::backend::RedisBackend>,
}

impl RedisLockNode {
    pub fn new(backend: Arc<crate::backend::RedisBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl LockNode for RedisLockNode {
    async fn try_acquire(&self, key: &str, owner: &str, ttl: Duration) -> OxCacheResult<bool> {
        let mut conn = self.backend.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(owner)
            .arg("NX")
            .arg("PX")
            .arg(ttl.as_millis() as u64)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("redlock node acquire failed: {e}")))?;
        Ok(result.is_some())
    }

    async fn release(&self, key: &str, owner: &str) -> OxCacheResult<()> {
        let mut conn = self.backend.conn();
        // owner 校验的安全释放（与 DistributedLock RELEASE_SCRIPT 语义一致）
        let _: i64 = redis::cmd("EVAL")
            .arg(
                "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) else return 0 end",
            )
            .arg(1)
            .arg(key)
            .arg(owner)
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("redlock node release failed: {e}")))?;
        Ok(())
    }

    async fn incr_fence(&self, key: &str) -> OxCacheResult<u64> {
        let mut conn = self.backend.conn();
        let token: i64 = redis::cmd("INCR")
            .arg(format!("{key}:fence"))
            .query_async(&mut conn)
            .await
            .map_err(|e| OxCacheError::Operation(format!("redlock fence incr failed: {e}")))?;
        Ok(u64::try_from(token).unwrap_or(0))
    }
}

/// RedLock 多节点多数派锁
pub struct RedLock {
    nodes: Vec<Arc<dyn LockNode>>,
    key: String,
    owner: String,
    ttl: Duration,
    state: Mutex<RedLockState>,
}

#[derive(Default)]
struct RedLockState {
    held: bool,
    /// 成功获取的节点索引
    acquired_nodes: Vec<usize>,
    fencing_token: u64,
}

impl RedLock {
    /// 多数派阈值：`N/2 + 1`
    pub fn quorum_for(node_count: usize) -> usize {
        node_count / 2 + 1
    }

    /// 创建 RedLock（owner 自动生成 uuid）
    pub fn new(nodes: Vec<Arc<dyn LockNode>>, key: impl Into<String>) -> Self {
        Self::with_ttl(nodes, key, Duration::from_secs(30))
    }

    /// 创建 RedLock 并指定 TTL
    pub fn with_ttl(
        nodes: Vec<Arc<dyn LockNode>>,
        key: impl Into<String>,
        ttl: Duration,
    ) -> Self {
        Self {
            nodes,
            key: key.into(),
            owner: uuid::Uuid::new_v4().to_string(),
            ttl,
            state: Mutex::new(RedLockState::default()),
        }
    }

    /// 尝试一次性多数派获取（单轮，无重试）
    ///
    /// 成功节点数不足多数派时回滚已获取节点（释放）并返回 `Ok(false)`。
    pub async fn try_acquire(&mut self) -> OxCacheResult<bool> {
        let quorum = Self::quorum_for(self.nodes.len());
        let mut acquired: Vec<usize> = Vec::new();
        let mut max_token: u64 = 0;

        for (idx, node) in self.nodes.iter().enumerate() {
            match node
                .try_acquire(&self.key, &self.owner, self.ttl)
                .await
            {
                Ok(true) => {
                    acquired.push(idx);
                    // 每个成功节点取 fence，取最大值作为全局 token
                    if let Ok(token) = node.incr_fence(&self.key).await {
                        max_token = max_token.max(token);
                    }
                }
                Ok(false) => {}
                Err(_) => {
                    // 节点故障：跳过（不参与多数派计数）
                }
            }
        }

        if acquired.len() >= quorum {
            let mut state = self.state.lock().unwrap();
            state.held = true;
            state.acquired_nodes = acquired;
            state.fencing_token = max_token;
            Ok(true)
        } else {
            // 不足多数派：回滚已获取节点
            for idx in acquired {
                let _ = self.nodes[idx].release(&self.key, &self.owner).await;
            }
            Ok(false)
        }
    }

    /// 释放：向全部节点广播（非本人释放为 no-op）
    pub async fn release(&mut self) -> OxCacheResult<()> {
        // 先摘除持有标记（不跨 await 持锁）
        let was_held = {
            let mut state = self.state.lock().unwrap();
            std::mem::replace(&mut state.held, false)
        };
        if !was_held {
            return Ok(());
        }
        for node in &self.nodes {
            let _ = node.release(&self.key, &self.owner).await;
        }
        self.state.lock().unwrap().acquired_nodes.clear();
        Ok(())
    }

    /// 是否持有
    pub async fn is_held(&self) -> bool {
        self.state.lock().unwrap().held
    }

    /// fencing token（成功节点的最大值；未持有时 None）
    pub fn fencing_token(&self) -> Option<u64> {
        let state = self.state.lock().unwrap();
        if state.held && state.fencing_token > 0 {
            Some(state.fencing_token)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cluster(count: usize, down: usize) -> Vec<Arc<dyn LockNode>> {
        (0..count)
            .map(|i| {
                if i < down {
                    Arc::new(InMemoryLockNode::down()) as Arc<dyn LockNode>
                } else {
                    Arc::new(InMemoryLockNode::healthy()) as Arc<dyn LockNode>
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn majority_acquisition_succeeds_with_quorum() {
        // 5 节点、2 宕机：3/5 达到多数派
        let nodes = cluster(5, 2);
        let mut lock = RedLock::with_ttl(nodes, "job:singleton", Duration::from_secs(5));

        assert!(lock.try_acquire().await.unwrap());
        assert!(lock.is_held().await);
        let token = lock.fencing_token().expect("多数派获取应有 fencing token");
        assert!(token > 0);
        lock.release().await.unwrap();
        assert!(!lock.is_held().await);
    }

    #[tokio::test]
    async fn minority_available_fails_fast_and_rolls_back() {
        // 5 节点、3 宕机：只剩 2/5 < 多数派
        let nodes = cluster(5, 3);
        let mut lock = RedLock::with_ttl(nodes, "job:singleton", Duration::from_secs(5));

        assert!(!lock.try_acquire().await.unwrap(), "不足多数派必须失败");
        assert!(!lock.is_held().await);
        assert!(lock.fencing_token().is_none());
    }

    #[tokio::test]
    async fn mutual_exclusion_across_instances() {
        let nodes: Vec<Arc<dyn LockNode>> = (0..5)
            .map(|_| Arc::new(InMemoryLockNode::healthy()) as Arc<dyn LockNode>)
            .collect();
        let mut a = RedLock::with_ttl(nodes.clone(), "shared", Duration::from_secs(5));
        let mut b = RedLock::with_ttl(nodes, "shared", Duration::from_secs(5));

        assert!(a.try_acquire().await.unwrap(), "A 应获取成功");
        assert!(
            !b.try_acquire().await.unwrap(),
            "A 持有期间 B 在同一节点集上必须获取失败（多数派互斥）"
        );
        a.release().await.unwrap();
        assert!(b.try_acquire().await.unwrap(), "A 释放后 B 应能获取");
    }

    #[tokio::test]
    async fn fencing_tokens_increase_monotonically() {
        let nodes: Vec<Arc<dyn LockNode>> = (0..3)
            .map(|_| Arc::new(InMemoryLockNode::healthy()) as Arc<dyn LockNode>)
            .collect();
        let mut lock = RedLock::with_ttl(nodes, "fenced", Duration::from_secs(5));

        lock.try_acquire().await.unwrap();
        let t1 = lock.fencing_token().unwrap();
        lock.release().await.unwrap();

        lock.try_acquire().await.unwrap();
        let t2 = lock.fencing_token().unwrap();
        assert!(t2 > t1, "fencing token 必须单调递增 ({t1} -> {t2})");
    }

    #[tokio::test]
    async fn release_is_noop_when_not_held() {
        let nodes: Vec<Arc<dyn LockNode>> = (0..3)
            .map(|_| Arc::new(InMemoryLockNode::healthy()) as Arc<dyn LockNode>)
            .collect();
        let mut lock = RedLock::with_ttl(nodes, "k", Duration::from_secs(5));
        lock.release().await.unwrap();
        assert!(!lock.is_held().await);
    }

    #[test]
    fn quorum_formula() {
        assert_eq!(RedLock::quorum_for(5), 3);
        assert_eq!(RedLock::quorum_for(3), 2);
        assert_eq!(RedLock::quorum_for(1), 1);
    }

    #[tokio::test]
    async fn node_down_is_not_counted() {
        // 单节点宕机：该节点的 acquire 返回连接错误，不计入成功数
        let down = Arc::new(InMemoryLockNode::down());
        let mut lock = RedLock::with_ttl(vec![down], "k", Duration::from_secs(5));
        assert!(!lock.try_acquire().await.unwrap());
    }
}
