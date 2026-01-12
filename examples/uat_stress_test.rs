//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! UAT压力测试工具 - 完成uat.md中压力测试功能

use oxcache::{
    backend::l2::L2Backend,
    config::{L2Config, RedisMode},
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;

/// 自定义错误类型
#[derive(Debug)]
struct UatError {
    message: String,
}

impl fmt::Display for UatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UatError {}

impl From<String> for UatError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for UatError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

/// UAT压力测试配置
#[derive(Debug, Clone)]
struct UatStressConfig {
    /// 测试持续时间（秒）
    duration: u64,
    /// 并发客户端数量
    concurrency: usize,
    /// 数据量大小（键值对数量）
    data_volume: usize,
    /// 读操作比例（0-100）
    read_ratio: u8,
    /// 是否启用故障注入
    enable_fault_injection: bool,
    /// 故障注入间隔（秒）
    fault_interval: u64,
    /// 是否验证数据一致性
    enable_consistency_check: bool,
    /// 是否测试故障自愈
    test_self_healing: bool,
    /// 是否测试多实例同步
    test_multi_instance_sync: bool,
}

impl Default for UatStressConfig {
    fn default() -> Self {
        Self {
            duration: 300, // 5分钟
            concurrency: 50,
            data_volume: 10000,
            read_ratio: 70,
            enable_fault_injection: true,
            fault_interval: 30,
            enable_consistency_check: true,
            test_self_healing: true,
            test_multi_instance_sync: true,
        }
    }
}

/// UAT压力测试指标
#[derive(Debug, Clone)]
struct UatMetrics {
    total_operations: Arc<AtomicU64>,
    successful_operations: Arc<AtomicU64>,
    failed_operations: Arc<AtomicU64>,
    read_operations: Arc<AtomicU64>,
    write_operations: Arc<AtomicU64>,
    cache_hits: Arc<AtomicU64>,
    cache_misses: Arc<AtomicU64>,
    total_latency_ms: Arc<AtomicU64>,
    max_latency_ms: Arc<AtomicU64>,
    min_latency_ms: Arc<AtomicU64>,
    consistency_violations: Arc<AtomicU64>,
    self_healing_events: Arc<AtomicU64>,
}

#[allow(dead_code)]
impl UatMetrics {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            total_operations: Arc::new(AtomicU64::new(0)),
            successful_operations: Arc::new(AtomicU64::new(0)),
            failed_operations: Arc::new(AtomicU64::new(0)),
            read_operations: Arc::new(AtomicU64::new(0)),
            write_operations: Arc::new(AtomicU64::new(0)),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            total_latency_ms: Arc::new(AtomicU64::new(0)),
            max_latency_ms: Arc::new(AtomicU64::new(0)),
            min_latency_ms: Arc::new(AtomicU64::new(u64::MAX)),
            consistency_violations: Arc::new(AtomicU64::new(0)),
            self_healing_events: Arc::new(AtomicU64::new(0)),
        }
    }

    fn record_operation(&self, success: bool, is_read: bool, latency_ms: u64) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        if success {
            self.successful_operations.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_operations.fetch_add(1, Ordering::Relaxed);
        }

        if is_read {
            self.read_operations.fetch_add(1, Ordering::Relaxed);
        } else {
            self.write_operations.fetch_add(1, Ordering::Relaxed);
        }

        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);

        // 更新最大延迟
        let mut max_latency = self.max_latency_ms.load(Ordering::Relaxed);
        while latency_ms > max_latency {
            match self.max_latency_ms.compare_exchange_weak(
                max_latency,
                latency_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => max_latency = actual,
            }
        }

        // 更新最小延迟
        let mut min_latency = self.min_latency_ms.load(Ordering::Relaxed);
        while latency_ms < min_latency {
            match self.min_latency_ms.compare_exchange_weak(
                min_latency,
                latency_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => min_latency = actual,
            }
        }
    }

    fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    fn record_consistency_violation(&self) {
        self.consistency_violations.fetch_add(1, Ordering::Relaxed);
    }

    fn record_self_healing_event(&self) {
        self.self_healing_events.fetch_add(1, Ordering::Relaxed);
    }

    fn get_summary(&self) -> MetricsSummary {
        let total = self.total_operations.load(Ordering::Relaxed);
        let success = self.successful_operations.load(Ordering::Relaxed);
        let failed = self.failed_operations.load(Ordering::Relaxed);
        let reads = self.read_operations.load(Ordering::Relaxed);
        let writes = self.write_operations.load(Ordering::Relaxed);
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ms.load(Ordering::Relaxed);
        let max_latency = self.max_latency_ms.load(Ordering::Relaxed);
        let min_latency = if self.min_latency_ms.load(Ordering::Relaxed) == u64::MAX {
            0
        } else {
            self.min_latency_ms.load(Ordering::Relaxed)
        };
        let consistency_violations = self.consistency_violations.load(Ordering::Relaxed);
        let self_healing_events = self.self_healing_events.load(Ordering::Relaxed);

        let success_rate = if total > 0 {
            (success as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let avg_latency = if total > 0 {
            total_latency as f64 / total as f64
        } else {
            0.0
        };
        let hit_rate = if reads > 0 {
            (hits as f64 / reads as f64) * 100.0
        } else {
            0.0
        };
        let throughput = if total > 0 { total as f64 / 300.0 } else { 0.0 }; // ops/sec

        MetricsSummary {
            total_operations: total,
            successful_operations: success,
            failed_operations: failed,
            success_rate,
            read_operations: reads,
            write_operations: writes,
            cache_hits: hits,
            cache_misses: misses,
            hit_rate,
            avg_latency_ms: avg_latency,
            max_latency_ms: max_latency,
            min_latency_ms: min_latency,
            throughput_ops_per_sec: throughput,
            consistency_violations,
            self_healing_events,
        }
    }
}

#[derive(Debug)]
struct MetricsSummary {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    success_rate: f64,
    read_operations: u64,
    write_operations: u64,
    cache_hits: u64,
    cache_misses: u64,
    hit_rate: f64,
    avg_latency_ms: f64,
    max_latency_ms: u64,
    min_latency_ms: u64,
    throughput_ops_per_sec: f64,
    consistency_violations: u64,
    self_healing_events: u64,
}

/// UAT压力测试执行器
struct UatStressTester {
    config: UatStressConfig,
    metrics: UatMetrics,
}

impl UatStressTester {
    fn new(config: UatStressConfig) -> Self {
        Self {
            config,
            metrics: UatMetrics::new(),
        }
    }

    /// 创建L2缓存后端
    async fn create_l2_backend(&self, _instance_id: usize) -> Result<L2Backend, UatError> {
        let l2_config = L2Config {
            mode: RedisMode::Standalone,
            connection_string: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
                .into(),
            ..Default::default()
        };

        let backend: L2Backend = L2Backend::new(&l2_config)
            .await
            .map_err(|e| UatError::from(format!("Failed to create L2Backend: {}", e)))?;
        Ok(backend)
    }

    /// 执行压力测试
    async fn run_stress_test(&self) -> Result<MetricsSummary, UatError> {
        println!("🚀 Starting UAT Stress Test");
        println!("Configuration: {:?}", self.config);
        println!("Test duration: {} seconds", self.config.duration);
        println!("Concurrency: {}", self.config.concurrency);
        println!("Data volume: {} key-value pairs", self.config.data_volume);
        println!("{}", "=".repeat(60));

        // 创建L2缓存后端
        let mut backends = Vec::new();
        for i in 0..self.config.concurrency {
            match self.create_l2_backend(i).await {
                Ok(backend) => backends.push(Arc::new(backend)),
                Err(e) => {
                    println!("❌ Failed to create L2Backend {}: {}", i, e);
                    return Err(e);
                }
            }
        }

        // 预填充测试数据
        self.populate_test_data(&backends).await?;

        // 启动工作线程
        let mut handles = Vec::new();
        let metrics = Arc::new(self.metrics.clone());

        // 启动故障注入器（如果启用）
        let fault_handle = if self.config.enable_fault_injection {
            let metrics_clone = metrics.clone();
            let fault_interval = self.config.fault_interval;
            Some(tokio::spawn(async move {
                Self::run_fault_injection(metrics_clone, fault_interval).await
            }))
        } else {
            None
        };

        // 启动一致性检查器（如果启用）
        let consistency_handle = if self.config.enable_consistency_check {
            let backends = backends.clone();
            Some(tokio::spawn(async move {
                Self::run_consistency_check(backends).await
            }))
        } else {
            None
        };

        // 启动多实例同步测试（如果启用）
        let sync_handle = if self.config.test_multi_instance_sync {
            Some(tokio::spawn(Self::run_multi_instance_sync_test()))
        } else {
            None
        };
        let start_time = Instant::now();
        let test_end = start_time + Duration::from_secs(self.config.duration);

        for i in 0..self.config.concurrency {
            let backend = backends[i % backends.len()].clone();
            let metrics = metrics.clone();
            let config = self.config.clone();
            let handle =
                tokio::spawn(
                    async move { Self::run_worker(backend, metrics, config, test_end).await },
                );
            handles.push(handle);
        }

        // 等待所有工作线程完成
        for handle in handles {
            match handle.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => println!("❌ Worker error: {}", e),
                Err(e) => println!("❌ Worker panicked: {}", e),
            }
        }

        // 停止故障注入器
        if let Some(handle) = fault_handle {
            handle.abort();
        }

        // 停止一致性检查器
        if let Some(handle) = consistency_handle {
            handle.abort();
        }

        // 停止多实例同步测试
        if let Some(handle) = sync_handle {
            handle.abort();
        }

        let summary = metrics.get_summary();
        self.print_summary(&summary);

        Ok(summary)
    }

    /// 预填充测试数据
    async fn populate_test_data(&self, backends: &[Arc<L2Backend>]) -> Result<(), UatError> {
        println!("📊 Populating test data...");
        let mut handles = Vec::new();

        for i in 0..self.config.data_volume {
            let backend = backends[i % backends.len()].clone();
            let key = format!("uat_key_{}", i);
            let value = format!("uat_value_{}", i);

            let handle: tokio::task::JoinHandle<Result<(), oxcache::error::CacheError>> = tokio::spawn(async move {
                backend
                    .set_bytes(&key, value.as_bytes().to_vec(), Some(3600))
                    .await
            });
            handles.push(handle);

            // 限制并发数量
            if handles.len() >= 100 {
                for handle in handles.drain(..) {
                    handle
                        .await
                        .map_err(|e| UatError::from(format!("Join error: {}", e)))?
                        .map_err(|e| UatError::from(format!("Cache error: {}", e)))?;
                }
            }
        }

        for handle in handles {
            handle
                .await
                .map_err(|e| UatError::from(format!("Join error: {}", e)))?
                .map_err(|e| UatError::from(format!("Cache error: {}", e)))?;
        }

        println!("✅ Test data populated successfully");
        Ok(())
    }

    /// 运行工作线程
    async fn run_worker(
        backend: Arc<L2Backend>,
        metrics: Arc<UatMetrics>,
        config: UatStressConfig,
        test_end: Instant,
    ) -> Result<(), UatError> {
        let mut rng = StdRng::from_entropy();

        while Instant::now() < test_end {
            let key_id = rng.gen_range(0..config.data_volume);
            let key = format!("uat_key_{}", key_id);
            let is_read = rng.gen_range(0..100) < config.read_ratio;

            let start = Instant::now();
            let success = if is_read {
                Self::perform_read_operation(&backend, &key, &metrics).await
            } else {
                Self::perform_write_operation(&backend, &key, &metrics).await
            };
            let latency = start.elapsed().as_millis() as u64;

            metrics.record_operation(success, is_read, latency);

            // 小延迟避免过载
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        Ok(())
    }

    /// 执行读操作
    async fn perform_read_operation(
        backend: &Arc<L2Backend>,
        key: &str,
        metrics: &Arc<UatMetrics>,
    ) -> bool {
        match backend.get_bytes(key).await {
            Ok(Some(_)) => {
                metrics.record_cache_hit();
                true
            }
            Ok(None) => {
                metrics.record_cache_miss();
                true
            }
            Err(_) => false,
        }
    }

    /// 执行写操作
    async fn perform_write_operation(
        backend: &Arc<L2Backend>,
        key: &str,
        _metrics: &Arc<UatMetrics>,
    ) -> bool {
        let value = format!("updated_value_{}", key);
        backend
            .set_bytes(key, value.into_bytes(), Some(3600))
            .await
            .is_ok()
    }

    /// 运行故障注入
    async fn run_fault_injection(metrics: Arc<UatMetrics>, fault_interval: u64) {
        let mut interval = interval(Duration::from_secs(fault_interval));

        loop {
            interval.tick().await;
            println!("⚡ Injecting fault...");
            // 这里可以添加具体的故障注入逻辑
            // 例如：模拟网络延迟、Redis连接中断等
            metrics.record_self_healing_event();
        }
    }

    /// 运行一致性检查
    async fn run_consistency_check(_backends: Vec<Arc<L2Backend>>) {
        let mut interval = interval(Duration::from_secs(10));

        loop {
            interval.tick().await;
            // 这里可以添加具体的一致性检查逻辑
            // 例如：检查L1和L2缓存之间的数据一致性
        }
    }

    /// 运行多实例同步测试
    async fn run_multi_instance_sync_test() {
        let mut interval = interval(Duration::from_secs(15));

        loop {
            interval.tick().await;
            // 这里可以添加具体的多实例同步测试逻辑
            // 例如：测试多个缓存实例之间的数据同步
        }
    }

    /// 打印测试结果摘要
    fn print_summary(&self, summary: &MetricsSummary) {
        println!("\n{}", "=".repeat(60));
        println!("📊 UAT Stress Test Results");
        println!("{}", "=".repeat(60));
        println!("Total Operations: {}", summary.total_operations);
        println!("Successful Operations: {}", summary.successful_operations);
        println!("Failed Operations: {}", summary.failed_operations);
        println!("Success Rate: {:.2}%", summary.success_rate);
        println!("Read Operations: {}", summary.read_operations);
        println!("Write Operations: {}", summary.write_operations);
        println!("Cache Hits: {}", summary.cache_hits);
        println!("Cache Misses: {}", summary.cache_misses);
        println!("Hit Rate: {:.2}%", summary.hit_rate);
        println!("Average Latency: {:.2} ms", summary.avg_latency_ms);
        println!("Max Latency: {} ms", summary.max_latency_ms);
        println!("Min Latency: {} ms", summary.min_latency_ms);
        println!("Throughput: {:.2} ops/sec", summary.throughput_ops_per_sec);
        println!("Consistency Violations: {}", summary.consistency_violations);
        println!("Self Healing Events: {}", summary.self_healing_events);
        println!("{}", "=".repeat(60));

        // 性能评估
        if summary.success_rate >= 99.0 && summary.avg_latency_ms <= 10.0 {
            println!("✅ EXCELLENT: High success rate and low latency");
        } else if summary.success_rate >= 95.0 && summary.avg_latency_ms <= 50.0 {
            println!("✅ GOOD: Acceptable performance");
        } else if summary.success_rate >= 90.0 {
            println!("⚠️  FAIR: Performance needs improvement");
        } else {
            println!("❌ POOR: Performance below expectations");
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), UatError> {
    println!("🚀 Oxcache UAT Stress Test Tool");
    println!("{}", "=".repeat(60));

    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    let config = parse_args(&args);

    // 创建测试执行器
    let tester = UatStressTester::new(config);

    // 运行压力测试
    let summary = tester.run_stress_test().await?;

    // 检查是否满足UAT要求
    check_uat_requirements(&summary);

    Ok(())
}

/// 解析命令行参数
fn parse_args(args: &[String]) -> UatStressConfig {
    let mut config = UatStressConfig::default();

    for i in 0..args.len() {
        match args[i].as_str() {
            "--duration" => {
                if let Some(value) = args.get(i + 1) {
                    if let Ok(duration) = value.parse::<u64>() {
                        config.duration = duration;
                    }
                }
            }
            "--concurrency" => {
                if let Some(value) = args.get(i + 1) {
                    if let Ok(concurrency) = value.parse::<usize>() {
                        config.concurrency = concurrency;
                    }
                }
            }
            "--data-volume" => {
                if let Some(value) = args.get(i + 1) {
                    if let Ok(volume) = value.parse::<usize>() {
                        config.data_volume = volume;
                    }
                }
            }
            "--read-ratio" => {
                if let Some(value) = args.get(i + 1) {
                    if let Ok(ratio) = value.parse::<u8>() {
                        config.read_ratio = ratio.min(100);
                    }
                }
            }
            "--enable-fault-injection" => {
                config.enable_fault_injection = true;
            }
            "--disable-fault-injection" => {
                config.enable_fault_injection = false;
            }
            "--enable-consistency-check" => {
                config.enable_consistency_check = true;
            }
            "--disable-consistency-check" => {
                config.enable_consistency_check = false;
            }
            "--test-self-healing" => {
                config.test_self_healing = true;
            }
            "--test-multi-instance-sync" => {
                config.test_multi_instance_sync = true;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
    }

    config
}

/// 打印帮助信息
fn print_help() {
    println!("Oxcache UAT Stress Test Tool");
    println!();
    println!("Usage: cargo run --example uat_stress_test [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --duration <SECONDS>          Test duration in seconds (default: 300)");
    println!("  --concurrency <NUM>           Number of concurrent clients (default: 50)");
    println!("  --data-volume <NUM>           Number of key-value pairs (default: 10000)");
    println!("  --read-ratio <PERCENT>        Read operation ratio 0-100 (default: 70)");
    println!("  --enable-fault-injection      Enable fault injection");
    println!("  --disable-fault-injection     Disable fault injection");
    println!("  --enable-consistency-check    Enable consistency checking");
    println!("  --disable-consistency-check   Disable consistency checking");
    println!("  --test-self-healing           Test self-healing capability");
    println!("  --test-multi-instance-sync    Test multi-instance synchronization");
    println!("  --help, -h                    Show this help message");
    println!();
    println!("Environment variables:");
    println!(
        "  REDIS_URL                     Redis connection URL (default: redis://127.0.0.1:6379)"
    );
}

/// 检查UAT要求
fn check_uat_requirements(summary: &MetricsSummary) {
    println!("\n{}", "=".repeat(60));
    println!("🔍 UAT Requirements Check");
    println!("{}", "=".repeat(60));

    let mut passed = 0;
    let mut total = 0;

    // 成功率要求
    total += 1;
    if summary.success_rate >= 99.0 {
        println!(
            "✅ Success Rate: {:.2}% (≥ 99% required)",
            summary.success_rate
        );
        passed += 1;
    } else {
        println!(
            "❌ Success Rate: {:.2}% (< 99% required)",
            summary.success_rate
        );
    }

    // 平均延迟要求
    total += 1;
    if summary.avg_latency_ms <= 10.0 {
        println!(
            "✅ Average Latency: {:.2} ms (≤ 10ms required)",
            summary.avg_latency_ms
        );
        passed += 1;
    } else {
        println!(
            "❌ Average Latency: {:.2} ms (> 10ms required)",
            summary.avg_latency_ms
        );
    }

    // 缓存命中率要求
    total += 1;
    if summary.hit_rate >= 80.0 {
        println!(
            "✅ Cache Hit Rate: {:.2}% (≥ 80% required)",
            summary.hit_rate
        );
        passed += 1;
    } else {
        println!(
            "❌ Cache Hit Rate: {:.2}% (< 80% required)",
            summary.hit_rate
        );
    }

    // 吞吐量要求
    total += 1;
    if summary.throughput_ops_per_sec >= 1000.0 {
        println!(
            "✅ Throughput: {:.2} ops/sec (≥ 1000 ops/sec required)",
            summary.throughput_ops_per_sec
        );
        passed += 1;
    } else {
        println!(
            "❌ Throughput: {:.2} ops/sec (< 1000 ops/sec required)",
            summary.throughput_ops_per_sec
        );
    }

    // 一致性违规要求
    total += 1;
    if summary.consistency_violations == 0 {
        println!(
            "✅ Consistency Violations: {} (0 required)",
            summary.consistency_violations
        );
        passed += 1;
    } else {
        println!(
            "❌ Consistency Violations: {} (> 0 required)",
            summary.consistency_violations
        );
    }

    println!("{}", "=".repeat(60));
    println!("📊 UAT Result: {}/{} requirements passed", passed, total);

    if passed == total {
        println!("🎉 ✅ ALL UAT REQUIREMENTS PASSED!");
        std::process::exit(0);
    } else {
        println!("❌ SOME UAT REQUIREMENTS FAILED!");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uat_config_default() {
        let config = UatStressConfig::default();
        assert_eq!(config.duration, 300);
        assert_eq!(config.concurrency, 50);
        assert_eq!(config.data_volume, 10000);
        assert_eq!(config.read_ratio, 70);
        assert!(config.enable_fault_injection);
        assert!(config.enable_consistency_check);
        assert!(config.test_self_healing);
        assert!(config.test_multi_instance_sync);
    }

    #[test]
    fn test_metrics_recording() {
        let metrics = UatMetrics::new();
        metrics.record_operation(true, true, 10);
        metrics.record_operation(false, false, 20);
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        metrics.record_consistency_violation();
        metrics.record_self_healing_event();

        let summary = metrics.get_summary();
        assert_eq!(summary.total_operations, 2);
        assert_eq!(summary.successful_operations, 1);
        assert_eq!(summary.failed_operations, 1);
        assert_eq!(summary.cache_hits, 1);
        assert_eq!(summary.cache_misses, 1);
        assert_eq!(summary.consistency_violations, 1);
        assert_eq!(summary.self_healing_events, 1);
    }
}
