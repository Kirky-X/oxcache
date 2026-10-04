# OxCache 简体中文消息目录（Fluent/FTL）。
#
# 键为 API 消息 ID 的连字符形式（`.` 与 `_` 统一转为 `-`，
# 如 `error.not_found` -> `error-not-found`）。
# 本文件经 `include_str!` 编译期内嵌 —— 与 locales/en/messages.ftl 保持键齐
# （键齐性守卫测试强制校验）。

# -- OxCacheError messages --
error-serialization = 序列化错误：{ $detail }。请检查数据格式并确保序列化器兼容。
error-operation = 操作失败：{ $detail }。请重试或检查请求。
error-connection = 连接错误：{ $detail }。请检查网络连接和服务器可用性。
error-not-found = 键未找到：{ $detail }。请求的键在缓存中不存在。
error-degraded = 缓存降级：{ $detail }。缓存正在以受限功能降级模式运行。
error-l1 = L1 缓存操作失败：{ $detail }。可能存在内存压力或配置问题。
error-l2 = L2 缓存操作失败：{ $detail }。请检查 Redis 连接和服务器状态。
error-not-supported = 操作不支持：{ $detail }。此功能在当前缓存类型下可能不可用。
error-wal = WAL（预写日志）操作失败：{ $detail }。请检查磁盘空间和文件权限。
error-database = 数据库错误：{ $detail }。请检查数据库连接和查询语法。
error-redis = Redis 连接失败：{ $detail }。
error-io = I/O 错误：{ $detail }。请检查文件权限和磁盘空间。
error-backend = 后端错误：{ $detail }。可能是暂时性问题，请重试。
error-timeout = 操作超时：{ $detail }。请考虑增加超时值或检查系统性能。
error-shutdown = 关闭错误：{ $detail }。部分资源可能未正确释放。
error-key-too-long = 键过长：{ $actual }。最大键长度为 { $max } 字节。
error-value-too-large = 值过大：{ $actual }。最大值大小为 { $max } 字节。
error-buffer-full = 缓冲区已满：{ $detail }。批量写入缓冲区已达容量上限。请稍后重试或增大缓冲区。
error-invalid-input = 无效输入：{ $detail }。提供的输入不符合所需格式或约束。
error-invalid-key = 无效键：{ $detail }。提供的键不符合所需格式或包含禁止字符。
error-lock = 锁错误：{ $detail }。锁可能因之前的 panic 而被毒害。
error-service-not-found = 服务未找到：{ $detail }。请求的服务配置在 UnifiedConfig 中不存在。
error-internal = 内部错误：{ $detail }。

# -- OxCacheConfigError messages --
config-missing-field = 缺少必需字段：{ $field }。
config-invalid-value = 字段 '{ $field }' 的值无效：{ $reason }。
config-unsupported-backend = 不支持的后端组合：{ $detail }。
config-connection-failed = 初始化时连接失败：{ $detail }。

# -- I18nError messages --
i18n-invalid-locale = 无效的区域设置 '{ $input }'：{ $reason }。
i18n-invalid-number = 无效的数字 '{ $input }'：{ $reason }。
i18n-date-error = 日期错误：{ $detail }。
i18n-format-error = 格式化错误：{ $detail }。

# -- #[cached] 宏生成的 strict 模式 panic --
macro-service-not-registered = oxcache：服务 '{ $service }' 未注册（严格模式）

# -- Tracing 日志消息（作为事件的 message 字段渲染） --
log-chain-read-completed = 链路读取完成
log-chain-expire-backend-failed = 后端过期操作失败
log-chain-iter-entries-key-failed = iter_entries 批量层处理键失败
log-offload-lifecycle-event = offload 生命周期事件
log-offload-timeout-policy-exceeded = offload 任务超出超时策略
log-degradation-entered = L2 降级已进入，仅以 L1 服务
log-degradation-half-open-probing = L2 降级半开，正在探测恢复
log-degradation-recovered = L2 降级已恢复
log-confers-config-reload-rejected = confers 配置重载被拒；保留先前快照
log-backend-bridge-shutdown-skipped = async→sync 桥接关闭已跳过：无法在 current_thread runtime 内嵌套阻塞驱动
log-disk-sweep-failed = 磁盘缓存清扫失败
log-audit-event = 缓存审计事件
log-stale-hit-via-get-or = 经 get_or 返回 stale 命中（Return 降级；后台再验证请使用 get_or_refresh）
log-stale-revalidation-scheduled = stale 命中；已调度后台再验证
log-stale-hit-served = 已按 stale 命中返回

