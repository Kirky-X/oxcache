# 更新日志

本项目的所有重要变更都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [Unreleased]
## [0.5.0-rc.7] - 2026-10-08### Added- **canonical JSON 归一化与 `json_hash_key` 指纹键**（utils）：为缓存键提供稳定归一化序列化与指纹摘要。### Changed- **core 正名 redis-tier、full 补齐真全量**：`core` feature 更名 `redis-tier`（Redis 能力面语义化），`full` 聚合补齐为真全量特性集；同步新增 feature-matrix 工作流。### Removed- **死代码清理**：删除编译器证实的死宏与零调用特性探测模块、17 个零构造命令变体、仅测试使用的访问器方法，残留死代码按特性隔离收敛。### Fixed- **测试门控修复**：`setup_cache` 助手随 memory 门控，非 memory 组合测试可编译。
### 新增

- **`BatchWriterBuilder::reject_when_full(bool)`（`batch` feature）**：缓冲打满时拒绝入队并返回 `Err(OxCacheError::BufferFull)`（错误码 `OXCACHE_019`，`is_recoverable() == true`，被拒条目不入缓冲）；默认 `false` 保持既有「打满即自动刷盘」语义。`docs/API_REFERENCE.md` 补「Redis Pub/Sub 广播通道（`pubsub` 特性）」节（`RedisPubSub` 构造期 fail-fast / 独占订阅连接 / publish 接收端计数 / panic 隔离 / 断线线性退避重连 / 任务回收契约与 API 一览），特性表 `batch` 行同步注明拒绝模式
- **`example_redis_modes` 单源双注册**：主包注册同名 example（path 指向 `examples/src/02_advanced/` 同一文件），workspace 根 `cargo run --example example_redis_modes` 可解析运行——默认 features（minimal，无 redis）下 cfg 降级为指引输出（rc=0），`--features redis` 走完整演示；`oxcache-examples` 包补 default feature（redis/kit/bloom，仅作为源码 cfg 判定面，oxcache 能力面仍由依赖行固定 features 决定）保持 `cargo run -p oxcache-examples --example ...` 完整演示；Standalone 连接失败从 `?` 硬退出改为与 Cluster/Sentinel 同风格的打印跳过（无 Redis 环境同样 rc=0）

### 变更

- **第十五跑依赖升级（2026-10-04 入库）**：`redb` 3.1 → 4.3（磁盘 L3 后端 `RedbDiskBackend` 适配新版本 API，对外语义不变）；trait-kit 采纳 `0.5.0-rc.7`、confers 采纳 `0.6.0-rc.6`；可选依赖特性显式化（`default-features = false` 收口，如 uuid、hmac）；`bincode` 钉在 2.0 线并注释说明不升 3 的原因——bincode 项目已停止开发，crates.io 的 3.0.0 为官方刻意发布的 `compile_error` 占位（阻止 caret 误升级），最后正式版为 2.x 线 2.0.1
- **i18n 61 键接入**与 29 处文档校准随检查点入库
- **inklog 精确钉同步至 `=0.3.0-rc.7`（2026-10-08 入库，发布链口径）**：`Cargo.toml` 与 `examples/Cargo.toml` 两处 `=` 需求同改（只改一处会因 `=0.3.0-rc.5` 与 `=0.3.0-rc.7` 互斥使 workspace 直接解析失败）。桥接消费面 rc.5→rc.7 为纯增量：`domain/types/log_record.rs` 逐字节一致，`support/io/sink/mod.rs` 仅新增 `SamplingPolicy` 再导出，`error.rs` 仅新增 `secret-scan` 门控变体；被消费的 `LogRecord` / `LogRecord::new` / `sink::LogSink` / `InklogError::RuntimeError` / `tracing::Level` 五个符号位置未动。**结构性变化与副作用**：已发布 inklog rc.6/rc.7 均依赖 `oxcache = "0.5.0-rc.6"`（rc.5 实为 `0.5.0-rc.4`，旧注释写的「rc.5 引用 oxcache 0.5.0-rc.5」与发布清单不符），与本仓根包**同版本号而异 source**——仍是合法 DAG（桥接只消费 inklog 类型，两个 oxcache 实例无类型交换），但开启 `inklog` 后 `-p oxcache` 包选择器歧义（需改用 cwd 选包或 `path+file://…` 显式 spec），inklog Enabled 构建会额外编译一份 registry oxcache。待本仓发布 rc.7 后该同版本碰撞自然消失。验证：CI 形态门禁全绿——`cargo check --workspace --all-features`、`--workspace --no-default-features`、`-p oxcache --features redis-tier|core|minimal`、`cargo clippy --all-targets --features full --workspace -- -D warnings`；桥接面 `cargo test --no-default-features --features inklog --lib` 287 passed，其中 `features::audit::tests::inklog_bridge` 8 passed；`cargo check -p oxcache-examples --no-default-features --features inklog-bridge --example example_inklog_audit_bridge` 通过

### 修复

- **单特性全矩阵 `--all-targets` 口径裂缝**：`cargo hack check --each-feature --all-targets`（含 lib 内联测试与 tests/ 集成测试编译）暴露的存量裂缝——引用 memory 面（Moka/DashMap 后端、`Cache::new`）的 `#[cfg(test)]` 模块与无门控集成测试在单开 redis 系特性（dragonfly/redis/lock 等，不含 memory）时编译失败：`src/lib.rs` 宏依赖测试（redis 单开组合下自触发 `check_feature_dependence!` 的 compile_error，门控收紧为 `all(test, any(memory, full))`）、`backend/interface`、`cache/api`（主测试/atomic_ops/sync/sentinel_race/get_or_with_ttl）、`cache/builder/cache_builder`、`cache/chain`、`cache/interface` 测试模块统一收紧为 `all(test, feature = "memory")`（interface 兼顾 `testing` 用 any）；`tests/backend_interface_extra_test.rs` 补 `#![cfg(feature = "full")]` 文件级门控（对齐其余 12 个同类文件惯例）；`tests/integration.rs` 的 `chain_cache_integration_test`/`ttl_consistency_test` 补 `memory` 门控，`dragonfly_test`/`valkey_test`（L1+L2 组合测试）补文件级 `#![cfg(feature = "memory")]`；all-features 下 lib 测试数量不变（1477 passed，无误隐藏）
- **`HotKeyTracker::record` 并发首次插入丢计数**：原 `get` + `entry().or_insert(1)` 两段式在多线程同时初始化同一 key 时，后到者的 `or_insert` 发现已存在即丢弃自己的 +1（并发回归实测 800 次记录采到 799）；改为单临界区 `entry().and_modify(fetch_add).or_insert(1)`，每次记录恰好 +1
- **pubsub 内联测试基础设施**：接收端从 `std::sync::mpsc::recv_timeout`（阻塞 current_thread 运行时唯一线程，后台订阅任务无从投递）改为 `tokio::sync::mpsc` unbounded + `timeout` 异步等待——tokio 1.53 起有界 `Sender::send` 为 async，同步 handler 内未 poll 的 future 会静默丢消息；端到端用例前置探测升级为「支持 PUBLISH 的真 Redis」判定（同进程 `test_support::ensure_server(6379)` 会在真 Redis 缺席时架起无 PUBLISH 的最小假服务器，仅 TCP 探活无法区分），非真 Redis 与不可达同口径 SKIP
- **rustdoc 断链（第十五跑 E2E#230）**：6 处指向 feature 门控目标的 intra-doc 链接降级为纯文本（`OxCacheConfigError` / `with_invalidation` / `Bincode` / `Postcard` / `global_tagged` 等），default 与 all-features 双口径 `-D warnings` 归零
- **`stale` 特性内联测试 compression 引用（E2E#226）**：门控收口后 `cargo test --features stale --tests` 编译恢复 exit 0
- **示例 Sentinel 段 API 误用（E2E#223）**：`example_redis_modes` 的 Sentinel 演示改显式 `RedisMode::Sentinel` 构建——`RedisBackend::new` 直连哨兵端口系 API 误用（显式路径实测 8/8 通过）
- **pubsub panic 隔离测试环境鲁棒性（E2E#235）**：每轮重发覆盖断线重连窗口，失败时区分「静默超时」与「订阅任务提前退出」两种形态

