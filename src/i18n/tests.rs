#![allow(clippy::module_inception)]
#[allow(unused_imports)]
pub use super::*;

#[cfg(test)]
mod tests {
    // locale 探测测试操作进程级 env（LANG/LC_ALL/LC_MESSAGES），
    // 并行执行互相污染 → 以下 5 个测试共用互斥锁。
    // 修改进程级默认 locale（DEFAULT_LOCALE 全局单例）的测试也必须持锁：
    // 并行测试互相覆盖会让 Display 语言随机（经典 flaky）。
    static LOCALE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_locale_parsing_en() {
        let fmt = CacheI18nFormatter::new("en-US");
        assert!(fmt.is_ok(), "en-US should parse successfully");
    }

    #[test]
    fn test_locale_parsing_zh() {
        let fmt = CacheI18nFormatter::new("zh-CN");
        assert!(fmt.is_ok(), "zh-CN should parse successfully");
    }

    #[test]
    fn test_invalid_locale() {
        let result = CacheI18nFormatter::new("not-a-valid-locale!!!");
        assert!(result.is_err(), "invalid locale should return error");
        match result.err().unwrap() {
            I18nError::InvalidLocale { input, .. } => assert_eq!(input, "not-a-valid-locale!!!"),
            other => panic!("expected InvalidLocale, got {other:?}"),
        }
    }

