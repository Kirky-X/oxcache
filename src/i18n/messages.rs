// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Message catalog for locale-aware error and status messages, powered by
//! Fluent (FTL).
//!
//! Translations live in `locales/en/messages.ftl` and
//! `locales/zh/messages.ftl`, embedded at compile time via `include_str!`
//! and parsed into concurrent (`Send + Sync`) [`FluentBundle`] instances on
//! first access. Placeholders use the Fluent syntax `{ $name }` and are
//! resolved at format time by the Fluent engine.
//!
//! # Message IDs vs. FTL keys
//!
//! Message IDs use the dotted form exposed by the public API (e.g.
//! `"error.not_found"`). Fluent identifiers cannot contain dots, so catalog
//! keys are the dashed equivalent (`error-not-found`); the conversion is
//! purely mechanical (`.` and `_` become `-`) and happens inside
//! `lookup`/`t`.
//!
//! # Supported locales
//!
//! - `en` / `en-US` / `en-*` → English (fallback for any English variant)
//! - `zh` / `zh-CN` / `zh-*` → Simplified Chinese
//!
//! Unknown locales fall back to English. A key missing from the requested
//! bundle falls back to the English bundle, then to `None` (callers render
//! the raw message ID) — lookup never panics.
//!
//! # Adding a new message
//!
//! 1. Add a `MSG_ID_*` constant in the **Message IDs** section.
//! 2. Add the English entry in `locales/en/messages.ftl`.
//! 3. Add the Chinese entry in `locales/zh/messages.ftl`.
//! 4. Use `formatter.format_message(MSG_ID_*, &[("key", "value")])` to render.

use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

// === Message ID constants ===

// -- OxCacheError messages --
pub const MSG_ERR_SERIALIZATION: &str = "error.serialization";
pub const MSG_ERR_OPERATION: &str = "error.operation";
pub const MSG_ERR_CONNECTION: &str = "error.connection";
pub const MSG_ERR_NOT_FOUND: &str = "error.not_found";
pub const MSG_ERR_DEGRADED: &str = "error.degraded";
pub const MSG_ERR_L1: &str = "error.l1";
pub const MSG_ERR_L2: &str = "error.l2";
pub const MSG_ERR_NOT_SUPPORTED: &str = "error.not_supported";
pub const MSG_ERR_WAL: &str = "error.wal";
pub const MSG_ERR_DATABASE: &str = "error.database";
pub const MSG_ERR_REDIS: &str = "error.redis";
pub const MSG_ERR_IO: &str = "error.io";
pub const MSG_ERR_BACKEND: &str = "error.backend";
pub const MSG_ERR_TIMEOUT: &str = "error.timeout";
pub const MSG_ERR_SHUTDOWN: &str = "error.shutdown";
pub const MSG_ERR_KEY_TOO_LONG: &str = "error.key_too_long";
pub const MSG_ERR_VALUE_TOO_LARGE: &str = "error.value_too_large";
pub const MSG_ERR_BUFFER_FULL: &str = "error.buffer_full";
pub const MSG_ERR_INVALID_INPUT: &str = "error.invalid_input";
pub const MSG_ERR_INVALID_KEY: &str = "error.invalid_key";
pub const MSG_ERR_LOCK: &str = "error.lock";
pub const MSG_ERR_SERVICE_NOT_FOUND: &str = "error.service_not_found";
pub const MSG_ERR_INTERNAL: &str = "error.internal";

// -- OxCacheConfigError messages --
pub const MSG_CFG_MISSING_FIELD: &str = "config.missing_field";
pub const MSG_CFG_INVALID_VALUE: &str = "config.invalid_value";
pub const MSG_CFG_UNSUPPORTED_BACKEND: &str = "config.unsupported_backend";
pub const MSG_CFG_CONNECTION_FAILED: &str = "config.connection_failed";

// -- I18nError messages --
pub const MSG_I18N_INVALID_LOCALE: &str = "i18n.invalid_locale";
pub const MSG_I18N_INVALID_NUMBER: &str = "i18n.invalid_number";
pub const MSG_I18N_DATE_ERROR: &str = "i18n.date_error";
pub const MSG_I18N_FORMAT_ERROR: &str = "i18n.format_error";

// -- Tracing log messages (rendered as the event message field) --
pub const MSG_LOG_CHAIN_READ_COMPLETED: &str = "log.chain.read_completed";
pub const MSG_LOG_CHAIN_EXPIRE_BACKEND_FAILED: &str = "log.chain.expire_backend_failed";
pub const MSG_LOG_CHAIN_ITER_ENTRIES_KEY_FAILED: &str = "log.chain.iter_entries_key_failed";
pub const MSG_LOG_CHAIN_BACKFILL_SUCCEEDED: &str = "log.chain.backfill_succeeded";
pub const MSG_LOG_CHAIN_BACKFILL_FAILED: &str = "log.chain.backfill_failed";
pub const MSG_LOG_OFFLOAD_LIFECYCLE_EVENT: &str = "log.offload.lifecycle_event";
pub const MSG_LOG_OFFLOAD_TIMEOUT_POLICY_EXCEEDED: &str = "log.offload.timeout_policy_exceeded";
pub const MSG_LOG_DEGRADATION_ENTERED: &str = "log.degradation.entered";
pub const MSG_LOG_DEGRADATION_HALF_OPEN: &str = "log.degradation.half_open_probing";
pub const MSG_LOG_DEGRADATION_RECOVERED: &str = "log.degradation.recovered";
pub const MSG_LOG_CONFERS_RELOAD_REJECTED: &str = "log.confers_config.reload_rejected";
pub const MSG_LOG_BRIDGE_SHUTDOWN_SKIPPED: &str = "log.backend.bridge_shutdown_skipped";
pub const MSG_LOG_DISK_SWEEP_FAILED: &str = "log.disk.sweep_failed";
pub const MSG_LOG_AUDIT_EVENT: &str = "log.audit.event";
pub const MSG_LOG_STALE_HIT_VIA_GET_OR: &str = "log.stale.hit_via_get_or";
pub const MSG_LOG_STALE_REVALIDATION_SCHEDULED: &str = "log.stale.revalidation_scheduled";
pub const MSG_LOG_STALE_HIT_SERVED: &str = "log.stale.hit_served";
pub const MSG_LOG_PUBSUB_RECONNECT_FAILED: &str = "log.pubsub.reconnect_failed";
pub const MSG_LOG_PUBSUB_HANDLER_PANICKED: &str = "log.pubsub.handler_panicked";
pub const MSG_LOG_PUBSUB_PAYLOAD_PARSE_FAILED: &str = "log.pubsub.payload_parse_failed";
pub const MSG_LOG_PUBSUB_MAX_RECONNECT_ATTEMPTS_REACHED: &str =
    "log.pubsub.max_reconnect_attempts_reached";
pub const MSG_LOG_PUBSUB_RECONNECTING: &str = "log.pubsub.reconnecting";
pub const MSG_LOG_PUBSUB_RESUBSCRIBE_FAILED: &str = "log.pubsub.resubscribe_failed";
pub const MSG_LOG_MACRO_PASSTHROUGH: &str = "log.macro.passthrough";
pub const MSG_LOG_CIRCUIT_BREAKER_TRANSITION: &str = "log.circuit_breaker.transition";

// -- Panic / invariant messages (assert!/expect-equivalent texts) --
pub const MSG_PANIC_CHAIN_FRESHNESS_INVARIANT: &str = "panic.chain.parallel_freshest_invariant";
pub const MSG_PANIC_BRIDGE_TEMP_RUNTIME: &str = "panic.bridge.temp_runtime_create_failed";
pub const MSG_PANIC_WARMUP_SEMAPHORE: &str = "panic.warmup.semaphore_not_closed";
pub const MSG_PANIC_AUDIT_WRITER_LOCK: &str = "panic.audit.writer_lock_not_poisoned";
pub const MSG_PANIC_AUDIT_WRITER_RX: &str = "panic.audit.writer_rx_present_on_first_start";
pub const MSG_PANIC_STALE_STATE_PAYLOAD: &str = "panic.stale.state_must_carry_payload";
pub const MSG_PANIC_BLOOM_CAPACITY_POSITIVE: &str = "panic.bloom.capacity_must_be_positive";
pub const MSG_PANIC_BLOOM_HASH_COUNT_POSITIVE: &str = "panic.bloom.hash_count_must_be_positive";
pub const MSG_PANIC_BLOOM_FPR_RANGE: &str = "panic.bloom.fpr_must_be_in_open_interval";
pub const MSG_PANIC_BLOOM_HASH_COUNT_UNREACHABLE: &str = "panic.bloom.hash_count_unreachable";
pub const MSG_PANIC_BLOOM_BACKSOLVE_DRIFT: &str = "panic.bloom.hash_count_backsolve_drift";
pub const MSG_PANIC_BLOOM_SEED_FAILED: &str = "panic.bloom.seed_generation_failed";
pub const MSG_PANIC_CONFIG_VALIDATE_MOKA_MEMORY: &str =
    "panic.config.validate_moka_requires_memory";
pub const MSG_PANIC_CONFIG_VALIDATE_DASHMAP_MEMORY: &str =
    "panic.config.validate_dashmap_requires_memory";
pub const MSG_PANIC_CONFIG_VALIDATE_MOCK_TEST_MEMORY: &str =
    "panic.config.validate_mock_requires_test_with_memory";
pub const MSG_PANIC_CONFIG_VALIDATE_REDIS_FEATURE: &str =
    "panic.config.validate_redis_requires_redis_feature";
pub const MSG_PANIC_CONFIG_VALIDATE_DRAGONFLY_FEATURE: &str =
    "panic.config.validate_dragonfly_requires_dragonfly_feature";
pub const MSG_PANIC_CONFIG_VALIDATE_DISK_FEATURE: &str =
    "panic.config.validate_disk_requires_disk_feature";
pub const MSG_PANIC_CANONICAL_SERIALIZE_VALUE_CANNOT_FAIL: &str =
    "panic.canonical.serialize_value_cannot_fail";

// -- User-visible detail messages (interpolated into the { $detail } slot of
//    the error/config templates above) --
pub const MSG_DETAIL_DISK_REDB_OPEN_FAILED: &str = "detail.disk.redb_open_failed";
pub const MSG_DETAIL_DISK_REDB_CREATE_FAILED: &str = "detail.disk.redb_create_failed";
pub const MSG_DETAIL_DISK_CORRUPT_ENVELOPE: &str = "detail.disk.corrupt_envelope";
pub const MSG_DETAIL_SYNC_ATOMIC_INCREMENT: &str = "detail.not_supported.sync_atomic_increment";
pub const MSG_DETAIL_SYNC_ATOMIC_CAS: &str = "detail.not_supported.sync_atomic_cas";
pub const MSG_DETAIL_SYNC_ATOMIC_SET_IF_ABSENT: &str =
    "detail.not_supported.sync_atomic_set_if_absent";
pub const MSG_DETAIL_ASYNC_ATOMIC_INCREMENT: &str = "detail.not_supported.async_atomic_increment";
pub const MSG_DETAIL_ASYNC_ATOMIC_CAS: &str = "detail.not_supported.async_atomic_cas";
pub const MSG_DETAIL_ASYNC_ATOMIC_SET_IF_ABSENT: &str =
    "detail.not_supported.async_atomic_set_if_absent";
pub const MSG_DETAIL_SYNC_REQUIRES_RUNTIME: &str = "detail.not_supported.sync_requires_runtime";
pub const MSG_DETAIL_SYNC_REQUIRES_MULTI_THREAD: &str =
    "detail.not_supported.sync_requires_multi_thread_runtime";
pub const MSG_DETAIL_CONFIG_ENV_INVALID_VALUE: &str = "detail.config.env_invalid_value";
pub const MSG_DETAIL_CONFIG_ENV_BACKEND_FEATURES: &str =
    "detail.config.env_backend_requires_features";
pub const MSG_DETAIL_CONFIG_ENV_SERIALIZATION_FEATURE: &str =
    "detail.config.env_serialization_requires_feature";
pub const MSG_DETAIL_CONFIG_BACKEND_FEATURES: &str = "detail.config.backend_requires_features";
pub const MSG_DETAIL_CONFIG_CAPACITY_ZERO: &str = "detail.config.capacity_zero";
pub const MSG_DETAIL_CONFIG_CAPACITY_EXCEEDS_USIZE: &str = "detail.config.capacity_exceeds_usize";
pub const MSG_DETAIL_CONFIG_TTL_ZERO: &str = "detail.config.ttl_zero";
pub const MSG_DETAIL_CONFIG_METRICS_FEATURE: &str = "detail.config.metrics_requires_feature";
pub const MSG_DETAIL_CONFIG_SERIALIZATION_FEATURE: &str =
    "detail.config.serialization_requires_feature";
pub const MSG_DETAIL_CONFIG_SERIALIZATION_BINCODE_REQUIRES_FEATURE: &str =
    "detail.config.serialization_bincode_requires_feature";
pub const MSG_DETAIL_CONFIG_SERIALIZATION_POSTCARD_REQUIRES_FEATURE: &str =
    "detail.config.serialization_postcard_requires_feature";
pub const MSG_DETAIL_CONFIG_SERIALIZATION_INVALID_FORMAT: &str =
    "detail.config.serialization_invalid_format";
pub const MSG_DETAIL_CONFIG_CB_THRESHOLD_ZERO: &str =
    "detail.config.circuit_breaker_threshold_zero";
pub const MSG_DETAIL_CONFIG_SERVICE_NAME_EMPTY: &str = "detail.config.service_name_empty";
pub const MSG_DETAIL_CONFIG_POOL_SIZE_ZERO: &str = "detail.config.connection_pool_size_zero";
pub const MSG_DETAIL_ADAPTIVE_TTL_MIN_EXCEEDS_MAX: &str = "detail.adaptive_ttl.min_ttl_exceeds_max";
pub const MSG_DETAIL_ADAPTIVE_TTL_MULTIPLIER_FINITE: &str =
    "detail.adaptive_ttl.multiplier_not_finite_positive";
pub const MSG_DETAIL_ADAPTIVE_TTL_DIVISOR_MIN: &str = "detail.adaptive_ttl.divisor_at_least_one";
pub const MSG_DETAIL_ADAPTIVE_TTL_TRACKED_KEYS_MIN: &str =
    "detail.adaptive_ttl.max_tracked_keys_at_least_one";
pub const MSG_DETAIL_GET_OR_LEADER_NOT_CACHED: &str = "detail.get_or.leader_result_not_cached";
pub const MSG_DETAIL_GET_OR_OPTION_LEADER_NOT_CACHED: &str =
    "detail.get_or_option.leader_result_not_cached";
pub const MSG_DETAIL_WARMUP_TTL_LOOKUP_FAILED: &str = "detail.warmup.ttl_lookup_failed";
pub const MSG_DETAIL_REDIS_TTL_MIN_MILLIS: &str = "detail.redis.ttl_min_millis";
pub const MSG_DETAIL_REDIS_TTL_EXCEEDS_MAX: &str = "detail.redis.ttl_exceeds_max";
pub const MSG_DETAIL_REDIS_CLUSTER_CONNECT_FAILED: &str = "detail.redis.cluster_connect_failed";
pub const MSG_DETAIL_REDIS_CLUSTER_CONNECT_TIMEOUT: &str = "detail.redis.cluster_connect_timeout";
pub const MSG_DETAIL_REDIS_KEY_SQL_INJECTION_PATTERN: &str =
    "detail.redis.key_sql_injection_pattern";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_AUTH_BYPASS: &str =
    "detail.redis.sql_injection_auth_bypass";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_COMMENT: &str = "detail.redis.sql_injection_comment";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_DELETE: &str = "detail.redis.sql_injection_delete";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_DROP: &str = "detail.redis.sql_injection_drop";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_INSERT: &str = "detail.redis.sql_injection_insert";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_QUOTE_OR: &str = "detail.redis.sql_injection_quote_or";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_TAUTOLOGY: &str = "detail.redis.sql_injection_tautology";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_UNION_SELECT: &str =
    "detail.redis.sql_injection_union_select";
pub const MSG_DETAIL_REDIS_SQL_INJECTION_XP_CMDSHELL: &str =
    "detail.redis.sql_injection_xp_cmdshell";
pub const MSG_DETAIL_CONFERS_EXPECTS_U64: &str = "detail.confers.expects_u64";
pub const MSG_DETAIL_CONFERS_VALUE_EXCEEDS_RANGE: &str = "detail.confers.value_exceeds_range";
pub const MSG_DETAIL_CONFERS_EXPECTS_BOOL: &str = "detail.confers.expects_bool";
pub const MSG_DETAIL_CONFERS_EXPECTS_F64: &str = "detail.confers.expects_f64";
pub const MSG_DETAIL_CONFERS_EXPECTS_STRING: &str = "detail.confers.expects_string";
pub const MSG_DETAIL_CONFERS_READ_FAILED: &str = "detail.confers.read_failed";
pub const MSG_DETAIL_BUILDER_NO_BACKEND_REQUIRES_MEMORY: &str =
    "detail.builder.no_backend_requires_memory";
pub const MSG_DETAIL_BUILDER_STALE_TTL_SYNC_CONFLICT: &str =
    "detail.builder.stale_ttl_sync_conflict";
pub const MSG_DETAIL_BUILDER_ADAPTIVE_TTL_SYNC_CONFLICT: &str =
    "detail.builder.adaptive_ttl_sync_conflict";
pub const MSG_DETAIL_BUILDER_ADAPTIVE_TTL_STALE_CONFLICT: &str =
    "detail.builder.adaptive_ttl_stale_conflict";
pub const MSG_DETAIL_CONFIG_BACKEND_REQUIRES_FEATURE: &str =
    "detail.config.backend_requires_feature";
pub const MSG_DETAIL_CONFIG_BACKEND_KIND_REQUIRES_FEATURE: &str =
    "detail.config.backend_kind_requires_feature";
pub const MSG_DETAIL_CONFIG_BACKEND_MOCK_TEST_ONLY: &str = "detail.config.backend_mock_test_only";
pub const MSG_DETAIL_CONFIG_BACKEND_AEROSPIKE_PROGRAMMATIC: &str =
    "detail.config.backend_aerospike_programmatic";
pub const MSG_DETAIL_CONFIG_BACKEND_VALKEY_NO_IMPL: &str = "detail.config.backend_valkey_no_impl";
pub const MSG_DETAIL_CONFIG_BACKEND_CHAIN_NEEDS_BUILDER: &str =
    "detail.config.backend_chain_needs_builder";
pub const MSG_DETAIL_CONFIG_BACKEND_UNKNOWN_KIND: &str = "detail.config.backend_unknown_kind";
pub const MSG_DETAIL_CONFIG_BACKEND_NOT_CONFIG_BUILDABLE: &str =
    "detail.config.backend_not_config_buildable";
pub const MSG_DETAIL_CONFIG_ENV_NOT_UNICODE: &str = "detail.config.env_not_unicode";
pub const MSG_DETAIL_CONFIG_ENV_INVALID_BOOL: &str = "detail.config.env_invalid_bool";
pub const MSG_DETAIL_CONFIG_BACKEND_INVALID_VALUE: &str = "detail.config.backend_invalid_value";
pub const MSG_DETAIL_DISK_OPEN_CREATE_FAILED: &str = "detail.disk.open_create_failed";
pub const MSG_DETAIL_CACHE_MEMORY_REQUIRES_FEATURE: &str = "detail.cache.memory_requires_feature";
pub const MSG_DETAIL_NOT_SUPPORTED_MOKA_SYNC_CURRENT_THREAD: &str =
    "detail.not_supported.moka_sync_current_thread";
pub const MSG_DETAIL_REDIS_UNEXPECTED_INFO_REPLY: &str = "detail.redis.unexpected_info_reply";
pub const MSG_DETAIL_REDIS_KEY_CONTROL_CHARACTER: &str = "detail.redis.key_control_character";
pub const MSG_DETAIL_REDIS_KEY_PATH_TRAVERSAL_PATTERN: &str =
    "detail.redis.key_path_traversal_pattern";
pub const MSG_DETAIL_REDIS_KEY_COMMAND_INJECTION_CHARACTER: &str =
    "detail.redis.key_command_injection_character";
pub const MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_LONG: &str = "detail.redis.scan_pattern_too_long";
pub const MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_MANY_WILDCARDS: &str =
    "detail.redis.scan_pattern_too_many_wildcards";
pub const MSG_DETAIL_PUBSUB_CLIENT_OPEN_FAILED: &str = "detail.pubsub.client_open_failed";
pub const MSG_DETAIL_PUBSUB_CONNECTION_FAILED: &str = "detail.pubsub.connection_failed";
pub const MSG_DETAIL_PUBSUB_PUBLISH_FAILED: &str = "detail.pubsub.publish_failed";
pub const MSG_DETAIL_PUBSUB_CONNECT_FAILED: &str = "detail.pubsub.connect_failed";
pub const MSG_DETAIL_PUBSUB_SUBSCRIBE_FAILED: &str = "detail.pubsub.subscribe_failed";
pub const MSG_DETAIL_PUBSUB_SUBSCRIBE_TASK_ENDED: &str = "detail.pubsub.subscribe_task_ended";
pub const MSG_DETAIL_LUA_SCRIPT_TOO_LONG: &str = "detail.lua.script_exceeds_max_length";
pub const MSG_DETAIL_LUA_SCRIPT_TOO_MANY_KEYS: &str = "detail.lua.script_exceeds_max_key_count";
pub const MSG_DETAIL_LUA_SCRIPT_FORBIDDEN_PATTERN: &str = "detail.lua.script_forbidden_pattern";
pub const MSG_DETAIL_LUA_NESTED_EVAL: &str = "detail.lua.nested_eval";
pub const MSG_DETAIL_LUA_INFINITE_LOOP_PATTERNS: &str = "detail.lua.infinite_loop_patterns";
pub const MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_TRUE: &str = "detail.lua.infinite_loop_while_true";
pub const MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_1: &str = "detail.lua.infinite_loop_while_1";
pub const MSG_DETAIL_LUA_INFINITE_LOOP_REPEAT: &str = "detail.lua.infinite_loop_repeat";
pub const MSG_DETAIL_LUA_INFINITE_LOOP_GOTO: &str = "detail.lua.infinite_loop_goto";
pub const MSG_DETAIL_BACKEND_LAYER_RESTRICTION_L1_ONLY: &str =
    "detail.backend.layer_restriction_l1_only";
pub const MSG_DETAIL_BACKEND_LAYER_RESTRICTION_L2_AND_L3_ONLY: &str =
    "detail.backend.layer_restriction_l2_and_l3_only";
pub const MSG_DETAIL_BACKEND_LAYER_RESTRICTION_ANY: &str = "detail.backend.layer_restriction_any";
pub const MSG_DETAIL_CHAIN_GET_MANY_LENGTH_MISMATCH: &str = "detail.chain.get_many_length_mismatch";
pub const MSG_DETAIL_CHAIN_PARALLEL_FRESHEST_ALL_FAILED: &str =
    "detail.chain.parallel_freshest_all_failed";
pub const MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_READ: &str = "detail.chain.all_backends_failed_read";
pub const MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_RACE_READ: &str =
    "detail.chain.all_backends_failed_race_read";
pub const MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_WRITE: &str =
    "detail.chain.all_backends_failed_write";
pub const MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_DELETE: &str =
    "detail.chain.all_backends_failed_delete";
pub const MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_CLEAR: &str =
    "detail.chain.all_backends_failed_clear";
pub const MSG_DETAIL_CHAIN_SYNC_REQUIRES_SYNC_BACKEND: &str =
    "detail.chain.sync_requires_sync_backend";
pub const MSG_DETAIL_CHAIN_NO_BACKENDS: &str = "detail.chain.no_backends";
pub const MSG_DETAIL_CHAIN_HEALTH_CHECK_TIMED_OUT: &str = "detail.chain.health_check_timed_out";
pub const MSG_DETAIL_CHAIN_HEALTH_CHECK_FAILED: &str = "detail.chain.health_check_failed";
pub const MSG_DETAIL_CHAIN_INCR_NO_ATOMIC_WRITER: &str = "detail.chain.incr_no_atomic_writer";
pub const MSG_DETAIL_CHAIN_CAS_NO_ATOMIC_WRITER: &str = "detail.chain.cas_no_atomic_writer";
pub const MSG_DETAIL_CHAIN_SET_IF_ABSENT_NO_ATOMIC_WRITER: &str =
    "detail.chain.set_if_absent_no_atomic_writer";
pub const MSG_DETAIL_KEY_GENERATOR_PART_CONTAINS_SEGMENT_SEPARATOR: &str =
    "detail.key_generator.part_contains_segment_separator";

// -- Example binary output messages --
pub const MSG_EXAMPLE_INKLOG_BRIDGE_TITLE: &str = "example.inklog_bridge.title";
pub const MSG_EXAMPLE_INKLOG_BRIDGE_OBSERVABILITY: &str = "example.inklog_bridge.observability";
pub const MSG_EXAMPLE_INKLOG_BRIDGE_DROPPED: &str = "example.inklog_bridge.dropped";
pub const MSG_EXAMPLE_INKLOG_BRIDGE_WRITE_FAILURES: &str = "example.inklog_bridge.write_failures";
pub const MSG_EXAMPLE_REDIS_MODES_FEATURE_DISABLED: &str = "example.redis_modes.feature_disabled";
pub const MSG_EXAMPLE_REDIS_MODES_RUN_WITH_REDIS_FEATURE: &str =
    "example.redis_modes.run_with_redis_feature";
pub const MSG_EXAMPLE_REDIS_MODES_RUN_WITH_EXAMPLES_PACKAGE: &str =
    "example.redis_modes.run_with_examples_package";
pub const MSG_EXAMPLE_REDIS_MODES_STANDALONE_CONNECT_FAILED: &str =
    "example.redis_modes.standalone_connect_failed";
pub const MSG_EXAMPLE_REDIS_MODES_STANDALONE_ENV_REQUIRED: &str =
    "example.redis_modes.standalone_env_required";
pub const MSG_EXAMPLE_REDIS_MODES_DONE_STANDALONE_SKIPPED: &str =
    "example.redis_modes.done_standalone_skipped";

// example_basic_operations.rs
pub const MSG_EXAMPLE_BASIC_OPERATIONS_SET_USER: &str = "example.basic_operations.set_user";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_GETTING_USER: &str = "example.basic_operations.getting_user";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_GOT_USER: &str = "example.basic_operations.got_user";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_DELETING_USER: &str =
    "example.basic_operations.deleting_user";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_UPDATING_USER: &str =
    "example.basic_operations.updating_user";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_UPDATED_USER: &str = "example.basic_operations.updated_user";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_EXAMPLE_COMPLETE: &str =
    "example.basic_operations.example_complete";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_SET_STORE_A_VALUE: &str =
    "example.basic_operations.set_store_a_value";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_GET_READ_A_VALUE: &str =
    "example.basic_operations.get_read_a_value";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_DELETE_REMOVE_A_VALUE: &str =
    "example.basic_operations.delete_remove_a_value";
pub const MSG_EXAMPLE_BASIC_OPERATIONS_UPDATE_OVERWRITE_THE_EXISTING: &str =
    "example.basic_operations.update_overwrite_the_existing";

// example_byte_ops.rs
pub const MSG_EXAMPLE_BYTE_OPS_BYTE_LEVEL_OPERATIONS_EXAMPLE: &str =
    "example.byte_ops.byte_level_operations_example";
pub const MSG_EXAMPLE_BYTE_OPS_1_BASIC_BYTE_OPERATIONS: &str =
    "example.byte_ops.1_basic_byte_operations";
pub const MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES: &str = "example.byte_ops.write_bytes";
pub const MSG_EXAMPLE_BYTE_OPS_READ: &str = "example.byte_ops.read";
pub const MSG_EXAMPLE_BYTE_OPS_2_BYTE_OPERATIONS_WITHOUT: &str =
    "example.byte_ops.2_byte_operations_without";
pub const MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_NO_TTL: &str = "example.byte_ops.write_bytes_no_ttl";
pub const MSG_EXAMPLE_BYTE_OPS_READ_BYTES: &str = "example.byte_ops.read_bytes";
pub const MSG_EXAMPLE_BYTE_OPS_3_BINARY_DATA: &str = "example.byte_ops.3_binary_data";
pub const MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_OF_BINARY: &str =
    "example.byte_ops.write_bytes_of_binary";
pub const MSG_EXAMPLE_BYTE_OPS_READ_BYTES_2: &str = "example.byte_ops.read_bytes_2";
pub const MSG_EXAMPLE_BYTE_OPS_CHECKSUM: &str = "example.byte_ops.checksum";
pub const MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MATCH: &str = "example.byte_ops.checksum_match";
pub const MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MISMATCH: &str = "example.byte_ops.checksum_mismatch";
pub const MSG_EXAMPLE_BYTE_OPS_4_CACHE_METADATA_OPERATIONS: &str =
    "example.byte_ops.4_cache_metadata_operations";
pub const MSG_EXAMPLE_BYTE_OPS_ENTRIES: &str = "example.byte_ops.entries";
pub const MSG_EXAMPLE_BYTE_OPS_IS_EMPTY: &str = "example.byte_ops.is_empty";
pub const MSG_EXAMPLE_BYTE_OPS_CAPACITY: &str = "example.byte_ops.capacity";
pub const MSG_EXAMPLE_BYTE_OPS_5_SERIALIZER_ACCESS: &str = "example.byte_ops.5_serializer_access";
pub const MSG_EXAMPLE_BYTE_OPS_SERIALIZER_INSTANCE_OBTAINED_DYN: &str =
    "example.byte_ops.serializer_instance_obtained_dyn";
pub const MSG_EXAMPLE_BYTE_OPS_6_LARGE_DATA_OPERATIONS: &str =
    "example.byte_ops.6_large_data_operations";
pub const MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_2: &str = "example.byte_ops.write_bytes_2";
pub const MSG_EXAMPLE_BYTE_OPS_READ_BYTES_3: &str = "example.byte_ops.read_bytes_3";
pub const MSG_EXAMPLE_BYTE_OPS_AFTER_CLEANUP_ENTRIES_IS: &str =
    "example.byte_ops.after_cleanup_entries_is";