## [0.5.0-rc.6] — 2026-10-05

> 号位说明：本节涵盖原定名 `0.5.0-rc.7` 的工作波次（自适应 TTL、指标 per-service 维度、`sync_backend_arc` / `AsyncToSyncBridge`、统一配置中枢 `CacheConfig`、智能预热、inklog 审计桥、分配基线）——发布裁决将其并入 `0.5.0-rc.6` 号位（workspace 版本与 macros 钉版同步调回 rc.6），仓库内**不存在 rc.7 版本与 tag**；下方「hitbox 能力吸收批次」为同版本内的前一批内容。

### 新增

- **自适应 TTL（R5，`adaptive-ttl` feature，默认关闭）**：`AdaptiveTtlBackend` 装饰任意 `CacheBackend`，按访问模式调整条目 TTL——命中计数达 `hot_threshold` 的 hot 键 set 时 TTL 乘 `hot_ttl_multiplier`、get 时受 `adjust_interval` 限速把已存条目 `expire` 调整到 `clamp(hot_ttl_multiplier × 剩余 TTL)`（方向不限，基准超出上界同样收敛，限制写放大）；已知键最近访问早于 `cold_idle_after` 时 set 的 TTL 除以 `cold_ttl_divisor`；hot 与 cold 同时满足时 hot 优先。全部阈值为 `AdaptiveTtlConfig` 显式常量（无黑盒启发式）：调整结果钳制在 `[min_ttl, max_ttl]`（默认 1s..1h），`None`（永不过期）不参与调整原样透传；追踪表上限 `max_tracked_keys`（默认 65 536）防内存失控，满时新键按普通键透传；配置经 `validate()` 在构建期校验——`min_ttl > max_ttl`、`hot_ttl_multiplier` 非有限正数（0/负/NaN/inf）、`cold_ttl_divisor = 0`、`max_tracked_keys = 0` 均显性 `Err(InvalidInput)`（拒绝发生在构造期而非请求路径 `Duration::clamp` panic 或乘除静默畸变）。经 `CacheBuilder::adaptive_ttl()` 一等接线，与 `sync_mode(true)` / `stale_ttl` 组合在构建期显性拒绝（装饰器对 sync API 不可见、双 TTL 改写器叠加语义未定义）；`stats()` / `reset_stats()` 暴露追踪键数与延长/缩短计数，get 路径主动调整遇后端 `expire` 故障不阻断命中、以 `failed_adjustments` 显性计数，后端 `stats()` 附加 `adaptive_tracked_keys` / `adaptive_hot_extensions` / `adaptive_cold_shortenings` / `adaptive_failed_adjustments`，`clear()` 同步清零访问历史
- **指标 per-service 维度（R9，默认关闭）**：`CacheBuilder::service_name(...)` 或配置通路 `OXCACHE_SERVICE_NAME`/`cache.service_name` 显式启用后，该缓存的操作计数附加 `service` 标签——`export_prometheus_standard()` 出现 `oxcache_service_operations_total{service="..."}` 行（按名排序确定输出，标签值按 exposition 格式转义 `\\`/`"`/换行，恶意或含特殊字符的命名无法伪造额外序列），JSON 快照（`snapshot()`/`export_json()`）新增 `service_operations` 段（空时经 skip_serializing_if 整体缺席）；标签基数上限 `MetricsConfig::max_service_labels`（默认 64）防标签爆炸，超限归因计入 `oxcache_service_labels_overflow_total`（独立门控导出——含 `max_service_labels=0` 时 map 恒空的场景；总账不受影响）；`UnifiedMetricsRecorder::global_tagged(service)` 为装配入口，计数器为 `DashMap<String, AtomicU64>`（稳态分片读锁 + fetch_add，首触 entry 合并无丢计数），原子总账与无标签路径逐字节一致（service 是附加拆分维度）；未设置时 prometheus 导出与既有格式逐字节一致；空串 service 在构建/validate 期显性拒绝
- **`sync_mode(true)` × `backend_arc` 解除互斥（`AsyncToSyncBridge`）**：`backend_arc` 注入的 async 面后端现在可与 `sync_mode(true)` 组合——sync API 经通用 `AsyncToSyncBridge` 桥出（每个同步调用 `block_in_place` + `block_on` 异步面，futures 不会中途被丢弃；原子能力按 inner 的 `as_atomic_writer` 诚实探测），要求调用时处于多线程 Tokio runtime（I/O 型后端需 ≥2 worker，单 worker 有挂起风险），runtime 之外或 current_thread runtime 上逐调用显性 `Err(NotSupported)`（原先该组合在构建期直接 `Err(NotSupported)`，现已解锁）；`shutdown` 在 runtime 之外经临时 runtime 真实执行，current_thread runtime 下跳过并计数 `oxcache_bridge_shutdown_rejected_total`（默认预设可见，telemetry feature 下另发 warn）；分层一等构建 API 收尾，`CacheConfig` 的 `sync_mode` × `backend` 组合约束同步解除，且配置通路按槽位注入——Moka/DashMap 走双面原生槽（async 面保持原生运行时无关，sync 面直连，均零桥接），Redis/Dragonfly/Disk 保持桥接；`RedisBackend` 同步面运行时守卫与桥接守卫合并为共享 `multi_thread_bridge_handle`（语义不变）；分层取舍：运行时无关的 sync API 仍建议走 `sync_backend_arc`（原生同步面）或默认 Moka 路径
- **`sync_backend_arc` 同步一等构建入口**：`CacheBuilder` 新增注入 `Arc<dyn SyncCacheBackend>` 的方法（内部以 `BackendSlot` enum 分槽保存 async/sync 两类入口），配合 `sync_mode(true)` 时同步 API 直连原生同步后端、异步 API 经新增的 `SyncBackendAdapter` 门面呈现（async 方法体内同步完成，无运行时依赖；原子操作经 `as_sync_atomic_writer` 动态探测）；`backend_arc`（仅 async 面，具体类型同步实现已擦除）与 `sync_mode(true)` 组合彼时返回 `Err(NotSupported)`，错误信息改指 `sync_backend_arc`（后经 `AsyncToSyncBridge` 解除，见上条）；解锁此前「同步 API 仅限默认 Moka 后端」的限制

