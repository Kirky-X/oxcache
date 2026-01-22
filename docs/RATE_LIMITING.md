# 速率限制使用指南

## 概述

速率限制（Rate Limiting）是一种保护缓存系统免受 DoS 攻击和滥用的重要机制。Oxcache 提供了基于令牌桶算法的速率限制功能，可以有效防止恶意请求消耗系统资源。

### 核心特性

- ✅ **防止 DoS 攻击**：限制每秒请求数，防止系统过载
- ✅ **令牌桶算法**：平滑限流，支持突发流量
- ✅ **灵活配置**：可针对不同用户/服务设置不同限制
- ✅ **自动恢复**：限流后自动恢复访问权限
- ✅ **分布式支持**：基于 Redis 实现分布式限流
- ✅ **监控指标**：提供详细的限流统计信息

## 工作原理

### 令牌桶算法

```
令牌桶模型：

┌─────────────────────────────┐
│        Token Bucket          │
│  [🪙] [🪙] [🪙] [🪙] [ ]   │  ← 令牌（每秒补充）
└─────────────────────────────┘
         ↓
         ↓ 每个请求消耗 1 个令牌
         ↓
    ┌─────────┐
    │ Request │
    └─────────┘

- 桶容量：突发容量（burst_capacity）
- 填充速率：每秒补充的令牌数（max_requests_per_second）
- 消耗：每个请求消耗 1 个令牌
- 拒绝：令牌不足时拒绝请求
```

### 限流流程

```
1. 用户请求到达
   ↓
2. 检查令牌桶是否有令牌
   ↓
3. 有令牌：
   - 消耗 1 个令牌
   - 允许请求
   ↓
4. 无令牌：
   - 拒绝请求
   - 记录限流事件
   - 返回 429 Too Many Requests
```

## 使用方式

### 基础用法

```rust
use oxcache::rate_limiting::{RateLimitConfig, GlobalRateLimiter};

// 创建速率限制配置
let config = RateLimitConfig {
    max_requests_per_second: 1000,  // 每秒最多 1000 个请求
    burst_capacity: 2000,            // 突发容量 2000
    block_duration_secs: 10,         // 限流后阻塞 10 秒
};

// 创建全局速率限制器
let limiter = GlobalRateLimiter::new(Some(config));

// 检查并限流
if limiter.check("user:123").await? {
    // 允许请求
    process_request("user:123").await?;
} else {
    // 请求被限流
    return Err(Error::RateLimitExceeded);
}
```

### 与缓存集成

```rust
use oxcache::{Cache, CacheOps};
use oxcache::rate_limiting::{RateLimitConfig, GlobalRateLimiter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
    
    // 创建速率限制器
    let config = RateLimitConfig {
        max_requests_per_second: 1000,
        burst_capacity: 2000,
        block_duration_secs: 10,
    };
    let limiter = GlobalRateLimiter::new(Some(config));
    
    // 处理请求
    let user_id = "user:123";
    
    // 检查速率限制
    if !limiter.check(user_id).await? {
        return Err("请求过于频繁，请稍后再试".into());
    }
    
    // 查询缓存
    if let Some(user) = cache.get(user_id).await? {
        return Ok(user);
    }
    
    // 查询数据库
    let user = database::query_user(123).await?;
    
    // 更新缓存
    cache.set(user_id, &user, Some(3600)).await?;
    
    Ok(user)
}
```

### 使用 #[cached] 宏

```rust
use oxcache::cached;
use oxcache::rate_limiting::{RateLimitConfig, GlobalRateLimiter};

// 创建速率限制器
let limiter = GlobalRateLimiter::new(Some(RateLimitConfig {
    max_requests_per_second: 1000,
    burst_capacity: 2000,
    block_duration_secs: 10,
}));

// 带速率限制的缓存函数
#[cached(service = "user_cache", ttl = 3600)]
async fn get_user(user_id: u64) -> Result<User, String> {
    let key = format!("user:{}", user_id);
    
    // 检查速率限制
    if !limiter.check(&key).await.map_err(|e| e.to_string())? {
        return Err("请求过于频繁".to_string());
    }
    
    // 继续正常查询流程
    database::query_user(user_id).await
}
```

## 配置参数

### RateLimitConfig

