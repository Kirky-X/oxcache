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