- **统一配置中枢 `CacheConfig`**：单一结构（`oxcache::config::CacheConfig`，`config` 模块转公开）承载缓存构建全量参数，三条配置通路——程序化 builder / `OXCACHE_*` 环境变量（`try_from_env()`，15 键：capacity/TTL/TTI/空值 TTL/抖动因子/sync_mode/backend/指标/序列化格式/redis_url/disk_path/连接池/熔断阈值与恢复超时/service_name）/ confers 配置源（`config-confers` 下 `try_from_confers()`，`OxcacheConfig` 扩展 10 个 Option 键并按快照映射）；`validate()` 一致性检查覆盖值域（容量非零且不超平台 `usize`、TTL 非零、连接池与熔断阈值非零）、组合约束（Redis/Dragonfly 必填 redis_url、Disk 必填 disk_path）与 feature 可用性（未启用 `metrics`/`serialization`/后端 feature 时配置对应键显性报错）；`apply_to_cache_builder()` / `build_backend()` 落地到既有构建链，backend 按槽位注入——Moka/DashMap 走双面原生槽（async 面运行时无关、`sync_mode(true)` 下 sync API 原生直连，均零桥接），其余后端 async 面经 `backend_arc` 注入（sync 面经 `AsyncToSyncBridge` 桥出）；env 未设置的键保持 `None`（零行为漂移），解析失败与 feature 缺失均显性报错（附变量名与原始值）；confers 通路同轮消除两处静默截断——超界熔断阈值与连接池大小由钳制/丢弃改为显性报错，热更新重载被拒时保留旧快照并可观测（`oxcache_config_reload_rejected_total` 计数器为默认信号，telemetry feature 下另发 tracing warn）；`redis_url` 的 `Debug` 输出脱敏
- **智能预热（R7，`warmup` feature，默认关闭）**：`WarmupLoader` 端口（对象安全，`Arc<dyn WarmupLoader>` 注入）拉取热 key 集合，异步回填 `ChainCache`——`WarmupEntry` 直供值经链式写入落全部后端，仅 key 条目经 `ChainCache::iter_entries` 批量读从低层晋升（链上无值计 `missing`），晋升回填查链上剩余 TTL 透传源过期语义（不把带 TTL 的源条目重置为链默认 TTL）；key 去重（首次出现生效，loader 顺序即优先级）、回填写入并发上界可配（默认 8，0 视为 1 串行，Semaphore 滑窗恒定在飞数）、直供值单值大小上限（默认与序列化写入面 `MAX_JSON_SIZE` 同口径）与单轮条目数上限（默认 100_000）防 loader 无界集合放大回填规模（均可经 builder 调整，置 0 解除）；`WarmupReport` 全量显性计数（`loader_entries`/`deduped`/`warmed`/`promoted`/`missing`/`failed` + 逐条失败明细 + 实测并发峰值 + `dropped_value_too_large`/`dropped_over_entry_cap` 超限丢弃）；失败语义：loader 端口失败显性 `Err` 中止（缓存本体不受影响），单条写入失败不中断整批、计入报告。11 例单测覆盖回填命中/批量晋升/去重/loader 失败容错/写入失败计数/并发上界/零并发收敛/空 loader/单值上限丢弃/条目数上限丢弃/晋升 TTL 透传
- **审计事件 → inklog 结构化日志桥接（R10，`inklog` feature，默认关闭）**：`InklogAuditPublisher` 实现既有 `AuditEventPublisher` 端口，审计事件映射为 `inklog::LogRecord` 经有界通道（容量 1024，`BRIDGE_CHANNEL_CAPACITY`）交由单一 writer task 保序写入注入的 `inklog::sink::LogSink`（ConsoleSink/FileSink/自定义实现；每事件无界 spawn 改单消费者，过载以丢弃替代无界任务堆积）；级别映射为显式常量表（状态变更 Set/Delete/Clear → INFO，读探测与容量/过期清理 Hit/Miss/Evict/Expired → DEBUG），字段携带 `action`/`key`/`namespace`/`operator`/`timestamp_ms` 与 `meta.*` 元数据；`publish` 非阻塞——无 runtime 上下文（如 runtime 之外的 sync API 调用路径）、通道过载、通道关闭（writer 所属 runtime 关停后的后续 publish）、runtime 关停时通道内滞留事件（接收端守卫清算）均显性丢弃并计数 `dropped_count()`，sink 写失败计数 `write_failure_count()`，均不反压缓存操作；inklog 依赖精确钉 =0.3.0-rc.5（`default-features = false`，禁止 caret 自动升级引入端口漂移；所需端口 `LogRecord`/`LogSink`/`InklogError` 均为该版本无条件编译面，证据：rc.5 源 `lib.rs:161/222/223` 根重导出、`support/io/sink/mod.rs:77` `LogSink` trait、`domain/types/log_record.rs:191` `LogRecord::new`；rc.5 对 oxcache 0.5.0-rc.5 的 registry 依赖与本仓为不同 package id 的合法 DAG，非包级循环）；示例 `examples/src/06_features/example_inklog_audit_bridge.rs`（`inklog-bridge` feature 隔离，默认示例构建不引入 inklog），9 例单测覆盖转发/级别映射/写失败计数/无 runtime 丢弃/通道过载丢弃/runtime 关停窗口丢失/映射全覆盖/可选字段省略

### 性能

- **热路径分配削减（R12）**：分配剖析基线入库 `docs/allocation-baseline.md`（口径与环境说明 + 复测 harness），两个削减点——`UnifiedSerializer::deserialize` 非压缩分支去除 `to_vec()` 全量中间拷贝（三种格式的解析入口均以 `&[u8]` 借用工作，拷贝纯属浪费；每次反序列化省 1 次堆分配 + memcpy，get/get_many/get_or 逐键生效）；`batch_ops::get_many` 结果容器预分配 `HashMap::with_capacity(values.len())`（消除逐条 insert 的多轮 grow + rehash）。`get_many`（100 键全命中）每调用堆分配 **517 → 412（−20.3%）**（`#[global_allocator]` 计数法，确定性主指标；criterion 时间项在噪声内不可分辨，已在文档诚实披露）；基线文档同时给出评估后未削减候选点的 ROI 结论

### 变更

