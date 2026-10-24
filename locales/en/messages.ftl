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
log-chain-backfill-succeeded = backfill succeeded
log-chain-backfill-failed = backfill failed
log-offload-lifecycle-event = offload lifecycle event
log-offload-timeout-policy-exceeded = offload task exceeded timeout policy
log-degradation-entered = L2 degradation entered, serving L1-only
log-degradation-half-open-probing = L2 degradation half-open, probing recovery
log-degradation-recovered = L2 degradation recovered
log-confers-config-reload-rejected = confers config reload rejected; keeping previous snapshot
log-backend-bridge-shutdown-skipped = async→sync bridge shutdown skipped: cannot nest a blocking driver inside a current_thread runtime
log-disk-sweep-failed = disk cache sweep failed
log-disk-open-create-failed = disk backend open/create failed
log-audit-event = cache audit event
log-stale-hit-via-get-or = stale hit served via get_or (Return downgrade; use get_or_refresh for background revalidation)
log-stale-revalidation-scheduled = stale hit; background revalidation scheduled
log-stale-hit-served = stale hit served
log-invalidation-apply-failed = cross-instance invalidation could not be applied to the local cache
log-pubsub-reconnect-failed = pubsub reconnect failed
log-pubsub-handler-panicked = pubsub handler panicked; continuing
log-pubsub-payload-parse-failed = pubsub payload parse failed
log-pubsub-max-reconnect-attempts-reached = pubsub stream ended, max reconnect attempts reached
log-pubsub-reconnecting = pubsub stream ended (connection lost); reconnecting
log-pubsub-resubscribe-failed = pubsub resubscribe failed
log-macro-passthrough = cache passthrough
log-circuit-breaker-transition = circuit breaker state transition

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
panic-canonical-serialize-value-cannot-fail = serializing a Value cannot fail

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
detail-redis-key-sql-injection-pattern = Redis key contains suspicious SQL injection pattern: { $detail }
detail-redis-sql-injection-auth-bypass = SQL authentication bypass
detail-redis-sql-injection-comment = SQL comment pattern
detail-redis-sql-injection-delete = SQL DELETE statement
detail-redis-sql-injection-drop = SQL DROP statement
detail-redis-sql-injection-insert = SQL INSERT statement
detail-redis-sql-injection-quote-or = single quote followed by OR pattern
detail-redis-sql-injection-tautology = classic SQL injection tautology condition
detail-redis-sql-injection-union-select = SQL UNION query
detail-redis-sql-injection-xp-cmdshell = SQL Server command execution
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
detail-disk-open-create-failed = disk backend open failed ({ $open_err }) and create failed ({ $create_err })
detail-cache-memory-requires-feature = Cache::memory() requires the `memory` feature; construct via Cache::new_with_backend instead.
detail-not-supported-moka-sync-current-thread = Moka sync surface cannot be driven from within a current-thread runtime async context (tokio forbids nested blocking drivers). Call the sync API outside a runtime, or use a multi_thread runtime (block_in_place handles it).
detail-redis-unexpected-info-reply = Unexpected INFO { $section } reply type
detail-redis-key-control-character = Redis key contains control character: U+{ $code }
detail-redis-key-path-traversal-pattern = Redis key contains path traversal pattern: { $detail }
detail-redis-key-command-injection-character = Redis key contains potential command injection character: { $detail }
detail-redis-scan-pattern-too-long = SCAN pattern exceeds maximum length of { $max } characters (got { $actual } characters)
detail-redis-scan-pattern-too-many-wildcards = SCAN pattern contains too many wildcards (max { $max }, got { $actual })
detail-pubsub-client-open-failed = pubsub client open: { $err }
detail-pubsub-connection-failed = pubsub connection: { $err }
detail-pubsub-publish-failed = pubsub publish: { $err }
detail-pubsub-connect-failed = pubsub connect: { $err }
detail-pubsub-subscribe-failed = pubsub subscribe: { $err }
detail-pubsub-subscribe-task-ended = pubsub subscribe task ended before ready
detail-lua-script-exceeds-max-length = Lua script exceeds maximum length of { $max } bytes (got { $actual } bytes)
detail-lua-script-exceeds-max-key-count = Lua script exceeds maximum key count of { $max } (got { $actual } keys)
detail-lua-script-forbidden-pattern = Lua script contains forbidden pattern: { $detail }
detail-lua-nested-eval = Lua script contains nested redis.eval/evalsha
detail-lua-infinite-loop-patterns = Lua script contains potential infinite loop patterns
detail-lua-infinite-loop-while-true = WHILE TRUE loop
detail-lua-infinite-loop-while-1 = WHILE 1 loop
detail-lua-infinite-loop-repeat = REPEAT loop
detail-lua-infinite-loop-goto = GOTO statement
detail-backend-layer-restriction-l1-only = L1 only
detail-backend-layer-restriction-l2-and-l3-only = L2/L3 only
detail-backend-layer-restriction-any = Any layer
detail-chain-get-many-length-mismatch = get_many returned { $returned } results for { $expected } keys
detail-chain-parallel-freshest-all-failed = All backends failed during parallel freshest read
detail-chain-all-backends-failed-read = All backends failed during sequential read
detail-chain-all-backends-failed-race-read = All backends failed during race read
detail-chain-all-backends-failed-write = All backends failed to write
detail-chain-all-backends-failed-delete = All backends failed to delete: { $errors }
detail-chain-all-backends-failed-clear = All backends failed to clear: { $errors }
detail-chain-sync-requires-sync-backend = chain sync API requires all links to support SyncCacheBackend
detail-chain-no-backends = Chain has no backends
detail-chain-health-check-timed-out = health_check timed out after 5s
detail-chain-health-check-failed = health_check failed for { $count } backend(s): { $failures }
detail-chain-incr-no-atomic-writer = incr: no link in chain implements AtomicCacheWriter
detail-chain-cas-no-atomic-writer = compare_and_swap: no link in chain implements AtomicCacheWriter
detail-chain-set-if-absent-no-atomic-writer = set_if_absent: no link in chain implements AtomicCacheWriter
detail-key-generator-part-contains-segment-separator = json_hash_key parts must not contain the ':' segment separator: '{ $part }'

# -- Bench diagnostics --
bench-skip-service-unreachable = [bench-skip] required service unreachable: { $services } — start it as the bench module docs describe and rerun (e.g. docker run -d -p 6379:6379 redis:7-alpine)

