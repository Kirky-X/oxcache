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
log-chain-backfill-succeeded = 回填成功
log-chain-backfill-failed = 回填失败
log-offload-lifecycle-event = offload 生命周期事件
log-offload-timeout-policy-exceeded = offload 任务超出超时策略
log-degradation-entered = L2 降级已进入，仅以 L1 服务
log-degradation-half-open-probing = L2 降级半开，正在探测恢复
log-degradation-recovered = L2 降级已恢复
log-confers-config-reload-rejected = confers 配置重载被拒；保留先前快照
log-backend-bridge-shutdown-skipped = async→sync 桥接关闭已跳过：无法在 current_thread runtime 内嵌套阻塞驱动
log-disk-sweep-failed = 磁盘缓存清扫失败
log-disk-open-create-failed = 磁盘后端打开/创建失败
log-audit-event = 缓存审计事件
log-stale-hit-via-get-or = 经 get_or 返回 stale 命中（Return 降级；后台再验证请使用 get_or_refresh）
log-stale-revalidation-scheduled = stale 命中；已调度后台再验证
log-stale-hit-served = 已按 stale 命中返回
log-invalidation-apply-failed = 跨实例失效事件未能应用到本地缓存
log-pubsub-reconnect-failed = pubsub 重连失败
log-pubsub-handler-panicked = pubsub 回调 panic，继续订阅
log-pubsub-payload-parse-failed = pubsub 消息体解析失败
log-pubsub-max-reconnect-attempts-reached = pubsub 消息流结束，已达最大重连次数
log-pubsub-reconnecting = pubsub 消息流结束（连接断开），正在重连
log-pubsub-resubscribe-failed = pubsub 重新订阅失败
log-macro-passthrough = 缓存直通（服务未注册）
log-circuit-breaker-transition = 熔断器状态迁移

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
panic-canonical-serialize-value-cannot-fail = 对 Value 的序列化不会失败

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
detail-redis-key-sql-injection-pattern = Redis 键包含可疑的 SQL 注入模式：{ $detail }
detail-redis-sql-injection-auth-bypass = SQL 认证绕过
detail-redis-sql-injection-comment = SQL 注释模式
detail-redis-sql-injection-delete = SQL DELETE 语句
detail-redis-sql-injection-drop = SQL DROP 语句
detail-redis-sql-injection-insert = SQL INSERT 语句
detail-redis-sql-injection-quote-or = 单引号后跟 OR 模式
detail-redis-sql-injection-tautology = 经典 SQL 注入永真条件
detail-redis-sql-injection-union-select = SQL UNION 查询
detail-redis-sql-injection-xp-cmdshell = SQL Server 命令执行
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
detail-disk-open-create-failed = 磁盘后端打开失败（{ $open_err }）且创建失败（{ $create_err }）
detail-cache-memory-requires-feature = Cache::memory() 需要 `memory` feature；请改用 Cache::new_with_backend 构建。
detail-not-supported-moka-sync-current-thread = Moka 同步面无法在 current-thread runtime 的异步上下文中驱动（tokio 禁止嵌套阻塞驱动）。请在 runtime 之外调用同步 API，或改用 multi_thread runtime（block_in_place 可自动处理）。
detail-redis-unexpected-info-reply = INFO { $section } 应答类型异常
detail-redis-key-control-character = Redis 键包含控制字符：U+{ $code }
detail-redis-key-path-traversal-pattern = Redis 键包含路径遍历模式：{ $detail }
detail-redis-key-command-injection-character = Redis 键包含潜在命令注入字符：{ $detail }
detail-redis-scan-pattern-too-long = SCAN 模式超出最大长度 { $max } 字符（实际 { $actual } 字符）
detail-redis-scan-pattern-too-many-wildcards = SCAN 模式包含过多通配符（最多 { $max } 个，实际 { $actual } 个）
detail-pubsub-client-open-failed = pubsub 客户端打开失败：{ $err }
detail-pubsub-connection-failed = pubsub 连接失败：{ $err }
detail-pubsub-publish-failed = pubsub 发布失败：{ $err }
detail-pubsub-connect-failed = pubsub 连接失败：{ $err }
detail-pubsub-subscribe-failed = pubsub 订阅失败：{ $err }
detail-pubsub-subscribe-task-ended = pubsub 订阅任务在就绪前结束
detail-lua-script-exceeds-max-length = Lua 脚本超出最大长度 { $max } 字节（实际 { $actual } 字节）
detail-lua-script-exceeds-max-key-count = Lua 脚本超出最大键数量 { $max }（实际 { $actual } 个）
detail-lua-script-forbidden-pattern = Lua 脚本包含禁止的模式：{ $detail }
detail-lua-nested-eval = Lua 脚本包含嵌套的 redis.eval/evalsha
detail-lua-infinite-loop-patterns = Lua 脚本包含潜在的无限循环模式
detail-lua-infinite-loop-while-true = WHILE TRUE 循环
detail-lua-infinite-loop-while-1 = WHILE 1 循环
detail-lua-infinite-loop-repeat = REPEAT 循环
detail-lua-infinite-loop-goto = GOTO 语句
detail-backend-layer-restriction-l1-only = 仅支持 L1 层级
detail-backend-layer-restriction-l2-and-l3-only = 仅支持 L2/L3 层级
detail-backend-layer-restriction-any = 支持任意层级
detail-chain-get-many-length-mismatch = get_many 对 { $expected } 个键返回了 { $returned } 个结果
detail-chain-parallel-freshest-all-failed = 并行最新值读取期间所有后端均失败
detail-chain-all-backends-failed-read = 顺序读取期间所有后端均失败
detail-chain-all-backends-failed-race-read = 竞速读取期间所有后端均失败
detail-chain-all-backends-failed-write = 所有后端写入失败
detail-chain-all-backends-failed-delete = 所有后端删除失败：{ $errors }
detail-chain-all-backends-failed-clear = 所有后端清空失败：{ $errors }
detail-chain-sync-requires-sync-backend = 链路 sync API 要求所有链接都支持 SyncCacheBackend
detail-chain-no-backends = 链路中没有后端
detail-chain-health-check-timed-out = health_check 5 秒后超时
detail-chain-health-check-failed = health_check 对 { $count } 个后端失败：{ $failures }
detail-chain-incr-no-atomic-writer = incr：链路中没有实现 AtomicCacheWriter 的链接
detail-chain-cas-no-atomic-writer = compare_and_swap：链路中没有实现 AtomicCacheWriter 的链接
detail-chain-set-if-absent-no-atomic-writer = set_if_absent：链路中没有实现 AtomicCacheWriter 的链接
detail-key-generator-part-contains-segment-separator = json_hash_key 的 parts 不得包含 ':' 段分隔符：'{ $part }'

# -- Bench 诊断消息 --
bench-skip-service-unreachable = [bench-skip] 所需服务不可达：{ $services }；请按 bench 模块文档启动服务后重跑（例如 docker run -d -p 6379:6379 redis:7-alpine）

# -- 示例二进制输出消息 --
example-redis-modes-feature-disabled = 当前编译未启用 redis feature，无演示内容。
example-redis-modes-run-with-redis-feature = 完整运行：cargo run -p oxcache-examples --features redis --example example_redis_modes
example-redis-modes-run-with-examples-package = 或：cargo run -p oxcache-examples --example example_redis_modes
example-redis-modes-standalone-connect-failed = ✗ Standalone 连接失败：{ $err }
example-redis-modes-standalone-env-required = 需要运行中的 Redis（可用 REDIS_URL 覆盖，默认 redis://127.0.0.1:6379），跳过 Standalone/Builder 演示
example-redis-modes-done-standalone-skipped = ✓ 示例完成（Standalone 因环境不可用而跳过）

# -- example_basic_operations.rs --
example-basic-operations-set-user = 设置用户: { $name }
example-basic-operations-getting-user = 获取用户...
example-basic-operations-got-user = 获取的用户: { $name } ({ $email })
example-basic-operations-deleting-user = 删除用户...
example-basic-operations-updating-user = 更新用户...
example-basic-operations-updated-user = 更新的用户: { $name } ({ $email })
example-basic-operations-example-complete = ✓ 基本操作示例完成!
example-basic-operations-set-store-a-value = - Set: 在缓存中存储值
example-basic-operations-get-read-a-value = - Get: 从缓存中获取值
example-basic-operations-delete-remove-a-value = - Delete: 从缓存中删除值
example-basic-operations-update-overwrite-the-existing = - Update: 覆盖现有值

# -- example_byte_ops.rs --
example-byte-ops-byte-level-operations-example = === 字节级操作示例 ===
example-byte-ops-1-basic-byte-operations = --- 1. 基本字节操作 ---
example-byte-ops-write-bytes = 写入: { $data } 字节
example-byte-ops-read = 读取: { $retrieved }
example-byte-ops-2-byte-operations-without = --- 2. 无 TTL 的字节操作 ---
example-byte-ops-write-bytes-no-ttl = 写入: { $data2 } 字节（无 TTL）
example-byte-ops-read-bytes = 读取: { $retrieved2 } 字节
example-byte-ops-3-binary-data = --- 3. 二进制数据 ---
example-byte-ops-write-bytes-of-binary = 写入: { $binary_data } 字节二进制数据
example-byte-ops-read-bytes-2 = 读取: { $data } 字节
example-byte-ops-checksum = 校验: { $result }
example-byte-ops-checksum-match = ✓ 一致
example-byte-ops-checksum-mismatch = ✗ 不一致
example-byte-ops-4-cache-metadata-operations = --- 4. 缓存元信息操作 ---
example-byte-ops-entries = 条目数: { $count }
example-byte-ops-is-empty = 是否为空: { $empty }
example-byte-ops-capacity = 容量: { $cap }
example-byte-ops-5-serializer-access = --- 5. 序列化器访问 ---
example-byte-ops-serializer-instance-obtained-dyn = ✓ 已获取序列化器实例（dyn Serializer 不实现 Debug，无法打印）
example-byte-ops-6-large-data-operations = --- 6. 大数据操作 ---
example-byte-ops-write-bytes-2 = 写入: { $large_data } 字节
example-byte-ops-read-bytes-3 = 读取: { $retrieved4 } 字节
example-byte-ops-after-cleanup-entries-is = 清理后: 条目数={ $count_after }, 为空={ $empty_after }
example-byte-ops-cache-closed = ✓ 缓存已关闭
example-byte-ops-example-complete = ✓ 示例完成

