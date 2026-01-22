# 配置验证与自动修复

## 概述

Oxcache 提供了强大的配置验证和自动修复功能，确保缓存配置的正确性和稳定性。该功能可以：

- ✅ 自动检测不合理的配置
- ✅ 自动修复常见的配置错误
- ✅ 提供详细的验证报告
- ✅ 防止因配置错误导致的运行时问题

## 核心概念

### 后端类型层级限制

每个后端类型都有其推荐的层级限制：

| 后端类型 | 推荐层级 | 限制 | 说明 |
|---------|---------|------|------|
| `Moka` | L1 | L1Only | 仅支持 L1（内存缓存） |
| `Memory` | L1 | L1Only | 仅支持 L1（内存缓存） |
| `Redis` | L2 | L2Only | 仅支持 L2（分布式缓存） |
| `Tiered` | 任意 | Any | 支持任意层级 |

### 自动修复规则

当检测到错误配置时，系统会自动应用以下修复规则：

1. **L1 层错误**：自动替换为默认的 L1 后端（Moka/Memory）
2. **L2 层错误**：自动替换为默认的 L2 后端（Redis）
3. **参数越界**：自动调整到合法范围
4. **无效名称**：自动清理或拒绝

## 使用方式

### 方式 1：使用 CustomTieredConfig

```rust
use oxcache::backend::custom_tiered::{
    CustomTieredConfig, BackendType, AutoFixConfig,
    LayerBackendConfig
};

// 创建配置（故意设置错误）
let mut config = CustomTieredConfig::new();
config.l1.backend_type = BackendType::Redis;  // ❌ 错误：Redis 不应该作为 L1
config.l2.backend_type = BackendType::Moka;   // ❌ 错误：Moka 不应该作为 L2
config.auto_fix.enabled = true;               // ✅ 启用自动修复

// 验证并自动修复
let (result, fixed_config) = config.validate_and_fix();

if let Some(fixed) = fixed_config {
    println!("✅ 配置已自动修复");
    println!("L1: {} → {}", BackendType::Redis, fixed.l1.backend_type);
    println!("L2: {} → {}", BackendType::Moka, fixed.l2.backend_type);
    // 输出：
    // L1: redis → moka
    // L2: moka → redis
} else {
    println!("❌ 配置无法自动修复");
}
```

### 方式 2：使用 Builder 模式

```rust
use oxcache::backend::custom_tiered::{
    CustomTieredConfigBuilder, BackendType
};

let config = CustomTieredConfigBuilder::new()
    .l1(BackendType::Redis)  // ❌ 错误配置
    .l2(BackendType::Moka)   // ❌ 错误配置
    .auto_fix(true)          // ✅ 启用自动修复
    .build();

// 自动修复并验证
let (result, fixed_config) = config.validate_and_fix();
```

### 方式 3：从配置文件加载

```toml
# config.toml
[cache.my_service]
l1_backend = "redis"  # ❌ 错误配置
l2_backend = "moka"   # ❌ 错误配置
auto_fix = true       # ✅ 启用自动修复
```

```rust
use oxcache::backend::custom_tiered::CustomTieredConfigBuilder;

let config = CustomTieredConfigBuilder::new()
    .from_file("config.toml")  // 加载配置
    .await?;

// 自动修复
let (result, fixed_config) = config.validate_and_fix();
```

## 配置选项

### AutoFixConfig

```rust
pub struct AutoFixConfig {
    /// 是否启用自动修复
    pub enabled: bool,
    /// 是否在修复时输出警告日志
    pub warn_on_fix: bool,
}
```

- `enabled: true` - 启用自动修复（默认）
- `warn_on_fix: true` - 输出警告日志（默认）

## 验证报告

### 获取验证报告

```rust
let result = config.validate();
let report = result.get_validation_report();

println!("{}", report);
```

### 报告示例

