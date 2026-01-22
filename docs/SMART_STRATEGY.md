# 智能策略详细说明

## 概述

Oxcache 的智能策略（Smart Strategy）是一个自适应的缓存优化系统，能够根据实际运行情况自动调整缓存策略，包括智能预取（Intelligent Prefetching）和自适应压缩（Adaptive Compression）。该功能可以显著提升缓存命中率和降低内存占用。

### 核心特性

- ✅ **智能预取**：根据访问模式自动预取可能需要的缓存项
- ✅ **自适应压缩**：根据数据大小和压缩比自动决定是否压缩
- ✅ **命中率统计**：实时监控缓存命中率，动态调整策略
- ✅ **自动学习**：通过机器学习算法优化预取和压缩决策
- ✅ **性能监控**：提供详细的性能指标和建议

## 工作原理

### 智能预取（Intelligent Prefetching）

```
预取流程：

1. 用户请求 key1 → 缓存命中
   ↓
2. 分析访问模式
   ↓
3. 识别相关联的 key（key2, key3）
   ↓
4. 预取 key2, key3 到 L1 缓存
   ↓
5. 用户后续请求 key2, key3 → L1 命中
```

### 自适应压缩（Adaptive Compression）

```
压缩决策流程：

1. 数据写入缓存
   ↓
2. 检查数据大小
   ↓
3. 如果大小 > 阈值：
   - 尝试压缩
   - 计算压缩比
   - 如果压缩比 > 阈值：使用压缩版本
   - 否则：使用原始版本
   ↓
4. 如果大小 <= 阈值：不压缩
```

## 使用方式

### 基础用法

```rust
use oxcache::smart_strategy::{SmartStrategyConfig, SmartStrategyManager};

// 创建智能策略管理器
let config = SmartStrategyConfig {
    prefetch_enabled: true,
    prefetch_threshold: 0.7,      // 命中率低于 70% 时触发预取
    prefetch_window_size: 500,    // 预取窗口大小
    prefetch_batch_size: 20,      // 每次预取的数量
    compression_enabled: true,
    compression_threshold: 1024,  // 超过 1KB 才考虑压缩
    min_compression_ratio: 0.7,   // 压缩比必须 > 70%
    compression_sample_rate: 0.2,  // 20% 的数据用于采样
};

let manager = SmartStrategyManager::new(Some(config));

// 记录缓存访问
manager.record_access(true);  // 命中
manager.record_access(false); // 未命中

// 获取统计信息
let stats = manager.hit_rate_stats();
println!("命中率: {:.1}%", stats.hit_rate * 100.0);
```

### 与缓存集成

```rust
use oxcache::{Cache, CacheOps};
use oxcache::smart_strategy::{SmartStrategyConfig, SmartStrategyManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6yncache").await?;
    
    // 创建智能策略管理器
    let config = SmartStrategyConfig {
        prefetch_enabled: true,
        prefetch_threshold: 0.7,
        compression_enabled: true,
        compression_threshold: 1024,
        min_compression_ratio: 0.7,
        ..Default::default()
    };
    let manager = SmartStrategyManager::new(Some(config));
    
    // 查询用户
    let user_id = "user:123";
    
    // 记录访问
    if let Some(user) = cache.get(user_id).await? {
        manager.record_access(true);  // 命中
        
        // 检查是否需要预取
        if manager.should_prefetch() {
            // 预取相关用户
            let related_users = get_related_users(123).await?;
            for related_id in related_users {
                let key = format!("user:{}", related_id);
                if cache.get(&key).await?.is_none() {
                    // 预取到缓存
                    let user = database::query_user(related_id).await?;
                    cache.set(&key, &user, Some(3600)).await?;
                }
            }
        }
        
        return Ok(user);
    }
    
    manager.record_access(false);  // 未命中
    
    // 查询数据库
    let user = database::query_user(123).await?;
    
    // 写入缓存时应用压缩策略
    let serialized = serde_json::to_vec(&user)?;
    if manager.should_compress(&serialized) {
        let compressed = compress(&serialized)?;
        cache.set_raw(user_id, &compressed, Some(3600)).await?;
    } else {
        cache.set(user_id, &user, Some(3600)).await?;
    }
    
    Ok(user)
}
```

### 使用 #[cached] 宏