# -- example_cache_builder.rs --
example-cache-builder-cachebuilder-full-usage-example = === CacheBuilder 完整用法示例 ===
example-cache-builder-1-basic-construction = --- 1. 基本构建 ---
example-cache-builder-basic-cache-created = ✓ 创建基本缓存
example-cache-builder-2-set-capacity = --- 2. 设置容量 ---
example-cache-builder-cache-created-with-capacity = ✓ 创建容量为 10000 的缓存
example-cache-builder-3-set-ttl = --- 3. 设置 TTL ---
example-cache-builder-cache-created-with-1 = ✓ 创建 TTL 为 1 小时的缓存
example-cache-builder-4-set-tti = --- 4. 设置 TTI ---
example-cache-builder-cache-created-with-5 = ✓ 创建 TTI 为 5 分钟的缓存
example-cache-builder-5-combined-configuration = --- 5. 组合配置 ---
example-cache-builder-combined-cache-created-capacity = ✓ 创建组合配置缓存（容量=5000, TTL=30min, TTI=10min）
example-cache-builder-6-use-a-custom = --- 6. 使用自定义后端 ---
example-cache-builder-cache-created-with-a = ✓ 创建使用自定义 Moka 后端的缓存
example-cache-builder-7-real-usage = --- 7. 实际使用 ---
example-cache-builder-write = 写入: { $product }
example-cache-builder-read = 读取: { $cached }
example-cache-builder-8-set-with-ttl = --- 8. 带 TTL 的设置 ---
example-cache-builder-storing-a-product-with = 写入带 60s TTL 的产品
example-cache-builder-exists-check = 存在检查: { $exists }
example-cache-builder-9-statistics = --- 9. 统计信息 ---
example-cache-builder-cache-stats = 缓存统计:
example-cache-builder-10-health-check = --- 10. 健康检查 ---
example-cache-builder-health-status-ok = 健康状态: ✓ 正常
example-cache-builder-example-complete = ✓ 示例完成

# -- example_cache_key.rs --
example-cache-key-cachekey-trait-example = === CacheKey trait 示例 ===
example-cache-key-1-cachekey-for-built = --- 1. 内置类型的 CacheKey 实现 ---
example-cache-key-2-cachekey-for-custom = --- 2. 自定义类型的 CacheKey 实现 ---
example-cache-key-3-using-cachekey-in = --- 3. 在缓存中使用 CacheKey ---
example-cache-key-store-key-product = 存储: key='{ $key1 }', product={ $product }
example-cache-key-read = 读取: { $cached }
example-cache-key-store-key-product-2 = 存储: key='{ $key2 }', product={ $another_product }
example-cache-key-4-batch-operations = --- 4. 批量操作演示 ---
example-cache-key-store-key = 存储: key='{ $key }'
example-cache-key-batch-read-results = 批量读取: { $results } 个结果
example-cache-key-5-key-format-consistency = --- 5. 键格式一致性验证 ---
example-cache-key-userid-100-userid-100 = UserId(100) == UserId(100): { $key_b } (应该为 true)
example-cache-key-userid-100-userid-200 = UserId(100) == UserId(200): { $key_c } (应该为 false)
example-cache-key-same-value-produces-same = 相同值产生相同键: { $to_key_string }
example-cache-key-cachekey-trait-example-complete = === CacheKey trait 示例完成 ===
example-cache-key-key-points = 关键点:
example-cache-key-built-in-types-string = - 内置类型 (String, u64, i64 等) 已有 CacheKey 实现
example-cache-key-custom-types-can-generate = - 自定义类型可以通过实现 CacheKey trait 生成缓存键
example-cache-key-to-key-string-should = - to_key_string() 应该返回确定性的、唯一的字符串表示
example-cache-key-cachekey-can-be-used = - CacheKey 可以在 set/get/delete 等操作中使用

# -- example_cached_macro.rs --
example-cached-macro-executing-the-original-function = 执行原始函数逻辑 (数据库查询)...
example-cached-macro-executing-the-original-function-2 = 执行原始函数逻辑 (自定义键)...
example-cached-macro-executing-the-original-function-3 = 执行原始函数逻辑 (热点数据)...
example-cached-macro-usage-example = === #[cached] 宏使用示例 ===
example-cached-macro-cache-registered-for-use = ✓ 缓存已注册供 #[cached] 宏使用
example-cached-macro-1-basic-caching-demonstration = 1. 基础缓存功能演示
example-cached-macro-first-call-of-get = 第一次调用 get_user(1) - 执行函数逻辑 + 缓存结果...
example-cached-macro-first-call-result = 第一次调用结果: { $user }
example-cached-macro-second-call-of-get = 第二次调用 get_user(1) - 直接从缓存返回...
example-cached-macro-second-call-result = 第二次调用结果: { $cached_user }
example-cached-macro-2-custom-key-format = 2. 自定义键格式演示
example-cached-macro-calling-get-user-custom = 调用 get_user_custom_key(2) - 使用 'user_' 前缀键格式...
example-cached-macro-result = 结果: { $user_custom }
example-cached-macro-calling-get-user-custom-2 = 再次调用 get_user_custom_key(2) - 从缓存返回...
example-cached-macro-cached-result = 缓存结果: { $cached_custom }
example-cached-macro-3-short-ttl-caching = 3. 短 TTL 缓存演示
example-cached-macro-calling-get-hot-data = 调用 get_hot_data(1) - 使用较短 TTL（300s）...
example-cached-macro-result-2 = 结果: { $hot_data }
example-cached-macro-calling-get-hot-data-2 = 再次调用 get_hot_data(1) - 从缓存返回...
example-cached-macro-cached-result-2 = 缓存结果: { $cached_hot_data }
example-cached-macro-4-performance-comparison = 4. 性能对比演示
example-cached-macro-cache-hits-are-typically = 缓存命中通常比原始函数执行快得多
example-cached-macro-first-call-took = 首次调用耗时: { $first_duration }
example-cached-macro-cache-hit-took = 缓存命中耗时: { $second_duration }
example-cached-macro-speedup-x = 性能提升: { $f64 }x
example-cached-macro-executing-the-original-function-4 = 执行原始函数逻辑 (多参数)...
example-cached-macro-5-multi-argument-function = 5. 多参数函数缓存演示
example-cached-macro-calling-get-user-with = 调用 get_user_with_role(4, "admin")...
example-cached-macro-result-3 = 结果: { $result }
example-cached-macro-calling-with-the-same = 再次调用相同参数 - 从缓存返回...
example-cached-macro-cached-result-3 = 缓存结果: { $cached_result }
example-cached-macro-calling-with-different-arguments = 调用不同参数 - 执行原始函数...
example-cached-macro-different-argument-result = 不同参数结果: { $diff_result }
example-cached-macro-example-complete = === #[cached] 宏示例完成 ===
example-cached-macro-main-advantages-of-the = #[cached] 宏的主要优势:
example-cached-macro-zero-boilerplate-caching-logic = - 零样板代码：自动处理缓存逻辑
example-cached-macro-type-safe-checked-at = - 类型安全：编译时检查
example-cached-macro-flexible-configuration-supports-multiple = - 灵活配置：支持多种缓存策略
example-cached-macro-automatic-serialization-no-manual = - 自动序列化：无需手动处理
example-cached-macro-performance-significantly-improves-cache = - 性能优化：显著提升缓存命中性能

# -- example_comprehensive_usage.rs --
example-comprehensive-usage-oxcache-comprehensive-usage-example = === Oxcache 综合使用示例 ===
example-comprehensive-usage-1-create-an-in = 1. 创建内存缓存 (L1)
example-comprehensive-usage-in-memory-cache-created = ✓ 内存缓存创建成功
example-comprehensive-usage-2-create-a-redis = 2. 创建 Redis 缓存 (L2 only)
example-comprehensive-usage-redis-cache-created = ✓ Redis 缓存创建成功
example-comprehensive-usage-redis-unavailable-degraded = ⚠ Redis 不可用，已降级: { $e }
example-comprehensive-usage-3-create-a-tiered = 3. 创建分层缓存 (L1 + L2)
example-comprehensive-usage-tiered-cache-created = ✓ 分层缓存创建成功
example-comprehensive-usage-4-basic-crud-operations = 4. 基本 CRUD 操作演示
example-comprehensive-usage-adding-user = 添加用户...
example-comprehensive-usage-user-added = ✓ 用户添加成功: { $name }
example-comprehensive-usage-getting-user = 获取用户...
example-comprehensive-usage-user-retrieved = ✓ 用户获取成功: { $name }
example-comprehensive-usage-user-not-found = ✗ 用户未找到
example-comprehensive-usage-updating-user = 更新用户...
example-comprehensive-usage-user-updated = ✓ 用户更新成功
example-comprehensive-usage-user-after-the-update = ✓ 更新后用户: { $name }
example-comprehensive-usage-user-not-found-2 = ✗ 用户未找到
example-comprehensive-usage-deleting-user = 删除用户...
example-comprehensive-usage-failed-to-delete-the = ✗ 用户删除失败
example-comprehensive-usage-user-deleted = ✓ 用户删除成功
example-comprehensive-usage-5-batch-operations = 5. 批量操作演示
example-comprehensive-usage-adding-products-in-batch = 批量添加商品...
example-comprehensive-usage-batch-add-succeeded-products = ✓ 批量添加成功 ({ $products } 个商品)
example-comprehensive-usage-fetching-products-in-batch = 批量获取商品...
example-comprehensive-usage-batch-fetch-succeeded-products = ✓ 批量获取成功 ({ $fetched } 个商品)
example-comprehensive-usage-deleting-products-in-batch = 批量删除商品...
example-comprehensive-usage-batch-delete-succeeded = ✓ 批量删除成功
example-comprehensive-usage-6-ttl-control = 6. TTL 控制演示
example-comprehensive-usage-adding-data-that-expires = 添加 3 秒过期的数据...
example-comprehensive-usage-immediate-get = 立即获取: { $await }
example-comprehensive-usage-waiting-4-seconds-then = 等待 4 秒后获取...
example-comprehensive-usage-get-after-4-seconds = 4 秒后获取: { $await }
example-comprehensive-usage-7-performance-test = 7. 性能测试
example-comprehensive-usage-writing-10000-entries-concurrently = 并发写入 10000 条数据...
example-comprehensive-usage-write-took = ✓ 写入耗时: { $write_time }
example-comprehensive-usage-reading-10000-entries-concurrently = 并发读取 10000 条数据...
example-comprehensive-usage-read-took = ✓ 读取耗时: { $read_time }
example-comprehensive-usage-8-statistics = 8. 统计信息
example-comprehensive-usage-cache-stats = 缓存统计:
example-comprehensive-usage-example-run-complete = === 示例运行完成 ===