pub const MSG_EXAMPLE_BYTE_OPS_CACHE_CLOSED: &str = "example.byte_ops.cache_closed";
pub const MSG_EXAMPLE_BYTE_OPS_EXAMPLE_COMPLETE: &str = "example.byte_ops.example_complete";

// example_cache_builder.rs
pub const MSG_EXAMPLE_CACHE_BUILDER_CACHEBUILDER_FULL_USAGE_EXAMPLE: &str =
    "example.cache_builder.cachebuilder_full_usage_example";
pub const MSG_EXAMPLE_CACHE_BUILDER_1_BASIC_CONSTRUCTION: &str =
    "example.cache_builder.1_basic_construction";
pub const MSG_EXAMPLE_CACHE_BUILDER_BASIC_CACHE_CREATED: &str =
    "example.cache_builder.basic_cache_created";
pub const MSG_EXAMPLE_CACHE_BUILDER_2_SET_CAPACITY: &str = "example.cache_builder.2_set_capacity";
pub const MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_CAPACITY: &str =
    "example.cache_builder.cache_created_with_capacity";
pub const MSG_EXAMPLE_CACHE_BUILDER_3_SET_TTL: &str = "example.cache_builder.3_set_ttl";
pub const MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_1: &str =
    "example.cache_builder.cache_created_with_1";
pub const MSG_EXAMPLE_CACHE_BUILDER_4_SET_TTI: &str = "example.cache_builder.4_set_tti";
pub const MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_5: &str =
    "example.cache_builder.cache_created_with_5";
pub const MSG_EXAMPLE_CACHE_BUILDER_5_COMBINED_CONFIGURATION: &str =
    "example.cache_builder.5_combined_configuration";
pub const MSG_EXAMPLE_CACHE_BUILDER_COMBINED_CACHE_CREATED_CAPACITY: &str =
    "example.cache_builder.combined_cache_created_capacity";
pub const MSG_EXAMPLE_CACHE_BUILDER_6_USE_A_CUSTOM: &str = "example.cache_builder.6_use_a_custom";
pub const MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_A: &str =
    "example.cache_builder.cache_created_with_a";
pub const MSG_EXAMPLE_CACHE_BUILDER_7_REAL_USAGE: &str = "example.cache_builder.7_real_usage";
pub const MSG_EXAMPLE_CACHE_BUILDER_WRITE: &str = "example.cache_builder.write";
pub const MSG_EXAMPLE_CACHE_BUILDER_READ: &str = "example.cache_builder.read";
pub const MSG_EXAMPLE_CACHE_BUILDER_8_SET_WITH_TTL: &str = "example.cache_builder.8_set_with_ttl";
pub const MSG_EXAMPLE_CACHE_BUILDER_STORING_A_PRODUCT_WITH: &str =
    "example.cache_builder.storing_a_product_with";
pub const MSG_EXAMPLE_CACHE_BUILDER_EXISTS_CHECK: &str = "example.cache_builder.exists_check";
pub const MSG_EXAMPLE_CACHE_BUILDER_9_STATISTICS: &str = "example.cache_builder.9_statistics";
pub const MSG_EXAMPLE_CACHE_BUILDER_CACHE_STATS: &str = "example.cache_builder.cache_stats";
pub const MSG_EXAMPLE_CACHE_BUILDER_10_HEALTH_CHECK: &str = "example.cache_builder.10_health_check";
pub const MSG_EXAMPLE_CACHE_BUILDER_HEALTH_STATUS_OK: &str =
    "example.cache_builder.health_status_ok";
pub const MSG_EXAMPLE_CACHE_BUILDER_EXAMPLE_COMPLETE: &str =
    "example.cache_builder.example_complete";

// example_cache_key.rs
pub const MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE: &str =
    "example.cache_key.cachekey_trait_example";
pub const MSG_EXAMPLE_CACHE_KEY_1_CACHEKEY_FOR_BUILT: &str =
    "example.cache_key.1_cachekey_for_built";
pub const MSG_EXAMPLE_CACHE_KEY_2_CACHEKEY_FOR_CUSTOM: &str =
    "example.cache_key.2_cachekey_for_custom";
pub const MSG_EXAMPLE_CACHE_KEY_3_USING_CACHEKEY_IN: &str = "example.cache_key.3_using_cachekey_in";
pub const MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT: &str = "example.cache_key.store_key_product";
pub const MSG_EXAMPLE_CACHE_KEY_READ: &str = "example.cache_key.read";
pub const MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT_2: &str = "example.cache_key.store_key_product_2";
pub const MSG_EXAMPLE_CACHE_KEY_4_BATCH_OPERATIONS: &str = "example.cache_key.4_batch_operations";
pub const MSG_EXAMPLE_CACHE_KEY_STORE_KEY: &str = "example.cache_key.store_key";
pub const MSG_EXAMPLE_CACHE_KEY_BATCH_READ_RESULTS: &str = "example.cache_key.batch_read_results";
pub const MSG_EXAMPLE_CACHE_KEY_5_KEY_FORMAT_CONSISTENCY: &str =
    "example.cache_key.5_key_format_consistency";
pub const MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_100: &str =
    "example.cache_key.userid_100_userid_100";
pub const MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_200: &str =
    "example.cache_key.userid_100_userid_200";
pub const MSG_EXAMPLE_CACHE_KEY_SAME_VALUE_PRODUCES_SAME: &str =
    "example.cache_key.same_value_produces_same";
pub const MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE_COMPLETE: &str =
    "example.cache_key.cachekey_trait_example_complete";
pub const MSG_EXAMPLE_CACHE_KEY_KEY_POINTS: &str = "example.cache_key.key_points";
pub const MSG_EXAMPLE_CACHE_KEY_BUILT_IN_TYPES_STRING: &str =
    "example.cache_key.built_in_types_string";
pub const MSG_EXAMPLE_CACHE_KEY_CUSTOM_TYPES_CAN_GENERATE: &str =
    "example.cache_key.custom_types_can_generate";
pub const MSG_EXAMPLE_CACHE_KEY_TO_KEY_STRING_SHOULD: &str =
    "example.cache_key.to_key_string_should";
pub const MSG_EXAMPLE_CACHE_KEY_CACHEKEY_CAN_BE_USED: &str =
    "example.cache_key.cachekey_can_be_used";

// example_cached_macro.rs
pub const MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION: &str =
    "example.cached_macro.executing_the_original_function";
pub const MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_2: &str =
    "example.cached_macro.executing_the_original_function_2";
pub const MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_3: &str =
    "example.cached_macro.executing_the_original_function_3";
pub const MSG_EXAMPLE_CACHED_MACRO_USAGE_EXAMPLE: &str = "example.cached_macro.usage_example";
pub const MSG_EXAMPLE_CACHED_MACRO_CACHE_REGISTERED_FOR_USE: &str =
    "example.cached_macro.cache_registered_for_use";
pub const MSG_EXAMPLE_CACHED_MACRO_1_BASIC_CACHING_DEMONSTRATION: &str =
    "example.cached_macro.1_basic_caching_demonstration";
pub const MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_OF_GET: &str =
    "example.cached_macro.first_call_of_get";
pub const MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_RESULT: &str =
    "example.cached_macro.first_call_result";
pub const MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_OF_GET: &str =
    "example.cached_macro.second_call_of_get";
pub const MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_RESULT: &str =
    "example.cached_macro.second_call_result";
pub const MSG_EXAMPLE_CACHED_MACRO_2_CUSTOM_KEY_FORMAT: &str =
    "example.cached_macro.2_custom_key_format";
pub const MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM: &str =
    "example.cached_macro.calling_get_user_custom";
pub const MSG_EXAMPLE_CACHED_MACRO_RESULT: &str = "example.cached_macro.result";
pub const MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM_2: &str =
    "example.cached_macro.calling_get_user_custom_2";
pub const MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT: &str = "example.cached_macro.cached_result";
pub const MSG_EXAMPLE_CACHED_MACRO_3_SHORT_TTL_CACHING: &str =
    "example.cached_macro.3_short_ttl_caching";
pub const MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA: &str =
    "example.cached_macro.calling_get_hot_data";
pub const MSG_EXAMPLE_CACHED_MACRO_RESULT_2: &str = "example.cached_macro.result_2";
pub const MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA_2: &str =
    "example.cached_macro.calling_get_hot_data_2";
pub const MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_2: &str = "example.cached_macro.cached_result_2";
pub const MSG_EXAMPLE_CACHED_MACRO_4_PERFORMANCE_COMPARISON: &str =
    "example.cached_macro.4_performance_comparison";
pub const MSG_EXAMPLE_CACHED_MACRO_CACHE_HITS_ARE_TYPICALLY: &str =
    "example.cached_macro.cache_hits_are_typically";
pub const MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_TOOK: &str = "example.cached_macro.first_call_took";
pub const MSG_EXAMPLE_CACHED_MACRO_CACHE_HIT_TOOK: &str = "example.cached_macro.cache_hit_took";
pub const MSG_EXAMPLE_CACHED_MACRO_SPEEDUP_X: &str = "example.cached_macro.speedup_x";
pub const MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_4: &str =
    "example.cached_macro.executing_the_original_function_4";
pub const MSG_EXAMPLE_CACHED_MACRO_5_MULTI_ARGUMENT_FUNCTION: &str =
    "example.cached_macro.5_multi_argument_function";
pub const MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_WITH: &str =
    "example.cached_macro.calling_get_user_with";
pub const MSG_EXAMPLE_CACHED_MACRO_RESULT_3: &str = "example.cached_macro.result_3";
pub const MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_THE_SAME: &str =
    "example.cached_macro.calling_with_the_same";
pub const MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_3: &str = "example.cached_macro.cached_result_3";
pub const MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_DIFFERENT_ARGUMENTS: &str =
    "example.cached_macro.calling_with_different_arguments";
pub const MSG_EXAMPLE_CACHED_MACRO_DIFFERENT_ARGUMENT_RESULT: &str =
    "example.cached_macro.different_argument_result";
pub const MSG_EXAMPLE_CACHED_MACRO_EXAMPLE_COMPLETE: &str = "example.cached_macro.example_complete";
pub const MSG_EXAMPLE_CACHED_MACRO_MAIN_ADVANTAGES_OF_THE: &str =
    "example.cached_macro.main_advantages_of_the";
pub const MSG_EXAMPLE_CACHED_MACRO_ZERO_BOILERPLATE_CACHING_LOGIC: &str =
    "example.cached_macro.zero_boilerplate_caching_logic";
pub const MSG_EXAMPLE_CACHED_MACRO_TYPE_SAFE_CHECKED_AT: &str =
    "example.cached_macro.type_safe_checked_at";
pub const MSG_EXAMPLE_CACHED_MACRO_FLEXIBLE_CONFIGURATION_SUPPORTS_MULTIPLE: &str =
    "example.cached_macro.flexible_configuration_supports_multiple";
pub const MSG_EXAMPLE_CACHED_MACRO_AUTOMATIC_SERIALIZATION_NO_MANUAL: &str =
    "example.cached_macro.automatic_serialization_no_manual";
pub const MSG_EXAMPLE_CACHED_MACRO_PERFORMANCE_SIGNIFICANTLY_IMPROVES_CACHE: &str =
    "example.cached_macro.performance_significantly_improves_cache";

// example_comprehensive_usage.rs
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_OXCACHE_COMPREHENSIVE_USAGE_EXAMPLE: &str =
    "example.comprehensive_usage.oxcache_comprehensive_usage_example";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_1_CREATE_AN_IN: &str =
    "example.comprehensive_usage.1_create_an_in";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_IN_MEMORY_CACHE_CREATED: &str =
    "example.comprehensive_usage.in_memory_cache_created";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_2_CREATE_A_REDIS: &str =
    "example.comprehensive_usage.2_create_a_redis";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_CACHE_CREATED: &str =
    "example.comprehensive_usage.redis_cache_created";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_UNAVAILABLE_DEGRADED: &str =
    "example.comprehensive_usage.redis_unavailable_degraded";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_3_CREATE_A_TIERED: &str =
    "example.comprehensive_usage.3_create_a_tiered";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_TIERED_CACHE_CREATED: &str =
    "example.comprehensive_usage.tiered_cache_created";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_4_BASIC_CRUD_OPERATIONS: &str =
    "example.comprehensive_usage.4_basic_crud_operations";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_USER: &str =
    "example.comprehensive_usage.adding_user";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_ADDED: &str =
    "example.comprehensive_usage.user_added";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_GETTING_USER: &str =
    "example.comprehensive_usage.getting_user";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_RETRIEVED: &str =
    "example.comprehensive_usage.user_retrieved";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND: &str =
    "example.comprehensive_usage.user_not_found";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_UPDATING_USER: &str =
    "example.comprehensive_usage.updating_user";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_UPDATED: &str =
    "example.comprehensive_usage.user_updated";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_AFTER_THE_UPDATE: &str =
    "example.comprehensive_usage.user_after_the_update";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND_2: &str =
    "example.comprehensive_usage.user_not_found_2";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_USER: &str =
    "example.comprehensive_usage.deleting_user";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_FAILED_TO_DELETE_THE: &str =
    "example.comprehensive_usage.failed_to_delete_the";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_DELETED: &str =
    "example.comprehensive_usage.user_deleted";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_5_BATCH_OPERATIONS: &str =
    "example.comprehensive_usage.5_batch_operations";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_PRODUCTS_IN_BATCH: &str =
    "example.comprehensive_usage.adding_products_in_batch";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_ADD_SUCCEEDED_PRODUCTS: &str =
    "example.comprehensive_usage.batch_add_succeeded_products";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_FETCHING_PRODUCTS_IN_BATCH: &str =
    "example.comprehensive_usage.fetching_products_in_batch";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_FETCH_SUCCEEDED_PRODUCTS: &str =
    "example.comprehensive_usage.batch_fetch_succeeded_products";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_PRODUCTS_IN_BATCH: &str =
    "example.comprehensive_usage.deleting_products_in_batch";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_DELETE_SUCCEEDED: &str =
    "example.comprehensive_usage.batch_delete_succeeded";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_6_TTL_CONTROL: &str =
    "example.comprehensive_usage.6_ttl_control";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_DATA_THAT_EXPIRES: &str =
    "example.comprehensive_usage.adding_data_that_expires";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_IMMEDIATE_GET: &str =
    "example.comprehensive_usage.immediate_get";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_WAITING_4_SECONDS_THEN: &str =
    "example.comprehensive_usage.waiting_4_seconds_then";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_GET_AFTER_4_SECONDS: &str =
    "example.comprehensive_usage.get_after_4_seconds";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_7_PERFORMANCE_TEST: &str =
    "example.comprehensive_usage.7_performance_test";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITING_10000_ENTRIES_CONCURRENTLY: &str =
    "example.comprehensive_usage.writing_10000_entries_concurrently";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITE_TOOK: &str =
    "example.comprehensive_usage.write_took";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_READING_10000_ENTRIES_CONCURRENTLY: &str =
    "example.comprehensive_usage.reading_10000_entries_concurrently";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_READ_TOOK: &str = "example.comprehensive_usage.read_took";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_8_STATISTICS: &str =
    "example.comprehensive_usage.8_statistics";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_CACHE_STATS: &str =
    "example.comprehensive_usage.cache_stats";
pub const MSG_EXAMPLE_COMPREHENSIVE_USAGE_EXAMPLE_RUN_COMPLETE: &str =
    "example.comprehensive_usage.example_run_complete";

// example_get_or.rs
pub const MSG_EXAMPLE_GET_OR_DB_LOADING_USER_FROM: &str = "example.get_or.db_loading_user_from";
pub const MSG_EXAMPLE_GET_OR_SINGLE_FLIGHT: &str = "example.get_or.single_flight";
pub const MSG_EXAMPLE_GET_OR_1_BASIC_USAGE: &str = "example.get_or.1_basic_usage";
pub const MSG_EXAMPLE_GET_OR_FIRST_GET_OR_CALL: &str = "example.get_or.first_get_or_call";
pub const MSG_EXAMPLE_GET_OR_RESULT: &str = "example.get_or.result";
pub const MSG_EXAMPLE_GET_OR_SECOND_GET_OR_CALL: &str = "example.get_or.second_get_or_call";
pub const MSG_EXAMPLE_GET_OR_RESULT_2: &str = "example.get_or.result_2";
pub const MSG_EXAMPLE_GET_OR_2_CONCURRENT_SINGLE_FLIGHT: &str =
    "example.get_or.2_concurrent_single_flight";
pub const MSG_EXAMPLE_GET_OR_REQUEST_STARTED: &str = "example.get_or.request_started";
pub const MSG_EXAMPLE_GET_OR_REQUEST_COMPLETED: &str = "example.get_or.request_completed";
pub const MSG_EXAMPLE_GET_OR_NOTE_ONLY_ONE_DB: &str = "example.get_or.note_only_one_db";
pub const MSG_EXAMPLE_GET_OR_3_COMPARISON_CONCURRENCY_WITHOUT: &str =
    "example.get_or.3_comparison_concurrency_without";
pub const MSG_EXAMPLE_GET_OR_NOTE_WITHOUT_SINGLE_FLIGHT: &str =
    "example.get_or.note_without_single_flight";
pub const MSG_EXAMPLE_GET_OR_EXAMPLE_COMPLETE: &str = "example.get_or.example_complete";

// example_new_api.rs
pub const MSG_EXAMPLE_NEW_API_USAGE_EXAMPLE: &str = "example.new_api.usage_example";
pub const MSG_EXAMPLE_NEW_API_1_IN_MEMORY_CACHE: &str = "example.new_api.1_in_memory_cache";
pub const MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_MEMORY: &str =
    "example.new_api.user_retrieved_from_memory";
pub const MSG_EXAMPLE_NEW_API_2_REDIS_CACHE_L2: &str = "example.new_api.2_redis_cache_l2";
pub const MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_REDIS: &str =
    "example.new_api.user_retrieved_from_redis";
pub const MSG_EXAMPLE_NEW_API_REDIS_UNAVAILABLE_SKIPPING_REDIS: &str =
    "example.new_api.redis_unavailable_skipping_redis";
pub const MSG_EXAMPLE_NEW_API_3_CUSTOM_KEY_TYPE: &str = "example.new_api.3_custom_key_type";
pub const MSG_EXAMPLE_NEW_API_USER_RETRIEVED_WITH_CUSTOM: &str =
    "example.new_api.user_retrieved_with_custom";
pub const MSG_EXAMPLE_NEW_API_4_ADVANCED_CONFIGURATION_USING: &str =
    "example.new_api.4_advanced_configuration_using";
pub const MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_THE: &str =
    "example.new_api.user_retrieved_from_the";
pub const MSG_EXAMPLE_NEW_API_5_TTL_OPERATIONS_USING: &str =
    "example.new_api.5_ttl_operations_using";
pub const MSG_EXAMPLE_NEW_API_SET_USER_7_WITH: &str = "example.new_api.set_user_7_with";
pub const MSG_EXAMPLE_NEW_API_UPDATED_THE_TTL_OF: &str = "example.new_api.updated_the_ttl_of";
pub const MSG_EXAMPLE_NEW_API_6_BATCH_OPERATIONS: &str = "example.new_api.6_batch_operations";
pub const MSG_EXAMPLE_NEW_API_BATCH_SET_USERS: &str = "example.new_api.batch_set_users";
pub const MSG_EXAMPLE_NEW_API_RETRIEVED_USERS_FROM_THE: &str =
    "example.new_api.retrieved_users_from_the";
pub const MSG_EXAMPLE_NEW_API_7_DELETE_OPERATIONS: &str = "example.new_api.7_delete_operations";
pub const MSG_EXAMPLE_NEW_API_USER_8_EXISTS: &str = "example.new_api.user_8_exists";
pub const MSG_EXAMPLE_NEW_API_USER_8_EXISTS_AFTER: &str = "example.new_api.user_8_exists_after";
pub const MSG_EXAMPLE_NEW_API_8_CLEAR_OPERATIONS: &str = "example.new_api.8_clear_operations";
pub const MSG_EXAMPLE_NEW_API_SET_3_USERS_IN: &str = "example.new_api.set_3_users_in";
pub const MSG_EXAMPLE_NEW_API_CLEARED_ALL_CACHE_ENTRIES: &str =
    "example.new_api.cleared_all_cache_entries";
pub const MSG_EXAMPLE_NEW_API_USER_1_EXISTS_AFTER: &str = "example.new_api.user_1_exists_after";
pub const MSG_EXAMPLE_NEW_API_9_CACHE_ASIDE_MODE: &str = "example.new_api.9_cache_aside_mode";
pub const MSG_EXAMPLE_NEW_API_USER_RETRIEVED_VIA_THE: &str =
    "example.new_api.user_retrieved_via_the";
pub const MSG_EXAMPLE_NEW_API_10_SUMMARY: &str = "example.new_api.10_summary";
pub const MSG_EXAMPLE_NEW_API_FEATURES: &str = "example.new_api.features";
pub const MSG_EXAMPLE_NEW_API_TYPE_SAFE_CACHE_INTERFACE: &str =
    "example.new_api.type_safe_cache_interface";
pub const MSG_EXAMPLE_NEW_API_IN_MEMORY_CACHE_CACHE: &str = "example.new_api.in_memory_cache_cache";
pub const MSG_EXAMPLE_NEW_API_REDIS_CACHE_CACHE_REDIS: &str =
    "example.new_api.redis_cache_cache_redis";
pub const MSG_EXAMPLE_NEW_API_CUSTOM_KEY_TYPES_CACHEKEY: &str =
    "example.new_api.custom_key_types_cachekey";
pub const MSG_EXAMPLE_NEW_API_BUILDER_PATTERN_FOR_CONFIGURATION: &str =
    "example.new_api.builder_pattern_for_configuration";
pub const MSG_EXAMPLE_NEW_API_TTL_OPERATIONS_SET_WITH: &str =
    "example.new_api.ttl_operations_set_with";
pub const MSG_EXAMPLE_NEW_API_BATCH_OPERATIONS_SET_MANY: &str =
    "example.new_api.batch_operations_set_many";
pub const MSG_EXAMPLE_NEW_API_DELETE_AND_CLEAR_OPERATIONS: &str =
    "example.new_api.delete_and_clear_operations";
pub const MSG_EXAMPLE_NEW_API_EXISTENCE_CHECK_EXISTS: &str =
    "example.new_api.existence_check_exists";
pub const MSG_EXAMPLE_NEW_API_CACHE_ASIDE_MODE_WITH: &str = "example.new_api.cache_aside_mode_with";
pub const MSG_EXAMPLE_NEW_API_EXAMPLE_COMPLETE: &str = "example.new_api.example_complete";

// example_serialization.rs
pub const MSG_EXAMPLE_SERIALIZATION_OPTIONS_EXAMPLE: &str = "example.serialization.options_example";
pub const MSG_EXAMPLE_SERIALIZATION_1_JSON_SERIALIZATION_DEFAULT: &str =
    "example.serialization.1_json_serialization_default";
pub const MSG_EXAMPLE_SERIALIZATION_HUMAN_READABLE_FORMAT: &str =
    "example.serialization.human_readable_format";
pub const MSG_EXAMPLE_SERIALIZATION_GOOD_FOR_DEBUGGING: &str =
    "example.serialization.good_for_debugging";
pub const MSG_EXAMPLE_SERIALIZATION_SLIGHTLY_LARGER_SIZE: &str =
    "example.serialization.slightly_larger_size";
pub const MSG_EXAMPLE_SERIALIZATION_STORED_USER: &str = "example.serialization.stored_user";
pub const MSG_EXAMPLE_SERIALIZATION_RETRIEVED: &str = "example.serialization.retrieved";
pub const MSG_EXAMPLE_SERIALIZATION_CONFIG: &str = "example.serialization.config";
pub const MSG_EXAMPLE_SERIALIZATION_2_SERIALIZATION_FEATURES: &str =
    "example.serialization.2_serialization_features";
pub const MSG_EXAMPLE_SERIALIZATION_JSON_STANDARD_READABLE_FORMAT: &str =
    "example.serialization.json_standard_readable_format";
pub const MSG_EXAMPLE_SERIALIZATION_BINCODE_COMPACT_BINARY_FORMAT: &str =
    "example.serialization.bincode_compact_binary_format";
pub const MSG_EXAMPLE_SERIALIZATION_SUPPORTS_CUSTOM_SERIALIZERS: &str =
    "example.serialization.supports_custom_serializers";
pub const MSG_EXAMPLE_SERIALIZATION_EXAMPLE_COMPLETE: &str =
    "example.serialization.example_complete";

// example_sync_api.rs
pub const MSG_EXAMPLE_SYNC_API_BASIC_SYNC_OPERATIONS: &str =
    "example.sync_api.basic_sync_operations";
pub const MSG_EXAMPLE_SYNC_API_SET_USER_1: &str = "example.sync_api.set_user_1";
pub const MSG_EXAMPLE_SYNC_API_GET_USER_1: &str = "example.sync_api.get_user_1";
pub const MSG_EXAMPLE_SYNC_API_DELETE_USER_1: &str = "example.sync_api.delete_user_1";
pub const MSG_EXAMPLE_SYNC_API_SET_TEMP_60S_TTL: &str = "example.sync_api.set_temp_60s_ttl";
pub const MSG_EXAMPLE_SYNC_API_GET_TEMP: &str = "example.sync_api.get_temp";
pub const MSG_EXAMPLE_SYNC_API_GET_OR_SYNC_SINGLE: &str = "example.sync_api.get_or_sync_single";
pub const MSG_EXAMPLE_SYNC_API_FIRST_GET_OR_SYNC: &str = "example.sync_api.first_get_or_sync";
pub const MSG_EXAMPLE_SYNC_API_SECOND_GET_OR_SYNC: &str = "example.sync_api.second_get_or_sync";
pub const MSG_EXAMPLE_SYNC_API_MIXING_SYNC_AND_ASYNC: &str =
    "example.sync_api.mixing_sync_and_async";
pub const MSG_EXAMPLE_SYNC_API_CACHE_METADATA_SYNC: &str = "example.sync_api.cache_metadata_sync";
pub const MSG_EXAMPLE_SYNC_API_ENTRIES_LEN_SYNC: &str = "example.sync_api.entries_len_sync";
pub const MSG_EXAMPLE_SYNC_API_CACHE_CLEARED: &str = "example.sync_api.cache_cleared";
pub const MSG_EXAMPLE_SYNC_API_ENTRIES_AFTER_CLEAR: &str = "example.sync_api.entries_after_clear";
pub const MSG_EXAMPLE_SYNC_API_EXAMPLE_COMPLETE: &str = "example.sync_api.example_complete";

// example_batch_write.rs
pub const MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE: &str =
    "example.batch_write.optimization_example";
pub const MSG_EXAMPLE_BATCH_WRITE_1_BATCH_ADD_PRODUCTS: &str =
    "example.batch_write.1_batch_add_products";
pub const MSG_EXAMPLE_BATCH_WRITE_BATCH_ADDING_PRODUCTS: &str =
    "example.batch_write.batch_adding_products";
pub const MSG_EXAMPLE_BATCH_WRITE_BATCH_ADD_COMPLETE_TOOK: &str =
    "example.batch_write.batch_add_complete_took";
pub const MSG_EXAMPLE_BATCH_WRITE_2_BATCH_UPDATE_PRICES: &str =
    "example.batch_write.2_batch_update_prices";
pub const MSG_EXAMPLE_BATCH_WRITE_UPDATING_PRODUCT_PRICES: &str =
    "example.batch_write.updating_product_prices";
pub const MSG_EXAMPLE_BATCH_WRITE_PRODUCT_NEW_PRICE: &str = "example.batch_write.product_new_price";
pub const MSG_EXAMPLE_BATCH_WRITE_3_BATCH_READ_VERIFICATION: &str =
    "example.batch_write.3_batch_read_verification";
pub const MSG_EXAMPLE_BATCH_WRITE_READING_ALL_PRODUCT_INFO: &str =
    "example.batch_write.reading_all_product_info";
pub const MSG_EXAMPLE_BATCH_WRITE_STOCK: &str = "example.batch_write.stock";
pub const MSG_EXAMPLE_BATCH_WRITE_4_USING_SET_MANY: &str = "example.batch_write.4_using_set_many";
pub const MSG_EXAMPLE_BATCH_WRITE_SET_MANY_WROTE_ENTRIES: &str =
    "example.batch_write.set_many_wrote_entries";
pub const MSG_EXAMPLE_BATCH_WRITE_GET_MANY_READ_ENTRIES: &str =
    "example.batch_write.get_many_read_entries";
pub const MSG_EXAMPLE_BATCH_WRITE_DELETE_MANY_DELETED_ENTRIES: &str =
    "example.batch_write.delete_many_deleted_entries";
pub const MSG_EXAMPLE_BATCH_WRITE_ENTRIES_REMAIN_AFTER_DELETE: &str =
    "example.batch_write.entries_remain_after_delete";
pub const MSG_EXAMPLE_BATCH_WRITE_5_BATCH_DELETE_SIMULATING: &str =
    "example.batch_write.5_batch_delete_simulating";
pub const MSG_EXAMPLE_BATCH_WRITE_DELISTED_PRODUCT_IDS: &str =
    "example.batch_write.delisted_product_ids";
pub const MSG_EXAMPLE_BATCH_WRITE_PRODUCTS_DELISTED: &str = "example.batch_write.products_delisted";
pub const MSG_EXAMPLE_BATCH_WRITE_6_CACHE_STATISTICS: &str =
    "example.batch_write.6_cache_statistics";
pub const MSG_EXAMPLE_BATCH_WRITE_CACHE_TYPE: &str = "example.batch_write.cache_type";
pub const MSG_EXAMPLE_BATCH_WRITE_CAPACITY: &str = "example.batch_write.capacity";
pub const MSG_EXAMPLE_BATCH_WRITE_ENTRIES: &str = "example.batch_write.entries";
pub const MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE_2: &str =
    "example.batch_write.optimization_example_2";

// example_cache_promotion.rs
pub const MSG_EXAMPLE_CACHE_PROMOTION_STRATEGY_EXAMPLE: &str =
    "example.cache_promotion.strategy_example";
pub const MSG_EXAMPLE_CACHE_PROMOTION_1_SIMULATING_L1_L2: &str =
    "example.cache_promotion.1_simulating_l1_l2";
pub const MSG_EXAMPLE_CACHE_PROMOTION_L1_MEMORY_CAPACITY_3: &str =
    "example.cache_promotion.l1_memory_capacity_3";