```rust
use oxcache::cached;
use oxcache::smart_strategy::{SmartStrategyConfig, SmartStrategyManager};

// 创建智能策略管理器
let manager = SmartStrategyManager::new(Some(SmartStrategyConfig {
    prefetch_enabled: true,
    compression_enabled: true,
    ..Default::default()
}));

// 带智能策略的缓存函数
#[cached(service = "user_cache", ttl = 3600)]
async fn get_user(user_id: u64) -> Result<User, String> {
    let key = format!("user:{}", user_id);
    
    // 记录访问
    manager.record_access(true);
    
    // 检查是否需要预取
    if manager.should_prefetch() {
        // 异步预取相关数据
        tokio::spawn(async move {
            prefetch_related_users(user_id).await;
        });
    }
    
    // 继续正常查询流程
    database::query_user(user_id).await
}
```

## 配置参数

### SmartStrategyConfig

```rust
pub struct SmartStrategyConfig {
    /// 是否启用预取功能
    pub prefetch_enabled: bool,
    
    /// 预取阈值（命中率低于此值时触发预取）
    pub prefetch_threshold: f64,
    
    /// 预取窗口大小
    pub prefetch_window_size: usize,
    
    /// 预取批次大小
    pub prefetch_batch_size: usize,
    
    /// 是否启用压缩功能
    pub compression_enabled: bool,
    
    /// 压缩阈值（数据大小超过此值才考虑压缩）
    pub compression_threshold: usize,
    
    /// 最小压缩比（压缩比必须大于此值才使用压缩）
    pub min_compression_ratio: f64,
    
    /// 压缩采样率（用于学习最优压缩策略）
    pub compression_sample_rate: f64,
}
```

### 参数选择指南

#### prefetch_threshold（预取阈值）

| 场景 | 推荐值 | 说明 |
|------|--------|------|
| 高命中率场景 | 0.8-0.9 | 只在命中率显著下降时预取 |
| 低命中率场景 | 0.5-0.7 | 积极预取，提升命中率 |
| 动态场景 | 0.7 | 平衡预取和资源消耗 |

#### compression_threshold（压缩阈值）

| 场景 | 推荐值 | 说明 |
|------|--------|------|
| 小数据为主 | 512-1024 字节 | 只压缩较大的数据 |
| 大数据为主 | 2048-4096 字节 | 压缩更多数据 |
| 内存受限 | 512 字节 | 积极压缩节省内存 |

#### min_compression_ratio（最小压缩比）

| 场景 | 推荐值 | 说明 |
|------|--------|------|
| 网络受限 | 0.5 | 即使压缩比不高也压缩 |
| CPU 受限 | 0.8 | 只压缩效果好的数据 |
| 平衡场景 | 0.7 | 平衡 CPU 和内存 |

## 高级用法

### 自定义预取策略

```rust
use oxcache::smart_strategy::{SmartStrategyConfig, SmartStrategyManager};

let config = SmartStrategyConfig {
    prefetch_enabled: true,
    prefetch_threshold: 0.7,
    prefetch_window_size: 1000,
    prefetch_batch_size: 50,
    compression_enabled: true,
    ..Default::default()
};

let manager = SmartStrategyManager::new(Some(config));

// 自定义预取逻辑
manager.set_prefetch_strategy(|key, access_pattern| {
    // 根据访问模式决定预取哪些键
    if access_pattern.sequential {
        // 顺序访问：预取下一个键
        let next_key = format!("user:{}", extract_id(key) + 1);
        vec![next_key]
    } else if access_pattern.random {
        // 随机访问：预取热门键
        vec!["user:1".to_string(), "user:2".to_string()]
    } else {
        vec![]
    }
}).await?;
```

### 自定义压缩策略

```rust
use oxcache::smart_strategy::{SmartStrategyConfig, SmartStrategyManager};

let config = SmartStrategyConfig {
    compression_enabled: true,
    compression_threshold: 1024,
    min_compression_ratio: 0.7,
    compression_sample_rate: 0.2,
    ..Default::default()
};

let manager = SmartStrategyManager::new(Some(config));

// 自定义压缩决策
manager.set_compression_strategy(|data| {
    let size = data.len();
    
    // 小于阈值：不压缩
    if size < 1024 {
        return CompressionDecision::NoCompression;
    }
    
    // 尝试压缩
    let compressed = compress(data)?;
    let ratio = compressed.len() as f64 / size as f64;
    
    // 压缩比足够：使用压缩
    if ratio < 0.7 {
        CompressionDecision::Compressed(compressed)
    } else {
        CompressionDecision::NoCompression
    }
}).await?;
```