# -- example_get_or.rs --
example-get-or-db-loading-user-from = [DB] 正在从数据库加载用户 { $id }...
example-get-or-single-flight = === get_or 单飞模式示例 ===
example-get-or-1-basic-usage = --- 1. 基本用法 ---
example-get-or-first-get-or-call = 第一次调用 get_or:
example-get-or-result = 结果: { $user1 }
example-get-or-second-get-or-call = 第二次调用 get_or:
example-get-or-result-2 = 结果: { $user2 }
example-get-or-2-concurrent-single-flight = --- 2. 并发 single-flight 去重 ---
example-get-or-request-started = 请求 { $i } 开始
example-get-or-request-completed = 请求 { $i } 完成
example-get-or-note-only-one-db = 注意：只有一次 [DB] 加载，其余请求等待结果
example-get-or-3-comparison-concurrency-without = --- 3. 对比：无 single-flight 的并发场景 ---
example-get-or-note-without-single-flight = 注意：没有 single-flight 时，可能多次执行 [DB] 加载
example-get-or-example-complete = ✓ 示例完成

# -- example_new_api.rs --
example-new-api-usage-example = 新API使用示例
example-new-api-1-in-memory-cache = 1. 内存缓存 (仅L1)
example-new-api-user-retrieved-from-memory = ✓ 从内存缓存检索用户: { $name }
example-new-api-2-redis-cache-l2 = 2. Redis缓存 (仅L2)
example-new-api-user-retrieved-from-redis = ✓ 从Redis缓存检索用户: { $name }
example-new-api-redis-unavailable-skipping-redis = ⚠ Redis不可用，跳过Redis测试: { $e }
example-new-api-3-custom-key-type = 3. 自定义键类型
example-new-api-user-retrieved-with-custom = ✓ 使用自定义键检索用户: { $name }
example-new-api-4-advanced-configuration-using = 4. 高级配置 (使用 Builder)
example-new-api-user-retrieved-from-the = ✓ 从高级配置缓存检索用户: { $name }
example-new-api-5-ttl-operations-using = 5. TTL操作 (使用 set_with_ttl)
example-new-api-set-user-7-with = ✓ 设置带60秒TTL的用户:7
example-new-api-updated-the-ttl-of = ✓ 更新用户:7的TTL为120秒
example-new-api-6-batch-operations = 6. 批量操作
example-new-api-batch-set-users = ✓ 批量设置 { $batch } 个用户
example-new-api-retrieved-users-from-the = ✓ 从缓存检索 { $retrieved_count } 个用户
example-new-api-7-delete-operations = 7. 删除操作
example-new-api-user-8-exists = ✓ 用户:8存在: { $exists }
example-new-api-user-8-exists-after = ✓ 删除后用户:8存在: { $exists }
example-new-api-8-clear-operations = 8. 清空操作
example-new-api-set-3-users-in = ✓ 在缓存中设置3个用户
example-new-api-cleared-all-cache-entries = ✓ 已清除所有缓存条目
example-new-api-user-1-exists-after = ✓ 清空后用户:1存在: { $exists }
example-new-api-9-cache-aside-mode = 9. 带回退的缓存旁路模式 (get_or)
example-new-api-user-retrieved-via-the = ✓ 通过回退函数检索用户: { $name }
example-new-api-10-summary = 10. 总结
example-new-api-features = 新API特性:
example-new-api-type-safe-cache-interface = ✓ 类型安全的缓存接口
example-new-api-in-memory-cache-cache = ✓ 内存缓存 (Cache::memory())
example-new-api-redis-cache-cache-redis = ✓ Redis缓存 (Cache::redis())
example-new-api-custom-key-types-cachekey = ✓ 自定义键类型 (CacheKey trait)
example-new-api-builder-pattern-for-configuration = ✓ 用于配置的构建器模式 (Cache::builder())
example-new-api-ttl-operations-set-with = ✓ TTL操作 (set_with_ttl)
example-new-api-batch-operations-set-many = ✓ 批量操作 (set_many, get_many)
example-new-api-delete-and-clear-operations = ✓ 删除和清空操作 (delete, clear)
example-new-api-existence-check-exists = ✓ 存在检查 (exists)
example-new-api-cache-aside-mode-with = ✓ 带回退的缓存旁路模式 (get_or)
example-new-api-example-complete = ✅ 新API示例完成!

# -- example_serialization.rs --
example-serialization-options-example = 序列化选项示例
example-serialization-1-json-serialization-default = 1. JSON序列化 (默认):
example-serialization-human-readable-format = - 人类可读格式
example-serialization-good-for-debugging = - 适合调试
example-serialization-slightly-larger-size = - 略微更大的尺寸
example-serialization-stored-user = 存储的用户: { $name } - { $email }
example-serialization-retrieved = 检索到: { $name } - { $email }
example-serialization-config = 配置文件: { $profile }
example-serialization-2-serialization-features = 2. 序列化特性:
example-serialization-json-standard-readable-format = - JSON: 标准格式，可读
example-serialization-bincode-compact-binary-format = - Bincode: 二进制格式，高效 (启用时)
example-serialization-supports-custom-serializers = - 支持自定义序列化器
example-serialization-example-complete = ✓ 序列化示例完成!

# -- example_sync_api.rs --
example-sync-api-basic-sync-operations = === 基本 sync 操作 ===
example-sync-api-set-user-1 = 设置 user:1 = { $alice }
example-sync-api-get-user-1 = 获取 user:1 = { $cached }
example-sync-api-delete-user-1 = 删除 user:1
example-sync-api-set-temp-60s-ttl = 设置 temp（60s TTL）
example-sync-api-get-temp = 获取 temp = { $cached }
example-sync-api-get-or-sync-single = === get_or_sync（单飞） ===
example-sync-api-first-get-or-sync = 第一次 get_or_sync = { $user }（fallback 被调用）
example-sync-api-second-get-or-sync = 第二次 get_or_sync = { $user }（cache hit）
example-sync-api-mixing-sync-and-async = === sync 与 async 混用 ===
example-sync-api-cache-metadata-sync = === 缓存元信息 sync ===
example-sync-api-entries-len-sync = 条目数 (len_sync): { $len }
example-sync-api-cache-cleared = ✓ 缓存已清空
example-sync-api-entries-after-clear = 清空后条目数: { $len_after }
example-sync-api-example-complete = 同步 API 示例完成！

# -- example_batch_write.rs --
example-batch-write-optimization-example = === 批量写入优化示例 ===
example-batch-write-1-batch-add-products = 1. 批量添加商品
example-batch-write-batch-adding-products = 开始批量添加 { $products } 个商品...
example-batch-write-batch-add-complete-took = ✓ 批量添加完成，耗时: { $elapsed }
example-batch-write-2-batch-update-prices = 2. 批量更新价格 (模拟促销活动)
example-batch-write-updating-product-prices = 更新商品价格:
example-batch-write-product-new-price = 产品 { $id }: { $name } 新价格: ¥{ $price }
example-batch-write-3-batch-read-verification = 3. 批量读取验证
example-batch-write-reading-all-product-info = 读取所有商品信息:
example-batch-write-stock = [{ $id }] { $name } - ¥{ $price } (库存: { $stock })
example-batch-write-4-using-set-many = 4. 使用 set_many / get_many / delete_many
example-batch-write-set-many-wrote-entries = ✓ set_many 写入 { $new_products } 条
example-batch-write-get-many-read-entries = ✓ get_many 读取 { $fetched } 条
example-batch-write-delete-many-deleted-entries = ✓ delete_many 删除 { $del_keys } 条
example-batch-write-entries-remain-after-delete = 删除后剩余 { $remaining } 条
example-batch-write-5-batch-delete-simulating = 5. 批量删除 (模拟下架商品)
example-batch-write-delisted-product-ids = 下架商品 ID: { $out_of_stock_ids }
example-batch-write-products-delisted = ✓ 商品下架完成
example-batch-write-6-cache-statistics = 6. 缓存统计
example-batch-write-cache-type = - 缓存类型: { $stats }
example-batch-write-capacity = - 容量: { $stats }
example-batch-write-entries = - 条目数: { $stats }
example-batch-write-optimization-example-2 = === 批量写入优化示例完成 ===

# -- example_cache_promotion.rs --
example-cache-promotion-strategy-example = === 缓存提升策略示例 ===
example-cache-promotion-1-simulating-l1-l2 = --- 1. 模拟 L1 + L2 分层 ---
example-cache-promotion-l1-memory-capacity-3 = L1 (内存): 容量=3 (模拟快速缓存)
example-cache-promotion-l2-cache-capacity-100 = L2 (缓存): 容量=100 (模拟慢速/大容量缓存)
example-cache-promotion-2-initialize-data-into = --- 2. 初始化数据到 L2 ---
example-cache-promotion-wrote-sessions-into-l2 = ✓ 写入 { $sessions } 个会话到 L2
example-cache-promotion-3-reads-with-promotion = --- 3. 带提升的读取（promote_on_hit） ---
example-cache-promotion-l1-hit = [L1 命中] { $key }
example-cache-promotion-l2-hit-promoted-to = [L2 命中 → 提升到 L1] { $key }
example-cache-promotion-all-miss = [全部未命中] { $key }
example-cache-promotion-access-sequence-accesses = 访问序列 ({ $access_sequence } 次):
example-cache-promotion-l1-final-state-entries = L1 最终状态: { $l1 } 条
example-cache-promotion-4-access-frequency-analysis = --- 4. 访问频率分析 ---
example-cache-promotion-access-frequency-ranking = 访问频率排名:
example-cache-promotion-times = { $key } { $bar } ({ $count } 次)
example-cache-promotion-hot-data-sess-1 = ✓ 热点数据 (sess_1) 自动留在 L1 中
example-cache-promotion-5-production-configuration-suggestions = --- 5. 生产环境配置建议 ---
example-cache-promotion-a-real-tiered-cache = 真正的分层缓存配置:
example-cache-promotion-l1-moka-l2-redis = // L1 (Moka) + L2 (Redis) 分层
example-cache-promotion-ttl-duration-from-secs = .ttl(Duration::from_secs(3600))     // L1 默认 TTL
example-cache-promotion-capacity-10-000-l1 = .capacity(10_000)                    // L1 容量
example-cache-promotion-promotion-strategy-notes = 提升策略要点:
example-cache-promotion-l1-is-small-but = - L1 容量小但速度快，L2 容量大但较慢
example-cache-promotion-on-an-l2-hit = - L2 命中时自动回填 L1（promote_on_hit）
example-cache-promotion-hot-data-is-promoted = - 热点数据自然提升到 L1，冷数据仅在 L2
example-cache-promotion-size-l1-sensibly-to = - 合理设置 L1 容量避免频繁淘汰
example-cache-promotion-6-ttl-effects-on = --- 6. TTL 对提升的影响 ---
example-cache-promotion-set-a-session-with = ✓ 设置短 TTL (5s) 会话
example-cache-promotion-remaining-ttl = 剩余 TTL: { $remaining }
example-cache-promotion-example-complete = ✓ 缓存提升示例完成

