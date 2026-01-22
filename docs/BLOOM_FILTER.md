# 布隆过滤器使用指南

## 概述

布隆过滤器（Bloom Filter）是一种空间效率极高的概率型数据结构，用于判断一个元素是否在一个集合中。在缓存系统中，布隆过滤器可以有效防止**缓存穿透**攻击。

### 核心特性

- ✅ **防止缓存穿透**：快速判断数据是否存在，避免无效查询穿透到数据库
- ✅ **高性能**：时间复杂度为 O(k)，k 为哈希函数数量
- ✅ **空间高效**：相比 HashSet，布隆过滤器节省 90%+ 的空间
- ⚠️ **误判率**：存在一定的假阳性（False Positive），但不存在假阴性（False Negative）
- ✅ **可配置**：可以根据预期元素数量和误判率调整参数

## 工作原理

```
查询流程：
1. 用户请求 key → 布隆过滤器
2. 布隆过滤器判断：
   - 不存在 → 直接返回 None（100% 准确）
   - 可能存在 → 查询缓存
3. 缓存未命中 → 查询数据库
4. 数据库返回结果 → 更新缓存 + 布隆过滤器
```

### 误判率说明

- **假阳性（False Positive）**：布隆过滤器说"存在"，但实际不存在
  - 影响：会多查询一次缓存，但不会漏掉有效数据
  - 概率：可配置，通常设置为 1%-5%

- **假阴性（False Negative）**：布隆过滤器说"不存在"，但实际存在
  - 影响：会漏掉有效数据（布隆过滤器不存在这种情况）
  - 概率：0%（布隆过滤器的特性）

## 使用方式

### 基础用法

```rust
use oxcache::bloom_filter::{BloomFilterConfig, BloomFilterOptions};

// 创建布隆过滤器
let options = BloomFilterOptions::new(
    "user_filter".to_string(),  // 过滤器名称
    1_000_000,                   // 预期元素数量
    0.01,                        // 误判率 1%
);

let filter = options.build()?;

// 检查键是否存在
if filter.might_contain("user:123") {
    // 可能存在，继续查询缓存
    let cached = cache.get("user:123").await?;
    if cached.is_none() {
        // 缓存未命中，查询数据库
        let data = database.query("user:123").await?;
        // 更新缓存
        cache.set("user:123", &data, Some(3600)).await?;
        // 更新布隆过滤器
        filter.add("user:123")?;
    }
} else {
    // 确定不存在，直接返回 None
    return Ok(None);
}
```

### 与缓存集成

```rust
use oxcache::{Cache, CacheOps};
use oxcache::bloom_filter::{BloomFilterOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;

    // 创建布隆过滤器
    let filter = BloomFilterOptions::new(
        "user_cache".to_string(),
        1_000_000,
        0.01,
    ).build()?;

    // 查询用户
    let user_id = "user:123";
    
    if filter.might_contain(user_id) {
        // 可能存在，查询缓存
        if let Some(user) = cache.get(user_id).await? {
            return Ok(user);
        }
        
        // 缓存未命中，查询数据库
        let user = database::query_user(123).await?;
        
        // 更新缓存
        cache.set(user_id, &user, Some(3600)).await?;
        
        // 更新布隆过滤器
        filter.add(user_id)?;
        
        Ok(user)
    } else {
        // 确定不存在
        Ok(None)
    }
}
```

### 使用 #[cached] 宏

```rust
use oxcache::cached;
use oxcache::bloom_filter::BloomFilterOptions;

// 创建布隆过滤器
let filter = BloomFilterOptions::new(
    "user_cache".to_string(),
    1_000_000,
    0.01,
).build()?;

// 使用布隆过滤器的缓存函数
#[cached(service = "user_cache", ttl = 3600)]
async fn get_user(user_id: u64) -> Result<User, String> {
    let key = format!("user:{}", user_id);
    
    // 先检查布隆过滤器
    if !filter.might_contain(&key) {
        return Err("User not found".to_string());
    }
    
    // 继续正常查询流程
    database::query_user(user_id).await
}
```

## 配置参数

### BloomFilterOptions

```rust
pub struct BloomFilterOptions {
    /// 过滤器名称
    pub name: String,
    /// 预期元素数量
    pub expected_elements: u64,
    /// 误判率 (0.0 - 1.0)
    pub false_positive_rate: f64,
}
```