### 分层智能策略

```rust
// L1 层：积极预取，积极压缩
let l1_config = SmartStrategyConfig {
    prefetch_enabled: true,
    prefetch_threshold: 0.5,  // 较低的阈值
    compression_enabled: true,
    compression_threshold: 512,   // 较低的阈值
    min_compression_ratio: 0.5,  // 较低的压缩比
    ..Default::default()
};

// L2 层：保守预取，保守压缩
let l2_config = SmartStrategyConfig {
    prefetch_enabled: true,
    prefetch_threshold: 0.8,  // 较高的阈值
    compression_enabled: true,
    compression_threshold: 2048,  // 较高的阈值
    min_compression_ratio: 0.8,   // 较高的压缩比
    ..Default::default()
};

let l1_manager = SmartStrategyManager::new(Some(l1_config));
let l2_manager = SmartStrategyManager::new(Some(l2_config));
```

## 性能优化

### 性能监控

```rust
use oxcache::smart_strategy::SmartStrategyManager;

let manager = SmartStrategy::new(None);

// 获取性能指标
let metrics = manager.get_metrics()?;

println!("性能指标：");
println!("  预取次数: {}", metrics.prefetch_count);
println!("  预取命中率: {:.2}%", metrics.prefetch_hit_rate * 100.0);
println!("  压缩次数: {}", metrics.compression_count);
println!("  平均压缩比: {:.2}%", metrics.avg_compression_ratio * 100.0);
println!("  节省的内存: {} bytes", metrics.memory_saved);
println!("  CPU 时间开销: {} ms", metrics.cpu_time_ms);
```

### 自适应调整

```rust
// 根据性能指标自动调整策略
let metrics = manager.get_metrics()?;

if metrics.prefetch_hit_rate < 0.3 {
    // 预取效果不好，降低预取频率
    manager.adjust_prefetch_strategy(|config| {
        config.prefetch_threshold = 0.9;  // 提高阈值，减少预取
        config.prefetch_batch_size = 10;  // 减少批次大小
    }).await?;
} else if metrics.prefetch_hit_rate > 0.7 {
    // 预取效果好，增加预取频率
    manager.adjust_prefetch_strategy(|config| {
        config.prefetch_threshold = 0.6;  // 降低阈值，增加预取
        config.prefetch_batch_size = 30;  // 增加批次大小
    }).await?;
}
```

## 最佳实践

### ✅ 推荐做法

1. **合理设置阈值**：根据实际业务场景设置合理的预取和压缩阈值
2. **监控性能指标**：定期检查智能策略的性能指标
3. **自适应调整**：根据实际效果动态调整策略参数
4. **分层策略**：为 L1 和 L2 层设置不同的策略
5. **A/B 测试**：通过 A/B 测试找到最优配置

### ❌ 避免做法

1. **阈值设置不合理**：不要设置过高或过低的阈值
2. **忽略性能指标**：不要忽略智能策略的性能指标
3. **固定策略**：不要使用固定策略，无法适应变化
4. **过度优化**：不要过度优化，增加复杂度
5. **忽略成本**：不要忽略预取和压缩的 CPU 和内存成本

## 完整示例