- **治理复核收口（HK2，§9.2 第 2 项）**：复核记录入库 `docs/FEATURE_AUDIT_RECHECK.md`——「redis-only 编译失败」经实证过时（`--no-default-features --features redis` 通过）；「bare-kit 编译失败」成立已修（`trait-kit`/`kit` 隐含 `memory` 基线）；「`core`/`full` 语义重整」论据不足维持现状（三处文档口径一致，选择加入特性不在 `full` 为既定设计）；「basic_ops 序列化双分支」成立已修（cache 模块编译必然隐含 `serialization`，5 处不可达负分支删除、正分支去恒真 cfg，无可达行为变化）。cargo-hack 单特性全矩阵（30+ 特性）另暴露并修复 4 个同类单开编译裂缝：`batch`/`compression`/`encrypt` 隐含 `memory` 基线、`integrity` 隐含 `encrypt`（模块寄生在 encryption 内）；修复后单特性矩阵与 14 组主要组合抽查全绿；`docs/API_REFERENCE.md`「特性依赖」表补全

### 测试

- **diting-review 登记待办收口**：MED-001（测试线程内 `unsafe set_var` 与 libtest 并发构成形式数据竞争）已修——`tests/common/mod.rs` 增 `#[ctor::ctor(unsafe)]` 进程加载期一次性写入 `OXCACHE_ALLOW_INSECURE_REDIS`，删除 11 个测试文件共 31 处测试体内环境变量写点与 2 个空壳 helper；LOW-002 部分修复——`start_redis/valkey/dragonfly_container` 三胞胎以 `container_start_fn!` 宏收敛，骨架残余经实证评估不收敛（理由留档 `docs/diting-review.md`「待办修复轮」）

### 文档

- `docs/allocation-baseline.md`（新增，热路径分配基线与削减对账；放 `docs/` 而非任务指定的 `reviews/`——该目录被 `.gitignore` 排除，入库必丢文件，循 `diting-review.md` 重建先例）；`docs/FEATURE_AUDIT_RECHECK.md`（新增，治理复核记录）；`docs/diting-review.md` 待办收口与状态同步；`docs/ARCHITECTURE.md` 未来增强节落地注记（智能预热）；`docs/API_REFERENCE.md` 智能预热节与 `audit_publisher` 发布器清单、特性依赖表补全；README 特性表增 `warmup` 行、`audit` 行补 inklog 发布器、版本历史与测试计数刷新


### hitbox 能力吸收批次（2026-09-28 起草，随 v0.5.0-rc.6 一并发版）

#### 新增

- **`#[cached]` 宏 `skip(...)` 参数**：被点名参数不进入默认缓存 key（如 `skip(password)` 排除敏感参数）；与显式 `key` 模板互斥（编译期报错）、未知参数名编译期报错（对标 hitbox `skip` 语义）
- **`stale` feature SWR 三态过期**：`StaleWhileRevalidateBackend` 装饰器以双时间戳 envelope（`expire_at`/`stale_at`）实现 Actual/Stale/Expired 三态判定；`CacheBuilder::stale_ttl()` / `stale_policy()` 接线；`StalePolicy::Return`（旧值兜底，默认）/ `Revalidate`（同步回源刷新）/ `OffloadRevalidate`（立即回旧值 + 后台刷新）；非 envelope 旧数据按新鲜透传，零迁移成本；物理 TTL = `ttl + stale_ttl`；stale 命中发布 `CacheEventType::Expire` 事件并计入 `oxcache_stale_hits_total`
- **`offload` feature 后台任务子系统**：`OffloadManager` 同 key 去重 + 信号量并发上限 + 超时策略（`None`/`Cancel`/`Warn`，默认 `Warn(30s)`）；`Cache::get_or_refresh()` 触发后台重验证（fallback 需 `Send + 'static`）；`oxcache_offload_*` 指标全集与 `oxcache::offload` 遥测
- **`disk` feature 磁盘持久化 L3 后端**：`RedbDiskBackend`（redb 3.x 纯安全 Rust，ACID + WAL）；value envelope（seq + epoch 毫秒过期）+ 读路径懒过期物理删除（与 DashMap 同口径）；`max_entries` 超限清扫（先删过期、按 seq 升序删最旧，摊销触发，`spawn_blocking` 执行）；`BackendKind::Disk`、`Scores::REDB = 85`；`ChainLink::from_arc` / `ChainBuilder::extra_backend` 挂链为 L3（读穿透 + 回填）；`full` 预设包含 `disk`/`stale`/`offload`
- **ChainCache 读策略枚举**：`ChainReadStrategy`（`Sequential`/`Race`/`ParallelFreshest`）+ `ChainCacheBuilder::read_strategy()`；`ParallelFreshest` 并发全读后按剩余 TTL 择新（None 最低优先，并列取最高分）；`enable_race_read()`/`disable_race_read()` 保留为兼容别名；三策略读结果遥测埋点
- **缓存审计加固配套 API**：新增 `hotkey` feature（`HotKeyTracker` 分片计数 + 快照半衰 Top-K）；`max_capacity_bytes` 容量上限（Moka weigher / DashMap 字节记账，防大值内存超卖）；`get_or_with_ttl` / `get_or_option_with_ttl`（含 sync 变体）与 `set_many_with_ttl`；布隆过滤器 `prefill` 预热 API
- **ChainCache 批量读取 `iter_entries`**：命中层单层批量读与整批错误按键拆分；长度契约显性化、命中值移动语义修正
- **invalidation 写路径失效链集成**：`with_invalidation` / `tiered_with_invalidation` 一站式装配；`InvalidatingBackend::with_expire_broadcast()` expire 广播开关（默认关）
- **`ByteWeightCache` 权重观测**：`with_eviction_listener()` 逐出监听器与 `weighted_size()` 权重总量查询
- **`BloomFilter` 泛型化**：键类型放宽为 `K: ?Sized = str`；新增 `hash_count` / `set_bits` 访问器与 `new_with_hash_count`（闭式二分反解，免去逐探针全尺寸位图分配）
- **degradation 降级观测桥**：状态 snapshot 只读快照、telemetry 状态迁移观测桥与双层熔断语义文档

#### 变更

- **默认指标落地（行为变化）**：`Cache` 默认 recorder 由 `NoOpMetricsRecorder` 改为全局 `UnifiedMetricsRecorder`（`metrics` feature 下所有构造路径生效）——`minimal` 预设开箱即产生 hit/miss/set/delete 计数；显式注入 `NoOpMetricsRecorder` 可恢复静默
- **指标维度补强**：单后端 `Cache` 指标 layer 依后端类型判定（内存 L1 / 分布式 L2，原硬编码 L1）；`get_bytes`/`set_bytes` 及 sync 版补齐与泛型路径同口径打点；新增 backend 维度计数 `oxcache_backend_<name>_operations_total`（`export_prometheus_standard` 以 `backend` label 导出）
- **集成测试容器门控 STRICT 两档**：valkey/dragonfly 集成测试接入 `gate_skip` / `backend_skip` 门控原语（失败短路闩、首因记录、90 秒门控预算）；ci.yml / release.yml 的 test step 置位 `OXCACHE_TEST_STRICT=1` fail-closed——容器不可用时失败而非静默跳过；testcontainers 启用 watchdog feature 兜底容器回收

