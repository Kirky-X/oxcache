# OxCache English message catalog (Fluent/FTL).
#
# Keys are the dashed form of the dotted message IDs exposed by the API
# (`.` and `_` become `-`, e.g. `error.not_found` -> `error-not-found`).
# This file is embedded at compile time via `include_str!` — keep it in sync
# with locales/zh/messages.ftl (a key-parity guard test enforces this).

# -- OxCacheError messages --
error-serialization = Serialization error: { $detail }. Please check the data format and ensure the serializer is compatible.
error-operation = Operation failed: { $detail }. Please retry or check your request.
error-connection = Connection error: { $detail }. Please check network connectivity and server availability.
error-not-found = Key not found: { $detail }. The requested key does not exist in the cache.
error-degraded = Cache degraded: { $detail }. The cache is operating in degraded mode with limited functionality.
error-l1 = L1 cache operation failed: { $detail }. This may indicate memory pressure or configuration issues.
error-l2 = L2 cache operation failed: { $detail }. Please check Redis connection and server status.
error-not-supported = Operation not supported: { $detail }. This feature may not be available for the current cache type.
error-wal = WAL (Write-Ahead Log) operation failed: { $detail }. Check disk space and file permissions.
error-database = Database error: { $detail }. Please check database connectivity and query syntax.
error-redis = Redis connection failed: { $detail }.
error-io = I/O error: { $detail }. Check file permissions and disk space.
error-backend = Backend error: { $detail }. This may be a transient issue, please retry.
error-timeout = Operation timed out: { $detail }. Consider increasing the timeout value or check system performance.
error-shutdown = Shutdown error: { $detail }. Some resources may not have been properly released.
error-key-too-long = Key too long: { $actual }. Maximum key length is { $max } bytes.
error-value-too-large = Value too large: { $actual }. Maximum value size is { $max } bytes.
error-buffer-full = Buffer full: { $detail }. The batch write buffer has reached capacity. Please retry later or increase buffer size.
error-invalid-input = Invalid input: { $detail }. The provided input does not meet the required format or constraints.
error-invalid-key = Invalid key: { $detail }. The provided key does not meet the required format or contains forbidden characters.
error-lock = Lock error: { $detail }. The lock may have been poisoned by a previous panic.
error-service-not-found = Service not found: { $detail }. The requested service configuration does not exist in the UnifiedConfig.
error-internal = Internal error: { $detail }.

# -- OxCacheConfigError messages --
config-missing-field = Missing required field: { $field }.
config-invalid-value = Invalid value for field '{ $field }': { $reason }.
config-unsupported-backend = Unsupported backend combination: { $detail }.
config-connection-failed = Connection failed during initialization: { $detail }.

# -- I18nError messages --
i18n-invalid-locale = invalid locale '{ $input }': { $reason }.
i18n-invalid-number = invalid number '{ $input }': { $reason }.
i18n-date-error = date error: { $detail }.
i18n-format-error = formatting error: { $detail }.

# -- #[cached] macro-generated strict-mode panic --
macro-service-not-registered = oxcache: service '{ $service }' not registered (strict mode)