### 参数选择指南

#### 预期元素数量 (expected_elements)

- **建议值**：实际存储数据的 1.5-2 倍
- **原因**：预留空间，避免误判率上升
- **示例**：如果存储 100 万个用户，设置为 150-200 万

#### 误判率 (false_positive_rate)

| 误判率 | 适用场景 | 空间开销 |
|-------|---------|---------|
| 0.1% | 高准确性要求 | 较高 |
| 1% | 推荐值，平衡性能和空间 | 中等 |
| 5% | 高性能要求 | 较低 |

#### 计算公式

```
位数组长度 m = -n * ln(p) / (ln(2)^2)
哈希函数数量 k = m / n * ln(2)

其中：
- n = 预期元素数量
- p = 误判率
- m = 位数组长度（bit）
- k = 哈希函数数量
```

### BloomFilterConfig

```rust
pub struct BloomFilterConfig {
    /// 布隆过滤器配置
    pub name: String,
    /// 位数组长度（字节）
    pub bit_array_size: usize,
    /// 哈希函数数量
    pub hash_count: usize,
}
```

## 高级用法

### 自定义哈希函数

```rust
use oxcache::bloom_filter::{BloomFilterConfig, BloomFilter};

let config = BloomFilterConfig {
    name: "custom_filter".to_string(),
    bit_array_size: 10_000_000,  // 10 MB
    hash_count: 3,              // 3 个哈希函数
};

let filter = BloomFilter::new(config)?;
```

### 多个布隆过滤器

```rust
use oxcache::bloom_filter::BloomFilterOptions;

// 用户过滤器
let user_filter = BloomFilterOptions::new(
    "users".to_string(),
    1_000_000,
    0.01,
).build()?;

// 产品过滤器
let product_filter = BloomFilterOptions::new(
    "products".to_string(),
    500_000,
    0.005,
).build()?;

// 订单过滤器
let order_filter = BloomFilterOptions::new(
    "orders".to_string(),
    2_000_000,
    0.02,
).build()?;
```

### 持久化布隆过滤器

```rust
use oxcache::bloom_filter::{BloomFilterOptions, BloomFilter};

// 创建布隆过滤器
let filter = BloomFilterOptions::new(
    "persistent_filter".to_string(),
    1_000_000,
    0.01,
).build()?;

// 添加元素
filter.add("key1")?;
filter.add("key2")?;

// 序列化到文件
let serialized = filter.serialize()?;

// 保存到磁盘
std::fs::write("bloom_filter.dat", serialized)?;

// 从文件加载
let data = std::fs::read("bloom_filter.dat")?;
let loaded_filter = BloomFilter::deserialize(&data)?;
```

## 最佳实践

### ✅ 推荐做法

1. **合理设置参数**：根据实际数据量设置预期元素数量
2. **监控误判率**：定期检查实际误判率，调整参数
3. **定期重建**：对于动态数据，定期重建布隆过滤器
4. **分层使用**：对不同类型的数据使用不同的布隆过滤器
5. **与缓存结合**：布隆过滤器 + 缓存 = 完美的穿透防护

### ❌ 避免做法

1. **误判率过低**：不要设置过低的误判率（如 0.0001%），会浪费大量空间
2. **预期数量过小**：不要设置过小的预期数量，会导致误判率上升
3. **忽略误判**：不要忽略误判的影响，虽然不会漏数据，但会增加缓存查询
4. **单一过滤器**：不要对所有数据使用同一个布隆过滤器
5. **频繁重建**：不要频繁重建布隆过滤器，会增加开销

## 性能优化

### 空间优化

```rust
// 使用较低的误判率节省空间
let filter = BloomFilterOptions::new(
    "space_optimized".to_string(),
    1_000_000,
    0.05,  // 5% 误判率，节省空间
).build()?;
```

### 速度优化

```rust
// 使用较少的哈希函数提升速度
let filter = BloomFilterOptions::new(
    "speed_optimized".to_string(),
    1_000_000,
    0.01,  // 1% 误判率
).build()?;

// 内部会自动计算最优哈希函数数量（通常 3-7 个）
```

### 内存优化

```rust
// 对于小数据集，使用较小的布隆过滤器
let filter = BloomFilterOptions::new(
    "small_dataset".to_string(),
    10_000,     // 较小的数据集
    0.01,
).build()?;
```

## 监控与调试

