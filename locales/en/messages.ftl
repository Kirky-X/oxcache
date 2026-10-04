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

# -- Tracing log messages (rendered as the event message field) --
log-chain-read-completed = chain read completed
log-chain-expire-backend-failed = backend expire failed
log-chain-iter-entries-key-failed = iter_entries batch layer failed for key
log-offload-lifecycle-event = offload lifecycle event
log-offload-timeout-policy-exceeded = offload task exceeded timeout policy
log-degradation-entered = L2 degradation entered, serving L1-only
log-degradation-half-open-probing = L2 degradation half-open, probing recovery
log-degradation-recovered = L2 degradation recovered
log-confers-config-reload-rejected = confers config reload rejected; keeping previous snapshot
log-backend-bridge-shutdown-skipped = async→sync bridge shutdown skipped: cannot nest a blocking driver inside a current_thread runtime
log-disk-sweep-failed = disk cache sweep failed
log-audit-event = cache audit event
log-stale-hit-via-get-or = stale hit served via get_or (Return downgrade; use get_or_refresh for background revalidation)
log-stale-revalidation-scheduled = stale hit; background revalidation scheduled
log-stale-hit-served = stale hit served

# -- Panic / invariant messages --
panic-chain-parallel-freshest-invariant = hits non-empty implies freshness non-empty
panic-bridge-temp-runtime-create-failed = failed to create temporary runtime for bridge shutdown
panic-warmup-semaphore-not-closed = warmup semaphore must not be closed
panic-audit-writer-lock-not-poisoned = writer receiver lock is never poisoned (critical section has no panic points)
panic-audit-writer-rx-present-on-first-start = writer receiver must be present on first startup
panic-stale-state-must-carry-payload = Stale state must carry payload
panic-bloom-capacity-must-be-positive = capacity must be greater than 0
panic-bloom-hash-count-must-be-positive = hash_count must be greater than 0
panic-bloom-fpr-must-be-in-open-interval = false_positive_rate must be in (0.0, 1.0)
panic-bloom-hash-count-unreachable = hash_count { $hash_count } unreachable for capacity { $capacity }
panic-bloom-hash-count-backsolve-drift = hash_count { $hash_count } unreachable for capacity { $capacity }: k steps by more than 1 per bitmap byte at this capacity
panic-bloom-seed-generation-failed = failed to create bloom filter: random seed generation failed
panic-config-validate-moka-requires-memory = validate rejects Moka without the memory feature
panic-config-validate-dashmap-requires-memory = validate rejects DashMap without the memory feature
panic-config-validate-mock-requires-test-with-memory = validate rejects Mock outside test builds with memory
panic-config-validate-redis-requires-redis-feature = validate rejects Redis without the redis feature
panic-config-validate-dragonfly-requires-dragonfly-feature = validate rejects Dragonfly without the dragonfly feature
panic-config-validate-disk-requires-disk-feature = validate rejects Disk without the disk feature