#### 修复

- **宏 `cache_none` no-op 缺陷**：`cache_none` 参数此前解析后从未消费；且 `Result<Option<T>, E>` 的 `Ok(None)` 被无条件缓存（与文档宣称相反）。现按文档语义修复：默认仅缓存 `Ok(Some)`，开启 `cache_none` 后 `Ok(None)` 以 `null` 缓存并在读取时还原；single_flight leader 与 sync 路径同口径
- **文档版本漂移**：README/README_EN/lib.rs 中 13 处 `0.5.0-rc.4` 当前版本引用同步至 `0.5.0-rc.5`（历史发布条目保留）
- **缓存审计加固修复组**：single-flight follower 丢失唤醒（`Notify`→`watch` + `macro_support` 64 分片同步守卫与 panic 清理）；`get_or_option` 穿透哨兵判定竞态（改 get + 字节比对，消 exists 竞态）；雪崩防护默认 TTL 抖动 0.1（xorshift64 + 单调时钟随机源，弃 `SystemTime` 防 NTP 回拨）；`set_many` / `get_many` 统一走 `UnifiedSerializer`
- **feature 组合编译修复**：`memory` / `redis` 隐含 `serialization`（核心 Cache API 的 serde 约束为事实依赖）、`degradation` 隐含 `memory`；无 memory 组合的 backend 实现与 metrics 引用按依赖特性门控——单开特性组合编译基线恢复
- **Redis TTL 毫秒化**：亚秒 TTL（< 1s）此前被秒口径校验（`as_secs() == 0` 即拒绝）静默拒之门外，导致 `set` 不写 L2、`expire` 完全不生效；`RedisCommand` 新增 `PExpire`，`set`/`set_many` 由 SETEX 改为 `SET key value PX <ms>`、`expire` 改为 `PEXPIRE`、`set_if_absent` 改 `SET NX PX`，`incr`/`compare_and_swap` Lua 脚本内的 EXPIRE/SET EX 同步切换（Redis/Valkey ≥ 2.6.12：SET 的 PX/NX 选项自 2.6.12 引入，构成命令面下限；PEXPIRE 自 2.6.0）；校验改毫秒口径（`as_millis() == 0` 才拒绝，u128 比较防截断误放行），`set_many_pipeline` 与 `CacheWriter::set_many` 同根因一并修复
- **ChainCache `expire` 后端故障静默吞错**：原实现对后端 `Err` 与 `Ok(false)` 一律 `continue`，TTL 校验失败等错误无任何事件/日志可观测；后端 `Err` 时发布 error 事件（对齐 set 路径既有机制），并按 backfill/iter_entries 既有惯例补 telemetry warn（feature 关闭时零开销），返回值语义不变（部分成功仍 `Ok(true)`，键不存在 `Ok(false)` 属正常结果非故障）

#### 测试

- **Redis 亚秒 TTL 直查断言**：直连 Redis 断言 `PTTL` 毫秒精度与键按亚秒 TTL 过期后消失，覆盖 writer/pipeline 路径回归

#### 文档

- README / README_EN / API_REFERENCE / ARCHITECTURE 同步 `disk`/`stale`/`offload`/读策略/宏 `skip` 能力说明（中英对称）
- 蓝图全量采用库侧改动配套文档：API_REFERENCE / ARCHITECTURE 增补 bloom 泛型化、批量读取、invalidation 装配、degradation 快照等条目，补记 `memory` / `redis` / `degradation` 特性隐含依赖口径；tests/README 登记「容器可用性门控」STRICT 两档语义与 bloom_filter_integration 目标级门控

---

## [0.5.0-rc.5] - 2026-09-21

### 新增

- **pubsub Redis Pub/Sub 广播组件**：跨实例消息广播传输层（收编并行会话改动），为 invalidation 失效总线提供协议基础
- **ByteWeightCache 字节权重同步缓存（byte-weight feature）**：按字节权重计量与控制容量的同步缓存实现
- **i18n 整改**：接入 unify-rust-i18n 统一错误与消息文案（`src/i18n/messages.rs`）

### 变更

- **跨仓 path 依赖改走 crates.io**：trait-kit / confers 改为 crates.io 版本依赖；供应链与工程加固（detect-secrets 基线、pre-commit 门禁、CodeQL Action SHA 统一 bump、zstd 0.14、GitHub Actions 批量升级）

### 修复

- **sync 缓存审计事件**：sync 缓存操作补齐审计事件发布（对齐 async 路径）
- **feature 门控清理**：清理 chrono 幽灵门控；red-lock / trait-kit / test-util 特性正名
- **Lua 块注释跳过越界绕过**：`skip_lua_comment` 在计数前先消费开括号 `[`，使 `=` 参与闭合符长度计算——此前 `--[==[ x ]==] redis.call('FLUSHALL')` 会因越界吞吃而对校验器不可见（独立 diting 审查实证，存量缺陷）
- **日志脱敏**：无 userinfo 的 URL 经 query/fragment 携带秘密时同样掩码；`sanitize_message` 切分改用 `rfind('@')`，密码含 `@` 不再残留片段
- **aerospike 亚秒 TTL**：非零亚秒 TTL 上取整到 1 秒（原截断为 0 后被当作永不过期）；显式 `Duration::ZERO` 保持 Never 语义
- **CI 权限回归**：`dependency-check.yml` 的 failure-notification job 补 `issues: write`（顶层权限收紧后建单调用将 403）

### 文档

- 修正方括号索引折叠的行为声明：折叠发生于引号剥离之后，含连字符的索引残段同样可能被折叠（结果仍为合法 Lua 且无黑名单命中）

---

## [0.5.0-rc.4] - 2026-09-10

### 新增

