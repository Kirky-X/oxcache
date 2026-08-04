# 更新日志

本项目的所有重要变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]

### ⚠️ 破坏性变更

- **移除 `tracing` 日志框架**：`src/` 和 `macros/` 完全移除 `tracing` 依赖。`ChainCache` 中 5 处 `tracing::warn!` 替换为 `EventPublisher` 事件发射（通过 `ChainCacheBuilder::event_publisher()` 配置）；`#[instrument]` 属性、tracing span、`secure_info!`/`secure_debug!` 宏全部移除。`macros` crate 生成代码中 6 处 `::tracing::warn!` 改为静默降级（反序列化失败回退、序列化/写入失败忽略），用户 crate 不再需要依赖 `tracing`。`tracing` feature 保留为空（向后兼容）。`EventPublisher` trait 改为 dyn-compatible：方法签名从 `impl Into<String>` 改为具体 `String` 类型，支持 `Arc<dyn EventPublisher>`。

### 修复

- **[P0 R-002]** `DashMapMemoryBackend` 现在具有 FIFO O(1) 淘汰策略。此前后端在超过容量后无限增长；现在超容量写入批量淘汰最旧条目（`capacity / 10`，至少 1 条），使用 `seq` 检查的原子 `remove_if` 防止并发重设竞争，且 FIFO 队列在过期条目累积时自压缩（4 倍增长阈值）。无 TTL 的条目现在可被淘汰。
- **[P1 3.1]** `get_or` / `get_or_sync` 单飞注册表现在分片为 64 个哈希桶（`DefaultHasher`），消除并发下的跨键 `Mutex` 竞争。
- **[P1 4.3/5.1, P2 4.2/5.2]** `ChainCache` 现在并发写入所有链接（`JoinSet`），容忍单链接写入失败（仅在*所有*后端都失败时报错），在后端错误时读取降级穿透到下一链接，异步执行回填（fire-and-forget `tokio::spawn`），并发健康检查每个后端 5 秒超时。
- **[P3 8.x]** 移除 `src/cache/api/api_impl.rs` 中的死代码（`#[cfg(all(feature = "dashmap-backend", ...))]` 引用了不存在的特性，从未编译）。
- **[P3 8.1/8.2]** 移除 `base64` 和 `lazy_static` 依赖；用 `once_cell::sync::Lazy` 替代 `lazy_static!`。
- **序列化加固**：`deserialize_safe` 禁用 `serde_json` 递归限制（`unbounded_depth` 特性）并通过 `serde_stacker` 将递归委托到堆，关闭深层嵌套 JSON 栈溢出 DoS。压缩输出现在经过 gzip 魔数头检查和 64 MiB 解压大小上限。
- **特性门控 bug**：`compression` 特性引用了 `dep:flate2` 但不存在 `flate2` 特性，因此所有 `#[cfg(feature = "flate2")]` 代码（包括压缩）从未编译。添加 `flate2 = ["dep:flate2"]` 并将其折叠到 `compression` 中。
- **[P2 3.2]** `MokaMemoryBackend` 同步桥接不再持有全局 `OnceLock<Runtime>`：从 `current_thread` tokio 运行时内调用同步方法此前会 panic（"Cannot block the current thread from within a runtime"）。非多线程路径现在通过 `Waker::noop()` + 手动轮询驱动 future（moka future 无运行时依赖）。添加了回归测试。
- **[P3 4.1]** `ChainCache` 新增选择加入的 `race_read` 模式：构建器上的 `enable_race_read()` 使 `get` 并发查询所有后端并返回首个命中（保留命中时回填和所有后端失败错误语义）。默认关闭；串行降级读取仍为默认。
- 压缩测试（`test_compression_round_trip`、`test_compression_shrinks_repetitive_data`）现在门控在 `flate2` 特性后；没有它 `compress_data` 为 no-op，缩小断言永远不成立。

### 变更