```
❌ Configuration has issues:
  - Layer L1: redis - Backend type 'redis' does not support layer L1. Only supports L2 layer
  - Layer L2: moka - Backend type 'moka' does not support layer L2. Only supports L1 layer

🔧 Suggested fixes:
  - L1: 'redis' → 'moka' (reason: Backend type 'redis' does not support layer L1. Only supports L2 layer)
  - L2: 'moka' → 'redis' (reason: Backend type 'moka' does not support layer L2. Only supports L1 layer)
```

## 参数验证

### 容量验证

```rust
use oxcache::backend::custom_tiered::ConfigValidation;

// 有效容量
assert!(ConfigValidation::validate_capacity(1000).is_ok());
assert!(ConfigValidation::validate_capacity(1_000_000_000).is_ok());

// 无效容量
assert!(ConfigValidation::validate_capacity(0).is_err());                    // 零
assert!(ConfigValidation::validate_capacity(1_000_000_001).is_err());        // 超限
```

### TTL 验证

```rust
// 有效 TTL
assert!(ConfigValidation::validate_ttl(3600).is_ok());
assert!(ConfigValidation::validate_ttl(30 * 24 * 60 * 60).is_ok());

// 无效 TTL
assert!(ConfigValidation::validate_ttl(0).is_err());                        // 零
assert!(ConfigValidation::validate_ttl(30 * 24 * 60 * 60 + 1).is_err());    // 超限
```

### TTI 验证

```rust
// 有效 TTI
assert!(ConfigValidation::validate_tti(1800).is_ok());
assert!(ConfigValidation::validate_tti(30 * 24 * 60 * 60).is_ok());

// 无效 TTI
assert!(ConfigValidation::validate_tti(30 * 24 * 60 * 60 + 1).is_err());    // 超限
```

### 自定义名称验证

```rust
// 有效名称
assert!(ConfigValidation::validate_custom_name("valid_name").is_ok());
assert!(ConfigValidation::validate_custom_name("my-backend.123").is_ok());

// 无效名称
assert!(ConfigValidation::validate_custom_name("").is_err());               // 空
assert!(ConfigValidation::validate_custom_name("invalid/name").is_err());    // 特殊字符
assert!(ConfigValidation::validate_custom_name("invalid@name").is_err());    // 特殊字符
```

## 路径验证

### PathValidationConfig

```rust
use oxcache::backend::custom_tiered::{PathValidationConfig};

let config = PathValidationConfig::new()
    .add_allowed_base_dir("/var/cache")  // 允许的目录
    .allow_symbolic_links(false)        // 拒绝符号链接
    .with_max_path_length(4096);         // 最大路径长度

// 验证路径
let safe_path = config.validate("/var/cache/config.toml")?;

// 拒绝相对路径
assert!(config.validate("relative/path/config.toml").is_err());

// 拒绝无效字符
assert!(config.validate("/path/with\ninvalid/chars.toml").is_err());
```

## 高级用法

### 自定义验证规则

```rust
use oxcache::backend::custom_tiered::{
    LayerBackendConfig, Layer, BackendType, AutoFixConfig
};

// 创建自定义配置
let l1_config = LayerBackendConfig::new(BackendType::Moka)
    .with_options(serde_json::json!({
        "capacity": 10000,
        "ttl": 300
    }))
    .with_enabled(true);

// 验证配置
assert!(l1_config.validate(Layer::L1).is_ok());
assert!(l1_config.validate(Layer::L2).is_err());
```

### 禁用自动修复

```rust
let config = CustomTieredConfigBuilder::new()
    .l1(BackendType::Redis)  // 错误配置
    .auto_fix(false)         // 禁用自动修复
    .build();

let (result, fixed_config) = config.validate_and_fix();

// 配置验证失败，但不会自动修复
assert!(!result.is_valid());
assert!(fixed_config.is_none());
```

### 自定义警告行为

```rust
let config = CustomTieredConfigBuilder::new()
    .l1(BackendType::Redis)
    .auto_fix(true)
    .with_warn_on_fix(false)  // 禁用警告日志
    .build();
```

