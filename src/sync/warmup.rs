//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了缓存预热机制。
//! 始终可用（不依赖 batch-write feature）

use crate::config::{CacheWarmupConfig, WarmupDataSource};
use crate::error::Result;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

pub struct WarmupManager {
    service_name: String,
    config: CacheWarmupConfig,
    warmup_status: Arc<RwLock<HashMap<String, WarmupStatus>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WarmupStatus {
    Pending,
    InProgress { progress: usize, total: usize },
    Completed { loaded: usize, failed: usize },
    Failed { error: String },
}

#[derive(Debug, Clone)]
pub struct WarmupResult {
    pub loaded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub success: bool,
    pub error: Option<String>,
}

impl WarmupResult {
    pub fn skipped() -> Self {
        Self {
            loaded: 0,
            failed: 0,
            skipped: 1,
            success: true,
            error: None,
        }
    }

    pub fn failed(error: String) -> Self {
        Self {
            loaded: 0,
            failed: 0,
            skipped: 0,
            success: false,
            error: Some(error),
        }
    }
}

impl WarmupManager {
    pub fn new(service_name: String, config: CacheWarmupConfig) -> Self {
        Self {
            service_name,
            config,
            warmup_status: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run_warmup<F, Fut>(&self, load_fn: F) -> Result<WarmupResult>
    where
        F: Fn(Vec<String>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<HashMap<String, Vec<u8>>>> + Send,
    {
        info!(
            "Starting cache warmup for service: {}, enabled: {}",
            self.service_name, self.config.enabled
        );

        if !self.config.enabled {
            info!("Cache warmup is disabled, skipping");
            return Ok(WarmupResult::skipped());
        }

        let timeout = tokio::time::Duration::from_secs(self.config.timeout_seconds);
        let result = tokio::time::timeout(timeout, self.warmup_inner(load_fn)).await;

        match result {
            Ok(Ok(result)) => {
                info!(
                    "Cache warmup completed: loaded={}, failed={}, skipped={}",
                    result.loaded, result.failed, result.skipped
                );
                Ok(result)
            }
            Ok(Err(e)) => {
                warn!("Cache warmup failed: {}", e);
                Ok(WarmupResult::failed(e.to_string()))
            }
            Err(_) => {
                warn!(
                    "Cache warmup timed out after {} seconds",
                    self.config.timeout_seconds
                );
                Ok(WarmupResult::failed("timeout".to_string()))
            }
        }
    }

    async fn warmup_inner<F, Fut>(&self, load_fn: F) -> Result<WarmupResult>
    where
        F: Fn(Vec<String>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<HashMap<String, Vec<u8>>>> + Send,
    {
        let mut total_loaded = 0usize;
        let mut total_failed = 0usize;
        let mut total_skipped = 0usize;

        for source in &self.config.data_sources {
            info!("Loading keys from source: {:?}", source);

            let keys: Vec<String> = match source {
                WarmupDataSource::Static { keys } => keys.clone(),
                WarmupDataSource::RedisList { .. } => {
                    warn!("RedisList warmup source requires custom implementation");
                    total_skipped = total_skipped.saturating_add(1);
                    continue;
                }
                WarmupDataSource::Database { .. } => {
                    warn!("Database warmup source requires custom implementation");
                    total_skipped = total_skipped.saturating_add(1);
                    continue;
                }
                WarmupDataSource::Api { .. } => {
                    warn!("API warmup source requires custom implementation");
                    total_skipped = total_skipped.saturating_add(1);
                    continue;
                }
            };

            let keys_count = keys.len();
            debug!("Loaded {} keys from source", keys_count);

            let batch_size = self.config.batch_size;
            let interval_ms = self.config.batch_interval_ms;

            for chunk in keys.chunks(batch_size) {
                let chunk_keys: Vec<String> = chunk.to_vec();

                match load_fn(chunk_keys.clone()).await {
                    Ok(data_map) => {
                        let loaded = data_map.len();
                        let failed = chunk_keys.len().saturating_sub(loaded);
                        total_loaded = total_loaded.saturating_add(loaded);
                        total_failed = total_failed.saturating_add(failed);
                    }
                    Err(e) => {
                        warn!("Failed to load data batch: {}", e);
                        total_failed = total_failed.saturating_add(chunk_keys.len());
                    }
                }

                if interval_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
                }
            }
        }

        Ok(WarmupResult {
            loaded: total_loaded,
            failed: total_failed,
            skipped: total_skipped,
            success: total_failed == 0,
            error: None,
        })
    }

    pub async fn get_status(&self, source_type: &str) -> WarmupStatus {
        let status_map = self.warmup_status.read().await;
        status_map
            .get(source_type)
            .cloned()
            .unwrap_or(WarmupStatus::Pending)
    }
}

#[cfg(test)]
mod warmup_tests {
    use super::*;
    use crate::config::{CacheWarmupConfig, WarmupDataSource};
    use std::time::Duration;

    fn create_test_config() -> CacheWarmupConfig {
        CacheWarmupConfig {
            enabled: true,
            timeout_seconds: 30,
            batch_size: 10,
            batch_interval_ms: 10,
            data_sources: vec![WarmupDataSource::Static {
                keys: vec!["key1".to_string(), "key2".to_string()],
            }],
        }
    }

    #[tokio::test]
    async fn test_warmup_result_error_stored() {
        let result = WarmupResult::failed("test error".to_string());
        assert!(!result.success);
        assert_eq!(result.error, Some("test error".to_string()));
    }

    #[tokio::test]
    async fn test_warmup_result_skipped() {
        let result = WarmupResult::skipped();
        assert!(result.success);
        assert_eq!(result.skipped, 1);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_warmup_result_full() {
        let result = WarmupResult {
            loaded: 10,
            failed: 2,
            skipped: 1,
            success: false,
            error: Some("partial failure".to_string()),
        };
        assert_eq!(result.loaded, 10);
        assert_eq!(result.failed, 2);
        assert_eq!(result.skipped, 1);
        assert!(!result.success);
        assert_eq!(result.error, Some("partial failure".to_string()));
    }

    #[tokio::test]
    async fn test_warmup_disabled_returns_skipped() {
        let mut config = create_test_config();
        config.enabled = false;

        let manager = WarmupManager::new("test_service".to_string(), config);

        let result = manager
            .run_warmup(|_keys| async { Ok(HashMap::new()) })
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.skipped, 1);
    }

    #[tokio::test]
    async fn test_warmup_timeout_returns_error() {
        let config = CacheWarmupConfig {
            enabled: true,
            timeout_seconds: 0, // 立即超时
            batch_size: 100,
            batch_interval_ms: 0,
            data_sources: vec![WarmupDataSource::Static {
                keys: vec!["key1".to_string()],
            }],
        };

        let manager = WarmupManager::new("test_service".to_string(), config);

        let result = manager
            .run_warmup(|_keys| async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(HashMap::new())
            })
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("timeout"));
    }

    #[tokio::test]
    async fn test_warmup_loads_keys_successfully() {
        let config = create_test_config();

        let manager = WarmupManager::new("test_service".to_string(), config);

        let result = manager
            .run_warmup(|keys| async move {
                let mut map = HashMap::new();
                for key in keys {
                    map.insert(key, vec![1, 2, 3]);
                }
                Ok(map)
            })
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.loaded, 2);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn test_warmup_partial_failure() {
        let config = create_test_config();

        let manager = WarmupManager::new("test_service".to_string(), config);

        // 第一个key成功，第二个key失败
        let result = manager
            .run_warmup(|keys| async move {
                let mut map = HashMap::new();
                for (i, key) in keys.iter().enumerate() {
                    if i == 0 {
                        map.insert(key.clone(), vec![1, 2, 3]);
                    }
                    // 跳过第二个key
                }
                Ok(map)
            })
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.loaded, 1);
        assert_eq!(result.failed, 1);
    }

    #[tokio::test]
    async fn test_warmup_status_retrieval() {
        let config = create_test_config();
        let manager = WarmupManager::new("test_service".to_string(), config);

        let status = manager.get_status("static").await;
        assert_eq!(status, WarmupStatus::Pending);
    }

    #[tokio::test]
    async fn test_warmup_status_nonexistent_source() {
        let config = create_test_config();
        let manager = WarmupManager::new("test_service".to_string(), config);

        let status = manager.get_status("nonexistent").await;
        assert_eq!(status, WarmupStatus::Pending);
    }
}