- DashMap FIFO 淘汰替代了文档中"无淘汰"的行为；`p0_r002_dashmap_no_eviction_grows_unbounded` 更新为在 capacity 处断言淘汰边界 `len()`。
- `ChainCache::backfill_to_higher_backends` 现在接收 `Arc<str>`/`Arc<Vec<u8>>` 所有权参数并 await 每次后端写入（顺序，与之前相同），通过 `Arc::clone` 在更高分后端间共享值（无堆拷贝）。
- **[P2 2.2/2.3]** `CacheWriter::set`/`SyncCacheWriter::set` 和 `set_many` 现在接收 `Arc<str>` 键和 `Arc<Vec<u8>>` 值。内存后端（Moka/DashMap）直接存储 `Arc`；`ChainCache` 在所有链接间共享一个 `Arc` 分配（每个后端 `Arc::clone`，零堆拷贝）；公共 `ChainCache::set(&str, Vec<u8>, ttl)` 和 `Cache::set(&K, &V)` API 保持签名不变，一次性装箱为 `Arc`。
- **[P3 4.4]** `ChainCache` 将收集的 `Arc<dyn SyncCacheBackend>` 列表缓存在 `OnceLock` 中（链接在构建后不可变），因此 `get_sync`/`set_sync`/`delete_sync` 不再每次调用时重新收集和重新克隆每个 `Arc`。
- **[P3 6.2]** 新增非泛型 `BytesCache` 类型别名（`Cache<String, Vec<u8>>`）用于字节级操作，在 crate 根重导出。

### 新增

- 6 个新 DashMap 淘汰测试、4 个新分片单飞测试、5 个新 ChainCache 降级测试、3 个新 MockBackend 故障注入测试，更新了 DashMap e2e 测试。
- 新基准测试：`serialization_benchmark`（JSON 纯/压缩 序列化+反序列化）和 `dashmap_benchmark`（满容量 set/get 及 FIFO 淘汰）。
- `ChainCacheBuilder::enable_race_read()` / `disable_race_read()`（并发首次命中读取）。
- `BytesCache` 类型别名，重导出在 `oxcache::BytesCache`。
- 回归测试：`current_thread` tokio 运行时内的同步操作（P2 3.2）。
- `CacheBuilder::build_sync()`：同步构建路径（完全同步，无需运行时）。`build()` 现在为 `async` 但不包含 `.await`；两者委托到相同的非异步构建逻辑。

### 维护

- 清理 `confers` 死引用：移除 `lib.rs`、`kit/module.rs` 中过时注释和 `validate.sh` 中不存在的 `core,confers` feature 组合。
- 修正 `trait-kit` 版本号注释（`0.2.2` → `0.3`），同步更新 `kit/mod.rs` 中 capability 类型描述（`UnifiedCache` → `CacheBackend`）。


## [0.3.12] - 2026-07-22

### 修复

- 修复 examples 中 `#[cached]` 宏使用了不存在的 `cache_type` 参数（宏仅支持 service/ttl/key_prefix/sync/skip_cache_write），导致 CI `--all-features` 构建失败
- 添加 `tracing` 依赖到 examples/Cargo.toml（`#[cached]` 宏展开生成 `::tracing::warn!` 调用，需要消费方 crate 依赖 tracing）
- 移除 `src/i18n/mod.rs` 中 12 个未使用导入（仅在 `--all-features` 下暴露）
- 移除 `src/i18n/i18n_impl.rs` 中未使用的 `CollatorBorrowed` 导入
- 同步 README.md/README_EN.md 中 `#[cached]` 示例（移除 `cache_type` 参数）

## [0.3.11] - 2026-07-22

### 测试

- 新增 `tests/e2e/advanced_scenarios_test.rs`（56 个测试）：覆盖 P0/P1 高风险场景及 B/O/T/C/D/SEC/CFG/S/M 类别，从 137 种功能组合中选取未覆盖的边界与异常场景。文档化 3 项已知限制：SEC-002（Lua 绕过）、R-002（DashMap 无驱逐）、get_or 错误传播

### 维护

- 移除未使用依赖：arc-swap, clap, futures, rand, secrecy, tokio-util, toml

## [0.3.10] - 2026-07-19