## 最佳实践

### ✅ 推荐做法

1. **始终启用自动修复**：在生产环境中启用 `auto_fix.enabled = true`
2. **启用警告日志**：设置 `warn_on_fix = true` 以便追踪配置修复
3. **验证配置**：在应用启动时验证配置并处理修复结果
4. **使用 Builder 模式**：使用 Builder 模式创建配置，链式调用更清晰
5. **检查验证结果**：始终检查 `validate_and_fix()` 的返回结果

### ❌ 避免做法

1. **禁用自动修复**：不要在生产环境中禁用自动修复
2. **忽略验证结果**：不要忽略 `validate_and_fix()` 的返回值
3. **硬编码错误配置**：避免在代码中硬编码错误的配置
4. **跳过验证**：不要跳过配置验证直接使用

## 完整示例

```rust
use oxcache::backend::custom_tiered::{
    CustomTieredConfigBuilder, BackendType, AutoFixConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Oxcache 配置验证与自动修复示例 ===\n");

    // 1. 创建配置（包含错误）
    let config = CustomTieredConfigBuilder::new()
        .l1(BackendType::Redis)  // ❌ 错误：Redis 不应该作为 L1
        .l2(BackendType::Moka)   // ❌ 错误：Moka 不应该作为 L2
        .auto_fix(true)          // ✅ 启用自动修复
        .with_warn_on_fix(true)  // ✅ 启用警告日志
        .build();

    println!("2. 原始配置：");
    println!("   L1 后端: {}", config.l1.backend_type);
    println!("   L2 后端: {}", config.l2.backend_type);
    println!("   自动修复: {}", config.auto_fix.enabled);
    println!();

    // 3. 验证并自动修复
    println!("3. 验证配置...");
    let (result, fixed_config) = config.validate_and_fix();
    println!();

    // 4. 处理验证结果
    if result.is_valid() {
        println!("✅ 配置有效，无需修复");
    } else {
        println!("❌ 配置存在问题");
        println!("{}", result.get_validation_report());
    }

    if let Some(fixed) = fixed_config {
        println!("\n4. 自动修复结果：");
        println!("   L1 后端: {} → {}", BackendType::Redis, fixed.l1.backend_type);
        println!("   L2 后端: {} → {}", BackendType::Moka, fixed.l2.backend_type);
        println!("   修复警告: {}", fixed.warnings.len());
    }

    // 5. 使用修复后的配置
    if let Some(fixed) = fixed_config {
        println!("\n5. 使用修复后的配置创建缓存...");
        // 在这里使用 fixed 配置创建缓存实例
        println!("   ✅ 缓存创建成功");
    }

    Ok(())
}
```

## 故障排除

### 问题：配置验证失败

**原因**：配置中存在不合法的参数或后端类型

**解决方案**：
1. 检查验证报告中的错误信息
2. 启用自动修复：`auto_fix.enabled = true`
3. 检查后端类型是否匹配层级要求
4. 验证参数是否在合法范围内

### 问题：自动修复后仍无效

**原因**：配置错误过于严重，无法自动修复

**解决方案**：
1. 检查验证报告中的详细错误
2. 手动修正配置中的错误
3. 使用默认配置作为参考
4. 查看示例代码了解正确配置

### 问题：路径验证失败

**原因**：配置文件路径不安全

**解决方案**：
1. 使用绝对路径
2. 确保路径在允许的目录内
3. 检查文件权限（建议 0600）
4. 避免使用符号链接

## 相关文档

- [用户指南](USER_GUIDE.md)
- [架构文档](ARCHITECTURE.md)
- [API 参考](API_REFERENCE.md)
- [安全文档](SECURITY.md)

## 示例代码

- `examples/src/06_features/example_health_check.rs` - 健康检查示例
- `tests/config_test.rs` - 配置测试
- `tests/feature_combinations.rs` - 特性组合测试