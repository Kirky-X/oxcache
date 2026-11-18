// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 安全配置自审计（`doctor` feature）
//!
//! [`audit`] 盘点两类安全事实：编译期 feature（能力是否存在）与运行时
//! [`CacheConfig`](crate::config::CacheConfig)（配置是否启用防护）。输出
//! [`SecurityReport`] 为结构化数据，消费方可程序化判定生产就绪度；
//! [`SecurityReport::render`] 提供人可读的多行文本。
//!
//! 本模块为纯函数：不写状态、不发网络请求、不改配置。

use crate::config::CacheConfig;

/// 审计发现级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingLevel {
    /// 防护到位
    Ok,
    /// 缺一项防护，值得关注
    Warning,
    /// 生产就绪阻断项
    Critical,
}

/// 单个安全能力域的审计结果
#[derive(Debug, Clone)]
pub struct SecurityFinding {
    /// 发现级别
    pub level: FindingLevel,
    /// 能力域标识（如 `value-encryption`）
    pub capability: &'static str,
    /// 人可读的判定详情
    pub detail: String,
}

impl SecurityFinding {
    fn new(level: FindingLevel, capability: &'static str, detail: impl Into<String>) -> Self {
        Self {
            level,
            capability,
            detail: detail.into(),
        }
    }
}

/// 结构化审计报告
#[derive(Debug, Clone, Default)]
pub struct SecurityReport {
    /// 全部审计发现
    pub findings: Vec<SecurityFinding>,
}

impl SecurityReport {
    /// 无 Critical 项即视为生产就绪
    pub fn is_production_ready(&self) -> bool {
        self.critical_count() == 0
    }

    /// Critical 项数量
    pub fn critical_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.level == FindingLevel::Critical)
            .count()
    }

    /// Warning 项数量
    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.level == FindingLevel::Warning)
            .count()
    }

    /// 渲染为多行文本：首行汇总，随后按 Critical → Warning → Ok 排序
    pub fn render(&self) -> String {
        let mut lines = Vec::with_capacity(self.findings.len() + 1);
        let ready = if self.is_production_ready() {
            "READY"
        } else {
            "NOT READY"
        };
        lines.push(format!(
            "oxcache security audit: {ready} ({} critical, {} warnings)",
            self.critical_count(),
            self.warning_count()
        ));
        let mut ordered: Vec<&SecurityFinding> = self.findings.iter().collect();
        ordered.sort_by_key(|f| match f.level {
            FindingLevel::Critical => 0,
            FindingLevel::Warning => 1,
            FindingLevel::Ok => 2,
        });
        for finding in ordered {
            let marker = match finding.level {
                FindingLevel::Ok => "[OK]",
                FindingLevel::Warning => "[WARN]",
                FindingLevel::Critical => "[CRIT]",
            };
            lines.push(format!(
                "{marker} {}: {}",
                finding.capability, finding.detail
            ));
        }
        lines.join("\n")
    }
}