pub const MSG_EXAMPLE_CACHE_PROMOTION_L2_CACHE_CAPACITY_100: &str =
    "example.cache_promotion.l2_cache_capacity_100";
pub const MSG_EXAMPLE_CACHE_PROMOTION_2_INITIALIZE_DATA_INTO: &str =
    "example.cache_promotion.2_initialize_data_into";
pub const MSG_EXAMPLE_CACHE_PROMOTION_WROTE_SESSIONS_INTO_L2: &str =
    "example.cache_promotion.wrote_sessions_into_l2";
pub const MSG_EXAMPLE_CACHE_PROMOTION_3_READS_WITH_PROMOTION: &str =
    "example.cache_promotion.3_reads_with_promotion";
pub const MSG_EXAMPLE_CACHE_PROMOTION_L1_HIT: &str = "example.cache_promotion.l1_hit";
pub const MSG_EXAMPLE_CACHE_PROMOTION_L2_HIT_PROMOTED_TO: &str =
    "example.cache_promotion.l2_hit_promoted_to";
pub const MSG_EXAMPLE_CACHE_PROMOTION_ALL_MISS: &str = "example.cache_promotion.all_miss";
pub const MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_SEQUENCE_ACCESSES: &str =
    "example.cache_promotion.access_sequence_accesses";
pub const MSG_EXAMPLE_CACHE_PROMOTION_L1_FINAL_STATE_ENTRIES: &str =
    "example.cache_promotion.l1_final_state_entries";
pub const MSG_EXAMPLE_CACHE_PROMOTION_4_ACCESS_FREQUENCY_ANALYSIS: &str =
    "example.cache_promotion.4_access_frequency_analysis";
pub const MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_FREQUENCY_RANKING: &str =
    "example.cache_promotion.access_frequency_ranking";
pub const MSG_EXAMPLE_CACHE_PROMOTION_TIMES: &str = "example.cache_promotion.times";
pub const MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_SESS_1: &str =
    "example.cache_promotion.hot_data_sess_1";
pub const MSG_EXAMPLE_CACHE_PROMOTION_5_PRODUCTION_CONFIGURATION_SUGGESTIONS: &str =
    "example.cache_promotion.5_production_configuration_suggestions";
pub const MSG_EXAMPLE_CACHE_PROMOTION_A_REAL_TIERED_CACHE: &str =
    "example.cache_promotion.a_real_tiered_cache";
pub const MSG_EXAMPLE_CACHE_PROMOTION_L1_MOKA_L2_REDIS: &str =
    "example.cache_promotion.l1_moka_l2_redis";
pub const MSG_EXAMPLE_CACHE_PROMOTION_TTL_DURATION_FROM_SECS: &str =
    "example.cache_promotion.ttl_duration_from_secs";
pub const MSG_EXAMPLE_CACHE_PROMOTION_CAPACITY_10_000_L1: &str =
    "example.cache_promotion.capacity_10_000_l1";
pub const MSG_EXAMPLE_CACHE_PROMOTION_PROMOTION_STRATEGY_NOTES: &str =
    "example.cache_promotion.promotion_strategy_notes";
pub const MSG_EXAMPLE_CACHE_PROMOTION_L1_IS_SMALL_BUT: &str =
    "example.cache_promotion.l1_is_small_but";
pub const MSG_EXAMPLE_CACHE_PROMOTION_ON_AN_L2_HIT: &str = "example.cache_promotion.on_an_l2_hit";
pub const MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_IS_PROMOTED: &str =
    "example.cache_promotion.hot_data_is_promoted";
pub const MSG_EXAMPLE_CACHE_PROMOTION_SIZE_L1_SENSIBLY_TO: &str =
    "example.cache_promotion.size_l1_sensibly_to";
pub const MSG_EXAMPLE_CACHE_PROMOTION_6_TTL_EFFECTS_ON: &str =
    "example.cache_promotion.6_ttl_effects_on";
pub const MSG_EXAMPLE_CACHE_PROMOTION_SET_A_SESSION_WITH: &str =
    "example.cache_promotion.set_a_session_with";
pub const MSG_EXAMPLE_CACHE_PROMOTION_REMAINING_TTL: &str = "example.cache_promotion.remaining_ttl";
pub const MSG_EXAMPLE_CACHE_PROMOTION_EXAMPLE_COMPLETE: &str =
    "example.cache_promotion.example_complete";

// example_chain_cache.rs
pub const MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE: &str = "example.chain_cache.example";
pub const MSG_EXAMPLE_CHAIN_CACHE_1_CHAINCACHE_BUILDER_CREATES: &str =
    "example.chain_cache.1_chaincache_builder_creates";
pub const MSG_EXAMPLE_CHAIN_CACHE_CONNECTING_TO_REDIS: &str =
    "example.chain_cache.connecting_to_redis";
pub const MSG_EXAMPLE_CHAIN_CACHE_CREATED_A_2_TIER: &str = "example.chain_cache.created_a_2_tier";
pub const MSG_EXAMPLE_CHAIN_CACHE_CANNOT_CONNECT_TO_REDIS: &str =
    "example.chain_cache.cannot_connect_to_redis";
pub const MSG_EXAMPLE_CHAIN_CACHE_SKIPPING_THE_REDIS_EXAMPLE: &str =
    "example.chain_cache.skipping_the_redis_example";
pub const MSG_EXAMPLE_CHAIN_CACHE_2_IN_MEMORY_ONLY: &str = "example.chain_cache.2_in_memory_only";
pub const MSG_EXAMPLE_CHAIN_CACHE_STORE: &str = "example.chain_cache.store";
pub const MSG_EXAMPLE_CHAIN_CACHE_READ: &str = "example.chain_cache.read";
pub const MSG_EXAMPLE_CHAIN_CACHE_3_BATCH_OPERATIONS_EXAMPLE: &str =
    "example.chain_cache.3_batch_operations_example";
pub const MSG_EXAMPLE_CHAIN_CACHE_BATCH_STORED_USERS: &str =
    "example.chain_cache.batch_stored_users";
pub const MSG_EXAMPLE_CHAIN_CACHE_BATCH_READ_USERS: &str = "example.chain_cache.batch_read_users";
pub const MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE_COMPLETE: &str = "example.chain_cache.example_complete";
pub const MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA: &str = "example.chain_cache.test_data";
pub const MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA: &str = "example.chain_cache.writing_data";
pub const MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE: &str =
    "example.chain_cache.data_written_to_the";
pub const MSG_EXAMPLE_CHAIN_CACHE_READING_DATA: &str = "example.chain_cache.reading_data";
pub const MSG_EXAMPLE_CHAIN_CACHE_HIT: &str = "example.chain_cache.hit";
pub const MSG_EXAMPLE_CHAIN_CACHE_FAILED_TO_PARSE_THE: &str =
    "example.chain_cache.failed_to_parse_the";
pub const MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND: &str = "example.chain_cache.not_found";
pub const MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS: &str = "example.chain_cache.cache_stats";
pub const MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA_2: &str = "example.chain_cache.test_data_2";
pub const MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA_2: &str = "example.chain_cache.writing_data_2";
pub const MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE_2: &str =
    "example.chain_cache.data_written_to_the_2";
pub const MSG_EXAMPLE_CHAIN_CACHE_READING_DATA_2: &str = "example.chain_cache.reading_data_2";
pub const MSG_EXAMPLE_CHAIN_CACHE_HIT_2: &str = "example.chain_cache.hit_2";
pub const MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND_2: &str = "example.chain_cache.not_found_2";
pub const MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS_2: &str = "example.chain_cache.cache_stats_2";

// example_custom_backend.rs
pub const MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_SHUT_DOWN_DATA: &str =
    "example.custom_backend.hashmapbackend_shut_down_data";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE: &str = "example.custom_backend.example";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_1_CREATE_A_CUSTOM: &str =
    "example.custom_backend.1_create_a_custom";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_CREATED_CAPACITY_100: &str =
    "example.custom_backend.hashmapbackend_created_capacity_100";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_2_INJECT_IT_VIA: &str =
    "example.custom_backend.2_inject_it_via";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_BUILT_WITH_THE: &str =
    "example.custom_backend.cache_built_with_the";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_3_BASIC_CRUD_OPERATIONS: &str =
    "example.custom_backend.3_basic_crud_operations";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_4_TTL_OPERATIONS: &str =
    "example.custom_backend.4_ttl_operations";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_SET_WITH_TTL_60S: &str =
    "example.custom_backend.set_with_ttl_60s";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_EXPIRE_UPDATED_TO_120S: &str =
    "example.custom_backend.expire_updated_to_120s";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_5_BATCH_OPERATIONS: &str =
    "example.custom_backend.5_batch_operations";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_BATCH_WROTE_ENTRIES: &str =
    "example.custom_backend.batch_wrote_entries";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_ENTRIES: &str = "example.custom_backend.cache_entries";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_6_STATISTICS: &str = "example.custom_backend.6_statistics";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_7_HEALTH_CHECK: &str = "example.custom_backend.7_health_check";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_BACKEND_HEALTHY: &str =
    "example.custom_backend.backend_healthy";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_HEALTH_CHECK_FAILED: &str =
    "example.custom_backend.health_check_failed";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_8_CAPACITY_LIMIT_DEMONSTRATION: &str =
    "example.custom_backend.8_capacity_limit_demonstration";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_WROTE_3_ENTRIES_CAPACITY: &str =
    "example.custom_backend.wrote_3_entries_capacity";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_UNEXPECTED_THE_WRITE_SUCCEEDED: &str =
    "example.custom_backend.unexpected_the_write_succeeded";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_CAPACITY_FULL_THE_WRITE: &str =
    "example.custom_backend.capacity_full_the_write";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_9_SHUTDOWN: &str = "example.custom_backend.9_shutdown";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_CLOSED: &str = "example.custom_backend.cache_closed";
pub const MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE_COMPLETE: &str =
    "example.custom_backend.example_complete";

// example_dashmap_backend.rs
pub const MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE: &str = "example.dashmap_backend.example";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_1_CREATE_A_DASHMAP: &str =
    "example.dashmap_backend.1_create_a_dashmap";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_CREATED: &str = "example.dashmap_backend.created";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_2_BASIC_OPERATIONS: &str =
    "example.dashmap_backend.2_basic_operations";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_WRITE_KEY1_VALUE1: &str =
    "example.dashmap_backend.write_key1_value1";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_READ_KEY1: &str = "example.dashmap_backend.read_key1";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_KEY1: &str = "example.dashmap_backend.exists_key1";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_3_BATCH_OPERATIONS: &str =
    "example.dashmap_backend.3_batch_operations";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_WRITE_KEY_VALUE: &str =
    "example.dashmap_backend.batch_write_key_value";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_READ_RESULTS: &str =
    "example.dashmap_backend.batch_read_results";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_4_COMPARING_MOKA_AND: &str =
    "example.dashmap_backend.4_comparing_moka_and";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_KEYS: &str =
    "example.dashmap_backend.moka_backend_keys";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_KEYS: &str = "example.dashmap_backend.keys";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_5_DELETE_OPERATIONS: &str =
    "example.dashmap_backend.5_delete_operations";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_DELETE_KEY1: &str = "example.dashmap_backend.delete_key1";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_CHECK_KEY1: &str =
    "example.dashmap_backend.exists_check_key1";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_6_CLEAR_OPERATIONS: &str =
    "example.dashmap_backend.6_clear_operations";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_BEFORE_CLEAR_KEYS: &str =
    "example.dashmap_backend.before_clear_keys";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_AFTER_CLEAR_KEYS: &str =
    "example.dashmap_backend.after_clear_keys";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_7_FEATURE_COMPARISON: &str =
    "example.dashmap_backend.7_feature_comparison";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_TRAITS: &str =
    "example.dashmap_backend.moka_backend_traits";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_LRU_TINYLFU_EVICTION_POLICIES: &str =
    "example.dashmap_backend.lru_tinylfu_eviction_policies";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_CAPACITY_LIMITS: &str =
    "example.dashmap_backend.capacity_limits";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_AUTOMATIC_EXPIRY: &str =
    "example.dashmap_backend.automatic_expiry";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_SUITABLE_FOR_PRODUCTION: &str =
    "example.dashmap_backend.suitable_for_production";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_TRAITS: &str = "example.dashmap_backend.traits";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_PLAIN_CONCURRENT_HASHMAP: &str =
    "example.dashmap_backend.plain_concurrent_hashmap";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_NO_EVICTION_POLICY: &str =
    "example.dashmap_backend.no_eviction_policy";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_MANUAL_TTL_MANAGEMENT: &str =
    "example.dashmap_backend.manual_ttl_management";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_SUITED_TO_SIMPLE_SCENARIOS: &str =
    "example.dashmap_backend.suited_to_simple_scenarios";
pub const MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE_COMPLETE: &str =
    "example.dashmap_backend.example_complete";

// example_error_handling.rs
pub const MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE: &str = "example.error_handling.example";
pub const MSG_EXAMPLE_ERROR_HANDLING_1_ERROR_TYPE_CLASSIFICATION: &str =
    "example.error_handling.1_error_type_classification";
pub const MSG_EXAMPLE_ERROR_HANDLING_2_RECOVERABILITY: &str =
    "example.error_handling.2_recoverability";
pub const MSG_EXAMPLE_ERROR_HANDLING_3_ERROR_CODES_AND: &str =
    "example.error_handling.3_error_codes_and";
pub const MSG_EXAMPLE_ERROR_HANDLING_4_RESILIENT_OPERATIONS_WITH: &str =
    "example.error_handling.4_resilient_operations_with";
pub const MSG_EXAMPLE_ERROR_HANDLING_5_TRIGGERING_RUNTIME_ERRORS: &str =
    "example.error_handling.5_triggering_runtime_errors";
pub const MSG_EXAMPLE_ERROR_HANDLING_6_ERROR_PROPAGATION_WITH: &str =
    "example.error_handling.6_error_propagation_with";
pub const MSG_EXAMPLE_ERROR_HANDLING_7_CONFIGURATION_TIME_ERRORS: &str =
    "example.error_handling.7_configuration_time_errors";
pub const MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE_COMPLETE: &str =
    "example.error_handling.example_complete";
pub const MSG_EXAMPLE_ERROR_HANDLING_CONNECTION_ERROR_RECOVERABLE: &str =
    "example.error_handling.connection_error_recoverable";
pub const MSG_EXAMPLE_ERROR_HANDLING_TIMEOUT_ERROR_RECOVERABLE: &str =
    "example.error_handling.timeout_error_recoverable";
pub const MSG_EXAMPLE_ERROR_HANDLING_BACKEND_ERROR_RECOVERABLE: &str =
    "example.error_handling.backend_error_recoverable";
pub const MSG_EXAMPLE_ERROR_HANDLING_NOT_FOUND_RECOVERABLE: &str =
    "example.error_handling.not_found_recoverable";
pub const MSG_EXAMPLE_ERROR_HANDLING_SERIALIZATION_RECOVERABLE: &str =
    "example.error_handling.serialization_recoverable";
pub const MSG_EXAMPLE_ERROR_HANDLING_DEGRADED_DEGRADED_MODE: &str =
    "example.error_handling.degraded_degraded_mode";
pub const MSG_EXAMPLE_ERROR_HANDLING_INTERNAL_ERROR_RECOVERABLE: &str =
    "example.error_handling.internal_error_recoverable";
pub const MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE: &str = "example.error_handling.error_code";
pub const MSG_EXAMPLE_ERROR_HANDLING_ERROR_MESSAGE: &str = "example.error_handling.error_message";
pub const MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE_2: &str = "example.error_handling.error_code_2";
pub const MSG_EXAMPLE_ERROR_HANDLING_RETRYABLE: &str = "example.error_handling.retryable";
pub const MSG_EXAMPLE_ERROR_HANDLING_SUCCEEDED_ON_RETRY: &str =
    "example.error_handling.succeeded_on_retry";
pub const MSG_EXAMPLE_ERROR_HANDLING_HIT_AN_UNRECOVERABLE_ERROR: &str =
    "example.error_handling.hit_an_unrecoverable_error";
pub const MSG_EXAMPLE_ERROR_HANDLING_FAILED_ON_ATTEMPT_RETRYING: &str =
    "example.error_handling.failed_on_attempt_retrying";
pub const MSG_EXAMPLE_ERROR_HANDLING_REACHED_THE_MAXIMUM_RETRY: &str =
    "example.error_handling.reached_the_maximum_retry";
pub const MSG_EXAMPLE_ERROR_HANDLING_FINAL_RESULT: &str = "example.error_handling.final_result";
pub const MSG_EXAMPLE_ERROR_HANDLING_UNRECOVERABLE_ERROR_RESULT: &str =
    "example.error_handling.unrecoverable_error_result";
pub const MSG_EXAMPLE_ERROR_HANDLING_COL_ERROR_TYPE: &str = "example.error_handling.col_error_type";
pub const MSG_EXAMPLE_ERROR_HANDLING_COL_RECOVERABLE: &str =
    "example.error_handling.col_recoverable";
pub const MSG_EXAMPLE_ERROR_HANDLING_COL_CONNECTION_ERROR: &str =
    "example.error_handling.col_connection_error";
pub const MSG_EXAMPLE_ERROR_HANDLING_RESULT_RETRY_SKIPPED: &str =
    "example.error_handling.result_retry_skipped";
pub const MSG_EXAMPLE_ERROR_HANDLING_RESULT_UNEXPECTED: &str =
    "example.error_handling.result_unexpected";
pub const MSG_EXAMPLE_ERROR_HANDLING_CACHE_OPERATION_SUCCEEDED: &str =
    "example.error_handling.cache_operation_succeeded";
pub const MSG_EXAMPLE_ERROR_HANDLING_NORMAL_READ: &str = "example.error_handling.normal_read";
pub const MSG_EXAMPLE_ERROR_HANDLING_A_MISSING_KEY_RETURNS: &str =
    "example.error_handling.a_missing_key_returns";
pub const MSG_EXAMPLE_ERROR_HANDLING_GET_OR_COMPUTED_RESULT: &str =
    "example.error_handling.get_or_computed_result";
pub const MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK_PASSED: &str =
    "example.error_handling.health_check_passed";
pub const MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK: &str = "example.error_handling.health_check";
pub const MSG_EXAMPLE_ERROR_HANDLING_USER_1_SCORE: &str = "example.error_handling.user_1_score";
pub const MSG_EXAMPLE_ERROR_HANDLING_LOAD_FAILED: &str = "example.error_handling.load_failed";
pub const MSG_EXAMPLE_ERROR_HANDLING_USER_999_SCORE: &str = "example.error_handling.user_999_score";
pub const MSG_EXAMPLE_ERROR_HANDLING_USER_999_LOAD_FAILED: &str =
    "example.error_handling.user_999_load_failed";
pub const MSG_EXAMPLE_ERROR_HANDLING_CONFIG_ERROR_CODE: &str =
    "example.error_handling.config_error_code";

// example_invalidation.rs
pub const MSG_EXAMPLE_INVALIDATION_CACHE_INVALIDATION_STRATEGY_EXAMPLE: &str =
    "example.invalidation.cache_invalidation_strategy_example";
pub const MSG_EXAMPLE_INVALIDATION_1_PREPARE_TEST_DATA: &str =
    "example.invalidation.1_prepare_test_data";
pub const MSG_EXAMPLE_INVALIDATION_ADDED_ORDERS: &str = "example.invalidation.added_orders";
pub const MSG_EXAMPLE_INVALIDATION_2_SINGLE_KEY_INVALIDATION: &str =
    "example.invalidation.2_single_key_invalidation";
pub const MSG_EXAMPLE_INVALIDATION_DELETING_ORDER_1: &str = "example.invalidation.deleting_order_1";
pub const MSG_EXAMPLE_INVALIDATION_THE_ORDER_STILL_EXISTS: &str =
    "example.invalidation.the_order_still_exists";
pub const MSG_EXAMPLE_INVALIDATION_THE_ORDER_WAS_DELETED: &str =
    "example.invalidation.the_order_was_deleted";
pub const MSG_EXAMPLE_INVALIDATION_3_BULK_INVALIDATION_BY: &str =
    "example.invalidation.3_bulk_invalidation_by";
pub const MSG_EXAMPLE_INVALIDATION_ORDER_2_STILL_EXISTS: &str =
    "example.invalidation.order_2_still_exists";
pub const MSG_EXAMPLE_INVALIDATION_ALL_ORDERS_CLEARED: &str =
    "example.invalidation.all_orders_cleared";
pub const MSG_EXAMPLE_INVALIDATION_4_USER_ID_BASED: &str = "example.invalidation.4_user_id_based";
pub const MSG_EXAMPLE_INVALIDATION_ORIGINAL_CART: &str = "example.invalidation.original_cart";
pub const MSG_EXAMPLE_INVALIDATION_USER_100_CHECKS_OUT: &str =
    "example.invalidation.user_100_checks_out";
pub const MSG_EXAMPLE_INVALIDATION_AFTER_CLEAR: &str = "example.invalidation.after_clear";
pub const MSG_EXAMPLE_INVALIDATION_5_TTL_BASED_AUTOMATIC: &str =
    "example.invalidation.5_ttl_based_automatic";
pub const MSG_EXAMPLE_INVALIDATION_ADDING_DATA_THAT_EXPIRES: &str =
    "example.invalidation.adding_data_that_expires";
pub const MSG_EXAMPLE_INVALIDATION_IMMEDIATE_GET: &str = "example.invalidation.immediate_get";
pub const MSG_EXAMPLE_INVALIDATION_WAITING_3_SECONDS: &str =
    "example.invalidation.waiting_3_seconds";
pub const MSG_EXAMPLE_INVALIDATION_GET_AFTER_3_SECONDS: &str =
    "example.invalidation.get_after_3_seconds";
pub const MSG_EXAMPLE_INVALIDATION_6_WRITE_TIME_INVALIDATION: &str =
    "example.invalidation.6_write_time_invalidation";
pub const MSG_EXAMPLE_INVALIDATION_INITIAL_THEME: &str = "example.invalidation.initial_theme";
pub const MSG_EXAMPLE_INVALIDATION_UPDATED_THEME: &str = "example.invalidation.updated_theme";
pub const MSG_EXAMPLE_INVALIDATION_STRATEGY_EXAMPLE_COMPLETE: &str =
    "example.invalidation.strategy_example_complete";

// example_lua_script.rs
pub const MSG_EXAMPLE_LUA_SCRIPT_EXECUTION_EXAMPLE: &str = "example.lua_script.execution_example";
pub const MSG_EXAMPLE_LUA_SCRIPT_CONNECTING_TO_REDIS: &str =
    "example.lua_script.connecting_to_redis";
pub const MSG_EXAMPLE_LUA_SCRIPT_REDIS_CONNECTED: &str = "example.lua_script.redis_connected";
pub const MSG_EXAMPLE_LUA_SCRIPT_1_ATOMIC_COUNTER_EVAL: &str =
    "example.lua_script.1_atomic_counter_eval";
pub const MSG_EXAMPLE_LUA_SCRIPT_CALL_COUNTER_VALUE: &str = "example.lua_script.call_counter_value";
pub const MSG_EXAMPLE_LUA_SCRIPT_2_PRELOADING_SCRIPTS_SCRIPT: &str =
    "example.lua_script.2_preloading_scripts_script";
pub const MSG_EXAMPLE_LUA_SCRIPT_SCRIPT_SHA1: &str = "example.lua_script.script_sha1";
pub const MSG_EXAMPLE_LUA_SCRIPT_3_EXECUTING_SCRIPTS_BY: &str =
    "example.lua_script.3_executing_scripts_by";
pub const MSG_EXAMPLE_LUA_SCRIPT_EVAL_LUA_RESULT: &str = "example.lua_script.eval_lua_result";
pub const MSG_EXAMPLE_LUA_SCRIPT_EVAL_SHA_RESULT: &str = "example.lua_script.eval_sha_result";
pub const MSG_EXAMPLE_LUA_SCRIPT_4_CONDITIONAL_UPDATES_COMPARE: &str =
    "example.lua_script.4_conditional_updates_compare";
pub const MSG_EXAMPLE_LUA_SCRIPT_INITIAL_VALUE_V1: &str = "example.lua_script.initial_value_v1";
pub const MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V2_UPDATING_TO: &str =
    "example.lua_script.expecting_v2_updating_to";
pub const MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V1_UPDATING_TO: &str =
    "example.lua_script.expecting_v1_updating_to";
pub const MSG_EXAMPLE_LUA_SCRIPT_EXAMPLE_COMPLETE_TEST_DATA: &str =
    "example.lua_script.example_complete_test_data";

// example_moka_ttl.rs
pub const MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_NO_GLOBAL: &str =
    "example.moka_ttl.creating_mokamemorybackend_no_global";
pub const MSG_EXAMPLE_MOKA_TTL_SET_K1_60S_PER: &str = "example.moka_ttl.set_k1_60s_per";
pub const MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE: &str = "example.moka_ttl.ttl_k1_should_be";
pub const MSG_EXAMPLE_MOKA_TTL_EXPIRE_UPDATES_TTL: &str = "example.moka_ttl.expire_updates_ttl";
pub const MSG_EXAMPLE_MOKA_TTL_EXPIRE_K1_120S_SHOULD: &str =
    "example.moka_ttl.expire_k1_120s_should";
pub const MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE_2: &str = "example.moka_ttl.ttl_k1_should_be_2";
pub const MSG_EXAMPLE_MOKA_TTL_EXPIRE_MISSING_60S_SHOULD: &str =
    "example.moka_ttl.expire_missing_60s_should";
pub const MSG_EXAMPLE_MOKA_TTL_KEYS_WITHOUT_TTL: &str = "example.moka_ttl.keys_without_ttl";
pub const MSG_EXAMPLE_MOKA_TTL_SET_K2_NO_TTL: &str = "example.moka_ttl.set_k2_no_ttl";
pub const MSG_EXAMPLE_MOKA_TTL_TTL_K2_SHOULD_BE: &str = "example.moka_ttl.ttl_k2_should_be";
pub const MSG_EXAMPLE_MOKA_TTL_TTL_EXPIRY_VERIFICATION: &str =
    "example.moka_ttl.ttl_expiry_verification";
pub const MSG_EXAMPLE_MOKA_TTL_SET_TEMP_50MS_TTL: &str = "example.moka_ttl.set_temp_50ms_ttl";
pub const MSG_EXAMPLE_MOKA_TTL_IMMEDIATE_GET_TEMP: &str = "example.moka_ttl.immediate_get_temp";
pub const MSG_EXAMPLE_MOKA_TTL_GET_TEMP_AFTER_100MS: &str = "example.moka_ttl.get_temp_after_100ms";
pub const MSG_EXAMPLE_MOKA_TTL_GLOBAL_TTL_VS_PER: &str = "example.moka_ttl.global_ttl_vs_per";
pub const MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_GLOBAL_TTL: &str =
    "example.moka_ttl.creating_mokamemorybackend_global_ttl";
pub const MSG_EXAMPLE_MOKA_TTL_SET_A_NO_PER: &str = "example.moka_ttl.set_a_no_per";
pub const MSG_EXAMPLE_MOKA_TTL_SET_B_PER_ENTRY: &str = "example.moka_ttl.set_b_per_entry";
pub const MSG_EXAMPLE_MOKA_TTL_TTL_A_NONE_TTL: &str = "example.moka_ttl.ttl_a_none_ttl";
pub const MSG_EXAMPLE_MOKA_TTL_TTL_B_SHOULD_BE: &str = "example.moka_ttl.ttl_b_should_be";
pub const MSG_EXAMPLE_MOKA_TTL_GET_A_AFTER_450MS: &str = "example.moka_ttl.get_a_after_450ms";
pub const MSG_EXAMPLE_MOKA_TTL_GET_B_AFTER_450MS: &str = "example.moka_ttl.get_b_after_450ms";
pub const MSG_EXAMPLE_MOKA_TTL_MOKA_PER_ENTRY_TTL: &str = "example.moka_ttl.moka_per_entry_ttl";

// example_redis_modes.rs
pub const MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION: &str =
    "example.redis_modes.redis_multi_mode_connection";
pub const MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION_2: &str =
    "example.redis_modes.redis_multi_mode_connection_2";
pub const MSG_EXAMPLE_REDIS_MODES_1_STANDALONE_MODE: &str = "example.redis_modes.1_standalone_mode";
pub const MSG_EXAMPLE_REDIS_MODES_CONNECTED: &str = "example.redis_modes.connected";
pub const MSG_EXAMPLE_REDIS_MODES_CONNECTED_MODE: &str = "example.redis_modes.connected_mode";
pub const MSG_EXAMPLE_REDIS_MODES_WRITE_READ: &str = "example.redis_modes.write_read";
pub const MSG_EXAMPLE_REDIS_MODES_2_EXPLICITLY_SELECTING_STANDALONE: &str =
    "example.redis_modes.2_explicitly_selecting_standalone";
pub const MSG_EXAMPLE_REDIS_MODES_BUILDER_CREATED_MODE: &str =
    "example.redis_modes.builder_created_mode";
pub const MSG_EXAMPLE_REDIS_MODES_3_CLUSTER_MODE: &str = "example.redis_modes.3_cluster_mode";
pub const MSG_EXAMPLE_REDIS_MODES_CONNECTED_2: &str = "example.redis_modes.connected_2";
pub const MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTED_MODE: &str =
    "example.redis_modes.cluster_connected_mode";
pub const MSG_EXAMPLE_REDIS_MODES_WRITE_READ_2: &str = "example.redis_modes.write_read_2";
pub const MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTION_FAILED: &str =
    "example.redis_modes.cluster_connection_failed";
pub const MSG_EXAMPLE_REDIS_MODES_REDIS_CLUSTER_AVAILABLE_IS: &str =
    "example.redis_modes.redis_cluster_available_is";
pub const MSG_EXAMPLE_REDIS_MODES_START_CLUSTER_CD_TESTS: &str =
    "example.redis_modes.start_cluster_cd_tests";
pub const MSG_EXAMPLE_REDIS_MODES_4_SENTINEL_MODE: &str = "example.redis_modes.4_sentinel_mode";
pub const MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION: &str =
    "example.redis_modes.sentinel_connection";
pub const MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTED_MODE: &str =
    "example.redis_modes.sentinel_connected_mode";
