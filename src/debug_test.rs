//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块包含调试测试代码。

#[cfg(test)]
mod debug_partition_tests {
    use chrono::{Datelike, TimeZone, Timelike};

    #[test]
    fn debug_partition_dates() {
        let now = chrono::Utc::now();
        let old_date = now - chrono::Duration::days(100); // ~3 months ago
        let recent_date = now - chrono::Duration::days(30); // ~1 month ago
        let cutoff_date = now - chrono::Duration::days((2 * 30) as i64); // 2 months ago

        println!("Now: {}", now);
        println!("Old date (100 days ago): {}", old_date);
        println!("Recent date (30 days ago): {}", recent_date);
        println!("Cutoff date (2 months ago): {}", cutoff_date);

        // Create PartitionInfo for recent partition
        let recent_start = recent_date
            .with_day(1)
            .unwrap()
            .with_hour(0)
            .unwrap()
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap();

        let recent_end = if recent_date.month() == 12 {
            chrono::Utc
                .with_ymd_and_hms(recent_date.year() + 1, 1, 1, 0, 0, 0)
                .unwrap()
        } else {
            chrono::Utc
                .with_ymd_and_hms(recent_date.year(), recent_date.month() + 1, 1, 0, 0, 0)
                .unwrap()
        };

        println!("Recent partition start: {}", recent_start);
        println!("Recent partition end: {}", recent_end);
        println!("Recent end < cutoff? {}", recent_end < cutoff_date);
    }
}