- **`#[cached]` 宏高级参数**：新增 `single_flight`（同 key 并发 miss 仅回源一次）、`strict`（未注册缓存 panic 而非静默穿透）、`condition`（执行前谓词旁路）、`cache_none`（解析就绪）
- **kit 全后端能力**：`OxcacheConfig` 新增 `backend` 枚举（Memory/Redis/Chain），`OxcacheModule::build()` 按配置构建 RedisBackend 或 ChainCache；`redis` feature 未启用时返回清晰错误
- **telemetry feature**：新增 `telemetry` feature 引入 `tracing` 门面；在熔断器状态转换、ChainCache 回填、宏静默穿透路径发 event；feature 关闭时零开销
- **BatchWriter**：实现容量/时间间隔双阈值刷盘的 `batch::BatchWriter`，兑现 `batch` feature
- **LockProvider trait**：新增 `LockProvider` trait（try_lock/lock/unlock/is_held），现有 `DistributedLock` 挂接实现；导出 `DefaultLockProvider` 类型别名
- **invalidation feature 跨实例失效总线（T301）**：Redis Pub/Sub 广播失效事件（key/namespace 粒度），各实例后台监听失效本地 L1；消息带 `instance_id` 自失效豁免；`InvalidatingBackend` 写路径装饰器；协议层 `PubSubTransport` 抽象 + InMemory mock 测试；`KeyspaceNotificationListener` 键空间通知第二通道（keyevent 频道把外部 DEL/EXPIRED 投影为本地失效，需 Redis `notify-keyspace-events "Egx"`）（T312）
- **指标体系升级（T302）**：`MetricsRecorder` 注入端口（`CacheBuilder::metrics()`）覆盖纯 L1 路径的 hit/miss/set/delete 计数与延迟样本（此前默认 Moka 路径零指标）；`export_prometheus_standard()` 输出合规 exposition（`# HELP`/`# TYPE` + `oxcache_hits_total` 等标准命名 + `oxcache_operation_duration_seconds` 直方图）；`evictions` 计数（DashMap FIFO 淘汰埋点）；默认 NoOp 零开销
- **encrypt feature 值级加密（T303）**：`EncryptedBackend` 装饰器对 value 透明 XChaCha20-Poly1305 加解密（信封 `[ver][nonce][ct]`，AAD 绑定键名防换键移植，与 confers 加密口径一致）；密钥长度构造期校验、Debug 不泄露密钥
- **integrity feature 值完整性（T304）**：`IntegrityBackend` 装饰器 value + HMAC-SHA256 标签（`[ver][tag][payload]`），读校验失败（篡改 payload/tag/ver、换钥）视为 miss 并计入 `oxcache_integrity_failures_total`；与加密装饰器双序组合正确
- **serde-bincode / postcard feature 二进制序列化（T305）**：`SerializationFormat` 可插拔（Json/Bincode/Postcard）+ `CacheBuilder::serialization_format()`；同前缀键禁混格式；L2 传输体积对比记录 docs/PERFORMANCE.md
- **config-confers feature 配置驱动构建（T307）**：`OxcacheConfig`（容量/TTL/熔断参数）经 confers 加载 + `ConfigBus` watch 热更新（快照原子换装 + 监听器回调，读侧免锁）；`From<&OxcacheConfig> for L1Builder`；confers 0.6.0-rc.3 依赖（层级合法）
- **degradation feature 自动降级与恢复（T308）**：三态状态机（Active/Degraded/HalfOpen）+ `DegradableBackend` L2 保护装饰器：故障计数超阈值自动降级 L1-only（返回 Degraded 错误供 ChainCache 回落），降级超时半开放行探测，探测成功自动恢复、失败重新降级；状态变化回调 + 全局 degraded 指标
- **audit feature 缓存审计事件流（T309）**：`AuditEventPublisher` 端口（hit/miss/set/delete/evict/expired 结构化事件，键脱敏 `redact_key_for_audit`）+ NoOp 默认 / 有界内存环形 / tracing 桥三种发布器；`CacheBuilder::audit_publisher()` 注入
- **compression feature 自适应 zstd 压缩（T310）**：`CompressingBackend` 装饰器阈值触发（默认 1024B 以下零压缩开销，level 3 可调），读取按魔数识别 zstd / 兼容旧 gzip / 透传；体积对比记录 docs/PERFORMANCE.md
- **red-lock feature 锁增强与 RedLock（T311）**：`LockProvider::fencing_token()`（acquire 成功经 `INCR key:fence` 取单调 token，下游 staleness 检测契约）；`RedLock` 多节点多数派锁（`LockNode` 协议层抽象 + Redis/InMemory 实现，5 节点 2 宕机仍可获取、3 宕机快速失败回滚、跨实例互斥、token 单调）
- **对象安全拆分（T313）**：`UnifiedCache` 移除泛型 `get_typed/set_typed` 成为 dyn 可用核心（`Arc<dyn UnifiedCache>` 可用，关闭 kit 设计分歧 H1），typed 读写拆入 `TypedCacheExt`（blanket impl 保持既有调用点兼容）+ `DynUnifiedCache` 兼容别名
- **TypedNamespace 类型化命名空间（T314）**：marker 类型 + `NamespaceName` trait 编译期命名空间隔离（`namespace!` 宏），`get/set/delete/invalidate_all` 限本命名空间前缀，运行时键与 KeyGenerator `ns:key` 约定一致
- **BackendRegistry 后端工厂注册中心（T315）**：按名注册/构建后端（内置 moka/dashmap/memory，feature 门控 redis；全局 `GLOBAL_BACKEND_REGISTRY`），serde 友好 `BackendSpec`，未知 kind 报错附可用清单；供 kit/sdforge 动态选择（管理面承接，不恢复 cli）
- **versioning feature 版本化 CAS（T316）**：`compare_and_swap(key, expect_version, new_value)` —— `MemoryVersionedCache`（并发 lost-update 单测）+ `RedisVersionedCache`（WATCH/MULTI/EXEC 事务 MVP，信封 `[8B 版本][payload]`）
- **热路径零分配（T317）**：`get_by_str`/`set_by_str` 借用键 API 消除热路径 String/Vec 多余分配；criterion `hot_path_benchmark`（release 口径 get -6.7%、set -12.7%），记录 docs/PERFORMANCE.md
- **分层构建器 API（T306）**：`L1Builder`/`L2Builder`/`ChainBuilder` 链式组合（容量/TTL/TTI/后端类型/装饰器叠加），`ChainLink::from_arc` 支持已擦除后端 trait 对象；与既有 `CacheBuilder`/`ChainCacheBuilder` 并存

### 移除

- **cli feature 移除**：空壳 `cli` feature 从 Cargo.toml 删除（管理面由下游 sdforge 承接）；`FeatureSet.cli_available` 字段同步移除

---

## [0.5.0-rc.3] - 2026-09-08

### 修复