    #[test]
    fn test_format_count() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        assert_eq!(
            fmt.format_count(1).expect("plural 1"),
            "One",
            "en: count=1 should be One"
        );
        assert_eq!(
            fmt.format_count(2).expect("plural 2"),
            "Other",
            "en: count=2 should be Other"
        );
    }

    #[test]
    fn test_format_number_en() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let result = fmt.format_number(1_234_567.89_f64).expect("format number");
        // en-US: thousands separator is comma, decimal separator is period
        assert!(
            result.contains(','),
            "en-US number should contain thousands separator: got '{result}'"
        );
        assert!(
            result.contains('.'),
            "en-US number should contain decimal point: got '{result}'"
        );
    }

    #[test]
    fn test_format_number_not_finite() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        assert!(fmt.format_number(f64::NAN).is_err());
        assert!(fmt.format_number(f64::INFINITY).is_err());
    }

    #[test]
    fn test_format_cache_key() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let key = fmt.format_cache_key("user", 1234).expect("cache key");
        assert!(
            key.starts_with("user:"),
            "cache key should start with namespace: got '{key}'"
        );
        assert!(
            key.contains('1'),
            "cache key should contain the count: got '{key}'"
        );
    }

    #[test]
    fn test_compare_keys() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        assert_eq!(
            fmt.compare_keys("apple", "banana").expect("compare"),
            Ordering::Less,
            "apple < banana"
        );
        assert_eq!(
            fmt.compare_keys("banana", "apple").expect("compare"),
            Ordering::Greater,
            "banana > apple"
        );
        assert_eq!(
            fmt.compare_keys("apple", "apple").expect("compare"),
            Ordering::Equal,
            "apple == apple"
        );
    }

    #[test]
    fn test_format_expiry() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let result = fmt.format_expiry(2026, 7, 11).expect("format expiry");
        assert!(
            result.contains("2026"),
            "expiry should contain year: got '{result}'"
        );
        assert!(
            !result.is_empty(),
            "expiry should be non-empty: got '{result}'"
        );
    }

    // ========================================================================
    // Message catalog tests
    // ========================================================================

    #[test]
    fn test_format_message_en_not_found() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_NOT_FOUND, &[("detail", "user:42")])
            .expect("format message");
        assert!(
            msg.contains("Key not found: user:42"),
            "en message should contain 'Key not found: user:42': got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_zh_not_found() {
        let fmt = CacheI18nFormatter::new("zh-CN").expect("zh-CN locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_NOT_FOUND, &[("detail", "user:42")])
            .expect("format message");
        assert!(
            msg.contains("键未找到：user:42"),
            "zh message should contain '键未找到：user:42': got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_en_key_too_long() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        let msg = fmt
            .format_message(
                messages::MSG_ERR_KEY_TOO_LONG,
                &[("actual", "600"), ("max", "512")],
            )
            .expect("format message");
        assert!(
            msg.contains("600") && msg.contains("512"),
            "en message should contain actual and max: got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_zh_key_too_long() {
        let fmt = CacheI18nFormatter::new("zh-CN").expect("zh-CN locale");
        let msg = fmt
            .format_message(
                messages::MSG_ERR_KEY_TOO_LONG,
                &[("actual", "600"), ("max", "512")],
            )
            .expect("format message");
        assert!(
            msg.contains("键过长") && msg.contains("600") && msg.contains("512"),
            "zh message should contain '键过长' with values: got '{msg}'"
        );
    }

    #[test]
    fn test_format_message_unknown_id_returns_id() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        let msg = fmt
            .format_message("unknown.message.id", &[])
            .expect("format message");
        assert_eq!(msg, "unknown.message.id", "unknown ID should return raw ID");
    }

    #[test]
    fn test_format_message_unsupported_locale_falls_back_to_en() {
        let fmt = CacheI18nFormatter::new("en-US").expect("en-US locale");
        let msg = fmt
            .format_message(messages::MSG_ERR_CONNECTION, &[("detail", "timeout")])
            .expect("format message");
        assert!(
            msg.contains("Connection error: timeout"),
            "unsupported locale should fall back to English: got '{msg}'"
        );
    }

    #[test]
    fn test_locale_getter() {
        let fmt = CacheI18nFormatter::new("zh-CN").expect("zh-CN locale");
        assert!(
            fmt.locale_tag().starts_with("zh"),
            "locale_tag should start with 'zh': got '{}'",
            fmt.locale_tag()
        );
        let expected: icu::locale::Locale = "zh-CN".parse().expect("valid tag");
        assert_eq!(fmt.locale(), &expected, "locale() returns the parsed tag");
    }

    /// 阿拉伯语复数规则覆盖 Zero/One/Two/Few/Many/Other 六类
    /// （en 只有 One/Other，覆盖不到 plural_category_name 的全部分支）
    #[test]
    fn test_format_count_arabic_plural_categories() {
        let fmt = CacheI18nFormatter::new("ar").expect("ar locale");
        let cases = [
            (0, "Zero"),
            (1, "One"),
            (2, "Two"),
            (3, "Few"),
            (11, "Many"),
            (100, "Other"),
        ];
        for (count, expected) in cases {
            assert_eq!(
                fmt.format_count(count).expect("plural category"),
                expected,
                "ar: count={count} should be {expected}"
            );
        }
    }

    /// 有限但超出 FixedDecimal 容量的数值（f64::MAX 约 309 位十进制）
    /// 必须返回 InvalidNumber 而非 panic
    #[test]
    fn test_format_number_finite_value_beyond_decimal_capacity() {
        let fmt = CacheI18nFormatter::new("en").expect("en locale");
        match fmt.format_number(f64::MAX) {
            Err(I18nError::InvalidNumber { input, .. }) => {
                assert!(!input.is_empty(), "input should echo the raw value");
            }
            Ok(formatted) => {
                // 实现侧 FixedDecimal 若能承载该量级，则格式化必须无失真告警路径
                assert!(
                    formatted.len() >= 300,
                    "unexpectedly short formatting of f64::MAX: '{formatted}'"
                );
            }
            other => panic!("unexpected result for f64::MAX: {other:?}"),
        }
    }

    #[test]
    fn test_i18n_error_message_id() {
        let err = I18nError::InvalidLocale {
            input: "bad".to_string(),
            reason: "parse failed".to_string(),
        };
        assert_eq!(err.message_id(), messages::MSG_I18N_INVALID_LOCALE);
    }

    #[test]
    fn test_i18n_error_message_id_number_and_format_variants() {
        let err = I18nError::InvalidNumber {
            input: "NaN".to_string(),
            reason: "not finite".to_string(),
        };
        assert_eq!(err.message_id(), messages::MSG_I18N_INVALID_NUMBER);
        let err = I18nError::FormatError("compiled data missing".to_string());
        assert_eq!(err.message_id(), messages::MSG_I18N_FORMAT_ERROR);
    }

    #[test]
    fn test_i18n_error_localized_message_locale_and_number_variants() {
        let err = I18nError::InvalidLocale {
            input: "not-a-locale!".to_string(),
            reason: "syntax error".to_string(),
        };
        let msg = err.localized_message("en");
        assert!(
            msg.contains("not-a-locale!") && msg.contains("syntax error"),
            "en InvalidLocale message should carry input and reason: got '{msg}'"
        );
        let err = I18nError::InvalidNumber {
            input: "Infinity".to_string(),
            reason: "value is not finite".to_string(),
        };
        let msg = err.localized_message("en");
        assert!(
            msg.contains("Infinity"),
            "en InvalidNumber message should carry input: got '{msg}'"
        );
    }

    #[test]
    fn test_i18n_error_localized_message_format_variant() {
        let err = I18nError::FormatError("decimal formatter unavailable".to_string());
        let msg = err.localized_message("en");
        assert!(
            msg.contains("decimal formatter unavailable"),
            "en FormatError message should carry detail: got '{msg}'"
        );
    }

    #[test]
    fn test_i18n_error_localized_message_en() {
        let err = I18nError::DateError("month out of range".to_string());
        let msg = err.localized_message("en");
        assert!(
            msg.contains("date error: month out of range"),
            "en I18nError message: got '{msg}'"
        );
    }

    #[test]
    fn test_i18n_error_localized_message_zh() {
        let err = I18nError::DateError("月份超出范围".to_string());
        let msg = err.localized_message("zh-CN");
        assert!(
            msg.contains("日期错误：月份超出范围"),
            "zh I18nError message: got '{msg}'"
        );
    }

    // ========================================================================
    // Global default locale tests
    // ========================================================================

    #[test]
    fn test_i18n_error_display_en() {
        let _guard = LOCALE_ENV_LOCK.lock();
        set_default_locale("en");
        let err = I18nError::DateError("month out of range".to_string());
        let s = err.to_string();
        assert!(
            s.contains("date error: month out of range"),
            "en I18nError Display: got '{s}'"
        );
    }

    #[test]
    fn test_i18n_error_display_zh() {
        let _guard = LOCALE_ENV_LOCK.lock();
        set_default_locale("zh-CN");
        let err = I18nError::DateError("月份超出范围".to_string());
        let s = err.to_string();
        assert!(
            s.contains("日期错误：月份超出范围"),
            "zh I18nError Display: got '{s}'"
        );
        set_default_locale("en");
    }

    #[test]
    fn test_set_get_default_locale() {
        let _guard = LOCALE_ENV_LOCK.lock();
        set_default_locale("en");
        assert_eq!(get_default_locale(), "en");
        set_default_locale("zh-CN");
        assert_eq!(get_default_locale(), "zh-CN");
        set_default_locale("en");
    }

    // ========================================================================
    // System locale detection tests
    // ========================================================================

    #[test]
    fn test_parse_locale_tag_en() {
        assert_eq!(super::parse_locale_tag("en_US.UTF-8"), "en-US");
    }

    #[test]
    fn test_parse_locale_tag_zh() {
        assert_eq!(super::parse_locale_tag("zh_CN.UTF-8"), "zh-CN");
    }

    #[test]
    fn test_parse_locale_tag_c() {
        assert_eq!(super::parse_locale_tag("C"), "C");
    }

    #[test]
    fn test_parse_locale_tag_posix() {
        assert_eq!(super::parse_locale_tag("POSIX"), "POSIX");
    }

    #[test]
    fn test_parse_locale_tag_with_modifier() {
        assert_eq!(super::parse_locale_tag("en_US.UTF-8@collation"), "en-US");
    }

    #[test]
    fn test_parse_locale_tag_simple() {
        assert_eq!(super::parse_locale_tag("fr"), "fr");
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_with_lang_zh() {
        let _locale_guard = LOCALE_ENV_LOCK.lock().unwrap();
        // Save original
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();
        let orig_oxcache_lang = std::env::var("OXCACHE_LANG").ok();

        // Clear higher-priority vars
        // SAFETY: test-only; serialised by `--test-threads=1` or env mutex in practice.
        unsafe {
            std::env::remove_var("OXCACHE_LANG");
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "zh_CN.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "zh-CN", "should detect zh-CN from LANG");

        // Restore
        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，回写块前保存的原值恢复进程环境。
        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
            if let Some(ref v) = orig_oxcache_lang {
                std::env::set_var("OXCACHE_LANG", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_with_lang_en() {
        let _locale_guard = LOCALE_ENV_LOCK.lock().unwrap();
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();
        let orig_oxcache_lang = std::env::var("OXCACHE_LANG").ok();

        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，清除高优先级 env 并设置测试 locale。
        unsafe {
            std::env::remove_var("OXCACHE_LANG");
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "en_US.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "en-US", "should detect en-US from LANG");

        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，回写块前保存的原值恢复进程环境。
        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
            if let Some(ref v) = orig_oxcache_lang {
                std::env::set_var("OXCACHE_LANG", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_c_fallback_to_en() {
        let _locale_guard = LOCALE_ENV_LOCK.lock().unwrap();
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();
        let orig_oxcache_lang = std::env::var("OXCACHE_LANG").ok();

        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，清除高优先级 env 并设置测试 locale。
        unsafe {
            std::env::remove_var("OXCACHE_LANG");
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "C");
        }

        let locale = detect_system_locale();
        assert_eq!(locale, "en", "C locale should fall back to en");

        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，回写块前保存的原值恢复进程环境。
        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
            if let Some(ref v) = orig_oxcache_lang {
                std::env::set_var("OXCACHE_LANG", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_unsupported_fallback_to_en() {
        let _locale_guard = LOCALE_ENV_LOCK.lock().unwrap();
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();
        let orig_oxcache_lang = std::env::var("OXCACHE_LANG").ok();

        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，清除高优先级 env 并设置测试 locale。
        unsafe {
            std::env::remove_var("OXCACHE_LANG");
            std::env::remove_var("LC_ALL");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "ja_JP.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(
            locale, "en",
            "unsupported locale (ja) should fall back to en"
        );

        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，回写块前保存的原值恢复进程环境。
        unsafe {
            std::env::remove_var("LANG");
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            }
            if let Some(ref v) = orig_oxcache_lang {
                std::env::set_var("OXCACHE_LANG", v);
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_lc_all_priority() {
        let _locale_guard = LOCALE_ENV_LOCK.lock().unwrap();
        let orig_lang = std::env::var("LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lc_messages = std::env::var("LC_MESSAGES").ok();
        let orig_oxcache_lang = std::env::var("OXCACHE_LANG").ok();

        // LC_ALL should take priority over LC_MESSAGES and LANG
        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，设置多个 locale env 验证优先级。
        unsafe {
            std::env::remove_var("OXCACHE_LANG");
            std::env::set_var("LC_ALL", "zh_CN.UTF-8");
            std::env::set_var("LC_MESSAGES", "en_US.UTF-8");
            std::env::set_var("LANG", "fr_FR.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(
            locale, "zh-CN",
            "LC_ALL should take priority: got '{locale}'"
        );

        // Restore
        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，回写块前保存的原值恢复进程环境。
        unsafe {
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            } else {
                std::env::remove_var("LC_ALL");
            }
            if let Some(ref v) = orig_lc_messages {
                std::env::set_var("LC_MESSAGES", v);
            } else {
                std::env::remove_var("LC_MESSAGES");
            }
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            } else {
                std::env::remove_var("LANG");
            }
            if let Some(ref v) = orig_oxcache_lang {
                std::env::set_var("OXCACHE_LANG", v);
            } else {
                std::env::remove_var("OXCACHE_LANG");
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_detect_system_locale_oxcache_lang_priority() {
        let _locale_guard = LOCALE_ENV_LOCK.lock().unwrap();
        let orig_oxcache_lang = std::env::var("OXCACHE_LANG").ok();
        let orig_lc_all = std::env::var("LC_ALL").ok();
        let orig_lang = std::env::var("LANG").ok();

        // OXCACHE_LANG should take priority over LC_ALL and LANG
        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，设置多个 locale env 验证优先级。
        unsafe {
            std::env::set_var("OXCACHE_LANG", "zh_CN.UTF-8");
            std::env::set_var("LC_ALL", "en_US.UTF-8");
            std::env::remove_var("LC_MESSAGES");
            std::env::set_var("LANG", "fr_FR.UTF-8");
        }

        let locale = detect_system_locale();
        assert_eq!(
            locale, "zh-CN",
            "OXCACHE_LANG should take priority: got '{locale}'"
        );

        // Empty OXCACHE_LANG must fall through the chain (LC_ALL wins)
        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，验证空值穿透。
        unsafe {
            std::env::set_var("OXCACHE_LANG", "");
        }
        let locale = detect_system_locale();
        assert_eq!(
            locale, "en-US",
            "empty OXCACHE_LANG should fall through to LC_ALL: got '{locale}'"
        );

        // Restore
        // SAFETY: edition 2024 下 set_var/remove_var 为 unsafe；测试单线程且持 LOCALE_ENV_LOCK，回写块前保存的原值恢复进程环境。
        unsafe {
            if let Some(ref v) = orig_oxcache_lang {
                std::env::set_var("OXCACHE_LANG", v);
            } else {
                std::env::remove_var("OXCACHE_LANG");
            }
            if let Some(ref v) = orig_lc_all {
                std::env::set_var("LC_ALL", v);
            } else {
                std::env::remove_var("LC_ALL");
            }
            if let Some(ref v) = orig_lang {
                std::env::set_var("LANG", v);
            } else {
                std::env::remove_var("LANG");
            }
        }
    }

    #[test]
    fn test_is_supported() {
        assert!(messages::is_supported("en"));
        assert!(messages::is_supported("zh"));
        assert!(!messages::is_supported("fr"));
        assert!(!messages::is_supported("ja"));
        assert!(!messages::is_supported("de"));
    }
}
