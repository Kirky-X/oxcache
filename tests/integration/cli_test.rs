#![allow(deprecated)]
// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// CLI 集成测试
//
// 测试 CLI 命令行工具的功能。

#[cfg(feature = "cli")]
#[cfg(feature = "l2-redis")]
mod tests {
    use oxcache::cli::{AdminArgs, AdminSubcommand, CleanArgs, MetricsArgs, StatusArgs, WarmupArgs};

    // ============================================================================
    // Status 命令测试
    // ============================================================================

    #[tokio::test]
    async fn test_status_args_parsing() {
        // 测试基本参数解析
        let args = StatusArgs {
            service: Some("test_service".to_string()),
            verbose: true,
        };

        assert_eq!(args.service, Some("test_service".to_string()));
        assert!(args.verbose);
    }

    #[tokio::test]
    async fn test_status_args_default() {
        // 测试默认参数
        let args = StatusArgs {
            service: None,
            verbose: false,
        };

        assert!(args.service.is_none());
        assert!(!args.verbose);
    }

    // ============================================================================
    // Admin 命令测试
    // ============================================================================

    #[tokio::test]
    async fn test_admin_clean_args_parsing() {
        let args = CleanArgs {
            service: "test_service".to_string(),
            l1: true,
            l2: true,
            wal: false,
            confirm: true,
        };

        assert_eq!(args.service, "test_service");
        assert!(args.l1);
        assert!(args.l2);
        assert!(!args.wal);
        assert!(args.confirm);
    }

    #[tokio::test]
    async fn test_admin_warmup_args_parsing() {
        let args = WarmupArgs {
            service: "test_service".to_string(),
            start: true,
            status: false,
            stop: false,
        };

        assert_eq!(args.service, "test_service");
        assert!(args.start);
        assert!(!args.status);
        assert!(!args.stop);
    }

    #[tokio::test]
    async fn test_admin_subcommand_variants() {
        // 测试 Clean 子命令
        let clean_args = CleanArgs {
            service: "test".to_string(),
            l1: true,
            l2: false,
            wal: false,
            confirm: false,
        };
        let admin_clean = AdminArgs {
            command: AdminSubcommand::Clean(clean_args),
        };
        assert!(matches!(admin_clean.command, AdminSubcommand::Clean(_)));

        // 测试 Warmup 子命令
        let warmup_args = WarmupArgs {
            service: "test".to_string(),
            start: false,
            status: true,
            stop: false,
        };
        let admin_warmup = AdminArgs {
            command: AdminSubcommand::Warmup(warmup_args),
        };
        assert!(matches!(admin_warmup.command, AdminSubcommand::Warmup(_)));
    }

    // ============================================================================
    // Metrics 命令测试
    // ============================================================================

    #[tokio::test]
    async fn test_metrics_args_parsing() {
        let args = MetricsArgs {
            service: Some("test_service".to_string()),
            prometheus: true,
            json: false,
        };

        assert_eq!(args.service, Some("test_service".to_string()));
        assert!(args.prometheus);
        assert!(!args.json);
    }

    #[tokio::test]
    async fn test_metrics_args_json_format() {
        let args = MetricsArgs {
            service: None,
            prometheus: false,
            json: true,
        };

        assert!(args.service.is_none());
        assert!(!args.prometheus);
        assert!(args.json);
    }

    #[tokio::test]
    async fn test_metrics_args_default() {
        let args = MetricsArgs {
            service: None,
            prometheus: false,
            json: false,
        };

        assert!(args.service.is_none());
        assert!(!args.prometheus);
        assert!(!args.json);
    }
}