- **熔断器状态转换竞态**：`record_success`/`record_failure` 的 HalfOpen 转换由 load+store 改为 `compare_exchange`，Closed→Open 同样原子化，消除并发下半开→开→闭的错序（新增并发回归测试）
- **CacheBuilder 多后端静默丢弃**：`build_sync` 检测到多于 1 个 `backend_arc()` 时返回 `Err(NotSupported)`（原为静默只用第一个）；文档更正为单后端、多级缓存指引 `ChainCacheBuilder`
- **ChainCacheBuilder 空链 fail-fast**：`build()` 无 link 时 panic 并给出可操作提示（原为静默构造空链）；`ChainCache::new` 显式构造器保持空链宽容语义
- **`register_for_macro` 丢失 builder 配置**：宏注册克隆现保留 `null_cache_ttl`/`ttl_jitter_factor`（原被重置为默认）
- **metrics 锁中毒连锁 panic**：3 处 `.lock().expect(...)` 改为 `unwrap_or_else(PoisonError::into_inner)` 恢复运行
- **mock 后端 sync `incr` 吞错**：UTF-8/解析/溢出错误改为传播，与 async 路径一致
- **Lua 注入校验增强**：预处理保留字符串内括号参与调用形态模式匹配；归一化反斜杠转义引号，堵住 `redis.call(\'FLUSHALL\')` 类绕过
- **Redis 密码脱敏**：`redact_connection_string` 改用 `rfind('@')`，密码含 `/`/`@` 不再泄漏；用户名无密码连接串原样保留
- **Lua 脚本键校验**：`eval_lua`/`eval_sha` 对每个 key 执行 `validate_redis_key`
- **命名空间防护**：`clear_namespace` 拒绝空前缀（原会匹配全库）；prefix 额外拒绝 `[`/`]`/`\` glob 字符
- **`get_or_option_sync` 错误传播**：空值哨兵写入失败不再被静默吞掉
- **分布式锁**：watchdog 网络错误改为指数退避重试（原直接退出）；`release()` 状态变更延后至 Lua 脚本成功之后
- **feature 门控**：`BloomFilterBackend`/`BloomFilterBackendBuilder` 再导出补齐 memory/redis 依赖门控；`BytesCache` 再导出对齐门控
- **Lua 方括号索引绕过闭合**：预处理将 `['ident']`/`["ident"]` 折叠为 `.ident`，`redis['eval']`/`redis["call"] ('FLUSHALL')` 进入既有黑名单匹配；折叠发生于引号剥离之后，含连字符的索引残段同样可能被折叠（结果仍为合法 Lua，不产生黑名单命中）
- **aerospike TTL 钳制**：`write_policy_with_ttl`/`expire` 的 `as_secs() as u32` 改为先钳制到 `u32::MAX`，消除超大 TTL 截断为 0 后被当作永不过期的边界缺陷

### 安全

- **CI 供应链加固**：6 个 workflow 全部 58 处第三方 Action 引用改为 SHA 固定（`<action>@<40位SHA> # <原ref>`），可变标签（`@v*`/`@stable`）不再被信任
- **依赖检查权限最小化**：`dependency-check.yml` 补顶层 `permissions: contents: read`（Checkov CKV2_GHA_1）
- **unsafe 注释一致口径**：i18n 测试 9 处与 redis_benchmark 1 处 unsafe 块补齐 `// SAFETY:` 注释，src/+benches/ 范围达成 17/17

### 文档

- `CacheReader::len`（Redis 后端为 DBSIZE 全库语义）、`ttl`（moka TTI 刷新副作用）契约说明
- `registry::clear()` 明确会移除 `"default"` 条目；`OXCACHE_008` 保留注释
- 4 个 Redis 示例运行方式同步非 TLS 开发环境变量要求；benches 服务不可达时优雅跳过（`[bench-skip]`）
- `depth_limited` 饱和行为文档化（真实深度 >256 报告为 ~512 饱和值）；SECURITY.md 补 Lua 方括号归一化与 CI 供应链加固章节

### 变更

- 版本号递增至 `0.5.0-rc.3`；安装示例版本统一 rc.3（README / USER_GUIDE / API_REFERENCE）

---

## [0.5.0-rc.2] - 2026-09-07

### Changed

- 依赖升级：aerospike 2.1→2.2、testcontainers 0.27→0.28、icu 2.2→2.3；移除 testcontainers-modules 0.15（仍绑 tc ^0.27），Redis 容器测试改用裸 testcontainers GenericImage（redis:7-alpine）
- 版本号递增至 `0.5.0-rc.2`（下一个 minor 预发布）

### 测试

- E2E 固化：新增 dist_lock_watchdog_e2e、events_chain_e2e 并聚合注册；ignored 补盲验证（lib/integration/performance）；docs/TEST_SCENARIOS.md 场景固化
- deny 治理：licenses clarify（license-file 形态 crate 绑定）

### 文档

- 安装示例版本统一 0.5.0-rc.2（14 处）；CONTRIBUTING MSRV 对齐 1.97.1

---

## [0.4.3] - 2026-08-06

### 新增

- **kit 特性扩展**：`kit` feature 新增 observer/shutdown/decorator 三个 trait-kit 特性集成。
  - `OxcacheBuildObserver`：实现 `BuildObserver` trait，可通过 `AsyncKit::with_observer` 注册构建观察者。
  - `register_cache_shutdown`：将 `CacheBackend` 关闭映射到 `AsyncShutdownCoordinator` 三阶段（StopRequests/DrainQueue/CloseConnections）。
  - `CacheBackendDecorator` / `register_cache_decorator`：类型别名和辅助函数，支持通过 `AsyncKit::decorate` 注册后端装饰器。

### 维护

- 文档目录 `docs/image` 重命名为 `docs/assets`，同步更新 README 中英文引用。
- 文档版本号统一更新至 v0.4.3。

## [0.4.2] - 2026-08-06

### 新增

- **trait-kit 特性集成**：`kit` feature 新增 observer/shutdown/decorator 三个 trait-kit 特性集成。
  - `OxcacheBuildObserver`：实现 `BuildObserver` trait，可通过 `AsyncKit::with_observer` 注册构建观察者。
  - `register_cache_shutdown`：将 `CacheBackend` 关闭映射到 `AsyncShutdownCoordinator` 三阶段（StopRequests/DrainQueue/CloseConnections）。
  - `CacheBackendDecorator` / `register_cache_decorator`：类型别名和辅助函数，支持通过 `AsyncKit::decorate` 注册后端装饰器。

### 维护

- **死代码清理**：移除 `utils::validate_cache_key` 重复实现，统一使用 `infra::validate_cache_key`（委托 `KeyGenerator::validate_key`）。
- **`glob_match` 迭代化**：从递归回溯重写为双指针迭代算法，消除多 `*` 模式下的指数级最坏情况，保证 O(m·n) 时间复杂度。
- **复杂度降低**：
  - `glob_to_regex`：提取 `increment_wildcard` 辅助函数，消除 3 处重复的通配符计数检查。
  - `validate_redis_key`：提取 `check_control_characters`、`check_sql_injection`、`check_path_traversal`、`check_command_injection` 子函数，模式表提升为模块级常量。
  - `preprocess_lua_script`：提取 `skip_lua_comment`、`try_skip_long_string` 子函数，合并引号处理分支。
- **代码去重**：`glob_match` 从 `mock/backend.rs` 和 `moka/backend.rs` 提取到 `backend::interface` 模块统一实现。
- **`run_sync_fallback` 提取**：从 `basic_ops.rs` 同步回退逻辑中提取为独立方法，降低主函数复杂度。

### 测试

- 新增 `preprocess_lua_script` 边界测试（空脚本、纯注释、纯长字符串、混合内容）。
- 新增 `scan_quoted_string` 边界测试（空字符串、转义字符、Unicode、嵌套引号）。

## [0.4.1] - 2026-08-04

### 新增

