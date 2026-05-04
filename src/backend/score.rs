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

impl Scores {
    /// 获取分数描述
    ///
    /// # Arguments
    ///
    /// * `score` - 分数值
    ///
    /// # Returns
    ///
    /// 分数对应的描述字符串
    pub fn describe(score: u8) -> &'static str {
        match score {
            90..=100 => "L1 内存缓存",
            70..=89 => "本地持久化缓存",
            40..=69 => "分布式缓存",
            1..=39 => "低速存储",
            _ => "未知类型",
        }
    }

    /// 检查分数是否有效
    ///
    /// # Arguments
    ///
    /// * `score` - 分数值
    ///
    /// # Returns
    ///
    /// 分数是否在有效范围内 (1-100)
    pub fn is_valid(score: u8) -> bool {
        score > 0 && score <= 100
    }

    /// 比较两个后端的分数
    ///
    /// # Arguments
    ///
    /// * `a` - 第一个后端
    /// * `b` - 第二个后端
    ///
    /// # Returns
    ///
    /// `std::cmp::Ordering` 结果
    pub fn compare<A: BackendScore, B: BackendScore>(a: &A, b: &B) -> std::cmp::Ordering {
        a.score().cmp(&b.score())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scores_constants() {
        assert_eq!(Scores::MOKA, 100);
        assert_eq!(Scores::DASHMAP, 90);
        assert_eq!(Scores::LMDB, 85);
        assert_eq!(Scores::SQLITE, 70);
        assert_eq!(Scores::REDIS, 50);
        assert_eq!(Scores::MEMCACHED, 40);
    }

    #[test]
    fn test_scores_describe() {
        assert_eq!(Scores::describe(100), "L1 内存缓存");
        assert_eq!(Scores::describe(90), "L1 内存缓存");
        assert_eq!(Scores::describe(85), "本地持久化缓存");
        assert_eq!(Scores::describe(70), "本地持久化缓存");
        assert_eq!(Scores::describe(50), "分布式缓存");
        assert_eq!(Scores::describe(40), "分布式缓存");
        assert_eq!(Scores::describe(20), "低速存储");
        assert_eq!(Scores::describe(0), "未知类型");
    }

    #[test]
    fn test_scores_is_valid() {
        assert!(Scores::is_valid(1));
        assert!(Scores::is_valid(50));
        assert!(Scores::is_valid(100));
        assert!(!Scores::is_valid(0));
        assert!(!Scores::is_valid(101));
    }

    struct TestBackend {
        score_value: u8,
        persistent: bool,
    }

    impl BackendScore for TestBackend {
        fn score(&self) -> u8 {
            self.score_value
        }

        fn is_persistent(&self) -> bool {
            self.persistent
        }

        fn backend_name(&self) -> &'static str {
            "test"
        }
    }

    #[test]
    fn test_backend_score_trait() {
        let backend = TestBackend {
            score_value: 75,
            persistent: true,
        };

        assert_eq!(backend.score(), 75);
        assert!(backend.is_persistent());
        assert_eq!(backend.backend_name(), "test");
    }

    #[test]
    fn test_scores_compare() {
        let high = TestBackend {
            score_value: 100,
            persistent: false,
        };
        let low = TestBackend {
            score_value: 50,
            persistent: true,
        };

        assert_eq!(Scores::compare(&high, &low), std::cmp::Ordering::Greater);
        assert_eq!(Scores::compare(&low, &high), std::cmp::Ordering::Less);
        assert_eq!(Scores::compare(&high, &high), std::cmp::Ordering::Equal);
    }
}
