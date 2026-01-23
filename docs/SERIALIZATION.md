# 多序列化支持说明

## 概述

Oxcache 提供了灵活的序列化支持，支持 JSON 和 Bincode 两种序列化格式。不同的序列化格式各有优劣，可以根据实际需求选择合适的格式。

### 核心特性

- ✅ **多种格式**：JSON、Bincode
- ✅ **性能优化**：支持压缩和批量序列化
- ✅ **类型安全**：基于 serde 的类型安全序列化
- ✅ **自定义序列化**：支持自定义序列化器

## 支持的序列化格式

| 格式 | 特点 | 优点 | 缺点 | 适用场景 |
|------|------|------|------|----------|
| JSON | 文本格式 | 可读性好、通用性强 | 体积大、速度慢 | 调试、跨语言 |
| Bincode | 二进制格式 | 速度快、体积小 | Rust 专用 | Rust 间通信 |

## 使用方式

### JSON 序列化

```rust
use oxcache::{Cache, CacheOps};
use oxcache::serialization::JsonSerializer;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建缓存
    let cache: Cache<String, User> = Cache::tiered(10000, "redis://localhost:6379").await?;
    
    // 创建 JSON 序列化器
    let serializer = JsonSerializer::new();
    
    // 序列化数据
    let user = User {
        id: 123,
        name: "张三".to_string(),
        email: "zhangsan@example.com".to_string(),
    };
    
    let serialized = serializer.serialize(&user)?;
    println!("序列化结果: {}", String::from_utf8(serialized)?);
    
    // 反序列化数据
    let deserialized: User = serializer.deserialize(&serialized)?;
    println!("反序列化结果: {:?}", deserialized);
    
    Ok(())
}
```

### Bincode 序列化

```rust
use oxcache::serialization::BincodeSerializer;

// 创建 Bincode 序列化器
let serializer = BincodeSerializer::new();

// 序列化数据
let serialized = serializer.serialize(&user)?;

// 反序列化数据
let deserialized: User = serializer.deserialize(&serialized)?;
```





## 高级用法

### 自定义序列化器

```rust
use oxcache::serialization::Serializer;
use serde::{Deserialize, Serialize};

struct CustomSerializer;

impl Serializer for CustomSerializer {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // 自定义序列化逻辑
        Ok(serde_json::to_vec(value)?)
    }
    
    fn deserialize<'de, T: Deserialize<'de>>(&self, data: &[u8]) -> Result<T, Box<dyn std::error::Error>> {
        // 自定义反序列化逻辑
        Ok(serde_json::from_slice(data)?)
    }
}
```

### 使用序列化缓存优化性能

```rust
use oxcache::serialization::{SerializationCache, JsonSerializer};

// 创建序列化缓存
let cache = SerializationCache::new(10000, 3600);
let serializer = JsonSerializer::new();

// 对同一对象多次序列化时，第二次会从缓存读取
for _ in 0..100 {
    let serialized = cache.serialize(&user, &serializer)?;
    // 使用序列化后的数据
}
```

### 动态选择序列化器

```rust
use oxcache::serialization::{SerializerRegistry, JsonSerializer, BincodeSerializer};

let mut registry = SerializerRegistry::new();
registry.register("json", Box::new(JsonSerializer::new()))?;
registry.register("bincode", Box::new(BincodeSerializer::new()))?;

// 根据配置动态选择
let format = get_configured_format(); // "json" 或 "bincode"
let serializer = registry.get(format)?;
```

## 性能对比

### 序列化性能

| 格式 | 速度 | 体积 | 兼容性 |
|------|------|------|--------|
| Bincode | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐ |
| JSON | ⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐⭐ |

### 基准测试

```rust
use oxcache::serialization::*;

let data = generate_test_data();
let iterations = 10000;

// JSON
bench("JSON", JsonSerializer::new(), &data, iterations);

// Bincode
bench("Bincode", BincodeSerializer::new(), &data, iterations);
```

## 最佳实践

### ✅ 推荐做法

1. **根据场景选择**：
   - Rust 间通信：使用 Bincode
   - 调试和日志：使用 JSON
   - 跨语言通信：使用 JSON