# -- User-visible detail messages (interpolated into error templates) --
detail-disk-redb-open-failed = redb open failed: { $err }
detail-disk-redb-create-failed = redb create failed: { $err }
detail-disk-corrupt-envelope = corrupt disk cache envelope
detail-not-supported-sync-atomic-increment = the wrapped sync backend does not support atomic increment
detail-not-supported-sync-atomic-cas = the wrapped sync backend does not support atomic compare-and-swap
detail-not-supported-sync-atomic-set-if-absent = the wrapped sync backend does not support atomic set-if-absent
detail-not-supported-async-atomic-increment = the wrapped async backend does not support atomic increment
detail-not-supported-async-atomic-cas = the wrapped async backend does not support atomic compare-and-swap
detail-not-supported-async-atomic-set-if-absent = the wrapped async backend does not support atomic set-if-absent
detail-not-supported-sync-requires-runtime = sync API requires a Tokio runtime: { $err }
detail-not-supported-sync-requires-multi-thread-runtime = sync API requires a multi-thread runtime; block_in_place is unavailable on current_thread runtime
detail-config-env-invalid-value = invalid value for { $key }: { $raw } ({ $err })
detail-config-env-backend-requires-features = { $key }={ $raw } requires one of the `memory`/`redis`/`disk` features, none of which is enabled in this build
detail-config-env-serialization-requires-feature = { $key }={ $raw } requires the `serialization` feature, which is not enabled in this build
detail-config-backend-requires-features = backend { $raw } requires one of the `memory`/`redis`/`disk` features, none of which is enabled in this build
detail-config-capacity-zero = capacity must be greater than 0 (drop the key to use the builder default)
detail-config-capacity-exceeds-usize = capacity { $capacity } exceeds this platform's usize range ({ $max })
detail-config-ttl-zero = { $name } must not be zero; use None (unset) for no expiry
detail-config-metrics-requires-feature = metrics_enabled requires the `metrics` feature, which is not enabled in this build
detail-config-serialization-requires-feature = serialization_format requires the `serialization` feature, which is not enabled in this build
detail-config-serialization-bincode-requires-feature = serialization format 'bincode' requires the `serde-bincode` feature, which is not enabled in this build (field: { $field })
detail-config-serialization-postcard-requires-feature = serialization format 'postcard' requires the `postcard` feature, which is not enabled in this build (field: { $field })
detail-config-serialization-invalid-format = invalid serialization format (field { $field }): { $raw } (expected one of json/bincode/postcard)
detail-config-circuit-breaker-threshold-zero = circuit_breaker_failure_threshold must be greater than 0
detail-config-service-name-empty = service_name must not be empty; drop the key to keep the service dimension disabled
detail-config-connection-pool-size-zero = connection_pool_size must be greater than 0 (drop the key to use the backend default)
detail-adaptive-ttl-min-ttl-exceeds-max = adaptive_ttl min_ttl ({ $min }) must not exceed max_ttl ({ $max })
detail-adaptive-ttl-multiplier-not-finite-positive = adaptive_ttl hot_ttl_multiplier ({ $value }) must be a finite positive number
detail-adaptive-ttl-divisor-at-least-one = adaptive_ttl cold_ttl_divisor must be at least 1
detail-adaptive-ttl-max-tracked-keys-at-least-one = adaptive_ttl max_tracked_keys must be at least 1
detail-get-or-leader-result-not-cached = get_or: concurrent fetch leader failed to cache result
detail-get-or-option-leader-result-not-cached = get_or_option: concurrent fetch leader failed to cache result
detail-warmup-ttl-lookup-failed = ttl lookup failed: { $err }
detail-redis-ttl-min-millis = TTL must be at least 1 millisecond for Redis SET PX/PEXPIRE
detail-redis-ttl-exceeds-max = TTL { $millis }ms exceeds Redis maximum of { $max }ms (~68 years)
detail-redis-cluster-connect-failed = Failed to connect to Redis Cluster: { $err }
detail-redis-cluster-connect-timeout = Connection timeout - Redis Cluster unavailable
detail-confers-expects-u64 = confers key '{ $key }' expects u64, got { $value }
detail-confers-value-exceeds-range = confers key '{ $key }' value { $value } exceeds the { $target } range
detail-confers-expects-bool = confers key '{ $key }' expects bool, got { $value }
detail-confers-expects-f64 = confers key '{ $key }' expects f64, got { $value }
detail-confers-expects-string = confers key '{ $key }' expects string, got { $value }
detail-confers-read-failed = confers read '{ $key }' failed: { $err }
detail-builder-no-backend-requires-memory = CacheBuilder with no backend requires the `memory` feature (default Moka); pass .backend_arc() explicitly otherwise.
detail-builder-stale-ttl-sync-conflict = stale_ttl cannot be combined with sync_mode(true); the sync API bypasses the decorator and would see incomplete stale semantics
detail-builder-adaptive-ttl-sync-conflict = adaptive_ttl cannot be combined with sync_mode(true); the sync API bypasses the decorator and would see unadjusted TTLs
detail-builder-adaptive-ttl-stale-conflict = adaptive_ttl cannot be combined with stale_ttl; stacking two TTL rewriters has undefined semantics
detail-config-backend-requires-feature = backend requires the `{ $feature }` feature, which is not enabled in this build
detail-config-backend-kind-requires-feature = backend `{ $kind }` requires the `{ $kind }` feature, which is not enabled in this build
detail-config-backend-mock-test-only = backend `mock` only exists in test builds and is not available here
detail-config-backend-aerospike-programmatic = backend `aerospike` needs namespace/set configuration and must be built programmatically, not via CacheConfig
detail-config-backend-valkey-no-impl = backend `valkey` has no implementation; use `redis` (protocol-compatible) or `dragonfly`
detail-config-backend-chain-needs-builder = backend `chain` must be assembled via ChainBuilder, not a single CacheConfig backend
detail-config-backend-unknown-kind = backend `{ $kind }` cannot be built from configuration
detail-config-backend-not-config-buildable = backend `{ $kind }` cannot be built via CacheConfig (rejected by validate or requires programmatic assembly)
detail-config-env-not-unicode = environment variable { $key } is not valid unicode: { $raw }
detail-config-env-invalid-bool = invalid bool value for { $key }: { $raw } (expected true/1/yes/on or false/0/no/off)
detail-config-backend-invalid-value = invalid value for { $key }: { $raw } (expected one of moka/dashmap/redis/valkey/dragonfly/aerospike/chain/mock/disk)
detail-disk-open-create-failed = disk backend open failed ({ $open_err }) and create failed ({ $create_err }): { $path }
detail-cache-memory-requires-feature = Cache::memory() requires the `memory` feature; construct via Cache::new_with_backend instead.
detail-not-supported-moka-sync-current-thread = Moka sync surface cannot be driven from within a current-thread runtime async context (tokio forbids nested blocking drivers). Call the sync API outside a runtime, or use a multi_thread runtime (block_in_place handles it).
detail-redis-unexpected-info-reply = Unexpected INFO { $section } reply type
detail-chain-get-many-length-mismatch = get_many returned { $returned } results for { $expected } keys
detail-chain-parallel-freshest-all-failed = All backends failed during parallel freshest read

# -- Example binary output messages --
example-inklog-bridge-title = === inklog audit log bridge example ===
example-inklog-bridge-observability = === bridge observability ===
example-inklog-bridge-dropped = dropped (no runtime context, dropped): { $count }
example-inklog-bridge-write-failures = write_failures (sink write failures): { $count }
example-redis-modes-feature-disabled = redis feature is not enabled in this build; nothing to demonstrate.
example-redis-modes-run-with-redis-feature = full run: cargo run --features redis --example example_redis_modes
example-redis-modes-run-with-examples-package = or: cargo run -p oxcache-examples --example example_redis_modes
example-redis-modes-standalone-connect-failed = ✗ Standalone connection failed: { $err }
example-redis-modes-standalone-env-required = a running Redis is required (override with REDIS_URL, default redis://127.0.0.1:6379); skipping the Standalone/Builder demos
example-redis-modes-done-standalone-skipped = ✓ example finished (Standalone skipped: environment unavailable)
