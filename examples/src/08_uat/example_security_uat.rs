//! 安全 UAT 测试示例
//!
//! 本示例展示安全相关的用户验收测试 (UAT) 场景。
//!
//! 运行方式：
//! ```bash
//! cd examples && cargo run --example example_security_uat
//!

use oxcache::Cache;

// 测试 1: 数据隔离
async fn test_data_isolation() -> Result<(), Box<dyn std::error::Error>> {
    println!("   安全测试 1: 数据隔离...");

    let cache1: Cache<String, String> = Cache::new().await?;
    let cache2: Cache<String, String> = Cache::new().await?;

    // 在 cache1 中添加数据
    cache1.set("shared:key", "cache1_value", None).await?;

    // cache2 应该无法访问 cache1 的数据
    let retrieved = cache2.get("shared:key").await?;
    assert!(retrieved.is_none(), "不同缓存实例应该数据隔离");

    // cache1 应该能访问自己的数据
    let retrieved = cache1.get("shared:key").await?;
    assert!(retrieved.is_some(), "缓存应该能访问自己的数据");
    assert_eq!(retrieved.unwrap(), "cache1_value", "数据值应该正确");

    // 清理
    cache1.clear().await?;
    println!("   ✓ 数据隔离测试通过");

    Ok(())
}

// 测试 2: 访问控制模拟
async fn test_access_control() -> Result<(), Box<dyn std::error::Error>> {
    println!("   安全测试 2: 访问控制模拟...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 模拟敏感数据
    cache
        .set("user:1:password", "secret_password", Some(300))
        .await?;
    cache
        .set("user:1:token", "access_token_xyz", Some(3600))
        .await?;
    cache.set("public:config", "config_value", None).await?;

    // 验证数据存在
    assert!(cache.get("user:1:password").await?.is_some());
    assert!(cache.get("user:1:token").await?.is_some());
    assert!(cache.get("public:config").await?.is_some());

    // 清理敏感数据
    cache.delete("user:1:password").await?;
    cache.delete("user:1:token").await?;

    // 验证清理
    assert!(cache.get("user:1:password").await?.is_none());
    assert!(cache.get("user:1:token").await?.is_none());
    assert!(cache.get("public:config").await?.is_some());

    // 清理公共数据
    cache.delete("public:config").await?;

    println!("   ✓ 访问控制测试通过");
    Ok(())
}

// 测试 3: 审计日志模拟
async fn test_audit_log() -> Result<(), Box<dyn std::error::Error>> {
    println!("   安全测试 3: 审计日志模拟...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 模拟审计日志记录
    let operations = vec![
        ("audit:1", "用户登录"),
        ("audit:2", "数据查询"),
        ("audit:3", "配置更改"),
        ("audit:4", "用户登出"),
    ];

    for (key, desc) in &operations {
        cache.set(key, desc, Some(86400)).await?; // 保留 24 小时
    }

    // 验证所有审计日志记录
    for (key, desc) in &operations {
        let retrieved = cache.get(key).await?;
        assert!(retrieved.is_some(), "审计日志应该存在");
        assert_eq!(retrieved.unwrap(), *desc, "审计日志内容应该匹配");
    }

    // 清理
    cache.clear().await?;
    println!("   ✓ 审计日志测试通过");

    Ok(())
}

// 测试 4: 加密数据存储模拟
async fn test_encrypted_storage() -> Result<(), Box<dyn std::error::Error>> {
    println!("   安全测试 4: 加密数据存储模拟...");

    let cache: Cache<String, String> = Cache::new().await?;

    // 模拟加密数据
    let sensitive_data = vec![
        ("encrypted:credit_card", "4111111111111111"),
        ("encrypted:ssn", "123-45-6789"),
        ("encrypted:api_key", "sk_live_abcdefg"),
    ];

    for (key, value) in &sensitive_data {
        cache.set(key, value, Some(300)).await?;
    }

    // 验证数据存在
    for (key, value) in &sensitive_data {
        let retrieved = cache.get(key).await?;
        assert!(retrieved.is_some(), "加密数据应该存在");
        assert_eq!(retrieved.unwrap(), *value, "加密数据值应该正确");
    }

    // 清理
    cache.clear().await?;
    println!("   ✓ 加密数据存储测试通过");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 安全 UAT 测试示例 ===\n");

    println!("运行安全测试...\n");

    test_data_isolation().await?;
    test_access_control().await?;
    test_audit_log().await?;
    test_encrypted_storage().await?;

    println!();
    println!("=== 所有安全 UAT 测试通过 ===");
    Ok(())
}