2. **启用压缩**：对大数据启用压缩，减少存储和传输开销

3. **版本控制**：使用版本化序列化器，确保向后兼容

4. **性能测试**：对不同格式进行性能测试，选择最优方案

5. **错误处理**：妥善处理序列化错误，避免数据丢失

### ❌ 避免做法

1. **过度优化**：不要为了微小的性能提升牺牲可读性

2. **忽略兼容性**：不要忽略跨语言兼容性问题

3. **错误格式**：不要在需要可读性的场景使用二进制格式

4. **忽略错误**：不要忽略序列化错误

5. **硬编码格式**：不要硬编码序列化格式，保持灵活性

## 完整示例

```rust
use oxcache::{Cache, CacheOps};
use oxcache::serialization::{SerializerConfig, SerializationType, CompressionType};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    id: u64,
    name: String,
    email: String,
    created_at: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 多序列化支持示例 ===\n");
    
    // 1. 创建测试数据
    println!("1. 创建测试数据...");
    let user = User {
        id: 123,
        name: "张三".to_string(),
        email: "zhangsan@example.com".to_string(),
        created_at: "2026-01-22T10:30:00Z".to_string(),
    };
    println!("   用户: {:?}\n", user);
    
    // 2. JSON 序列化
    println!("2. JSON 序列化...");
    let json_config = SerializerConfig::new()
        .with_type(SerializationType::Json);
    let json_serializer = json_config.build()?;
    
    let json_data = json_serializer.serialize(&user)?;
    println!("   JSON 大小: {} bytes", json_data.len());
    println!("   JSON 内容: {}", String::from_utf8(json_data.clone())?);
    
    let json_user: User = json_serializer.deserialize(&json_data)?;
    println!("   反序列化: {:?}\n", json_user);
    
    // 3. Bincode 序列化
    println!("3. Bincode 序列化...");
    let bincode_config = SerializerConfig::new()
        .with_type(SerializationType::Bincode);
    let bincode_serializer = bincode_config.build()?;
    
    let bincode_data = bincode_serializer.serialize(&user)?;
    println!("   Bincode 大小: {} bytes", bincode_data.len());
    println!("   压缩率: {:.1}%", 
        (bincode_data.len() as f64 / json_data.len() as f64) * 100.0);
    
    let bincode_user: User = bincode_serializer.deserialize(&bincode_data)?;
    println!("   反序列化: {:?}\n", bincode_user);
    
    // 4. 性能对比
    println!("4. 性能对比...");
    let iterations = 10000;
    
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = json_serializer.serialize(&user)?;
    }
    let json_time = start.elapsed();
    println!("   JSON 序列化: {:?} ({} 次)", json_time, iterations);
    
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = bincode_serializer.serialize(&user)?;
    }
    let bincode_time = start.elapsed();
    println!("   Bincode 序列化: {:?} ({} 次)", bincode_time, iterations);
    
    println!("\n5. 总结：");
    println!("   最小体积: Bincode ({} bytes)", bincode_data.len());
    println!("   最快速度: Bincode ({:?})", bincode_time);
    println!("   最佳兼容: JSON");
    
    Ok(())
}
```

## 故障排除

### 问题：序列化失败

**原因**：
- 数据类型不支持
- 循环引用
- 版本不兼容

**解决方案**：
1. 确保数据类型实现 Serialize/Deserialize
2. 避免循环引用
3. 使用版本化序列化器

### 问题：反序列化失败

**原因**：
- 数据损坏
- 格式不匹配
- 版本不兼容

**解决方案**：
1. 检查数据完整性
2. 确认序列化格式匹配
3. 升级或降级版本

### 问题：压缩失败

**原因**：
- 数据太小
- 内存不足
- 压缩算法不支持

**解决方案**：
1. 调整压缩阈值
2. 增加内存配置
3. 更换压缩算法

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [Serde 文档](https://serde.rs/)

## 示例代码

- `examples/src/serialization/` - 序列化示例
- `src/serialization/` - 序列化实现