# -- example_chain_cache.rs --
example-chain-cache-example = === 链式缓存示例 ===
example-chain-cache-1-chaincache-builder-creates = 1. ChainCache::builder() 创建多级缓存
example-chain-cache-connecting-to-redis = 连接 Redis: { $redis_url }
example-chain-cache-created-a-2-tier = ✓ 创建了2级缓存链：L1(Moka) -> L2(Redis)
example-chain-cache-cannot-connect-to-redis = ✗ 无法连接到 Redis: { $e }
example-chain-cache-skipping-the-redis-example = 跳过 Redis 示例，使用仅内存缓存
example-chain-cache-2-in-memory-only = 2. 仅内存缓存示例
example-chain-cache-store = 存储: { $user }
example-chain-cache-read = 读取: { $cached }
example-chain-cache-3-batch-operations-example = 3. 批量操作示例
example-chain-cache-batch-stored-users = 批量存储了 { $users } 个用户
example-chain-cache-batch-read-users = 批量读取: { $results } 个用户
example-chain-cache-example-complete = ✓ 示例完成
example-chain-cache-test-data = 测试数据: { $user }
example-chain-cache-writing-data = 写入数据...
example-chain-cache-data-written-to-the = ✓ 数据已写入缓存链
example-chain-cache-reading-data = 读取数据：
example-chain-cache-hit = ✓ 命中: { $u }
example-chain-cache-failed-to-parse-the = ✗ 数据解析失败
example-chain-cache-not-found = ✗ 未找到
example-chain-cache-cache-stats = 缓存统计:
example-chain-cache-test-data-2 = 测试数据: { $user }
example-chain-cache-writing-data-2 = 写入数据...
example-chain-cache-data-written-to-the-2 = ✓ 数据已写入缓存
example-chain-cache-reading-data-2 = 读取数据：
example-chain-cache-hit-2 = ✓ 命中: { $u }
example-chain-cache-not-found-2 = ✗ 未找到
example-chain-cache-cache-stats-2 = 缓存统计:

# -- example_custom_backend.rs --
example-custom-backend-hashmapbackend-shut-down-data = [HashMapBackend] 已关闭，数据已清理
example-custom-backend-example = === 自定义后端示例 ===
example-custom-backend-1-create-a-custom = --- 1. 创建自定义后端 ---
example-custom-backend-hashmapbackend-created-capacity-100 = ✓ HashMapBackend 创建成功 (capacity=100)
example-custom-backend-2-inject-it-via = --- 2. 通过 CacheBuilder 注入 ---
example-custom-backend-cache-built-with-the = ✓ Cache 使用自定义后端构建成功
example-custom-backend-3-basic-crud-operations = --- 3. 基本 CRUD 操作 ---
example-custom-backend-4-ttl-operations = --- 4. TTL 操作 ---
example-custom-backend-set-with-ttl-60s = ✓ set_with_ttl 60s, 剩余 TTL: { $remaining }
example-custom-backend-expire-updated-to-120s = ✓ expire 更新为 120s: { $expired }
example-custom-backend-5-batch-operations = --- 5. 批量操作 ---
example-custom-backend-batch-wrote-entries = ✓ 批量写入 { $items } 条
example-custom-backend-cache-entries = ✓ 缓存条目数: { $count }
example-custom-backend-6-statistics = --- 6. 统计信息 ---
example-custom-backend-7-health-check = --- 7. 健康检查 ---
example-custom-backend-backend-healthy = ✓ 后端健康
example-custom-backend-health-check-failed = ✗ 健康检查失败: { $e }
example-custom-backend-8-capacity-limit-demonstration = --- 8. 容量限制演示 ---
example-custom-backend-wrote-3-entries-capacity = ✓ 写入 3 条（容量=3）
example-custom-backend-unexpected-the-write-succeeded = 意外：写入成功
example-custom-backend-capacity-full-the-write = ✓ 容量已满，写入失败: { $e }
example-custom-backend-9-shutdown = --- 9. 关闭 ---
example-custom-backend-cache-closed = ✓ 缓存已关闭
example-custom-backend-example-complete = ✓ 自定义后端示例完成

# -- example_dashmap_backend.rs --
example-dashmap-backend-example = === DashMap 后端示例 ===
example-dashmap-backend-1-create-a-dashmap = --- 1. 创建 DashMap 后端 ---
example-dashmap-backend-created = ✓ 创建 DashMap 后端
example-dashmap-backend-2-basic-operations = --- 2. 基本操作 ---
example-dashmap-backend-write-key1-value1 = 写入: key1 = value1
example-dashmap-backend-read-key1 = 读取: key1 = { $value }
example-dashmap-backend-exists-key1 = 存在: key1 = { $exists }
example-dashmap-backend-3-batch-operations = --- 3. 批量操作 ---
example-dashmap-backend-batch-write-key-value = 批量写入: { $keys_values } 个键值对
example-dashmap-backend-batch-read-results = 批量读取: { $results } 个结果
example-dashmap-backend-4-comparing-moka-and = --- 4. 对比 Moka 和 DashMap ---
example-dashmap-backend-moka-backend-keys = Moka 后端: { $moka_count } 个键
example-dashmap-backend-keys = DashMap 后端: { $dashmap_count } 个键
example-dashmap-backend-5-delete-operations = --- 5. 删除操作 ---
example-dashmap-backend-delete-key1 = 删除: key1
example-dashmap-backend-exists-check-key1 = 存在检查: key1 = { $exists_after }
example-dashmap-backend-6-clear-operations = --- 6. 清空操作 ---
example-dashmap-backend-before-clear-keys = 清空前: { $count_before } 个键
example-dashmap-backend-after-clear-keys = 清空后: { $count_after } 个键
example-dashmap-backend-7-feature-comparison = --- 7. 特性对比 ---
example-dashmap-backend-moka-backend-traits = Moka 后端特点:
example-dashmap-backend-lru-tinylfu-eviction-policies = - 支持 LRU/TinyLFU 淘汰策略
example-dashmap-backend-capacity-limits = - 支持容量限制
example-dashmap-backend-automatic-expiry = - 支持自动过期
example-dashmap-backend-suitable-for-production = - 适合生产环境
example-dashmap-backend-traits = DashMap 后端特点:
example-dashmap-backend-plain-concurrent-hashmap = - 纯并发 HashMap
example-dashmap-backend-no-eviction-policy = - 无淘汰策略
example-dashmap-backend-manual-ttl-management = - 手动 TTL 管理
example-dashmap-backend-suited-to-simple-scenarios = - 适合简单场景或测试
example-dashmap-backend-example-complete = ✓ 示例完成

# -- example_error_handling.rs --
example-error-handling-example = === 错误处理示例 ===
example-error-handling-1-error-type-classification = --- 1. 错误类型分类 ---
example-error-handling-2-recoverability = --- 2. 可恢复性判断 ---
example-error-handling-3-error-codes-and = --- 3. 错误码与日志 ---
example-error-handling-4-resilient-operations-with = --- 4. 带重试的弹性操作 ---
example-error-handling-5-triggering-runtime-errors = --- 5. 运行时错误触发 ---
example-error-handling-6-error-propagation-with = --- 6. 错误传播与 ? 操作符 ---
example-error-handling-7-configuration-time-errors = --- 7. 配置阶段错误 ---
example-error-handling-example-complete = ✓ 错误处理示例完成
example-error-handling-connection-error-recoverable = 连接错误: { $conn_err } (可恢复: { $is_recoverable })
example-error-handling-timeout-error-recoverable = 超时错误: { $timeout_err } (可恢复: { $is_recoverable })
example-error-handling-backend-error-recoverable = 后端错误: { $backend_err } (可恢复: { $is_recoverable })
example-error-handling-not-found-recoverable = 未找到:   { $not_found } (可恢复: { $is_recoverable })
example-error-handling-serialization-recoverable = 序列化:   { $set_err } (可恢复: { $is_recoverable })
example-error-handling-degraded-degraded-mode = 降级:     { $degraded } (降级模式: { $is_degraded })
example-error-handling-internal-error-recoverable = 内部错误: { $internal } (可恢复: { $is_recoverable })
example-error-handling-error-code = { $type } { $recoverable } { $error } 错误码
example-error-handling-error-message = 错误消息: { $err }
example-error-handling-error-code-2 = 错误码:   { $code }
example-error-handling-retryable = 可重试:   { $is_recoverable }
example-error-handling-succeeded-on-retry = ✓ { $operation_name } 第 { $attempt } 次重试成功
example-error-handling-hit-an-unrecoverable-error = ✗ { $operation_name } 遇到不可恢复错误: { $e }
example-error-handling-failed-on-attempt-retrying = ⚠ { $operation_name } 第 { $attempt } 次失败 ({ $code }), { $as_millis }ms 后重试...
example-error-handling-reached-the-maximum-retry = ✗ { $operation_name } 已达最大重试次数 { $max_retries }
example-error-handling-final-result = 最终结果: { $result }
example-error-handling-unrecoverable-error-result = 不可恢复错误结果: { $result }
example-error-handling-col-error-type = 错误类型
example-error-handling-col-recoverable = 可恢复?
example-error-handling-col-connection-error = 连接错误?
example-error-handling-result-retry-skipped = 正确跳过重试
example-error-handling-result-unexpected = 意外
example-error-handling-cache-operation-succeeded = 缓存操作成功: { $name }
example-error-handling-normal-read = 正常读取: { $val }
example-error-handling-a-missing-key-returns = 不存在的键返回: { $missing } (非错误)
example-error-handling-get-or-computed-result = get_or 计算结果: { $value }
example-error-handling-health-check-passed = 健康检查: ✓ 通过
example-error-handling-health-check = 健康检查: ✗ { $e } ({ $code })
example-error-handling-user-1-score = 用户 1 分数: { $score }
example-error-handling-load-failed = 加载失败: { $e }
example-error-handling-user-999-score = 用户 999 分数: { $score }
example-error-handling-user-999-load-failed = 用户 999 加载失败: { $e } (code={ $code })
example-error-handling-config-error-code = 配置错误: { $err } [code={ $code }]