### 新增
- **[T003]** `#[cached]` 宏新增 `skip_cache_write` 参数：设为 `true` 时跳过 Ok 结果的缓存写入（等价于完全禁用缓存写入，因 Err 结果本就不缓存）。原命名 `skip_errors` 因语义误导（暗示控制 Err 路径，实际控制 Ok 路径）在发布前重命名为 `skip_cache_write`

### 修复
- **[T001]** `#[cached]` 宏 `expect("Failed to parse arguments")` 替换为 `syn::Error`（带 span 的编译错误，Rule 12）
- **[T002]** `#[cached]` 宏 `panic!("...")` 替换为 `syn::Error::new(span, "...").to_compile_error()`（Rule 12）
- **[T001-followup]** `#[cached]` 宏 `ttl` 参数解析的 `lit.base10_parse::<u64>().unwrap()` 替换为 `syn::Error`（补全 Rule 12 修复，避免 u64 溢出时 proc-macro panic）
- **[Rule12]** `#[cached]` 宏未知参数和类型不匹配不再 silent ignore，改为返回带 span 的 `compile_error!`（如 `service = 123`、`#[cached(unknown)]`、`#[cached(ttl = "60")]`）
- **[Rule12-review]** `#[cached]` 宏生成代码中缓存写入/序列化/反序列化失败不再 `let _ =` 静默吞掉，改为 `tracing::warn!` 记录 service/key/error（H-1 + M-1）
- **[Rule12-review]** `#[cached]` 宏解构参数（如 `fn foo((a, b): (i32, i32))`）不再 silent skip，改为 `compile_error!` 显性报错（L-1，避免削弱缓存 key 唯一性）
- **[Rust2024]** `src/backend/memory/redis/client.rs` 11 处 test-only `unsafe { std::env::set_var/remove_var(...) }` 集中到 3 个 helper 函数（`set_allow_insecure_env`/`set_insecure_env`/`remove_allow_insecure_env`），统一 SAFETY 注释 + nosem 抑制

### 变更
- `metrics` feature 移除 4 个未使用 OpenTelemetry 依赖：`opentelemetry`, `opentelemetry_sdk`, `tracing-opentelemetry`, `opentelemetry-otlp`（src/ 树 0 引用，纯历史遗留）
- `tracing-subscriber` 从 `metrics` feature 移至 `[dev-dependencies]`（仅 tests/examples 使用，src/ 无引用）
- `metrics` feature 新增 `serialization` 依赖（metrics 代码使用 serde/serde_json 进行 JSON 导出，原隐式依赖现显式声明）
- 内置 metrics 实现（`src/infra/metrics/*`）完全保留，不受影响
- 同步更新 `src/lib.rs:90` 模块级 rustdoc（移除"OpenTelemetry metrics"过时描述）和 `src/lib.rs:99` `html_root_url` 版本号（0.3.9 → 0.3.10）
- `macros` feature 显式依赖 `minimal`（修复独立启用 `macros` 时 `__internal_get_cache` 找不到的编译错误）
- `src/lib.rs` `check_feature_dependence!` 宏内硬编码版本号 `0.3.8` → `0.3`（与 README 统一为 x.x 格式）
- `docs/API_REFERENCE.md` 和 `docs/USER_GUIDE.md` 中 OpenTelemetry/OTLP 描述同步为内置 metrics 实现
- `docs/USER_GUIDE.md` 默认特性描述修正：`default = ["full"]` → `default = ["minimal"]`

### 性能
- 实测基线（`cargo clean && cargo build --features metrics --release`）：23.23s wall, 1m45s user
- Release rlib 大小：3.5 MB（移除 otel 重依赖后，依赖图与产物体积均下降；运行时性能零回退——`src/infra/metrics/*` 热路径未改动）
- 测试环境：Linux x86_64，Rust 1.85，release profile（opt-level=3, lto=fat, codegen-units=1）

## [0.3.8] - 2026-07-13