pub const MSG_EXAMPLE_REDIS_MODES_WRITE_READ_3: &str = "example.redis_modes.write_read_3";
pub const MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION_FAILED: &str =
    "example.redis_modes.sentinel_connection_failed";
pub const MSG_EXAMPLE_REDIS_MODES_REDIS_SENTINEL_AVAILABLE_IS: &str =
    "example.redis_modes.redis_sentinel_available_is";
pub const MSG_EXAMPLE_REDIS_MODES_START_SENTINEL_CD_TESTS: &str =
    "example.redis_modes.start_sentinel_cd_tests";
pub const MSG_EXAMPLE_REDIS_MODES_EXAMPLE_COMPLETE: &str = "example.redis_modes.example_complete";
pub const MSG_EXAMPLE_REDIS_MODES_5_THE_REDISMODETYPE_ENUM: &str =
    "example.redis_modes.5_the_redismodetype_enum";
pub const MSG_EXAMPLE_REDIS_MODES_MODE_DISPLAY: &str = "example.redis_modes.mode_display";

// example_redis_native.rs
pub const MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE: &str = "example.redis_native.client_example";
pub const MSG_EXAMPLE_REDIS_NATIVE_1_CONNECT_TO_REDIS: &str =
    "example.redis_native.1_connect_to_redis";
pub const MSG_EXAMPLE_REDIS_NATIVE_REDIS_CONNECTED: &str = "example.redis_native.redis_connected";
pub const MSG_EXAMPLE_REDIS_NATIVE_2_BASIC_CRUD_OPERATIONS: &str =
    "example.redis_native.2_basic_crud_operations";
pub const MSG_EXAMPLE_REDIS_NATIVE_ADDING_USER: &str = "example.redis_native.adding_user";
pub const MSG_EXAMPLE_REDIS_NATIVE_USER_ADDED: &str = "example.redis_native.user_added";
pub const MSG_EXAMPLE_REDIS_NATIVE_GETTING_USER: &str = "example.redis_native.getting_user";
pub const MSG_EXAMPLE_REDIS_NATIVE_USER_RETRIEVED: &str = "example.redis_native.user_retrieved";
pub const MSG_EXAMPLE_REDIS_NATIVE_USER_NOT_FOUND: &str = "example.redis_native.user_not_found";
pub const MSG_EXAMPLE_REDIS_NATIVE_UPDATING_USER: &str = "example.redis_native.updating_user";
pub const MSG_EXAMPLE_REDIS_NATIVE_USER_UPDATED: &str = "example.redis_native.user_updated";
pub const MSG_EXAMPLE_REDIS_NATIVE_DELETING_USER: &str = "example.redis_native.deleting_user";
pub const MSG_EXAMPLE_REDIS_NATIVE_FAILED_TO_DELETE_THE: &str =
    "example.redis_native.failed_to_delete_the";
pub const MSG_EXAMPLE_REDIS_NATIVE_USER_DELETED: &str = "example.redis_native.user_deleted";
pub const MSG_EXAMPLE_REDIS_NATIVE_3_BATCH_OPERATIONS: &str =
    "example.redis_native.3_batch_operations";
pub const MSG_EXAMPLE_REDIS_NATIVE_ADDING_USERS_IN_BATCH: &str =
    "example.redis_native.adding_users_in_batch";
pub const MSG_EXAMPLE_REDIS_NATIVE_BATCH_ADD_SUCCEEDED: &str =
    "example.redis_native.batch_add_succeeded";
pub const MSG_EXAMPLE_REDIS_NATIVE_FETCHING_USERS_IN_BATCH: &str =
    "example.redis_native.fetching_users_in_batch";
pub const MSG_EXAMPLE_REDIS_NATIVE_USER: &str = "example.redis_native.user";
pub const MSG_EXAMPLE_REDIS_NATIVE_CLEARING_TEST_DATA: &str =
    "example.redis_native.clearing_test_data";
pub const MSG_EXAMPLE_REDIS_NATIVE_CLEARED: &str = "example.redis_native.cleared";
pub const MSG_EXAMPLE_REDIS_NATIVE_4_CACHE_STATISTICS: &str =
    "example.redis_native.4_cache_statistics";
pub const MSG_EXAMPLE_REDIS_NATIVE_CACHE_STATISTICS_ARE_AVAILABLE: &str =
    "example.redis_native.cache_statistics_are_available";
pub const MSG_EXAMPLE_REDIS_NATIVE_SEE_THE_METRICS_TEST: &str =
    "example.redis_native.see_the_metrics_test";
pub const MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE_2: &str = "example.redis_native.client_example_2";

// example_redis_pipeline.rs
pub const MSG_EXAMPLE_REDIS_PIPELINE_BATCH_OPERATIONS: &str =
    "example.redis_pipeline.batch_operations";
pub const MSG_EXAMPLE_REDIS_PIPELINE_CONNECTING_TO_REDIS: &str =
    "example.redis_pipeline.connecting_to_redis";
pub const MSG_EXAMPLE_REDIS_PIPELINE_REDIS_CONNECTED: &str =
    "example.redis_pipeline.redis_connected";
pub const MSG_EXAMPLE_REDIS_PIPELINE_1_BATCH_SET_SET: &str =
    "example.redis_pipeline.1_batch_set_set";
pub const MSG_EXAMPLE_REDIS_PIPELINE_BATCH_SET_KEYS_TOOK: &str =
    "example.redis_pipeline.batch_set_keys_took";
pub const MSG_EXAMPLE_REDIS_PIPELINE_2_BATCH_GET_GET: &str =
    "example.redis_pipeline.2_batch_get_get";
pub const MSG_EXAMPLE_REDIS_PIPELINE_BATCH_GET_KEYS_TOOK: &str =
    "example.redis_pipeline.batch_get_keys_took";
pub const MSG_EXAMPLE_REDIS_PIPELINE_RESULTS: &str = "example.redis_pipeline.results";
pub const MSG_EXAMPLE_REDIS_PIPELINE_3_PERFORMANCE_COMPARISON_ONE: &str =
    "example.redis_pipeline.3_performance_comparison_one";
pub const MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_ONE: &str =
    "example.redis_pipeline.setting_10_keys_one";
pub const MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_VIA: &str =
    "example.redis_pipeline.setting_10_keys_via";
pub const MSG_EXAMPLE_REDIS_PIPELINE_PIPELINE_IS_X_FASTER: &str =
    "example.redis_pipeline.pipeline_is_x_faster";
pub const MSG_EXAMPLE_REDIS_PIPELINE_4_BATCH_DELETE_DELETE: &str =
    "example.redis_pipeline.4_batch_delete_delete";
pub const MSG_EXAMPLE_REDIS_PIPELINE_BATCH_DELETED_KEYS: &str =
    "example.redis_pipeline.batch_deleted_keys";
pub const MSG_EXAMPLE_REDIS_PIPELINE_REMAINING_KEYS_SHOULD_BE: &str =
    "example.redis_pipeline.remaining_keys_should_be";
pub const MSG_EXAMPLE_REDIS_PIPELINE_EXAMPLE_COMPLETE: &str =
    "example.redis_pipeline.example_complete";

// example_smart_strategy.rs
pub const MSG_EXAMPLE_SMART_STRATEGY_CACHE_STRATEGY_PATTERNS_EXAMPLE: &str =
    "example.smart_strategy.cache_strategy_patterns_example";
pub const MSG_EXAMPLE_SMART_STRATEGY_1_THE_CACHE_ASIDE: &str =
    "example.smart_strategy.1_the_cache_aside";
pub const MSG_EXAMPLE_SMART_STRATEGY_READ_FILL_ON_MISS: &str =
    "example.smart_strategy.read_fill_on_miss";
pub const MSG_EXAMPLE_SMART_STRATEGY_WRITE_INVALIDATE_AFTER_UPDATING: &str =
    "example.smart_strategy.write_invalidate_after_updating";
pub const MSG_EXAMPLE_SMART_STRATEGY_FIRST_READ_OF_1: &str =
    "example.smart_strategy.first_read_of_1";
pub const MSG_EXAMPLE_SMART_STRATEGY_SECOND_READ_OF_1: &str =
    "example.smart_strategy.second_read_of_1";
pub const MSG_EXAMPLE_SMART_STRATEGY_UPDATING_1_AND_INVALIDATING: &str =
    "example.smart_strategy.updating_1_and_invalidating";
pub const MSG_EXAMPLE_SMART_STRATEGY_READ_OF_1_AFTER: &str =
    "example.smart_strategy.read_of_1_after";
pub const MSG_EXAMPLE_SMART_STRATEGY_2_THE_LAZY_LOADING: &str =
    "example.smart_strategy.2_the_lazy_loading";
pub const MSG_EXAMPLE_SMART_STRATEGY_LAZY_COMPUTATION_WITH_GET: &str =
    "example.smart_strategy.lazy_computation_with_get";
pub const MSG_EXAMPLE_SMART_STRATEGY_FIRST_GET_OR_ON: &str =
    "example.smart_strategy.first_get_or_on";
pub const MSG_EXAMPLE_SMART_STRATEGY_SECOND_GET_OR_ON: &str =
    "example.smart_strategy.second_get_or_on";
pub const MSG_EXAMPLE_SMART_STRATEGY_3_TTL_LIFECYCLE_MANAGEMENT: &str =
    "example.smart_strategy.3_ttl_lifecycle_management";
pub const MSG_EXAMPLE_SMART_STRATEGY_DIFFERENT_DATA_TYPES_USE: &str =
    "example.smart_strategy.different_data_types_use";
pub const MSG_EXAMPLE_SMART_STRATEGY_SESSION_DATA_TTL_30S: &str =
    "example.smart_strategy.session_data_ttl_30s";
pub const MSG_EXAMPLE_SMART_STRATEGY_POPULAR_ARTICLES_TTL_1H: &str =
    "example.smart_strategy.popular_articles_ttl_1h";
pub const MSG_EXAMPLE_SMART_STRATEGY_SYSTEM_CONFIG_TTL_24H: &str =
    "example.smart_strategy.system_config_ttl_24h";
pub const MSG_EXAMPLE_SMART_STRATEGY_TERMS_OF_SERVICE_NO: &str =
    "example.smart_strategy.terms_of_service_no";
pub const MSG_EXAMPLE_SMART_STRATEGY_REMAINING_TTL: &str = "example.smart_strategy.remaining_ttl";
pub const MSG_EXAMPLE_SMART_STRATEGY_SESSIONS: &str = "example.smart_strategy.sessions";
pub const MSG_EXAMPLE_SMART_STRATEGY_CONFIG: &str = "example.smart_strategy.config";
pub const MSG_EXAMPLE_SMART_STRATEGY_TERMS: &str = "example.smart_strategy.terms";
pub const MSG_EXAMPLE_SMART_STRATEGY_4_CAPACITY_PLANNING_AND: &str =
    "example.smart_strategy.4_capacity_planning_and";
pub const MSG_EXAMPLE_SMART_STRATEGY_PREWARMED_POPULAR_ARTICLES: &str =
    "example.smart_strategy.prewarmed_popular_articles";
pub const MSG_EXAMPLE_SMART_STRATEGY_SIMULATING_ACCESSES: &str =
    "example.smart_strategy.simulating_accesses";
pub const MSG_EXAMPLE_SMART_STRATEGY_HITS: &str = "example.smart_strategy.hits";
pub const MSG_EXAMPLE_SMART_STRATEGY_MISSES: &str = "example.smart_strategy.misses";
pub const MSG_EXAMPLE_SMART_STRATEGY_5_CACHE_STATISTICS: &str =
    "example.smart_strategy.5_cache_statistics";
pub const MSG_EXAMPLE_SMART_STRATEGY_ENTRIES: &str = "example.smart_strategy.entries";
pub const MSG_EXAMPLE_SMART_STRATEGY_CAPACITY: &str = "example.smart_strategy.capacity";
pub const MSG_EXAMPLE_SMART_STRATEGY_CACHE_STRATEGY_EXAMPLE_COMPLETE: &str =
    "example.smart_strategy.cache_strategy_example_complete";

// example_warmup.rs
pub const MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE: &str = "example.warmup.cache_warmup_example";
pub const MSG_EXAMPLE_WARMUP_1_SIMULATING_APPLICATION_CONFIG: &str =
    "example.warmup.1_simulating_application_config";
pub const MSG_EXAMPLE_WARMUP_LOADING_CONFIG_FROM_THE: &str =
    "example.warmup.loading_config_from_the";
pub const MSG_EXAMPLE_WARMUP_LOADING_CONFIG: &str = "example.warmup.loading_config";
pub const MSG_EXAMPLE_WARMUP_CONFIG_WARMUP_COMPLETE_ITEMS: &str =
    "example.warmup.config_warmup_complete_items";
pub const MSG_EXAMPLE_WARMUP_2_SIMULATING_HOT_USER: &str = "example.warmup.2_simulating_hot_user";
pub const MSG_EXAMPLE_WARMUP_PRELOADING_HOT_USERS: &str = "example.warmup.preloading_hot_users";
pub const MSG_EXAMPLE_WARMUP_HOT_USER_WARMUP_COMPLETE: &str =
    "example.warmup.hot_user_warmup_complete";
pub const MSG_EXAMPLE_WARMUP_3_VERIFYING_THE_WARMED: &str = "example.warmup.3_verifying_the_warmed";
pub const MSG_EXAMPLE_WARMUP_CONFIG_VERIFICATION: &str = "example.warmup.config_verification";
pub const MSG_EXAMPLE_WARMUP_EXPECTED: &str = "example.warmup.expected";
pub const MSG_EXAMPLE_WARMUP_NOT_FOUND: &str = "example.warmup.not_found";
pub const MSG_EXAMPLE_WARMUP_USER_VERIFICATION: &str = "example.warmup.user_verification";
pub const MSG_EXAMPLE_WARMUP_NOT_FOUND_2: &str = "example.warmup.not_found_2";
pub const MSG_EXAMPLE_WARMUP_4_SIMULATING_A_CACHE: &str = "example.warmup.4_simulating_a_cache";
pub const MSG_EXAMPLE_WARMUP_CLEARING_THE_CACHE: &str = "example.warmup.clearing_the_cache";
pub const MSG_EXAMPLE_WARMUP_REWARMING: &str = "example.warmup.rewarming";
pub const MSG_EXAMPLE_WARMUP_CACHE_REBUILD_COMPLETE_TOOK: &str =
    "example.warmup.cache_rebuild_complete_took";
pub const MSG_EXAMPLE_WARMUP_5_POST_WARMUP_STATISTICS: &str =
    "example.warmup.5_post_warmup_statistics";
pub const MSG_EXAMPLE_WARMUP_TOTAL_ENTRIES: &str = "example.warmup.total_entries";
pub const MSG_EXAMPLE_WARMUP_HITS: &str = "example.warmup.hits";
pub const MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE_COMPLETE: &str =
    "example.warmup.cache_warmup_example_complete";

// example_dynamic_config.rs
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE: &str = "example.dynamic_config.example";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_1_INITIALIZING_CONFIG: &str =
    "example.dynamic_config.1_initializing_config";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_2_CONFIG_QUERIES: &str =
    "example.dynamic_config.2_config_queries";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_TYPE: &str = "example.dynamic_config.type";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_NOT_FOUND: &str = "example.dynamic_config.not_found";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_3_CONFIG_UPDATE_DEMONSTRATION: &str =
    "example.dynamic_config.3_config_update_demonstration";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_UPDATING_APP_THEME: &str =
    "example.dynamic_config.updating_app_theme";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_APP_THEME_UPDATED_TO: &str =
    "example.dynamic_config.app_theme_updated_to";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_CURRENT_APP_THEME: &str =
    "example.dynamic_config.current_app_theme";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_4_CONFIG_EXPORT: &str =
    "example.dynamic_config.4_config_export";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_EXPORTING_ALL_CONFIG: &str =
    "example.dynamic_config.exporting_all_config";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_5_CONFIG_GROUPS_BY: &str =
    "example.dynamic_config.5_config_groups_by";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_APPLICATION_CONFIG_ITEMS: &str =
    "example.dynamic_config.application_config_items";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_DATABASE_CONFIG_ITEMS: &str =
    "example.dynamic_config.database_config_items";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_6_CLEANING_UP_TEST: &str =
    "example.dynamic_config.6_cleaning_up_test";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_TEST_DATA_CLEANED_UP: &str =
    "example.dynamic_config.test_data_cleaned_up";
pub const MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE_COMPLETE: &str =
    "example.dynamic_config.example_complete";

// example_key_generator.rs
pub const MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE: &str = "example.key_generator.example";
pub const MSG_EXAMPLE_KEY_GENERATOR_1_BASIC_USAGE: &str = "example.key_generator.1_basic_usage";
pub const MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_NAMESPACE_DEFAULT: &str =
    "example.key_generator.default_namespace_default";
pub const MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_MAX_LENGTH_256: &str =
    "example.key_generator.default_max_length_256";
pub const MSG_EXAMPLE_KEY_GENERATOR_2_USING_NAMESPACES: &str =
    "example.key_generator.2_using_namespaces";
pub const MSG_EXAMPLE_KEY_GENERATOR_USER_KEY: &str = "example.key_generator.user_key";
pub const MSG_EXAMPLE_KEY_GENERATOR_ORDER_KEY: &str = "example.key_generator.order_key";
pub const MSG_EXAMPLE_KEY_GENERATOR_3_USING_PREFIXES: &str =
    "example.key_generator.3_using_prefixes";
pub const MSG_EXAMPLE_KEY_GENERATOR_CACHE_KEY: &str = "example.key_generator.cache_key";
pub const MSG_EXAMPLE_KEY_GENERATOR_SESSION_KEY: &str = "example.key_generator.session_key";
pub const MSG_EXAMPLE_KEY_GENERATOR_4_CHAINING_NAMESPACE_PREFIX: &str =
    "example.key_generator.4_chaining_namespace_prefix";
pub const MSG_EXAMPLE_KEY_GENERATOR_FULL_KEY: &str = "example.key_generator.full_key";
pub const MSG_EXAMPLE_KEY_GENERATOR_5_TEMPLATE_GENERATION: &str =
    "example.key_generator.5_template_generation";
pub const MSG_EXAMPLE_KEY_GENERATOR_GENERATE_USER_ID_PROFILE: &str =
    "example.key_generator.generate_user_id_profile";
pub const MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_SEARCH_TYPE: &str =
    "example.key_generator.generate_full_search_type";
pub const MSG_EXAMPLE_KEY_GENERATOR_6_FULL_KEY_GENERATION: &str =
    "example.key_generator.6_full_key_generation";
pub const MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_USER_ID: &str =
    "example.key_generator.generate_full_user_id";
pub const MSG_EXAMPLE_KEY_GENERATOR_7_KEY_VALIDATION: &str =
    "example.key_generator.7_key_validation";
pub const MSG_EXAMPLE_KEY_GENERATOR_IS_VALID: &str = "example.key_generator.is_valid";
pub const MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID: &str = "example.key_generator.is_invalid";
pub const MSG_EXAMPLE_KEY_GENERATOR_IS_VALID_2: &str = "example.key_generator.is_valid_2";
pub const MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID_2: &str = "example.key_generator.is_invalid_2";
pub const MSG_EXAMPLE_KEY_GENERATOR_8_IN_PRACTICE_CACHE: &str =
    "example.key_generator.8_in_practice_cache";
pub const MSG_EXAMPLE_KEY_GENERATOR_STORE: &str = "example.key_generator.store";
pub const MSG_EXAMPLE_KEY_GENERATOR_STORE_2: &str = "example.key_generator.store_2";
pub const MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE_COMPLETE: &str =
    "example.key_generator.example_complete";

// example_database_integration.rs
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_USAGE_EXAMPLE: &str =
    "example.database_integration.usage_example";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_1_CREATING_A_CACHE: &str =
    "example.database_integration.1_creating_a_cache";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_IN_MEMORY_CACHE_CREATED: &str =
    "example.database_integration.in_memory_cache_created";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_2_CONNECTION_STRING_EXAMPLES: &str =
    "example.database_integration.2_connection_string_examples";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_OXCACHE_CAN_CREATE_A: &str =
    "example.database_integration.oxcache_can_create_a";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_127: &str =
    "example.database_integration.cache_redis_redis_127";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_HOST1: &str =
    "example.database_integration.cache_redis_redis_host1";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_SEE_THE_EXAMPLE_REDIS: &str =
    "example.database_integration.see_the_example_redis";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_3_SIMULATING_A_DATABASE: &str =
    "example.database_integration.3_simulating_a_database";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_SIMULATED_DATABASE_LOADER_CREATED: &str =
    "example.database_integration.simulated_database_loader_created";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_4_IMPLEMENTING_THE_CACHE: &str =
    "example.database_integration.4_implementing_the_cache";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_TRYING_TO_READ_FROM: &str =
    "example.database_integration.trying_to_read_from";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT: &str =
    "example.database_integration.cache_hit";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_MISS_QUERYING_THE: &str =
    "example.database_integration.cache_miss_querying_the";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_SUCCEEDED: &str =
    "example.database_integration.database_query_succeeded";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_CACHED: &str =
    "example.database_integration.result_cached";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_USER_NOT_FOUND_IN: &str =
    "example.database_integration.user_not_found_in";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_ERROR: &str =
    "example.database_integration.database_query_error";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_5_TESTING_THE_CACHE: &str =
    "example.database_integration.5_testing_the_cache";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_FIRST_QUERY_FOR_USER: &str =
    "example.database_integration.first_query_for_user";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT: &str = "example.database_integration.result";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_SECOND_QUERY_FOR_USER: &str =
    "example.database_integration.second_query_for_user";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_2: &str = "example.database_integration.result_2";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_QUERYING_THE_NONEXISTENT_USER: &str =
    "example.database_integration.querying_the_nonexistent_user";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_UNEXPECTED_FOUND_THE_USER: &str =
    "example.database_integration.unexpected_found_the_user";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CORRECT_THE_USER_DOES: &str =
    "example.database_integration.correct_the_user_does";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_6_BATCH_LOADING_EXAMPLE: &str =
    "example.database_integration.6_batch_loading_example";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_PROCESSING_USER: &str =
    "example.database_integration.processing_user";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT_2: &str =
    "example.database_integration.cache_hit_2";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_LOAD_SUCCEEDED: &str =
    "example.database_integration.database_load_succeeded";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHED: &str = "example.database_integration.cached";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_ABSENT_FROM_THE_DATABASE: &str =
    "example.database_integration.absent_from_the_database";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_LOAD_ERROR: &str =
    "example.database_integration.load_error";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_BATCH_LOAD_RESULTS: &str =
    "example.database_integration.batch_load_results";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_NOT_FOUND: &str =
    "example.database_integration.not_found";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_7_CACHE_UPDATES_AND: &str =
    "example.database_integration.7_cache_updates_and";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_UPDATING_THE_USER_RECORD: &str =
    "example.database_integration.updating_the_user_record";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_USER_RECORD_UPDATED: &str =
    "example.database_integration.user_record_updated";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_THE_UPDATE: &str =
    "example.database_integration.verifying_the_update";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_8_CACHE_INVALIDATION_EXAMPLE: &str =
    "example.database_integration.8_cache_invalidation_example";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_INVALIDATING_THE_CACHE_FOR: &str =
    "example.database_integration.invalidating_the_cache_for";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_INVALIDATED: &str =
    "example.database_integration.cache_invalidated";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_INVALIDATION: &str =
    "example.database_integration.verifying_invalidation";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_RELOAD_SUCCEEDED: &str =
    "example.database_integration.reload_succeeded";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_9_CACHE_STATISTICS: &str =
    "example.database_integration.9_cache_statistics";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_TYPE: &str =
    "example.database_integration.cache_type";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_ENTRIES: &str = "example.database_integration.entries";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CAPACITY: &str = "example.database_integration.capacity";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_FAILED_TO_GET_STATISTICS: &str =
    "example.database_integration.failed_to_get_statistics";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_10_SIMULATING_A_PARTITIONING: &str =
    "example.database_integration.10_simulating_a_partitioning";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_COUNT: &str =
    "example.database_integration.partition_count";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_ASSIGNMENT: &str =
    "example.database_integration.partition_assignment";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION: &str =
    "example.database_integration.partition";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_EXAMPLE_COMPLETE: &str =
    "example.database_integration.example_complete";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_KEY_BENEFITS_OF_DATABASE: &str =
    "example.database_integration.key_benefits_of_database";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_ASIDE_CACHE_MISSES: &str =
    "example.database_integration.cache_aside_cache_misses";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_LESS_DATABASE_LOAD_HIGHER: &str =
    "example.database_integration.less_database_load_higher";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_DATA_CONSISTENCY_PROMPT_UPDATES: &str =
    "example.database_integration.data_consistency_prompt_updates";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITIONING_SUPPORTS_LARGE_SCALE: &str =
    "example.database_integration.partitioning_supports_large_scale";
pub const MSG_EXAMPLE_DATABASE_INTEGRATION_CONNECTION_MANAGEMENT_UNIFORM_CONNECTION: &str =
    "example.database_integration.connection_management_uniform_connection";

// example_bloom_filter.rs
pub const MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTER_STANDALONE_TYPE: &str =
    "example.bloom_filter.bloomfilter_standalone_type";
pub const MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTER_CAPACITY_10000: &str =
    "example.bloom_filter.creating_bloomfilter_capacity_10000";
pub const MSG_EXAMPLE_BLOOM_FILTER_INSERTING_EXISTING_KEY: &str =
    "example.bloom_filter.inserting_existing_key";
pub const MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_SHOULD: &str =
    "example.bloom_filter.contains_existing_key_should";
pub const MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_MISSING_KEY_SHOULD: &str =
    "example.bloom_filter.contains_missing_key_should";
pub const MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_AFTER: &str =
    "example.bloom_filter.contains_existing_key_after";
pub const MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTERBACKEND_DECORATOR: &str =
    "example.bloom_filter.bloomfilterbackend_decorator";
pub const MSG_EXAMPLE_BLOOM_FILTER_INNER_BACKEND_SCORE: &str =
    "example.bloom_filter.inner_backend_score";
pub const MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTERBACKEND_WRAPPING_MOK: &str =
    "example.bloom_filter.creating_bloomfilterbackend_wrapping_mok";
pub const MSG_EXAMPLE_BLOOM_FILTER_GET_USER_999_FILTERED: &str =
    "example.bloom_filter.get_user_999_filtered";
pub const MSG_EXAMPLE_BLOOM_FILTER_TTL_PASSTHROUGH: &str = "example.bloom_filter.ttl_passthrough";
pub const MSG_EXAMPLE_BLOOM_FILTER_SET_TEMP_60S_TTL: &str = "example.bloom_filter.set_temp_60s_ttl";
pub const MSG_EXAMPLE_BLOOM_FILTER_TTL_TEMP_SHOULD_BE: &str =
    "example.bloom_filter.ttl_temp_should_be";
pub const MSG_EXAMPLE_BLOOM_FILTER_EXAMPLE_COMPLETE: &str = "example.bloom_filter.example_complete";

// example_cli_usage.rs
pub const MSG_EXAMPLE_CLI_USAGE_OXCACHE_CLI_USAGE_EXAMPLE: &str =
    "example.cli_usage.oxcache_cli_usage_example";
pub const MSG_EXAMPLE_CLI_USAGE_CACHE_CREATED_DEMO_CACHE: &str =
    "example.cli_usage.cache_created_demo_cache";
pub const MSG_EXAMPLE_CLI_USAGE_PERFORMING_CACHE_OPERATIONS: &str =
    "example.cli_usage.performing_cache_operations";
pub const MSG_EXAMPLE_CLI_USAGE_SET_10_CACHE_ENTRIES: &str =
    "example.cli_usage.set_10_cache_entries";
pub const MSG_EXAMPLE_CLI_USAGE_RETRIEVED_5_CACHE_ENTRIES: &str =
    "example.cli_usage.retrieved_5_cache_entries";
pub const MSG_EXAMPLE_CLI_USAGE_ATTEMPTING_TO_RETRIEVE_5: &str =
    "example.cli_usage.attempting_to_retrieve_5";
pub const MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_STATUS_OUTPUT: &str =
    "example.cli_usage.simulated_oxcache_status_output";
pub const MSG_EXAMPLE_CLI_USAGE_CACHE_NAME_DEMO_CACHE: &str =
    "example.cli_usage.cache_name_demo_cache";
pub const MSG_EXAMPLE_CLI_USAGE_CACHE_TYPE_L1_IN: &str = "example.cli_usage.cache_type_l1_in";
pub const MSG_EXAMPLE_CLI_USAGE_MAX_CAPACITY_1000: &str = "example.cli_usage.max_capacity_1000";
pub const MSG_EXAMPLE_CLI_USAGE_CURRENT_ENTRIES: &str = "example.cli_usage.current_entries";
pub const MSG_EXAMPLE_CLI_USAGE_STATUS_RUNNING: &str = "example.cli_usage.status_running";
pub const MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_METRICS_OUTPUT: &str =
    "example.cli_usage.simulated_oxcache_metrics_output";
pub const MSG_EXAMPLE_CLI_USAGE_CLI_COMMAND_FORMAT: &str = "example.cli_usage.cli_command_format";
pub const MSG_EXAMPLE_CLI_USAGE_CHECK_STATUS_OXCACHE_STATUS: &str =
    "example.cli_usage.check_status_oxcache_status";
pub const MSG_EXAMPLE_CLI_USAGE_CHECK_METRICS_OXCACHE_METRICS: &str =
    "example.cli_usage.check_metrics_oxcache_metrics";
pub const MSG_EXAMPLE_CLI_USAGE_CLEAN_CACHE_OXCACHE_ADMIN: &str =
    "example.cli_usage.clean_cache_oxcache_admin";
pub const MSG_EXAMPLE_CLI_USAGE_WARM_UP_CACHE_OXCACHE: &str =
    "example.cli_usage.warm_up_cache_oxcache";
pub const MSG_EXAMPLE_CLI_USAGE_CLEANING_UP: &str = "example.cli_usage.cleaning_up";
pub const MSG_EXAMPLE_CLI_USAGE_CACHE_CLEARED: &str = "example.cli_usage.cache_cleared";
pub const MSG_EXAMPLE_CLI_USAGE_EXAMPLE_COMPLETE: &str = "example.cli_usage.example_complete";

