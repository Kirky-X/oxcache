// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// 故障恢复集成测试 - 使用新API

use common::setup_logging;
use oxcache::Cache;

use crate::common;

/// 测试降级逻辑
///
/// 验证当L2缓存不可用时，系统能否正确报告错误
#[tokio::test]
async fn test_degradation_logic() {
    setup_logging();
    let redis_url = "redis://127.0.0.1:9999"; // 无效端口

    // 尝试创建到无效Redis的连接
    let cache_result: Result<Cache<String, String>, oxcache::CacheError> =
        Cache::tiered(100, redis_url).await;

    // 应该返回错误（因为Redis不可用）
    // 注意：新API在连接失败时会返回错误，而不是降级
    assert!(cache_result.is_err(), "应该无法连接到无效的Redis");

    let error = cache_result.unwrap_err();
    println!("Expected connection error: {:?}", error);
}
