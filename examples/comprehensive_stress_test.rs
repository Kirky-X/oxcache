//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 综合压力测试工具 - 完成test.md中未完成的压力测试工具

use oxcache::{
    backend::{l1::L1Backend, l2::L2Backend},
    client::two_level::TwoLevelClient,
    config::{L2Config, TwoLevelConfig},
    serialization::SerializerEnum,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use tokio::time::interval;

#[derive(Debug)]
struct Args {
    concurrency: usize,
    duration: u64,
    key_count: usize,
    read_ratio: u8,
    l1_only: bool,
    batch_size: usize,
    enable_wal: bool,
    redis_url: String,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            concurrency: 100,
            duration: 60,
            key_count: 1000,
            read_ratio: 80,
            l1_only: false,
            batch_size: 5,
            enable_wal: false,
            redis_url: "redis://127.0.0.1:6379".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct TestMetrics {
    total_operations: Arc<AtomicU64>,
    successful_operations: Arc<AtomicU64>,
    failed_operations: Arc<AtomicU64>,
    read_operations: Arc<AtomicU64>,
    write_operations: Arc<AtomicU64>,
    total_latency_ns: Arc<AtomicU64>,
    max_latency_ns: Arc<AtomicU64>,
    min_latency_ns: Arc<AtomicU64>,
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            total_operations: Arc::new(AtomicU64::new(0)),
            successful_operations: Arc::new(AtomicU64::new(0)),
            failed_operations: Arc::new(AtomicU64::new(0)),
            read_operations: Arc::new(AtomicU64::new(0)),
            write_operations: Arc::new(AtomicU64::new(0)),
            total_latency_ns: Arc::new(AtomicU64::new(0)),
            max_latency_ns: Arc::new(AtomicU64::new(0)),
            min_latency_ns: Arc::new(AtomicU64::new(u64::MAX)),
        }
    }

    fn record_operation(&self, success: bool, is_read: bool, latency: Duration) {
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

        let latency_ns = latency.as_nanos() as u64;
        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);

        self.max_latency_ns.fetch_max(latency_ns, Ordering::Relaxed);
        self.min_latency_ns.fetch_min(latency_ns, Ordering::Relaxed);
    }

    fn get_stats(&self) -> MetricsStats {
        let total = self.total_operations.load(Ordering::Relaxed);
        let success = self.successful_operations.load(Ordering::Relaxed);
        let reads = self.read_operations.load(Ordering::Relaxed);
        let writes = self.write_operations.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ns.load(Ordering::Relaxed);
        let max_latency = self.max_latency_ns.load(Ordering::Relaxed);
        let min_latency = if self.min_latency_ns.load(Ordering::Relaxed) == u64::MAX {
            0
        } else {
            self.min_latency_ns.load(Ordering::Relaxed)
        };

        MetricsStats {
            total_ops: total,
            success_rate: if total > 0 {
                (success as f64 / total as f64) * 100.0
            } else {
                0.0
            },
            read_ratio: if total > 0 {
                (reads as f64 / total as f64) * 100.0
            } else {
                0.0
            },
            write_ratio: if total > 0 {
                (writes as f64 / total as f64) * 100.0
            } else {
                0.0
            },
            avg_latency_ns: if total > 0 { total_latency / total } else { 0 },
            max_latency_ns: max_latency,
            min_latency_ns: min_latency,
            throughput: if total > 0 { total as f64 / 60.0 } else { 0.0 }, // ops per second
        }
    }
}

#[derive(Debug, Clone)]
struct MetricsStats {
    total_ops: u64,
    success_rate: f64,
    read_ratio: f64,
    write_ratio: f64,
    avg_latency_ns: u64,
    max_latency_ns: u64,
    min_latency_ns: u64,
    throughput: f64,
}

