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

// ============================================================================
// Message ID constants
// ============================================================================

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
pub const MSG_DETAIL_CONFERS_EXPECTS_U64: &str = "detail.confers.expects_u64";
pub const MSG_DETAIL_CONFERS_VALUE_EXCEEDS_RANGE: &str = "detail.confers.value_exceeds_range";
pub const MSG_DETAIL_CONFERS_EXPECTS_BOOL: &str = "detail.confers.expects_bool";
pub const MSG_DETAIL_CONFERS_EXPECTS_F64: &str = "detail.confers.expects_f64";
pub const MSG_DETAIL_CONFERS_EXPECTS_STRING: &str = "detail.confers.expects_string";

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

// ============================================================================
// Catalog lookup
// ============================================================================

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

// ============================================================================
// Fluent bundle management
// ============================================================================

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

// ============================================================================
// Tests
// ============================================================================

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
        MSG_DETAIL_CONFERS_EXPECTS_U64,
        MSG_DETAIL_CONFERS_VALUE_EXCEEDS_RANGE,
        MSG_DETAIL_CONFERS_EXPECTS_BOOL,
        MSG_DETAIL_CONFERS_EXPECTS_F64,
        MSG_DETAIL_CONFERS_EXPECTS_STRING,
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
    ];

    // --------------------------------------------------------------------
    // Guard: EN/ZH key parity
    // --------------------------------------------------------------------

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

    // --------------------------------------------------------------------
    // Guard: every message ID resolves in both bundles (validates FTL
    // parsing of the dashed keys)
    // --------------------------------------------------------------------

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

    // --------------------------------------------------------------------
    // Guard: embedded FTL covers the locales/ directory (sync guard)
    // --------------------------------------------------------------------

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

    // --------------------------------------------------------------------
    // Guard: fallbacks (unknown lang → en; missing key → key/None, no panic)
    // --------------------------------------------------------------------

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

    // --------------------------------------------------------------------
    // Formatting behaviour (direct, per-language — no global locale state)
    // --------------------------------------------------------------------

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