/// 盘点编译期 feature 与运行时配置，产出结构化安全报告
pub fn audit(config: &CacheConfig) -> SecurityReport {
    let mut findings = Vec::new();

    // 编译期能力盘点
    if cfg!(feature = "encrypt") {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "value-encryption",
            "encrypt feature compiled; values can be encrypted at rest",
        ));
    } else {
        findings.push(SecurityFinding::new(
            FindingLevel::Critical,
            "value-encryption",
            "encrypt feature not compiled; values are stored in plaintext",
        ));
    }
    if cfg!(feature = "integrity") {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "tamper-detection",
            "integrity feature compiled; value tampering is detectable",
        ));
    } else {
        findings.push(SecurityFinding::new(
            FindingLevel::Warning,
            "tamper-detection",
            "integrity feature not compiled; value tampering is undetectable",
        ));
    }
    if cfg!(feature = "audit") {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "audit-trail",
            "audit feature compiled; security events can be traced",
        ));
    } else {
        findings.push(SecurityFinding::new(
            FindingLevel::Warning,
            "audit-trail",
            "audit feature not compiled; security events cannot be traced",
        ));
    }
    if cfg!(feature = "bloom") {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "negative-query-guard",
            "bloom feature compiled; negative queries can be short-circuited",
        ));
    } else {
        findings.push(SecurityFinding::new(
            FindingLevel::Warning,
            "negative-query-guard",
            "bloom feature not compiled; negative queries lack a short-circuit guard",
        ));
    }
    if !cfg!(feature = "lua") && config.redis_url.is_some() {
        findings.push(SecurityFinding::new(
            FindingLevel::Warning,
            "lua-sandbox",
            "lua feature not compiled while Redis is configured; EVAL scripts are unguarded",
        ));
    } else if cfg!(feature = "lua") {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "lua-sandbox",
            "lua feature compiled; script validation is available",
        ));
    }

    // 运行时配置盘点：TLS 强制
    let redis_url = config.redis_url.as_deref();
    if let Some(url) = redis_url {
        if !cfg!(feature = "redis") {
            findings.push(SecurityFinding::new(
                FindingLevel::Critical,
                "tls-enforcement",
                "redis_url is set but the redis feature is not compiled",
            ));
        } else if std::env::var_os("OXCACHE_ALLOW_INSECURE_REDIS").is_some() {
            findings.push(SecurityFinding::new(
                FindingLevel::Critical,
                "tls-enforcement",
                "OXCACHE_ALLOW_INSECURE_REDIS escape hatch is set; TLS enforcement is disabled",
            ));
        } else if !url.starts_with("rediss://") {
            findings.push(SecurityFinding::new(
                FindingLevel::Critical,
                "tls-enforcement",
                "redis_url does not use the rediss:// scheme; traffic is not TLS-enforced",
            ));
        } else {
            findings.push(SecurityFinding::new(
                FindingLevel::Ok,
                "tls-enforcement",
                "redis_url uses rediss:// and the insecure escape hatch is unset",
            ));
        }
    }

    if config.null_cache_ttl.is_none() {
        findings.push(SecurityFinding::new(
            FindingLevel::Warning,
            "null-sentinel",
            "null_cache_ttl is not set; cache-penetration guard is incomplete",
        ));
    } else {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "null-sentinel",
            "null_cache_ttl is set; cache-penetration guard is active",
        ));
    }
    if config.ttl_jitter_factor.is_none() {
        findings.push(SecurityFinding::new(
            FindingLevel::Warning,
            "ttl-jitter",
            "ttl_jitter_factor is not set; synchronized expiry may cause stampede",
        ));
    } else {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "ttl-jitter",
            "ttl_jitter_factor is set; expiry is jittered",
        ));
    }
    if config.metrics_enabled == Some(false) {
        findings.push(SecurityFinding::new(
            FindingLevel::Warning,
            "security-metrics",
            "metrics are explicitly disabled; security counters are not observable",
        ));
    } else {
        findings.push(SecurityFinding::new(
            FindingLevel::Ok,
            "security-metrics",
            "metrics are enabled or left at default; security counters are observable",
        ));
    }

    SecurityReport { findings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CacheConfigBuilder;

    fn finding<'a>(report: &'a SecurityReport, capability: &str) -> Option<&'a SecurityFinding> {
        report.findings.iter().find(|f| f.capability == capability)
    }

    #[test]
    fn empty_report_is_ready_with_zero_counts() {
        let report = SecurityReport::default();
        assert!(report.is_production_ready());
        assert_eq!(report.critical_count(), 0);
        assert_eq!(report.warning_count(), 0);
        let rendered = report.render();
        assert!(rendered.lines().next().is_some_and(|l| l.contains("READY")));
    }

    #[test]
    fn critical_makes_report_not_ready() {
        let report = SecurityReport {
            findings: vec![SecurityFinding::new(
                FindingLevel::Critical,
                "test-critical",
                "broken",
            )],
        };
        assert!(!report.is_production_ready());
        assert_eq!(report.critical_count(), 1);
    }

    #[test]
    fn counts_and_render_markers() {
        let report = SecurityReport {
            findings: vec![
                SecurityFinding::new(FindingLevel::Warning, "w1", "warn detail"),
                SecurityFinding::new(FindingLevel::Ok, "o1", "ok detail"),
                SecurityFinding::new(FindingLevel::Critical, "c1", "crit detail"),
                SecurityFinding::new(FindingLevel::Warning, "w2", "warn2 detail"),
            ],
        };
        assert_eq!(report.critical_count(), 1);
        assert_eq!(report.warning_count(), 2);
        let rendered = report.render();
        let lines: Vec<&str> = rendered.lines().collect();
        assert!(lines[0].contains("NOT READY"));
        assert!(lines[0].contains("1 critical"));
        assert!(lines[0].contains("2 warnings"));
        assert!(lines[1].starts_with("[CRIT] c1:"));
        assert!(lines[2].starts_with("[WARN]"));
        assert!(lines[3].starts_with("[WARN]"));
        assert!(lines[4].starts_with("[OK]"));
    }

    #[test]
    fn compile_time_findings_present_in_default_features() {
        // 默认 feature 组合不含 encrypt/integrity/audit/bloom
        let config = CacheConfig::default();
        let report = audit(&config);
        for capability in [
            "value-encryption",
            "tamper-detection",
            "audit-trail",
            "negative-query-guard",
        ] {
            assert!(
                finding(&report, capability).is_some(),
                "missing {capability}"
            );
        }
        let enc_level = if cfg!(feature = "encrypt") {
            FindingLevel::Ok
        } else {
            FindingLevel::Critical
        };
        assert_eq!(
            finding(&report, "value-encryption").unwrap().level,
            enc_level
        );
        for (capability, feature_on) in [
            ("tamper-detection", cfg!(feature = "integrity")),
            ("audit-trail", cfg!(feature = "audit")),
            ("negative-query-guard", cfg!(feature = "bloom")),
        ] {
            let level = if feature_on {
                FindingLevel::Ok
            } else {
                FindingLevel::Warning
            };
            assert_eq!(finding(&report, capability).unwrap().level, level);
        }
    }

    #[test]
    #[serial_test::serial]
    #[allow(unsafe_code)]
    fn production_intent_config_enables_runtime_guards() {
        // SAFETY: Rust 2024 edition — remove_var 为 unsafe；本测试以 #[serial] 串行，进程级 env 无并发读写
        unsafe { std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS") };
        let config = CacheConfigBuilder::default()
            .redis_url("rediss://cache.example.com:6379")
            .null_cache_ttl(std::time::Duration::from_secs(30))
            .ttl_jitter_factor(0.2)
            .build();
        let report = audit(&config);
        if cfg!(feature = "redis") && cfg!(feature = "lua") {
            assert_eq!(
                finding(&report, "tls-enforcement").unwrap().level,
                FindingLevel::Ok
            );
            assert_eq!(
                finding(&report, "lua-sandbox").map(|f| f.level),
                Some(FindingLevel::Ok)
            );
        }
        assert_eq!(
            finding(&report, "null-sentinel").unwrap().level,
            FindingLevel::Ok
        );
        assert_eq!(
            finding(&report, "ttl-jitter").unwrap().level,
            FindingLevel::Ok
        );
        assert_eq!(
            finding(&report, "security-metrics").unwrap().level,
            FindingLevel::Ok
        );
    }

    #[test]
    #[serial_test::serial]
    #[allow(unsafe_code)]
    fn insecure_escape_hatch_is_critical_even_with_rediss() {
        // SAFETY: Rust 2024 edition — set_var 为 unsafe；本测试以 #[serial] 串行
        unsafe { std::env::set_var("OXCACHE_ALLOW_INSECURE_REDIS", "I_UNDERSTAND_THE_RISKS") };
        let config = CacheConfigBuilder::default()
            .redis_url("rediss://cache.example.com:6379")
            .build();
        let report = audit(&config);
        assert_eq!(
            finding(&report, "tls-enforcement").unwrap().level,
            FindingLevel::Critical
        );
        // SAFETY: 同上，串行清理进程级 env
        unsafe { std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS") };
    }

    #[test]
    #[serial_test::serial]
    #[allow(unsafe_code)]
    fn insecure_redis_url_is_critical() {
        // SAFETY: Rust 2024 edition — remove_var 为 unsafe；本测试以 #[serial] 串行，进程级 env 无并发读写
        unsafe { std::env::remove_var("OXCACHE_ALLOW_INSECURE_REDIS") };
        let config = CacheConfigBuilder::default()
            .redis_url("redis://cache.example.com:6379")
            .build();
        let report = audit(&config);
        assert_eq!(
            finding(&report, "tls-enforcement").unwrap().level,
            FindingLevel::Critical
        );
    }
}