```rust
pub struct RateLimitConfig {
    /// 每秒最大请求数
    pub max_requests_per_second: u64,
    /// 突发容量（令牌桶容量）
    pub burst_capacity: u64,
    /// 限流后阻塞时长（秒）
    pub block_duration_secs: u64,
}
```

### 参数选择指南

#### max_requests_per_second（每秒最大请求数）

| 场景 | 推荐值 | 说明 |
|------|--------|------|
| 公共 API | 100-1000 | 平衡性能和可用性 |
| 内部服务 | 1000-10000 | 较高的并发限制 |
| 敏感操作 | 10-100 | 严格的限流 |
| 批量操作 | 1-10 | 防止批量滥用 |

#### burst_capacity（突发容量）

- **建议值**：`max_requests_per_second` 的 2-5 倍
- **作用**：允许短时间内的突发流量
- **示例**：如果每秒 1000 个请求，突发容量可设为 2000-5000

#### block_duration_secs（阻塞时长）

| 场景 | 推荐值 | 说明 |
|------|--------|------|
| 一般限流 | 10-60 秒 | 给用户合理的等待时间 |
| 严格限流 | 60-300 秒 | 对恶意请求更严厉 |
| 轻度限流 | 1-5 秒 | 快速恢复访问 |

## 高级用法

### 分层速率限制

```rust
use oxcache::rate_limiting::{RateLimitConfig, GlobalRateLimiter};

// 全局限流：所有用户共享
let global_limiter = GlobalRateLimiter::new(Some(RateLimitConfig {
    max_requests_per_second: 10000,
    burst_capacity: 20000,
    block_duration_secs: 10,
}));

// 用户级限流：每个用户独立限制
let user_limiter = GlobalRateLimiter::new(Some(RateLimitConfig {
    max_requests_per_second: 100,
    burst_capacity: 200,
    block_duration_secs: 30,
}));

// API 级限流：每个 API 端点独立限制
let api_limiter = GlobalRateLimiter::new(Some(RateLimitConfig {
    max_requests_per_second: 50,
    burst_capacity: 100,
    block_duration_secs: 60,
}));

// 检查多层限流
let user_id = "user:123";
let api_endpoint = "api:get_user";

if !global_limiter.check("global").await? {
    return Err("全局限流".into());
}

if !user_limiter.check(user_id).await? {
    return Err("用户限流".into());
}

if !api_limiter.check(api_endpoint).await? {
    return Err("API 限流".into());
}

// 所有检查通过，处理请求
process_request().await
```

### 动态调整限制

```rust
use oxcache::rate_limiting::{RateLimitConfig, GlobalRateLimiter};

let limiter = GlobalRateLimiter::new(Some(RateLimitConfig {
    max_requests_per_second: 1000,
    burst_capacity: 2000,
    block_duration_secs: 10,
}));

// 根据系统负载动态调整
let system_load = get_system_load().await?;

if system_load > 0.8 {
    // 高负载：降低限制
    limiter.update_config(RateLimitConfig {
        max_requests_per_second: 500,
        burst_capacity: 1000,
        block_duration_secs: 30,
    }).await?;
} else if system_load < 0.3 {
    // 低负载：提高限制
    limiter.update_config(RateLimitConfig {
        max_requests_per_second: 2000,
        burst_capacity: 4000,
        block_duration_secs: 5,
    }).await?;
}
```

### 限流统计

```rust
use oxcache::rate_limiting::RateLimiterStats;

let stats = limiter.get_stats("user:123").await?;

println!("速率限制统计：");
println!("  允许的请求数: {}", stats.allowed_requests);
println!("  拒绝的请求数: {}", stats.rejected_requests);
println!("  总请求数: {}", stats.total_requests);
println!("  限流率: {:.2}%", stats.rejection_rate * 100.0);
println!("  当前令牌数: {}", stats.current_tokens);
println!("  最后更新时间: {:?}", stats.last_update);
```

### 自定义限流策略

