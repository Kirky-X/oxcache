// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! `#[cached]` 宏 single-flight 的共享支撑设施。
//!
//! 审计（2026-09-23，F04）发现宏生成的 single-flight 使用**单把全局锁**且无
//! panic 守卫：leader panic 后注册表条目永久残留，该 key 的所有后续调用变为
//! 等待已死信号的 follower——key 级永久死锁。本模块提供与
//! `cache::api::basic_ops` 同源的修复设施：
//!
//! - **64 路分片**：`shard_index` 按 key hash 路由，消除单锁热点；
//! - **watch flight 信号**：`tokio::sync::watch` 的版本比对语义使
//!   "leader 先完成、follower 后订阅"仍立即返回（`Notify::notify_waiters`
//!   在该时序下丢失唤醒，见审计 F03）；
//! - **panic 守卫**：[`AsyncSfGuard`] 的 `Drop` 兜底移除注册表条目并放行
//!   等待者，leader panic / 早退不再产生 key 永久死锁。
//!
//! 宏生成代码引用本模块路径（`::oxcache::macro_support::…`），故必须始终
//! 编译（无 feature 门）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// 单个分片的存储类型：key → 该 key 的 flight 完成信号。
pub type SfShard = Mutex<HashMap<String, Arc<watch::Sender<()>>>>;

/// 分片数量（2 的幂，通过掩码路由，与 basic_ops 的 GET_OR_LOCKS 一致）。
pub const SF_SHARDS: usize = 64;

/// 计算 key 对应的分片索引（DefaultHasher + 掩码，同 key 稳定）。
pub fn shard_index(key: &str) -> usize {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) & (SF_SHARDS - 1)
}

/// leader 侧 panic 安全守卫。
///
/// `finish()`（幂等）：从分片注册表移除条目并向 flight 信号发送完成。
/// 未 `finish()` 即 Drop（panic / `?` 早退 / 提前 return）时由 `Drop` 兜底
/// 执行同一清理，保证该 key 后续调用可重新成为 leader、等待者全部放行。
pub struct AsyncSfGuard {
    shards: &'static [SfShard; SF_SHARDS],
    idx: usize,
    key: String,
    signal: Arc<watch::Sender<()>>,
    finished: bool,
}

impl AsyncSfGuard {
    /// 由 leader 在注册为 flight 后创建。
    pub fn new(
        shards: &'static [SfShard; SF_SHARDS],
        idx: usize,
        key: String,
        signal: Arc<watch::Sender<()>>,
    ) -> Self {
        Self {
            shards,
            idx,
            key,
            signal,
            finished: false,
        }
    }

    /// 标记 flight 完成：移除注册表条目并放行全部等待者（幂等）。
    pub fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Ok(mut map) = self.shards[self.idx].lock() {
            map.remove(&self.key);
        }
        // 无 receiver 时发送无害（watch 语义）；等待者通过 changed() 或
        // 通道关闭（全部 Sender 释放）两条路径都会被放行。
        let _ = self.signal.send(());
    }
}

impl Drop for AsyncSfGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.finish();
        }
    }
}

/// follower 等待 leader flight 完成信号。
///
/// `changed()` 对未读版本（含 leader 先完成再订阅的时序）立即返回；
/// 通道关闭（leader 终止并释放 Sender）返回 `Err`，两者均为正常放行。
pub async fn wait_flight(mut rx: watch::Receiver<()>) {
    let _ = rx.changed().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    static SHARDS: std::sync::LazyLock<[SfShard; SF_SHARDS]> =
        std::sync::LazyLock::new(|| std::array::from_fn(|_| Mutex::new(HashMap::new())));

    fn register(key: &str) -> (Arc<watch::Sender<()>>, watch::Receiver<()>) {
        let idx = shard_index(key);
        let mut map = SHARDS[idx].lock().unwrap();
        match map.entry(key.to_string()) {
            std::collections::hash_map::Entry::Occupied(e) => {
                let tx = e.get().clone();
                let rx = tx.subscribe();
                (tx, rx)
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                let (tx, _rx) = watch::channel(());
                let tx = Arc::new(tx);
                let rx = tx.subscribe();
                e.insert(tx.clone());
                (tx, rx)
            }
        }
    }

    #[test]
    fn test_shard_index_in_range_and_stable() {
        for key in [
            "",
            "a",
            "user:123",
            "很长很长的中文key🎯",
            &"x".repeat(1024),
        ] {
            let idx = shard_index(key);
            assert!(idx < SF_SHARDS);
            assert_eq!(idx, shard_index(key));
        }
    }

    #[tokio::test]
    async fn test_finish_removes_entry_and_signals() {
        let (tx, rx) = register("k-finish");
        let mut guard = AsyncSfGuard::new(&SHARDS, shard_index("k-finish"), "k-finish".into(), tx);
        guard.finish();
        // 幂等：二次 finish 不 panic、不重复清理
        guard.finish();
        assert!(
            SHARDS[shard_index("k-finish")]
                .lock()
                .unwrap()
                .get("k-finish")
                .is_none()
        );
        let waited = tokio::time::timeout(Duration::from_secs(1), wait_flight(rx)).await;
        assert!(waited.is_ok(), "finish 后等待者必须被放行");
    }

    #[tokio::test]
    async fn test_drop_without_finish_cleans_and_releases() {
        let (tx, rx) = register("k-drop");
        {
            let _guard = AsyncSfGuard::new(&SHARDS, shard_index("k-drop"), "k-drop".into(), tx);
            // 不调用 finish，直接 Drop（模拟 panic 展开路径）
        }
        assert!(
            SHARDS[shard_index("k-drop")]
                .lock()
                .unwrap()
                .get("k-drop")
                .is_none()
        );
        let waited = tokio::time::timeout(Duration::from_secs(1), wait_flight(rx)).await;
        assert!(waited.is_ok(), "Drop 兜底必须放行等待者");
    }

    #[tokio::test]
    async fn test_wait_flight_survives_sender_drop() {
        // 独立通道（不经注册表）：注册表持有额外 Sender Arc 会让通道无法关闭
        let (tx, _rx) = watch::channel(());
        let rx2 = tx.subscribe();
        drop(tx);
        let waited = tokio::time::timeout(Duration::from_secs(1), wait_flight(rx2)).await;
        assert!(waited.is_ok(), "通道关闭必须放行等待者");
    }
}