# -- example_invalidation.rs --
example-invalidation-cache-invalidation-strategy-example = === 缓存失效策略示例 ===
example-invalidation-1-prepare-test-data = 1. 准备测试数据
example-invalidation-added-orders = ✓ 添加了 { $orders } 个订单
example-invalidation-2-single-key-invalidation = 2. 单个 key 失效
example-invalidation-deleting-order-1 = 删除 order:1
example-invalidation-the-order-still-exists = ✗ 订单仍然存在
example-invalidation-the-order-was-deleted = ✓ 订单已删除
example-invalidation-3-bulk-invalidation-by = 3. 批量失效 (通过清空缓存)
example-invalidation-order-2-still-exists = ✗ order:2 仍然存在
example-invalidation-all-orders-cleared = ✓ 所有订单已清空
example-invalidation-4-user-id-based = 4. 基于用户 ID 的失效模式
example-invalidation-original-cart = 原始购物车:
example-invalidation-user-100-checks-out = 用户 100 结账，清空其购物车...
example-invalidation-after-clear = 清空后:
example-invalidation-5-ttl-based-automatic = 5. TTL 自动失效
example-invalidation-adding-data-that-expires = 添加 2 秒过期的数据
example-invalidation-immediate-get = 立即获取: { $await }
example-invalidation-waiting-3-seconds = 等待 3 秒...
example-invalidation-get-after-3-seconds = 3 秒后获取: { $await }
example-invalidation-6-write-time-invalidation = 6. 更新时失效 (Write-Invalidation)
example-invalidation-initial-theme = 初始主题: { $await }
example-invalidation-updated-theme = 更新后主题: { $await }
example-invalidation-strategy-example-complete = === 失效策略示例完成 ===

# -- example_lua_script.rs --
example-lua-script-execution-example = === Lua 脚本执行示例 ===
example-lua-script-connecting-to-redis = 连接 Redis: { $redis_url }
example-lua-script-redis-connected = ✓ Redis 连接成功
example-lua-script-1-atomic-counter-eval = --- 1. 原子计数器（eval_lua） ---
example-lua-script-call-counter-value = 第 { $i } 次调用，计数器值: { $result }
example-lua-script-2-preloading-scripts-script = --- 2. 预加载脚本（script_load） ---
example-lua-script-script-sha1 = 脚本 SHA1: { $sha }
example-lua-script-3-executing-scripts-by = --- 3. 通过 SHA 执行脚本（eval_sha） ---
example-lua-script-eval-lua-result = eval_lua 结果: { $result }
example-lua-script-eval-sha-result = eval_sha 结果: { $result }
example-lua-script-4-conditional-updates-compare = --- 4. 条件更新（Compare-And-Swap） ---
example-lua-script-initial-value-v1 = 初始值: v1
example-lua-script-expecting-v2-updating-to = 期望 v2 更新为 v3: { $result } (应为 0，表示失败)
example-lua-script-expecting-v1-updating-to = 期望 v1 更新为 v2: { $result } (应为 1，表示成功)
example-lua-script-example-complete-test-data = ✓ 示例完成，已清理测试数据

# -- example_moka_ttl.rs --
example-moka-ttl-creating-mokamemorybackend-no-global = 创建 MokaMemoryBackend（无全局 TTL）
example-moka-ttl-set-k1-60s-per = set 'k1'（60s per-entry TTL）
example-moka-ttl-ttl-k1-should-be = ttl('k1') = { $as_secs }（应约为 60s）
example-moka-ttl-expire-updates-ttl = === expire 更新 TTL ===
example-moka-ttl-expire-k1-120s-should = expire('k1', 120s) = { $result }（key 存在应返回 true）
example-moka-ttl-ttl-k1-should-be-2 = ttl('k1') = { $as_secs }（应约为 120s）
example-moka-ttl-expire-missing-60s-should = expire('missing', 60s) = { $result }（key 不存在应返回 false）
example-moka-ttl-keys-without-ttl = === 无 TTL 的 key ===
example-moka-ttl-set-k2-no-ttl = set 'k2'（无 TTL，永不过期）
example-moka-ttl-ttl-k2-should-be = ttl('k2') = { $ttl }（无 per-entry TTL 应返回 None）
example-moka-ttl-ttl-expiry-verification = === TTL 过期验证 ===
example-moka-ttl-set-temp-50ms-ttl = set 'temp'（50ms TTL）
example-moka-ttl-immediate-get-temp = 立即 get('temp') = { $value }
example-moka-ttl-get-temp-after-100ms = 100ms 后 get('temp') = { $value }（应已过期）
example-moka-ttl-global-ttl-vs-per = === 全局 TTL vs per-entry TTL ===
example-moka-ttl-creating-mokamemorybackend-global-ttl = 创建 MokaMemoryBackend（全局 TTL=300ms）
example-moka-ttl-set-a-no-per = set 'a'（无 per-entry TTL → 沿用全局 300ms）
example-moka-ttl-set-b-per-entry = set 'b'（per-entry TTL=600ms > 全局 300ms → per-entry 覆盖全局）
example-moka-ttl-ttl-a-none-ttl = ttl('a') = { $as_millis }（None：ttl() 只上报 per-entry TTL，'a' 仍按全局 300ms 过期）
example-moka-ttl-ttl-b-should-be = ttl('b') = { $as_millis }（应约为 600ms：覆盖全局，而非被封顶到 300ms）
example-moka-ttl-get-a-after-450ms = 450ms 后 get('a') = { $a }（全局 300ms 已到 → None）
example-moka-ttl-get-b-after-450ms = 450ms 后 get('b') = { $b }（per-entry 600ms 未到 → Some）
example-moka-ttl-moka-per-entry-ttl = Moka per-entry TTL 示例完成！

# -- example_redis_modes.rs --
example-redis-modes-redis-multi-mode-connection = === Redis 多模式连接示例 ===
example-redis-modes-redis-multi-mode-connection-2 = === Redis 多模式连接示例 ===
example-redis-modes-1-standalone-mode = --- 1. Standalone 模式 ---
example-redis-modes-connected = 连接: { $standalone_url }
example-redis-modes-connected-mode = ✓ 连接成功，模式: { $mode }
example-redis-modes-write-read = 写入/读取: { $value }
example-redis-modes-2-explicitly-selecting-standalone = --- 2. 使用 Builder 显式指定 Standalone 模式 ---
example-redis-modes-builder-created-mode = ✓ Builder 创建成功，模式: { $mode }
example-redis-modes-3-cluster-mode = --- 3. Cluster 模式 ---
example-redis-modes-connected-2 = 连接: { $cluster_url }
example-redis-modes-cluster-connected-mode = ✓ Cluster 连接成功，模式: { $mode }
example-redis-modes-write-read-2 = 写入/读取: { $value }
example-redis-modes-cluster-connection-failed = ✗ Cluster 连接失败: { $e }
example-redis-modes-redis-cluster-available-is = ⚠ REDIS_CLUSTER_AVAILABLE 未设置，跳过 Cluster 测试
example-redis-modes-start-cluster-cd-tests = 启动 Cluster: cd tests/real_env && docker compose -f docker-compose.cluster.yml up -d
example-redis-modes-4-sentinel-mode = --- 4. Sentinel 模式 ---
example-redis-modes-sentinel-connection = 连接 Sentinel: { $sentinel_url }
example-redis-modes-sentinel-connected-mode = ✓ Sentinel 连接成功，模式: { $mode }
example-redis-modes-write-read-3 = 写入/读取: { $value }
example-redis-modes-sentinel-connection-failed = ✗ Sentinel 连接失败: { $e }
example-redis-modes-redis-sentinel-available-is = ⚠ REDIS_SENTINEL_AVAILABLE 未设置，跳过 Sentinel 测试
example-redis-modes-start-sentinel-cd-tests = 启动 Sentinel: cd tests/real_env && docker compose -f docker-compose.sentinel.yml up -d
example-redis-modes-example-complete = ✓ 示例完成
example-redis-modes-5-the-redismodetype-enum = --- 5. RedisModeType 枚举 ---
example-redis-modes-mode-display = 模式: { $mode } (Display: { $mode2 })

# -- example_redis_native.rs --
example-redis-native-client-example = === Redis 原生客户端示例 ===
example-redis-native-1-connect-to-redis = 1. 连接 Redis
example-redis-native-redis-connected = ✓ Redis 连接成功
example-redis-native-2-basic-crud-operations = 2. 基本 CRUD 操作
example-redis-native-adding-user = 添加用户...
example-redis-native-user-added = ✓ 用户添加成功
example-redis-native-getting-user = 获取用户...
example-redis-native-user-retrieved = ✓ 用户获取成功: { $name } ({ $email })
example-redis-native-user-not-found = ✗ 用户未找到
example-redis-native-updating-user = 更新用户...
example-redis-native-user-updated = ✓ 用户更新成功
example-redis-native-deleting-user = 删除用户...
example-redis-native-failed-to-delete-the = ✗ 用户删除失败
example-redis-native-user-deleted = ✓ 用户删除成功
example-redis-native-3-batch-operations = 3. 批量操作
example-redis-native-adding-users-in-batch = 批量添加用户...
example-redis-native-batch-add-succeeded = ✓ 批量添加成功
example-redis-native-fetching-users-in-batch = 批量获取用户...
example-redis-native-user = ✓ 用户 { $id }: { $name }
example-redis-native-clearing-test-data = 清空测试数据...
example-redis-native-cleared = ✓ 清空完成
example-redis-native-4-cache-statistics = 4. 缓存统计
example-redis-native-cache-statistics-are-available = 缓存统计功能需要通过 metrics 接口获取
example-redis-native-see-the-metrics-test = (详细统计信息请参考 metrics_test 示例)
example-redis-native-client-example-2 = === Redis 原生客户端示例完成 ===