```rust
use oxcache::rate_limiting::{RateLimitConfig, GlobalRateLimiter};

// 为不同用户类型设置不同限制
let normal_user_config = RateLimitConfig {
    max_requests_per_second: 100,
    burst_capacity: 200,
    block_duration_secs: 30,
};

let premium_user_config = RateLimitConfig {
    max_requests_per_second: 1000,
    burst_capacity: 2000,
    block_duration_secs: 5,
};

let admin_user_config = RateLimitConfig {
    max_requests_per_second: 10000,
    burst_capacity: 20000,
    block_duration_secs: 1,
};

// 根据用户类型选择限流配置
let user_type = get_user_type("user:123").await?;
let config = match user_type {
    UserType::Normal => normal_user_config,
    UserType::Premium => premium_user_config,
    UserType::Admin => admin_user_config,
};

let limiter = GlobalRateLimiter::new(Some(config));
```

## 最佳实践

### ✅ 推荐做法

1. **合理设置限制**：根据系统容量和业务需求设置合理的限流参数
2. **分层限流**：使用全局限流、用户级限流、API 级限流多层防护
3. **监控限流**：定期检查限流统计，及时调整参数
4. **优雅降级**：限流时返回友好的错误信息和重试时间
5. **动态调整**：根据系统负载动态调整限流参数

### ❌ 避免做法

1. **限制过严**：不要设置过低的限制，影响正常用户体验
2. **限制过松**：不要设置过高的限制，无法起到防护作用
3. **忽略统计**：不要忽略限流统计信息，无法了解实际效果
4. **单一限流**：不要只使用一层限流，容易被绕过
5. **固定参数**：不要使用固定的限流参数，无法适应变化

## 性能优化

### 分布式限流

```rust
// 使用 Redis 实现分布式限流
// 这样多个实例可以共享限流状态
let limiter = GlobalRateLimiter::new(Some(RateLimitConfig {
    max_requests_per_second: 1000,
    burst_capacity: 2000,
    block_duration_secs: 10,
})).with_redis_client(redis_client)?;
```

### 缓存限流状态

```rust
use oxcache::{Cache, CacheOps};

// 缓存限流状态，减少 Redis 查询
let cache: Cache<String, bool> = Cache::memory().await?;

let key = format!("rate_limit:{}", user_id);

// 先检查缓存
if let Some(blocked) = cache.get(&key).await? {
    if blocked {
        return Err("用户已被限流".into());
    }
}

// 检查速率限制
if !limiter.check(user_id).await? {
    // 更新缓存
    cache.set(&key, &true, Some(10)).await?;
    return Err("请求被限流".into());
}
```

### 异步限流检查

```rust
use oxcache::rate_limiting::GlobalRateLimiter;

// 批量检查多个用户的限流状态
let user_ids = vec!["user:1", "user:2", "user:3"];
let results = limiter.check_batch(&user_ids).await?;

for (user_id, allowed) in results {
    if allowed {
        process_request(user_id).await?;
    } else {
        println!("用户 {} 被限流", user_id);
    }
}
```

## 监控与调试

### 获取统计信息

```rust
let stats = limiter.get_stats("user:123").await?;

println!("速率限制统计：");
println!("  允许的请求: {}", stats.allowed_requests);
println!("  拒绝的请求: {}", stats.rejected_requests);
println!("  拒绝率: {:.2}%", stats.rejection_rate * 100.0);
println!("  当前令牌: {}", stats.current_tokens);
println!("  最后更新: {:?}", stats.last_update);
```

### 监控限流事件

```rust
use oxcache::rate_limiting::RateLimitEvent;

// 订阅限流事件
limiter.on_event(|event| async move {
    match event {
        RateLimitEvent::RequestRejected { key, reason } => {
            eprintln!("⚠️  请求被拒绝: key={}, reason={}", key, reason);
        }
        RateLimitEvent::BucketRefilled { key, tokens_added } => {
            println!("🪙  令牌桶已填充: key={}, tokens={}", key, tokens_added);
        }
    }
}).await?;
```

### 调试限流问题

```rust
// 获取详细的调试信息
let debug_info = limiter.get_debug_info("user:123").await?;

println!("调试信息：");
println!("  当前令牌数: {}", debug_info.current_tokens);
println!("  令牌桶容量: {}", debug_info.bucket_capacity);
println!("  填充速率: {} tokens/sec", debug_info.refill_rate);
println!("  最后填充时间: {:?}", debug_info.last_refill);
println!("  被阻塞时长: {:?}", debug_info.blocked_since);
```

## 完整示例

