# oxcache_macros

[oxcache](https://github.com/Kirky-X/oxcache) 仓库的过程宏子 crate（目录 `oxcache/macros`，包名带下划线）。

## 与父 crate 的关系

| 项 | 内容 |
|---|---|
| 所属仓库 | [oxcache](https://github.com/Kirky-X/oxcache)（[crates.io](https://crates.io/crates/oxcache) · [docs.rs](https://docs.rs/oxcache)） |
| 依赖方向 | `oxcache` → `oxcache_macros`（父 crate 以 `path + version` 双写依赖本 crate） |
| 是否 re-export | 是：父 crate 在 `macros` 特性开启时导出 `cached`，并提供 `oxcache::macros` 模块重导出全部宏 |
| 适用场景 | 通常无需直接依赖；开启父 crate 的 `macros` 特性即可使用 |

## 提供的属性宏

| 宏 | 作用 |
|---|---|
| `#[cached]` | 把函数包装为带缓存的方法，支持键派生、TTL、`single_flight`、`strict`、`condition` 等高级参数 |

## 使用

经父 crate 使用（推荐）：

```toml
[dependencies]
oxcache = { features = ["macros"] }
```

```rust
use oxcache::Cache;
use oxcache::cached;

// 宏要求先注册：register_for_macro 仅支持 Cache<String, Vec<u8>>，
// 未注册的 service 会静默穿透、直接执行原函数
let cache: Cache<String, Vec<u8>> = Cache::builder().build().await?;
cache.register_for_macro("user_cache").await?;

#[cached(service = "user_cache", ttl = 600)]
async fn load_user(id: u64) -> User {
    // 回源逻辑
}
```

支持参数：`service` / `ttl` / `key` / `key_prefix` / `scope` / `sync` / `skip_cache_write` / `single_flight` / `strict` / `condition` / `skip` / `cache_none`。

## 版本与支持

- 版本经 `version.workspace = true` 随仓库统一，与 `oxcache` 同号发布。
- 仅维护当前 minor 版本线，详见 [oxcache 安全文档 · 支持的版本](https://github.com/Kirky-X/oxcache/blob/main/docs/SECURITY.md#-支持的版本与维护策略)。

## 许可证

MIT