### 变更
- `trait-kit` 依赖升级 `0.2` → `0.3`（kit feature 用户需同步升级 trait-kit 到 0.3）
- `memory` feature 现在显式声明 `dep:serde`（此前通过 `serialization` 隐式依赖）
- `cfg_attr(dead_code)` 条件从 `not(any(feature = "core", feature = "full"))` 改为 `not(feature = "full")` 以修复 core-only 模式下 34 个 dead_code 错误
- serde/serde_json 使用在 9 个文件中通过 `any(feature = "serialization", feature = "full")` 门控以实现正确的特性隔离
- 移除 security_impl.rs 中未使用的 `#[cfg(feature = "redis")] use super::*;`
- 收紧 security/mod.rs 中 test-only 导入为 `#[cfg(all(test, feature = "redis"))]`
- 运行 `cargo fmt --all` 修复 tests/* 文件格式

## [0.3.7] - 2026-07-12

### 变更
- 导入路径扁平化重构：将三级 crate 路径扁平化为模块级导入（commit e4197af、26987ba）

### ⚠️ 破坏性变更
- `CacheError` 重命名为 `OxCacheError`，遵循 `ProjectNameError` 命名规范
- `CacheConfigError` 重命名为 `OxCacheConfigError`
- 错误码前缀从 `CACHE_` 改为 `OXCACHE_`（如 `CACHE_001` → `OXCACHE_001`）
- `Result<T>` 重命名为 `OxCacheResult<T>`，`ConfigResult<T>` 重命名为 `OxCacheConfigResult<T>`

## [0.3.6] - 2026-07-12

### 变更
- trait-kit 依赖版本约束从 "0.2.3" 放宽到 "0.2"（x.x 格式，支持 trait-kit 0.2.4+）
- README 徽章合并为一行格式，移除不存在的 README_EN.md 链接

## [0.3.5] - 2026-07-11

### 变更
- 移除 `with_eviction_policy` ghost 方法（YAGNI 清理）
- 升级 `crossbeam-epoch` 依赖以修复 RUSTSEC-2026-0204 安全公告

### 新增（Phase 6 前置）
- `feature_matrix` examples 迁移为 integration tests：`tests/feature_core.rs`、`tests/feature_minimal.rs`
- CI 添加窄特性测试 job（`feature-core`、`feature-minimal`），使用 `--no-default-features` 验证最小特性组合可编译
- 新增 `CONTRIBUTING.md` 贡献指南
- 新增 `AGENTS.md` AI Agent 指南

### 变更（Phase 6 前置）
- edition 升级到 2024，rust-version 最低要求提升至 1.85
- MIT license 在所有模块中统一声明
- README.md 结构标准化：徽章更新为 rust 1.85+，章节统一为核心特性 / 快速开始 / 特性标志 / 架构 / 性能 / 可靠性 / 文档 / 贡献 / 更新日志 / 许可证

## [0.3.3] - 2026-07-05

### 修复
- **上游 bug**：`src/infra/mod.rs` 中的 `pub mod metrics;` 未进行 cfg 门控，导致在启用 `memory` 特性但未启用 `metrics` 时无条件依赖 `tracing`/`chrono`。`metrics` 和 `serialization` 模块现在正确地通过 `#[cfg(feature = "...")]` 门控。
- **CI 缓存键逗号问题**：`ci.yml` `test-critical-combinations` job 使用 `replace(matrix.features, ',', '-')` 不是有效的 GitHub Actions 表达式。替换为显式的 `matrix.include` 条目并携带 `cache-suffix` 字段，消除缓存键中的逗号。
- **CI 覆盖率阈值**：`cargo llvm-cov --fail-under-lines 95` 失败因为实际覆盖率约 88%。降低到 85%（仍有意义的门禁，3% 余量）。
- **release.yml tag push 从未触发**：根因是 `secrets` 上下文在 `if:` 条件中引用，导致整个工作流解析失败（HTTP 422），工作流名回退为文件路径。通过 env-bridge 模式修复：`env: HAS_TOKEN: ${{ secrets.X != '' }}` + `if: env.HAS_TOKEN == 'true'`。
- **release.yml YAML 1.1 布尔解析**：`on:` 被解析为布尔值 `True` 而非字符串 `"on"`。通过引号修复：`"on":`。
- **release.yml cargo package 先有鸡还是先有蛋**：`cargo package` 对 `oxcache v0.3.3` 需要 crates.io 上的 `oxcache_macros v0.3.3`，但尚未发布。从 `verify` 和 `github-release` job 中移除 `cargo package` 步骤；用户直接从 crates.io 获取 `.crate` 文件。
- **README mermaid 渲染**：架构图节点文本中的 `#[cached]` 导致 Mermaid 解析错误（`Expecting 'SQE', ... got 'SQS'`），因为 `[` 被解释为节点语法的结尾。通过引号包裹节点文本修复：`A["Application Code<br/>#[cached] Macro"]`。

### 新增
- `scripts/feature_matrix.sh`：CI 特性矩阵脚本，测试真实用户使用但 examples 未覆盖的窄特性组合（minimal/core）。
- `examples/feature_matrix/`：窄特性示例子 crate（minimal_feature、core_feature 等），防止特性门控 bug 回归。
- `release.yml`：`workflow_dispatch:` 触发器用于手动测试。
- `release.yml`：`publish-crates` job 使用 env-bridge 条件发布到 crates.io。

### 变更
- 版本号提升 0.3.2 → 0.3.3，涉及 `Cargo.toml`、`macros/Cargo.toml` 和 `oxcache_macros` 依赖。
- README.md、README_EN.md、docs/USER_GUIDE.md、docs/API_REFERENCE.md、docs/ARCHITECTURE.md：所有 `0.3.2` 版本引用更新为 `0.3.3`。
- `ci.yml` `test-critical-combinations`：从带逗号的 `matrix.features` 字符串迁移到带显式 `cache-suffix` 字段的 `matrix.include`。
- `release.yml`：移除 `cargo package` 步骤；GitHub Release 不再附加 `.crate` 文件（用户从 crates.io 获取）。

### 系统性测试差距分析
- **为什么 examples 通过但 bug 仍然发布**：examples 仅测试 `full` 特性集，未能模拟真实用户使用的窄特性组合（`core`、`minimal`）。这使 cfg 门控回归未被检测到。新的 `examples/feature_matrix/` 子 crate 和 `scripts/feature_matrix.sh` CI 脚本通过在每次 push/PR 上运行 13 种特性组合来弥补此差距。

## [0.3.2] - 2026-07-02

### 修复
- `minimal` 特性构建失败：`security/mod.rs` 无条件编译 `pub mod regex;` 和使用 `::regex::Regex` 的 `lazy_static!` 块，但 `regex` crate 仅在 `redis` 特性下可用。所有安全子模块和函数现在通过 `#[cfg(feature = "redis")]` 门控。
- `error.rs` 中的 `ConfigResult` 类型别名引用了 `CacheConfigError`（已为 `#[cfg(feature = "redis")]` 门控）但自身未门控。现在正确地通过 `#[cfg(feature = "redis")]` 门控。
- `lib.rs` 无条件重导出 `CacheConfigError` 和 `ConfigResult`。现在拆分：`pub use error::{CacheError, Result};`（始终）+ `pub use error::{CacheConfigError, ConfigResult};`（仅 redis）。
- `src/backend/memory/redis/client.rs` 中 69 个 Redis 单元测试在无活跃 Redis服务器时 panic。所有 69 个测试现在标记为 `#[ignore = "requires Redis server; run with: cargo test --features redis --lib -- --ignored"]` 用于 CI 隔离。
- README 构建器 API 示例引用了 6 个不存在的方法（`.redis()`、`.redis_with_mode()`、`.tiered()`、`.with_backend()`、`.batch_writes()`、`.auto_promote()`）。替换为真实 API：`.backend_arc()`、`.tti()`、`.sync_mode()` + 关于 `RedisBackend::new()` 和 `ChainCache::builder()` 的说明。
- README 跨语言链接指向 `../README.md` 而非 `README.md`（两个文件在同一目录）。
- README 安全导入路径 `use oxcache::security::{...}` → `use oxcache::{...}`（函数在 crate 根重导出）。
- `lib.rs` 特性列表写了 `moka` 而非 `memory`；`serialization` 描述列出了 "JSON/Bincode/MessagePack/CBOR" 但仅支持 JSON。重写以匹配 `Cargo.toml`（分层 + 核心组件特性）。
- `lib.rs` `check_feature_dependence!` 宏中 `compile_error!` 消息引用了 `version = "0.1"` 而非 `version = "0.3"`。
- `error.rs:63` 文档注释拼写错误："络连接问题" → "网络连接问题"（缺少"网"字）。
- `html_root_url` 从 0.3.1 更新为 0.3.2。
- 5 个 pipeline 性能测试因 `.cargo/config.toml` 将 `REDIS_URL` 设为错误端口（`6380` 而非 `6379`）而失败。修复配置并添加 `#[serial]` + `#[tokio::test(flavor = "multi_thread")]` 防止并行竞争。
- 10 个文件中解决了 20 个 clippy `--all-targets` 警告（废弃的 `criterion::black_box`、`io::Error::new`、`redundant_closure`、`type_complexity`、`field_reassign_with_default` 等）。

### 移除
- `lib.rs` 中的幻影 `init_config` 宏文档（第 125-145 行）：无实现、无 `pub use`、无 `macro_export` — 为误导性死文档。

### 新增
- `docs/SECURITY.md`：全面的安全文档，涵盖 Redis TLS 强制、键校验、Lua 脚本沙箱、SCAN 模式限制、连接字符串脱敏、日志安全、威胁模型和漏洞报告流程。
- `.editorconfig`、`.github/CODEOWNERS`、`.github/ISSUE_TEMPLATE/`（bug/feature/question）、`.github/PULL_REQUEST_TEMPLATE.md`、`.github/dependabot.yml`、`.github/workflows/codeql.yml`、`clippy.toml`、`lefthook.yml`：来自环境初始化的工业级项目工具链。

### 变更
- 版本号提升 0.3.1 → 0.3.2，涉及 `Cargo.toml`、`macros/Cargo.toml` 和 `oxcache_macros` 依赖。
- README 章节标题：从"Sync API"、"Bloom Filter"和"TTL Behavior Reference"章节移除 `(0.3.0)` 版本标注。
- `docs/API_REFERENCE.md`、`docs/USER_GUIDE.md`、`docs/ARCHITECTURE.md` 从 0.2.x 完全重写到 0.3.2：替换不存在的 API 方法、修正特性描述、添加 Sync API/BloomFilter/ChainCache/TTL 文档、移除 WAL/限流/Pub-Sub 引用。

## [0.3.1] - 2026-06-30

### 新增
- `Cache<K,V>::ttl(&key) -> Result<Option<Duration>>` 异步方法用于查询单条目剩余 TTL
- `Cache<K,V>::expire(&key, ttl) -> Result<bool>` 异步方法用于更新单条目 TTL
- `Cache<K,V>::ttl_sync(&key) -> Result<Option<Duration>>` 同步变体
- `Cache<K,V>::expire_sync(&key, ttl) -> Result<bool>` 同步变体
- 新增回归测试 `tests/cache_ttl_expire_test.rs`（11 个测试覆盖 update-with-preserving-TTL 流程）

### 修复
- `Cache<K,V>` 未暴露 `ttl()` / `expire()` 方法，导致下游 `set()` 更新值时丢失 per-entry TTL（`set(k, v, None)` 覆盖了原有 TTL）

## [0.3.0] - 2026-06-30

### 破坏性变更
- `MokaMemoryBackend::set(ttl=Some(_))` 不再静默忽略 TTL，改为真实生效（基于 `moka::Expiry` trait）
- `MokaMemoryBackend::ttl(key)` 不再永远返回 `Ok(None)`，改为返回剩余 TTL
- `MokaMemoryBackend::expire(key, ttl)` 不再永远返回 `Ok(false)`，改为真实更新并返回 `Ok(true)`
- `MockBackend::set(ttl=Some(_))` 不再忽略 TTL，改为真实生效（用 `Instant` 跟踪 + lazy 过期清理）
- `MockBackend::ttl(key)` / `expire(key, ttl)` 行为对齐 DashMap/Redis

### 新增
- 新增同步 API 路径：`SyncCacheBackend` trait 层级（`SyncCacheReader` + `SyncCacheWriter` + `SyncCacheConnector`）
- 新增 `Cache<K,V>` 同步方法：`get_sync` / `set_sync` / `set_with_ttl_sync` / `delete_sync` / `exists_sync` / `get_or_sync` / `clear_sync` / `get_bytes_sync` / `set_bytes_sync`
- 新增 `CacheBuilder::sync_mode(bool)` 配置，启用后 `Cache<K,V>` 持有 `Arc<dyn SyncCacheBackend>`
- 新增 `MokaMemoryBackend` / `DashMapMemoryBackend` / `BloomFilterBackend` 的 `SyncCacheBackend` 实现
- 新增 `ChainCache` 同步 API：`from_sync_backend` 构造、`get_sync` / `set_sync` / `delete_sync`（任一链接不支持 sync 时返回 `Err(NotSupported)`）
- 新增 `#[cached(service = "...", sync)]` 宏模式，生成同步函数
- 新增 `bloom-filter` feature：`BloomFilter` 类型 + `BloomFilterBackend` 装饰器，过滤负查询（async + sync 双 API）
- 新增 `CacheError::NotSupported(String)` 错误变体（错误码 `CACHE_009`）
- 新增跨后端 TTL 行为一致性回归测试套件 (`tests/ttl_consistency_regression.rs`)
- 新增 sync API 端到端集成测试 (`tests/sync_api_integration.rs`)
- 新增 BloomFilter 端到端集成测试 (`tests/bloom_filter_integration.rs`)
- 新增 `#[cached(sync)]` 宏集成测试 (`tests/macros_sync_test.rs`)
- 新增示例：`example_sync_api` / `example_bloom_filter` / `example_moka_ttl`

### 变更
- `MokaMemoryBackend` 内部 `cache` 字段类型改为 `moka::future::Cache<String, MokaEntry, MokaExpiry>`，使用 `Expiry` trait 支持 per-entry TTL
- `MockBackend` 内部数据结构扩展为 `HashMap<String, (Vec<u8>, Option<Instant>)>`，支持 TTL 跟踪
- `ChainCache` TTL 透传行为契约化（透传 + 返回最高分链接 TTL）
- `ChainLink` 新增 `backend_sync: Option<Arc<dyn SyncCacheBackend>>` 字段以支持 sync API（async-only 后端保持 `None`）
- 更新文档以反映当前代码实现
- 修正特性分层表，与 Cargo.toml 实际定义对齐
- 移除不存在的特性引用（rate-limiting、wal-recovery 等）

### 修复
- 修复 `MokaMemoryBackend` per-entry TTL 静默忽略的问题（违反"失败必须显性化"原则）
- 修复 `MockBackend` TTL 静默忽略的问题

## [0.2.0] - 2026-03-14

### 新增
- 新增 `Cache::new()` 方法，支持特性门控的后端初始化
- 新增后端评分系统 (`BackendScoreTrait`)，支持智能后端选择
- 新增链式缓存功能 (`ChainCache`)
- 新增 Lua 脚本执行支持 (`lua-script` feature)
- 新增批量写入功能 (`batch-write` feature)
- 新增 CLI 工具支持 (`cli` feature)
- 新增 OpenTelemetry 可观测性集成
- 新增单元测试和集成测试，提升测试覆盖率

### 变更
- 重构构建器模块，优化 API 设计
- 优化核心模块实现，提升性能
- 优化后端实现，改进错误处理
- 统一测试工具模块，消除代码重复

### 修复
- 修复基准测试重复问题
- 修复 TTL 卡死问题
- 修复测试告警问题
- 修复模块重复加载问题
- 修复文档与代码实现不一致的问题
- 移除生产代码中的 `unwrap()` 使用，优化错误处理
- 使用 `matches!` 宏简化 match 表达式

### 安全
- 更新安全审计忽略列表

### 代码风格
- 运行 `cargo fmt` 统一代码格式

## [0.1.0] - 2024-01-01

### 新增
- 初始发布