# -- Example binary output messages --
example-redis-modes-feature-disabled = redis feature is not enabled in this build; nothing to demonstrate.
example-redis-modes-run-with-redis-feature = full run: cargo run -p oxcache-examples --features redis --example example_redis_modes
example-redis-modes-run-with-examples-package = or: cargo run -p oxcache-examples --example example_redis_modes
example-redis-modes-standalone-connect-failed = ✗ Standalone connection failed: { $err }
example-redis-modes-standalone-env-required = a running Redis is required (override with REDIS_URL, default redis://127.0.0.1:6379); skipping the Standalone/Builder demos
example-redis-modes-done-standalone-skipped = ✓ example finished (Standalone skipped: environment unavailable)

# -- example_basic_operations.rs --
example-basic-operations-set-user = Set user: { $name }
example-basic-operations-getting-user = Getting user...
example-basic-operations-got-user = Got user: { $name } ({ $email })
example-basic-operations-deleting-user = Deleting user...
example-basic-operations-updating-user = Updating user...
example-basic-operations-updated-user = Updated user: { $name } ({ $email })
example-basic-operations-example-complete = ✓ Basic operations example complete!
example-basic-operations-set-store-a-value = - Set: store a value in the cache
example-basic-operations-get-read-a-value = - Get: read a value from the cache
example-basic-operations-delete-remove-a-value = - Delete: remove a value from the cache
example-basic-operations-update-overwrite-the-existing = - Update: overwrite the existing value

# -- example_byte_ops.rs --
example-byte-ops-byte-level-operations-example = === Byte-level operations example ===
example-byte-ops-1-basic-byte-operations = --- 1. Basic byte operations ---
example-byte-ops-write-bytes = write: { $data } bytes
example-byte-ops-read = read: { $retrieved }
example-byte-ops-2-byte-operations-without = --- 2. Byte operations without TTL ---
example-byte-ops-write-bytes-no-ttl = write: { $data2 } bytes (no TTL)
example-byte-ops-read-bytes = read: { $retrieved2 } bytes
example-byte-ops-3-binary-data = --- 3. Binary data ---
example-byte-ops-write-bytes-of-binary = write: { $binary_data } bytes of binary data
example-byte-ops-read-bytes-2 = read: { $data } bytes
example-byte-ops-checksum = checksum: { $result }
example-byte-ops-checksum-match = ✓ match
example-byte-ops-checksum-mismatch = ✗ mismatch
example-byte-ops-4-cache-metadata-operations = --- 4. Cache metadata operations ---
example-byte-ops-entries = entries: { $count }
example-byte-ops-is-empty = is empty: { $empty }
example-byte-ops-capacity = capacity: { $cap }
example-byte-ops-5-serializer-access = --- 5. Serializer access ---
example-byte-ops-serializer-instance-obtained-dyn = ✓ serializer instance obtained (dyn Serializer does not implement Debug, cannot print)
example-byte-ops-6-large-data-operations = --- 6. Large data operations ---
example-byte-ops-write-bytes-2 = write: { $large_data } bytes
example-byte-ops-read-bytes-3 = read: { $retrieved4 } bytes
example-byte-ops-after-cleanup-entries-is = after cleanup: entries={ $count_after }, is_empty={ $empty_after }
example-byte-ops-cache-closed = ✓ cache closed
example-byte-ops-example-complete = ✓ Example complete

# -- example_cache_builder.rs --
example-cache-builder-cachebuilder-full-usage-example = === CacheBuilder full usage example ===
example-cache-builder-1-basic-construction = --- 1. Basic construction ---
example-cache-builder-basic-cache-created = ✓ basic cache created
example-cache-builder-2-set-capacity = --- 2. Set capacity ---
example-cache-builder-cache-created-with-capacity = ✓ cache created with capacity 10000
example-cache-builder-3-set-ttl = --- 3. Set TTL ---
example-cache-builder-cache-created-with-1 = ✓ cache created with 1 hour TTL
example-cache-builder-4-set-tti = --- 4. Set TTI ---
example-cache-builder-cache-created-with-5 = ✓ cache created with 5 minute TTI
example-cache-builder-5-combined-configuration = --- 5. Combined configuration ---
example-cache-builder-combined-cache-created-capacity = ✓ combined cache created (capacity=5000, TTL=30min, TTI=10min)
example-cache-builder-6-use-a-custom = --- 6. Use a custom backend ---
example-cache-builder-cache-created-with-a = ✓ cache created with a custom Moka backend
example-cache-builder-7-real-usage = --- 7. Real usage ---
example-cache-builder-write = write: { $product }
example-cache-builder-read = read: { $cached }
example-cache-builder-8-set-with-ttl = --- 8. Set with TTL ---
example-cache-builder-storing-a-product-with = storing a product with 60s TTL
example-cache-builder-exists-check = exists check: { $exists }
example-cache-builder-9-statistics = --- 9. Statistics ---
example-cache-builder-cache-stats = cache stats:
example-cache-builder-10-health-check = --- 10. Health check ---
example-cache-builder-health-status-ok = health status: ✓ OK
example-cache-builder-example-complete = ✓ Example complete

# -- example_cache_key.rs --
example-cache-key-cachekey-trait-example = === CacheKey trait example ===
example-cache-key-1-cachekey-for-built = --- 1. CacheKey for built-in types ---
example-cache-key-2-cachekey-for-custom = --- 2. CacheKey for custom types ---
example-cache-key-3-using-cachekey-in = --- 3. Using CacheKey in the cache ---
example-cache-key-store-key-product = store: key='{ $key1 }', product={ $product }
example-cache-key-read = read: { $cached }
example-cache-key-store-key-product-2 = store: key='{ $key2 }', product={ $another_product }
example-cache-key-4-batch-operations = --- 4. Batch operations ---
example-cache-key-store-key = store: key='{ $key }'
example-cache-key-batch-read-results = batch read: { $results } results
example-cache-key-5-key-format-consistency = --- 5. Key format consistency check ---
example-cache-key-userid-100-userid-100 = UserId(100) == UserId(100): { $key_b } (should be true)
example-cache-key-userid-100-userid-200 = UserId(100) == UserId(200): { $key_c } (should be false)
example-cache-key-same-value-produces-same = same value produces same key: { $to_key_string }
example-cache-key-cachekey-trait-example-complete = === CacheKey trait example complete ===
example-cache-key-key-points = key points:
example-cache-key-built-in-types-string = - built-in types (String, u64, i64, etc.) already implement CacheKey
example-cache-key-custom-types-can-generate = - custom types can generate cache keys by implementing the CacheKey trait
example-cache-key-to-key-string-should = - to_key_string() should return a deterministic, unique string representation
example-cache-key-cachekey-can-be-used = - CacheKey can be used with set/get/delete operations

# -- example_cached_macro.rs --
example-cached-macro-executing-the-original-function = executing the original function (database query)...
example-cached-macro-executing-the-original-function-2 = executing the original function (custom key)...
example-cached-macro-executing-the-original-function-3 = executing the original function (hot data)...
example-cached-macro-usage-example = === #[cached] macro usage example ===
example-cached-macro-cache-registered-for-use = ✓ cache registered for use by the #[cached] macro
example-cached-macro-1-basic-caching-demonstration = 1. Basic caching demonstration
example-cached-macro-first-call-of-get = first call of get_user(1) - runs the function and caches the result...
example-cached-macro-first-call-result = first call result: { $user }
example-cached-macro-second-call-of-get = second call of get_user(1) - served directly from cache...
example-cached-macro-second-call-result = second call result: { $cached_user }
example-cached-macro-2-custom-key-format = 2. Custom key format demonstration
example-cached-macro-calling-get-user-custom = calling get_user_custom_key(2) - uses the 'user_' key prefix...
example-cached-macro-result = result: { $user_custom }
example-cached-macro-calling-get-user-custom-2 = calling get_user_custom_key(2) again - served from cache...
example-cached-macro-cached-result = cached result: { $cached_custom }
example-cached-macro-3-short-ttl-caching = 3. Short TTL caching demonstration
example-cached-macro-calling-get-hot-data = calling get_hot_data(1) - uses a short TTL (300s)...
example-cached-macro-result-2 = result: { $hot_data }
example-cached-macro-calling-get-hot-data-2 = calling get_hot_data(1) again - served from cache...
example-cached-macro-cached-result-2 = cached result: { $cached_hot_data }
example-cached-macro-4-performance-comparison = 4. Performance comparison
example-cached-macro-cache-hits-are-typically = cache hits are typically much faster than running the original function
example-cached-macro-first-call-took = first call took: { $first_duration }
example-cached-macro-cache-hit-took = cache hit took: { $second_duration }
example-cached-macro-speedup-x = speedup: { $f64 }x
example-cached-macro-executing-the-original-function-4 = executing the original function (multi-argument)...
example-cached-macro-5-multi-argument-function = 5. Multi-argument function caching
example-cached-macro-calling-get-user-with = calling get_user_with_role(4, "admin")...
example-cached-macro-result-3 = result: { $result }
example-cached-macro-calling-with-the-same = calling with the same arguments again - served from cache...
example-cached-macro-cached-result-3 = cached result: { $cached_result }
example-cached-macro-calling-with-different-arguments = calling with different arguments - runs the original function...
example-cached-macro-different-argument-result = different-argument result: { $diff_result }
example-cached-macro-example-complete = === #[cached] macro example complete ===
example-cached-macro-main-advantages-of-the = main advantages of the #[cached] macro:
example-cached-macro-zero-boilerplate-caching-logic = - zero boilerplate: caching logic handled automatically
example-cached-macro-type-safe-checked-at = - type safe: checked at compile time
example-cached-macro-flexible-configuration-supports-multiple = - flexible configuration: supports multiple caching strategies
example-cached-macro-automatic-serialization-no-manual = - automatic serialization: no manual handling needed
example-cached-macro-performance-significantly-improves-cache = - performance: significantly improves cache hit performance

# -- example_comprehensive_usage.rs --
example-comprehensive-usage-oxcache-comprehensive-usage-example = === Oxcache comprehensive usage example ===
example-comprehensive-usage-1-create-an-in = 1. Create an in-memory cache (L1)
example-comprehensive-usage-in-memory-cache-created = ✓ in-memory cache created
example-comprehensive-usage-2-create-a-redis = 2. Create a Redis cache (L2 only)
example-comprehensive-usage-redis-cache-created = ✓ Redis cache created
example-comprehensive-usage-redis-unavailable-degraded = ⚠ Redis unavailable, degraded: { $e }
example-comprehensive-usage-3-create-a-tiered = 3. Create a tiered cache (L1 + L2)
example-comprehensive-usage-tiered-cache-created = ✓ tiered cache created
example-comprehensive-usage-4-basic-crud-operations = 4. Basic CRUD operations
example-comprehensive-usage-adding-user = adding user...
example-comprehensive-usage-user-added = ✓ user added: { $name }
example-comprehensive-usage-getting-user = getting user...
example-comprehensive-usage-user-retrieved = ✓ user retrieved: { $name }
example-comprehensive-usage-user-not-found = ✗ user not found
example-comprehensive-usage-updating-user = updating user...
example-comprehensive-usage-user-updated = ✓ user updated
example-comprehensive-usage-user-after-the-update = ✓ user after the update: { $name }
example-comprehensive-usage-user-not-found-2 = ✗ user not found
example-comprehensive-usage-deleting-user = deleting user...
example-comprehensive-usage-failed-to-delete-the = ✗ failed to delete the user
example-comprehensive-usage-user-deleted = ✓ user deleted
example-comprehensive-usage-5-batch-operations = 5. Batch operations
example-comprehensive-usage-adding-products-in-batch = adding products in batch...
example-comprehensive-usage-batch-add-succeeded-products = ✓ batch add succeeded ({ $products } products)
example-comprehensive-usage-fetching-products-in-batch = fetching products in batch...
example-comprehensive-usage-batch-fetch-succeeded-products = ✓ batch fetch succeeded ({ $fetched } products)
example-comprehensive-usage-deleting-products-in-batch = deleting products in batch...
example-comprehensive-usage-batch-delete-succeeded = ✓ batch delete succeeded
example-comprehensive-usage-6-ttl-control = 6. TTL control
example-comprehensive-usage-adding-data-that-expires = adding data that expires in 3 seconds...
example-comprehensive-usage-immediate-get = immediate get: { $await }
example-comprehensive-usage-waiting-4-seconds-then = waiting 4 seconds, then reading...
example-comprehensive-usage-get-after-4-seconds = get after 4 seconds: { $await }
example-comprehensive-usage-7-performance-test = 7. Performance test
example-comprehensive-usage-writing-10000-entries-concurrently = writing 10000 entries concurrently...
example-comprehensive-usage-write-took = ✓ write took: { $write_time }
example-comprehensive-usage-reading-10000-entries-concurrently = reading 10000 entries concurrently...
example-comprehensive-usage-read-took = ✓ read took: { $read_time }
example-comprehensive-usage-8-statistics = 8. Statistics
example-comprehensive-usage-cache-stats = cache stats:
example-comprehensive-usage-example-run-complete = === Example run complete ===

# -- example_get_or.rs --
example-get-or-db-loading-user-from = [DB] loading user { $id } from the database...
example-get-or-single-flight = === get_or single-flight example ===
example-get-or-1-basic-usage = --- 1. Basic usage ---
example-get-or-first-get-or-call = First get_or call:
example-get-or-result = result: { $user1 }
example-get-or-second-get-or-call = Second get_or call:
example-get-or-result-2 = result: { $user2 }
example-get-or-2-concurrent-single-flight = --- 2. Concurrent single-flight deduplication ---
example-get-or-request-started = request { $i } started
example-get-or-request-completed = request { $i } completed
example-get-or-note-only-one-db = note: only one [DB] load runs; the other requests wait for the result
example-get-or-3-comparison-concurrency-without = --- 3. Comparison: concurrency without single-flight ---
example-get-or-note-without-single-flight = note: without single-flight, the [DB] load may run multiple times
example-get-or-example-complete = ✓ Example complete

# -- example_new_api.rs --
example-new-api-usage-example = New API usage example
example-new-api-1-in-memory-cache = 1. In-memory cache (L1 only)
example-new-api-user-retrieved-from-memory = ✓ user retrieved from memory cache: { $name }
example-new-api-2-redis-cache-l2 = 2. Redis cache (L2 only)
example-new-api-user-retrieved-from-redis = ✓ user retrieved from Redis cache: { $name }
example-new-api-redis-unavailable-skipping-redis = ⚠ Redis unavailable, skipping Redis test: { $e }
example-new-api-3-custom-key-type = 3. Custom key type
example-new-api-user-retrieved-with-custom = ✓ user retrieved with custom key: { $name }
example-new-api-4-advanced-configuration-using = 4. Advanced configuration (using Builder)
example-new-api-user-retrieved-from-the = ✓ user retrieved from the advanced-config cache: { $name }
example-new-api-5-ttl-operations-using = 5. TTL operations (using set_with_ttl)
example-new-api-set-user-7-with = ✓ set user:7 with a 60 second TTL
example-new-api-updated-the-ttl-of = ✓ updated the TTL of user:7 to 120 seconds
example-new-api-6-batch-operations = 6. Batch operations
example-new-api-batch-set-users = ✓ batch set { $batch } users
example-new-api-retrieved-users-from-the = ✓ retrieved { $retrieved_count } users from the cache
example-new-api-7-delete-operations = 7. Delete operations
example-new-api-user-8-exists = ✓ user:8 exists: { $exists }
example-new-api-user-8-exists-after = ✓ user:8 exists after delete: { $exists }
example-new-api-8-clear-operations = 8. Clear operations
example-new-api-set-3-users-in = ✓ set 3 users in the cache
example-new-api-cleared-all-cache-entries = ✓ cleared all cache entries
example-new-api-user-1-exists-after = ✓ user:1 exists after clear: { $exists }
example-new-api-9-cache-aside-mode = 9. Cache-aside mode with fallback (get_or)
example-new-api-user-retrieved-via-the = ✓ user retrieved via the fallback function: { $name }
example-new-api-10-summary = 10. Summary
example-new-api-features = New API features:
example-new-api-type-safe-cache-interface = ✓ type-safe cache interface
example-new-api-in-memory-cache-cache = ✓ in-memory cache (Cache::memory())
example-new-api-redis-cache-cache-redis = ✓ Redis cache (Cache::redis())
example-new-api-custom-key-types-cachekey = ✓ custom key types (CacheKey trait)
example-new-api-builder-pattern-for-configuration = ✓ builder pattern for configuration (Cache::builder())
example-new-api-ttl-operations-set-with = ✓ TTL operations (set_with_ttl)
example-new-api-batch-operations-set-many = ✓ batch operations (set_many, get_many)
example-new-api-delete-and-clear-operations = ✓ delete and clear operations (delete, clear)
example-new-api-existence-check-exists = ✓ existence check (exists)
example-new-api-cache-aside-mode-with = ✓ cache-aside mode with fallback (get_or)
example-new-api-example-complete = ✅ New API example complete!

# -- example_serialization.rs --
example-serialization-options-example = Serialization options example
example-serialization-1-json-serialization-default = 1. JSON serialization (default):
example-serialization-human-readable-format = - human-readable format
example-serialization-good-for-debugging = - good for debugging
example-serialization-slightly-larger-size = - slightly larger size
example-serialization-stored-user = stored user: { $name } - { $email }
example-serialization-retrieved = retrieved: { $name } - { $email }
example-serialization-config = config: { $profile }
example-serialization-2-serialization-features = 2. Serialization features:
example-serialization-json-standard-readable-format = - JSON: standard, readable format
example-serialization-bincode-compact-binary-format = - Bincode: compact binary format, efficient (when enabled)
example-serialization-supports-custom-serializers = - supports custom serializers
example-serialization-example-complete = ✓ Serialization example complete!

# -- example_sync_api.rs --
example-sync-api-basic-sync-operations = === Basic sync operations ===
example-sync-api-set-user-1 = Set user:1 = { $alice }
example-sync-api-get-user-1 = Get user:1 = { $cached }
example-sync-api-delete-user-1 = Delete user:1
example-sync-api-set-temp-60s-ttl = Set temp (60s TTL)
example-sync-api-get-temp = Get temp = { $cached }
example-sync-api-get-or-sync-single = === get_or_sync (single-flight) ===
example-sync-api-first-get-or-sync = First get_or_sync = { $user } (fallback was called)
example-sync-api-second-get-or-sync = Second get_or_sync = { $user } (cache hit)
example-sync-api-mixing-sync-and-async = === Mixing sync and async ===
example-sync-api-cache-metadata-sync = === Cache metadata (sync) ===
example-sync-api-entries-len-sync = Entries (len_sync): { $len }
example-sync-api-cache-cleared = ✓ cache cleared
example-sync-api-entries-after-clear = Entries after clear: { $len_after }
example-sync-api-example-complete = Sync API example complete!

# -- example_batch_write.rs --
example-batch-write-optimization-example = === Batch write optimization example ===
example-batch-write-1-batch-add-products = 1. Batch add products
example-batch-write-batch-adding-products = batch adding { $products } products...
example-batch-write-batch-add-complete-took = ✓ batch add complete, took: { $elapsed }
example-batch-write-2-batch-update-prices = 2. Batch update prices (simulating a promotion)
example-batch-write-updating-product-prices = Updating product prices:
example-batch-write-product-new-price = product { $id }: { $name } new price: ¥{ $price }
example-batch-write-3-batch-read-verification = 3. Batch read verification
example-batch-write-reading-all-product-info = Reading all product info:
example-batch-write-stock = [{ $id }] { $name } - ¥{ $price } (stock: { $stock })
example-batch-write-4-using-set-many = 4. Using set_many / get_many / delete_many
example-batch-write-set-many-wrote-entries = ✓ set_many wrote { $new_products } entries
example-batch-write-get-many-read-entries = ✓ get_many read { $fetched } entries
example-batch-write-delete-many-deleted-entries = ✓ delete_many deleted { $del_keys } entries
example-batch-write-entries-remain-after-delete = { $remaining } entries remain after delete
example-batch-write-5-batch-delete-simulating = 5. Batch delete (simulating delisted products)
example-batch-write-delisted-product-ids = Delisted product IDs: { $out_of_stock_ids }
example-batch-write-products-delisted = ✓ products delisted
example-batch-write-6-cache-statistics = 6. Cache statistics
example-batch-write-cache-type = - cache type: { $stats }
example-batch-write-capacity = - capacity: { $stats }
example-batch-write-entries = - entries: { $stats }
example-batch-write-optimization-example-2 = === Batch write optimization example complete ===

# -- example_cache_promotion.rs --
example-cache-promotion-strategy-example = === Cache promotion strategy example ===
example-cache-promotion-1-simulating-l1-l2 = --- 1. Simulating L1 + L2 tiers ---
example-cache-promotion-l1-memory-capacity-3 = L1 (memory): capacity=3 (simulating a fast cache)
example-cache-promotion-l2-cache-capacity-100 = L2 (cache): capacity=100 (simulating a slow/large cache)
example-cache-promotion-2-initialize-data-into = --- 2. Initialize data into L2 ---
example-cache-promotion-wrote-sessions-into-l2 = ✓ wrote { $sessions } sessions into L2
example-cache-promotion-3-reads-with-promotion = --- 3. Reads with promotion (promote_on_hit) ---
example-cache-promotion-l1-hit = [L1 hit] { $key }
example-cache-promotion-l2-hit-promoted-to = [L2 hit -> promoted to L1] { $key }
example-cache-promotion-all-miss = [all miss] { $key }
example-cache-promotion-access-sequence-accesses = Access sequence ({ $access_sequence } accesses):
example-cache-promotion-l1-final-state-entries = L1 final state: { $l1 } entries
example-cache-promotion-4-access-frequency-analysis = --- 4. Access frequency analysis ---
example-cache-promotion-access-frequency-ranking = Access frequency ranking:
example-cache-promotion-times = { $key } { $bar } ({ $count } times)
example-cache-promotion-hot-data-sess-1 = ✓ hot data (sess_1) stays in L1 automatically
example-cache-promotion-5-production-configuration-suggestions = --- 5. Production configuration suggestions ---
example-cache-promotion-a-real-tiered-cache = A real tiered-cache configuration:
example-cache-promotion-l1-moka-l2-redis = // L1 (Moka) + L2 (Redis) tiers
example-cache-promotion-ttl-duration-from-secs = .ttl(Duration::from_secs(3600))     // L1 default TTL
example-cache-promotion-capacity-10-000-l1 = .capacity(10_000)                    // L1 capacity
example-cache-promotion-promotion-strategy-notes = Promotion strategy notes:
example-cache-promotion-l1-is-small-but = - L1 is small but fast, L2 is large but slower
example-cache-promotion-on-an-l2-hit = - on an L2 hit, backfill into L1 automatically (promote_on_hit)
example-cache-promotion-hot-data-is-promoted = - hot data is promoted to L1 naturally; cold data stays in L2
example-cache-promotion-size-l1-sensibly-to = - size L1 sensibly to avoid frequent eviction
example-cache-promotion-6-ttl-effects-on = --- 6. TTL effects on promotion ---
example-cache-promotion-set-a-session-with = ✓ set a session with a short TTL (5s)
example-cache-promotion-remaining-ttl = remaining TTL: { $remaining }
example-cache-promotion-example-complete = ✓ Cache promotion example complete

# -- example_chain_cache.rs --
example-chain-cache-example = === Chain cache example ===
example-chain-cache-1-chaincache-builder-creates = 1. ChainCache::builder() creates a multi-tier cache
example-chain-cache-connecting-to-redis = Connecting to Redis: { $redis_url }
example-chain-cache-created-a-2-tier = ✓ created a 2-tier cache chain: L1(Moka) -> L2(Redis)
example-chain-cache-cannot-connect-to-redis = ✗ cannot connect to Redis: { $e }
example-chain-cache-skipping-the-redis-example = skipping the Redis example, using an in-memory cache only
example-chain-cache-2-in-memory-only = 2. In-memory-only cache example
example-chain-cache-store = store: { $user }
example-chain-cache-read = read: { $cached }
example-chain-cache-3-batch-operations-example = 3. Batch operations example
example-chain-cache-batch-stored-users = batch stored { $users } users
example-chain-cache-batch-read-users = batch read: { $results } users
example-chain-cache-example-complete = ✓ Example complete
example-chain-cache-test-data = test data: { $user }
example-chain-cache-writing-data = writing data...
example-chain-cache-data-written-to-the = ✓ data written to the cache chain
example-chain-cache-reading-data = reading data:
example-chain-cache-hit = ✓ hit: { $u }
example-chain-cache-failed-to-parse-the = ✗ failed to parse the data
example-chain-cache-not-found = ✗ not found
example-chain-cache-cache-stats = cache stats:
example-chain-cache-test-data-2 = test data: { $user }
example-chain-cache-writing-data-2 = writing data...
example-chain-cache-data-written-to-the-2 = ✓ data written to the cache
example-chain-cache-reading-data-2 = reading data:
example-chain-cache-hit-2 = ✓ hit: { $u }
example-chain-cache-not-found-2 = ✗ not found
example-chain-cache-cache-stats-2 = cache stats:

# -- example_custom_backend.rs --
example-custom-backend-hashmapbackend-shut-down-data = [HashMapBackend] shut down, data cleared
example-custom-backend-example = === Custom backend example ===
example-custom-backend-1-create-a-custom = --- 1. Create a custom backend ---
example-custom-backend-hashmapbackend-created-capacity-100 = ✓ HashMapBackend created (capacity=100)
example-custom-backend-2-inject-it-via = --- 2. Inject it via CacheBuilder ---
example-custom-backend-cache-built-with-the = ✓ Cache built with the custom backend
example-custom-backend-3-basic-crud-operations = --- 3. Basic CRUD operations ---
example-custom-backend-4-ttl-operations = --- 4. TTL operations ---
example-custom-backend-set-with-ttl-60s = ✓ set_with_ttl 60s, remaining TTL: { $remaining }
example-custom-backend-expire-updated-to-120s = ✓ expire updated to 120s: { $expired }
example-custom-backend-5-batch-operations = --- 5. Batch operations ---
example-custom-backend-batch-wrote-entries = ✓ batch wrote { $items } entries
example-custom-backend-cache-entries = ✓ cache entries: { $count }
example-custom-backend-6-statistics = --- 6. Statistics ---
example-custom-backend-7-health-check = --- 7. Health check ---
example-custom-backend-backend-healthy = ✓ backend healthy
example-custom-backend-health-check-failed = ✗ health check failed: { $e }
example-custom-backend-8-capacity-limit-demonstration = --- 8. Capacity limit demonstration ---
example-custom-backend-wrote-3-entries-capacity = ✓ wrote 3 entries (capacity=3)
example-custom-backend-unexpected-the-write-succeeded = unexpected: the write succeeded
example-custom-backend-capacity-full-the-write = ✓ capacity full, the write failed: { $e }
example-custom-backend-9-shutdown = --- 9. Shutdown ---
example-custom-backend-cache-closed = ✓ cache closed
example-custom-backend-example-complete = ✓ Custom backend example complete

# -- example_dashmap_backend.rs --
example-dashmap-backend-example = === DashMap backend example ===
example-dashmap-backend-1-create-a-dashmap = --- 1. Create a DashMap backend ---
example-dashmap-backend-created = ✓ DashMap backend created
example-dashmap-backend-2-basic-operations = --- 2. Basic operations ---
example-dashmap-backend-write-key1-value1 = write: key1 = value1
example-dashmap-backend-read-key1 = read: key1 = { $value }
example-dashmap-backend-exists-key1 = exists: key1 = { $exists }
example-dashmap-backend-3-batch-operations = --- 3. Batch operations ---
example-dashmap-backend-batch-write-key-value = batch write: { $keys_values } key-value pairs
example-dashmap-backend-batch-read-results = batch read: { $results } results
example-dashmap-backend-4-comparing-moka-and = --- 4. Comparing Moka and DashMap ---
example-dashmap-backend-moka-backend-keys = Moka backend: { $moka_count } keys
example-dashmap-backend-keys = DashMap backend: { $dashmap_count } keys
example-dashmap-backend-5-delete-operations = --- 5. Delete operations ---
example-dashmap-backend-delete-key1 = delete: key1
example-dashmap-backend-exists-check-key1 = exists check: key1 = { $exists_after }
example-dashmap-backend-6-clear-operations = --- 6. Clear operations ---
example-dashmap-backend-before-clear-keys = before clear: { $count_before } keys
example-dashmap-backend-after-clear-keys = after clear: { $count_after } keys
example-dashmap-backend-7-feature-comparison = --- 7. Feature comparison ---
example-dashmap-backend-moka-backend-traits = Moka backend traits:
example-dashmap-backend-lru-tinylfu-eviction-policies = - LRU/TinyLFU eviction policies
example-dashmap-backend-capacity-limits = - capacity limits
example-dashmap-backend-automatic-expiry = - automatic expiry
example-dashmap-backend-suitable-for-production = - suitable for production
example-dashmap-backend-traits = DashMap backend traits:
example-dashmap-backend-plain-concurrent-hashmap = - plain concurrent HashMap
example-dashmap-backend-no-eviction-policy = - no eviction policy
example-dashmap-backend-manual-ttl-management = - manual TTL management
example-dashmap-backend-suited-to-simple-scenarios = - suited to simple scenarios or tests
example-dashmap-backend-example-complete = ✓ Example complete

# -- example_error_handling.rs --
example-error-handling-example = === Error handling example ===
example-error-handling-1-error-type-classification = --- 1. Error type classification ---
example-error-handling-2-recoverability = --- 2. Recoverability ---
example-error-handling-3-error-codes-and = --- 3. Error codes and logging ---
example-error-handling-4-resilient-operations-with = --- 4. Resilient operations with retry ---
example-error-handling-5-triggering-runtime-errors = --- 5. Triggering runtime errors ---
example-error-handling-6-error-propagation-with = --- 6. Error propagation with the ? operator ---
example-error-handling-7-configuration-time-errors = --- 7. Configuration-time errors ---
example-error-handling-example-complete = ✓ Error handling example complete
example-error-handling-connection-error-recoverable = connection error: { $conn_err } (recoverable: { $is_recoverable })
example-error-handling-timeout-error-recoverable = timeout error: { $timeout_err } (recoverable: { $is_recoverable })
example-error-handling-backend-error-recoverable = backend error: { $backend_err } (recoverable: { $is_recoverable })
example-error-handling-not-found-recoverable = not found:   { $not_found } (recoverable: { $is_recoverable })
example-error-handling-serialization-recoverable = serialization:   { $set_err } (recoverable: { $is_recoverable })
example-error-handling-degraded-degraded-mode = degraded:     { $degraded } (degraded mode: { $is_degraded })
example-error-handling-internal-error-recoverable = internal error: { $internal } (recoverable: { $is_recoverable })
example-error-handling-error-code = { $type } { $recoverable } { $error } error code
example-error-handling-error-message = error message: { $err }
example-error-handling-error-code-2 = error code:   { $code }
example-error-handling-retryable = retryable:   { $is_recoverable }
example-error-handling-succeeded-on-retry = ✓ { $operation_name } succeeded on retry { $attempt }
example-error-handling-hit-an-unrecoverable-error = ✗ { $operation_name } hit an unrecoverable error: { $e }
example-error-handling-failed-on-attempt-retrying = ⚠ { $operation_name } failed on attempt { $attempt } ({ $code }), retrying in { $as_millis }ms...
example-error-handling-reached-the-maximum-retry = ✗ { $operation_name } reached the maximum retry count { $max_retries }
example-error-handling-final-result = final result: { $result }
example-error-handling-unrecoverable-error-result = unrecoverable error result: { $result }
example-error-handling-col-error-type = error type
example-error-handling-col-recoverable = recoverable?
example-error-handling-col-connection-error = connection error?
example-error-handling-result-retry-skipped = correctly skipped the retry
example-error-handling-result-unexpected = unexpected
example-error-handling-cache-operation-succeeded = cache operation succeeded: { $name }
example-error-handling-normal-read = normal read: { $val }
example-error-handling-a-missing-key-returns = a missing key returns: { $missing } (not an error)
example-error-handling-get-or-computed-result = get_or computed result: { $value }
example-error-handling-health-check-passed = health check: ✓ passed
example-error-handling-health-check = health check: ✗ { $e } ({ $code })
example-error-handling-user-1-score = user 1 score: { $score }
example-error-handling-load-failed = load failed: { $e }
example-error-handling-user-999-score = user 999 score: { $score }
example-error-handling-user-999-load-failed = user 999 load failed: { $e } (code={ $code })
example-error-handling-config-error-code = config error: { $err } [code={ $code }]

# -- example_invalidation.rs --
example-invalidation-cache-invalidation-strategy-example = === Cache invalidation strategy example ===
example-invalidation-1-prepare-test-data = 1. Prepare test data
example-invalidation-added-orders = ✓ added { $orders } orders
example-invalidation-2-single-key-invalidation = 2. Single-key invalidation
example-invalidation-deleting-order-1 = Deleting order:1
example-invalidation-the-order-still-exists = ✗ the order still exists
example-invalidation-the-order-was-deleted = ✓ the order was deleted
example-invalidation-3-bulk-invalidation-by = 3. Bulk invalidation (by clearing the cache)
example-invalidation-order-2-still-exists = ✗ order:2 still exists
example-invalidation-all-orders-cleared = ✓ all orders cleared
example-invalidation-4-user-id-based = 4. User-ID-based invalidation patterns
example-invalidation-original-cart = Original cart:
example-invalidation-user-100-checks-out = user 100 checks out, clearing their cart...
example-invalidation-after-clear = After clear:
example-invalidation-5-ttl-based-automatic = 5. TTL-based automatic invalidation
example-invalidation-adding-data-that-expires = Adding data that expires in 2 seconds
example-invalidation-immediate-get = immediate get: { $await }
example-invalidation-waiting-3-seconds = Waiting 3 seconds...
example-invalidation-get-after-3-seconds = get after 3 seconds: { $await }
example-invalidation-6-write-time-invalidation = 6. Write-time invalidation (write-invalidation)
example-invalidation-initial-theme = Initial theme: { $await }
example-invalidation-updated-theme = Updated theme: { $await }
example-invalidation-strategy-example-complete = === Invalidation strategy example complete ===

# -- example_lua_script.rs --
example-lua-script-execution-example = === Lua script execution example ===
example-lua-script-connecting-to-redis = Connecting to Redis: { $redis_url }
example-lua-script-redis-connected = ✓ Redis connected
example-lua-script-1-atomic-counter-eval = --- 1. Atomic counter (eval_lua) ---
example-lua-script-call-counter-value = call { $i }: counter value: { $result }
example-lua-script-2-preloading-scripts-script = --- 2. Preloading scripts (script_load) ---
example-lua-script-script-sha1 = script SHA1: { $sha }
example-lua-script-3-executing-scripts-by = --- 3. Executing scripts by SHA (eval_sha) ---
example-lua-script-eval-lua-result = eval_lua result: { $result }
example-lua-script-eval-sha-result = eval_sha result: { $result }
example-lua-script-4-conditional-updates-compare = --- 4. Conditional updates (compare-and-swap) ---
example-lua-script-initial-value-v1 = Initial value: v1
example-lua-script-expecting-v2-updating-to = expecting v2, updating to v3: { $result } (should be 0, meaning failure)
example-lua-script-expecting-v1-updating-to = expecting v1, updating to v2: { $result } (should be 1, meaning success)
example-lua-script-example-complete-test-data = ✓ Example complete, test data cleaned up

# -- example_moka_ttl.rs --
example-moka-ttl-creating-mokamemorybackend-no-global = Creating MokaMemoryBackend (no global TTL)
example-moka-ttl-set-k1-60s-per = set 'k1' (60s per-entry TTL)
example-moka-ttl-ttl-k1-should-be = ttl('k1') = { $as_secs } (should be about 60s)
example-moka-ttl-expire-updates-ttl = === expire updates TTL ===
example-moka-ttl-expire-k1-120s-should = expire('k1', 120s) = { $result } (should return true when the key exists)
example-moka-ttl-ttl-k1-should-be-2 = ttl('k1') = { $as_secs } (should be about 120s)
example-moka-ttl-expire-missing-60s-should = expire('missing', 60s) = { $result } (should return false when the key does not exist)
example-moka-ttl-keys-without-ttl = === Keys without TTL ===
example-moka-ttl-set-k2-no-ttl = set 'k2' (no TTL, never expires)
example-moka-ttl-ttl-k2-should-be = ttl('k2') = { $ttl } (should be None without a per-entry TTL)
example-moka-ttl-ttl-expiry-verification = === TTL expiry verification ===
example-moka-ttl-set-temp-50ms-ttl = set 'temp' (50ms TTL)
example-moka-ttl-immediate-get-temp = immediate get('temp') = { $value }
example-moka-ttl-get-temp-after-100ms = get('temp') after 100ms = { $value } (should be expired)
example-moka-ttl-global-ttl-vs-per = === Global TTL vs per-entry TTL ===
example-moka-ttl-creating-mokamemorybackend-global-ttl = Creating MokaMemoryBackend (global TTL=300ms)
example-moka-ttl-set-a-no-per = set 'a' (no per-entry TTL -> inherits the global 300ms)
example-moka-ttl-set-b-per-entry = set 'b' (per-entry TTL=600ms > global 300ms -> per-entry wins)
example-moka-ttl-ttl-a-none-ttl = ttl('a') = { $as_millis } (None: ttl() reports only per-entry TTL; 'a' still expires on the global 300ms)
example-moka-ttl-ttl-b-should-be = ttl('b') = { $as_millis } (should be about 600ms: overrides the global, not capped at 300ms)
example-moka-ttl-get-a-after-450ms = get('a') after 450ms = { $a } (global 300ms elapsed -> None)
example-moka-ttl-get-b-after-450ms = get('b') after 450ms = { $b } (per-entry 600ms not yet elapsed -> Some)
example-moka-ttl-moka-per-entry-ttl = Moka per-entry TTL example complete!

# -- example_redis_modes.rs --
example-redis-modes-redis-multi-mode-connection = === Redis multi-mode connection example ===
example-redis-modes-redis-multi-mode-connection-2 = === Redis multi-mode connection example ===
example-redis-modes-1-standalone-mode = --- 1. Standalone mode ---
example-redis-modes-connected = connected: { $standalone_url }
example-redis-modes-connected-mode = ✓ connected, mode: { $mode }
example-redis-modes-write-read = write/read: { $value }
example-redis-modes-2-explicitly-selecting-standalone = --- 2. Explicitly selecting Standalone mode with the Builder ---
example-redis-modes-builder-created-mode = ✓ Builder created, mode: { $mode }
example-redis-modes-3-cluster-mode = --- 3. Cluster mode ---
example-redis-modes-connected-2 = connected: { $cluster_url }
example-redis-modes-cluster-connected-mode = ✓ Cluster connected, mode: { $mode }
example-redis-modes-write-read-2 = write/read: { $value }
example-redis-modes-cluster-connection-failed = ✗ Cluster connection failed: { $e }
example-redis-modes-redis-cluster-available-is = ⚠ REDIS_CLUSTER_AVAILABLE is not set, skipping the Cluster test
example-redis-modes-start-cluster-cd-tests = Start Cluster: cd tests/real_env && docker compose -f docker-compose.cluster.yml up -d
example-redis-modes-4-sentinel-mode = --- 4. Sentinel mode ---
example-redis-modes-sentinel-connection = Sentinel connection: { $sentinel_url }
example-redis-modes-sentinel-connected-mode = ✓ Sentinel connected, mode: { $mode }
example-redis-modes-write-read-3 = write/read: { $value }
example-redis-modes-sentinel-connection-failed = ✗ Sentinel connection failed: { $e }
example-redis-modes-redis-sentinel-available-is = ⚠ REDIS_SENTINEL_AVAILABLE is not set, skipping the Sentinel test
example-redis-modes-start-sentinel-cd-tests = Start Sentinel: cd tests/real_env && docker compose -f docker-compose.sentinel.yml up -d
example-redis-modes-example-complete = ✓ Example complete
example-redis-modes-5-the-redismodetype-enum = --- 5. The RedisModeType enum ---
example-redis-modes-mode-display = mode: { $mode } (Display: { $mode2 })

# -- example_redis_native.rs --
example-redis-native-client-example = === Redis native client example ===
example-redis-native-1-connect-to-redis = 1. Connect to Redis
example-redis-native-redis-connected = ✓ Redis connected
example-redis-native-2-basic-crud-operations = 2. Basic CRUD operations
example-redis-native-adding-user = adding user...
example-redis-native-user-added = ✓ user added
example-redis-native-getting-user = getting user...
example-redis-native-user-retrieved = ✓ user retrieved: { $name } ({ $email })
example-redis-native-user-not-found = ✗ user not found
example-redis-native-updating-user = updating user...
example-redis-native-user-updated = ✓ user updated
example-redis-native-deleting-user = deleting user...
example-redis-native-failed-to-delete-the = ✗ failed to delete the user
example-redis-native-user-deleted = ✓ user deleted
example-redis-native-3-batch-operations = 3. Batch operations
example-redis-native-adding-users-in-batch = adding users in batch...
example-redis-native-batch-add-succeeded = ✓ batch add succeeded
example-redis-native-fetching-users-in-batch = fetching users in batch...
example-redis-native-user = ✓ user { $id }: { $name }
example-redis-native-clearing-test-data = clearing test data...
example-redis-native-cleared = ✓ cleared
example-redis-native-4-cache-statistics = 4. Cache statistics
example-redis-native-cache-statistics-are-available = cache statistics are available through the metrics interface
example-redis-native-see-the-metrics-test = (see the metrics_test example for detailed statistics)
example-redis-native-client-example-2 = === Redis native client example complete ===

# -- example_redis_pipeline.rs --
example-redis-pipeline-batch-operations = === Redis Pipeline batch operations example ===
example-redis-pipeline-connecting-to-redis = Connecting to Redis: { $redis_url }
example-redis-pipeline-redis-connected = ✓ Redis connected
example-redis-pipeline-1-batch-set-set = --- 1. Batch set (set_many_pipeline) ---
example-redis-pipeline-batch-set-keys-took = batch set { $items } keys, took: { $elapsed }
example-redis-pipeline-2-batch-get-get = --- 2. Batch get (get_many_pipeline) ---
example-redis-pipeline-batch-get-keys-took = batch get { $keys } keys, took: { $elapsed }
example-redis-pipeline-results = results:
example-redis-pipeline-3-performance-comparison-one = --- 3. Performance comparison: one-by-one vs Pipeline ---
example-redis-pipeline-setting-10-keys-one = setting 10 keys one by one: { $individual_elapsed }
example-redis-pipeline-setting-10-keys-via = setting 10 keys via Pipeline: { $pipeline_elapsed }
example-redis-pipeline-pipeline-is-x-faster = Pipeline is { $max }x faster
example-redis-pipeline-4-batch-delete-delete = --- 4. Batch delete (delete_many_pipeline) ---
example-redis-pipeline-batch-deleted-keys = batch deleted { $all_keys } keys
example-redis-pipeline-remaining-keys-should-be = remaining keys: { $remaining } (should be 0)
example-redis-pipeline-example-complete = ✓ Example complete

# -- example_smart_strategy.rs --
example-smart-strategy-cache-strategy-patterns-example = === Cache strategy patterns example ===
example-smart-strategy-1-the-cache-aside = --- 1. The Cache-Aside strategy ---
example-smart-strategy-read-fill-on-miss = Read-fill on miss: load from the data source and write into the cache
example-smart-strategy-write-invalidate-after-updating = Write-invalidate: after updating the data source, delete the cache entry
example-smart-strategy-first-read-of-1 = first read of #1: { $title } (data source queries: { $query_count })
example-smart-strategy-second-read-of-1 = second read of #1: { $title } (data source queries: { $query_count })
example-smart-strategy-updating-1-and-invalidating = updating #1 and invalidating the cache
example-smart-strategy-read-of-1-after = read of #1 after the update: { $title } (data source queries: { $query_count })
example-smart-strategy-2-the-lazy-loading = --- 2. The Lazy Loading strategy ---
example-smart-strategy-lazy-computation-with-get = lazy computation with get_or: run the expensive computation only on a cache miss
example-smart-strategy-first-get-or-on = first get_or on #42: { $title } (computations: { $seqcst })
example-smart-strategy-second-get-or-on = second get_or on #42: { $title } (computations: { $seqcst })
example-smart-strategy-3-ttl-lifecycle-management = --- 3. TTL lifecycle management ---
example-smart-strategy-different-data-types-use = different data types use different expiry policies
example-smart-strategy-session-data-ttl-30s = ✓ session data: TTL=30s
example-smart-strategy-popular-articles-ttl-1h = ✓ popular articles: TTL=1h
example-smart-strategy-system-config-ttl-24h = ✓ system config: TTL=24h
example-smart-strategy-terms-of-service-no = ✓ terms of service: no TTL (never expires)
example-smart-strategy-remaining-ttl = remaining TTL:
example-smart-strategy-sessions = sessions:   { $session_ttl }
example-smart-strategy-config = config:   { $config_ttl }
example-smart-strategy-terms = terms:   { $zero }
example-smart-strategy-4-capacity-planning-and = --- 4. Capacity planning and access-frequency tiers ---
example-smart-strategy-prewarmed-popular-articles = ✓ prewarmed { $hot_ids } popular articles
example-smart-strategy-simulating-accesses = simulating { $total } accesses:
example-smart-strategy-hits = hits: { $hits } ({ $f64 }%)
example-smart-strategy-misses = misses: { $misses } ({ $f64 }%)
example-smart-strategy-5-cache-statistics = --- 5. Cache statistics ---
example-smart-strategy-entries = entries: { $stats }
example-smart-strategy-capacity = capacity:   { $stats }
example-smart-strategy-cache-strategy-example-complete = ✓ Cache strategy example complete

# -- example_warmup.rs --
example-warmup-cache-warmup-example = === Cache warmup example ===
example-warmup-1-simulating-application-config = 1. Simulating application config warmup
example-warmup-loading-config-from-the = loading config from the database...
example-warmup-loading-config = loading config: { $k } = { $v } ({ $desc })
example-warmup-config-warmup-complete-items = ✓ config warmup complete ({ $configs } items)
example-warmup-2-simulating-hot-user = 2. Simulating hot-user data warmup
example-warmup-preloading-hot-users = preloading hot users...
example-warmup-hot-user-warmup-complete = ✓ hot-user warmup complete ({ $hot_users } users, took: { $elapsed })
example-warmup-3-verifying-the-warmed = 3. Verifying the warmed data
example-warmup-config-verification = Config verification:
example-warmup-expected = ✗ { $k } = { $val } (expected: { $v })
example-warmup-not-found = ✗ { $k } not found
example-warmup-user-verification = User verification:
example-warmup-not-found-2 = ✗ { $key } not found
example-warmup-4-simulating-a-cache = 4. Simulating a cache rebuild
example-warmup-clearing-the-cache = clearing the cache...
example-warmup-rewarming = rewarming...
example-warmup-cache-rebuild-complete-took = ✓ cache rebuild complete, took: { $elapsed }
example-warmup-5-post-warmup-statistics = 5. Post-warmup statistics
example-warmup-total-entries = - total entries: { $stats }
example-warmup-hits = - hits: { $stats }
example-warmup-cache-warmup-example-complete = === Cache warmup example complete ===

# -- example_dynamic_config.rs --
example-dynamic-config-example = === Dynamic config example ===
example-dynamic-config-1-initializing-config = 1. Initializing config
example-dynamic-config-2-config-queries = 2. Config queries
example-dynamic-config-type = ✓ { $key } = { $value } (type: { $config_type })
example-dynamic-config-not-found = ✗ { $key } not found
example-dynamic-config-3-config-update-demonstration = 3. Config update demonstration
example-dynamic-config-updating-app-theme = updating app.theme...
example-dynamic-config-app-theme-updated-to = ✓ app.theme updated to light
example-dynamic-config-current-app-theme = ✓ current app.theme = { $value }
example-dynamic-config-4-config-export = 4. Config export
example-dynamic-config-exporting-all-config = exporting all config:
example-dynamic-config-5-config-groups-by = 5. Config groups by type
example-dynamic-config-application-config-items = application config ({ $app_configs } items):
example-dynamic-config-database-config-items = database config ({ $db_configs } items):
example-dynamic-config-6-cleaning-up-test = 6. Cleaning up test data
example-dynamic-config-test-data-cleaned-up = ✓ test data cleaned up
example-dynamic-config-example-complete = === Dynamic config example complete ===

# -- example_key_generator.rs --
example-key-generator-example = === Key generator example ===
example-key-generator-1-basic-usage = --- 1. Basic usage ---
example-key-generator-default-namespace-default = default namespace: 'default'
example-key-generator-default-max-length-256 = default max length: 256
example-key-generator-2-using-namespaces = --- 2. Using namespaces ---
example-key-generator-user-key = user key: { $user_key }
example-key-generator-order-key = order key: { $order_key }
example-key-generator-3-using-prefixes = --- 3. Using prefixes ---
example-key-generator-cache-key = cache key: { $key1 }
example-key-generator-session-key = session key: { $key2 }
example-key-generator-4-chaining-namespace-prefix = --- 4. Chaining (namespace + prefix) ---
example-key-generator-full-key = full key: { $key }
example-key-generator-5-template-generation = --- 5. Template generation ---
example-key-generator-generate-user-id-profile = generate('user:{"{"}id{"}"}:profile', id=42) -> { $key }
example-key-generator-generate-full-search-type = generate_full('search:{"{"}type{"}"}:{"{"}query{"}"}') -> { $key }
example-key-generator-6-full-key-generation = --- 6. Full key generation (generate_full) ---
example-key-generator-generate-full-user-id = generate_full('user:{"{"}id{"}"}', id=123) -> { $key }
example-key-generator-7-key-validation = --- 7. Key validation ---
example-key-generator-is-valid = ✓ '{ $key }' is valid
example-key-generator-is-invalid = ✗ '{ $key }' is invalid: { $e }
example-key-generator-is-valid-2 = ✓ '{ $key }' is valid
example-key-generator-is-invalid-2 = ✗ '{ $key }' is invalid: { $e }
example-key-generator-8-in-practice-cache = --- 8. In practice: cache operations ---
example-key-generator-store = store: { $user_key } = { $name }
example-key-generator-store-2 = store: { $user_key2 } = { $name2 }
example-key-generator-example-complete = ✓ Example complete

# -- example_database_integration.rs --
example-database-integration-usage-example = === Database integration usage example ===
example-database-integration-1-creating-a-cache = 1. Creating a cache instance
example-database-integration-in-memory-cache-created = ✓ in-memory cache created
example-database-integration-2-connection-string-examples = 2. Connection string examples
example-database-integration-oxcache-can-create-a = oxcache can create a Redis cache from a connection string:
example-database-integration-cache-redis-redis-127 = Cache::redis("redis://127.0.0.1:6379")  - standalone mode
example-database-integration-cache-redis-redis-host1 = Cache::redis("redis://host1:6379,host2:6379")  - cluster mode
example-database-integration-see-the-example-redis = see the example_redis_modes example for more
example-database-integration-3-simulating-a-database = 3. Simulating a database loader
example-database-integration-simulated-database-loader-created = ✓ simulated database loader created
example-database-integration-4-implementing-the-cache = 4. Implementing the cache-aside pattern
example-database-integration-trying-to-read-from = trying to read '{ $key }' from the cache
example-database-integration-cache-hit = ✓ cache hit: { $name }
example-database-integration-cache-miss-querying-the = × cache miss, querying the database
example-database-integration-database-query-succeeded = ✓ database query succeeded: { $name }
example-database-integration-result-cached = ✓ result cached
example-database-integration-user-not-found-in = × user not found in the database
example-database-integration-database-query-error = × database query error: { $e }
example-database-integration-5-testing-the-cache = 5. Testing the cache-aside pattern
example-database-integration-first-query-for-user = first query for 'user:1':
example-database-integration-result = result: { $user }
example-database-integration-second-query-for-user = second query for 'user:1':
example-database-integration-result-2 = result: { $user }
example-database-integration-querying-the-nonexistent-user = querying the nonexistent user 'user:999':
example-database-integration-unexpected-found-the-user = unexpected: found the user { $user }
example-database-integration-correct-the-user-does = ✓ correct: the user does not exist
example-database-integration-6-batch-loading-example = 6. Batch loading example
example-database-integration-processing-user = processing user '{ $user_id }':
example-database-integration-cache-hit-2 = ✓ cache hit
example-database-integration-database-load-succeeded = ✓ database load succeeded
example-database-integration-cached = ✓ cached
example-database-integration-absent-from-the-database = × absent from the database
example-database-integration-load-error = × load error: { $e }
example-database-integration-batch-load-results = batch load results:
example-database-integration-not-found = { $i }: not found
example-database-integration-7-cache-updates-and = 7. Cache updates and invalidation
example-database-integration-updating-the-user-record = updating the user record in the cache...
example-database-integration-user-record-updated = ✓ user record updated
example-database-integration-verifying-the-update = verifying the update: { $name }
example-database-integration-8-cache-invalidation-example = 8. Cache invalidation example
example-database-integration-invalidating-the-cache-for = invalidating the cache for 'user:2'...
example-database-integration-cache-invalidated = ✓ cache invalidated
example-database-integration-verifying-invalidation = verifying invalidation...
example-database-integration-reload-succeeded = reload succeeded: { $name }
example-database-integration-9-cache-statistics = 9. Cache statistics
example-database-integration-cache-type = cache type: { $stats }
example-database-integration-entries = entries: { $stats }
example-database-integration-capacity = capacity: { $stats }
example-database-integration-failed-to-get-statistics = failed to get statistics: { $e }
example-database-integration-10-simulating-a-partitioning = 10. Simulating a partitioning strategy
example-database-integration-partition-count = partition count: { $partitions }
example-database-integration-partition-assignment = partition assignment:
example-database-integration-partition = { $key } -> partition { $partition }
example-database-integration-example-complete = === Database integration example complete ===
example-database-integration-key-benefits-of-database = key benefits of database integration:
example-database-integration-cache-aside-cache-misses = - cache-aside: cache misses handled automatically
example-database-integration-less-database-load-higher = - less database load: higher cache hit rates
example-database-integration-data-consistency-prompt-updates = - data consistency: prompt updates and invalidation
example-database-integration-partitioning-supports-large-scale = - partitioning: supports large-scale data distribution
example-database-integration-connection-management-uniform-connection = - connection management: uniform connection-string handling

# -- example_bloom_filter.rs --
example-bloom-filter-bloomfilter-standalone-type = === BloomFilter standalone type ===
example-bloom-filter-creating-bloomfilter-capacity-10000 = Creating BloomFilter (capacity=10000, fpr=0.01)
example-bloom-filter-inserting-existing-key = inserting 'existing_key'
example-bloom-filter-contains-existing-key-should = contains 'existing_key' = { $existing_key } (should be true)
example-bloom-filter-contains-missing-key-should = contains 'missing_key'  = { $missing_key } (should be false)
example-bloom-filter-contains-existing-key-after = contains 'existing_key' after clear = { $existing_key } (should be false)
example-bloom-filter-bloomfilterbackend-decorator = === BloomFilterBackend decorator ===
example-bloom-filter-inner-backend-score = inner backend: { $backend_name } (score={ $score })
example-bloom-filter-creating-bloomfilterbackend-wrapping-mok = Creating BloomFilterBackend (wrapping Moka)
example-bloom-filter-get-user-999-filtered = get 'user:999' = { $value } (filtered by the BF, inner not queried)
example-bloom-filter-ttl-passthrough = === TTL passthrough ===
example-bloom-filter-set-temp-60s-ttl = set 'temp' (60s TTL)
example-bloom-filter-ttl-temp-should-be = ttl('temp') = { $as_secs } (should be about 60s)
example-bloom-filter-example-complete = Bloom Filter example complete!

# -- example_cli_usage.rs --
example-cli-usage-oxcache-cli-usage-example = === oxcache CLI usage example ===
example-cli-usage-cache-created-demo-cache = Cache created: demo_cache
example-cli-usage-performing-cache-operations = --- Performing cache operations ---
example-cli-usage-set-10-cache-entries = Set 10 cache entries
example-cli-usage-retrieved-5-cache-entries = Retrieved 5 cache entries (hits)
example-cli-usage-attempting-to-retrieve-5 = Attempting to retrieve 5 nonexistent entries (misses)
example-cli-usage-simulated-oxcache-status-output = --- Simulated `oxcache status` output ---
example-cli-usage-cache-name-demo-cache = Cache name: demo_cache
example-cli-usage-cache-type-l1-in = Cache type: L1 in-memory cache
example-cli-usage-max-capacity-1000 = Max capacity: 1000
example-cli-usage-current-entries = Current entries: { $await }
example-cli-usage-status-running = Status: running
example-cli-usage-simulated-oxcache-metrics-output = --- Simulated `oxcache metrics` output ---
example-cli-usage-cli-command-format = --- CLI command format ---
example-cli-usage-check-status-oxcache-status = Check status: oxcache status --verbose
example-cli-usage-check-metrics-oxcache-metrics = Check metrics: oxcache metrics --prometheus
example-cli-usage-clean-cache-oxcache-admin = Clean cache: oxcache admin clean --cache demo_cache
example-cli-usage-warm-up-cache-oxcache = Warm up cache: oxcache admin warmup --file warmup_data.json
example-cli-usage-cleaning-up = --- Cleaning up ---
example-cli-usage-cache-cleared = Cache cleared
example-cli-usage-example-complete = Example complete!

# -- example_compression.rs --
example-compression-original-size-bytes = original size: { $data } bytes
example-compression-serialized-bytes = serialized: { $serialized } bytes
example-compression-ratio-x = compression ratio: { $ratio }x
example-compression-serialization-took = serialization took: { $serialize_time }
example-compression-deserialization-took = deserialization took: { $deserialize_time }
example-compression-data-compression-example = === Data compression example ===
example-compression-1-small-data-100 = --- 1. Small data (< 100 bytes) ---
example-compression-2-medium-data-100b = --- 2. Medium data (100B - 1KB) ---
example-compression-3-large-data-1kb = --- 3. Large data (1KB - 100KB) ---
example-compression-4-repeated-data-high = --- 4. Repeated data (high compression ratio) ---
example-compression-5-using-compression-in = --- 5. Using compression in the cache ---
example-compression-storing-compressed-data-bytes = storing compressed data: { $large_bytes } bytes -> { $compressed } bytes
example-compression-reading-and-decompressing-bytes = reading and decompressing: { $stored } bytes -> { $decompressed } bytes
example-compression-data-integrity-verified = ✓ data integrity verified
example-compression-6-smart-compression-strategy = --- 6. Smart compression strategy ---
example-compression-oxcache-picks-the-compression = oxcache picks the compression level automatically by data size:
example-compression-100-bytes-no-compression = - < 100 bytes: no compression (the overhead outweighs the gain)
example-compression-100b-1kb-fast-compression = - 100B - 1KB: fast compression (Compression::fast())
example-compression-1kb-100kb-medium-compression = - 1KB - 100KB: medium compression (Compression::new(6))
example-compression-100kb-high-compression-ratio = - > 100KB: high compression ratio (Compression::best())
example-compression-data-compression-example-complete = === Data compression example complete ===
example-compression-key-points = key points:
example-compression-enable-compression-with-jsonserializer = - enable compression with JsonSerializer::with_compression()
example-compression-small-data-is-left = - small data is left uncompressed, avoiding compression overhead
example-compression-repeated-data-compresses-well = - repeated data compresses well
example-compression-decompression-is-transparent = - compression/decompression is transparent and automatic

# -- example_events.rs --
example-events-oxcache-event-system-example = === oxcache event system example ===
example-events-1-creating-and-publishing = --- 1. Creating and publishing events ---
example-events-2-convenience-methods = --- 2. Convenience methods ---
example-events-3-monitoring-cache-operation = --- 3. Monitoring cache-operation events ---
example-events-set-operation-took = set operation took: { $elapsed }
example-events-4-batch-operation-events = --- 4. Batch-operation events ---
example-events-5-custom-events = --- 5. Custom events ---
example-events-6-all-event-types = --- 6. All event types ---
example-events-event-type = event type: { $event_type }
example-events-example-complete = Example complete!

# -- example_i18n.rs --
example-i18n-internationalization-i18n-example = === Internationalization (i18n) example ===
example-i18n-1-creating-formatters = --- 1. Creating formatters ---
example-i18n-en-us-formatter-created = ✓ en-US formatter created
example-i18n-zh-cn-formatter-created = ✓ zh-CN formatter created
example-i18n-de-de-formatter-created = ✓ de-DE formatter created
example-i18n-unexpected-an-invalid-locale = unexpected: an invalid locale was created successfully
example-i18n-invalid-locale-correctly-errored = ✓ invalid locale correctly errored: { $e }
example-i18n-2-number-formatting = --- 2. Number formatting ---
example-i18n-raw-value = raw value:   { $value }
example-i18n-nan-correctly-errored = ✓ NaN correctly errored: { $e }
example-i18n-3-plural-rules = --- 3. Plural rules ---
example-i18n-4-date-formatting = --- 4. Date formatting ---
example-i18n-expiry-date = expiry date: { $year }-{ $month }-{ $day }
example-i18n-5-cache-key-generation = --- 5. Cache key generation ---
example-i18n-6-key-comparison-collation = --- 6. Key comparison (collation) ---
example-i18n-original-order = original order: { $keys }
example-i18n-en-us-collation = en-US collation: { $sorted_keys }
example-i18n-7-multilingual-cache-statistics = --- 7. Multilingual cache statistics ---
example-i18n-cache-hits = cache hits: { $cache_hit_count }
example-i18n-cache-misses = cache misses: { $cache_miss_count }
example-i18n-example-complete = ✓ i18n example complete

# -- example_kit_integration.rs --
example-kit-integration-1-building-asynckit-oxcachemodule = === 1. Building AsyncKit + OxcacheModule ===
example-kit-integration-registered-oxcachebuildobserver = Registered OxcacheBuildObserver
example-kit-integration-registered-oxcachemodule = Registered OxcacheModule
example-kit-integration-registered-the-countingdecorator-decorat = Registered the CountingDecorator decorator
example-kit-integration-asynckit-build-complete = AsyncKit build complete
example-kit-integration-2-using-the-cache = === 2. Using the cache backend ===
example-kit-integration-get-user-999-should = get 'user:999' = { $miss } (should be None)
example-kit-integration-operation-count-should-be = operation count: { $seqcst } (should be 3: set + get + get)
example-kit-integration-3-three-phase-shutdown = === 3. Three-phase shutdown ===
example-kit-integration-registered-three-phase-shutdown = Registered three-phase shutdown hooks:
example-kit-integration-drainqueue-health-check-liveness = - DrainQueue: health_check liveness probing
example-kit-integration-shutdown-result = shutdown result: { $result }
example-kit-integration-done = === Done ===
example-kit-integration-trait-kit-integration-demo = trait-kit integration demo finished. All three features (observer/decorator/shutdown) have been demonstrated.

# -- example_metrics.rs --
example-metrics-and-observability-example = === Metrics and observability example ===
example-metrics-1-creating-a-cache = --- 1. Creating a cache and running operations ---
example-metrics-performed-20-writes-and = performed 20 writes and 20 reads (15 hits + 5 misses)
example-metrics-2-backend-statistics = --- 2. Backend statistics ---
example-metrics-backend-statistics = backend statistics:
example-metrics-3-enhanced-statistics-cachestats = --- 3. Enhanced statistics (CacheStats) ---
example-metrics-l1-hits = L1 hits: { $l1_hits }
example-metrics-l1-misses = L1 misses: { $l1_misses }
example-metrics-l2-hits = L2 hits: { $l2_hits }
example-metrics-l2-misses = L2 misses: { $l2_misses }
example-metrics-l1-writes = L1 writes: { $l1_sets }
example-metrics-l2-writes = L2 writes: { $l2_sets }
example-metrics-total-operations = total operations: { $total_operations }
example-metrics-l1-entries = L1 entries: { $l1_item_count }
example-metrics-4-hit-rate = --- 4. Hit rate ---
example-metrics-l1-hit-rate = L1 hit rate: { $l1_hit_rate_percent }
example-metrics-l2-hit-rate = L2 hit rate: { $l2_hit_rate_percent }
example-metrics-overall-hit-rate = overall hit rate: { $overall_hit_rate_percent }
example-metrics-5-prometheus-format-export = --- 5. Prometheus format export ---
example-metrics-6-json-format-export = --- 6. JSON format export ---
example-metrics-7-cachestats-instance-method = --- 7. CacheStats instance-method exports ---
example-metrics-cachestats-export-prometheus-first = CacheStats.export_prometheus() (first 200 characters):
example-metrics-cachestats-export-json-first = CacheStats.export_json() (first 200 characters):
example-metrics-example-complete = ✓ Example complete

# -- example_security.rs --
example-security-oxcache-security-features-example = === oxcache security features example ===
example-security-1-sensitive-data-redaction = --- 1. Sensitive data redaction ---
example-security-raw-password = raw password: { $password }
example-security-redacted = redacted:   { $redacted }
example-security-connection-string-redaction = --- Connection string redaction ---
example-security-raw-url = raw URL: { $redis_url }
example-security-redacted-2 = redacted:  { $safe_url }
example-security-sensitive-cache-key-redaction = --- Sensitive cache key redaction ---
example-security-field-value-redaction = --- Field value redaction ---
example-security-2-the-redacted-wrapper = --- 2. The Redacted wrapper ---
example-security-redacted-wrapper = Redacted wrapper: { $secret }
example-security-custom-visible-characters = custom visible characters: { $custom_redacted }
example-security-3-safe-log-handling = --- 3. Safe log handling ---
example-security-raw = raw: { $msg }
example-security-safe = safe: { $sanitized }
example-security-4-cache-operation-safety = --- 4. Cache operation safety ---
example-security-storing-the-user-session = storing the user session: { $user_token }
example-security-data-read-back-bytes = data read back: { $data } bytes
example-security-5-security-best-practices = --- 5. Security best practices ---
example-security-never-log-sensitive-information = ✓ never log sensitive information
example-security-use-the-redaction-module = ✓ use the redaction module to mask sensitive data
example-security-encrypt-sensitive-data-before = ✓ encrypt sensitive data before caching it
example-security-use-secure-connection-string = ✓ use secure connection string formats
example-security-rotate-sensitive-tokens-in = ✓ rotate sensitive tokens in the cache regularly
example-security-keep-cache-key-names = ✓ keep cache key names from leaking information
example-security-6-configuration-safety = --- 6. Configuration safety ---
example-security-unsafe-storing-passwords-in = ❌ unsafe: storing passwords in plaintext in config
example-security-safe-use-environment-variables = ✓ safe: use environment variables or a secret management service
example-security-safe-hide-connection-string = ✓ safe: hide connection string passwords in logs
example-security-log-connecting-to = log: connecting to { $safe_conn_str }
example-security-example-complete = Example complete!

# -- example_security_validation.rs --
example-security-validation-oxcache-security-validation-example = === oxcache security validation example ===
example-security-validation-1-redis-key-validation = --- 1. Redis key validation (validate_redis_key) ---
example-security-validation-valid = ✓ valid: '{ $key }'
example-security-validation-invalid = ✗ invalid: '{ $key }' -> { $e }
example-security-validation-command-injection-character-detection = --- Command injection character detection ---
example-security-validation-passed = ✓ passed: '{ $key }'
example-security-validation-rejected = ✗ rejected: '{ $key }' -> { $e }
example-security-validation-sql-injection-pattern-detection = --- SQL injection pattern detection ---
example-security-validation-passed-2 = ✓ passed: '{ $key }'
example-security-validation-rejected-2 = ✗ rejected: '{ $key }' -> { $e }
example-security-validation-path-traversal-detection = --- Path traversal detection ---
example-security-validation-passed-3 = ✓ passed: '{ $key }'
example-security-validation-rejected-3 = ✗ rejected: '{ $key }' -> { $e }
example-security-validation-control-character-detection = --- Control character detection ---
example-security-validation-passed-4 = ✓ passed: { $key }
example-security-validation-rejected-4 = ✗ rejected: { $key } -> { $e }
example-security-validation-2-lua-script-validation = --- 2. Lua script validation (validate_lua_script) ---
example-security-validation-valid-script-passed-validation = ✓ valid script passed validation
example-security-validation-error = ✗ error: { $e }
example-security-validation-dangerous-command-detection = --- Dangerous command detection ---
example-security-validation-passed-5 = ✓ passed: { $desc }
example-security-validation-rejected-5 = ✗ rejected: { $desc }
example-security-validation-infinite-loop-detection = --- Infinite loop detection ---
example-security-validation-passed-6 = ✓ passed: { $script }
example-security-validation-rejected-6 = ✗ rejected: { $script }
example-security-validation-length-and-key-count = --- Length and key-count limits ---
example-security-validation-oversized-script-passed = ✓ oversized script passed
example-security-validation-rejected-an-oversized-script = ✗ rejected an oversized script (>10KB)
example-security-validation-excess-key-count-passed = ✓ excess key count passed
example-security-validation-rejected-excess-key-count = ✗ rejected excess key count (>100)
example-security-validation-3-scan-pattern-validation = --- 3. SCAN pattern validation (validate_scan_pattern) ---
example-security-validation-valid-2 = ✓ valid: '{ $pattern }'
example-security-validation-invalid-2 = ✗ invalid: '{ $pattern }' -> { $e }
example-security-validation-excess-wildcard-detection = --- Excess wildcard detection ---
example-security-validation-passed-7 = ✓ passed
example-security-validation-rejected-excess-wildcards-10 = ✗ rejected excess wildcards (>10)
example-security-validation-oversized-pattern-detection = --- Oversized pattern detection ---
example-security-validation-passed-8 = ✓ passed
example-security-validation-rejected-an-oversized-pattern = ✗ rejected an oversized pattern (>256 characters)
example-security-validation-4-scan-count-limits = --- 4. SCAN count limits (clamp_scan_count) ---
example-security-validation-raw-clamped = raw: { $count } -> clamped: { $clamped }
example-security-validation-5-safe-logging-log = --- 5. Safe logging (log_cache_key) ---
example-security-validation-key = [{ $level }] { $message } (key: { $key })
example-security-validation-6-in-practice-a = --- 6. In practice: a safe validation flow ---
example-security-validation-validating-a-user-supplied = validating a user-supplied cache key:
example-security-validation-accepted = ✓ accepted: '{ $input }'
example-security-validation-rejected-7 = ✗ rejected: '{ $input }'
example-security-validation-validating-a-user-submitted = validating a user-submitted Lua script:
example-security-validation-accepted-script = ✓ accepted script: { $name }
example-security-validation-rejected-script = ✗ rejected script: { $name }
example-security-validation-example-complete = ✓ Example complete
example-security-validation-best-practices = Security validation best practices:
example-security-validation-1-every-user-supplied = 1. every user-supplied cache key must pass validate_redis_key
example-security-validation-2-every-user-submitted = 2. every user-submitted Lua script must pass validate_lua_script
example-security-validation-3-every-scan-pattern = 3. every SCAN pattern must pass validate_scan_pattern
example-security-validation-4-every-scan-count = 4. every SCAN count must be clamped with clamp_scan_count
example-security-validation-5-use-log-cache = 5. use log_cache_key to redact cache keys automatically in logs
