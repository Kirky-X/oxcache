// Copyright (c) 2025-2026, Kirky.X
//
// MIT License
//
// CLI Metrics Command Example
//
// This example demonstrates how to use the oxcache CLI metrics command
// to retrieve cache performance metrics in various formats.
//
// Note: Requires `cli` and `metrics` features.

use oxcache::config::{L1Config, L2Config, OxcacheConfig, RedisMode, ServiceConfig};
use oxcache::manager::init;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CLI Metrics Command Example");
    println!("============================\n");

    // Initialize cache service
    let config = OxcacheConfig::builder()
        .with_service(
            "default",
            ServiceConfig::two_level()
                .with_l1(L1Config::new().with_max_capacity(10000))
                .with_l2(
                    L2Config::new()
                        .with_mode(RedisMode::Standalone)
                        .with_connection_string("redis://127.0.0.1:6379"),
                ),
        )
        .build();

    let _ = init(config).await;

    println!("CLI Metrics Command Usage:");
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("1. Basic Metrics Query");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nShow metrics for all services:");
    println!("   $ oxcache metrics");
    println!("\nShow metrics for specific service:");
    println!("   $ oxcache metrics --service default");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("2. Output Formats");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nHuman-readable format (default):");
    println!("   $ oxcache metrics --service default");
    println!("\nPrometheus format:");
    println!("   $ oxcache metrics --service default --prometheus");
    println!("   → Compatible with Prometheus and Grafana");
    println!("\nJSON format:");
    println!("   $ oxcache metrics --service default --json");
    println!("   → Machine-readable, easy to parse");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("3. Example Output");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nHuman-readable format:");
    println!("─────────────────────────────────────────────────────");
    println!("Service: default");
    println!("");
    println!("L1 Cache:");
    println!("  Hits:           12,345");
    println!("  Misses:         567");
    println!("  Hit Rate:       95.6%");
    println!("  Size:           7,890 / 10,000 items");
    println!("  Evictions:      23");
    println!("");
    println!("L2 Cache:");
    println!("  Hits:           23,456");
    println!("  Misses:         789");
    println!("  Hit Rate:       96.7%");
    println!("  Size:           4,567 keys");
    println!("  Latency (P50):  1.2ms");
    println!("  Latency (P99):  5.8ms");
    println!("");
    println!("Operations:");
    println!("  Sets:           15,678");
    println!("  Gets:           38,901");
    println!("  Deletes:        234");
    println!("");
    println!("Errors:");
    println!("  L1 Errors:      0");
    println!("  L2 Errors:      12");
    println!("─────────────────────────────────────────────────────");

    println!("\nPrometheus format:");
    println!("─────────────────────────────────────────────────────");
    println!("oxcache_l1_hits{service=\"default\"} 12345");
    println!("oxcache_l1_misses{service=\"default\"} 567");
    println!("oxcache_l1_hit_rate{service=\"default\"} 0.956");
    println!("oxcache_l1_size{service=\"default\"} 7890");
    println!("oxcache_l2_hits{service=\"default\"} 23456");
    println!("oxcache_l2_misses{service=\"default\"} 789");
    println!("oxcache_l2_hit_rate{service=\"default\"} 0.967");
    println!("oxcache_l2_latency_p50{service=\"default\"} 0.0012");
    println!("oxcache_l2_latency_p99{service=\"default\"} 0.0058");
    println!("─────────────────────────────────────────────────────");

    println!("\nJSON format:");
    println!("─────────────────────────────────────────────────────");
    println!("{");
    println!("  \"service\": \"default\",");
    println!("  \"timestamp\": 1706328000,");
    println!("  \"l1\": {");
    println!("    \"hits\": 12345,");
    println!("    \"misses\": 567,");
    println!("    \"hit_rate\": 0.956,");
    println!("    \"size\": 7890");
    println!("  },");
    println!("  \"l2\": {");
    println!("    \"hits\": 23456,");
    println!("    \"misses\": 789,");
    println!("    \"hit_rate\": 0.967,");
    println!("    \"latency_p50\": 0.0012,");
    println!("    \"latency_p99\": 0.0058");
    println!("  }");
    println!("}");
    println!("─────────────────────────────────────────────────────");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("4. Integration Examples");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nExport to Prometheus:");
    println!("   $ oxcache metrics --prometheus | tee /tmp/metrics.prom");
    println!("   → Save metrics to file for Prometheus scraping\n");

    println!("Monitor with watch:");
    println!("   $ watch -n 5 'oxcache metrics --service default'");
    println!("   → Refresh metrics every 5 seconds\n");

    println!("Parse with jq:");
    println!("   $ oxcache metrics --json | jq '.l1.hit_rate'");
    println!("   → Extract specific metric values\n");

    println!("Grafana Integration:");
    println!("   1. Configure Prometheus to scrape metrics endpoint");
    println!("   2. Use oxcache metrics in Grafana dashboards");
    println!("   3. Monitor hit rates, latencies, and errors");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("5. Key Metrics Explained");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nHit Rate: (Hits / (Hits + Misses)) × 100%");
    println!("  → Higher is better, target > 90%");
    println!("\nLatency P50: Median response time");
    println!("  → Typical user experience");
    println!("\nLatency P99: 99th percentile response time");
    println!("  → Worst-case user experience");
    println!("\nEvictions: Items removed due to capacity");
    println!("  → High evictions may indicate need for larger cache");

    Ok(())
}
