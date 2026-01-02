//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存系统的遥测和链路追踪功能。

use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;
use tracing_subscriber::{layer::SubscriberExt, Registry};

/// 初始化 OpenTelemetry Tracing
///
/// 此函数应该在应用程序启动时调用一次。
/// 它配置全局 tracer provider 并设置 tracing subscriber。
///
/// # 参数
///
/// * `service_name` - 服务名称
/// * `endpoint` - OTLP 收集器端点 (例如 "http://localhost:4317")
///
/// # 返回值
///
/// 返回一个 Guard，当它被 drop 时，会关闭 tracer provider。
/// 但在此简化实现中，我们只是设置全局 subscriber。
pub fn init_tracing(service_name: &str, _endpoint: Option<&str>) {
    // 简单的控制台日志作为 fallback
    let subscriber = Registry::default();

    // 在实际生产环境中，这里会配置 OTLP exporter
    // 但为了避免引入过多的依赖和复杂性，我们这里先使用 stdout 或者 no-op
    // 如果需要完整的 OTLP 支持，需要配置 opentelemetry-otlp exporter

    // 这里演示如何创建一个 TracerProvider
    let provider = SdkTracerProvider::builder()
        // .with_simple_exporter(opentelemetry::trace::noop::NoopSpanExporter::new())
        // In 0.22, NoopSpanExporter is different or default.
        // Let's just use default builder which is no-op if no exporter.
        .build();

    // 设置全局 provider
    global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer(service_name.to_string());

    // 创建 telemetry layer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // 组合 layer
    let subscriber = subscriber.with(telemetry);

    // 设置全局 subscriber
    // 注意：这可能会与其他 tracing 初始化冲突，所以通常由应用层决定
    // 这里仅作为库提供的辅助函数
    let _ = tracing::subscriber::set_global_default(subscriber);
}