# -- example_redis_pipeline.rs --
example-redis-pipeline-batch-operations = === Redis Pipeline 批量操作示例 ===
example-redis-pipeline-connecting-to-redis = 连接 Redis: { $redis_url }
example-redis-pipeline-redis-connected = ✓ Redis 连接成功
example-redis-pipeline-1-batch-set-set = --- 1. 批量设置（set_many_pipeline） ---
example-redis-pipeline-batch-set-keys-took = 批量设置 { $items } 个键，耗时: { $elapsed }
example-redis-pipeline-2-batch-get-get = --- 2. 批量获取（get_many_pipeline） ---
example-redis-pipeline-batch-get-keys-took = 批量获取 { $keys } 个键，耗时: { $elapsed }
example-redis-pipeline-results = 结果:
example-redis-pipeline-3-performance-comparison-one = --- 3. 性能对比：逐个设置 vs Pipeline ---
example-redis-pipeline-setting-10-keys-one = 逐个设置 10 个键: { $individual_elapsed }
example-redis-pipeline-setting-10-keys-via = Pipeline 设置 10 个键: { $pipeline_elapsed }
example-redis-pipeline-pipeline-is-x-faster = Pipeline 快 { $max }x
example-redis-pipeline-4-batch-delete-delete = --- 4. 批量删除（delete_many_pipeline） ---
example-redis-pipeline-batch-deleted-keys = 批量删除 { $all_keys } 个键
example-redis-pipeline-remaining-keys-should-be = 剩余键数: { $remaining } (应为 0)
example-redis-pipeline-example-complete = ✓ 示例完成

# -- example_smart_strategy.rs --
example-smart-strategy-cache-strategy-patterns-example = === 缓存策略模式示例 ===
example-smart-strategy-1-the-cache-aside = --- 1. Cache-Aside（缓存旁路）策略 ---
example-smart-strategy-read-fill-on-miss = 读时填充：缓存未命中时从数据源加载并写入缓存
example-smart-strategy-write-invalidate-after-updating = 写时失效：更新数据源后删除缓存条目
example-smart-strategy-first-read-of-1 = 首次读取 #1: { $title } (数据源查询次数: { $query_count })
example-smart-strategy-second-read-of-1 = 再次读取 #1: { $title } (数据源查询次数: { $query_count })
example-smart-strategy-updating-1-and-invalidating = 更新 #1 并失效缓存
example-smart-strategy-read-of-1-after = 读取更新后 #1: { $title } (数据源查询次数: { $query_count })
example-smart-strategy-2-the-lazy-loading = --- 2. Lazy Loading（懒加载）策略 ---
example-smart-strategy-lazy-computation-with-get = 使用 get_or 延迟计算：仅在缓存未命中时执行昂贵的计算
example-smart-strategy-first-get-or-on = 第一次 get_or #42: { $title } (计算次数: { $seqcst })
example-smart-strategy-second-get-or-on = 第二次 get_or #42: { $title } (计算次数: { $seqcst })
example-smart-strategy-3-ttl-lifecycle-management = --- 3. TTL 生命周期管理 ---
example-smart-strategy-different-data-types-use = 不同数据类型使用不同的过期策略
example-smart-strategy-session-data-ttl-30s = ✓ 会话数据: TTL=30s
example-smart-strategy-popular-articles-ttl-1h = ✓ 热门文章: TTL=1h
example-smart-strategy-system-config-ttl-24h = ✓ 系统配置: TTL=24h
example-smart-strategy-terms-of-service-no = ✓ 服务条款: 无 TTL（永不过期）
example-smart-strategy-remaining-ttl = 剩余 TTL:
example-smart-strategy-sessions = 会话:   { $session_ttl }
example-smart-strategy-config = 配置:   { $config_ttl }
example-smart-strategy-terms = 条款:   { $zero }
example-smart-strategy-4-capacity-planning-and = --- 4. 容量规划与访问频率分层 ---
example-smart-strategy-prewarmed-popular-articles = ✓ 预热 { $hot_ids } 篇热门文章
example-smart-strategy-simulating-accesses = 模拟 { $total } 次访问:
example-smart-strategy-hits = 命中: { $hits } ({ $f64 }%)
example-smart-strategy-misses = 未中: { $misses } ({ $f64 }%)
example-smart-strategy-5-cache-statistics = --- 5. 缓存统计 ---
example-smart-strategy-entries = 条目数: { $stats }
example-smart-strategy-capacity = 容量:   { $stats }
example-smart-strategy-cache-strategy-example-complete = ✓ 缓存策略示例完成

# -- example_warmup.rs --
example-warmup-cache-warmup-example = === 缓存预热示例 ===
example-warmup-1-simulating-application-config = 1. 模拟应用配置预热
example-warmup-loading-config-from-the = 从数据库加载配置...
example-warmup-loading-config = 加载配置: { $k } = { $v } ({ $desc })
example-warmup-config-warmup-complete-items = ✓ 配置预热完成 ({ $configs } 个配置项)
example-warmup-2-simulating-hot-user = 2. 模拟热点用户数据预热
example-warmup-preloading-hot-users = 预加载热点用户...
example-warmup-hot-user-warmup-complete = ✓ 热点用户预热完成 ({ $hot_users } 个用户, 耗时: { $elapsed })
example-warmup-3-verifying-the-warmed = 3. 验证预热数据
example-warmup-config-verification = 配置验证:
example-warmup-expected = ✗ { $k } = { $val } (期望: { $v })
example-warmup-not-found = ✗ { $k } 未找到
example-warmup-user-verification = 用户验证:
example-warmup-not-found-2 = ✗ { $key } 未找到
example-warmup-4-simulating-a-cache = 4. 模拟缓存重建场景
example-warmup-clearing-the-cache = 清空缓存...
example-warmup-rewarming = 重新预热...
example-warmup-cache-rebuild-complete-took = ✓ 缓存重建完成，耗时: { $elapsed }
example-warmup-5-post-warmup-statistics = 5. 预热后统计
example-warmup-total-entries = - 总条目数: { $stats }
example-warmup-hits = - 命中次数: { $stats }
example-warmup-cache-warmup-example-complete = === 缓存预热示例完成 ===

# -- example_dynamic_config.rs --
example-dynamic-config-example = === 动态配置示例 ===
example-dynamic-config-1-initializing-config = 1. 初始化配置
example-dynamic-config-2-config-queries = 2. 配置查询
example-dynamic-config-type = ✓ { $key } = { $value } (类型: { $config_type })
example-dynamic-config-not-found = ✗ { $key } 未找到
example-dynamic-config-3-config-update-demonstration = 3. 配置更新演示
example-dynamic-config-updating-app-theme = 更新 app.theme...
example-dynamic-config-app-theme-updated-to = ✓ app.theme 更新为 light
example-dynamic-config-current-app-theme = ✓ 当前 app.theme = { $value }
example-dynamic-config-4-config-export = 4. 配置导出
example-dynamic-config-exporting-all-config = 导出所有配置:
example-dynamic-config-5-config-groups-by = 5. 配置类型分组
example-dynamic-config-application-config-items = 应用配置 ({ $app_configs } 个):
example-dynamic-config-database-config-items = 数据库配置 ({ $db_configs } 个):
example-dynamic-config-6-cleaning-up-test = 6. 清理测试数据
example-dynamic-config-test-data-cleaned-up = ✓ 测试数据已清理
example-dynamic-config-example-complete = === 动态配置示例完成 ===

# -- example_key_generator.rs --
example-key-generator-example = === 键生成器示例 ===
example-key-generator-1-basic-usage = --- 1. 基本用法 ---
example-key-generator-default-namespace-default = 默认命名空间: 'default'
example-key-generator-default-max-length-256 = 默认最大长度: 256
example-key-generator-2-using-namespaces = --- 2. 使用命名空间 ---
example-key-generator-user-key = 用户键: { $user_key }
example-key-generator-order-key = 订单键: { $order_key }
example-key-generator-3-using-prefixes = --- 3. 使用前缀 ---
example-key-generator-cache-key = 缓存键: { $key1 }
example-key-generator-session-key = 会话键: { $key2 }
example-key-generator-4-chaining-namespace-prefix = --- 4. 链式设置（命名空间 + 前缀） ---
example-key-generator-full-key = 完整键: { $key }
example-key-generator-5-template-generation = --- 5. 模板生成 ---
example-key-generator-generate-user-id-profile = 模板 'user:{"{"}id{"}"}:profile' + id=42 -> { $key }
example-key-generator-generate-full-search-type = 模板 'search:{"{"}type{"}"}:{"{"}query{"}"}' -> { $key }
example-key-generator-6-full-key-generation = --- 6. 完整键生成（generate_full） ---
example-key-generator-generate-full-user-id = generate_full('user:{"{"}id{"}"}', id=123) -> { $key }
example-key-generator-7-key-validation = --- 7. 键验证 ---
example-key-generator-is-valid = ✓ '{ $key }' 有效
example-key-generator-is-invalid = ✗ '{ $key }' 无效: { $e }
example-key-generator-is-valid-2 = ✓ '{ $key }' 有效
example-key-generator-is-invalid-2 = ✗ '{ $key }' 无效: { $e }
example-key-generator-8-in-practice-cache = --- 8. 实际应用：缓存操作 ---
example-key-generator-store = 存储: { $user_key } = { $name }
example-key-generator-store-2 = 存储: { $user_key2 } = { $name2 }
example-key-generator-example-complete = ✓ 示例完成

