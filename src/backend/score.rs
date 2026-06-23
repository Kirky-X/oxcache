// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 后端分数管理系统
//
// 每个缓存后端都有一个内置分数，用于自动排序。
// 分数越高，后端越快，越靠近链式缓存的前端。

/// 后端分数 trait - 每个后端必须实现
///
/// 分数用于自动排序链式缓存中的后端顺序。
/// 分数越高，表示后端速度越快，应该优先访问。
///
/// # 分数规则
///
/// - 90-100: L1 内存缓存 (Moka, DashMap)
/// - 70-89: 本地持久化缓存 (LMDB, SQLite)
/// - 40-69: 分布式缓存 (Redis, Memcached)
///
/// # Example
///
/// ```rust,ignore
/// use oxcache::backend::score::{BackendScore, Scores};
///
/// struct MyBackend;
///
/// impl BackendScore for MyBackend {
///     fn score(&self) -> u8 {
///         75 // 自定义分数
///     }
///
///     fn is_persistent(&self) -> bool {
///         true // 持久化后端
///     }
/// }
/// ```
pub trait BackendScore: Send + Sync + 'static {
    /// 获取后端分数
    ///
    /// 分数越高，后端越快，越靠近链式缓存的前端。
    ///
    /// # Returns
    ///
    /// 0-100 的分数值
    fn score(&self) -> u8;

    /// 检查后端是否持久化
    ///
    /// 持久化后端的数据在重启后仍然存在。
    /// 此信息用于链式缓存的写入策略。
    ///
    /// # Returns
    ///
    /// - `true`: 后端是持久化的
    /// - `false`: 后端是非持久化的（内存缓存）
    fn is_persistent(&self) -> bool;

    /// 获取后端名称
    ///
    /// 用于日志和调试。
    ///
    /// # Returns
    ///
    /// 后端类型名称
    fn backend_name(&self) -> &'static str {
        "unknown"
    }
}

/// 内置后端分数常量
///
/// 这些分数代表各种后端的典型性能特征。
/// 分数越高，后端速度越快。
pub struct Scores;

impl Scores {
    /// Moka 内存缓存分数
    ///
    /// Moka 是高性能内存缓存，使用 TinyLFU 驱逐策略。
    /// 适合作为 L1 缓存。
    pub const MOKA: u8 = 100;

    /// DashMap 内存缓存分数
    ///
    /// DashMap 是纯并发 HashMap，无驱逐策略。
    /// 适合作为 L1 缓存。
    pub const DASHMAP: u8 = 90;

    /// LMDB 持久化缓存分数
    ///
    /// LMDB 是高性能嵌入式键值存储。
    /// 适合作为 L2 本地持久化缓存。
    pub const LMDB: u8 = 85;

    /// SQLite 持久化缓存分数
    ///
    /// SQLite 是轻量级嵌入式数据库。
    /// 适合作为 L2 本地持久化缓存。
    pub const SQLITE: u8 = 70;

    /// Redis 分布式缓存分数
    ///
    /// Redis 是高性能分布式缓存。
    /// 适合作为 L2/L3 分布式缓存。
    pub const REDIS: u8 = 50;

    /// Memcached 分布式缓存分数
    ///
    /// Memcached 是简单高效的分布式缓存。
    /// 适合作为 L2/L3 分布式缓存。
    pub const MEMCACHED: u8 = 40;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Scores 常量测试
    // ============================================================================

    #[test]
    fn test_scores_moka() {
        assert_eq!(Scores::MOKA, 100);
    }

    #[test]
    fn test_scores_dashmap() {
        assert_eq!(Scores::DASHMAP, 90);
    }

    #[test]
    fn test_scores_lmdb() {
        assert_eq!(Scores::LMDB, 85);
    }

    #[test]
    fn test_scores_sqlite() {
        assert_eq!(Scores::SQLITE, 70);
    }

    #[test]
    fn test_scores_redis() {
        assert_eq!(Scores::REDIS, 50);
    }

    #[test]
    fn test_scores_memcached() {
        assert_eq!(Scores::MEMCACHED, 40);
    }

    #[test]
    fn test_scores_ordering() {
        // 验证分数顺序：Moka > DashMap > LMDB > SQLite > Redis > Memcached
        assert!(Scores::MOKA > Scores::DASHMAP);
        assert!(Scores::DASHMAP > Scores::LMDB);
        assert!(Scores::LMDB > Scores::SQLITE);
        assert!(Scores::SQLITE > Scores::REDIS);
        assert!(Scores::REDIS > Scores::MEMCACHED);
    }

    // ============================================================================
    // BackendScore trait 测试
    // ============================================================================

    struct TestMemoryBackend;

    impl BackendScore for TestMemoryBackend {
        fn score(&self) -> u8 {
            Scores::MOKA
        }

        fn is_persistent(&self) -> bool {
            false
        }

        fn backend_name(&self) -> &'static str {
            "test_memory"
        }
    }

    struct TestPersistentBackend;

    impl BackendScore for TestPersistentBackend {
        fn score(&self) -> u8 {
            Scores::SQLITE
        }

        fn is_persistent(&self) -> bool {
            true
        }

        fn backend_name(&self) -> &'static str {
            "test_persistent"
        }
    }

    struct TestDefaultNameBackend;

    impl BackendScore for TestDefaultNameBackend {
        fn score(&self) -> u8 {
            50
        }

        fn is_persistent(&self) -> bool {
            false
        }
        // 使用默认 backend_name 实现
    }

    #[test]
    fn test_backend_score_memory() {
        let backend = TestMemoryBackend;
        assert_eq!(backend.score(), 100);
        assert!(!backend.is_persistent());
        assert_eq!(backend.backend_name(), "test_memory");
    }

    #[test]
    fn test_backend_score_persistent() {
        let backend = TestPersistentBackend;
        assert_eq!(backend.score(), 70);
        assert!(backend.is_persistent());
        assert_eq!(backend.backend_name(), "test_persistent");
    }

    #[test]
    fn test_backend_score_default_name() {
        let backend = TestDefaultNameBackend;
        assert_eq!(backend.score(), 50);
        assert!(!backend.is_persistent());
        // 默认 backend_name 应该返回 "unknown"
        assert_eq!(backend.backend_name(), "unknown");
    }

    #[test]
    fn test_backend_score_trait_object() {
        // 测试 trait 对象动态分发
        let backends: Vec<Box<dyn BackendScore>> = vec![
            Box::new(TestMemoryBackend),
            Box::new(TestPersistentBackend),
            Box::new(TestDefaultNameBackend),
        ];

        assert_eq!(backends[0].score(), 100);
        assert_eq!(backends[1].score(), 70);
        assert_eq!(backends[2].score(), 50);

        assert!(!backends[0].is_persistent());
        assert!(backends[1].is_persistent());
        assert!(!backends[2].is_persistent());

        assert_eq!(backends[0].backend_name(), "test_memory");
        assert_eq!(backends[1].backend_name(), "test_persistent");
        assert_eq!(backends[2].backend_name(), "unknown");
    }

    #[test]
    fn test_backend_score_sorting_by_score() {
        // 测试按分数排序
        let mut backends: Vec<Box<dyn BackendScore>> = vec![
            Box::new(TestPersistentBackend),  // 70
            Box::new(TestMemoryBackend),      // 100
            Box::new(TestDefaultNameBackend), // 50
        ];

        backends.sort_by(|a, b| b.score().cmp(&a.score()));

        assert_eq!(backends[0].score(), 100);
        assert_eq!(backends[1].score(), 70);
        assert_eq!(backends[2].score(), 50);
    }
}