```rust
use oxcache::{Cache, CacheOps};
use oxcache::smart_strategy::{SmartStrategyConfig, SmartStrategyManager};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

async fn get_user_with_smart_strategy(
    cache: &Cache<String, User>,
    manager: &SmartStrategyManager,
    user_id: u64,
) -> Result<Option<User>, Box<dyn std::error::Error>> {
    let key = format!("user:{}", user_id);
    
    // 1. 查询缓存
    if let Some(user) = cache.get(&key).await? {
        // 记录命中
        manager.record_access(true);
        
        println!("💾 缓存命中：user_id={}", user_id);
        
        // 检查是否需要预取
        if manager.should_prefetch() {
            println!("🔄 触发智能预取...");
            
            // 获取相关用户 ID
            let related_ids = get_related_user_ids(user_id).await?;
            
            // 预取相关用户
            let mut prefetch_count = 0;
            for related_id in related_ids {
                let related_key = format!("user:{}", related_id);
                
                // 只预取不在缓存中的用户
                if cache.get(&related_key).await?.is_none() {
                    if let Some(related_user) = database::query_user(related_id).await? {
                        // 应用压缩策略
                        let should_compress = manager.should_compress(
                            &serde_json::to_vec(&related_user)?
                        );
                        
                        if should_compress {
                            let compressed = compress(&serde_json::to_vec(&related_user)?)?;
                            cache.set_raw(&related_key, &compressed, Some(3600)).await?;
                        } else {
                            cache.set(&related_key, &related_user, Some(3600)).await?;
                        }
                        
                        prefetch_count += 1;
                    }
                }
            }
            
            println!("   预取了 {} 个相关用户", prefetch_count);
        }
        
        return Ok(Some(user));
    }
    
    // 记录未命中
    manager.record_access(false);
    println!("🔍 缓存未命中：user_id={}", user_id);
    
    // 查询数据库
    let user = database::query_user(user_id).await?;
    
    // 写入缓存时应用压缩策略
    let serialized = serde_json::to_vec(&user)?;
    if manager.should_compress(&serialized) {
        println!("🗜️  应用压缩策略");
        let compressed = compress(&serialized)?;
        cache.set_raw(&key, &compressed, Some(3600)).await?;
    } else {
        cache.set(&key, &user, Some(3600)).await?;
    }
    
    println!("✅ 数据库查询成功，已更新缓存");
    
    Ok(Some(user))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 智能策略使用示例 ===\n");
    
    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
    
    // 创建智能策略管理器
    let config = SmartStrategyConfig {
        prefetch_enabled: true,
        prefetch_threshold: 0.7,
        prefetch_window_size: 500,
        prefetch_batch_size: 20,
        compression_enabled: true,
        compression_threshold: 1024,
        min_compression_ratio: 0.7,
        compression_sample_rate: 0.2,
    };
    let manager = SmartStrategyManager::new(Some(config));
    
    println!("1. 智能策略配置：");
    println!("   预取功能: {}", config.prefetch_enabled);
    println!("   预取阈值: {:.0}%", config.prefetch_threshold * 100.0);
    println!("   压缩功能: {}", config.compression_enabled);
    println!("   压缩阈值: {} bytes", config.compression_threshold);
    println!("   最小压缩比: {:.0}%", config.min_compression_ratio * 100.0);
    println!();
    
    // 模拟访问模式
    println!("2. 模拟访问模式...");
    for i in 1..=10 {
        let result = get_user_with_smart_strategy(&cache, &manager, i).await?;
        println!("   用户 {}: {:?}", i, result.as_ref().map(|u| u.name.clone()));
    }
    println!();
    
    // 显示统计信息
    println!("3. 智能策略统计：");
    let stats = manager.hit_rate_stats();
    println!("   命中率: {:.1}%", stats.hit_rate * 100.0);
    println!("   总命中: {}", stats.total_hits);
    println!("   总未命中: {}", stats.total_misses);
    
    // 显示性能指标
    let metrics = manager.get_metrics()?;
    println!("   预取次数: {}", metrics.prefetch_count);
    println!("   预取命中率: {:.2}%", metrics.prefetch_hit_rate * 100.0);
    println!("   压缩次数: {}", metrics.compression_count);
    println!("   平均压缩比: {:.2}%", metrics.avg_compression_ratio * 100.0);
    println!("   节省的内存: {} bytes", metrics.memory_saved);
    
    Ok(())
}
```

## 故障排除

### 问题：预取命中率低

**原因**：
- 预取阈值设置过高
- 预取窗口大小不合适
- 访问模式难以预测

**解决方案**：
1. 降低预取阈值（如从 0.8 降到 0.6）
2. 调整预取窗口大小
3. 分析访问模式，优化预取策略

### 问题：压缩效果差

**原因**：
- 压缩阈值设置过低
- 最小压缩比设置过高
- 数据本身难以压缩

**解决方案**：
1. 提高压缩阈值（如从 512 提高到 2048）
2. 降低最小压缩比（如从 0.8 降到 0.6）
3. 跳过难以压缩的数据类型

### 问题：CPU 开销过高

**原因**：
- 预取频率过高
- 压缩采样率过高
- 策略过于激进

**解决方案**：
1. 提高预取阈值，减少预取频率
2. 降低压缩采样率
3. 使用更保守的策略参数

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [性能优化指南](README.md#-performance-optimization)

## 示例代码

- `examples/src/smart_strategy.rs` - 智能策略完整示例
- `examples/src/06_features/example_metrics.rs` - 指标监控示例
- `src/smart_strategy.rs` - 智能策略实现