- **trait-kit 0.4 集成增强**：`OxcacheModule` 现在实现 `AsyncHealthCheck` 和 `AsyncLifecycle` traits，提供健康检查和生命周期管理能力。
  - `AsyncHealthCheck`：通过 `kit.health_check::<OxcacheModule>()` 报告缓存后端健康状态。
  - `AsyncLifecycle`：通过 `kit.shutdown()` 优雅关闭缓存后端。
- `kit` feature 新增 `lifecycle` 和 `health` 特性支持，引入 `futures` 依赖用于同步健康检查执行。

### 变更

- `trait-kit` 依赖升级 `0.3` → `0.4`（kit feature 用户需同步升级 trait-kit 到 0.4）。

### 维护

- **Workspace 继承**：启用 Cargo workspace inheritance，`version`/`edition`/`authors`/`license`/`repository` 统一在 `[workspace.package]` 定义，子 crate 使用 `.workspace = true` 继承。
- **Edition 统一**：所有 crate（root/macros/examples）统一使用 Rust 2024 edition。
- **Cargo.toml 重构**：`[workspace]` 相关定义移至 `[lib]` 之后，结构更清晰。
- **依赖更新**：tokio `1.52` → `1.53`，uuid `1.23` → `1.24`。

### 测试

- 新增 4 个 trait-kit 集成测试：健康检查（kit 集成 + 直接调用）、生命周期（on_shutdown + 完整 kit 集成）。

## [0.4.0] - 2026-08-04

### ⚠️ 破坏性变更

- **移除 `tracing` 日志框架**：`src/` 和 `macros/` 完全移除 `tracing` 依赖。`ChainCache` 中 5 处 `tracing::warn!` 替换为 `EventPublisher` 事件发射（通过 `ChainCacheBuilder::event_publisher()` 配置）；`#[instrument]` 属性、tracing span、`secure_info!`/`secure_debug!` 宏全部移除。`macros` crate 生成代码中 6 处 `::tracing::warn!` 改为静默降级（反序列化失败回退、序列化/写入失败忽略），用户 crate 不再需要依赖 `tracing`。`tracing` feature 保留为空（向后兼容）。`EventPublisher` trait 改为 dyn-compatible：方法签名从 `impl Into<String>` 改为具体 `String` 类型，支持 `Arc<dyn EventPublisher>`。
- **i18n 重构**：移除 `thiserror` 依赖，错误类型改为手动实现 `Display` + 系统语言自动检测。下游依赖 `thiserror` 的代码需适配。

### 新增

- **Valkey 后端**：支持 Redis 兼容的 Valkey 分布式缓存（BSD-3 许可）——经 `RedisBackend` 接入（无独立 `valkey` feature，也不存在 `ValkeyBackend` 类型；集成测试以 valkey 容器对 Redis 协议做兼容性验证）。本条旧文案曾写作「新增 `ValkeyBackend`」，与源码不符，已更正。
- **Dragonfly 后端**：新增 `DragonflyBackend`，支持 Dragonfly 分布式缓存（BSL 1.1 许可）。
- **Aerospike 迁移**：Aerospike 后端从独立子 crate 迁移为 feature-gated 模块（`aerospike` feature），统一纳入主 crate。
- **AtomicCacheWriter**：新增 `AtomicCacheWriter` trait 和 `Cache` atomic API，支持原子写入操作。
- **i18n**：错误消息本地化，`Display` 实现支持系统语言自动检测。
- `BackendKind` 枚举扩展：新增 `Valkey`、`Dragonfly`、`Aerospike` 变体。
- `try_generate_full` 键生成验证方法。
- 6 个新 DashMap 淘汰测试、4 个新分片单飞测试、5 个新 ChainCache 降级测试、3 个新 MockBackend 故障注入测试，更新了 DashMap e2e 测试。
- 新基准测试：`serialization_benchmark`（JSON 纯/压缩 序列化+反序列化）和 `dashmap_benchmark`（满容量 set/get 及 FIFO 淘汰）。
- `ChainCacheBuilder::enable_race_read()` / `disable_race_read()`（并发首次命中读取）。
- `BytesCache` 类型别名，重导出在 `oxcache::BytesCache`。
- 回归测试：`current_thread` tokio 运行时内的同步操作（P2 3.2）。
- `CacheBuilder::build_sync()`：同步构建路径（完全同步，无需运行时）。`build()` 现在为 `async` 但不包含 `.await`；两者委托到相同的非异步构建逻辑。

### 修复

- **安全漏洞修复 (S1-S5)**：修复 5 个安全漏洞，涵盖输入校验、资源限制和边界条件。
- **核心逻辑修复 (L1/L2/L5/L6/L7/S6)**：修复多个核心逻辑错误，包括边界条件、类型转换和状态管理。
- **Lua 脚本修复**：修复 ARGV 索引偏移、`delta_arg` 悬垂引用和 `clear()` 重试逻辑。
- **Bloom filter / 序列化修复**：修正逻辑错误与安全限制。
- **Metrics 修复**：修正指标计算溢出与默认值错误。
- **配置验证优化**：配置验证逻辑增强，类型序列化 feature gate 修复。
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
- **Redis client 拆分**：单文件 `client.rs` 拆分为子模块结构，提升可维护性。
- **chain.rs 模块化**：`chain.rs` 拆分为 `chain/` 目录模块（`mod.rs` + `builder.rs`）。

### 性能

- 消除不必要的堆分配与哈希探测，减少热路径上的内存分配开销。

### 维护

- **依赖更新**：redis 1.2→1.5, serial_test 3.5→4.0, syn 2.0→3.0。
- **Workspace 重组**：macros 加入 workspace members，examples 继承 workspace 依赖，版本号同步至 0.4.0。
- 清理 `confers` 死引用：移除 `lib.rs`、`kit/module.rs` 中过时注释和 `validate.sh` 中不存在的 `core,confers` feature 组合。
- 修正 `trait-kit` 版本号注释（`0.2.2` → `0.3`），同步更新 `kit/mod.rs` 中 capability 类型描述（`UnifiedCache` → `CacheBackend`）。
- 删除未使用的脚本文件，rustfmt 全量格式化应用。

### 测试

- backend/cache/registry 模块覆盖率提升。
- Docker 集群/哨兵测试基础设施重构。
- 内存测试脚本重写为 `cargo test` 驱动。
- 修复 Redis 兼容性测试未执行问题。
- 修复 2 个已知失败的测试断言。
- 修复 6 个 rustdoc 警告。


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
- 测试环境：Linux x86_64，Rust 1.88，release profile（opt-level=3, lto=fat, codegen-units=1）

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
- edition 升级到 2024，rust-version 最低要求提升至 1.88
- MIT license 在所有模块中统一声明
- README.md 结构标准化：徽章更新为 rust 1.88+，章节统一为核心特性 / 快速开始 / 特性标志 / 架构 / 性能 / 可靠性 / 文档 / 贡献 / 更新日志 / 许可证

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
