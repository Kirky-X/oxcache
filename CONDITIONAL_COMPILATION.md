# 条件编译重构总结

## 重构目标
为 `/home/project/oxcache/src/client/` 和 `/home/project/oxcache/src/backend/` 模块添加条件编译支持，实现：

1. **L1Client** 需要 `l1-moka` feature
2. **L2Client** 需要 `l2-redis` feature  
3. **TwoLevelClient** 需要两个 feature 同时启用
4. 所有导入都是条件性的
5. 保持 API 稳定

## 主要变更

### 1. Client 模块 (`src/client/`)

#### `mod.rs`
- `l1` 子模块：添加 `#[cfg(feature = "l1-moka")]`
- `l2` 子模块：添加 `#[cfg(feature = "l2-redis")]`
- `two_level` 子模块：添加 `#[cfg(all(feature = "l1-moka", feature = "l2-redis"))]`

#### `l1.rs`
- 整个模块添加 `#[cfg(feature = "l1-moka")]`
- 所有导入都是条件性的
- `L1Client` 结构体和方法只在 feature 启用时可用

#### `l2.rs`
- 整个模块添加 `#[cfg(feature = "l2-redis")]`
- 所有导入都是条件性的
- `L2Client` 结构体和方法只在 feature 启用时可用

#### `two_level.rs`
- 整个模块添加 `#[cfg(all(feature = "l1-moka", feature = "l2-redis"))]`
- 所有导入都是条件性的
- `TwoLevelClient` 结构体和方法只在两个 feature 同时启用时可用

### 2. Backend 模块 (`src/backend/`)

#### `mod.rs`
- `l1` 子模块：条件编译控制
- `l2` 子模块：条件编译控制
- `redis_provider` 子模块：条件编译控制

#### `l1.rs`
- 整个模块添加 `#[cfg(feature = "l1-moka")]`
- `L1Backend` 结构体和方法只在 feature 启用时可用

#### `l2.rs`
- 整个模块添加 `#[cfg(feature = "l2-redis")]`
- `L2Backend` 结构体和方法只在 feature 启用时可用

#### `redis_provider.rs`
- 整个模块添加 `#[cfg(feature = "l2-redis")]`
- `RedisProvider` trait 和实现只在 feature 启用时可用

### 3. 相关模块更新

#### `manager.rs`
- 为 `get_typed_client` 添加条件编译
- 为 `EvictionPolicy` 导入添加条件编译
- 处理功能降级逻辑

#### `key_generator.rs`
- 为 `KeyGenerator` 添加 `#[cfg(feature = "regex")]`
- 相关方法和结构体使用条件编译

#### `lib.rs`
- 修复了无效的 feature 条件值（`sync` → `batch-write`）

## Feature 组合

| Feature | L1Client | L2Client | TwoLevelClient |
|---------|----------|----------|----------------|
| `l1-moka` | ✅ | ❌ | ❌ |
| `l2-redis` | ❌ | ✅ | ❌ |
| `l1-moka` + `l2-redis` | ✅ | ✅ | ✅ |
| `minimal` (l1-moka) | ✅ | ❌ | ❌ |
| `core` (l1-moka + l2-redis) | ✅ | ✅ | ✅ |
| `full` (all features) | ✅ | ✅ | ✅ |

## 使用示例

### 仅启用 L1
```toml
[dependencies]
oxcache = { version = "0.1", features = ["l1-moka"] }
```

### 仅启用 L2
```toml
[dependencies]
oxcache = { version = "0.1", features = ["l2-redis"] }
```

### 启用完整双层缓存
```toml
[dependencies]
oxcache = { version = "0.1", features = ["l1-moka", "l2-redis"] }
```

或者使用组合 feature：
```toml
[dependencies]
oxcache = { version = "0.1", features = ["core"] }  # 包含 l1-moka 和 l2-redis
```

## API 稳定性

所有公开 API 保持向后兼容：
- `CacheOps` trait 不变
- `CacheExt` trait 不变
- 客户端创建接口不变
- 降级逻辑自动处理

## 编译测试

```bash
# 仅 L1
cargo check --features l1-moka

# 仅 L2  
cargo check --features l2-redis

# 双层缓存
cargo check --features l1-moka,l2-redis

# 默认（完整）
cargo check

# 所有 features
cargo check --all-features
```