// example_compression.rs
pub const MSG_EXAMPLE_COMPRESSION_ORIGINAL_SIZE_BYTES: &str =
    "example.compression.original_size_bytes";
pub const MSG_EXAMPLE_COMPRESSION_SERIALIZED_BYTES: &str = "example.compression.serialized_bytes";
pub const MSG_EXAMPLE_COMPRESSION_RATIO_X: &str = "example.compression.ratio_x";
pub const MSG_EXAMPLE_COMPRESSION_SERIALIZATION_TOOK: &str =
    "example.compression.serialization_took";
pub const MSG_EXAMPLE_COMPRESSION_DESERIALIZATION_TOOK: &str =
    "example.compression.deserialization_took";
pub const MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE: &str =
    "example.compression.data_compression_example";
pub const MSG_EXAMPLE_COMPRESSION_1_SMALL_DATA_100: &str = "example.compression.1_small_data_100";
pub const MSG_EXAMPLE_COMPRESSION_2_MEDIUM_DATA_100B: &str =
    "example.compression.2_medium_data_100b";
pub const MSG_EXAMPLE_COMPRESSION_3_LARGE_DATA_1KB: &str = "example.compression.3_large_data_1kb";
pub const MSG_EXAMPLE_COMPRESSION_4_REPEATED_DATA_HIGH: &str =
    "example.compression.4_repeated_data_high";
pub const MSG_EXAMPLE_COMPRESSION_5_USING_COMPRESSION_IN: &str =
    "example.compression.5_using_compression_in";
pub const MSG_EXAMPLE_COMPRESSION_STORING_COMPRESSED_DATA_BYTES: &str =
    "example.compression.storing_compressed_data_bytes";
pub const MSG_EXAMPLE_COMPRESSION_READING_AND_DECOMPRESSING_BYTES: &str =
    "example.compression.reading_and_decompressing_bytes";
pub const MSG_EXAMPLE_COMPRESSION_DATA_INTEGRITY_VERIFIED: &str =
    "example.compression.data_integrity_verified";
pub const MSG_EXAMPLE_COMPRESSION_6_SMART_COMPRESSION_STRATEGY: &str =
    "example.compression.6_smart_compression_strategy";
pub const MSG_EXAMPLE_COMPRESSION_OXCACHE_PICKS_THE_COMPRESSION: &str =
    "example.compression.oxcache_picks_the_compression";
pub const MSG_EXAMPLE_COMPRESSION_100_BYTES_NO_COMPRESSION: &str =
    "example.compression.100_bytes_no_compression";
pub const MSG_EXAMPLE_COMPRESSION_100B_1KB_FAST_COMPRESSION: &str =
    "example.compression.100b_1kb_fast_compression";
pub const MSG_EXAMPLE_COMPRESSION_1KB_100KB_MEDIUM_COMPRESSION: &str =
    "example.compression.1kb_100kb_medium_compression";
pub const MSG_EXAMPLE_COMPRESSION_100KB_HIGH_COMPRESSION_RATIO: &str =
    "example.compression.100kb_high_compression_ratio";
pub const MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE_COMPLETE: &str =
    "example.compression.data_compression_example_complete";
pub const MSG_EXAMPLE_COMPRESSION_KEY_POINTS: &str = "example.compression.key_points";
pub const MSG_EXAMPLE_COMPRESSION_ENABLE_COMPRESSION_WITH_JSONSERIALIZER: &str =
    "example.compression.enable_compression_with_jsonserializer";
pub const MSG_EXAMPLE_COMPRESSION_SMALL_DATA_IS_LEFT: &str =
    "example.compression.small_data_is_left";
pub const MSG_EXAMPLE_COMPRESSION_REPEATED_DATA_COMPRESSES_WELL: &str =
    "example.compression.repeated_data_compresses_well";
pub const MSG_EXAMPLE_COMPRESSION_DECOMPRESSION_IS_TRANSPARENT: &str =
    "example.compression.decompression_is_transparent";

// example_events.rs
pub const MSG_EXAMPLE_EVENTS_OXCACHE_EVENT_SYSTEM_EXAMPLE: &str =
    "example.events.oxcache_event_system_example";
pub const MSG_EXAMPLE_EVENTS_1_CREATING_AND_PUBLISHING: &str =
    "example.events.1_creating_and_publishing";
pub const MSG_EXAMPLE_EVENTS_2_CONVENIENCE_METHODS: &str = "example.events.2_convenience_methods";
pub const MSG_EXAMPLE_EVENTS_3_MONITORING_CACHE_OPERATION: &str =
    "example.events.3_monitoring_cache_operation";
pub const MSG_EXAMPLE_EVENTS_SET_OPERATION_TOOK: &str = "example.events.set_operation_took";
pub const MSG_EXAMPLE_EVENTS_4_BATCH_OPERATION_EVENTS: &str =
    "example.events.4_batch_operation_events";
pub const MSG_EXAMPLE_EVENTS_5_CUSTOM_EVENTS: &str = "example.events.5_custom_events";
pub const MSG_EXAMPLE_EVENTS_6_ALL_EVENT_TYPES: &str = "example.events.6_all_event_types";
pub const MSG_EXAMPLE_EVENTS_EVENT_TYPE: &str = "example.events.event_type";
pub const MSG_EXAMPLE_EVENTS_EXAMPLE_COMPLETE: &str = "example.events.example_complete";

// example_i18n.rs
pub const MSG_EXAMPLE_I18N_INTERNATIONALIZATION_I18N_EXAMPLE: &str =
    "example.i18n.internationalization_i18n_example";
pub const MSG_EXAMPLE_I18N_1_CREATING_FORMATTERS: &str = "example.i18n.1_creating_formatters";
pub const MSG_EXAMPLE_I18N_EN_US_FORMATTER_CREATED: &str = "example.i18n.en_us_formatter_created";
pub const MSG_EXAMPLE_I18N_ZH_CN_FORMATTER_CREATED: &str = "example.i18n.zh_cn_formatter_created";
pub const MSG_EXAMPLE_I18N_DE_DE_FORMATTER_CREATED: &str = "example.i18n.de_de_formatter_created";
pub const MSG_EXAMPLE_I18N_UNEXPECTED_AN_INVALID_LOCALE: &str =
    "example.i18n.unexpected_an_invalid_locale";
pub const MSG_EXAMPLE_I18N_INVALID_LOCALE_CORRECTLY_ERRORED: &str =
    "example.i18n.invalid_locale_correctly_errored";
pub const MSG_EXAMPLE_I18N_2_NUMBER_FORMATTING: &str = "example.i18n.2_number_formatting";
pub const MSG_EXAMPLE_I18N_RAW_VALUE: &str = "example.i18n.raw_value";
pub const MSG_EXAMPLE_I18N_NAN_CORRECTLY_ERRORED: &str = "example.i18n.nan_correctly_errored";
pub const MSG_EXAMPLE_I18N_3_PLURAL_RULES: &str = "example.i18n.3_plural_rules";
pub const MSG_EXAMPLE_I18N_4_DATE_FORMATTING: &str = "example.i18n.4_date_formatting";
pub const MSG_EXAMPLE_I18N_EXPIRY_DATE: &str = "example.i18n.expiry_date";
pub const MSG_EXAMPLE_I18N_5_CACHE_KEY_GENERATION: &str = "example.i18n.5_cache_key_generation";
pub const MSG_EXAMPLE_I18N_6_KEY_COMPARISON_COLLATION: &str =
    "example.i18n.6_key_comparison_collation";
pub const MSG_EXAMPLE_I18N_ORIGINAL_ORDER: &str = "example.i18n.original_order";
pub const MSG_EXAMPLE_I18N_EN_US_COLLATION: &str = "example.i18n.en_us_collation";
pub const MSG_EXAMPLE_I18N_7_MULTILINGUAL_CACHE_STATISTICS: &str =
    "example.i18n.7_multilingual_cache_statistics";
pub const MSG_EXAMPLE_I18N_CACHE_HITS: &str = "example.i18n.cache_hits";
pub const MSG_EXAMPLE_I18N_CACHE_MISSES: &str = "example.i18n.cache_misses";
pub const MSG_EXAMPLE_I18N_EXAMPLE_COMPLETE: &str = "example.i18n.example_complete";

// example_kit_integration.rs
pub const MSG_EXAMPLE_KIT_INTEGRATION_1_BUILDING_ASYNCKIT_OXCACHEMODULE: &str =
    "example.kit_integration.1_building_asynckit_oxcachemodule";
pub const MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEBUILDOBSERVER: &str =
    "example.kit_integration.registered_oxcachebuildobserver";
pub const MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEMODULE: &str =
    "example.kit_integration.registered_oxcachemodule";
pub const MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THE_COUNTINGDECORATOR_DECORAT: &str =
    "example.kit_integration.registered_the_countingdecorator_decorat";
pub const MSG_EXAMPLE_KIT_INTEGRATION_ASYNCKIT_BUILD_COMPLETE: &str =
    "example.kit_integration.asynckit_build_complete";
pub const MSG_EXAMPLE_KIT_INTEGRATION_2_USING_THE_CACHE: &str =
    "example.kit_integration.2_using_the_cache";
pub const MSG_EXAMPLE_KIT_INTEGRATION_GET_USER_999_SHOULD: &str =
    "example.kit_integration.get_user_999_should";
pub const MSG_EXAMPLE_KIT_INTEGRATION_OPERATION_COUNT_SHOULD_BE: &str =
    "example.kit_integration.operation_count_should_be";
pub const MSG_EXAMPLE_KIT_INTEGRATION_3_THREE_PHASE_SHUTDOWN: &str =
    "example.kit_integration.3_three_phase_shutdown";
pub const MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THREE_PHASE_SHUTDOWN: &str =
    "example.kit_integration.registered_three_phase_shutdown";
pub const MSG_EXAMPLE_KIT_INTEGRATION_DRAINQUEUE_HEALTH_CHECK_LIVENESS: &str =
    "example.kit_integration.drainqueue_health_check_liveness";
pub const MSG_EXAMPLE_KIT_INTEGRATION_SHUTDOWN_RESULT: &str =
    "example.kit_integration.shutdown_result";
pub const MSG_EXAMPLE_KIT_INTEGRATION_DONE: &str = "example.kit_integration.done";
pub const MSG_EXAMPLE_KIT_INTEGRATION_TRAIT_KIT_INTEGRATION_DEMO: &str =
    "example.kit_integration.trait_kit_integration_demo";

// example_metrics.rs
pub const MSG_EXAMPLE_METRICS_AND_OBSERVABILITY_EXAMPLE: &str =
    "example.metrics.and_observability_example";
pub const MSG_EXAMPLE_METRICS_1_CREATING_A_CACHE: &str = "example.metrics.1_creating_a_cache";
pub const MSG_EXAMPLE_METRICS_PERFORMED_20_WRITES_AND: &str =
    "example.metrics.performed_20_writes_and";
pub const MSG_EXAMPLE_METRICS_2_BACKEND_STATISTICS: &str = "example.metrics.2_backend_statistics";
pub const MSG_EXAMPLE_METRICS_BACKEND_STATISTICS: &str = "example.metrics.backend_statistics";
pub const MSG_EXAMPLE_METRICS_3_ENHANCED_STATISTICS_CACHESTATS: &str =
    "example.metrics.3_enhanced_statistics_cachestats";
pub const MSG_EXAMPLE_METRICS_L1_HITS: &str = "example.metrics.l1_hits";
pub const MSG_EXAMPLE_METRICS_L1_MISSES: &str = "example.metrics.l1_misses";
pub const MSG_EXAMPLE_METRICS_L2_HITS: &str = "example.metrics.l2_hits";
pub const MSG_EXAMPLE_METRICS_L2_MISSES: &str = "example.metrics.l2_misses";
pub const MSG_EXAMPLE_METRICS_L1_WRITES: &str = "example.metrics.l1_writes";
pub const MSG_EXAMPLE_METRICS_L2_WRITES: &str = "example.metrics.l2_writes";
pub const MSG_EXAMPLE_METRICS_TOTAL_OPERATIONS: &str = "example.metrics.total_operations";
pub const MSG_EXAMPLE_METRICS_L1_ENTRIES: &str = "example.metrics.l1_entries";
pub const MSG_EXAMPLE_METRICS_4_HIT_RATE: &str = "example.metrics.4_hit_rate";
pub const MSG_EXAMPLE_METRICS_L1_HIT_RATE: &str = "example.metrics.l1_hit_rate";
pub const MSG_EXAMPLE_METRICS_L2_HIT_RATE: &str = "example.metrics.l2_hit_rate";
pub const MSG_EXAMPLE_METRICS_OVERALL_HIT_RATE: &str = "example.metrics.overall_hit_rate";
pub const MSG_EXAMPLE_METRICS_5_PROMETHEUS_FORMAT_EXPORT: &str =
    "example.metrics.5_prometheus_format_export";
pub const MSG_EXAMPLE_METRICS_6_JSON_FORMAT_EXPORT: &str = "example.metrics.6_json_format_export";
pub const MSG_EXAMPLE_METRICS_7_CACHESTATS_INSTANCE_METHOD: &str =
    "example.metrics.7_cachestats_instance_method";
pub const MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_PROMETHEUS_FIRST: &str =
    "example.metrics.cachestats_export_prometheus_first";
pub const MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_JSON_FIRST: &str =
    "example.metrics.cachestats_export_json_first";
pub const MSG_EXAMPLE_METRICS_EXAMPLE_COMPLETE: &str = "example.metrics.example_complete";

// example_security.rs
pub const MSG_EXAMPLE_SECURITY_OXCACHE_SECURITY_FEATURES_EXAMPLE: &str =
    "example.security.oxcache_security_features_example";
pub const MSG_EXAMPLE_SECURITY_1_SENSITIVE_DATA_REDACTION: &str =
    "example.security.1_sensitive_data_redaction";
pub const MSG_EXAMPLE_SECURITY_RAW_PASSWORD: &str = "example.security.raw_password";
pub const MSG_EXAMPLE_SECURITY_REDACTED: &str = "example.security.redacted";
pub const MSG_EXAMPLE_SECURITY_CONNECTION_STRING_REDACTION: &str =
    "example.security.connection_string_redaction";
pub const MSG_EXAMPLE_SECURITY_RAW_URL: &str = "example.security.raw_url";
pub const MSG_EXAMPLE_SECURITY_REDACTED_2: &str = "example.security.redacted_2";
pub const MSG_EXAMPLE_SECURITY_SENSITIVE_CACHE_KEY_REDACTION: &str =
    "example.security.sensitive_cache_key_redaction";
pub const MSG_EXAMPLE_SECURITY_FIELD_VALUE_REDACTION: &str =
    "example.security.field_value_redaction";
pub const MSG_EXAMPLE_SECURITY_2_THE_REDACTED_WRAPPER: &str =
    "example.security.2_the_redacted_wrapper";
pub const MSG_EXAMPLE_SECURITY_REDACTED_WRAPPER: &str = "example.security.redacted_wrapper";
pub const MSG_EXAMPLE_SECURITY_CUSTOM_VISIBLE_CHARACTERS: &str =
    "example.security.custom_visible_characters";
pub const MSG_EXAMPLE_SECURITY_3_SAFE_LOG_HANDLING: &str = "example.security.3_safe_log_handling";
pub const MSG_EXAMPLE_SECURITY_RAW: &str = "example.security.raw";
pub const MSG_EXAMPLE_SECURITY_SAFE: &str = "example.security.safe";
pub const MSG_EXAMPLE_SECURITY_4_CACHE_OPERATION_SAFETY: &str =
    "example.security.4_cache_operation_safety";
pub const MSG_EXAMPLE_SECURITY_STORING_THE_USER_SESSION: &str =
    "example.security.storing_the_user_session";
pub const MSG_EXAMPLE_SECURITY_DATA_READ_BACK_BYTES: &str = "example.security.data_read_back_bytes";
pub const MSG_EXAMPLE_SECURITY_5_SECURITY_BEST_PRACTICES: &str =
    "example.security.5_security_best_practices";
pub const MSG_EXAMPLE_SECURITY_NEVER_LOG_SENSITIVE_INFORMATION: &str =
    "example.security.never_log_sensitive_information";
pub const MSG_EXAMPLE_SECURITY_USE_THE_REDACTION_MODULE: &str =
    "example.security.use_the_redaction_module";
pub const MSG_EXAMPLE_SECURITY_ENCRYPT_SENSITIVE_DATA_BEFORE: &str =
    "example.security.encrypt_sensitive_data_before";
pub const MSG_EXAMPLE_SECURITY_USE_SECURE_CONNECTION_STRING: &str =
    "example.security.use_secure_connection_string";
pub const MSG_EXAMPLE_SECURITY_ROTATE_SENSITIVE_TOKENS_IN: &str =
    "example.security.rotate_sensitive_tokens_in";
pub const MSG_EXAMPLE_SECURITY_KEEP_CACHE_KEY_NAMES: &str = "example.security.keep_cache_key_names";
pub const MSG_EXAMPLE_SECURITY_6_CONFIGURATION_SAFETY: &str =
    "example.security.6_configuration_safety";
pub const MSG_EXAMPLE_SECURITY_UNSAFE_STORING_PASSWORDS_IN: &str =
    "example.security.unsafe_storing_passwords_in";
pub const MSG_EXAMPLE_SECURITY_SAFE_USE_ENVIRONMENT_VARIABLES: &str =
    "example.security.safe_use_environment_variables";
pub const MSG_EXAMPLE_SECURITY_SAFE_HIDE_CONNECTION_STRING: &str =
    "example.security.safe_hide_connection_string";
pub const MSG_EXAMPLE_SECURITY_LOG_CONNECTING_TO: &str = "example.security.log_connecting_to";
pub const MSG_EXAMPLE_SECURITY_EXAMPLE_COMPLETE: &str = "example.security.example_complete";

// example_security_validation.rs
pub const MSG_EXAMPLE_SECURITY_VALIDATION_OXCACHE_SECURITY_VALIDATION_EXAMPLE: &str =
    "example.security_validation.oxcache_security_validation_example";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_1_REDIS_KEY_VALIDATION: &str =
    "example.security_validation.1_redis_key_validation";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_VALID: &str = "example.security_validation.valid";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_INVALID: &str = "example.security_validation.invalid";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_COMMAND_INJECTION_CHARACTER_DETECTION: &str =
    "example.security_validation.command_injection_character_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED: &str = "example.security_validation.passed";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED: &str = "example.security_validation.rejected";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_SQL_INJECTION_PATTERN_DETECTION: &str =
    "example.security_validation.sql_injection_pattern_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_2: &str = "example.security_validation.passed_2";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_2: &str =
    "example.security_validation.rejected_2";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PATH_TRAVERSAL_DETECTION: &str =
    "example.security_validation.path_traversal_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_3: &str = "example.security_validation.passed_3";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_3: &str =
    "example.security_validation.rejected_3";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_CONTROL_CHARACTER_DETECTION: &str =
    "example.security_validation.control_character_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_4: &str = "example.security_validation.passed_4";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_4: &str =
    "example.security_validation.rejected_4";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_2_LUA_SCRIPT_VALIDATION: &str =
    "example.security_validation.2_lua_script_validation";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_VALID_SCRIPT_PASSED_VALIDATION: &str =
    "example.security_validation.valid_script_passed_validation";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_ERROR: &str = "example.security_validation.error";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_DANGEROUS_COMMAND_DETECTION: &str =
    "example.security_validation.dangerous_command_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_5: &str = "example.security_validation.passed_5";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_5: &str =
    "example.security_validation.rejected_5";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_INFINITE_LOOP_DETECTION: &str =
    "example.security_validation.infinite_loop_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_6: &str = "example.security_validation.passed_6";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_6: &str =
    "example.security_validation.rejected_6";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_LENGTH_AND_KEY_COUNT: &str =
    "example.security_validation.length_and_key_count";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_SCRIPT_PASSED: &str =
    "example.security_validation.oversized_script_passed";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_SCRIPT: &str =
    "example.security_validation.rejected_an_oversized_script";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_KEY_COUNT_PASSED: &str =
    "example.security_validation.excess_key_count_passed";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_KEY_COUNT: &str =
    "example.security_validation.rejected_excess_key_count";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_3_SCAN_PATTERN_VALIDATION: &str =
    "example.security_validation.3_scan_pattern_validation";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_VALID_2: &str = "example.security_validation.valid_2";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_INVALID_2: &str = "example.security_validation.invalid_2";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_WILDCARD_DETECTION: &str =
    "example.security_validation.excess_wildcard_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_7: &str = "example.security_validation.passed_7";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_WILDCARDS_10: &str =
    "example.security_validation.rejected_excess_wildcards_10";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_PATTERN_DETECTION: &str =
    "example.security_validation.oversized_pattern_detection";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_8: &str = "example.security_validation.passed_8";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_PATTERN: &str =
    "example.security_validation.rejected_an_oversized_pattern";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_4_SCAN_COUNT_LIMITS: &str =
    "example.security_validation.4_scan_count_limits";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_RAW_CLAMPED: &str =
    "example.security_validation.raw_clamped";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_5_SAFE_LOGGING_LOG: &str =
    "example.security_validation.5_safe_logging_log";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_KEY: &str = "example.security_validation.key";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_6_IN_PRACTICE_A: &str =
    "example.security_validation.6_in_practice_a";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUPPLIED: &str =
    "example.security_validation.validating_a_user_supplied";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED: &str = "example.security_validation.accepted";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_7: &str =
    "example.security_validation.rejected_7";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUBMITTED: &str =
    "example.security_validation.validating_a_user_submitted";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED_SCRIPT: &str =
    "example.security_validation.accepted_script";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_SCRIPT: &str =
    "example.security_validation.rejected_script";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_EXAMPLE_COMPLETE: &str =
    "example.security_validation.example_complete";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_BEST_PRACTICES: &str =
    "example.security_validation.best_practices";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_1_EVERY_USER_SUPPLIED: &str =
    "example.security_validation.1_every_user_supplied";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_2_EVERY_USER_SUBMITTED: &str =
    "example.security_validation.2_every_user_submitted";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_3_EVERY_SCAN_PATTERN: &str =
    "example.security_validation.3_every_scan_pattern";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_4_EVERY_SCAN_COUNT: &str =
    "example.security_validation.4_every_scan_count";
pub const MSG_EXAMPLE_SECURITY_VALIDATION_5_USE_LOG_CACHE: &str =
    "example.security_validation.5_use_log_cache";

// -- Bench diagnostics --
pub const MSG_BENCH_SKIP_SERVICE_UNREACHABLE: &str = "bench.skip_service_unreachable";

// === Catalog lookup ===

/// Check whether a language code has a dedicated message catalog.
///
/// Currently supported: `"en"`, `"zh"`.
pub fn is_supported(lang: &str) -> bool {
    matches!(lang, "en" | "zh")
}

/// Convert a dotted message ID (`error.not_found`) into its dashed FTL key
/// (`error-not-found`).
fn ftl_key(message_id: &str) -> String {
    message_id.replace(['.', '_'], "-")
}

/// Translate a message under the current default locale (see
/// [`super::get_default_locale`]).
///
/// This is the general-purpose runtime entry point, also called by
/// macro-generated code (e.g. the `#[cached(strict)]` panic message).
/// Missing keys render as the key itself; this function never panics.
pub fn t(key: &str, args: &[(&str, String)]) -> String {
    let locale = super::get_default_locale();
    let lang = locale.split('-').next().unwrap_or("en");
    let borrowed: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
    lookup(lang, key, &borrowed).unwrap_or_else(|| key.to_string())
}

/// Look up and format a message by `locale` and `message_id`.
///
/// Locale matching strategy:
/// 1. Language prefix match (e.g. `zh` matches `zh-CN`)
/// 2. Fallback to English (`en`)
///
/// Returns `None` if `message_id` is not found in any locale.
pub(crate) fn lookup(locale: &str, message_id: &str, params: &[(&str, &str)]) -> Option<String> {
    let lang = locale.split('-').next().unwrap_or(locale);
    let key = ftl_key(message_id);
    match lang {
        "zh" => format_from_bundle("zh", &key, params)
            .or_else(|| format_from_bundle("en", &key, params)),
        _ => format_from_bundle("en", &key, params),
    }
}

// === Fluent bundle management ===

const EN_FTL: &str = include_str!("../../locales/en/messages.ftl");
const ZH_FTL: &str = include_str!("../../locales/zh/messages.ftl");

/// Cached concurrent Fluent bundles (thread-safe, built once on first access).
static EN_BUNDLE: OnceLock<FluentBundle<FluentResource>> = OnceLock::new();
static ZH_BUNDLE: OnceLock<FluentBundle<FluentResource>> = OnceLock::new();

/// Format a message from the Fluent catalog for the given language.
fn format_from_bundle(lang: &str, key: &str, args: &[(&str, &str)]) -> Option<String> {
    let bundle = match lang {
        "zh" => ZH_BUNDLE.get_or_init(build_zh_bundle),
        _ => EN_BUNDLE.get_or_init(build_en_bundle),
    };

    let msg = bundle.get_message(key)?;
    let pattern = msg.value()?;

    let mut fluent_args = FluentArgs::new();
    for (name, value) in args {
        fluent_args.set(*name, FluentValue::from(*value));
    }

    let mut errors = vec![];
    let result = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
    Some(result.to_string())
}

fn build_en_bundle() -> FluentBundle<FluentResource> {
    let resource = FluentResource::try_new(EN_FTL.to_string()).unwrap_or_else(|e| e.0);
    let langid: LanguageIdentifier = "en".parse().expect("'en' is a valid language identifier");
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .expect("EN resources should add without conflict");
    bundle
}

fn build_zh_bundle() -> FluentBundle<FluentResource> {
    let resource = FluentResource::try_new(ZH_FTL.to_string()).unwrap_or_else(|e| e.0);
    let langid: LanguageIdentifier = "zh".parse().expect("'zh' is a valid language identifier");
    let mut bundle = FluentBundle::new_concurrent(vec![langid]);
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .expect("ZH resources should add without conflict");
    bundle
}

