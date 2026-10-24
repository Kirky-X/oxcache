# Changelog

> 本文件记录尚未发布的变更；已发布版本历史见 [docs/CHANGELOG.md](docs/CHANGELOG.md)。

## Unreleased

### Breaking Changes

- **移除 `RedisBackend::client()`**：此前把裸 `redis::Client` 交出去，调用方可
  绕过 oxcache 的安全门控直连 Redis。绕过路径按定义不提供替代；`mode()` /
  `ping()` 等受门控的访问器不受影响。
- **移除 `inklog` feature 与 `InklogAuditPublisher`**：oxcache 不再向 inklog 输出
  反向集成边。审计事件流仍由 `audit` feature 提供（`NoOpAuditPublisher` /
  `InMemoryAuditPublisher` / `TracingAuditPublisher`），端口
  `AuditEventPublisher` 保持稳定。
- **`#[cached]` 对带 `self` 的方法改为编译期报错**（下游既有代码编译失败）：
  默认键不含任何归属维度，不同租户会共享同一条缓存条目。方法必须给出
  `scope = <expr>`（如 `scope = self.tenant_id`）或带归属维度的
  `key = "..."` 模板。
- **`#[cached]` 方法上的常量键模板不再编译**：`key = "user_profile"` 这类
  与 `self` 无关的常量模板无法承载归属维度（所有租户共用一个常量键），
  现要求模板引用参数或 `self` 字段路径（如 `key = "user_{self.tenant_id}"`）。
  自由函数上的常量模板不受影响。
- **宏默认键格式由明文 Debug 改为 sha256 摘要**：实参以 `Debug` 串参与
  sha256 摘要，明文实参不进入键空间（实参常含口令 / 会话令牌）。全部宏
  缓存条目在升级后失效（键名变化）。
- **`HmacSigner::sign` / `verify` 签名变更**（公开 API 破坏）：标签覆盖
  **cache key + value**（键以长度前缀接入 MAC 输入），不可信 L2 无法把
  A 键的合法信封搬到 B 键下通过校验。`sign(&self, key: &str, message:
  &[u8])` / `verify(&self, key: &str, message: &[u8], tag: &[u8])`。
- **integrity 旧信封校验失败读为 miss**：升级即缓存失效（信封格式变更）。
- **`KeyGenerator` 段分隔符转义口径变更**：值中的 `:` 由长度前缀
  （`_<len>_<原值>`）改为百分号编码（`%3A`），转义标记 `%` 自身先行编码
  为 `%25`。长度前缀编码非单射，攻击者可构造 `_len_` 前缀让不同实参别名
  到同一个键。含 `:` / `%` 的实参生成的键名变化（相关缓存条目失效）。
- **键脱敏格式统一**：`redact_cache_key` 与 `redact_key_for_audit` 收敛到
  同一实现（此前同一公共事件类型存在两套格式，消费方按格式解析不一致）；
  标识符形态的段（邮箱、纯数字 ID、UUID、长十六进制摘要）掩码为 `<id>`。

### Added

- **模式级操作能力 `CachePatternOps`**：后端可原生完成「按 glob 模式计数 /
  删除」，不受单次键枚举上限约束（能力原语方法名为 `pattern_count` /
  `pattern_delete`）。`CacheBackend` 新增
  `delete_by_pattern` / `count_by_pattern`（默认实现退回「枚举 + 逐条
  删除」，支持流式操作的后端覆写）。
- **`MAX_KEYS_RETURN` 公开导出**（`oxcache::MAX_KEYS_RETURN`）：下游按
  自己的阈值判断键枚举结果时无需复制魔数。
- **`MokaMemoryBackend::try_keys_matching`**：匹配键数超限时返回错误
  （`keys_matching` 仍按上限截断，但其截断是静默的）。
- **失效总线可观测信号**：跨实例失效应用失败时计入
  `oxcache_invalidation_apply_failed_total`（metrics feature）并输出
  tracing warn（telemetry feature），不再静默变成空操作。

### Fixed

- **workspace 构建不再因同名 example 目标而失败**：主包与 `oxcache-examples` 包
  重复注册同一示例源文件（`example_redis_modes`），两个目标共享 target 目录的
  输出文件名（cargo 报 "output filename collision"），Windows 上并行链接时后写者
  抢不到输出文件（LNK1104），`cargo test --workspace` 整体 exit 101。改为只由
  `oxcache-examples` 包注册，运行命令随之改为
  `cargo run -p oxcache-examples --example example_redis_modes`；新增守卫测试钉住
  「同一 workspace 内 example 目标名唯一」。