# -- Panic / 不变量消息 --
panic-chain-parallel-freshest-invariant = 命中列表非空则新鲜度列表必非空
panic-bridge-temp-runtime-create-failed = 为桥接关闭创建临时 runtime 失败
panic-warmup-semaphore-not-closed = 预热信号量不会被 close
panic-audit-writer-lock-not-poisoned = writer 接收端锁不会中毒（临界区无 panic 点）
panic-audit-writer-rx-present-on-first-start = writer 首次启动时接收端必然在位
panic-stale-state-must-carry-payload = Stale 状态必须携带 payload
panic-bloom-capacity-must-be-positive = capacity 必须大于 0
panic-bloom-hash-count-must-be-positive = hash_count 必须大于 0
panic-bloom-fpr-must-be-in-open-interval = false_positive_rate 必须在 (0.0, 1.0) 区间内
panic-bloom-hash-count-unreachable = 容量 { $capacity } 下 hash_count { $hash_count } 不可达
panic-bloom-hash-count-backsolve-drift = 容量 { $capacity } 下 hash_count { $hash_count } 不可达：该容量下位图每字节使 k 的步进超过 1
panic-bloom-seed-generation-failed = 创建布隆过滤器失败：随机种子生成失败
panic-config-validate-moka-requires-memory = validate 拒绝未启用 memory feature 的 Moka
panic-config-validate-dashmap-requires-memory = validate 拒绝未启用 memory feature 的 DashMap
panic-config-validate-mock-requires-test-with-memory = validate 拒绝测试构建 + memory 组合之外的 Mock
panic-config-validate-redis-requires-redis-feature = validate 拒绝未启用 redis feature 的 Redis
panic-config-validate-dragonfly-requires-dragonfly-feature = validate 拒绝未启用 dragonfly feature 的 Dragonfly
panic-config-validate-disk-requires-disk-feature = validate 拒绝未启用 disk feature 的 Disk