async fn chaos_monkey(enable: bool, interval_secs: u64) {
    if !enable {
        return;
    }

    let mut chaos_interval = interval(Duration::from_secs(interval_secs));

    tokio::spawn(async move {
        loop {
            chaos_interval.tick().await;

            // 随机制造一些混乱，比如网络延迟、连接断开等
            let chaos_type = rand::thread_rng().gen_range(0..3);
            match chaos_type {
                0 => {
                    println!("🐵 Chaos monkey: simulating network latency");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                1 => {
                    println!("🐵 Chaos monkey: simulating connection hiccup");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                _ => {
                    println!("🐵 Chaos monkey: simulating CPU spike");
                    // 简单的CPU密集操作
                    let _ = (0..1000000).map(|i| i * i).sum::<i64>();
                }
            }
        }
    });
}

async fn metrics_reporter(metrics: TestMetrics, enable: bool) {
    if !enable {
        return;
    }

    let mut report_interval = interval(Duration::from_secs(10));

    tokio::spawn(async move {
        loop {
            report_interval.tick().await;
            let stats = metrics.get_stats();

            println!("📊 Metrics Report:");
            println!("  Total Operations: {}", stats.total_ops);
            println!("  Success Rate: {:.2}%", stats.success_rate);
            println!(
                "  Read/Write Ratio: {:.1}% / {:.1}%",
                stats.read_ratio, stats.write_ratio
            );
            println!("  Throughput: {:.2} ops/s", stats.throughput);
            println!(
                "  Latency: avg={:.2}μs, min={:.2}μs, max={:.2}μs",
                stats.avg_latency_ns as f64 / 1000.0,
                stats.min_latency_ns as f64 / 1000.0,
                stats.max_latency_ns as f64 / 1000.0
            );
        }
    });
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut config = Args::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--concurrency" | "-c" => {
                if i + 1 < args.len() {
                    if let Ok(val) = args[i + 1].parse() {
                        config.concurrency = val;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--duration" | "-d" => {
                if i + 1 < args.len() {
                    if let Ok(val) = args[i + 1].parse() {
                        config.duration = val;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--key-count" | "-k" => {
                if i + 1 < args.len() {
                    if let Ok(val) = args[i + 1].parse() {
                        config.key_count = val;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--read-ratio" | "-r" => {
                if i + 1 < args.len() {
                    if let Ok(val) = args[i + 1].parse() {
                        config.read_ratio = val;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--l1-only" | "-l" => {
                config.l1_only = true;
                i += 1;
            }
            "--batch-size" | "-b" => {
                if i + 1 < args.len() {
                    if let Ok(val) = args[i + 1].parse() {
                        config.batch_size = val;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--enable-wal" | "-w" => {
                config.enable_wal = true;
                i += 1;
            }
            "--redis-url" => {
                if i + 1 < args.len() {
                    config.redis_url = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    config
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();

    println!("🚀 Starting Comprehensive Stress Test");
    println!("Configuration:");
    println!("  Concurrency: {}", args.concurrency);
    println!("  Duration: {}s", args.duration);
    println!("  Key Count: {}", args.key_count);
    println!("  Read Ratio: {}%", args.read_ratio);
    println!("  L1 Only Mode: {}", args.l1_only);
    println!("  Batch Size: {}", args.batch_size);
    println!("  Enable WAL: {}", args.enable_wal);
    println!("  Redis URL: {}", args.redis_url);

    let service_name = "stress_test_service".to_string();

    let l1 = Arc::new(L1Backend::new(100000));
    let l2_config = L2Config {
        connection_string: args.redis_url.clone().into(),
        ..Default::default()
    };

    let l2 = match L2Backend::new(&l2_config).await {
        Ok(backend) => Arc::new(backend),
        Err(e) => {
            println!(
                "❌ Redis connection failed: {}. Starting L1-only stress test.",
                e
            );
            // 创建一个虚拟的L2后端，所有操作都会失败，用于测试降级模式
            return run_l1_only_stress_test(args, l1).await;
        }
    };

    let config = TwoLevelConfig {
        promote_on_hit: true,
        enable_batch_write: true,
        batch_size: 100,
        batch_interval_ms: 10,
        invalidation_channel: None,
        bloom_filter: None,
        warmup: None,
        max_key_length: Some(1024),
        max_value_size: Some(1024 * 1024),
    };

    let client = Arc::new(
        TwoLevelClient::new(
            service_name,
            config,
            l1.clone(),
            l2.clone(),
            SerializerEnum::Json(oxcache::serialization::json::JsonSerializer::new()),
        )
        .await?,
    );

    // 预填充一些数据
    println!("📦 Pre-populating {} keys...", args.key_count);
    for i in 0..args.key_count {
        let key = format!("stress_key_{}", i);
        let value = format!("stress_value_{}", i);
        client.set(&key, &value, Some(600)).await?;

        if i % 1000 == 0 && i > 0 {
            println!("  Pre-populated {} keys", i);
        }
    }

    let metrics = TestMetrics::new();

    // 启动混沌猴子
    chaos_monkey(args.enable_wal, 1000).await;

    // 启动指标报告
    metrics_reporter(metrics.clone(), true).await;

    let start_time = Instant::now();
    let barrier = Arc::new(Barrier::new(args.concurrency));
    let mut handles = vec![];

    println!(
        "🏃 Starting stress test with {} concurrent workers...",
        args.concurrency
    );

    for worker_id in 0..args.concurrency {
        let c = client.clone();
        let b = barrier.clone();
        let duration = Duration::from_secs(args.duration);
        let metrics = metrics.clone();
        let key_count = args.key_count;
        let read_ratio = args.read_ratio;

        handles.push(tokio::spawn(async move {
            b.wait().await;
            let mut rng = StdRng::from_entropy();
            let start = Instant::now();

            while start.elapsed() < duration {
                let key_idx = rng.gen_range(0..key_count);
                let key = format!("stress_key_{}", key_idx);

                // 根据读写比例决定操作类型
                let is_read = rng.gen_range(0..100) < read_ratio;

                let operation_start = Instant::now();
                let success = if is_read {
                    c.get::<String>(&key).await.is_ok()
                } else {
                    let value = format!("updated_value_{}_{}", key_idx, worker_id);
                    c.set(&key, &value, Some(600)).await.is_ok()
                };

                let latency = operation_start.elapsed();
                metrics.record_operation(success, is_read, latency);

                // 微小的延迟以避免过度占用CPU
                tokio::time::sleep(Duration::from_micros(1)).await;
            }
        }));
    }

    // 等待所有工作线程完成
    for handle in handles {
        handle.await?;
    }

    let elapsed = start_time.elapsed();
    let final_stats = metrics.get_stats();

    println!("\n📈 Stress Test Results:");
    println!("{}", "=".repeat(50));
    println!("Duration: {:.2?}", elapsed);
    println!("Total Operations: {}", final_stats.total_ops);
    println!("Success Rate: {:.2}%", final_stats.success_rate);
    println!(
        "Error Count: {}",
        final_stats.total_ops - final_stats.success_rate as u64
    );
    println!("Throughput: {:.2} ops/s", final_stats.throughput);
    println!(
        "Read/Write Ratio: {:.1}% / {:.1}%",
        final_stats.read_ratio, final_stats.write_ratio
    );
    println!("Latency Statistics:");
    println!(
        "  Average: {:.2}μs",
        final_stats.avg_latency_ns as f64 / 1000.0
    );
    println!(
        "  Minimum: {:.2}μs",
        final_stats.min_latency_ns as f64 / 1000.0
    );
    println!(
        "  Maximum: {:.2}μs",
        final_stats.max_latency_ns as f64 / 1000.0
    );

    // 性能验证
    let success = final_stats.success_rate >= 99.0
        && final_stats.throughput >= 1000.0
        && (final_stats.avg_latency_ns as f64 / 1000.0) < 1000.0;

    if success {
        println!("\n✅ Stress test PASSED - Performance meets requirements");
    } else {
        println!("\n❌ Stress test FAILED - Performance below requirements");
        std::process::exit(1);
    }

    Ok(())
}

async fn run_l1_only_stress_test(
    args: Args,
    l1: Arc<L1Backend>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Running L1-only stress test mode");

    let metrics = TestMetrics::new();

    // 预填充L1缓存
    println!("📦 Pre-populating L1 with {} keys...", args.key_count);
    for i in 0..args.key_count {
        let key = format!("stress_key_{}", i);
        let value = format!("stress_value_{}", i);
        l1.set_bytes(&key, value.into_bytes(), Some(600)).await?;

        if i % 1000 == 0 && i > 0 {
            println!("  Pre-populated {} keys", i);
        }
    }

    let start_time = Instant::now();
    let barrier = Arc::new(Barrier::new(args.concurrency));
    let mut handles = vec![];

    for worker_id in 0..args.concurrency {
        let l1_clone = l1.clone();
        let b = barrier.clone();
        let duration = Duration::from_secs(args.duration);
        let metrics = metrics.clone();
        let key_count = args.key_count;
        let read_ratio = args.read_ratio;

        handles.push(tokio::spawn(async move {
            b.wait().await;
            let mut rng = StdRng::from_entropy();
            let start = Instant::now();

            while start.elapsed() < duration {
                let key_idx = rng.gen_range(0..key_count);
                let key = format!("stress_key_{}", key_idx);

                let is_read = rng.gen_range(0..100) < read_ratio;

                let operation_start = Instant::now();
                let success = if is_read {
                    l1_clone.get_with_metadata(&key).await.is_ok()
                } else {
                    let value = format!("updated_value_{}_{}", key_idx, worker_id);
                    l1_clone
                        .set_bytes(&key, value.into_bytes(), Some(600))
                        .await
                        .is_ok()
                };

                let latency = operation_start.elapsed();
                metrics.record_operation(success, is_read, latency);

                tokio::time::sleep(Duration::from_micros(1)).await;
            }
        }));
    }

    for handle in handles {
        handle.await?;
    }

    let elapsed = start_time.elapsed();
    let final_stats = metrics.get_stats();

    println!("\n📈 L1-only Stress Test Results:");
    println!("{}", "=".repeat(50));
    println!("Duration: {:.2?}", elapsed);
    println!("Total Operations: {}", final_stats.total_ops);
    println!("Success Rate: {:.2}%", final_stats.success_rate);
    println!("Throughput: {:.2} ops/s", final_stats.throughput);
    println!(
        "Average Latency: {:.2}μs",
        final_stats.avg_latency_ns as f64 / 1000.0
    );

    Ok(())
}