```rust
use oxcache::{Cache, CacheOps};
use oxcache::rate_limiting::{RateLimitConfig, GlobalRateLimiter};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

async fn get_user_with_rate_limit(
    cache: &Cache<String, User>,
    limiter: &GlobalRateLimiter,
    user_id: u64,
) -> Result<Option<User>, Box<dyn std::error::Error>> {
    let key = format!("user:{}", user_id);
    
    // 1. 检查速率限制
    if !limiter.check(&key).await? {
        println!("⚠️  请求被限流：user_id={}", user_id);
        
        // 获取统计信息
        let stats = limiter.get_stats(&key).await?;
        println!("   限流统计：允许={}, 拒绝={}, 拒绝率={:.2}%",
            stats.allowed_requests,
            stats.rejected_requests,
            stats.rejection_rate * 100.0
        );
        
        return Ok(None);
    }
    
    println!("✅ 请求通过速率限制检查：user_id={}", user_id);
    
    // 2. 查询缓存
    if let Some(user) = cache.get(&key).await? {
        println!("💾 缓存命中：user_id={}", user_id);
        return Ok(Some(user));
    }
    
    println!("🔍 缓存未命中，查询数据库");
    
    // 3. 查询数据库
    let user = database::query_user(user_id).await?;
    
    // 4. 更新缓存
    cache.set(&key, &user, Some(3600)).await?;
    
    println!("✅ 数据库查询成功，已更新缓存");
    
    Ok(Some(user))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 速率限制使用示例 ===\n");
    
    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
    
    // 创建速率限制器
    let config = RateLimitConfig {
        max_requests_per_second: 100,  // 每秒 100 个请求
        burst_capacity: 200,            // 突发容量 200
        block_duration_secs: 10,         // 限流 10 秒
    };
    let limiter = GlobalRateLimiter::new(Some(config));
    
    println!("1. 速率限制配置：");
    println!("   每秒最大请求: {}", config.max_requests_per_second);
    println!("   突发容量: {}", config.burst_capacity);
    println!("   限流时长: {} 秒", config.block_duration_secs);
    println!();
    
    // 模拟正常请求
    println!("2. 模拟正常请求...");
    for i in 1..=5 {
        let result = get_user_with_rate_limit(&cache, &limiter, i).await?;
        println!("   请求 {}: {:?}", i, result.as_ref().map(|u| u.name.clone()));
    }
    println!();
    
    // 模拟快速请求（触发限流）
    println!("3. 模拟快速请求（触发限流）...");
    for i in 6..=15 {
        let result = get_user_with_rate_limit(&cache, &limiter, i).await?;
        println!("   请求 {}: {:?}", i, result.as_ref().map(|u| u.name.clone()));
    }
    println!();
    
    // 显示统计信息
    println!("4. 速率限制统计：");
    let stats = limiter.get_stats("user:1").await?;
    println!("   允许的请求: {}", stats.allowed_requests);
    println!("   拒绝的请求: {}", stats.rejected_requests);
    println!("   拒绝率: {:.2}%", stats.rejection_rate * 100.0);
    
    Ok(())
}
```

## 故障排除

### 问题：限流过于严格

**原因**：
- `max_requests_per_second` 设置过低
- `burst_capacity` 设置过小
- 系统负载过高，自动降低限制

**解决方案**：
1. 提高 `max_requests_per_second`
2. 增加 `burst_capacity`
3. 检查系统负载，优化性能

### 问题：限流无效

**原因**：
- 限流器未正确初始化
- 限流检查被跳过
- 分布式限流未正确配置 Redis

**解决方案**：
1. 确保限流器正确初始化
2. 检查所有请求路径都经过限流检查
3. 验证 Redis 连接配置

### 问题：限流统计不准确

**原因**：
- 统计信息未正确更新
- 多实例统计未聚合
- 统计信息被重置

**解决方案**：
1. 检查统计更新逻辑
2. 使用分布式统计聚合
3. 避免清空统计信息

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [布隆过滤器指南](BLOOM_FILTER.md)
- [安全文档](SECURITY.md)

## 示例代码

- `examples/src/06_features/example_rate_limiting.rs` - 速率限制完整示例
- `tests/security_tests.rs` - 安全测试
- `src/rate_limiting.rs` - 速率限制实现