# -- 用户可见明细消息（插值进错误模板） --
detail-disk-redb-open-failed = redb 打开失败：{ $err }
detail-disk-redb-create-failed = redb 创建失败：{ $err }
detail-disk-corrupt-envelope = 磁盘缓存 envelope 损坏
detail-not-supported-sync-atomic-increment = 所包裹的同步后端不支持原子递增
detail-not-supported-sync-atomic-cas = 所包裹的同步后端不支持原子比较并交换（compare-and-swap）
detail-not-supported-sync-atomic-set-if-absent = 所包裹的同步后端不支持原子 set-if-absent（不存在时写入）
detail-not-supported-async-atomic-increment = 所包裹的异步后端不支持原子递增
detail-not-supported-async-atomic-cas = 所包裹的异步后端不支持原子比较并交换（compare-and-swap）
detail-not-supported-async-atomic-set-if-absent = 所包裹的异步后端不支持原子 set-if-absent（不存在时写入）
detail-not-supported-sync-requires-runtime = 同步 API 需要 Tokio runtime：{ $err }
detail-not-supported-sync-requires-multi-thread-runtime = 同步 API 需要多线程 runtime；current_thread runtime 上 block_in_place 不可用
detail-config-env-invalid-value = { $key } 的值无效：{ $raw }（{ $err }）
detail-config-env-backend-requires-features = { $key }={ $raw } 需要 `memory`/`redis`/`disk` 中的 feature，当前构建一个也未启用
detail-config-env-serialization-requires-feature = { $key }={ $raw } 需要 `serialization` feature，当前构建未启用
detail-config-backend-requires-features = backend { $raw } 需要 `memory`/`redis`/`disk` 中的 feature，当前构建一个也未启用
detail-config-capacity-zero = capacity 必须大于 0（删除该键以使用 builder 默认值）
detail-config-capacity-exceeds-usize = capacity { $capacity } 超出本平台 usize 范围（{ $max }）
detail-config-ttl-zero = { $name } 不得为零；如需永不过期请用 None（不设置）
detail-config-metrics-requires-feature = metrics_enabled 需要 `metrics` feature，当前构建未启用
detail-config-serialization-requires-feature = serialization_format 需要 `serialization` feature，当前构建未启用
detail-config-serialization-bincode-requires-feature = serialization format 'bincode' 需要 `serde-bincode` feature，当前构建未启用（字段：{ $field }）
detail-config-serialization-postcard-requires-feature = serialization format 'postcard' 需要 `postcard` feature，当前构建未启用（字段：{ $field }）
detail-config-serialization-invalid-format = 无效的序列化格式（字段 { $field }）：{ $raw }（应为 json/bincode/postcard 之一）
detail-config-circuit-breaker-threshold-zero = circuit_breaker_failure_threshold 必须大于 0
detail-config-service-name-empty = service_name 不得为空；删除该键以保持服务维度关闭
detail-config-connection-pool-size-zero = connection_pool_size 必须大于 0（删除该键以使用后端默认值）
detail-adaptive-ttl-min-ttl-exceeds-max = adaptive_ttl min_ttl（{ $min }）不得大于 max_ttl（{ $max }）
detail-adaptive-ttl-multiplier-not-finite-positive = adaptive_ttl hot_ttl_multiplier（{ $value }）必须为有限正数
detail-adaptive-ttl-divisor-at-least-one = adaptive_ttl cold_ttl_divisor 至少为 1
detail-adaptive-ttl-max-tracked-keys-at-least-one = adaptive_ttl max_tracked_keys 至少为 1
detail-get-or-leader-result-not-cached = get_or：并发拉取的 leader 未能将结果写入缓存
detail-get-or-option-leader-result-not-cached = get_or_option：并发拉取的 leader 未能将结果写入缓存
detail-warmup-ttl-lookup-failed = ttl 查询失败：{ $err }
detail-redis-ttl-min-millis = Redis SET PX/PEXPIRE 的 TTL 至少须为 1 毫秒
detail-redis-ttl-exceeds-max = TTL { $millis }ms 超出 Redis 上限 { $max }ms（约 68 年）
detail-redis-cluster-connect-failed = Redis Cluster 连接失败：{ $err }
detail-redis-cluster-connect-timeout = 连接超时：Redis Cluster 不可用
detail-confers-expects-u64 = confers 键 '{ $key }' 应为 u64，实际为 { $value }
detail-confers-value-exceeds-range = confers 键 '{ $key }' 的值 { $value } 超出 { $target } 的取值范围
detail-confers-expects-bool = confers 键 '{ $key }' 应为 bool，实际为 { $value }
detail-confers-expects-f64 = confers 键 '{ $key }' 应为 f64，实际为 { $value }
detail-confers-expects-string = confers 键 '{ $key }' 应为字符串，实际为 { $value }
detail-confers-read-failed = confers 读取 '{ $key }' 失败：{ $err }
detail-builder-no-backend-requires-memory = CacheBuilder 未提供后端时需要 `memory` feature（默认 Moka）；否则请显式传入 .backend_arc()。
detail-builder-stale-ttl-sync-conflict = stale_ttl 不能与 sync_mode(true) 组合使用；同步 API 绕过装饰器，将看到不完整的 stale 语义
detail-builder-adaptive-ttl-sync-conflict = adaptive_ttl 不能与 sync_mode(true) 组合使用；同步 API 绕过装饰器，将看到未经调整的 TTL
detail-builder-adaptive-ttl-stale-conflict = adaptive_ttl 不能与 stale_ttl 组合使用；叠加两个 TTL 改写器的语义未定义
detail-config-backend-requires-feature = backend 需要 `{ $feature }` feature，当前构建未启用
detail-config-backend-kind-requires-feature = backend `{ $kind }` 需要 `{ $kind }` feature，当前构建未启用
detail-config-backend-mock-test-only = backend `mock` 仅存在于测试构建，当前不可用
detail-config-backend-aerospike-programmatic = backend `aerospike` 需要 namespace/set 配置，必须以编程方式构建，不能经由 CacheConfig
detail-config-backend-valkey-no-impl = backend `valkey` 没有实现；请改用 `redis`（协议兼容）或 `dragonfly`
detail-config-backend-chain-needs-builder = backend `chain` 必须经 ChainBuilder 组装，不能作为单个 CacheConfig backend 使用
detail-config-backend-unknown-kind = backend `{ $kind }` 无法从配置构建
detail-config-backend-not-config-buildable = backend `{ $kind }` 无法经由 CacheConfig 构建（已被 validate 拒绝，或需程序化组装）
detail-config-env-not-unicode = 环境变量 { $key } 的值不是有效的 Unicode：{ $raw }
detail-config-env-invalid-bool = { $key } 的 bool 值无效：{ $raw }（应为 true/1/yes/on 或 false/0/no/off）
detail-config-backend-invalid-value = { $key } 的值无效：{ $raw }（应为 moka/dashmap/redis/valkey/dragonfly/aerospike/chain/mock/disk 之一）
detail-disk-open-create-failed = 磁盘后端打开失败（{ $open_err }）且创建失败（{ $create_err }）：{ $path }
detail-cache-memory-requires-feature = Cache::memory() 需要 `memory` feature；请改用 Cache::new_with_backend 构建。
detail-not-supported-moka-sync-current-thread = Moka 同步面无法在 current-thread runtime 的异步上下文中驱动（tokio 禁止嵌套阻塞驱动）。请在 runtime 之外调用同步 API，或改用 multi_thread runtime（block_in_place 可自动处理）。
detail-redis-unexpected-info-reply = INFO { $section } 应答类型异常
detail-chain-get-many-length-mismatch = get_many 对 { $expected } 个键返回了 { $returned } 个结果
detail-chain-parallel-freshest-all-failed = 并行最新值读取期间所有后端均失败

# -- 示例二进制输出消息 --
example-inklog-bridge-title = === inklog 审计日志桥接示例 ===
example-inklog-bridge-observability = === 桥接可观测面 ===
example-inklog-bridge-dropped = dropped（无 runtime 上下文丢弃）: { $count }
example-inklog-bridge-write-failures = write_failures（sink 写失败）: { $count }
example-redis-modes-feature-disabled = 当前编译未启用 redis feature，无演示内容。
example-redis-modes-run-with-redis-feature = 完整运行：cargo run --features redis --example example_redis_modes
example-redis-modes-run-with-examples-package = 或：cargo run -p oxcache-examples --example example_redis_modes
example-redis-modes-standalone-connect-failed = ✗ Standalone 连接失败：{ $err }
example-redis-modes-standalone-env-required = 需要运行中的 Redis（可用 REDIS_URL 覆盖，默认 redis://127.0.0.1:6379），跳过 Standalone/Builder 演示
example-redis-modes-done-standalone-skipped = ✓ 示例完成（Standalone 因环境不可用而跳过）