- **磁盘后端模式删除不再每批从头重扫**：redb 键空间按键序游标续扫（每批从
  上一批末尾之后继续），一轮内全表只扫一遍；此前每批都从表头重新迭代，匹配
  键多时扫描量随轮数平方增长。
- **磁盘后端模式删除在并发写入下必然返回**：扫描轮数上限 8 轮、每轮访问
  预算取自该轮开始时的键数；此前并发写入方持续补匹配键时删除可能不退出
  （占用 blocking 线程、该 future 永不 resolve）。
- **模式删除返回实际删除条数**：磁盘与 Redis 后端改按删除操作的实际结果
  计数（键已被并发删除时不再计入返回值）。
- **磁盘后端全表扫描不再占用异步执行器**：`keys` 与模式计数移入
  `spawn_blocking`，百万级键空间的扫描不再占住 runtime worker 线程。
- **`CachePatternOps` 与 `CacheBackend` 不再同名**：能力 trait 的原语更名为
  `pattern_count` / `pattern_delete`（`CacheBackend` 侧仍是
  `count_by_pattern` / `delete_by_pattern`，为对外统一入口）。此前两个 trait
  同名，具体后端类型（经 blanket 实现同时是两个 trait 的实现）上的方法调用
  语法报 E0034（多个候选同时适用），外部用户按方法调用语法直接撞编译错误。
- **命名空间失效不再被键枚举上限打断**：`TypedNamespace::len` /
  `is_empty` / `invalidate_all` 改走模式级操作，命名空间条目数超过
  10 000 时仍能正确计数并整段失效（此前枚举报错且一条都删不掉）。
- **失效总线不再吞掉枚举错误**：命名空间失效失败会显性上报（计数 + 日志），
  跨实例缓存不一致不再无信号。
- **`ChainCache::keys` 不再静默截断**：后端枚举失败向上传播，调用方能判断
  结果是否完整。
- **Lua 命令级判定堵住函数引用别名**：`local f = redis.call` 后
  `f('FLUSHALL')` / `f 'FLUSHALL'`（含 `redis.pcall`、`redis['call']`、
  别名派生）与 `redis.call('FLUSHALL')` 等价，一律拒绝。
- **键枚举上限边界在各后端一致**：moka / dashmap / redis / disk 统一为
  「匹配键数超过 10 000 才报错」，恰好 10 000 条正常返回；dashmap 的上限
  此前误计不匹配条目。
- **`#[cached]` 键模板可引用 `self` 与参数**：模板占位符绑定到函数参数 /
  `self` 字段路径，占位符值按段分隔符转义后入键（此前模板逐字产出，
  `format!` 无实参，带占位符的模板无法编译）。

### Changed

- 跨仓自研依赖改为 `path` + `version` 双重声明：本地恒走工作树，发布时仍按 version 解析 registry。

### 迁移指引

- 需要「审计事件 → inklog 结构化日志」的下游，改用 inklog 侧 `oxcache-audit`
  feature 提供的 `inklog::integrations::OxcacheAuditPublisher`：构造
  `OxcacheAuditPublisher::new(sink)`（`sink` 为 `Arc<dyn inklog::sink::LogSink>`）
  并以 `Arc<dyn AuditEventPublisher>` 注入 oxcache builder 的 `audit_publisher`。
- 移除 `Cargo.toml` 中 `features = ["inklog"]`（或随 `full` 隐含）的启用，
  改为 `features = ["audit"]`。
- `examples/` 中的 `example_inklog_audit_bridge` 示例与 `inklog-bridge`
  examples feature 一并移除。
- 带 `self` 的 `#[cached]` 方法：改用 `scope = self.tenant_id`，或把常量
  键模板改为带占位符的模板（`key = "user_{self.tenant_id}_{id}"`）。
- `HmacSigner` 调用方补上 cache key 实参：`sign(key, message)` /
  `verify(key, message, tag)`。
- 依赖键名稳定性的下游：升级后 `#[cached]` 宏条目与含 `:` / `%` 的
  `KeyGenerator` 键会失效（键名变化），需接受一次缓存冷启动。