# -- example_database_integration.rs --
example-database-integration-usage-example = === 数据库集成使用示例 ===
example-database-integration-1-creating-a-cache = 1. 创建缓存实例
example-database-integration-in-memory-cache-created = ✓ 内存缓存创建成功
example-database-integration-2-connection-string-examples = 2. 连接字符串示例
example-database-integration-oxcache-can-create-a = oxcache 支持通过连接字符串创建 Redis 缓存:
example-database-integration-cache-redis-redis-127 = Cache::redis("redis://127.0.0.1:6379")  — 单机模式
example-database-integration-cache-redis-redis-host1 = Cache::redis("redis://host1:6379,host2:6379")  — 集群模式
example-database-integration-see-the-example-redis = 详细示例请参考 example_redis_modes
example-database-integration-3-simulating-a-database = 3. 模拟数据库加载器
example-database-integration-simulated-database-loader-created = ✓ 模拟数据库加载器创建成功
example-database-integration-4-implementing-the-cache = 4. 缓存旁路模式实现
example-database-integration-trying-to-read-from = 尝试从缓存获取 '{ $key }'
example-database-integration-cache-hit = ✓ 缓存命中: { $name }
example-database-integration-cache-miss-querying-the = × 缓存未命中，查询数据库
example-database-integration-database-query-succeeded = ✓ 数据库查询成功: { $name }
example-database-integration-result-cached = ✓ 结果已缓存
example-database-integration-user-not-found-in = × 数据库中未找到用户
example-database-integration-database-query-error = × 数据库查询错误: { $e }
example-database-integration-5-testing-the-cache = 5. 测试缓存旁路模式
example-database-integration-first-query-for-user = 首次查询 'user:1':
example-database-integration-result = 结果: { $user }
example-database-integration-second-query-for-user = 第二次查询 'user:1':
example-database-integration-result-2 = 结果: { $user }
example-database-integration-querying-the-nonexistent-user = 查询不存在的用户 'user:999':
example-database-integration-unexpected-found-the-user = 意外：找到了用户 { $user }
example-database-integration-correct-the-user-does = ✓ 正确返回：用户不存在
example-database-integration-6-batch-loading-example = 6. 批量加载示例
example-database-integration-processing-user = 处理用户 '{ $user_id }':
example-database-integration-cache-hit-2 = ✓ 缓存命中
example-database-integration-database-load-succeeded = ✓ 数据库加载成功
example-database-integration-cached = ✓ 已缓存
example-database-integration-absent-from-the-database = × 数据库中不存在
example-database-integration-load-error = × 加载错误: { $e }
example-database-integration-batch-load-results = 批量加载结果:
example-database-integration-not-found = { $i }: 未找到
example-database-integration-7-cache-updates-and = 7. 缓存更新和失效
example-database-integration-updating-the-user-record = 更新用户信息到缓存...
example-database-integration-user-record-updated = ✓ 用户信息已更新
example-database-integration-verifying-the-update = 验证更新结果: { $name }
example-database-integration-8-cache-invalidation-example = 8. 缓存失效示例
example-database-integration-invalidating-the-cache-for = 使 'user:2' 缓存失效...
example-database-integration-cache-invalidated = ✓ 缓存已失效
example-database-integration-verifying-invalidation = 验证缓存失效...
example-database-integration-reload-succeeded = 重新加载成功: { $name }
example-database-integration-9-cache-statistics = 9. 缓存统计信息
example-database-integration-cache-type = 缓存类型: { $stats }
example-database-integration-entries = 条目数: { $stats }
example-database-integration-capacity = 容量: { $stats }
example-database-integration-failed-to-get-statistics = 获取统计信息失败: { $e }
example-database-integration-10-simulating-a-partitioning = 10. 模拟分区策略
example-database-integration-partition-count = 分区数量: { $partitions }
example-database-integration-partition-assignment = 分区分配情况:
example-database-integration-partition = { $key } -> 分区 { $partition }
example-database-integration-example-complete = === 数据库集成示例完成 ===
example-database-integration-key-benefits-of-database = 数据库集成的主要优势:
example-database-integration-cache-aside-cache-misses = - 缓存旁路：自动处理缓存未命中
example-database-integration-less-database-load-higher = - 减少数据库负载：提高缓存命中率
example-database-integration-data-consistency-prompt-updates = - 数据一致性：及时更新和失效
example-database-integration-partitioning-supports-large-scale = - 分区策略：支持大规模数据分布
example-database-integration-connection-management-uniform-connection = - 连接管理：统一的连接字符串处理

# -- example_bloom_filter.rs --
example-bloom-filter-bloomfilter-standalone-type = === BloomFilter 独立类型 ===
example-bloom-filter-creating-bloomfilter-capacity-10000 = 创建 BloomFilter（capacity=10000, fpr=0.01）
example-bloom-filter-inserting-existing-key = 插入 'existing_key'
example-bloom-filter-contains-existing-key-should = contains 'existing_key' = { $existing_key }（应为 true）
example-bloom-filter-contains-missing-key-should = contains 'missing_key'  = { $missing_key }（应为 false）
example-bloom-filter-contains-existing-key-after = clear 后 contains 'existing_key' = { $existing_key }（应为 false）
example-bloom-filter-bloomfilterbackend-decorator = === BloomFilterBackend 装饰器 ===
example-bloom-filter-inner-backend-score = inner backend: { $backend_name }（score={ $score }）
example-bloom-filter-creating-bloomfilterbackend-wrapping-mok = 创建 BloomFilterBackend（装饰 Moka）
example-bloom-filter-get-user-999-filtered = get 'user:999' = { $value }（BF 过滤，不查询 inner）
example-bloom-filter-ttl-passthrough = === TTL 透传 ===
example-bloom-filter-set-temp-60s-ttl = set 'temp'（60s TTL）
example-bloom-filter-ttl-temp-should-be = ttl('temp') = { $as_secs }（应约为 60s）
example-bloom-filter-example-complete = Bloom Filter 示例完成！

# -- example_cli_usage.rs --
example-cli-usage-oxcache-cli-usage-example = === oxcache CLI 使用示例 ===
example-cli-usage-cache-created-demo-cache = 缓存已创建: demo_cache
example-cli-usage-performing-cache-operations = --- 执行缓存操作 ---
example-cli-usage-set-10-cache-entries = 已设置 10 个缓存条目
example-cli-usage-retrieved-5-cache-entries = 已获取 5 个缓存条目（命中）
example-cli-usage-attempting-to-retrieve-5 = 尝试获取 5 个不存在的条目（未命中）
example-cli-usage-simulated-oxcache-status-output = --- 模拟 `oxcache status` 输出 ---
example-cli-usage-cache-name-demo-cache = 缓存名称: demo_cache
example-cli-usage-cache-type-l1-in = 缓存类型: L1 内存缓存
example-cli-usage-max-capacity-1000 = 最大容量: 1000
example-cli-usage-current-entries = 当前条目数: { $await }
example-cli-usage-status-running = 状态: 运行中
example-cli-usage-simulated-oxcache-metrics-output = --- 模拟 `oxcache metrics` 输出 ---
example-cli-usage-cli-command-format = --- CLI 命令格式 ---
example-cli-usage-check-status-oxcache-status = 查看状态: oxcache status --verbose
example-cli-usage-check-metrics-oxcache-metrics = 查看指标: oxcache metrics --prometheus
example-cli-usage-clean-cache-oxcache-admin = 清理缓存: oxcache admin clean --cache demo_cache
example-cli-usage-warm-up-cache-oxcache = 预热缓存: oxcache admin warmup --file warmup_data.json
example-cli-usage-cleaning-up = --- 清理资源 ---
example-cli-usage-cache-cleared = 缓存已清理
example-cli-usage-example-complete = 示例完成！

# -- example_compression.rs --
example-compression-original-size-bytes = 原始大小: { $data } bytes
example-compression-serialized-bytes = 序列化后: { $serialized } bytes
example-compression-ratio-x = 压缩率: { $ratio }x
example-compression-serialization-took = 序列化耗时: { $serialize_time }
example-compression-deserialization-took = 反序列化耗时: { $deserialize_time }
example-compression-data-compression-example = === 数据压缩示例 ===
example-compression-1-small-data-100 = --- 1. 小数据 (< 100 bytes) ---
example-compression-2-medium-data-100b = --- 2. 中等数据 (100B - 1KB) ---
example-compression-3-large-data-1kb = --- 3. 大数据 (1KB - 100KB) ---
example-compression-4-repeated-data-high = --- 4. 重复数据 (高压缩率场景) ---
example-compression-5-using-compression-in = --- 5. 在缓存中使用压缩 ---
example-compression-storing-compressed-data-bytes = 存储压缩数据: { $large_bytes } bytes -> { $compressed } bytes
example-compression-reading-and-decompressing-bytes = 读取并解压: { $stored } bytes -> { $decompressed } bytes
example-compression-data-integrity-verified = ✓ 数据完整性验证通过
example-compression-6-smart-compression-strategy = --- 6. 智能压缩策略 ---
example-compression-oxcache-picks-the-compression = oxcache 根据数据大小自动选择压缩级别:
example-compression-100-bytes-no-compression = - < 100 bytes: 不压缩（压缩开销不划算）
example-compression-100b-1kb-fast-compression = - 100B - 1KB: 快速压缩 (Compression::fast())
example-compression-1kb-100kb-medium-compression = - 1KB - 100KB: 中等压缩 (Compression::new(6))
example-compression-100kb-high-compression-ratio = - > 100KB: 高压缩率 (Compression::best())
example-compression-data-compression-example-complete = === 数据压缩示例完成 ===
example-compression-key-points = 关键点:
example-compression-enable-compression-with-jsonserializer = - 使用 JsonSerializer::with_compression() 启用压缩
example-compression-small-data-is-left = - 小数据不压缩，避免压缩开销
example-compression-repeated-data-compresses-well = - 重复数据压缩率高
example-compression-decompression-is-transparent = - 压缩/解压对用户透明，自动处理

# -- example_events.rs --
example-events-oxcache-event-system-example = === oxcache 事件系统示例 ===
example-events-1-creating-and-publishing = --- 1. 创建和发布事件 ---
example-events-2-convenience-methods = --- 2. 使用便捷方法 ---
example-events-3-monitoring-cache-operation = --- 3. 缓存操作事件监控 ---
example-events-set-operation-took = 设置操作耗时: { $elapsed }
example-events-4-batch-operation-events = --- 4. 批量操作事件 ---
example-events-5-custom-events = --- 5. 自定义事件 ---
example-events-6-all-event-types = --- 6. 所有事件类型 ---
example-events-event-type = 事件类型: { $event_type }
example-events-example-complete = 示例完成！

# -- example_i18n.rs --
example-i18n-internationalization-i18n-example = === 国际化 (i18n) 示例 ===
example-i18n-1-creating-formatters = --- 1. 创建格式化器 ---
example-i18n-en-us-formatter-created = ✓ en-US 格式化器创建成功
example-i18n-zh-cn-formatter-created = ✓ zh-CN 格式化器创建成功
example-i18n-de-de-formatter-created = ✓ de-DE 格式化器创建成功
example-i18n-unexpected-an-invalid-locale = 意外：无效 locale 创建成功
example-i18n-invalid-locale-correctly-errored = ✓ 无效 locale 正确报错: { $e }
example-i18n-2-number-formatting = --- 2. 数字格式化 ---
example-i18n-raw-value = 原始值:   { $value }
example-i18n-nan-correctly-errored = ✓ NaN 正确报错: { $e }
example-i18n-3-plural-rules = --- 3. 复数规则 ---
example-i18n-4-date-formatting = --- 4. 日期格式化 ---
example-i18n-expiry-date = 过期日期: { $year }-{ $month }-{ $day }
example-i18n-5-cache-key-generation = --- 5. Cache key 生成 ---
example-i18n-6-key-comparison-collation = --- 6. 键比较（排序） ---
example-i18n-original-order = 原始顺序: { $keys }
example-i18n-en-us-collation = en-US 排序: { $sorted_keys }
example-i18n-7-multilingual-cache-statistics = --- 7. 多语言缓存统计 ---
example-i18n-cache-hits = 缓存命中: { $cache_hit_count } 次
example-i18n-cache-misses = 缓存未命中: { $cache_miss_count } 次
example-i18n-example-complete = ✓ i18n 示例完成