### 获取统计信息

```rust
use oxcache::bloom_filter::BloomFilter;

let stats = filter.get_stats()?;

println!("布隆过滤器统计：");
println!("  名称: {}", stats.name);
println!("  预期元素数: {}", stats.expected_elements);
println!("  实际元素数: {}", stats.actual_elements);
println!("  误判率: {:.2}%", stats.false_positive_rate * 100.0);
println!("  位数组大小: {} bytes", stats.bit_array_size);
println!("  哈希函数数: {}", stats.hash_count);
```

### 检查误判率

```rust
// 定期检查实际误判率
let actual_rate = filter.calculate_actual_false_positive_rate()?;

if actual_rate > 0.02 {  // 超过 2%
    println!("⚠️  误判率过高: {:.2}%，考虑重建过滤器", actual_rate * 100.0);
}
```

## 完整示例

```rust
use oxcache::{Cache, CacheOps};
use oxcache::bloom_filter::{BloomFilterOptions};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
}

async fn get_user_with_bloom_filter(
    cache: &Cache<String, User>,
    filter: &BloomFilter,
    user_id: u64,
) -> Result<Option<User>, Box<dyn std::error::Error>> {
    let key = format!("user:{}", user_id);
    
    // 1. 检查布隆过滤器
    if !filter.might_contain(&key) {
        println!("🚫 布隆过滤器：用户 {} 确定不存在", user_id);
        return Ok(None);
    }
    
    println!("✅ 布隆过滤器：用户 {} 可能存在", user_id);
    
    // 2. 查询缓存
    if let Some(user) = cache.get(&key).await? {
        println!("💾 缓存命中：用户 {}", user_id);
        return Ok(Some(user));
    }
    
    println!("🔍 缓存未命中，查询数据库");
    
    // 3. 查询数据库
    let user = database::query_user(user_id).await?;
    
    // 4. 更新缓存
    cache.set(&key, &user, Some(3600)).await?;
    
    // 5. 更新布隆过滤器
    filter.add(&key)?;
    
    println!("✅ 数据库查询成功，已更新缓存和布隆过滤器");
    
    Ok(Some(user))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 布隆过滤器使用示例 ===\n");
    
    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
    
    // 创建布隆过滤器
    let filter = BloomFilterOptions::new(
        "user_cache".to_string(),
        1_000_000,
        0.01,
    ).build()?;
    
    println!("1. 布隆过滤器配置：");
    println!("   预期元素数: 1,000,000");
    println!("   误判率: 1%");
    println!();
    
    // 模拟查询不存在的用户
    println!("2. 查询不存在的用户...");
    let result = get_user_with_bloom_filter(&cache, &filter, 999999).await?;
    println!("   结果: {:?}\n", result);
    
    // 模拟查询存在的用户
    println!("3. 查询存在的用户...");
    let result = get_user_with_bloom_filter(&cache, &filter, 123).await?;
    println!("   结果: {:?}\n", result);
    
    // 显示统计信息
    println!("4. 布隆过滤器统计：");
    let stats = filter.get_stats()?;
    println!("   实际元素数: {}", stats.actual_elements);
    println!("   误判率: {:.2}%", stats.false_positive_rate * 100.0);
    
    Ok(())
}
```

## 故障排除

### 问题：误判率过高

**原因**：
- 预期元素数量设置过小
- 实际元素数量远超预期
- 布隆过滤器已满

**解决方案**：
1. 增加预期元素数量（建议 1.5-2 倍）
2. 定期重建布隆过滤器
3. 监控实际元素数量，动态调整

### 问题：内存占用过大

**原因**：
- 预期元素数量设置过大
- 误判率设置过低

**解决方案**：
1. 减少预期元素数量
2. 提高误判率（如从 1% 提高到 5%）
3. 使用分层布隆过滤器

### 问题：性能下降

**原因**：
- 哈希函数数量过多
- 位数组过大，缓存未命中

**解决方案**：
1. 使用推荐的哈希函数数量（3-7 个）
2. 预热布隆过滤器到 CPU 缓存
3. 使用更小的布隆过滤器

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [速率限制指南](RATE_LIMITING.md)

## 示例代码

- `examples/src/06_features/example_bloom_filter.rs` - 布隆过滤器完整示例
- `tests/security_tests.rs` - 安全测试
- `src/bloom_filter.rs` - 布隆过滤器实现