// === Tests ===

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract FTL message keys (`key = value` lines) from a resource.
    fn ftl_keys(ftl: &str) -> Vec<&str> {
        ftl.lines()
            .filter_map(|line| line.split_once(" = "))
            .filter(|(key, _)| key.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
            .map(|(key, _)| key)
            .collect()
    }

    /// All message IDs: error/config/i18n templates plus log, panic, detail
    /// and example messages.
    const ALL_MESSAGE_IDS: &[&str] = &[
        MSG_ERR_SERIALIZATION,
        MSG_ERR_OPERATION,
        MSG_ERR_CONNECTION,
        MSG_ERR_NOT_FOUND,
        MSG_ERR_DEGRADED,
        MSG_ERR_L1,
        MSG_ERR_L2,
        MSG_ERR_NOT_SUPPORTED,
        MSG_ERR_WAL,
        MSG_ERR_DATABASE,
        MSG_ERR_REDIS,
        MSG_ERR_IO,
        MSG_ERR_BACKEND,
        MSG_ERR_TIMEOUT,
        MSG_ERR_SHUTDOWN,
        MSG_ERR_KEY_TOO_LONG,
        MSG_ERR_VALUE_TOO_LARGE,
        MSG_ERR_BUFFER_FULL,
        MSG_ERR_INVALID_INPUT,
        MSG_ERR_INVALID_KEY,
        MSG_ERR_LOCK,
        MSG_ERR_SERVICE_NOT_FOUND,
        MSG_ERR_INTERNAL,
        MSG_CFG_MISSING_FIELD,
        MSG_CFG_INVALID_VALUE,
        MSG_CFG_UNSUPPORTED_BACKEND,
        MSG_CFG_CONNECTION_FAILED,
        MSG_I18N_INVALID_LOCALE,
        MSG_I18N_INVALID_NUMBER,
        MSG_I18N_DATE_ERROR,
        MSG_I18N_FORMAT_ERROR,
        MSG_LOG_CHAIN_READ_COMPLETED,
        MSG_LOG_CHAIN_EXPIRE_BACKEND_FAILED,
        MSG_LOG_CHAIN_ITER_ENTRIES_KEY_FAILED,
        MSG_LOG_CHAIN_BACKFILL_SUCCEEDED,
        MSG_LOG_CHAIN_BACKFILL_FAILED,
        MSG_LOG_OFFLOAD_LIFECYCLE_EVENT,
        MSG_LOG_OFFLOAD_TIMEOUT_POLICY_EXCEEDED,
        MSG_LOG_DEGRADATION_ENTERED,
        MSG_LOG_DEGRADATION_HALF_OPEN,
        MSG_LOG_DEGRADATION_RECOVERED,
        MSG_LOG_CONFERS_RELOAD_REJECTED,
        MSG_LOG_BRIDGE_SHUTDOWN_SKIPPED,
        MSG_LOG_DISK_SWEEP_FAILED,
        MSG_LOG_AUDIT_EVENT,
        MSG_LOG_STALE_HIT_VIA_GET_OR,
        MSG_LOG_STALE_REVALIDATION_SCHEDULED,
        MSG_LOG_STALE_HIT_SERVED,
        MSG_LOG_PUBSUB_RECONNECT_FAILED,
        MSG_LOG_PUBSUB_HANDLER_PANICKED,
        MSG_LOG_PUBSUB_PAYLOAD_PARSE_FAILED,
        MSG_LOG_PUBSUB_MAX_RECONNECT_ATTEMPTS_REACHED,
        MSG_LOG_PUBSUB_RECONNECTING,
        MSG_LOG_PUBSUB_RESUBSCRIBE_FAILED,
        MSG_LOG_MACRO_PASSTHROUGH,
        MSG_LOG_CIRCUIT_BREAKER_TRANSITION,
        MSG_PANIC_CHAIN_FRESHNESS_INVARIANT,
        MSG_PANIC_BRIDGE_TEMP_RUNTIME,
        MSG_PANIC_WARMUP_SEMAPHORE,
        MSG_PANIC_AUDIT_WRITER_LOCK,
        MSG_PANIC_AUDIT_WRITER_RX,
        MSG_PANIC_STALE_STATE_PAYLOAD,
        MSG_PANIC_BLOOM_CAPACITY_POSITIVE,
        MSG_PANIC_BLOOM_HASH_COUNT_POSITIVE,
        MSG_PANIC_BLOOM_FPR_RANGE,
        MSG_PANIC_BLOOM_HASH_COUNT_UNREACHABLE,
        MSG_PANIC_BLOOM_BACKSOLVE_DRIFT,
        MSG_PANIC_BLOOM_SEED_FAILED,
        MSG_PANIC_CONFIG_VALIDATE_MOKA_MEMORY,
        MSG_PANIC_CONFIG_VALIDATE_DASHMAP_MEMORY,
        MSG_PANIC_CONFIG_VALIDATE_MOCK_TEST_MEMORY,
        MSG_PANIC_CONFIG_VALIDATE_REDIS_FEATURE,
        MSG_PANIC_CONFIG_VALIDATE_DRAGONFLY_FEATURE,
        MSG_PANIC_CONFIG_VALIDATE_DISK_FEATURE,
        MSG_PANIC_CANONICAL_SERIALIZE_VALUE_CANNOT_FAIL,
        MSG_DETAIL_DISK_REDB_OPEN_FAILED,
        MSG_DETAIL_DISK_REDB_CREATE_FAILED,
        MSG_DETAIL_DISK_CORRUPT_ENVELOPE,
        MSG_DETAIL_SYNC_ATOMIC_INCREMENT,
        MSG_DETAIL_SYNC_ATOMIC_CAS,
        MSG_DETAIL_SYNC_ATOMIC_SET_IF_ABSENT,
        MSG_DETAIL_ASYNC_ATOMIC_INCREMENT,
        MSG_DETAIL_ASYNC_ATOMIC_CAS,
        MSG_DETAIL_ASYNC_ATOMIC_SET_IF_ABSENT,
        MSG_DETAIL_SYNC_REQUIRES_RUNTIME,
        MSG_DETAIL_SYNC_REQUIRES_MULTI_THREAD,
        MSG_DETAIL_CONFIG_ENV_INVALID_VALUE,
        MSG_DETAIL_CONFIG_ENV_BACKEND_FEATURES,
        MSG_DETAIL_CONFIG_ENV_SERIALIZATION_FEATURE,
        MSG_DETAIL_CONFIG_BACKEND_FEATURES,
        MSG_DETAIL_CONFIG_CAPACITY_ZERO,
        MSG_DETAIL_CONFIG_CAPACITY_EXCEEDS_USIZE,
        MSG_DETAIL_CONFIG_TTL_ZERO,
        MSG_DETAIL_CONFIG_METRICS_FEATURE,
        MSG_DETAIL_CONFIG_SERIALIZATION_FEATURE,
        MSG_DETAIL_CONFIG_SERIALIZATION_BINCODE_REQUIRES_FEATURE,
        MSG_DETAIL_CONFIG_SERIALIZATION_POSTCARD_REQUIRES_FEATURE,
        MSG_DETAIL_CONFIG_SERIALIZATION_INVALID_FORMAT,
        MSG_DETAIL_CONFIG_CB_THRESHOLD_ZERO,
        MSG_DETAIL_CONFIG_SERVICE_NAME_EMPTY,
        MSG_DETAIL_CONFIG_POOL_SIZE_ZERO,
        MSG_DETAIL_ADAPTIVE_TTL_MIN_EXCEEDS_MAX,
        MSG_DETAIL_ADAPTIVE_TTL_MULTIPLIER_FINITE,
        MSG_DETAIL_ADAPTIVE_TTL_DIVISOR_MIN,
        MSG_DETAIL_ADAPTIVE_TTL_TRACKED_KEYS_MIN,
        MSG_DETAIL_GET_OR_LEADER_NOT_CACHED,
        MSG_DETAIL_GET_OR_OPTION_LEADER_NOT_CACHED,
        MSG_DETAIL_WARMUP_TTL_LOOKUP_FAILED,
        MSG_DETAIL_REDIS_TTL_MIN_MILLIS,
        MSG_DETAIL_REDIS_TTL_EXCEEDS_MAX,
        MSG_DETAIL_REDIS_CLUSTER_CONNECT_FAILED,
        MSG_DETAIL_REDIS_CLUSTER_CONNECT_TIMEOUT,
        MSG_DETAIL_REDIS_KEY_SQL_INJECTION_PATTERN,
        MSG_DETAIL_REDIS_SQL_INJECTION_AUTH_BYPASS,
        MSG_DETAIL_REDIS_SQL_INJECTION_COMMENT,
        MSG_DETAIL_REDIS_SQL_INJECTION_DELETE,
        MSG_DETAIL_REDIS_SQL_INJECTION_DROP,
        MSG_DETAIL_REDIS_SQL_INJECTION_INSERT,
        MSG_DETAIL_REDIS_SQL_INJECTION_QUOTE_OR,
        MSG_DETAIL_REDIS_SQL_INJECTION_TAUTOLOGY,
        MSG_DETAIL_REDIS_SQL_INJECTION_UNION_SELECT,
        MSG_DETAIL_REDIS_SQL_INJECTION_XP_CMDSHELL,
        MSG_DETAIL_CONFERS_EXPECTS_U64,
        MSG_DETAIL_CONFERS_VALUE_EXCEEDS_RANGE,
        MSG_DETAIL_CONFERS_EXPECTS_BOOL,
        MSG_DETAIL_CONFERS_EXPECTS_F64,
        MSG_DETAIL_CONFERS_EXPECTS_STRING,
        MSG_DETAIL_CONFERS_READ_FAILED,
        MSG_DETAIL_BUILDER_NO_BACKEND_REQUIRES_MEMORY,
        MSG_DETAIL_BUILDER_STALE_TTL_SYNC_CONFLICT,
        MSG_DETAIL_BUILDER_ADAPTIVE_TTL_SYNC_CONFLICT,
        MSG_DETAIL_BUILDER_ADAPTIVE_TTL_STALE_CONFLICT,
        MSG_DETAIL_CONFIG_BACKEND_REQUIRES_FEATURE,
        MSG_DETAIL_CONFIG_BACKEND_KIND_REQUIRES_FEATURE,
        MSG_DETAIL_CONFIG_BACKEND_MOCK_TEST_ONLY,
        MSG_DETAIL_CONFIG_BACKEND_AEROSPIKE_PROGRAMMATIC,
        MSG_DETAIL_CONFIG_BACKEND_VALKEY_NO_IMPL,
        MSG_DETAIL_CONFIG_BACKEND_CHAIN_NEEDS_BUILDER,
        MSG_DETAIL_CONFIG_BACKEND_UNKNOWN_KIND,
        MSG_DETAIL_CONFIG_BACKEND_NOT_CONFIG_BUILDABLE,
        MSG_DETAIL_CONFIG_ENV_NOT_UNICODE,
        MSG_DETAIL_CONFIG_ENV_INVALID_BOOL,
        MSG_DETAIL_CONFIG_BACKEND_INVALID_VALUE,
        MSG_DETAIL_DISK_OPEN_CREATE_FAILED,
        MSG_DETAIL_CACHE_MEMORY_REQUIRES_FEATURE,
        MSG_DETAIL_NOT_SUPPORTED_MOKA_SYNC_CURRENT_THREAD,
        MSG_DETAIL_REDIS_UNEXPECTED_INFO_REPLY,
        MSG_DETAIL_REDIS_KEY_CONTROL_CHARACTER,
        MSG_DETAIL_REDIS_KEY_PATH_TRAVERSAL_PATTERN,
        MSG_DETAIL_REDIS_KEY_COMMAND_INJECTION_CHARACTER,
        MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_LONG,
        MSG_DETAIL_REDIS_SCAN_PATTERN_TOO_MANY_WILDCARDS,
        MSG_DETAIL_PUBSUB_CLIENT_OPEN_FAILED,
        MSG_DETAIL_PUBSUB_CONNECTION_FAILED,
        MSG_DETAIL_PUBSUB_PUBLISH_FAILED,
        MSG_DETAIL_PUBSUB_CONNECT_FAILED,
        MSG_DETAIL_PUBSUB_SUBSCRIBE_FAILED,
        MSG_DETAIL_PUBSUB_SUBSCRIBE_TASK_ENDED,
        MSG_DETAIL_LUA_SCRIPT_TOO_LONG,
        MSG_DETAIL_LUA_SCRIPT_TOO_MANY_KEYS,
        MSG_DETAIL_LUA_SCRIPT_FORBIDDEN_PATTERN,
        MSG_DETAIL_LUA_NESTED_EVAL,
        MSG_DETAIL_LUA_INFINITE_LOOP_PATTERNS,
        MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_TRUE,
        MSG_DETAIL_LUA_INFINITE_LOOP_WHILE_1,
        MSG_DETAIL_LUA_INFINITE_LOOP_REPEAT,
        MSG_DETAIL_LUA_INFINITE_LOOP_GOTO,
        MSG_DETAIL_BACKEND_LAYER_RESTRICTION_L1_ONLY,
        MSG_DETAIL_BACKEND_LAYER_RESTRICTION_L2_AND_L3_ONLY,
        MSG_DETAIL_BACKEND_LAYER_RESTRICTION_ANY,
        MSG_DETAIL_CHAIN_GET_MANY_LENGTH_MISMATCH,
        MSG_DETAIL_CHAIN_PARALLEL_FRESHEST_ALL_FAILED,
        MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_READ,
        MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_RACE_READ,
        MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_WRITE,
        MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_DELETE,
        MSG_DETAIL_CHAIN_ALL_BACKENDS_FAILED_CLEAR,
        MSG_DETAIL_CHAIN_SYNC_REQUIRES_SYNC_BACKEND,
        MSG_DETAIL_CHAIN_NO_BACKENDS,
        MSG_DETAIL_CHAIN_HEALTH_CHECK_TIMED_OUT,
        MSG_DETAIL_CHAIN_HEALTH_CHECK_FAILED,
        MSG_DETAIL_CHAIN_INCR_NO_ATOMIC_WRITER,
        MSG_DETAIL_CHAIN_CAS_NO_ATOMIC_WRITER,
        MSG_DETAIL_CHAIN_SET_IF_ABSENT_NO_ATOMIC_WRITER,
        MSG_DETAIL_KEY_GENERATOR_PART_CONTAINS_SEGMENT_SEPARATOR,
        MSG_EXAMPLE_INKLOG_BRIDGE_TITLE,
        MSG_EXAMPLE_INKLOG_BRIDGE_OBSERVABILITY,
        MSG_EXAMPLE_INKLOG_BRIDGE_DROPPED,
        MSG_EXAMPLE_INKLOG_BRIDGE_WRITE_FAILURES,
        MSG_EXAMPLE_REDIS_MODES_FEATURE_DISABLED,
        MSG_EXAMPLE_REDIS_MODES_RUN_WITH_REDIS_FEATURE,
        MSG_EXAMPLE_REDIS_MODES_RUN_WITH_EXAMPLES_PACKAGE,
        MSG_EXAMPLE_REDIS_MODES_STANDALONE_CONNECT_FAILED,
        MSG_EXAMPLE_REDIS_MODES_STANDALONE_ENV_REQUIRED,
        MSG_EXAMPLE_REDIS_MODES_DONE_STANDALONE_SKIPPED,
        MSG_EXAMPLE_BASIC_OPERATIONS_SET_USER,
        MSG_EXAMPLE_BASIC_OPERATIONS_GETTING_USER,
        MSG_EXAMPLE_BASIC_OPERATIONS_GOT_USER,
        MSG_EXAMPLE_BASIC_OPERATIONS_DELETING_USER,
        MSG_EXAMPLE_BASIC_OPERATIONS_UPDATING_USER,
        MSG_EXAMPLE_BASIC_OPERATIONS_UPDATED_USER,
        MSG_EXAMPLE_BASIC_OPERATIONS_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_BASIC_OPERATIONS_SET_STORE_A_VALUE,
        MSG_EXAMPLE_BASIC_OPERATIONS_GET_READ_A_VALUE,
        MSG_EXAMPLE_BASIC_OPERATIONS_DELETE_REMOVE_A_VALUE,
        MSG_EXAMPLE_BASIC_OPERATIONS_UPDATE_OVERWRITE_THE_EXISTING,
        MSG_EXAMPLE_BYTE_OPS_BYTE_LEVEL_OPERATIONS_EXAMPLE,
        MSG_EXAMPLE_BYTE_OPS_1_BASIC_BYTE_OPERATIONS,
        MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES,
        MSG_EXAMPLE_BYTE_OPS_READ,
        MSG_EXAMPLE_BYTE_OPS_2_BYTE_OPERATIONS_WITHOUT,
        MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_NO_TTL,
        MSG_EXAMPLE_BYTE_OPS_READ_BYTES,
        MSG_EXAMPLE_BYTE_OPS_3_BINARY_DATA,
        MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_OF_BINARY,
        MSG_EXAMPLE_BYTE_OPS_READ_BYTES_2,
        MSG_EXAMPLE_BYTE_OPS_CHECKSUM,
        MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MATCH,
        MSG_EXAMPLE_BYTE_OPS_CHECKSUM_MISMATCH,
        MSG_EXAMPLE_BYTE_OPS_4_CACHE_METADATA_OPERATIONS,
        MSG_EXAMPLE_BYTE_OPS_ENTRIES,
        MSG_EXAMPLE_BYTE_OPS_IS_EMPTY,
        MSG_EXAMPLE_BYTE_OPS_CAPACITY,
        MSG_EXAMPLE_BYTE_OPS_5_SERIALIZER_ACCESS,
        MSG_EXAMPLE_BYTE_OPS_SERIALIZER_INSTANCE_OBTAINED_DYN,
        MSG_EXAMPLE_BYTE_OPS_6_LARGE_DATA_OPERATIONS,
        MSG_EXAMPLE_BYTE_OPS_WRITE_BYTES_2,
        MSG_EXAMPLE_BYTE_OPS_READ_BYTES_3,
        MSG_EXAMPLE_BYTE_OPS_AFTER_CLEANUP_ENTRIES_IS,
        MSG_EXAMPLE_BYTE_OPS_CACHE_CLOSED,
        MSG_EXAMPLE_BYTE_OPS_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_CACHE_BUILDER_CACHEBUILDER_FULL_USAGE_EXAMPLE,
        MSG_EXAMPLE_CACHE_BUILDER_1_BASIC_CONSTRUCTION,
        MSG_EXAMPLE_CACHE_BUILDER_BASIC_CACHE_CREATED,
        MSG_EXAMPLE_CACHE_BUILDER_2_SET_CAPACITY,
        MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_CAPACITY,
        MSG_EXAMPLE_CACHE_BUILDER_3_SET_TTL,
        MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_1,
        MSG_EXAMPLE_CACHE_BUILDER_4_SET_TTI,
        MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_5,
        MSG_EXAMPLE_CACHE_BUILDER_5_COMBINED_CONFIGURATION,
        MSG_EXAMPLE_CACHE_BUILDER_COMBINED_CACHE_CREATED_CAPACITY,
        MSG_EXAMPLE_CACHE_BUILDER_6_USE_A_CUSTOM,
        MSG_EXAMPLE_CACHE_BUILDER_CACHE_CREATED_WITH_A,
        MSG_EXAMPLE_CACHE_BUILDER_7_REAL_USAGE,
        MSG_EXAMPLE_CACHE_BUILDER_WRITE,
        MSG_EXAMPLE_CACHE_BUILDER_READ,
        MSG_EXAMPLE_CACHE_BUILDER_8_SET_WITH_TTL,
        MSG_EXAMPLE_CACHE_BUILDER_STORING_A_PRODUCT_WITH,
        MSG_EXAMPLE_CACHE_BUILDER_EXISTS_CHECK,
        MSG_EXAMPLE_CACHE_BUILDER_9_STATISTICS,
        MSG_EXAMPLE_CACHE_BUILDER_CACHE_STATS,
        MSG_EXAMPLE_CACHE_BUILDER_10_HEALTH_CHECK,
        MSG_EXAMPLE_CACHE_BUILDER_HEALTH_STATUS_OK,
        MSG_EXAMPLE_CACHE_BUILDER_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE,
        MSG_EXAMPLE_CACHE_KEY_1_CACHEKEY_FOR_BUILT,
        MSG_EXAMPLE_CACHE_KEY_2_CACHEKEY_FOR_CUSTOM,
        MSG_EXAMPLE_CACHE_KEY_3_USING_CACHEKEY_IN,
        MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT,
        MSG_EXAMPLE_CACHE_KEY_READ,
        MSG_EXAMPLE_CACHE_KEY_STORE_KEY_PRODUCT_2,
        MSG_EXAMPLE_CACHE_KEY_4_BATCH_OPERATIONS,
        MSG_EXAMPLE_CACHE_KEY_STORE_KEY,
        MSG_EXAMPLE_CACHE_KEY_BATCH_READ_RESULTS,
        MSG_EXAMPLE_CACHE_KEY_5_KEY_FORMAT_CONSISTENCY,
        MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_100,
        MSG_EXAMPLE_CACHE_KEY_USERID_100_USERID_200,
        MSG_EXAMPLE_CACHE_KEY_SAME_VALUE_PRODUCES_SAME,
        MSG_EXAMPLE_CACHE_KEY_CACHEKEY_TRAIT_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_CACHE_KEY_KEY_POINTS,
        MSG_EXAMPLE_CACHE_KEY_BUILT_IN_TYPES_STRING,
        MSG_EXAMPLE_CACHE_KEY_CUSTOM_TYPES_CAN_GENERATE,
        MSG_EXAMPLE_CACHE_KEY_TO_KEY_STRING_SHOULD,
        MSG_EXAMPLE_CACHE_KEY_CACHEKEY_CAN_BE_USED,
        MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION,
        MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_2,
        MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_3,
        MSG_EXAMPLE_CACHED_MACRO_USAGE_EXAMPLE,
        MSG_EXAMPLE_CACHED_MACRO_CACHE_REGISTERED_FOR_USE,
        MSG_EXAMPLE_CACHED_MACRO_1_BASIC_CACHING_DEMONSTRATION,
        MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_OF_GET,
        MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_RESULT,
        MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_OF_GET,
        MSG_EXAMPLE_CACHED_MACRO_SECOND_CALL_RESULT,
        MSG_EXAMPLE_CACHED_MACRO_2_CUSTOM_KEY_FORMAT,
        MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM,
        MSG_EXAMPLE_CACHED_MACRO_RESULT,
        MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_CUSTOM_2,
        MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT,
        MSG_EXAMPLE_CACHED_MACRO_3_SHORT_TTL_CACHING,
        MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA,
        MSG_EXAMPLE_CACHED_MACRO_RESULT_2,
        MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_HOT_DATA_2,
        MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_2,
        MSG_EXAMPLE_CACHED_MACRO_4_PERFORMANCE_COMPARISON,
        MSG_EXAMPLE_CACHED_MACRO_CACHE_HITS_ARE_TYPICALLY,
        MSG_EXAMPLE_CACHED_MACRO_FIRST_CALL_TOOK,
        MSG_EXAMPLE_CACHED_MACRO_CACHE_HIT_TOOK,
        MSG_EXAMPLE_CACHED_MACRO_SPEEDUP_X,
        MSG_EXAMPLE_CACHED_MACRO_EXECUTING_THE_ORIGINAL_FUNCTION_4,
        MSG_EXAMPLE_CACHED_MACRO_5_MULTI_ARGUMENT_FUNCTION,
        MSG_EXAMPLE_CACHED_MACRO_CALLING_GET_USER_WITH,
        MSG_EXAMPLE_CACHED_MACRO_RESULT_3,
        MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_THE_SAME,
        MSG_EXAMPLE_CACHED_MACRO_CACHED_RESULT_3,
        MSG_EXAMPLE_CACHED_MACRO_CALLING_WITH_DIFFERENT_ARGUMENTS,
        MSG_EXAMPLE_CACHED_MACRO_DIFFERENT_ARGUMENT_RESULT,
        MSG_EXAMPLE_CACHED_MACRO_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_CACHED_MACRO_MAIN_ADVANTAGES_OF_THE,
        MSG_EXAMPLE_CACHED_MACRO_ZERO_BOILERPLATE_CACHING_LOGIC,
        MSG_EXAMPLE_CACHED_MACRO_TYPE_SAFE_CHECKED_AT,
        MSG_EXAMPLE_CACHED_MACRO_FLEXIBLE_CONFIGURATION_SUPPORTS_MULTIPLE,
        MSG_EXAMPLE_CACHED_MACRO_AUTOMATIC_SERIALIZATION_NO_MANUAL,
        MSG_EXAMPLE_CACHED_MACRO_PERFORMANCE_SIGNIFICANTLY_IMPROVES_CACHE,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_OXCACHE_COMPREHENSIVE_USAGE_EXAMPLE,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_1_CREATE_AN_IN,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_IN_MEMORY_CACHE_CREATED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_2_CREATE_A_REDIS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_CACHE_CREATED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_REDIS_UNAVAILABLE_DEGRADED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_3_CREATE_A_TIERED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_TIERED_CACHE_CREATED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_4_BASIC_CRUD_OPERATIONS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_USER,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_ADDED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_GETTING_USER,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_RETRIEVED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_UPDATING_USER,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_UPDATED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_AFTER_THE_UPDATE,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_NOT_FOUND_2,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_USER,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_FAILED_TO_DELETE_THE,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_USER_DELETED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_5_BATCH_OPERATIONS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_PRODUCTS_IN_BATCH,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_ADD_SUCCEEDED_PRODUCTS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_FETCHING_PRODUCTS_IN_BATCH,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_FETCH_SUCCEEDED_PRODUCTS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_DELETING_PRODUCTS_IN_BATCH,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_BATCH_DELETE_SUCCEEDED,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_6_TTL_CONTROL,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_ADDING_DATA_THAT_EXPIRES,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_IMMEDIATE_GET,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_WAITING_4_SECONDS_THEN,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_GET_AFTER_4_SECONDS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_7_PERFORMANCE_TEST,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITING_10000_ENTRIES_CONCURRENTLY,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_WRITE_TOOK,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_READING_10000_ENTRIES_CONCURRENTLY,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_READ_TOOK,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_8_STATISTICS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_CACHE_STATS,
        MSG_EXAMPLE_COMPREHENSIVE_USAGE_EXAMPLE_RUN_COMPLETE,
        MSG_EXAMPLE_GET_OR_DB_LOADING_USER_FROM,
        MSG_EXAMPLE_GET_OR_SINGLE_FLIGHT,
        MSG_EXAMPLE_GET_OR_1_BASIC_USAGE,
        MSG_EXAMPLE_GET_OR_FIRST_GET_OR_CALL,
        MSG_EXAMPLE_GET_OR_RESULT,
        MSG_EXAMPLE_GET_OR_SECOND_GET_OR_CALL,
        MSG_EXAMPLE_GET_OR_RESULT_2,
        MSG_EXAMPLE_GET_OR_2_CONCURRENT_SINGLE_FLIGHT,
        MSG_EXAMPLE_GET_OR_REQUEST_STARTED,
        MSG_EXAMPLE_GET_OR_REQUEST_COMPLETED,
        MSG_EXAMPLE_GET_OR_NOTE_ONLY_ONE_DB,
        MSG_EXAMPLE_GET_OR_3_COMPARISON_CONCURRENCY_WITHOUT,
        MSG_EXAMPLE_GET_OR_NOTE_WITHOUT_SINGLE_FLIGHT,
        MSG_EXAMPLE_GET_OR_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_NEW_API_USAGE_EXAMPLE,
        MSG_EXAMPLE_NEW_API_1_IN_MEMORY_CACHE,
        MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_MEMORY,
        MSG_EXAMPLE_NEW_API_2_REDIS_CACHE_L2,
        MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_REDIS,
        MSG_EXAMPLE_NEW_API_REDIS_UNAVAILABLE_SKIPPING_REDIS,
        MSG_EXAMPLE_NEW_API_3_CUSTOM_KEY_TYPE,
        MSG_EXAMPLE_NEW_API_USER_RETRIEVED_WITH_CUSTOM,
        MSG_EXAMPLE_NEW_API_4_ADVANCED_CONFIGURATION_USING,
        MSG_EXAMPLE_NEW_API_USER_RETRIEVED_FROM_THE,
        MSG_EXAMPLE_NEW_API_5_TTL_OPERATIONS_USING,
        MSG_EXAMPLE_NEW_API_SET_USER_7_WITH,
        MSG_EXAMPLE_NEW_API_UPDATED_THE_TTL_OF,
        MSG_EXAMPLE_NEW_API_6_BATCH_OPERATIONS,
        MSG_EXAMPLE_NEW_API_BATCH_SET_USERS,
        MSG_EXAMPLE_NEW_API_RETRIEVED_USERS_FROM_THE,
        MSG_EXAMPLE_NEW_API_7_DELETE_OPERATIONS,
        MSG_EXAMPLE_NEW_API_USER_8_EXISTS,
        MSG_EXAMPLE_NEW_API_USER_8_EXISTS_AFTER,
        MSG_EXAMPLE_NEW_API_8_CLEAR_OPERATIONS,
        MSG_EXAMPLE_NEW_API_SET_3_USERS_IN,
        MSG_EXAMPLE_NEW_API_CLEARED_ALL_CACHE_ENTRIES,
        MSG_EXAMPLE_NEW_API_USER_1_EXISTS_AFTER,
        MSG_EXAMPLE_NEW_API_9_CACHE_ASIDE_MODE,
        MSG_EXAMPLE_NEW_API_USER_RETRIEVED_VIA_THE,
        MSG_EXAMPLE_NEW_API_10_SUMMARY,
        MSG_EXAMPLE_NEW_API_FEATURES,
        MSG_EXAMPLE_NEW_API_TYPE_SAFE_CACHE_INTERFACE,
        MSG_EXAMPLE_NEW_API_IN_MEMORY_CACHE_CACHE,
        MSG_EXAMPLE_NEW_API_REDIS_CACHE_CACHE_REDIS,
        MSG_EXAMPLE_NEW_API_CUSTOM_KEY_TYPES_CACHEKEY,
        MSG_EXAMPLE_NEW_API_BUILDER_PATTERN_FOR_CONFIGURATION,
        MSG_EXAMPLE_NEW_API_TTL_OPERATIONS_SET_WITH,
        MSG_EXAMPLE_NEW_API_BATCH_OPERATIONS_SET_MANY,
        MSG_EXAMPLE_NEW_API_DELETE_AND_CLEAR_OPERATIONS,
        MSG_EXAMPLE_NEW_API_EXISTENCE_CHECK_EXISTS,
        MSG_EXAMPLE_NEW_API_CACHE_ASIDE_MODE_WITH,
        MSG_EXAMPLE_NEW_API_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_SERIALIZATION_OPTIONS_EXAMPLE,
        MSG_EXAMPLE_SERIALIZATION_1_JSON_SERIALIZATION_DEFAULT,
        MSG_EXAMPLE_SERIALIZATION_HUMAN_READABLE_FORMAT,
        MSG_EXAMPLE_SERIALIZATION_GOOD_FOR_DEBUGGING,
        MSG_EXAMPLE_SERIALIZATION_SLIGHTLY_LARGER_SIZE,
        MSG_EXAMPLE_SERIALIZATION_STORED_USER,
        MSG_EXAMPLE_SERIALIZATION_RETRIEVED,
        MSG_EXAMPLE_SERIALIZATION_CONFIG,
        MSG_EXAMPLE_SERIALIZATION_2_SERIALIZATION_FEATURES,
        MSG_EXAMPLE_SERIALIZATION_JSON_STANDARD_READABLE_FORMAT,
        MSG_EXAMPLE_SERIALIZATION_BINCODE_COMPACT_BINARY_FORMAT,
        MSG_EXAMPLE_SERIALIZATION_SUPPORTS_CUSTOM_SERIALIZERS,
        MSG_EXAMPLE_SERIALIZATION_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_SYNC_API_BASIC_SYNC_OPERATIONS,
        MSG_EXAMPLE_SYNC_API_SET_USER_1,
        MSG_EXAMPLE_SYNC_API_GET_USER_1,
        MSG_EXAMPLE_SYNC_API_DELETE_USER_1,
        MSG_EXAMPLE_SYNC_API_SET_TEMP_60S_TTL,
        MSG_EXAMPLE_SYNC_API_GET_TEMP,
        MSG_EXAMPLE_SYNC_API_GET_OR_SYNC_SINGLE,
        MSG_EXAMPLE_SYNC_API_FIRST_GET_OR_SYNC,
        MSG_EXAMPLE_SYNC_API_SECOND_GET_OR_SYNC,
        MSG_EXAMPLE_SYNC_API_MIXING_SYNC_AND_ASYNC,
        MSG_EXAMPLE_SYNC_API_CACHE_METADATA_SYNC,
        MSG_EXAMPLE_SYNC_API_ENTRIES_LEN_SYNC,
        MSG_EXAMPLE_SYNC_API_CACHE_CLEARED,
        MSG_EXAMPLE_SYNC_API_ENTRIES_AFTER_CLEAR,
        MSG_EXAMPLE_SYNC_API_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE,
        MSG_EXAMPLE_BATCH_WRITE_1_BATCH_ADD_PRODUCTS,
        MSG_EXAMPLE_BATCH_WRITE_BATCH_ADDING_PRODUCTS,
        MSG_EXAMPLE_BATCH_WRITE_BATCH_ADD_COMPLETE_TOOK,
        MSG_EXAMPLE_BATCH_WRITE_2_BATCH_UPDATE_PRICES,
        MSG_EXAMPLE_BATCH_WRITE_UPDATING_PRODUCT_PRICES,
        MSG_EXAMPLE_BATCH_WRITE_PRODUCT_NEW_PRICE,
        MSG_EXAMPLE_BATCH_WRITE_3_BATCH_READ_VERIFICATION,
        MSG_EXAMPLE_BATCH_WRITE_READING_ALL_PRODUCT_INFO,
        MSG_EXAMPLE_BATCH_WRITE_STOCK,
        MSG_EXAMPLE_BATCH_WRITE_4_USING_SET_MANY,
        MSG_EXAMPLE_BATCH_WRITE_SET_MANY_WROTE_ENTRIES,
        MSG_EXAMPLE_BATCH_WRITE_GET_MANY_READ_ENTRIES,
        MSG_EXAMPLE_BATCH_WRITE_DELETE_MANY_DELETED_ENTRIES,
        MSG_EXAMPLE_BATCH_WRITE_ENTRIES_REMAIN_AFTER_DELETE,
        MSG_EXAMPLE_BATCH_WRITE_5_BATCH_DELETE_SIMULATING,
        MSG_EXAMPLE_BATCH_WRITE_DELISTED_PRODUCT_IDS,
        MSG_EXAMPLE_BATCH_WRITE_PRODUCTS_DELISTED,
        MSG_EXAMPLE_BATCH_WRITE_6_CACHE_STATISTICS,
        MSG_EXAMPLE_BATCH_WRITE_CACHE_TYPE,
        MSG_EXAMPLE_BATCH_WRITE_CAPACITY,
        MSG_EXAMPLE_BATCH_WRITE_ENTRIES,
        MSG_EXAMPLE_BATCH_WRITE_OPTIMIZATION_EXAMPLE_2,
        MSG_EXAMPLE_CACHE_PROMOTION_STRATEGY_EXAMPLE,
        MSG_EXAMPLE_CACHE_PROMOTION_1_SIMULATING_L1_L2,
        MSG_EXAMPLE_CACHE_PROMOTION_L1_MEMORY_CAPACITY_3,
        MSG_EXAMPLE_CACHE_PROMOTION_L2_CACHE_CAPACITY_100,
        MSG_EXAMPLE_CACHE_PROMOTION_2_INITIALIZE_DATA_INTO,
        MSG_EXAMPLE_CACHE_PROMOTION_WROTE_SESSIONS_INTO_L2,
        MSG_EXAMPLE_CACHE_PROMOTION_3_READS_WITH_PROMOTION,
        MSG_EXAMPLE_CACHE_PROMOTION_L1_HIT,
        MSG_EXAMPLE_CACHE_PROMOTION_L2_HIT_PROMOTED_TO,
        MSG_EXAMPLE_CACHE_PROMOTION_ALL_MISS,
        MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_SEQUENCE_ACCESSES,
        MSG_EXAMPLE_CACHE_PROMOTION_L1_FINAL_STATE_ENTRIES,
        MSG_EXAMPLE_CACHE_PROMOTION_4_ACCESS_FREQUENCY_ANALYSIS,
        MSG_EXAMPLE_CACHE_PROMOTION_ACCESS_FREQUENCY_RANKING,
        MSG_EXAMPLE_CACHE_PROMOTION_TIMES,
        MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_SESS_1,
        MSG_EXAMPLE_CACHE_PROMOTION_5_PRODUCTION_CONFIGURATION_SUGGESTIONS,
        MSG_EXAMPLE_CACHE_PROMOTION_A_REAL_TIERED_CACHE,
        MSG_EXAMPLE_CACHE_PROMOTION_L1_MOKA_L2_REDIS,
        MSG_EXAMPLE_CACHE_PROMOTION_TTL_DURATION_FROM_SECS,
        MSG_EXAMPLE_CACHE_PROMOTION_CAPACITY_10_000_L1,
        MSG_EXAMPLE_CACHE_PROMOTION_PROMOTION_STRATEGY_NOTES,
        MSG_EXAMPLE_CACHE_PROMOTION_L1_IS_SMALL_BUT,
        MSG_EXAMPLE_CACHE_PROMOTION_ON_AN_L2_HIT,
        MSG_EXAMPLE_CACHE_PROMOTION_HOT_DATA_IS_PROMOTED,
        MSG_EXAMPLE_CACHE_PROMOTION_SIZE_L1_SENSIBLY_TO,
        MSG_EXAMPLE_CACHE_PROMOTION_6_TTL_EFFECTS_ON,
        MSG_EXAMPLE_CACHE_PROMOTION_SET_A_SESSION_WITH,
        MSG_EXAMPLE_CACHE_PROMOTION_REMAINING_TTL,
        MSG_EXAMPLE_CACHE_PROMOTION_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE,
        MSG_EXAMPLE_CHAIN_CACHE_1_CHAINCACHE_BUILDER_CREATES,
        MSG_EXAMPLE_CHAIN_CACHE_CONNECTING_TO_REDIS,
        MSG_EXAMPLE_CHAIN_CACHE_CREATED_A_2_TIER,
        MSG_EXAMPLE_CHAIN_CACHE_CANNOT_CONNECT_TO_REDIS,
        MSG_EXAMPLE_CHAIN_CACHE_SKIPPING_THE_REDIS_EXAMPLE,
        MSG_EXAMPLE_CHAIN_CACHE_2_IN_MEMORY_ONLY,
        MSG_EXAMPLE_CHAIN_CACHE_STORE,
        MSG_EXAMPLE_CHAIN_CACHE_READ,
        MSG_EXAMPLE_CHAIN_CACHE_3_BATCH_OPERATIONS_EXAMPLE,
        MSG_EXAMPLE_CHAIN_CACHE_BATCH_STORED_USERS,
        MSG_EXAMPLE_CHAIN_CACHE_BATCH_READ_USERS,
        MSG_EXAMPLE_CHAIN_CACHE_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA,
        MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA,
        MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE,
        MSG_EXAMPLE_CHAIN_CACHE_READING_DATA,
        MSG_EXAMPLE_CHAIN_CACHE_HIT,
        MSG_EXAMPLE_CHAIN_CACHE_FAILED_TO_PARSE_THE,
        MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND,
        MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS,
        MSG_EXAMPLE_CHAIN_CACHE_TEST_DATA_2,
        MSG_EXAMPLE_CHAIN_CACHE_WRITING_DATA_2,
        MSG_EXAMPLE_CHAIN_CACHE_DATA_WRITTEN_TO_THE_2,
        MSG_EXAMPLE_CHAIN_CACHE_READING_DATA_2,
        MSG_EXAMPLE_CHAIN_CACHE_HIT_2,
        MSG_EXAMPLE_CHAIN_CACHE_NOT_FOUND_2,
        MSG_EXAMPLE_CHAIN_CACHE_CACHE_STATS_2,
        MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_SHUT_DOWN_DATA,
        MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE,
        MSG_EXAMPLE_CUSTOM_BACKEND_1_CREATE_A_CUSTOM,
        MSG_EXAMPLE_CUSTOM_BACKEND_HASHMAPBACKEND_CREATED_CAPACITY_100,
        MSG_EXAMPLE_CUSTOM_BACKEND_2_INJECT_IT_VIA,
        MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_BUILT_WITH_THE,
        MSG_EXAMPLE_CUSTOM_BACKEND_3_BASIC_CRUD_OPERATIONS,
        MSG_EXAMPLE_CUSTOM_BACKEND_4_TTL_OPERATIONS,
        MSG_EXAMPLE_CUSTOM_BACKEND_SET_WITH_TTL_60S,
        MSG_EXAMPLE_CUSTOM_BACKEND_EXPIRE_UPDATED_TO_120S,
        MSG_EXAMPLE_CUSTOM_BACKEND_5_BATCH_OPERATIONS,
        MSG_EXAMPLE_CUSTOM_BACKEND_BATCH_WROTE_ENTRIES,
        MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_ENTRIES,
        MSG_EXAMPLE_CUSTOM_BACKEND_6_STATISTICS,
        MSG_EXAMPLE_CUSTOM_BACKEND_7_HEALTH_CHECK,
        MSG_EXAMPLE_CUSTOM_BACKEND_BACKEND_HEALTHY,
        MSG_EXAMPLE_CUSTOM_BACKEND_HEALTH_CHECK_FAILED,
        MSG_EXAMPLE_CUSTOM_BACKEND_8_CAPACITY_LIMIT_DEMONSTRATION,
        MSG_EXAMPLE_CUSTOM_BACKEND_WROTE_3_ENTRIES_CAPACITY,
        MSG_EXAMPLE_CUSTOM_BACKEND_UNEXPECTED_THE_WRITE_SUCCEEDED,
        MSG_EXAMPLE_CUSTOM_BACKEND_CAPACITY_FULL_THE_WRITE,
        MSG_EXAMPLE_CUSTOM_BACKEND_9_SHUTDOWN,
        MSG_EXAMPLE_CUSTOM_BACKEND_CACHE_CLOSED,
        MSG_EXAMPLE_CUSTOM_BACKEND_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE,
        MSG_EXAMPLE_DASHMAP_BACKEND_1_CREATE_A_DASHMAP,
        MSG_EXAMPLE_DASHMAP_BACKEND_CREATED,
        MSG_EXAMPLE_DASHMAP_BACKEND_2_BASIC_OPERATIONS,
        MSG_EXAMPLE_DASHMAP_BACKEND_WRITE_KEY1_VALUE1,
        MSG_EXAMPLE_DASHMAP_BACKEND_READ_KEY1,
        MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_KEY1,
        MSG_EXAMPLE_DASHMAP_BACKEND_3_BATCH_OPERATIONS,
        MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_WRITE_KEY_VALUE,
        MSG_EXAMPLE_DASHMAP_BACKEND_BATCH_READ_RESULTS,
        MSG_EXAMPLE_DASHMAP_BACKEND_4_COMPARING_MOKA_AND,
        MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_KEYS,
        MSG_EXAMPLE_DASHMAP_BACKEND_KEYS,
        MSG_EXAMPLE_DASHMAP_BACKEND_5_DELETE_OPERATIONS,
        MSG_EXAMPLE_DASHMAP_BACKEND_DELETE_KEY1,
        MSG_EXAMPLE_DASHMAP_BACKEND_EXISTS_CHECK_KEY1,
        MSG_EXAMPLE_DASHMAP_BACKEND_6_CLEAR_OPERATIONS,
        MSG_EXAMPLE_DASHMAP_BACKEND_BEFORE_CLEAR_KEYS,
        MSG_EXAMPLE_DASHMAP_BACKEND_AFTER_CLEAR_KEYS,
        MSG_EXAMPLE_DASHMAP_BACKEND_7_FEATURE_COMPARISON,
        MSG_EXAMPLE_DASHMAP_BACKEND_MOKA_BACKEND_TRAITS,
        MSG_EXAMPLE_DASHMAP_BACKEND_LRU_TINYLFU_EVICTION_POLICIES,
        MSG_EXAMPLE_DASHMAP_BACKEND_CAPACITY_LIMITS,
        MSG_EXAMPLE_DASHMAP_BACKEND_AUTOMATIC_EXPIRY,
        MSG_EXAMPLE_DASHMAP_BACKEND_SUITABLE_FOR_PRODUCTION,
        MSG_EXAMPLE_DASHMAP_BACKEND_TRAITS,
        MSG_EXAMPLE_DASHMAP_BACKEND_PLAIN_CONCURRENT_HASHMAP,
        MSG_EXAMPLE_DASHMAP_BACKEND_NO_EVICTION_POLICY,
        MSG_EXAMPLE_DASHMAP_BACKEND_MANUAL_TTL_MANAGEMENT,
        MSG_EXAMPLE_DASHMAP_BACKEND_SUITED_TO_SIMPLE_SCENARIOS,
        MSG_EXAMPLE_DASHMAP_BACKEND_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE,
        MSG_EXAMPLE_ERROR_HANDLING_1_ERROR_TYPE_CLASSIFICATION,
        MSG_EXAMPLE_ERROR_HANDLING_2_RECOVERABILITY,
        MSG_EXAMPLE_ERROR_HANDLING_3_ERROR_CODES_AND,
        MSG_EXAMPLE_ERROR_HANDLING_4_RESILIENT_OPERATIONS_WITH,
        MSG_EXAMPLE_ERROR_HANDLING_5_TRIGGERING_RUNTIME_ERRORS,
        MSG_EXAMPLE_ERROR_HANDLING_6_ERROR_PROPAGATION_WITH,
        MSG_EXAMPLE_ERROR_HANDLING_7_CONFIGURATION_TIME_ERRORS,
        MSG_EXAMPLE_ERROR_HANDLING_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_ERROR_HANDLING_CONNECTION_ERROR_RECOVERABLE,
        MSG_EXAMPLE_ERROR_HANDLING_TIMEOUT_ERROR_RECOVERABLE,
        MSG_EXAMPLE_ERROR_HANDLING_BACKEND_ERROR_RECOVERABLE,
        MSG_EXAMPLE_ERROR_HANDLING_NOT_FOUND_RECOVERABLE,
        MSG_EXAMPLE_ERROR_HANDLING_SERIALIZATION_RECOVERABLE,
        MSG_EXAMPLE_ERROR_HANDLING_DEGRADED_DEGRADED_MODE,
        MSG_EXAMPLE_ERROR_HANDLING_INTERNAL_ERROR_RECOVERABLE,
        MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE,
        MSG_EXAMPLE_ERROR_HANDLING_ERROR_MESSAGE,
        MSG_EXAMPLE_ERROR_HANDLING_ERROR_CODE_2,
        MSG_EXAMPLE_ERROR_HANDLING_RETRYABLE,
        MSG_EXAMPLE_ERROR_HANDLING_SUCCEEDED_ON_RETRY,
        MSG_EXAMPLE_ERROR_HANDLING_HIT_AN_UNRECOVERABLE_ERROR,
        MSG_EXAMPLE_ERROR_HANDLING_FAILED_ON_ATTEMPT_RETRYING,
        MSG_EXAMPLE_ERROR_HANDLING_REACHED_THE_MAXIMUM_RETRY,
        MSG_EXAMPLE_ERROR_HANDLING_FINAL_RESULT,
        MSG_EXAMPLE_ERROR_HANDLING_UNRECOVERABLE_ERROR_RESULT,
        MSG_EXAMPLE_ERROR_HANDLING_COL_ERROR_TYPE,
        MSG_EXAMPLE_ERROR_HANDLING_COL_RECOVERABLE,
        MSG_EXAMPLE_ERROR_HANDLING_COL_CONNECTION_ERROR,
        MSG_EXAMPLE_ERROR_HANDLING_RESULT_RETRY_SKIPPED,
        MSG_EXAMPLE_ERROR_HANDLING_RESULT_UNEXPECTED,
        MSG_EXAMPLE_ERROR_HANDLING_CACHE_OPERATION_SUCCEEDED,
        MSG_EXAMPLE_ERROR_HANDLING_NORMAL_READ,
        MSG_EXAMPLE_ERROR_HANDLING_A_MISSING_KEY_RETURNS,
        MSG_EXAMPLE_ERROR_HANDLING_GET_OR_COMPUTED_RESULT,
        MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK_PASSED,
        MSG_EXAMPLE_ERROR_HANDLING_HEALTH_CHECK,
        MSG_EXAMPLE_ERROR_HANDLING_USER_1_SCORE,
        MSG_EXAMPLE_ERROR_HANDLING_LOAD_FAILED,
        MSG_EXAMPLE_ERROR_HANDLING_USER_999_SCORE,
        MSG_EXAMPLE_ERROR_HANDLING_USER_999_LOAD_FAILED,
        MSG_EXAMPLE_ERROR_HANDLING_CONFIG_ERROR_CODE,
        MSG_EXAMPLE_INVALIDATION_CACHE_INVALIDATION_STRATEGY_EXAMPLE,
        MSG_EXAMPLE_INVALIDATION_1_PREPARE_TEST_DATA,
        MSG_EXAMPLE_INVALIDATION_ADDED_ORDERS,
        MSG_EXAMPLE_INVALIDATION_2_SINGLE_KEY_INVALIDATION,
        MSG_EXAMPLE_INVALIDATION_DELETING_ORDER_1,
        MSG_EXAMPLE_INVALIDATION_THE_ORDER_STILL_EXISTS,
        MSG_EXAMPLE_INVALIDATION_THE_ORDER_WAS_DELETED,
        MSG_EXAMPLE_INVALIDATION_3_BULK_INVALIDATION_BY,
        MSG_EXAMPLE_INVALIDATION_ORDER_2_STILL_EXISTS,
        MSG_EXAMPLE_INVALIDATION_ALL_ORDERS_CLEARED,
        MSG_EXAMPLE_INVALIDATION_4_USER_ID_BASED,
        MSG_EXAMPLE_INVALIDATION_ORIGINAL_CART,
        MSG_EXAMPLE_INVALIDATION_USER_100_CHECKS_OUT,
        MSG_EXAMPLE_INVALIDATION_AFTER_CLEAR,
        MSG_EXAMPLE_INVALIDATION_5_TTL_BASED_AUTOMATIC,
        MSG_EXAMPLE_INVALIDATION_ADDING_DATA_THAT_EXPIRES,
        MSG_EXAMPLE_INVALIDATION_IMMEDIATE_GET,
        MSG_EXAMPLE_INVALIDATION_WAITING_3_SECONDS,
        MSG_EXAMPLE_INVALIDATION_GET_AFTER_3_SECONDS,
        MSG_EXAMPLE_INVALIDATION_6_WRITE_TIME_INVALIDATION,
        MSG_EXAMPLE_INVALIDATION_INITIAL_THEME,
        MSG_EXAMPLE_INVALIDATION_UPDATED_THEME,
        MSG_EXAMPLE_INVALIDATION_STRATEGY_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_LUA_SCRIPT_EXECUTION_EXAMPLE,
        MSG_EXAMPLE_LUA_SCRIPT_CONNECTING_TO_REDIS,
        MSG_EXAMPLE_LUA_SCRIPT_REDIS_CONNECTED,
        MSG_EXAMPLE_LUA_SCRIPT_1_ATOMIC_COUNTER_EVAL,
        MSG_EXAMPLE_LUA_SCRIPT_CALL_COUNTER_VALUE,
        MSG_EXAMPLE_LUA_SCRIPT_2_PRELOADING_SCRIPTS_SCRIPT,
        MSG_EXAMPLE_LUA_SCRIPT_SCRIPT_SHA1,
        MSG_EXAMPLE_LUA_SCRIPT_3_EXECUTING_SCRIPTS_BY,
        MSG_EXAMPLE_LUA_SCRIPT_EVAL_LUA_RESULT,
        MSG_EXAMPLE_LUA_SCRIPT_EVAL_SHA_RESULT,
        MSG_EXAMPLE_LUA_SCRIPT_4_CONDITIONAL_UPDATES_COMPARE,
        MSG_EXAMPLE_LUA_SCRIPT_INITIAL_VALUE_V1,
        MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V2_UPDATING_TO,
        MSG_EXAMPLE_LUA_SCRIPT_EXPECTING_V1_UPDATING_TO,
        MSG_EXAMPLE_LUA_SCRIPT_EXAMPLE_COMPLETE_TEST_DATA,
        MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_NO_GLOBAL,
        MSG_EXAMPLE_MOKA_TTL_SET_K1_60S_PER,
        MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE,
        MSG_EXAMPLE_MOKA_TTL_EXPIRE_UPDATES_TTL,
        MSG_EXAMPLE_MOKA_TTL_EXPIRE_K1_120S_SHOULD,
        MSG_EXAMPLE_MOKA_TTL_TTL_K1_SHOULD_BE_2,
        MSG_EXAMPLE_MOKA_TTL_EXPIRE_MISSING_60S_SHOULD,
        MSG_EXAMPLE_MOKA_TTL_KEYS_WITHOUT_TTL,
        MSG_EXAMPLE_MOKA_TTL_SET_K2_NO_TTL,
        MSG_EXAMPLE_MOKA_TTL_TTL_K2_SHOULD_BE,
        MSG_EXAMPLE_MOKA_TTL_TTL_EXPIRY_VERIFICATION,
        MSG_EXAMPLE_MOKA_TTL_SET_TEMP_50MS_TTL,
        MSG_EXAMPLE_MOKA_TTL_IMMEDIATE_GET_TEMP,
        MSG_EXAMPLE_MOKA_TTL_GET_TEMP_AFTER_100MS,
        MSG_EXAMPLE_MOKA_TTL_GLOBAL_TTL_VS_PER,
        MSG_EXAMPLE_MOKA_TTL_CREATING_MOKAMEMORYBACKEND_GLOBAL_TTL,
        MSG_EXAMPLE_MOKA_TTL_SET_A_NO_PER,
        MSG_EXAMPLE_MOKA_TTL_SET_B_PER_ENTRY,
        MSG_EXAMPLE_MOKA_TTL_TTL_A_NONE_TTL,
        MSG_EXAMPLE_MOKA_TTL_TTL_B_SHOULD_BE,
        MSG_EXAMPLE_MOKA_TTL_GET_A_AFTER_450MS,
        MSG_EXAMPLE_MOKA_TTL_GET_B_AFTER_450MS,
        MSG_EXAMPLE_MOKA_TTL_MOKA_PER_ENTRY_TTL,
        MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION,
        MSG_EXAMPLE_REDIS_MODES_REDIS_MULTI_MODE_CONNECTION_2,
        MSG_EXAMPLE_REDIS_MODES_1_STANDALONE_MODE,
        MSG_EXAMPLE_REDIS_MODES_CONNECTED,
        MSG_EXAMPLE_REDIS_MODES_CONNECTED_MODE,
        MSG_EXAMPLE_REDIS_MODES_WRITE_READ,
        MSG_EXAMPLE_REDIS_MODES_2_EXPLICITLY_SELECTING_STANDALONE,
        MSG_EXAMPLE_REDIS_MODES_BUILDER_CREATED_MODE,
        MSG_EXAMPLE_REDIS_MODES_3_CLUSTER_MODE,
        MSG_EXAMPLE_REDIS_MODES_CONNECTED_2,
        MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTED_MODE,
        MSG_EXAMPLE_REDIS_MODES_WRITE_READ_2,
        MSG_EXAMPLE_REDIS_MODES_CLUSTER_CONNECTION_FAILED,
        MSG_EXAMPLE_REDIS_MODES_REDIS_CLUSTER_AVAILABLE_IS,
        MSG_EXAMPLE_REDIS_MODES_START_CLUSTER_CD_TESTS,
        MSG_EXAMPLE_REDIS_MODES_4_SENTINEL_MODE,
        MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION,
        MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTED_MODE,
        MSG_EXAMPLE_REDIS_MODES_WRITE_READ_3,
        MSG_EXAMPLE_REDIS_MODES_SENTINEL_CONNECTION_FAILED,
        MSG_EXAMPLE_REDIS_MODES_REDIS_SENTINEL_AVAILABLE_IS,
        MSG_EXAMPLE_REDIS_MODES_START_SENTINEL_CD_TESTS,
        MSG_EXAMPLE_REDIS_MODES_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_REDIS_MODES_5_THE_REDISMODETYPE_ENUM,
        MSG_EXAMPLE_REDIS_MODES_MODE_DISPLAY,
        MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE,
        MSG_EXAMPLE_REDIS_NATIVE_1_CONNECT_TO_REDIS,
        MSG_EXAMPLE_REDIS_NATIVE_REDIS_CONNECTED,
        MSG_EXAMPLE_REDIS_NATIVE_2_BASIC_CRUD_OPERATIONS,
        MSG_EXAMPLE_REDIS_NATIVE_ADDING_USER,
        MSG_EXAMPLE_REDIS_NATIVE_USER_ADDED,
        MSG_EXAMPLE_REDIS_NATIVE_GETTING_USER,
        MSG_EXAMPLE_REDIS_NATIVE_USER_RETRIEVED,
        MSG_EXAMPLE_REDIS_NATIVE_USER_NOT_FOUND,
        MSG_EXAMPLE_REDIS_NATIVE_UPDATING_USER,
        MSG_EXAMPLE_REDIS_NATIVE_USER_UPDATED,
        MSG_EXAMPLE_REDIS_NATIVE_DELETING_USER,
        MSG_EXAMPLE_REDIS_NATIVE_FAILED_TO_DELETE_THE,
        MSG_EXAMPLE_REDIS_NATIVE_USER_DELETED,
        MSG_EXAMPLE_REDIS_NATIVE_3_BATCH_OPERATIONS,
        MSG_EXAMPLE_REDIS_NATIVE_ADDING_USERS_IN_BATCH,
        MSG_EXAMPLE_REDIS_NATIVE_BATCH_ADD_SUCCEEDED,
        MSG_EXAMPLE_REDIS_NATIVE_FETCHING_USERS_IN_BATCH,
        MSG_EXAMPLE_REDIS_NATIVE_USER,
        MSG_EXAMPLE_REDIS_NATIVE_CLEARING_TEST_DATA,
        MSG_EXAMPLE_REDIS_NATIVE_CLEARED,
        MSG_EXAMPLE_REDIS_NATIVE_4_CACHE_STATISTICS,
        MSG_EXAMPLE_REDIS_NATIVE_CACHE_STATISTICS_ARE_AVAILABLE,
        MSG_EXAMPLE_REDIS_NATIVE_SEE_THE_METRICS_TEST,
        MSG_EXAMPLE_REDIS_NATIVE_CLIENT_EXAMPLE_2,
        MSG_EXAMPLE_REDIS_PIPELINE_BATCH_OPERATIONS,
        MSG_EXAMPLE_REDIS_PIPELINE_CONNECTING_TO_REDIS,
        MSG_EXAMPLE_REDIS_PIPELINE_REDIS_CONNECTED,
        MSG_EXAMPLE_REDIS_PIPELINE_1_BATCH_SET_SET,
        MSG_EXAMPLE_REDIS_PIPELINE_BATCH_SET_KEYS_TOOK,
        MSG_EXAMPLE_REDIS_PIPELINE_2_BATCH_GET_GET,
        MSG_EXAMPLE_REDIS_PIPELINE_BATCH_GET_KEYS_TOOK,
        MSG_EXAMPLE_REDIS_PIPELINE_RESULTS,
        MSG_EXAMPLE_REDIS_PIPELINE_3_PERFORMANCE_COMPARISON_ONE,
        MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_ONE,
        MSG_EXAMPLE_REDIS_PIPELINE_SETTING_10_KEYS_VIA,
        MSG_EXAMPLE_REDIS_PIPELINE_PIPELINE_IS_X_FASTER,
        MSG_EXAMPLE_REDIS_PIPELINE_4_BATCH_DELETE_DELETE,
        MSG_EXAMPLE_REDIS_PIPELINE_BATCH_DELETED_KEYS,
        MSG_EXAMPLE_REDIS_PIPELINE_REMAINING_KEYS_SHOULD_BE,
        MSG_EXAMPLE_REDIS_PIPELINE_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_SMART_STRATEGY_CACHE_STRATEGY_PATTERNS_EXAMPLE,
        MSG_EXAMPLE_SMART_STRATEGY_1_THE_CACHE_ASIDE,
        MSG_EXAMPLE_SMART_STRATEGY_READ_FILL_ON_MISS,
        MSG_EXAMPLE_SMART_STRATEGY_WRITE_INVALIDATE_AFTER_UPDATING,
        MSG_EXAMPLE_SMART_STRATEGY_FIRST_READ_OF_1,
        MSG_EXAMPLE_SMART_STRATEGY_SECOND_READ_OF_1,
        MSG_EXAMPLE_SMART_STRATEGY_UPDATING_1_AND_INVALIDATING,
        MSG_EXAMPLE_SMART_STRATEGY_READ_OF_1_AFTER,
        MSG_EXAMPLE_SMART_STRATEGY_2_THE_LAZY_LOADING,
        MSG_EXAMPLE_SMART_STRATEGY_LAZY_COMPUTATION_WITH_GET,
        MSG_EXAMPLE_SMART_STRATEGY_FIRST_GET_OR_ON,
        MSG_EXAMPLE_SMART_STRATEGY_SECOND_GET_OR_ON,
        MSG_EXAMPLE_SMART_STRATEGY_3_TTL_LIFECYCLE_MANAGEMENT,
        MSG_EXAMPLE_SMART_STRATEGY_DIFFERENT_DATA_TYPES_USE,
        MSG_EXAMPLE_SMART_STRATEGY_SESSION_DATA_TTL_30S,
        MSG_EXAMPLE_SMART_STRATEGY_POPULAR_ARTICLES_TTL_1H,
        MSG_EXAMPLE_SMART_STRATEGY_SYSTEM_CONFIG_TTL_24H,
        MSG_EXAMPLE_SMART_STRATEGY_TERMS_OF_SERVICE_NO,
        MSG_EXAMPLE_SMART_STRATEGY_REMAINING_TTL,
        MSG_EXAMPLE_SMART_STRATEGY_SESSIONS,
        MSG_EXAMPLE_SMART_STRATEGY_CONFIG,
        MSG_EXAMPLE_SMART_STRATEGY_TERMS,
        MSG_EXAMPLE_SMART_STRATEGY_4_CAPACITY_PLANNING_AND,
        MSG_EXAMPLE_SMART_STRATEGY_PREWARMED_POPULAR_ARTICLES,
        MSG_EXAMPLE_SMART_STRATEGY_SIMULATING_ACCESSES,
        MSG_EXAMPLE_SMART_STRATEGY_HITS,
        MSG_EXAMPLE_SMART_STRATEGY_MISSES,
        MSG_EXAMPLE_SMART_STRATEGY_5_CACHE_STATISTICS,
        MSG_EXAMPLE_SMART_STRATEGY_ENTRIES,
        MSG_EXAMPLE_SMART_STRATEGY_CAPACITY,
        MSG_EXAMPLE_SMART_STRATEGY_CACHE_STRATEGY_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE,
        MSG_EXAMPLE_WARMUP_1_SIMULATING_APPLICATION_CONFIG,
        MSG_EXAMPLE_WARMUP_LOADING_CONFIG_FROM_THE,
        MSG_EXAMPLE_WARMUP_LOADING_CONFIG,
        MSG_EXAMPLE_WARMUP_CONFIG_WARMUP_COMPLETE_ITEMS,
        MSG_EXAMPLE_WARMUP_2_SIMULATING_HOT_USER,
        MSG_EXAMPLE_WARMUP_PRELOADING_HOT_USERS,
        MSG_EXAMPLE_WARMUP_HOT_USER_WARMUP_COMPLETE,
        MSG_EXAMPLE_WARMUP_3_VERIFYING_THE_WARMED,
        MSG_EXAMPLE_WARMUP_CONFIG_VERIFICATION,
        MSG_EXAMPLE_WARMUP_EXPECTED,
        MSG_EXAMPLE_WARMUP_NOT_FOUND,
        MSG_EXAMPLE_WARMUP_USER_VERIFICATION,
        MSG_EXAMPLE_WARMUP_NOT_FOUND_2,
        MSG_EXAMPLE_WARMUP_4_SIMULATING_A_CACHE,
        MSG_EXAMPLE_WARMUP_CLEARING_THE_CACHE,
        MSG_EXAMPLE_WARMUP_REWARMING,
        MSG_EXAMPLE_WARMUP_CACHE_REBUILD_COMPLETE_TOOK,
        MSG_EXAMPLE_WARMUP_5_POST_WARMUP_STATISTICS,
        MSG_EXAMPLE_WARMUP_TOTAL_ENTRIES,
        MSG_EXAMPLE_WARMUP_HITS,
        MSG_EXAMPLE_WARMUP_CACHE_WARMUP_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE,
        MSG_EXAMPLE_DYNAMIC_CONFIG_1_INITIALIZING_CONFIG,
        MSG_EXAMPLE_DYNAMIC_CONFIG_2_CONFIG_QUERIES,
        MSG_EXAMPLE_DYNAMIC_CONFIG_TYPE,
        MSG_EXAMPLE_DYNAMIC_CONFIG_NOT_FOUND,
        MSG_EXAMPLE_DYNAMIC_CONFIG_3_CONFIG_UPDATE_DEMONSTRATION,
        MSG_EXAMPLE_DYNAMIC_CONFIG_UPDATING_APP_THEME,
        MSG_EXAMPLE_DYNAMIC_CONFIG_APP_THEME_UPDATED_TO,
        MSG_EXAMPLE_DYNAMIC_CONFIG_CURRENT_APP_THEME,
        MSG_EXAMPLE_DYNAMIC_CONFIG_4_CONFIG_EXPORT,
        MSG_EXAMPLE_DYNAMIC_CONFIG_EXPORTING_ALL_CONFIG,
        MSG_EXAMPLE_DYNAMIC_CONFIG_5_CONFIG_GROUPS_BY,
        MSG_EXAMPLE_DYNAMIC_CONFIG_APPLICATION_CONFIG_ITEMS,
        MSG_EXAMPLE_DYNAMIC_CONFIG_DATABASE_CONFIG_ITEMS,
        MSG_EXAMPLE_DYNAMIC_CONFIG_6_CLEANING_UP_TEST,
        MSG_EXAMPLE_DYNAMIC_CONFIG_TEST_DATA_CLEANED_UP,
        MSG_EXAMPLE_DYNAMIC_CONFIG_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE,
        MSG_EXAMPLE_KEY_GENERATOR_1_BASIC_USAGE,
        MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_NAMESPACE_DEFAULT,
        MSG_EXAMPLE_KEY_GENERATOR_DEFAULT_MAX_LENGTH_256,
        MSG_EXAMPLE_KEY_GENERATOR_2_USING_NAMESPACES,
        MSG_EXAMPLE_KEY_GENERATOR_USER_KEY,
        MSG_EXAMPLE_KEY_GENERATOR_ORDER_KEY,
        MSG_EXAMPLE_KEY_GENERATOR_3_USING_PREFIXES,
        MSG_EXAMPLE_KEY_GENERATOR_CACHE_KEY,
        MSG_EXAMPLE_KEY_GENERATOR_SESSION_KEY,
        MSG_EXAMPLE_KEY_GENERATOR_4_CHAINING_NAMESPACE_PREFIX,
        MSG_EXAMPLE_KEY_GENERATOR_FULL_KEY,
        MSG_EXAMPLE_KEY_GENERATOR_5_TEMPLATE_GENERATION,
        MSG_EXAMPLE_KEY_GENERATOR_GENERATE_USER_ID_PROFILE,
        MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_SEARCH_TYPE,
        MSG_EXAMPLE_KEY_GENERATOR_6_FULL_KEY_GENERATION,
        MSG_EXAMPLE_KEY_GENERATOR_GENERATE_FULL_USER_ID,
        MSG_EXAMPLE_KEY_GENERATOR_7_KEY_VALIDATION,
        MSG_EXAMPLE_KEY_GENERATOR_IS_VALID,
        MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID,
        MSG_EXAMPLE_KEY_GENERATOR_IS_VALID_2,
        MSG_EXAMPLE_KEY_GENERATOR_IS_INVALID_2,
        MSG_EXAMPLE_KEY_GENERATOR_8_IN_PRACTICE_CACHE,
        MSG_EXAMPLE_KEY_GENERATOR_STORE,
        MSG_EXAMPLE_KEY_GENERATOR_STORE_2,
        MSG_EXAMPLE_KEY_GENERATOR_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_USAGE_EXAMPLE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_1_CREATING_A_CACHE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_IN_MEMORY_CACHE_CREATED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_2_CONNECTION_STRING_EXAMPLES,
        MSG_EXAMPLE_DATABASE_INTEGRATION_OXCACHE_CAN_CREATE_A,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_127,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_REDIS_REDIS_HOST1,
        MSG_EXAMPLE_DATABASE_INTEGRATION_SEE_THE_EXAMPLE_REDIS,
        MSG_EXAMPLE_DATABASE_INTEGRATION_3_SIMULATING_A_DATABASE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_SIMULATED_DATABASE_LOADER_CREATED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_4_IMPLEMENTING_THE_CACHE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_TRYING_TO_READ_FROM,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_MISS_QUERYING_THE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_SUCCEEDED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_CACHED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_USER_NOT_FOUND_IN,
        MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_QUERY_ERROR,
        MSG_EXAMPLE_DATABASE_INTEGRATION_5_TESTING_THE_CACHE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_FIRST_QUERY_FOR_USER,
        MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT,
        MSG_EXAMPLE_DATABASE_INTEGRATION_SECOND_QUERY_FOR_USER,
        MSG_EXAMPLE_DATABASE_INTEGRATION_RESULT_2,
        MSG_EXAMPLE_DATABASE_INTEGRATION_QUERYING_THE_NONEXISTENT_USER,
        MSG_EXAMPLE_DATABASE_INTEGRATION_UNEXPECTED_FOUND_THE_USER,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CORRECT_THE_USER_DOES,
        MSG_EXAMPLE_DATABASE_INTEGRATION_6_BATCH_LOADING_EXAMPLE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_PROCESSING_USER,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_HIT_2,
        MSG_EXAMPLE_DATABASE_INTEGRATION_DATABASE_LOAD_SUCCEEDED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_ABSENT_FROM_THE_DATABASE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_LOAD_ERROR,
        MSG_EXAMPLE_DATABASE_INTEGRATION_BATCH_LOAD_RESULTS,
        MSG_EXAMPLE_DATABASE_INTEGRATION_NOT_FOUND,
        MSG_EXAMPLE_DATABASE_INTEGRATION_7_CACHE_UPDATES_AND,
        MSG_EXAMPLE_DATABASE_INTEGRATION_UPDATING_THE_USER_RECORD,
        MSG_EXAMPLE_DATABASE_INTEGRATION_USER_RECORD_UPDATED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_THE_UPDATE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_8_CACHE_INVALIDATION_EXAMPLE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_INVALIDATING_THE_CACHE_FOR,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_INVALIDATED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_VERIFYING_INVALIDATION,
        MSG_EXAMPLE_DATABASE_INTEGRATION_RELOAD_SUCCEEDED,
        MSG_EXAMPLE_DATABASE_INTEGRATION_9_CACHE_STATISTICS,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_TYPE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_ENTRIES,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CAPACITY,
        MSG_EXAMPLE_DATABASE_INTEGRATION_FAILED_TO_GET_STATISTICS,
        MSG_EXAMPLE_DATABASE_INTEGRATION_10_SIMULATING_A_PARTITIONING,
        MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_COUNT,
        MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION_ASSIGNMENT,
        MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITION,
        MSG_EXAMPLE_DATABASE_INTEGRATION_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_KEY_BENEFITS_OF_DATABASE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CACHE_ASIDE_CACHE_MISSES,
        MSG_EXAMPLE_DATABASE_INTEGRATION_LESS_DATABASE_LOAD_HIGHER,
        MSG_EXAMPLE_DATABASE_INTEGRATION_DATA_CONSISTENCY_PROMPT_UPDATES,
        MSG_EXAMPLE_DATABASE_INTEGRATION_PARTITIONING_SUPPORTS_LARGE_SCALE,
        MSG_EXAMPLE_DATABASE_INTEGRATION_CONNECTION_MANAGEMENT_UNIFORM_CONNECTION,
        MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTER_STANDALONE_TYPE,
        MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTER_CAPACITY_10000,
        MSG_EXAMPLE_BLOOM_FILTER_INSERTING_EXISTING_KEY,
        MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_SHOULD,
        MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_MISSING_KEY_SHOULD,
        MSG_EXAMPLE_BLOOM_FILTER_CONTAINS_EXISTING_KEY_AFTER,
        MSG_EXAMPLE_BLOOM_FILTER_BLOOMFILTERBACKEND_DECORATOR,
        MSG_EXAMPLE_BLOOM_FILTER_INNER_BACKEND_SCORE,
        MSG_EXAMPLE_BLOOM_FILTER_CREATING_BLOOMFILTERBACKEND_WRAPPING_MOK,
        MSG_EXAMPLE_BLOOM_FILTER_GET_USER_999_FILTERED,
        MSG_EXAMPLE_BLOOM_FILTER_TTL_PASSTHROUGH,
        MSG_EXAMPLE_BLOOM_FILTER_SET_TEMP_60S_TTL,
        MSG_EXAMPLE_BLOOM_FILTER_TTL_TEMP_SHOULD_BE,
        MSG_EXAMPLE_BLOOM_FILTER_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_CLI_USAGE_OXCACHE_CLI_USAGE_EXAMPLE,
        MSG_EXAMPLE_CLI_USAGE_CACHE_CREATED_DEMO_CACHE,
        MSG_EXAMPLE_CLI_USAGE_PERFORMING_CACHE_OPERATIONS,
        MSG_EXAMPLE_CLI_USAGE_SET_10_CACHE_ENTRIES,
        MSG_EXAMPLE_CLI_USAGE_RETRIEVED_5_CACHE_ENTRIES,
        MSG_EXAMPLE_CLI_USAGE_ATTEMPTING_TO_RETRIEVE_5,
        MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_STATUS_OUTPUT,
        MSG_EXAMPLE_CLI_USAGE_CACHE_NAME_DEMO_CACHE,
        MSG_EXAMPLE_CLI_USAGE_CACHE_TYPE_L1_IN,
        MSG_EXAMPLE_CLI_USAGE_MAX_CAPACITY_1000,
        MSG_EXAMPLE_CLI_USAGE_CURRENT_ENTRIES,
        MSG_EXAMPLE_CLI_USAGE_STATUS_RUNNING,
        MSG_EXAMPLE_CLI_USAGE_SIMULATED_OXCACHE_METRICS_OUTPUT,
        MSG_EXAMPLE_CLI_USAGE_CLI_COMMAND_FORMAT,
        MSG_EXAMPLE_CLI_USAGE_CHECK_STATUS_OXCACHE_STATUS,
        MSG_EXAMPLE_CLI_USAGE_CHECK_METRICS_OXCACHE_METRICS,
        MSG_EXAMPLE_CLI_USAGE_CLEAN_CACHE_OXCACHE_ADMIN,
        MSG_EXAMPLE_CLI_USAGE_WARM_UP_CACHE_OXCACHE,
        MSG_EXAMPLE_CLI_USAGE_CLEANING_UP,
        MSG_EXAMPLE_CLI_USAGE_CACHE_CLEARED,
        MSG_EXAMPLE_CLI_USAGE_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_COMPRESSION_ORIGINAL_SIZE_BYTES,
        MSG_EXAMPLE_COMPRESSION_SERIALIZED_BYTES,
        MSG_EXAMPLE_COMPRESSION_RATIO_X,
        MSG_EXAMPLE_COMPRESSION_SERIALIZATION_TOOK,
        MSG_EXAMPLE_COMPRESSION_DESERIALIZATION_TOOK,
        MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE,
        MSG_EXAMPLE_COMPRESSION_1_SMALL_DATA_100,
        MSG_EXAMPLE_COMPRESSION_2_MEDIUM_DATA_100B,
        MSG_EXAMPLE_COMPRESSION_3_LARGE_DATA_1KB,
        MSG_EXAMPLE_COMPRESSION_4_REPEATED_DATA_HIGH,
        MSG_EXAMPLE_COMPRESSION_5_USING_COMPRESSION_IN,
        MSG_EXAMPLE_COMPRESSION_STORING_COMPRESSED_DATA_BYTES,
        MSG_EXAMPLE_COMPRESSION_READING_AND_DECOMPRESSING_BYTES,
        MSG_EXAMPLE_COMPRESSION_DATA_INTEGRITY_VERIFIED,
        MSG_EXAMPLE_COMPRESSION_6_SMART_COMPRESSION_STRATEGY,
        MSG_EXAMPLE_COMPRESSION_OXCACHE_PICKS_THE_COMPRESSION,
        MSG_EXAMPLE_COMPRESSION_100_BYTES_NO_COMPRESSION,
        MSG_EXAMPLE_COMPRESSION_100B_1KB_FAST_COMPRESSION,
        MSG_EXAMPLE_COMPRESSION_1KB_100KB_MEDIUM_COMPRESSION,
        MSG_EXAMPLE_COMPRESSION_100KB_HIGH_COMPRESSION_RATIO,
        MSG_EXAMPLE_COMPRESSION_DATA_COMPRESSION_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_COMPRESSION_KEY_POINTS,
        MSG_EXAMPLE_COMPRESSION_ENABLE_COMPRESSION_WITH_JSONSERIALIZER,
        MSG_EXAMPLE_COMPRESSION_SMALL_DATA_IS_LEFT,
        MSG_EXAMPLE_COMPRESSION_REPEATED_DATA_COMPRESSES_WELL,
        MSG_EXAMPLE_COMPRESSION_DECOMPRESSION_IS_TRANSPARENT,
        MSG_EXAMPLE_EVENTS_OXCACHE_EVENT_SYSTEM_EXAMPLE,
        MSG_EXAMPLE_EVENTS_1_CREATING_AND_PUBLISHING,
        MSG_EXAMPLE_EVENTS_2_CONVENIENCE_METHODS,
        MSG_EXAMPLE_EVENTS_3_MONITORING_CACHE_OPERATION,
        MSG_EXAMPLE_EVENTS_SET_OPERATION_TOOK,
        MSG_EXAMPLE_EVENTS_4_BATCH_OPERATION_EVENTS,
        MSG_EXAMPLE_EVENTS_5_CUSTOM_EVENTS,
        MSG_EXAMPLE_EVENTS_6_ALL_EVENT_TYPES,
        MSG_EXAMPLE_EVENTS_EVENT_TYPE,
        MSG_EXAMPLE_EVENTS_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_I18N_INTERNATIONALIZATION_I18N_EXAMPLE,
        MSG_EXAMPLE_I18N_1_CREATING_FORMATTERS,
        MSG_EXAMPLE_I18N_EN_US_FORMATTER_CREATED,
        MSG_EXAMPLE_I18N_ZH_CN_FORMATTER_CREATED,
        MSG_EXAMPLE_I18N_DE_DE_FORMATTER_CREATED,
        MSG_EXAMPLE_I18N_UNEXPECTED_AN_INVALID_LOCALE,
        MSG_EXAMPLE_I18N_INVALID_LOCALE_CORRECTLY_ERRORED,
        MSG_EXAMPLE_I18N_2_NUMBER_FORMATTING,
        MSG_EXAMPLE_I18N_RAW_VALUE,
        MSG_EXAMPLE_I18N_NAN_CORRECTLY_ERRORED,
        MSG_EXAMPLE_I18N_3_PLURAL_RULES,
        MSG_EXAMPLE_I18N_4_DATE_FORMATTING,
        MSG_EXAMPLE_I18N_EXPIRY_DATE,
        MSG_EXAMPLE_I18N_5_CACHE_KEY_GENERATION,
        MSG_EXAMPLE_I18N_6_KEY_COMPARISON_COLLATION,
        MSG_EXAMPLE_I18N_ORIGINAL_ORDER,
        MSG_EXAMPLE_I18N_EN_US_COLLATION,
        MSG_EXAMPLE_I18N_7_MULTILINGUAL_CACHE_STATISTICS,
        MSG_EXAMPLE_I18N_CACHE_HITS,
        MSG_EXAMPLE_I18N_CACHE_MISSES,
        MSG_EXAMPLE_I18N_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_KIT_INTEGRATION_1_BUILDING_ASYNCKIT_OXCACHEMODULE,
        MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEBUILDOBSERVER,
        MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_OXCACHEMODULE,
        MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THE_COUNTINGDECORATOR_DECORAT,
        MSG_EXAMPLE_KIT_INTEGRATION_ASYNCKIT_BUILD_COMPLETE,
        MSG_EXAMPLE_KIT_INTEGRATION_2_USING_THE_CACHE,
        MSG_EXAMPLE_KIT_INTEGRATION_GET_USER_999_SHOULD,
        MSG_EXAMPLE_KIT_INTEGRATION_OPERATION_COUNT_SHOULD_BE,
        MSG_EXAMPLE_KIT_INTEGRATION_3_THREE_PHASE_SHUTDOWN,
        MSG_EXAMPLE_KIT_INTEGRATION_REGISTERED_THREE_PHASE_SHUTDOWN,
        MSG_EXAMPLE_KIT_INTEGRATION_DRAINQUEUE_HEALTH_CHECK_LIVENESS,
        MSG_EXAMPLE_KIT_INTEGRATION_SHUTDOWN_RESULT,
        MSG_EXAMPLE_KIT_INTEGRATION_DONE,
        MSG_EXAMPLE_KIT_INTEGRATION_TRAIT_KIT_INTEGRATION_DEMO,
        MSG_EXAMPLE_METRICS_AND_OBSERVABILITY_EXAMPLE,
        MSG_EXAMPLE_METRICS_1_CREATING_A_CACHE,
        MSG_EXAMPLE_METRICS_PERFORMED_20_WRITES_AND,
        MSG_EXAMPLE_METRICS_2_BACKEND_STATISTICS,
        MSG_EXAMPLE_METRICS_BACKEND_STATISTICS,
        MSG_EXAMPLE_METRICS_3_ENHANCED_STATISTICS_CACHESTATS,
        MSG_EXAMPLE_METRICS_L1_HITS,
        MSG_EXAMPLE_METRICS_L1_MISSES,
        MSG_EXAMPLE_METRICS_L2_HITS,
        MSG_EXAMPLE_METRICS_L2_MISSES,
        MSG_EXAMPLE_METRICS_L1_WRITES,
        MSG_EXAMPLE_METRICS_L2_WRITES,
        MSG_EXAMPLE_METRICS_TOTAL_OPERATIONS,
        MSG_EXAMPLE_METRICS_L1_ENTRIES,
        MSG_EXAMPLE_METRICS_4_HIT_RATE,
        MSG_EXAMPLE_METRICS_L1_HIT_RATE,
        MSG_EXAMPLE_METRICS_L2_HIT_RATE,
        MSG_EXAMPLE_METRICS_OVERALL_HIT_RATE,
        MSG_EXAMPLE_METRICS_5_PROMETHEUS_FORMAT_EXPORT,
        MSG_EXAMPLE_METRICS_6_JSON_FORMAT_EXPORT,
        MSG_EXAMPLE_METRICS_7_CACHESTATS_INSTANCE_METHOD,
        MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_PROMETHEUS_FIRST,
        MSG_EXAMPLE_METRICS_CACHESTATS_EXPORT_JSON_FIRST,
        MSG_EXAMPLE_METRICS_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_SECURITY_OXCACHE_SECURITY_FEATURES_EXAMPLE,
        MSG_EXAMPLE_SECURITY_1_SENSITIVE_DATA_REDACTION,
        MSG_EXAMPLE_SECURITY_RAW_PASSWORD,
        MSG_EXAMPLE_SECURITY_REDACTED,
        MSG_EXAMPLE_SECURITY_CONNECTION_STRING_REDACTION,
        MSG_EXAMPLE_SECURITY_RAW_URL,
        MSG_EXAMPLE_SECURITY_REDACTED_2,
        MSG_EXAMPLE_SECURITY_SENSITIVE_CACHE_KEY_REDACTION,
        MSG_EXAMPLE_SECURITY_FIELD_VALUE_REDACTION,
        MSG_EXAMPLE_SECURITY_2_THE_REDACTED_WRAPPER,
        MSG_EXAMPLE_SECURITY_REDACTED_WRAPPER,
        MSG_EXAMPLE_SECURITY_CUSTOM_VISIBLE_CHARACTERS,
        MSG_EXAMPLE_SECURITY_3_SAFE_LOG_HANDLING,
        MSG_EXAMPLE_SECURITY_RAW,
        MSG_EXAMPLE_SECURITY_SAFE,
        MSG_EXAMPLE_SECURITY_4_CACHE_OPERATION_SAFETY,
        MSG_EXAMPLE_SECURITY_STORING_THE_USER_SESSION,
        MSG_EXAMPLE_SECURITY_DATA_READ_BACK_BYTES,
        MSG_EXAMPLE_SECURITY_5_SECURITY_BEST_PRACTICES,
        MSG_EXAMPLE_SECURITY_NEVER_LOG_SENSITIVE_INFORMATION,
        MSG_EXAMPLE_SECURITY_USE_THE_REDACTION_MODULE,
        MSG_EXAMPLE_SECURITY_ENCRYPT_SENSITIVE_DATA_BEFORE,
        MSG_EXAMPLE_SECURITY_USE_SECURE_CONNECTION_STRING,
        MSG_EXAMPLE_SECURITY_ROTATE_SENSITIVE_TOKENS_IN,
        MSG_EXAMPLE_SECURITY_KEEP_CACHE_KEY_NAMES,
        MSG_EXAMPLE_SECURITY_6_CONFIGURATION_SAFETY,
        MSG_EXAMPLE_SECURITY_UNSAFE_STORING_PASSWORDS_IN,
        MSG_EXAMPLE_SECURITY_SAFE_USE_ENVIRONMENT_VARIABLES,
        MSG_EXAMPLE_SECURITY_SAFE_HIDE_CONNECTION_STRING,
        MSG_EXAMPLE_SECURITY_LOG_CONNECTING_TO,
        MSG_EXAMPLE_SECURITY_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_SECURITY_VALIDATION_OXCACHE_SECURITY_VALIDATION_EXAMPLE,
        MSG_EXAMPLE_SECURITY_VALIDATION_1_REDIS_KEY_VALIDATION,
        MSG_EXAMPLE_SECURITY_VALIDATION_VALID,
        MSG_EXAMPLE_SECURITY_VALIDATION_INVALID,
        MSG_EXAMPLE_SECURITY_VALIDATION_COMMAND_INJECTION_CHARACTER_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED,
        MSG_EXAMPLE_SECURITY_VALIDATION_SQL_INJECTION_PATTERN_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_2,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_2,
        MSG_EXAMPLE_SECURITY_VALIDATION_PATH_TRAVERSAL_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_3,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_3,
        MSG_EXAMPLE_SECURITY_VALIDATION_CONTROL_CHARACTER_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_4,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_4,
        MSG_EXAMPLE_SECURITY_VALIDATION_2_LUA_SCRIPT_VALIDATION,
        MSG_EXAMPLE_SECURITY_VALIDATION_VALID_SCRIPT_PASSED_VALIDATION,
        MSG_EXAMPLE_SECURITY_VALIDATION_ERROR,
        MSG_EXAMPLE_SECURITY_VALIDATION_DANGEROUS_COMMAND_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_5,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_5,
        MSG_EXAMPLE_SECURITY_VALIDATION_INFINITE_LOOP_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_6,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_6,
        MSG_EXAMPLE_SECURITY_VALIDATION_LENGTH_AND_KEY_COUNT,
        MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_SCRIPT_PASSED,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_SCRIPT,
        MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_KEY_COUNT_PASSED,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_KEY_COUNT,
        MSG_EXAMPLE_SECURITY_VALIDATION_3_SCAN_PATTERN_VALIDATION,
        MSG_EXAMPLE_SECURITY_VALIDATION_VALID_2,
        MSG_EXAMPLE_SECURITY_VALIDATION_INVALID_2,
        MSG_EXAMPLE_SECURITY_VALIDATION_EXCESS_WILDCARD_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_7,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_EXCESS_WILDCARDS_10,
        MSG_EXAMPLE_SECURITY_VALIDATION_OVERSIZED_PATTERN_DETECTION,
        MSG_EXAMPLE_SECURITY_VALIDATION_PASSED_8,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_AN_OVERSIZED_PATTERN,
        MSG_EXAMPLE_SECURITY_VALIDATION_4_SCAN_COUNT_LIMITS,
        MSG_EXAMPLE_SECURITY_VALIDATION_RAW_CLAMPED,
        MSG_EXAMPLE_SECURITY_VALIDATION_5_SAFE_LOGGING_LOG,
        MSG_EXAMPLE_SECURITY_VALIDATION_KEY,
        MSG_EXAMPLE_SECURITY_VALIDATION_6_IN_PRACTICE_A,
        MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUPPLIED,
        MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_7,
        MSG_EXAMPLE_SECURITY_VALIDATION_VALIDATING_A_USER_SUBMITTED,
        MSG_EXAMPLE_SECURITY_VALIDATION_ACCEPTED_SCRIPT,
        MSG_EXAMPLE_SECURITY_VALIDATION_REJECTED_SCRIPT,
        MSG_EXAMPLE_SECURITY_VALIDATION_EXAMPLE_COMPLETE,
        MSG_EXAMPLE_SECURITY_VALIDATION_BEST_PRACTICES,
        MSG_EXAMPLE_SECURITY_VALIDATION_1_EVERY_USER_SUPPLIED,
        MSG_EXAMPLE_SECURITY_VALIDATION_2_EVERY_USER_SUBMITTED,
        MSG_EXAMPLE_SECURITY_VALIDATION_3_EVERY_SCAN_PATTERN,
        MSG_EXAMPLE_SECURITY_VALIDATION_4_EVERY_SCAN_COUNT,
        MSG_EXAMPLE_SECURITY_VALIDATION_5_USE_LOG_CACHE,
        MSG_BENCH_SKIP_SERVICE_UNREACHABLE,
    ];

    // === Guard: EN/ZH key parity ===

    #[test]
    fn test_en_zh_key_parity() {
        let mut en = ftl_keys(EN_FTL);
        let mut zh = ftl_keys(ZH_FTL);
        en.sort_unstable();
        zh.sort_unstable();
        assert!(
            !en.is_empty(),
            "catalog must not be empty: en={} zh={}",
            en.len(),
            zh.len()
        );
        assert_eq!(en, zh, "EN and ZH catalogs must define the same keys");
    }

    // Guard: every message ID resolves in both bundles (validates FTL
    // parsing of the dashed keys)

    #[test]
    fn test_all_message_ids_resolve_in_both_bundles() {
        for id in ALL_MESSAGE_IDS {
            let key = ftl_key(id);
            assert!(
                EN_BUNDLE
                    .get_or_init(build_en_bundle)
                    .get_message(&key)
                    .is_some(),
                "message ID '{id}' missing from EN catalog"
            );
            assert!(
                ZH_BUNDLE
                    .get_or_init(build_zh_bundle)
                    .get_message(&key)
                    .is_some(),
                "message ID '{id}' missing from ZH catalog"
            );
        }
    }

    // === Guard: embedded FTL covers the locales/ directory (sync guard) ===

    #[test]
    fn test_embedded_ftl_matches_locales_dir() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        for (lang, embedded) in [("en", EN_FTL), ("zh", ZH_FTL)] {
            let dir = base.join(lang);
            let mut files: Vec<String> = std::fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("locales/{lang} unreadable: {e}"))
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            files.sort();
            assert_eq!(
                files,
                vec!["messages.ftl".to_string()],
                "locales/{lang} contains files not embedded in the catalog"
            );

            let disk = std::fs::read_to_string(dir.join("messages.ftl"))
                .unwrap_or_else(|e| panic!("locales/{lang}/messages.ftl unreadable: {e}"));
            assert_eq!(embedded, disk, "embedded '{lang}' FTL drifted from disk");
        }
    }

    // === Guard: fallbacks (unknown lang → en; missing key → key/None, no panic) ===

    #[test]
    fn test_unknown_lang_falls_back_to_en() {
        let msg = lookup("ar", MSG_ERR_NOT_FOUND, &[("detail", "user:42")])
            .expect("unknown lang must fall back to EN bundle");
        assert!(
            msg.contains("Key not found: user:42"),
            "unknown lang should render the EN message: got '{msg}'"
        );
    }

    #[test]
    fn test_missing_key_returns_none() {
        assert!(lookup("en", "error.does_not_exist", &[]).is_none());
        assert!(lookup("zh-CN", "error.does_not_exist", &[]).is_none());
    }

    #[test]
    fn test_t_missing_key_returns_key_without_panicking() {
        assert_eq!(t("no.such_key", &[]), "no.such_key");
    }

    // === Formatting behaviour (direct, per-language — no global locale state) ===

    #[test]
    fn test_lookup_en_with_args() {
        let msg = lookup("en", MSG_ERR_NOT_FOUND, &[("detail", "user:42")]).expect("en message");
        assert!(
            msg.contains("Key not found: user:42"),
            "en message: got '{msg}'"
        );
    }

    #[test]
    fn test_lookup_zh_with_args() {
        let msg = lookup("zh-CN", MSG_ERR_NOT_FOUND, &[("detail", "user:42")]).expect("zh message");
        assert!(msg.contains("键未找到：user:42"), "zh message: got '{msg}'");
    }

    #[test]
    fn test_lookup_key_too_long_both_langs() {
        let args = &[("actual", "600"), ("max", "512")];
        let en = lookup("en", MSG_ERR_KEY_TOO_LONG, args).expect("en message");
        assert!(
            en.contains("600") && en.contains("512"),
            "en key-too-long should contain sizes: got '{en}'"
        );
        let zh = lookup("zh-CN", MSG_ERR_KEY_TOO_LONG, args).expect("zh message");
        assert!(
            zh.contains("键过长") && zh.contains("600") && zh.contains("512"),
            "zh key-too-long should contain sizes: got '{zh}'"
        );
    }

    #[test]
    fn test_macro_service_not_registered_message() {
        let args = &[("service", "strict_svc")];
        let en = lookup("en", "macro.service_not_registered", args).expect("en macro message");
        assert_eq!(
            en,
            "oxcache: service 'strict_svc' not registered (strict mode)"
        );
        let zh = lookup("zh-CN", "macro.service_not_registered", args).expect("zh macro message");
        assert_eq!(zh, "oxcache：服务 'strict_svc' 未注册（严格模式）");
    }
}