# -- example_kit_integration.rs --
example-kit-integration-1-building-asynckit-oxcachemodule = === 1. 构建 AsyncKit + OxcacheModule ===
example-kit-integration-registered-oxcachebuildobserver = 已注册 OxcacheBuildObserver
example-kit-integration-registered-oxcachemodule = 已注册 OxcacheModule
example-kit-integration-registered-the-countingdecorator-decorat = 已注册 CountingDecorator 装饰器
example-kit-integration-asynckit-build-complete = AsyncKit 构建完成
example-kit-integration-2-using-the-cache = === 2. 使用缓存后端 ===
example-kit-integration-get-user-999-should = get 'user:999' = { $miss } (应为 None)
example-kit-integration-operation-count-should-be = 操作计数: { $seqcst } (应为 3: set + get + get)
example-kit-integration-3-three-phase-shutdown = === 3. 三阶段关闭 ===
example-kit-integration-registered-three-phase-shutdown = 已注册三阶段关闭钩子:
example-kit-integration-drainqueue-health-check-liveness = - DrainQueue: health_check 探活
example-kit-integration-shutdown-result = 关闭结果: { $result }
example-kit-integration-done = === 完成 ===
example-kit-integration-trait-kit-integration-demo = trait-kit 集成演示结束。observer/decorator/shutdown 三个特性均已展示。

# -- example_metrics.rs --
example-metrics-and-observability-example = === 指标与可观测性示例 ===
example-metrics-1-creating-a-cache = --- 1. 创建缓存并执行操作 ---
example-metrics-performed-20-writes-and = 执行了 20 次写入，20 次读取（15 命中 + 5 未命中）
example-metrics-2-backend-statistics = --- 2. 后端统计信息 ---
example-metrics-backend-statistics = 后端统计:
example-metrics-3-enhanced-statistics-cachestats = --- 3. 增强统计（CacheStats） ---
example-metrics-l1-hits = L1 命中: { $l1_hits }
example-metrics-l1-misses = L1 未命中: { $l1_misses }
example-metrics-l2-hits = L2 命中: { $l2_hits }
example-metrics-l2-misses = L2 未命中: { $l2_misses }
example-metrics-l1-writes = L1 写入: { $l1_sets }
example-metrics-l2-writes = L2 写入: { $l2_sets }
example-metrics-total-operations = 总操作数: { $total_operations }
example-metrics-l1-entries = L1 条目数: { $l1_item_count }
example-metrics-4-hit-rate = --- 4. 命中率 ---
example-metrics-l1-hit-rate = L1 命中率: { $l1_hit_rate_percent }
example-metrics-l2-hit-rate = L2 命中率: { $l2_hit_rate_percent }
example-metrics-overall-hit-rate = 总体命中率: { $overall_hit_rate_percent }
example-metrics-5-prometheus-format-export = --- 5. Prometheus 格式导出 ---
example-metrics-6-json-format-export = --- 6. JSON 格式导出 ---
example-metrics-7-cachestats-instance-method = --- 7. CacheStats 实例方法导出 ---
example-metrics-cachestats-export-prometheus-first = CacheStats.export_prometheus() (前 200 字符):
example-metrics-cachestats-export-json-first = CacheStats.export_json() (前 200 字符):
example-metrics-example-complete = ✓ 示例完成

# -- example_security.rs --
example-security-oxcache-security-features-example = === oxcache 安全功能示例 ===
example-security-1-sensitive-data-redaction = --- 1. 敏感数据脱敏 ---
example-security-raw-password = 原始密码: { $password }
example-security-redacted = 脱敏后:   { $redacted }
example-security-connection-string-redaction = --- 连接字符串脱敏 ---
example-security-raw-url = 原始URL: { $redis_url }
example-security-redacted-2 = 脱敏后:  { $safe_url }
example-security-sensitive-cache-key-redaction = --- 敏感缓存键脱敏 ---
example-security-field-value-redaction = --- 字段值脱敏 ---
example-security-2-the-redacted-wrapper = --- 2. Redacted 包装器 ---
example-security-redacted-wrapper = Redacted 包装: { $secret }
example-security-custom-visible-characters = 自定义可见字符: { $custom_redacted }
example-security-3-safe-log-handling = --- 3. 安全日志处理 ---
example-security-raw = 原始: { $msg }
example-security-safe = 安全: { $sanitized }
example-security-4-cache-operation-safety = --- 4. 缓存操作安全示例 ---
example-security-storing-the-user-session = 存储用户会话: { $user_token }
example-security-data-read-back-bytes = 获取到数据: { $data } 字节
example-security-5-security-best-practices = --- 5. 安全最佳实践 ---
example-security-never-log-sensitive-information = ✓ 永远不要在日志中记录敏感信息
example-security-use-the-redaction-module = ✓ 使用 redaction 模块脱敏敏感数据
example-security-encrypt-sensitive-data-before = ✓ 缓存中的敏感数据应先加密
example-security-use-secure-connection-string = ✓ 使用安全的连接字符串格式
example-security-rotate-sensitive-tokens-in = ✓ 定期轮换缓存中的敏感令牌
example-security-keep-cache-key-names = ✓ 限制缓存键的命名避免泄露信息
example-security-6-configuration-safety = --- 6. 配置安全示例 ---
example-security-unsafe-storing-passwords-in = ❌ 不安全: 在配置中明文存储密码
example-security-safe-use-environment-variables = ✓ 安全: 使用环境变量或密钥管理服务
example-security-safe-hide-connection-string = ✓ 安全: 日志中隐藏连接字符串密码
example-security-log-connecting-to = 日志: 连接到 { $safe_conn_str }
example-security-example-complete = 示例完成！

# -- example_security_validation.rs --
example-security-validation-oxcache-security-validation-example = === oxcache 安全验证示例 ===
example-security-validation-1-redis-key-validation = --- 1. Redis 键验证（validate_redis_key） ---
example-security-validation-valid = ✓ 合法: '{ $key }'
example-security-validation-invalid = ✗ 非法: '{ $key }' -> { $e }
example-security-validation-command-injection-character-detection = --- 命令注入字符检测 ---
example-security-validation-passed = ✓ 通过: '{ $key }'
example-security-validation-rejected = ✗ 拦截: '{ $key }' -> { $e }
example-security-validation-sql-injection-pattern-detection = --- SQL 注入模式检测 ---
example-security-validation-passed-2 = ✓ 通过: '{ $key }'
example-security-validation-rejected-2 = ✗ 拦截: '{ $key }' -> { $e }
example-security-validation-path-traversal-detection = --- 路径遍历检测 ---
example-security-validation-passed-3 = ✓ 通过: '{ $key }'
example-security-validation-rejected-3 = ✗ 拦截: '{ $key }' -> { $e }
example-security-validation-control-character-detection = --- 控制字符检测 ---
example-security-validation-passed-4 = ✓ 通过: { $key }
example-security-validation-rejected-4 = ✗ 拦截: { $key } -> { $e }
example-security-validation-2-lua-script-validation = --- 2. Lua 脚本验证（validate_lua_script） ---
example-security-validation-valid-script-passed-validation = ✓ 合法脚本通过验证
example-security-validation-error = ✗ 错误: { $e }
example-security-validation-dangerous-command-detection = --- 危险命令检测 ---
example-security-validation-passed-5 = ✓ 通过: { $desc }
example-security-validation-rejected-5 = ✗ 拦截: { $desc }
example-security-validation-infinite-loop-detection = --- 无限循环检测 ---
example-security-validation-passed-6 = ✓ 通过: { $script }
example-security-validation-rejected-6 = ✗ 拦截: { $script }
example-security-validation-length-and-key-count = --- 长度和键数量限制 ---
example-security-validation-oversized-script-passed = ✓ 超长脚本通过
example-security-validation-rejected-an-oversized-script = ✗ 拦截超长脚本（>10KB）
example-security-validation-excess-key-count-passed = ✓ 键数量超限通过
example-security-validation-rejected-excess-key-count = ✗ 拦截键数量超限（>100）
example-security-validation-3-scan-pattern-validation = --- 3. SCAN 模式验证（validate_scan_pattern） ---
example-security-validation-valid-2 = ✓ 合法: '{ $pattern }'
example-security-validation-invalid-2 = ✗ 非法: '{ $pattern }' -> { $e }
example-security-validation-excess-wildcard-detection = --- 过多通配符检测 ---
example-security-validation-passed-7 = ✓ 通过
example-security-validation-rejected-excess-wildcards-10 = ✗ 拦截过多通配符（>10 个）
example-security-validation-oversized-pattern-detection = --- 过长模式检测 ---
example-security-validation-passed-8 = ✓ 通过
example-security-validation-rejected-an-oversized-pattern = ✗ 拦截过长模式（>256 字符）
example-security-validation-4-scan-count-limits = --- 4. SCAN count 限制（clamp_scan_count） ---
example-security-validation-raw-clamped = 原始: { $count } -> 限制后: { $clamped }
example-security-validation-5-safe-logging-log = --- 5. 安全日志记录（log_cache_key） ---
example-security-validation-key = [{ $level }] { $message } (键: { $key })
example-security-validation-6-in-practice-a = --- 6. 实际应用场景：安全验证流程 ---
example-security-validation-validating-a-user-supplied = 验证用户输入的缓存键:
example-security-validation-accepted = ✓ 接受: '{ $input }'
example-security-validation-rejected-7 = ✗ 拒绝: '{ $input }'
example-security-validation-validating-a-user-submitted = 验证用户提交的 Lua 脚本:
example-security-validation-accepted-script = ✓ 接受脚本: { $name }
example-security-validation-rejected-script = ✗ 拒绝脚本: { $name }
example-security-validation-example-complete = ✓ 示例完成
example-security-validation-best-practices = 安全验证最佳实践:
example-security-validation-1-every-user-supplied = 1. 所有用户输入的缓存键必须经过 validate_redis_key 验证
example-security-validation-2-every-user-submitted = 2. 所有用户提交的 Lua 脚本必须经过 validate_lua_script 验证
example-security-validation-3-every-scan-pattern = 3. 所有 SCAN 操作的模式必须经过 validate_scan_pattern 验证
example-security-validation-4-every-scan-count = 4. 所有 SCAN 操作的 count 必须经过 clamp_scan_count 限制
example-security-validation-5-use-log-cache = 5. 日志中记录缓存键时使用 log_cache_key 自动脱敏
